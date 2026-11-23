#[cfg(target_os = "macos")]
#[path = "../../../../tools/t/window_harness.rs"]
#[allow(clippy::type_complexity)]
mod harness;

#[cfg(target_os = "macos")]
#[path = "../../../../tools/t/render_edits.rs"]
mod render_edits;

#[cfg(target_os = "macos")]
fn partial_equals_full() {
    use crate::{Graphics, Window};
    use render_edits::{HEIGHT, STAGES, diff, edit, width};

    let mut window = Window::open_with("PXUI render oracle", i64::from(width(0)), i64::from(HEIGHT), false, true, false).unwrap();
    let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    let mut graphics = Graphics::open(&mut window, &ui).unwrap();
    let theme = pxui::ThemeHandle::parse("tokens.panel = #284557").unwrap();
    let mut previous = Vec::new();
    let mut saw_partial = false;
    for stage in 0..STAGES {
        if stage > 0 && width(stage) != width(stage - 1) {
            window.set_resizable(true).unwrap();
            window.resize(i64::from(width(stage)), i64::from(HEIGHT)).unwrap();
        }
        edit(&mut ui, stage, window.scale() as f32, &theme);
        let damage = ui.output().damage;
        saw_partial |= ui.damage_list().full == 0 && damage.has_damage != 0
            && damage.rect.w < width(stage) as f32 && damage.rect.h < HEIGHT as f32;
        let capture = graphics.capture_frame(&mut window, &ui, false);
        assert!(capture.is_ok(), "{capture:?}: {}", ui.last_error());
        let (w, h, partial) = capture.unwrap();
        assert_eq!((w, h), (width(stage), HEIGHT));
        let (_, _, full) = graphics.screenshot(&mut window, &ui).unwrap();
        diff(&partial, &full, 0, stage);
        for index in 0..ui.layers().count() { ui.drawn(index); }
        assert!(partial != previous, "edit {stage} made no pixel change");
        previous = partial;
    }
    assert!(saw_partial);
    graphics.close().unwrap();
    window.destroy().unwrap();
}

#[cfg(target_os = "macos")]
fn present_pixels_match_canvas_with_partial_copies_and_fallbacks() {
    use crate::{Graphics, Window};
    use pxui::ui::{Node, Size};
    use pxui::{Fill, FrameIn, Rgba, Ui};

    mem_sys::set_peer(pxui::library_counter);
    let mut window = Window::open_with("PXUI present oracle", 320, 240, false, true, false).unwrap();
    let mut ui = Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    let mut graphics = Graphics::open_present(&mut window, &ui, false, 1).unwrap();
    let build = |window: &Window, ui: &mut Ui, frame: usize| {
        let (w, h) = window.extent().logical;
        ui.frame(
            &FrameIn::new(w as f32, h as f32).scale(window.scale() as f32).time(frame as f64 / 60.0),
            &mut |ui| {
                for (name, x) in [("left", 8.0), ("right", 270.0)] {
                    let color = Rgba::rgb((frame * 13) as u8, 0, 0);
                    let node = ui.node(name, Node::DEFAULT.w(Size::Px(14.0)).h(Size::Px(11.0)).at(x, 4.0));
                    ui.canvas(&node, move |paint, rect| paint.fill(rect, Fill::new(color)));
                }
            },
        ).unwrap();
    };
    let mut partials = 0;
    let mut events = Vec::new();
    for frame in 0..20 {
        window.pump(&mut events);
        build(&window, &mut ui, frame);
        mem_sys::local_reset();
        assert!(graphics.draw(&mut window, &ui, None).unwrap());
        if frame >= 8 {
            assert_eq!(mem_sys::local_alloc_count(), 0, "warm partial present");
        }
        partials += usize::from(graphics.last_present_was_partial());
    }
    assert!(partials >= 8, "expected repeated partial copies, got {partials}");
    let (verified, failed) = graphics.present_verification();
    assert!(verified >= 17 && !failed, "{verified} completed comparisons, mismatch={failed}");

    window.set_resizable(true).unwrap();
    window.resize(352, 240).unwrap();
    build(&window, &mut ui, 20);
    assert!(graphics.draw(&mut window, &ui, None).unwrap());
    assert!(!graphics.last_present_was_partial(), "resize resets drawable history");

    let mut resumed_partial = false;
    for frame in 21..32 {
        window.pump(&mut events);
        build(&window, &mut ui, frame);
        assert!(graphics.draw(&mut window, &ui, None).unwrap());
        resumed_partial |= graphics.last_present_was_partial();
    }
    assert!(resumed_partial, "partial copies resume after resize");
    window.show().unwrap();
    let deadline = window.time() + 0.45;
    while window.presentation().is_none() && window.time() < deadline {
        window.wait(Some(deadline));
        assert!(window.pump(&mut events));
    }
    assert!(window.presentation().is_some(), "the revealed window must be exposed");
    build(&window, &mut ui, 32);
    assert!(graphics.draw(&mut window, &ui, None).unwrap());
    assert!(!graphics.last_present_was_partial(), "becoming visible resets drawable history");

    build(&window, &mut ui, 33);
    graphics.capture_frame(&mut window, &ui, false).unwrap();
    assert!(!graphics.last_present_was_partial(), "capture forces a full copy of a partial canvas");

    graphics.set_full_present(true).unwrap();
    build(&window, &mut ui, 34);
    assert!(graphics.draw(&mut window, &ui, None).unwrap());
    assert!(!graphics.last_present_was_partial(), "explicit full-copy mode");

    graphics.close().unwrap();
    window.destroy().unwrap();
}

