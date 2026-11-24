//! The host loop's GPU half, shared by the example and the compositor test: a Vulkan swapchain on the window's
//! token, and the UI painter drawing into a retained canvas (the source of truth) that each present copies to the
//! swapchain image, only where it changed when the frame names its damage.
use ogpu_core::backend::{self as be, AcquireResult, ColorAttachment, Device, Driver, Queue, RenderTarget, Surface, Texture};
use ogpu_core::error::{self, Error};
use ogpu_core::pipeline::{ColorFormat, DepthFormat};
use ogpu_core::render_pass::{Load, Rect, Store};
use ogpu_core::surface::{Configuration, Format, PresentMode};
use ogpu_core::types::{TextureDescriptor, TextureFormat, TextureUsage};
use pxui_render::{Instance, Painter};
use pxui_wayland::{Extent, Window};

pub fn ok<T>(result: Result<T, Error>) -> T {
    result.unwrap_or_else(|e| panic!("{}", error::to_string(&e)))
}

/// The canvas clear colour, also what a damaged region is repainted with before its instances.
pub const BACKGROUND: [u8; 4] = [31, 31, 36, 255];

pub struct Gpu {
    device: Device,
    queue: Queue,
    surface: Surface,
    target: Texture,
    painter: Option<Painter>,
    extent: Extent,
    /// The canvas holds a whole frame: a damaged frame may load it and redraw only its damage.
    drawn: bool,
}

fn configuration(window: &mut Window) -> Configuration {
    let extent = window.extent();
    Configuration {
        logical_width: extent.logical.0,
        logical_height: extent.logical.1,
        physical_width: extent.drawable.0,
        physical_height: extent.drawable.1,
        format: Format::Bgra8Unorm,
        present_mode: if window.vsync() { PresentMode::Fifo } else { PresentMode::Immediate },
        max_acquired: 1,
        layer: Some(window.token().unwrap()),
    }
}

/// What the painter draws into; the present copies it to the swapchain image.
fn target(device: &Device, (width, height): (i64, i64)) -> Texture {
    ok(be::create_texture(device, TextureDescriptor {
        label: Some("pxui_wayland.target".into()), width, height, depth: 1, mip_levels: 1, sample_count: 1,
        format: TextureFormat::Rgba8Unorm, usage: vec![TextureUsage::RenderAttachment, TextureUsage::TextureBinding, TextureUsage::TextureCopySrc],
    }))
}

impl Gpu {
    pub fn open(window: &mut Window, driver: fn() -> Box<dyn Driver>) -> Gpu {
        ogpu_core::impl_::select(ogpu_core::impl_::Impl {
            backend: ogpu_vulkan::impl_::BACKEND,
            create_driver: driver,
            live_handles: ogpu_vulkan::impl_::live_handles,
        });
        let device = ok(be::create_device(ok(ogpu_core::impl_::create_driver())));
        let queue = ok(be::create_queue(&device));
        let surface = ok(be::create_surface(&device, configuration(window)));
        let target = target(&device, window.extent().drawable);
        let painter = ok(Painter::new(&device, ColorFormat::Rgba8Unorm, DepthFormat::NoDepth, 1, (64, 64), (64, 64)));
        Gpu { device, queue, surface, target, painter: Some(painter), extent: window.extent(), drawn: false }
    }

    /// Draws `instances` (logical points) and presents; false when the compositor gave no image this time.
    pub fn frame(&mut self, window: &mut Window, instances: &[Instance]) -> bool {
        self.frame_damage(window, instances, None)
    }

    /// Draws `instances` and presents. `damage` (drawable pixels) redraws only that part of the retained canvas and
    /// copies only what each swapchain image lacks; `None`, a resize or a fresh canvas redraws everything. Asks for
    /// the next frame callback with this present. False when the compositor gave no image this time.
    pub fn frame_damage(&mut self, window: &mut Window, instances: &[Instance], damage: Option<Rect>) -> bool {
        if window.extent() != self.extent {
            self.extent = window.extent();
            ok(be::configure(&self.surface, configuration(window)));
            ok(be::destroy_texture(&self.target));
            self.target = target(&self.device, self.extent.drawable);
            self.drawn = false;
        }
        let damage = damage.filter(|_| self.drawn);
        let AcquireResult::Acquired(frame) = ok(be::acquire_sync(&self.surface)) else { return false };
        let painter = self.painter.as_mut().unwrap();
        painter.begin_frame();
        let commands = ok(be::begin_commands(&self.queue));
        let encoder = ok(be::render_encoder(&commands, &RenderTarget {
            colors: vec![ColorAttachment { texture: self.target.clone(), resolve: None, load: if damage.is_some() { Load::Load } else { Load::Clear },
                store: Store::Store, clear: (BACKGROUND[0] as f64 / 255.0, BACKGROUND[1] as f64 / 255.0, BACKGROUND[2] as f64 / 255.0, 1.0) }],
            depth: None,
        }));
        if let Some(rect) = damage {
            // the damage's own background, then the instances clipped to it
            ok(be::set_scissor(&encoder, rect));
            ok(painter.draw(&encoder, [self.extent.drawable.0 as u32, self.extent.drawable.1 as u32], 1.0,
                &[Instance::rect([rect.x as f32, rect.y as f32, (rect.x + rect.width) as f32, (rect.y + rect.height) as f32], BACKGROUND)], &[]));
        }
        let size = [self.extent.drawable.0 as u32, self.extent.drawable.1 as u32];
        ok(painter.draw(&encoder, size, window.scale() as f32, instances, &[]));
        ok(be::end_render(&encoder));
        window.request_frame();
        let receipt = ok(be::commit_present_damage(&commands, &self.target, &frame, damage.as_ref().map(std::slice::from_ref)));
        ok(be::complete_through(&self.queue, receipt.epoch));
        self.drawn = true;
        true
    }

    /// The canvas's pixels (RGBA8, rows of width x 4), after the last frame completed.
    pub fn canvas(&self) -> Vec<u8> {
        ok(be::read_texture(&self.target, self.extent.drawable.0 * 4))
    }

    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    /// Releases the GPU side; the window (whose token the surface uses) must outlive this.
    pub fn close(mut self) {
        drop(self.painter.take());
        ok(be::destroy_texture(&self.target));
        ok(be::destroy_surface(&self.surface));
        ok(be::destroy_queue(&self.queue));
        ok(be::destroy_device(&self.device));
    }
}
