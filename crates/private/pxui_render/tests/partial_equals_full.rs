use super::{finish, ok, open, public_commands, shared_renderer, shared_renderer_with_flags, texture};
use ogpu_core::backend::{self as be, ColorAttachment, RenderTarget, Texture};
use ogpu_core::render_pass::{Load, Rect, Store};
use ogpu_core::types::{BufferDescriptor, BufferUsage, TextureUsage};
use pxui::{Pass, Renderer, ThemeHandle, Ui};
use std::collections::HashMap;

#[path = "../../../../tools/t/render_edits.rs"]
mod edits;
use edits::{HEIGHT, STAGES, diff, edit, width};

fn pass(encoder: &be::RenderEncoder, commands: pxui::Commands) -> Pass {
    let native = ok(be::native_pass(encoder));
    Pass {
        vk_command_buffer: native.vk_command_buffer,
        mtl_command_buffer: commands.mtl_command_buffer,
        mtl_render_encoder: native.mtl_render_encoder,
        width_px: native.width_px,
        height_px: native.height_px,
        ..Pass::default()
    }
}

fn draw(
    queue: &be::Queue,
    target: &Texture,
    renderer: &mut Renderer,
    frame: u64,
    clip: Option<Rect>,
) -> Vec<u8> {
    draw_backdrop(queue, target, renderer, frame, clip, (0.0, 0.0, 0.0, 1.0)).0
}

fn draw_backdrop(
    queue: &be::Queue,
    target: &Texture,
    renderer: &mut Renderer,
    frame: u64,
    clip: Option<Rect>,
    clear: (f64, f64, f64, f64),
) -> (Vec<u8>, usize) {
    let commands = ok(be::begin_commands(queue));
    let handles = public_commands(&commands);
    renderer.prepare(&handles, frame, frame - 1).unwrap();
    let begin = |load| {
        ok(be::render_encoder(
            &commands,
            &RenderTarget {
                colors: vec![ColorAttachment {
                    texture: target.clone(),
                    load,
                    store: Store::Store,
                    resolve: None,
                    clear,
                }],
                depth: None,
            },
        ))
    };
    let mut encoder = begin(if clip.is_some() {
        Load::Load
    } else {
        Load::Clear
    });
    if let Some(clip) = clip {
        ok(be::set_scissor(&encoder, clip));
    }
    let opened = pass(&encoder, handles);
    let mut splits = 0;
    let mut split = |record: pxui::Recorder| {
        splits += 1;
        assert!(clip.is_none(), "blur redraws the complete backdrop");
        ok(be::invalidate_state(&encoder));
        ok(be::end_render(&encoder));
        let image = ok(be::native_texture(target));
        record(&pxui::Image::rgba8(
            opened.width_px,
            opened.height_px,
            image.vk_image_view,
            image.mtl_texture,
        ))?;
        encoder = begin(Load::Load);
        Ok(pass(&encoder, handles))
    };
    let drawn = match clip {
        Some(r) => renderer.draw_damage(&opened, [r.x as u32, r.y as u32, r.width as u32, r.height as u32], &mut split),
        None => renderer.draw_with_split(&opened, &mut split),
    };
    assert!(
        drawn.is_ok(),
        "native render oracle frame {frame}: {drawn:?}"
    );
    ok(be::invalidate_state(&encoder));
    ok(be::end_render(&encoder));
    let receipt = ok(be::commit(&commands));
    ok(be::complete_through(queue, receipt.epoch));
    (ok(be::read_texture(target, i64::from(opened.width_px) * 4)), splits)
}