#[cfg(target_os = "macos")]
fn occluded_window_skips_gpu_and_redraws_on_exposure() {
    use crate::{Graphics, Window};
    use pxui::{FrameIn, Ui};

    let mut window = Window::open("PXUI occlusion", 200, 140).unwrap();
    pxui_sdl::test_order_front(&window.token().unwrap(), false);
    let mut events = Vec::new();
    let wait_for = |window: &mut Window, events: &mut Vec<input::Event>, visible: bool| {
        let deadline = window.time() + 0.45;
        while window.visible().unwrap() != visible && window.time() < deadline {
            window.wait(Some(deadline));
            assert!(window.pump(events));
            events.clear();
        }
        assert_eq!(window.visible().unwrap(), visible, "native window visibility");
    };
    wait_for(&mut window, &mut events, true);
    let mut ui = Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    ui.frame(&FrameIn::new(200.0, 140.0).scale(window.scale() as f32), &mut |_| {}).unwrap();
    let mut graphics = Graphics::open(&mut window, &ui).unwrap();
    assert!(graphics.draw(&mut window, &ui, None).unwrap());
    for index in 0..ui.layers().count() { ui.drawn(index); }
    assert!(!graphics.draw(&mut window, &ui, None).unwrap());

    let mut cover = Window::open("PXUI cover", 600, 440).unwrap();
    pxui_sdl::test_order_front(&cover.token().unwrap(), true);
    wait_for(&mut window, &mut events, false);
    let calls = graphics.draw_calls();
    for _ in 0..10 {
        assert!(!graphics.draw_forced(&mut window, &ui, None).unwrap());
    }
    assert_eq!(graphics.draw_calls(), calls);

    cover.destroy().unwrap();
    wait_for(&mut window, &mut events, true);
    assert!(graphics.surface_changed(&window));
    assert!(graphics.draw(&mut window, &ui, None).unwrap());
    assert!(!graphics.surface_changed(&window));
    assert!(!graphics.draw(&mut window, &ui, None).unwrap());
    graphics.close().unwrap();
    window.destroy().unwrap();
}

#[cfg(target_os = "macos")]
fn release_keepalive() {}

