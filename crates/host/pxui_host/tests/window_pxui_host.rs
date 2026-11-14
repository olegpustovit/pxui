#[cfg(target_os = "macos")]
#[path = "../../../../tools/t/window_harness.rs"]
#[allow(clippy::type_complexity)]
mod harness;

#[cfg(target_os = "macos")]
fn release_keepalive() {}

#[cfg(target_os = "macos")]
fn framework_host_uploads_exact_rgba_reuses_warm_frames_and_fences_texture_release() {
    use pxui::images::{Key, Path};
    use pxui::ui::{Fit, Node, Rgba, Size};
    use pxui::{FrameIn, Ui};
    use pxui_host::{Graphics, Window};
    mem_sys::set_peer(pxui::library_counter);
    let mut window =
        Window::open_with("PXUI resource lifecycle", 320, 240, false, true, true).unwrap();
    let mut ui = Ui::new().unwrap();
    let mut graphics = Graphics::open(&mut window, &ui).unwrap();
    graphics.configure_images(1.0, 64).unwrap();
    graphics.sync_images(1).unwrap();
    let epoch = graphics.images().epoch();
    let key = Key {
        id: 1,
        path: Path::new("memory:rgba").unwrap(),
        max_pixels: 64,
    };
    let pixels = [255, 0, 0, 255, 0, 128, 255, 128];
    assert!(
        graphics
            .upload_image(Key::default(), epoch, 1, None)
            .is_err()
    );
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
            .upload_image(key, epoch, 1, Some((2, 1, &pixels)))
            .unwrap()
    );
    let texture = graphics.image_texture(1).unwrap().clone();
    assert_eq!(
        ogpu_core::backend::read_texture(&texture, 8).unwrap(),
        pixels
    );
    assert_eq!(graphics.resident_image_bytes(), pixels.len());
    for frame in 0..32 {
        mem_sys::local_reset();
        assert!(
            !graphics
                .upload_image(key, epoch, 1, Some((2, 1, &pixels)))
                .unwrap()
        );
        ui.frame(
            &FrameIn {
                width: 320.0,
                height: 240.0,
                time: frame as f64 / 60.0,
                ..Default::default()
            },
            &mut |ui| {
                let node = ui.node(
                    "image",
                    Node {
                        w: Size::Px(64.0),
                        h: Size::Px(32.0),
                        ..Node::DEFAULT
                    },
                );
                let image = graphics
                    .images()
                    .get_at(1, "memory:rgba", 32.0, node.box_key)
                    .unwrap();
                ui.draw(&node, move |paint, r| {
                    paint.image_fit(
                        r.x,
                        r.y,
                        r.w,
                        r.h,
                        &image,
                        Fit::Cover,
                        Rgba {
                            r: 255,
                            g: 255,
                            b: 255,
                            a: 255,
                        },
                    )
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
    assert!(
        !graphics
            .upload_image(key, epoch, 2, Some((2, 1, &pixels)))
            .unwrap(),
        "old epochs cannot resurrect images"
    );
    for frame in 0..3 {
        ui.frame(
            &FrameIn {
                width: 320.0,
                height: 240.0,
                time: 1.0 + frame as f64 / 60.0,
                ..Default::default()
            },
            &mut |_| {},
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
fn framework_window_chrome_preserves_resize_zoom_and_rejects_dead_layers_and_invalid_minimums() {
    use pxui_host::{Chrome, Window};
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
        pxui_host::Window::open_with("PXUI refused host", 320, 240, false, true, true).unwrap();
    window.destroy().unwrap();
    let ui = pxui::Ui::new().unwrap();
    let before = ogpu_metal::impl_::live_handles();
    for _ in 0..3 {
        assert!(pxui_host::Graphics::open(&mut window, &ui).is_err());
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
    let mut window = pxui_host::Window::open_with("PXUI platform output", 320, 240, false, true, true).unwrap();
    let mut platform = pxui_host::Platform::default();
    let mut ui = Ui::new().unwrap();
    let mut build = |ui: &mut Ui| { assert!(ui.text_field("Caption", "café track 10").is_none()); };
    let input = FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() };
    ui.frame(&input, &mut build).unwrap();
    let focus = Event { kind: Event::KEY_PRESSED, code: input::Key::Tab.code(), ..Event::default() };
    ui.frame(&FrameIn { events: &focus, event_count: 1, ..input }, &mut build).unwrap();
    assert!(ui.ime().is_some());
    for frame in 0..100 {
        mem_sys::local_reset();
        ui.frame(&FrameIn { time: 0.2 + frame as f64 / 60.0, ..input }, &mut build).unwrap();
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
fn main() {
    harness::run(&[
        ("framework_platform_applies_focused_ime_reuses_warm_frames_and_rejects_dead_windows",framework_platform_applies_focused_ime_reuses_warm_frames_and_rejects_dead_windows),
        ("framework_host_uploads_exact_rgba_reuses_warm_frames_and_fences_texture_release",framework_host_uploads_exact_rgba_reuses_warm_frames_and_fences_texture_release),
        ("framework_window_chrome_preserves_resize_zoom_and_rejects_dead_layers_and_invalid_minimums",framework_window_chrome_preserves_resize_zoom_and_rejects_dead_layers_and_invalid_minimums),
        ("framework_host_constructor_refusals_release_devices_and_queues",framework_host_constructor_refusals_release_devices_and_queues),
    ]);
}

#[cfg(not(target_os = "macos"))]
fn main() {}
