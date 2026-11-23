use super::{finish, ok, open, public_commands, shared_renderer, texture};
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
                    clear: (0.0, 0.0, 0.0, 1.0),
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
    let drawn = renderer.draw_with_split(&opened, &mut |record| {
        assert!(clip.is_none(), "blur redraws the complete backdrop");
        ok(be::invalidate_state(&encoder));
        ok(be::end_render(&encoder));
        let image = ok(be::native_texture(target));
        record(&pxui::Image {
            vk_image_view: image.vk_image_view,
            mtl_texture: image.mtl_texture,
            width: opened.width_px,
            height: opened.height_px,
            format: image.format,
            ..Default::default()
        })?;
        encoder = begin(Load::Load);
        Ok(pass(&encoder, handles))
    });
    assert!(
        drawn.is_ok(),
        "native render oracle frame {frame}: {drawn:?}"
    );
    ok(be::invalidate_state(&encoder));
    ok(be::end_render(&encoder));
    let receipt = ok(be::commit(&commands));
    ok(be::complete_through(queue, receipt.epoch));
    ok(be::read_texture(target, i64::from(opened.width_px) * 4))
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
        let clip =
            (stage != 0 && !matches!(stage, 5 | 6 | 9) && damage.has_damage != 0).then(|| {
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
    }
    assert!(saw_partial);
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