#[cfg(target_os = "macos")]
fn framework_host_uploads_exact_rgba_reuses_warm_frames_and_fences_texture_release() {
    use pxui::images::Want;
    use pxui::ui::{Fit, Node, Size};
    use pxui::{FrameIn, Ui};
    use crate::{Graphics, Window};
    mem_sys::set_peer(pxui::library_counter);
    let mut window =
        Window::open_with("PXUI resource lifecycle", 320, 240, false, true, true).unwrap();
    let mut ui = Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    let mut graphics = Graphics::open(&mut window, &ui).unwrap();
    graphics.configure_images(1.0, 64).unwrap();
    graphics.sync_images(1).unwrap();
    let epoch = graphics.images().epoch();
    let key = Want::new(1, 64).unwrap();
    let mut pixels: [u8; 8*4*4] = std::array::from_fn(|index| index as u8);
    pixels[3] = 255;
    for (width, height, bytes) in [
        (0, 1, &pixels[..]),
        (1025, 1, &pixels[..]),
        (2, 1, &pixels[..4]),
    ] {
        assert!(
            graphics
                .upload_image(key, epoch, 1, Some((width, height, bytes)))
                .is_err()
        );
        assert_eq!(graphics.resident_image_bytes(), 0);
    }
    assert!(
        graphics
            .upload_image(key, epoch, 1, Some((8, 4, &pixels)))
            .unwrap()
    );
    let texture = graphics.image_texture(1).unwrap().clone();
    for id in 2..=8 {
        let mut delivered = pixels;
        delivered[0] = id as u8;
        assert!(graphics.upload_image(Want::new(id, key.max_pixels()).unwrap(),epoch,id as u64,Some((8,4,&delivered))).unwrap());
    }
    // Read the entire batch after every upload, so aliasing the last source cannot pass.
    for id in 1..=8 {
        let mut expected = pixels;
        if id > 1 {expected[0] = id as u8;}
        let texture = graphics.image_texture(id).unwrap();
        assert_eq!(ogpu_core::backend::read_texture(texture,8*4).unwrap(),expected);
    }
    // each 8x4 image carries its 4x2, 2x1 and 1x1 mip levels
    assert_eq!(graphics.resident_image_bytes(),8*(pixels.len()+(4*2+2+1)*4));
    let draw_changed_marker = |ui: &mut Ui, frame: usize| {
        let marker = ui.node("frame-marker", Node::DEFAULT.w(Size::Px(2.0)).h(Size::Px(2.0)).at(70.0, 0.0));
        let red = if frame % 2 == 0 { 255 } else { 0 };
        ui.canvas(&marker, move |paint, r| paint.fill(r, pxui::Fill::new(pxui::Rgba::rgb(red, 0, 0))));
    };
    for frame in 0..32 {
        mem_sys::local_reset();
        assert!(
            !graphics
                .upload_image(key, epoch, 1, Some((8, 4, &pixels)))
                .unwrap()
        );
        ui.frame(
            &FrameIn::new(320.0, 240.0).time(frame as f64 / 60.0),
            &mut |ui| {
                let node = ui.node(
                    "image",
                    Node::DEFAULT.w(Size::Px(64.0)).h(Size::Px(32.0)),
                );
                let image = graphics
                    .images()
                    .get(1, "memory:rgba", 32.0, Some(&node))
                    .unwrap();
                draw_changed_marker(ui, frame);
                ui.canvas(&node, move |paint, r| {
                    paint.image(r, &image, &pxui::ImageStyle::DEFAULT.fit(Fit::Cover)).unwrap();
                });
            },
        )
        .unwrap();
        assert!(graphics.draw(&mut window, &ui, None).unwrap());
        if frame >= 8 {
            assert_eq!(
                mem_sys::local_alloc_count(),
                0,
                "framework input/build/draw with warm image wrappers"
            );
        }
    }
    assert!(graphics.sync_images(2).unwrap());
    assert!(
        !ogpu_core::backend::texture_destroyed(&texture),
        "in-flight images must survive namespace changes"
    );
    for id in 1..=8 {
        assert!(!graphics.upload_image(Want::new(id, key.max_pixels()).unwrap(),epoch,id as u64,Some((8,4,&pixels))).unwrap(),"old epochs cannot resurrect images");
        assert!(graphics.images().quality(id).is_none());
    }
    for frame in 0..3 {
        ui.frame(
            &FrameIn::new(320.0, 240.0).time(1.0 + frame as f64 / 60.0),
            &mut |ui| draw_changed_marker(ui, frame),
        )
        .unwrap();
        assert!(graphics.draw(&mut window, &ui, None).unwrap());
    }
    assert!(
        ogpu_core::backend::texture_destroyed(&texture),
        "the host releases textures only after GPU completion"
    );
    graphics.close().unwrap();
    window.destroy().unwrap();
}

