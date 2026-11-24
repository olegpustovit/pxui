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
    let reuse = std::env::var_os("PXUI_NO_BLUR_REUSE").as_deref() != Some(std::ffi::OsStr::new("1"));
    let clip = reuse.then_some(Rect { x: 208, y: 133, width: 16, height: 16 });
    let (updated, splits) = draw_backdrop(&queue, &target, &mut renderer, 2, clip, green);
    assert_eq!(splits, usize::from(!reuse), "an unchanged footprint splits only with reuse disabled");
    assert_ne!(updated, first, "the marker changed");
    let (full_repaint, splits) = draw_backdrop(&queue, &target, &mut renderer, 3, None, green);
    assert_eq!(splits, usize::from(!reuse), "repainting the whole footprint uses a completed result when enabled");
    renderer.resize(240, 160).unwrap();
    let (full, recomputed) = draw_backdrop(&queue, &target, &mut renderer, 4, None, green);
    assert_eq!(recomputed, 1);
    diff(&updated, &full, 0, 2);
    diff(&full_repaint, &full, 0, 3);

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

/// Compare the real composition ABI with normal in-canvas rendering on both native backends.
pub(super) fn promoted_parity_1x() { promoted_parity(1); }
pub(super) fn promoted_parity_2x() { promoted_parity(2); }

fn promoted_parity(density: u32) {
    use pxui::{Fill, FrameIn, Keyframes, Node, Rate, Repeat, Rgba, ui::{Curve, Keyframe}};

    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let mut ui = Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    let mut full = shared_renderer(&device, &ui);
    let mut layers = shared_renderer(&device, &ui);
    let size = [32 * density, 24 * density];
    let target = texture(&device, size[0], size[1], &[TextureUsage::RenderAttachment, TextureUsage::TextureCopySrc]);
    let base = texture(&device, size[0], size[1], &[TextureUsage::RenderAttachment, TextureUsage::TextureBinding]);
    let promoted_target = texture(&device, size[0], size[1], &[TextureUsage::RenderAttachment, TextureUsage::TextureCopySrc]);
    let native = ok(be::native_texture(&base));
    let canvas = pxui::Image::rgba8(native.width, native.height, native.vk_image_view, native.mtl_texture);

    ui.set_promotion(true);
    layers.exclude_promoted(true).unwrap();
    let curve = Curve::Bezier { duration: 0.45, points: [0.42, 0.0, 0.58, 1.0] };
    ui.frame(&FrameIn::new(32.0, 24.0).scale(density as f32), &mut |ui| {
        let marker = ui.node("canvas marker", Node::column().w_px(10.0).h_px(4.0).at(20.0, 16.0));
        ui.canvas(&marker, move |paint, rect| paint.fill(rect, Fill::new(Rgba::rgb(17, 96, 140))));

        for (name, x, y, offset, color) in [
            ("first", 4.0, 4.0, 0.0, Rgba::rgb(255, 255, 255)),
            ("second", 8.0, 4.1, 0.3, Rgba::rgb(208, 155, 231)),
            ("third", 12.0, 4.35, 0.6, Rgba::rgb(55, 221, 70)),
            ("translucent", 16.0, 4.65, 0.2, Rgba::rgba(224, 211, 70, 160)),
        ] {
            ui.column(Node::column().w_px(2.0).h_px(11.0).at(x, y), |ui| {
                let view = ui.view_of(name, &0, |ui| {
                    let shape = ui.node("rect", Node::column().w_px(2.0).h_px(11.0));
                    ui.canvas(&shape, move |paint, rect| paint.fill(rect, Fill::new(color).fractional()));
                });
                let alpha = if color.a == 255 { [1.0; 2] } else { [0.2, 0.85] };
                let keys = [
                    Keyframe::scale(0.0, 1.0, 0.4).anchor(0.5, 1.0).curve(curve),
                    Keyframe::opacity(0.0, alpha[0]).curve(curve),
                    Keyframe::scale(0.45, 1.0, 1.0).anchor(0.5, 1.0).curve(curve),
                    Keyframe::opacity(0.45, alpha[1]).curve(curve),
                ];
                ui.animate_keys(&view, &keys, &Keyframes::DEFAULT.repeat(Repeat::Forever)
                    .autoreverse().offset(offset).rate(Rate::Ambient(20)));
            });
        }
    }).unwrap();
    assert_eq!(ui.promoted().len(), 4, "{:?}", ui.promoted());
    let render = |renderer: &mut Renderer, serial, composite| {
        let commands = ok(be::begin_commands(&queue));
        let native = public_commands(&commands);
        let before = mem_sys::local_alloc_count();
        renderer.prepare(&native, serial, serial - 1).unwrap();
        if composite && serial >= 4 {
            assert_eq!(mem_sys::local_alloc_count() - before, 0, "warm composition prepare allocations");
        }
        let open = |texture: &Texture, load| {
            ok(be::render_encoder(&commands, &RenderTarget {
                colors: vec![ColorAttachment {
                    texture: texture.clone(), load, store: Store::Store, resolve: None,
                    clear: (0.13, 0.09, 0.21, 1.0),
                }],
                depth: None,
            }))
        };
        let finish_pass = |encoder: &be::RenderEncoder| {
            ok(be::invalidate_state(encoder));
            ok(be::end_render(encoder));
        };
        if !composite || serial == 1 {
            let encoder = open(if composite { &base } else { &target }, Load::Clear);
            renderer.draw(&Pass { scale: density as f32, ..pass(&encoder, native) }).unwrap();
            finish_pass(&encoder);
        }
        if composite {
            let encoder = open(&promoted_target, if serial == 1 { Load::DontCare } else { Load::Load });
            let opened = Pass { scale: density as f32, ..pass(&encoder, native) };
            let damage = (serial > 1).then_some([3 * density, 3 * density, 16 * density, 14 * density]);
            let (allocations, calls) = (mem_sys::local_alloc_count(), renderer.draw_calls());
            renderer.composite(&opened, Some(&canvas), damage).unwrap();
            if serial >= 4 {
                assert_eq!(mem_sys::local_alloc_count() - allocations, 0, "warm native composite allocations");
                assert_eq!(renderer.draw_calls() - calls, 5, "one base copy and one cached draw per layer");
            }
            finish_pass(&encoder);
        }
        let receipt = ok(be::commit(&commands));
        ok(be::complete_through(&queue, receipt.epoch));
        ok(be::read_texture(if composite { &promoted_target } else { &target }, i64::from(size[0]) * 4))
    };

    let mut maximum = 0;
    let mut changes = 0;
    let mut previous = Vec::new();
    for frame in 1..=200 {
        let time = (frame - 1) as f64 * 0.05;
        assert!(ui.step(time));
        assert_eq!(ui.output().repaint_hz, 20);
        let sample = render(&mut layers, frame, true);
        let reference = render(&mut full, frame, false);
        let (at, delta) = sample.iter().zip(&reference).enumerate()
            .map(|(i, (a, b))| (i, a.abs_diff(*b))).max_by_key(|&(_, d)| d).unwrap();
        assert!(delta <= 2, "{density}x at {time:.4}s, byte {at}: {} vs {}, delta {delta}", sample[at], reference[at]);
        maximum = maximum.max(delta);
        changes += usize::from(sample != previous);
        previous = sample;
    }
    assert!(changes > 100, "samples must exercise moving and fading pixels");
    eprintln!("promotion parity {density}x: 200 samples, maximum {maximum}/channel");
    drop((layers, full));
    for texture in [&target, &base, &promoted_target] { ok(be::destroy_texture(texture)); }
    finish(device, queue);
}