/// Offscreen Metal/Vulkan. CI selects Mesa's lavapipe ICD on Linux.
pub(super) fn native() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let mut ui = Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    let mut renderer = shared_renderer(&device, &ui);
    let create = |stage| {
        texture(
            &device,
            width(stage),
            HEIGHT,
            &[
                TextureUsage::RenderAttachment,
                TextureUsage::TextureBinding,
                TextureUsage::TextureCopySrc,
            ],
        )
    };
    let mut target = create(0);
    let extent = Pass { width_px: width(0), height_px: HEIGHT, ..Pass::default() };
    for rect in [[0, 0, 0, HEIGHT], [u32::MAX, 0, 2, 1], [0, HEIGHT, 1, 1]] {
        assert!(matches!(renderer.draw_damage(&extent, rect, &mut |_| unreachable!()), Err(pxui::Error::InvalidInput)));
    }
    renderer.resize(width(0), HEIGHT).unwrap();
    let theme = ThemeHandle::parse("tokens.panel = #284557").unwrap();
    let mut saw_partial = false;
    for stage in 0..STAGES {
        if stage > 0 && width(stage) != width(stage - 1) {
            ok(be::destroy_texture(&target));
            target = create(stage);
            renderer.resize(width(stage), HEIGHT).unwrap();
        }
        edit(&mut ui, stage, 1.0, &theme);
        let damage = ui.output().damage;
        let clip = (stage != 0
            && !matches!(stage, 5 | 6 | 9)
            && ui.damage_list().full == 0
            && damage.has_damage != 0)
            .then(|| {
                let r = damage.rect;
                let left = ((r.x.floor() as i64) - 1).clamp(0, i64::from(width(stage)) - 1);
                let top = ((r.y.floor() as i64) - 1).clamp(0, i64::from(HEIGHT) - 1);
                let right =
                    (((r.x + r.w).ceil() as i64) + 1).clamp(left + 1, i64::from(width(stage)));
                let bottom = (((r.y + r.h).ceil() as i64) + 1).clamp(top + 1, i64::from(HEIGHT));
                Rect {
                    x: left,
                    y: top,
                    width: right - left,
                    height: bottom - top,
                }
            });
        saw_partial |=
            clip.is_some_and(|r| r.width < i64::from(width(stage)) && r.height < i64::from(HEIGHT));
        let frame = stage as u64 * 2 + 1;
        let partial = draw(&queue, &target, &mut renderer, frame, clip);
        let full = draw(&queue, &target, &mut renderer, frame + 1, None);
        diff(
            &partial,
            &full,
            if cfg!(target_os = "macos") { 0 } else { 2 },
            stage,
        );
        for index in 0..ui.layers().count() {
            ui.drawn(index);
        }
    }
    assert!(saw_partial);
    drop(renderer);
    ok(be::destroy_texture(&target));
    finish(device, queue);
}