#[cfg(target_os = "macos")]
fn framework_host_screenshot_returns_presented_rgba_and_keeps_warm_draws_allocation_free() {
    use pxui::ui::{Node, Size};
    use pxui::{Fill, FrameIn, Rgba, Ui};
    use crate::{Graphics, Window};
    mem_sys::set_peer(pxui::library_counter);
    let mut window = Window::open_with("PXUI screenshot", 320, 240, false, true, true).unwrap();
    let mut ui = Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    let mut graphics = Graphics::open(&mut window, &ui).unwrap();
    let scale = window.scale();
    let build = |ui: &mut Ui, time: f64, green: u8| {
        ui.frame(&FrameIn::new(320.0, 240.0).scale(scale as f32).time(time), &mut |ui| {
            let node = ui.node("swatch", Node::DEFAULT.w(Size::Px(40.0)).h(Size::Px(20.0)));
            ui.canvas(&node, move |paint, r| paint.fill(r, Fill::new(Rgba::rgb(255, green, 0))));
        }).unwrap();
    };
    build(&mut ui, 0.0, 0);
    let (width, height, pixels) = graphics.screenshot(&mut window, &ui).unwrap();
    assert_eq!((width, height), ((320.0 * scale) as u32, (240.0 * scale) as u32));
    assert_eq!(pixels.len(), width as usize * height as usize * 4);
    let at = |x: f64, y: f64| { let i = ((y * scale) as usize * width as usize + (x * scale) as usize) * 4; [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]] };
    assert_eq!(at(10.0, 10.0), [255, 0, 0, 255], "the swatch reads back as RGBA");
    let panel = ui.theme().tokens.panel;
    assert_eq!(at(200.0, 200.0), [panel.r, panel.g, panel.b, 255], "the cleared panel outside it");
    assert!(!graphics.draw(&mut window, &ui, None).unwrap(), "a clean canvas needs no present");
    assert!(graphics.acquire_wall().is_zero(), "a clean canvas never acquires a drawable");
    for frame in 0..16 {
        mem_sys::local_reset();
        build(&mut ui, 1.0 + frame as f64 / 60.0, u8::from(frame % 2 == 0) * 255);
        assert!(graphics.draw(&mut window, &ui, None).unwrap());
        if frame >= 4 {
            assert_eq!(mem_sys::local_alloc_count(), 0, "draws after a screenshot stay allocation-free");
        }
    }
    graphics.close().unwrap();
    window.destroy().unwrap();
}

/// A small change redraws only its damage over the retained canvas: the presented frame matches what a full redraw shows.
#[cfg(target_os = "macos")]
fn framework_host_damage_redraws_only_the_change_and_keeps_the_rest() {
    use pxui::ui::{Node, Size};
    use pxui::{Fill, FrameIn, Rgba, Ui};
    use crate::{Graphics, Window};
    let mut window = Window::open_with("PXUI damage", 320, 240, false, true, true).unwrap();
    let mut ui = Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    let mut graphics = Graphics::open(&mut window, &ui).unwrap();
    let scale = window.scale();
    let build = |ui: &mut Ui, time: f64, left: Rgba, right: Option<Rgba>| {
        ui.frame(&FrameIn::new(320.0, 240.0).scale(scale as f32).time(time), &mut |ui| {
            ui.row(Node::row().w(Size::Grow).h(Size::Px(20.0)).gap(200.0), |ui| {
                let a = ui.node("left", Node::DEFAULT.w(Size::Px(40.0)).h(Size::Px(20.0)));
                ui.canvas(&a, move |paint, r| paint.fill(r, Fill::new(left)));
                let b = ui.node("right", Node::DEFAULT.w(Size::Px(40.0)).h(Size::Px(20.0)));
                if let Some(right) = right { ui.canvas(&b, move |paint, r| paint.fill(r, Fill::new(right))); }
            });
        }).unwrap();
    };
    let (red, green, blue) = (Rgba::rgb(255, 0, 0), Rgba::rgb(0, 255, 0), Rgba::rgb(0, 0, 255));
    let panel = ui.theme().tokens.panel;
    let panel = [panel.r, panel.g, panel.b, 255];
    let read = |graphics: &mut Graphics, window: &mut Window, ui: &Ui| graphics.capture_frame(window, ui, false).unwrap().2;
    let width = (320.0 * scale) as usize;
    let at = |pixels: &[u8], x: f64, y: f64| { let i = ((y * scale) as usize * width + (x * scale) as usize) * 4; [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]] };
    build(&mut ui, 0.0, red, Some(blue));
    let first = read(&mut graphics, &mut window, &ui);
    assert_eq!((at(&first, 10.0, 10.0), at(&first, 250.0, 10.0)), ([255, 0, 0, 255], [0, 0, 255, 255]));
    // only the left swatch changes: its damage is small, the right one and the panel come from the retained canvas
    build(&mut ui, 0.1, green, Some(blue));
    let damage = ui.output().damage;
    assert!(damage.has_damage != 0 && damage.rect.w <= 60.0 && damage.rect.h <= 40.0, "a swatch's change damages only it: {damage:?}");
    let second = read(&mut graphics, &mut window, &ui);
    assert_eq!(at(&second, 10.0, 10.0), [0, 255, 0, 255], "the damaged swatch is redrawn");
    assert_eq!(at(&second, 250.0, 10.0), [0, 0, 255, 255], "the undamaged swatch is kept");
    assert_eq!(at(&second, 200.0, 200.0), panel, "the panel is kept");
    // the right swatch disappears: its damage is cleared back to the panel colour
    build(&mut ui, 0.2, green, None);
    let third = read(&mut graphics, &mut window, &ui);
    assert_eq!((at(&third, 10.0, 10.0), at(&third, 250.0, 10.0)), ([0, 255, 0, 255], panel), "a removed swatch's damage is cleared");
    // and every pixel matches a full redraw of the same frame
    let mut full_ui = Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    let mut fresh = Graphics::open(&mut window, &full_ui).unwrap();
    build(&mut full_ui, 0.2, green, None);
    assert!(fresh.screenshot(&mut window, &full_ui).unwrap().2 == third, "a damage-limited frame differs from a full redraw");
    // Seeded translucent edits and removals on both sides exercise two scissors on one encoder.
    let mut seed = 13u32;
    for frame in 0..12 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let [r, g, b, _] = seed.to_le_bytes();
        let left = Rgba::rgba(r, g, b, 100);
        let right = (frame % 4 != 3).then_some(Rgba::rgba(b, r, g, 175));
        let time = 0.3 + frame as f64 * 0.1;
        build(&mut ui, time, left, right);
        build(&mut full_ui, time, left, right);
        assert_eq!(ui.damage_list().count, 2);
        let calls = graphics.draw_calls();
        let partial = read(&mut graphics, &mut window, &ui);
        assert_eq!(graphics.draw_calls() - calls, if right.is_some() { 2 } else { 1 });
        assert_eq!(partial, fresh.screenshot(&mut window, &full_ui).unwrap().2, "seed {seed}");
    }
    // Build, query damage, encode two scissors and submit with both allocation counters on.
    mem_sys::local_reset();
    build(&mut ui, 2.0, red, Some(green));
    assert_eq!(ui.damage_list().count, 2);
    let mut allocs = [0; 4];
    assert!(graphics.draw(&mut window, &ui, Some(&mut allocs)).unwrap());
    assert_eq!(allocs, [0; 4]);
    assert_eq!(mem_sys::local_alloc_count(), 0);
    fresh.close().unwrap();
    graphics.close().unwrap();
    window.destroy().unwrap();
}