/// A layer's pose changes while its texture stays resident. A revision repaints once and a
/// larger sweep replaces its surface; GPU completions bound the old surface's lifetime.
pub(super) fn promoted_mock() {
    use ogpu_core::types::TextureFormat;
    use pxui_core::ui::FrameOut;
    use pxui_render::{Instance, View, flags};
    use pxui_types::ffi::PxuiPromoted;

    ogpu_core::impl_::select(ogpu_mock::impl_::implementation());
    let (driver, control) = ogpu_mock::backend_mock::create_recording_render();
    let device = ok(be::create_device(driver));
    let queue = ok(be::create_queue(&device));
    let mut painter = super::painter(&device);
    let target = texture(&device, 32, 24, &[TextureUsage::RenderAttachment]);
    let mut layer = pxui_core::ui_core::Layer::default();
    let mut rect = Instance::rect([4.0, 4.0, 6.0, 15.0], [120, 90, 45, 170]);
    rect.flags = flags::rect::AREA;
    rect.view = 1;
    layer.instances.push(rect);
    let layers = [layer];
    let mut runs = [PxuiPromoted { view: 1, swept: [4.0, 4.0, 2.0, 11.0],
        color: rect.color, content_revision: 1, ..PxuiPromoted::default() }];
    ok(painter.set_view(1, View::IDENTITY));
    let original = ogpu_mock::backend_mock::live_counts(&control).1;
    for frame in 0..8 {
        if frame == 2 { runs[0].content_revision += 1; }
        if frame == 3 { runs[0].swept[3] = 12.0; }
        let out = FrameOut {
            draw: &layers, platform: &Default::default(), damage: &Default::default(), views: &[],
            blurs: &[], promoted: &runs, a11y: &pxui_core::a11y::A11yTree::new(),
        };
        painter.begin_frame();
        let commands = ok(be::begin_commands(&queue));
        let calls = painter.draw_calls();
        ok(painter.prepare_composition(&commands, &out, 1.0, TextureFormat::Rgba8Unorm));
        assert_eq!(painter.draw_calls() - calls, u64::from(matches!(frame, 0 | 2 | 3)));
        let encoder = ok(be::render_encoder(&commands, &RenderTarget {
            colors: vec![ColorAttachment { texture: target.clone(), resolve: None, load: Load::Clear,
                store: Store::Store, clear: (0.0, 0.0, 0.0, 1.0) }],
            depth: None,
        }));
        ok(painter.composite(&encoder, [32, 24], 1.0, &out, None));
        ok(be::end_render(&encoder));
        let receipt = ok(be::commit(&commands));
        ok(be::complete_through(&queue, receipt.epoch));
        let textures = ogpu_mock::backend_mock::live_counts(&control).1;
        assert_eq!(textures - original, if (3..6).contains(&frame) { 2 } else { 1 });
    }
    drop(painter);
    ok(be::destroy_texture(&target));
    finish(device, queue);
    assert_eq!(ogpu_mock::backend_mock::live_counts(&control), (0, 0, 0, 0, 0));
}