/// A changing external clear is visible to default renderers. A trusted, completed backdrop can
/// be read from its cache during a scissored redraw, and matches a fresh full redraw byte for byte.
pub(super) fn backdrop_reuse() {
    use pxui::{FrameIn, Node, PxuiTargetDesc, Rgba, ViewTransform};
    use pxui::ui::Look;

    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let target = texture(&device, 240, 160, &[TextureUsage::RenderAttachment, TextureUsage::TextureBinding, TextureUsage::TextureCopySrc]);
    let build = |ui: &mut Ui, source, marker, time| {
        ui.frame(&FrameIn::new(240.0, 160.0).time(time), &mut |ui| {
            ui.node("backdrop source", Node::DEFAULT.w_px(22.0).h_px(32.0).at(18.0, 18.0)
                .look(Look::NONE.fill(source)));
            let glass = ui.view_of("glass", &0, |ui| {
                ui.node("tint", Node::DEFAULT.w_px(64.0).h_px(48.0)
                    .look(Look::NONE.fill(Rgba::rgba(0, 0, 0, 8))));
            });
            ui.set_node_layout(&glass, &Node::column().w_px(64.0).h_px(48.0).at(20.0, 20.0));
            ui.set_view(&glass, &ViewTransform::DEFAULT.blur(8.0));
            ui.node("marker", Node::DEFAULT.w_px(12.0).h_px(12.0).at(210.0, 135.0)
                .look(Look::NONE.fill(marker)));
        }).unwrap();
    };
    let pane_pixel = |pixels: &[u8], x: usize| -> [u8; 4] {
        let at = (42 * 240 + x) * 4;
        pixels[at..at + 4].try_into().unwrap()
    };
    let mut ui = Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    let mut renderer = shared_renderer(&device, &ui);
    renderer.resize(240, 160).unwrap();
    build(&mut ui, Rgba::BLACK, Rgba::BLACK, 0.0);
    let red = (1.0, 0.0, 0.0, 1.0);
    let green = (0.0, 1.0, 0.0, 1.0);
    let (first, first_splits) = draw_backdrop(&queue, &target, &mut renderer, 1, None, red);
    assert_eq!(ui.damage_list().count, 0);
    let (second, second_splits) = draw_backdrop(&queue, &target, &mut renderer, 2, None, green);
    assert_eq!((first_splits, second_splits), (1, 1));
    assert!(pane_pixel(&first, 65)[0] > 200 && pane_pixel(&second, 65)[1] > 200);
    drop(renderer);

    let mut ui = Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    let mut renderer = shared_renderer_with_flags(&device, &ui, PxuiTargetDesc::BACKDROP_UI_DAMAGE);
    renderer.resize(240, 160).unwrap();
    build(&mut ui, Rgba::BLACK, Rgba::BLACK, 0.0);
    let (first, seeded) = draw_backdrop(&queue, &target, &mut renderer, 1, None, green);
    assert_eq!(seeded, 1);
    build(&mut ui, Rgba::BLACK, Rgba::WHITE, 0.1);
    assert_eq!(ui.damage_list().count, 1);
    let clip = Rect { x: 208, y: 133, width: 16, height: 16 };
    let (partial, reused) = draw_backdrop(&queue, &target, &mut renderer, 2, Some(clip), green);
    assert_eq!(reused, 0, "the unchanged footprint must not split the render pass");
    assert_ne!(partial, first, "the marker changed");
    let (cached_full, still_reused) = draw_backdrop(&queue, &target, &mut renderer, 3, None, green);
    assert_eq!(still_reused, 0, "the cached image can also repaint its whole footprint");
    renderer.resize(240, 160).unwrap();
    let (full, recomputed) = draw_backdrop(&queue, &target, &mut renderer, 4, None, green);
    assert_eq!(recomputed, 1);
    diff(&partial, &full, 0, 2);
    diff(&cached_full, &full, 0, 3);

    build(&mut ui, Rgba::WHITE, Rgba::WHITE, 0.2);
    let (changed, damaged) = draw_backdrop(&queue, &target, &mut renderer, 5, None, green);
    assert_eq!(damaged, 1, "damage within the footprint invalidates the cached result");
    assert_ne!(pane_pixel(&changed, 46), pane_pixel(&full, 46));
    drop(renderer);
    ok(be::destroy_texture(&target));
    finish(device, queue);
}

/// The mock has no rasterizer. Replay the published dirty instance ranges into retained GPU
/// storage and compare them to an independent full upload of the same frame.
pub(super) fn mock() {
    let (driver, control) = ogpu_mock::backend_mock::create();
    let device = ok(be::create_device(driver));
    let mut ui = Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    let theme = ThemeHandle::parse("tokens.panel = #284557").unwrap();
    let mut retained = HashMap::new();
    let mut saw_partial = false;
    for stage in 0..STAGES {
        edit(&mut ui, stage, 1.0, &theme);
        for index in 0..ui.layers().count() {
            let layer = ui.layer(index).unwrap();
            if layer.instances.is_empty() {
                continue;
            }
            let all = pxui_render::instance_bytes(layer.instances);
            let new = !retained.contains_key(&layer.id);
            let buffer = retained.entry(layer.id).or_insert_with(|| {
                ok(be::create_buffer(
                    &device,
                    BufferDescriptor {
                        label: None,
                        size: 1024 * 1024,
                        usage: vec![BufferUsage::Storage],
                    },
                    None,
                ))
            });
            if new {
                ok(be::write_buffer(buffer, 0, all));
            } else {
                let mut bytes = 0;
                for range in layer.dirty {
                    let start = range.start as usize * 64;
                    let end = (range.end as usize * 64).min(all.len());
                    if start < end {
                        ok(be::write_buffer(buffer, start as i64, &all[start..end]));
                        bytes += end - start;
                    }
                }
                saw_partial |= bytes > 0 && bytes < all.len();
                let partial = ok(be::read_buffer(buffer, 0, all.len() as i64));
                assert!(
                    partial == all,
                    "mock dirty uploads differ from full data at stage {stage}, layer {index}"
                );
            }
            ui.drawn(index);
        }
    }
    assert!(saw_partial);
    for buffer in retained.values() {
        ok(be::destroy_buffer(buffer));
    }
    ok(be::destroy_device(&device));
    assert_eq!(
        ogpu_mock::backend_mock::live_counts(&control),
        (0, 0, 0, 0, 0)
    );
}