#[cfg(target_os = "macos")]
fn backdrop_blur_reads_the_canvas_and_preserves_undamaged_pixels() {
    use crate::{Graphics, Window};
    use pxui::ui::{Look, Node};
    use pxui::{Fill, FrameIn, Rgba, Ui, ViewTransform};

    let mut window = Window::open_with("PXUI backdrop blur", 320, 240, false, true, false).unwrap();
    let mut ui = Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    let mut graphics = Graphics::open(&mut window, &ui).unwrap();
    let view = std::cell::Cell::new(None);
    let build = |ui: &mut Ui, time, left| {
        ui.frame(&FrameIn::new(320.0, 240.0).time(time), &mut |ui| {
            let backdrop = ui.node("edge", Node::DEFAULT.w_px(160.0).h_px(80.0));
            ui.canvas(&backdrop, move |paint, rect| {
                paint.fill(rect, Fill::new(Rgba::WHITE));
                paint.fill(pxui::Rect::new(rect.x, rect.y, 80.0, rect.h), Fill::new(left));
            });
            let glass = ui.view_of("glass", &1, |ui| {
                ui.node("pane", Node::DEFAULT.w_px(160.0).h_px(80.0)
                    .look(Look::NONE.fill(Rgba::rgba(0, 0, 0, 1))));
            });
            view.set(Some(glass));
        }).unwrap();
    };
    build(&mut ui, 0.0, Rgba::BLACK);
    ui.set_view(&view.get().unwrap(), &ViewTransform::DEFAULT.offset(0.0, -80.0).blur(8.0));
    build(&mut ui, 0.1, Rgba::BLACK);
    let capture = graphics.capture_frame(&mut window, &ui, false);
    assert!(capture.is_ok(), "{capture:?}: {}", ui.last_error());
    let pixels = capture.unwrap().2;
    let red = |pixels: &[u8], x: usize| pixels[(40 * 320 + x) * 4];
    assert!(red(&pixels, 76) > 40 && red(&pixels, 76) < 150, "the black side receives blurred white");
    assert!(red(&pixels, 84) > 150 && red(&pixels, 84) < 230, "the white side receives blurred black");
    build(&mut ui, 0.2, Rgba::rgb(160, 0, 0));
    let updated = graphics.capture_frame(&mut window, &ui, false).unwrap().2;
    let mut fresh = Graphics::open(&mut window, &ui).unwrap();
    assert!(fresh.screenshot(&mut window, &ui).unwrap().2 == updated);
    assert_ne!(red(&pixels, 76), red(&updated, 76));
    drop(fresh);
    drop(graphics);
    window.destroy().unwrap();
}

