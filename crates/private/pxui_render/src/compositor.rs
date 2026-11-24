//! GPU composition for the core's qualified solid fractional fills. The texture holds the straight
//! fill RGB. The same rectangle shader applies transformed edge coverage, clip and opacity as on
//! the canvas, avoiding a filtered or pre-multiplied second approximation of those edges.
use super::{Break, FRAMES, Instance, Painter, backend, invalid};
use super::{BLEND_LAYER_SHADERS, UI_SLOTS, pipeline};
use ogpu_core::pipeline::{Blend, ColorFormat, DepthFormat};
use ogpu_core::backend::{ColorAttachment, Commands, RenderEncoder, RenderTarget, Texture};
use ogpu_core::error::Error;
use ogpu_core::render_pass::{Load, Store};
use ogpu_core::types::{TextureDescriptor, TextureFormat, TextureUsage};
use pxui_core::ui::FrameOut;
use pxui_types::geom::Rect;

pub(super) const LIMIT: usize = 16;
const RETAINED_FILL: u8 = 8;
const RETAINED_CANVAS: u8 = 24;

pub(super) struct Surface {
    target: Option<backend::PreparedRenderTarget>,
    texture: Option<Texture>,
    size: [u32; 2],
    view: u16,
    revision: Option<u64>,
}

impl Drop for Surface {
    fn drop(&mut self) {
        drop(self.target.take());
        if let Some(texture) = self.texture.take() { let _ = backend::destroy_texture(&texture); }
    }
}

impl Painter {
    /// Forget writes from abandoned commands while keeping the allocated surfaces available.
    pub fn invalidate_composition(&mut self) {
        for surface in self.layer_surfaces.iter_mut().flatten() { surface.revision = None; }
    }

    /// Render new or revised content before opening the canvas or composite pass.
    pub fn prepare_composition(
        &mut self, commands: &Commands, out: &FrameOut<'_>, density: f32, format: TextureFormat,
    ) -> Result<(), Error> {
        // Color surfaces use the backend's portable RGBA format even on BGRA drawables.
        if format == TextureFormat::Bgra8Unorm && self.layer_pipeline.is_none() {
            self.layer_pipeline = Some(pipeline(&self.device, ColorFormat::Rgba8Unorm, DepthFormat::NoDepth, 1,
                BLEND_LAYER_SHADERS, ["ui_vertex", "ui_fragment"], &UI_SLOTS, Blend::Alpha)?);
        }
        for run in out.promoted {
            let size = [run.swept[2], run.swept[3]].map(|v| (v * density).ceil().max(1.0) as u32);
            let slot = self.layer_surfaces.iter().position(|slot| slot.as_ref().is_some_and(|s| s.view == run.view))
                .or_else(|| self.layer_surfaces.iter().position(Option::is_none))
                .or_else(|| self.layer_surfaces.iter().position(|slot| slot.as_ref().is_some_and(|s| {
                    !out.promoted.iter().any(|run| run.view == s.view)
                })))
                .ok_or_else(|| invalid("Painter.prepare_composition", "promotion exceeds surface capacity"))?;
            if !self.layer_surfaces[slot].as_ref().is_some_and(|s| s.size[0] >= size[0] && s.size[1] >= size[1]) {
                let texture = backend::create_texture(&self.device, TextureDescriptor {
                    label: Some("pxui.layer.content".into()),
                    width: i64::from(size[0]), height: i64::from(size[1]), depth: 1, mip_levels: 1, sample_count: 1,
                    format: TextureFormat::Rgba8Unorm, usage: vec![TextureUsage::RenderAttachment, TextureUsage::TextureBinding],
                })?;
                let mut surface = Surface { target: None, texture: Some(texture.clone()), size, view: run.view, revision: None };
                surface.target = Some(backend::prepare_render_target(&self.device, RenderTarget {
                    colors: vec![ColorAttachment { texture, resolve: None, load: Load::DontCare, store: Store::Store,
                        clear: (0.0, 0.0, 0.0, 0.0) }],
                    depth: None,
                })?);
                if let Some(mut old) = self.layer_surfaces[slot].replace(surface) {
                    drop(old.target.take());
                    self.retired_textures.push((old.texture.take().expect("live layer surface"), FRAMES));
                }
            }
            let surface = self.layer_surfaces[slot].as_mut().expect("allocated above");
            if surface.view != run.view || surface.revision != Some(run.content_revision) {
                surface.view = run.view;
                surface.revision = None;
                let encoder = backend::render_prepared(commands, surface.target.as_mut().expect("prepared layer target"))?;
                let [r, g, b, _] = run.color;
                let size = surface.size;
                let fill = Instance::rect([0.0, 0.0, size[0] as f32, size[1] as f32], [r, g, b, 255]);
                let painted = self.draw_segments(&encoder, size, 1.0, &[fill], &[], None, &[], true);
                let closed = backend::end_render(&encoder);
                painted.and(closed)?;
                self.layer_surfaces[slot].as_mut().expect("live layer surface").revision = Some(run.content_revision);
            }
        }
        Ok(())
    }

    /// Draw the textures in their retained z order. The destination already contains the canvas.
    pub fn composite(
        &mut self, encoder: &RenderEncoder, size: [u32; 2], scale: f32, out: &FrameOut<'_>,
        canvas: Option<(&Texture, [u32; 4])>,
    ) -> Result<(), Error> {
        let viewport = Rect::new(0.0, 0.0, size[0] as f32, size[1] as f32);
        let mut rects = [[0; 4]; LIMIT + 1];
        let mut instances = [Instance::DEFAULT; LIMIT + 1];
        let mut breaks = std::array::from_fn::<_, { LIMIT + 1 }, _>(|_| Break { at_instance: 0, texture: None });
        let mut len = 0;
        let mut clip = viewport;
        if let Some((texture, rect)) = canvas {
            let [x, y, w, h] = rect.map(|v| v as f32);
            clip = Rect::new(x, y, w, h);
            let mut copy = Instance::rect([0.0, 0.0, viewport.w / scale, viewport.h / scale], [255; 4]);
            copy.flags = RETAINED_CANVAS;
            instances[0] = copy;
            rects[0] = rect;
            breaks[0].texture = Some(texture.clone());
            len = 1;
        }
        for run in out.promoted {
            let [x, y, w, h] = run.swept;
            let (x0, y0, x1, y1) = ((x * scale).floor(), (y * scale).floor(),
                ((x + w) * scale).ceil(), ((y + h) * scale).ceil());
            let cover = Rect::new(x0, y0, x1 - x0, y1 - y0).intersect(clip);
            if cover.w <= 0.0 || cover.h <= 0.0 { continue; }
            let surface = self.layer_surfaces.iter().flatten().find(|s| s.view == run.view && s.revision == Some(run.content_revision))
                .ok_or_else(|| invalid("Painter.composite", "layer surface was not prepared"))?;
            let mut instance = out.draw[run.layer as usize].instances[run.instance as usize];
            // This private rectangle flag samples retained fill RGB. Coverage and alpha stay analytic.
            instance.flags |= RETAINED_FILL;
            instances[len] = instance;
            breaks[len] = Break { at_instance: len as u32, texture: surface.texture.clone() };
            rects[len] = [cover.x, cover.y, cover.w, cover.h].map(|v| v as u32);
            len += 1;
        }
        self.draw_segments(encoder, size, scale, &instances[..len], &breaks[..len], None, &rects[..len], false)
    }
}