#[cfg(target_os = "macos")]
fn framework_window_chrome_preserves_resize_zoom_and_rejects_dead_layers_and_invalid_minimums() {
    use crate::{Chrome, Window};
    let mut window = Window::open_with("PXUI window chrome", 800, 600, false, true, true).unwrap();
    window.set_resizable(true).unwrap();
    for minimum in [
        (f64::NAN, 480.0),
        (640.0, f64::INFINITY),
        (0.0, 480.0),
        (-1.0, 480.0),
    ] {
        assert!(Chrome::open(window.token().unwrap(), minimum).is_err());
    }
    let chrome = Chrome::open(window.token().unwrap(), (640.0, 480.0)).unwrap();
    for (width, height) in [(800, 600), (1024, 700), (640, 480)] {
        window.resize(width, height).unwrap();
        let state = chrome.snapshot().unwrap();
        assert_eq!(
            (
                state.resizable,
                state.zoom_enabled,
                state.full_size_content,
                state.full_screen_primary
            ),
            (1, 1, 1, 1)
        );
        assert!(state.top_inset > 0.0 && state.top_inset < 80.0);
        assert_eq!(
            (state.content_width, state.content_height),
            (width as f64, height as f64)
        );
    }
    window.destroy().unwrap();
    assert!(chrome.snapshot().is_err());
}

#[cfg(target_os = "macos")]
fn framework_host_constructor_refusals_release_devices_and_queues() {
    let mut window =
        crate::Window::open_with("PXUI refused host", 320, 240, false, true, true).unwrap();
    window.destroy().unwrap();
    let ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    let before = ogpu_metal::impl_::live_handles();
    for _ in 0..3 {
        assert!(crate::Graphics::open(&mut window, &ui).is_err());
    }
    assert_eq!(
        ogpu_metal::impl_::live_handles(),
        before,
        "failed construction must release devices/queues"
    );
}

#[cfg(target_os = "macos")]
fn framework_platform_applies_focused_ime_reuses_warm_frames_and_rejects_dead_windows() {
    use pxui::{Event, FrameIn, Ui};
    mem_sys::set_peer(pxui::library_counter);
    let mut window = crate::Window::open_with("PXUI platform output", 320, 240, false, true, true).unwrap();
    let mut platform = crate::Platform::default();
    let mut ui = Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    let mut build = |ui: &mut Ui| { assert!(ui.text_field("Caption", "café track 10", &pxui::TextField::DEFAULT).changed.is_none()); };
    let input = FrameIn::new(320.0, 240.0);
    ui.frame(&input, &mut build).unwrap();
    let focus = Event::key(Event::KEY_PRESSED, input::Key::Tab.code());
    ui.frame(&input.events(core::slice::from_ref(&focus)), &mut build).unwrap();
    assert!(ui.ime().is_some());
    for frame in 0..100 {
        mem_sys::local_reset();
        ui.frame(&input.time(0.2 + frame as f64 / 60.0), &mut build).unwrap();
        platform.apply(&mut window, &ui).unwrap();
        platform.apply(&mut window, &ui).unwrap();
        if frame >= 10 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm platform output allocated"); }
    }
    ui.frame(&input, &mut |_| {}).unwrap();
    assert!(ui.ime().is_none());
    platform.apply(&mut window, &ui).unwrap();
    window.destroy().unwrap();
    assert!(platform.apply(&mut window, &ui).is_err());
}

#[cfg(target_os = "macos")]
fn voiceover_bridge_maps_nodes_round_trips_actions_and_stays_quiet_when_unchanged() {
    use pxui::{FrameIn, Ui};
    use pxui_sdl::ax::test::{self as query, find};
    use std::cell::Cell;
    mem_sys::set_peer(pxui::library_counter);
    let mut window = crate::Window::open_with("PXUI accessibility", 320, 240, false, true, true).unwrap();
    let mut ax = crate::Accessibility::open(&mut window).unwrap();
    let mut ui = Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    ui.remove_surface(FrameIn::new(320.0, 240.0).wire.surface).unwrap();
    ui.add_surface(7).unwrap();
    let (seek, shuffle, pressed, hidden) = (Cell::new(25.0f32), Cell::new(false), Cell::new(0), Cell::new(false));
    let mut build = |ui: &mut Ui| {
        ui.label("Now playing", pxui::Label::DEFAULT);
        if ui.button("play", "Play", &pxui::Button::DEFAULT).clicked { pressed.set(pressed.get() + 1); }
        seek.set(ui.slider("seek", "Seek", seek.get(), 0.0..=100.0, &pxui::Slider::DEFAULT).value);
        if !hidden.get() { shuffle.set(shuffle.get() != ui.checkbox("shuffle", "Shuffle", shuffle.get(), &pxui::Checkbox::DEFAULT).clicked); }
    };
    let time = Cell::new(0.0);
    let step = |ui: &mut Ui, ax: &mut crate::Accessibility, build: &mut dyn FnMut(&mut Ui)| {
        ax.frame(ui, 7);
        time.set(time.get() + 1.0 / 60.0);
        ui.frame(&FrameIn::new(320.0, 240.0).surface(7).time(time.get()), build).unwrap();
    };
    // Off by default: no tree, no elements (skipped when VoiceOver really is running here).
    if !pxui_sdl::ax::Bridge::voiceover() {
        for _ in 0..3 { step(&mut ui, &mut ax, &mut build); }
        assert!(ui.a11y_nodes().is_empty());
        assert_eq!(query::top_level(ax.bridge()), 0);
    }
    ax.force(true);
    for _ in 0..4 { step(&mut ui, &mut ax, &mut build); }
    ax.frame(&mut ui, 7);

    let bridge = ax.bridge();
    let label = find(bridge, "Now playing").expect("label element");
    let play = find(bridge, "Play").expect("button element");
    assert_eq!((label.role(), label.value()), ("AXStaticText", "Now playing"));
    assert_eq!(play.role(), "AXButton");
    assert!(play.enabled != 0 && play.actionable != 0 && label.actionable == 0);
    assert!(play.frame[2] > 8.0 && play.frame[3] > 8.0, "frame {:?}", play.frame);
    assert!(label.frame[1] > play.frame[1], "screen y grows upward: the label sits above the button");
    let slider = find(bridge, "Seek").expect("slider element");
    assert_eq!(slider.role(), "AXSlider");
    assert_eq!((slider.has_number, slider.number, slider.min, slider.max), (1, 25.0, 0.0, 100.0));
    let check = find(bridge, "Shuffle").expect("checkbox element");
    assert_eq!((check.role(), check.has_number, check.number), ("AXCheckBox", 1, 0.0));
    assert!(query::top_level(bridge) > 0);

    // Press and increment queue actions that the next frame consumes as input.
    assert_eq!(query::perform(bridge, "Play", query::PRESS, 0.0), Some(true));
    assert_eq!(query::perform(bridge, "Now playing", query::PRESS, 0.0), Some(false), "labels have no press");
    step(&mut ui, &mut ax, &mut build);
    step(&mut ui, &mut ax, &mut build);
    assert_eq!(pressed.get(), 1, "a press reaches the next frame's intent exactly once");
    assert_eq!(query::perform(ax.bridge(), "Seek", query::INCREMENT, 0.0), Some(true));
    for _ in 0..2 { step(&mut ui, &mut ax, &mut build); }
    assert!(seek.get() > 25.0, "increment moved the slider to {}", seek.get());
    assert_eq!(query::perform(ax.bridge(), "Seek", query::SET_VALUE, 60.0), Some(true));
    for _ in 0..2 { step(&mut ui, &mut ax, &mut build); }
    assert_eq!(seek.get(), 60.0);

    // Settled and unchanged: no notifications, the same element objects, no Rust allocations.
    for _ in 0..4 { step(&mut ui, &mut ax, &mut build); }
    let (posted, identity) = (ax.notifications(), find(ax.bridge(), "Seek").unwrap().identity);
    assert_eq!(find(ax.bridge(), "Seek").unwrap().number, 60.0);
    for frame in 0..60 {
        mem_sys::local_reset();
        step(&mut ui, &mut ax, &mut build);
        if frame >= 5 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm frame with the bridge on allocated"); }
    }
    assert_eq!(ax.notifications(), posted, "an unchanged tree posts nothing");
    // A change posts, and keeps the element.
    shuffle.set(true);
    seek.set(80.0);
    for _ in 0..3 { step(&mut ui, &mut ax, &mut build); }
    assert!(ax.notifications() > posted, "value changes post notifications");
    let seek_now = find(ax.bridge(), "Seek").unwrap();
    assert_eq!((seek_now.identity, seek_now.number), (identity, 80.0));
    assert_eq!(find(ax.bridge(), "Shuffle").unwrap().number, 1.0);
    // An element that leaves the UI leaves the tree (with a notification); a malformed tree is refused.
    let posted = ax.notifications();
    hidden.set(true);
    for _ in 0..3 { step(&mut ui, &mut ax, &mut build); }
    assert!(find(ax.bridge(), "Shuffle").is_none() && ax.notifications() > posted);
    assert!(find(ax.bridge(), "Seek").is_some());
    let mut bad = [pxui_sdl::ax::Node { id: 1, parent: 7, role: 9, name: [0, 99], description: [0, 0], value: Default::default(), rect: [0.0; 4],
        states: 0, actions: 0, selection: [0; 2], child_count: 0, first_child: u32::MAX, next_sibling: u32::MAX, last_child: u32::MAX }];
    assert!(ax.bridge_mut().sync(&bad, b"", 0).is_err(), "out-of-range parent and string");
    bad[0].parent = u32::MAX;
    assert!(ax.bridge_mut().sync(&bad, b"", 0).is_err(), "string range past the text");
    assert!(find(ax.bridge(), "Seek").is_some(), "a refused tree leaves the last good one");
    // Turning it off empties the window and the tree.
    ax.force(false);
    for _ in 0..40 { step(&mut ui, &mut ax, &mut build); }
    if !pxui_sdl::ax::Bridge::voiceover() { assert_eq!(query::top_level(ax.bridge()), 0); }
    drop(ax);
    window.destroy().unwrap();
}

#[cfg(target_os = "macos")]
pub(crate) fn main() {
    harness::run(&[
        ("encoded_input_owns_text_and_reuses_dialog_and_numeric_storage",crate::events::tests::encoded_input_owns_text_and_reuses_dialog_and_numeric_storage),
        ("mip_chain_halves_by_alpha_weight_down_to_one_texel",crate::images::tests::mip_chain_halves_by_alpha_weight_down_to_one_texel),
        ("first_resume_batch_reuses_the_cold_event_reserve",crate::events::tests::first_resume_batch_reuses_the_cold_event_reserve),
        ("touch_payload_growth_owns_full_identities_and_reuses_warm_storage",crate::events::tests::touch_payload_growth_owns_full_identities_and_reuses_warm_storage),
        ("window_framework_platform_applies_focused_ime_reuses_warm_frames_and_rejects_dead_windows",framework_platform_applies_focused_ime_reuses_warm_frames_and_rejects_dead_windows),
        ("window_framework_host_uploads_exact_rgba_reuses_warm_frames_and_fences_texture_release",framework_host_uploads_exact_rgba_reuses_warm_frames_and_fences_texture_release),
        ("window_framework_host_screenshot_returns_presented_rgba_and_keeps_warm_draws_allocation_free",framework_host_screenshot_returns_presented_rgba_and_keeps_warm_draws_allocation_free),
        ("window_framework_window_chrome_preserves_resize_zoom_and_rejects_dead_layers_and_invalid_minimums",framework_window_chrome_preserves_resize_zoom_and_rejects_dead_layers_and_invalid_minimums),
        ("window_voiceover_bridge_maps_nodes_round_trips_actions_and_stays_quiet_when_unchanged",voiceover_bridge_maps_nodes_round_trips_actions_and_stays_quiet_when_unchanged),
        ("window_framework_host_constructor_refusals_release_devices_and_queues",framework_host_constructor_refusals_release_devices_and_queues),
        ("window_framework_host_damage_redraws_only_the_change_and_keeps_the_rest",framework_host_damage_redraws_only_the_change_and_keeps_the_rest),
        ("present_damage_tracks_three_rotating_drawables",crate::graphics::present_damage_tracks_three_rotating_drawables),
        ("window_present_pixels_match_canvas_with_partial_copies_and_fallbacks",present_pixels_match_canvas_with_partial_copies_and_fallbacks),
        ("window_present_mismatch_disables_partial_for_the_session",crate::graphics::present_mismatch_disables_partial_for_the_session),
        ("window_partial_equals_full",partial_equals_full),
        ("window_occluded_window_skips_gpu_and_redraws_on_exposure",occluded_window_skips_gpu_and_redraws_on_exposure),
        ("window_backdrop_blur_reads_the_canvas_and_preserves_undamaged_pixels",backdrop_blur_reads_the_canvas_and_preserves_undamaged_pixels),
        ("damage_rounds_out_pads_and_clamps_to_the_drawable",crate::graphics::damage_rounds_out_pads_and_clamps_to_the_drawable),
    ]);
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn main() {}
