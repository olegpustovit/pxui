//! The PXUI kit gallery as a native window app. `PXUI_GALLERY_PAGE=n` and
//! `PXUI_GALLERY_DUSK=1` choose the starting page and theme; the other switches and headless
//! screenshot controls are listed in the gallery README.
#[path = "../../t/png.rs"]
#[allow(dead_code)]
mod png;

use input::{Event as HostEvent, Key};
use ogpu_core::backend::{self as be, AcquireResult, ColorAttachment, RenderTarget, Surface};
use ogpu_core::render_pass::{Load, Store};
use ogpu_core::surface::{Configuration, Format, PresentMode};
use pxui::{Renderer, Ui};
use pxui_gallery::gpu::{self, fail, Gpu};
use pxui_gallery::*;
use pxui_types::{Event, Str, ffi::{Input, Slice}};
use std::time::Instant;

const PRAGMASEVKA: &[u8] = include_bytes!("../../../assets/fonts/Pragmasevka-Regular.ttf");

#[cfg(any(target_os = "macos", windows))]
mod host {
    pub use pxui_sdl::{Extent, Window};
    use pxui_sdl::CursorShape;

    pub fn open(w: i64, h: i64, hidden: bool) -> Result<Window, String> {
        Window::open_with("PXUI gallery", w, h, true, hidden, true)
    }
    pub fn wait(win: &mut Window, deadline: Option<f64>) { win.wait(deadline); }
    pub fn cursor(win: &mut Window, c: Option<u32>) {
        let shape = match c { Some(1) => CursorShape::HorizontalResize, Some(2) => CursorShape::VerticalResize, Some(3) => CursorShape::Text, _ => CursorShape::Default };
        let _ = win.set_cursor(shape);
    }
    pub fn clipboard(_: &mut Window, text: &str) { let _ = pxui_sdl::clipboard_set_text(text); }
    pub fn paste(_: &mut Window, events: &mut Vec<input::Event>) {
        events.insert(0, input::Event::Paste(pxui_sdl::clipboard_get_text().unwrap_or_default()));
    }
    pub fn background(win: &mut Window, c: (f64, f64, f64)) { let _ = win.set_background(c); }

    use pxui_types::ffi::{A11yAction, A11yTree};
    pub struct A11y;
    pub fn a11y(_: &mut Window) -> Option<A11y> { None }
    pub fn a11y_gone(_: &mut Window) {}
    pub fn a11y_update(_: &mut A11y, _: &A11yTree) {}
    pub fn a11y_pump(_: &mut A11y, _: &mut Vec<A11yAction>) -> bool { true }
}

#[cfg(target_os = "linux")]
mod host {
    pub use pxui_wayland::{Extent, Window};
    use pxui_wayland::CursorShape;

    pub fn open(w: i64, h: i64, hidden: bool) -> Result<Window, String> {
        Window::open_with("PXUI gallery", w, h, true, hidden, true)
    }
    pub fn wait(win: &mut Window, deadline: Option<f64>) { win.wait(deadline); }
    pub fn cursor(win: &mut Window, c: Option<u32>) {
        let shape = match c { Some(1) => CursorShape::HorizontalResize, Some(2) => CursorShape::VerticalResize, Some(3) => CursorShape::Text, _ => CursorShape::Default };
        let _ = win.set_cursor(shape);
    }
    pub fn clipboard(win: &mut Window, text: &str) { let _ = win.clipboard_set_text(text); }
    pub fn paste(win: &mut Window, _: &mut Vec<input::Event>) { let _ = win.request_paste(); }
    pub fn background(_: &mut Window, _: (f64, f64, f64)) {}

    use pxui_types::ffi::{A11yAction, A11yTree};
    pub use pxui::A11yBridge as A11y;
    pub fn a11y(win: &mut Window) -> Option<A11y> {
        let bridge = A11y::connect("PXUI gallery")?;
        win.set_wake_fd(bridge.fd());
        Some(bridge)
    }
    pub fn a11y_gone(win: &mut Window) { win.set_wake_fd(-1); }
    pub fn a11y_update(b: &mut A11y, tree: &A11yTree) {
        if !b.update(tree) { eprintln!("pxui_gallery: accessibility tree refused"); }
    }
    pub fn a11y_pump(b: &mut A11y, out: &mut Vec<A11yAction>) -> bool { b.pump(out) }
}

fn env(name: &str) -> Option<String> { std::env::var(name).ok().filter(|v| !v.is_empty()) }

fn load_theme(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("pxui_gallery: {path}: {e}");
        String::new()
    })
}

fn modified(path: &str) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn apply_theme(ui: &mut Ui, s: &Gallery, dusk: &mut String, path: &str) {
    let source = s.theme(dusk);
    if ui.set_theme(&source).is_err() {
        let error = ui.last_error().to_owned();
        eprintln!("pxui_gallery: {path}: {error}");
        dusk.clear();
        let fallback = s.theme(dusk);
        if let Err(code) = ui.set_theme(&fallback) {
            eprintln!("pxui_gallery: default theme failed ({code})");
        }
    }
}

fn encode_events(events: &[HostEvent], raw: &mut Vec<Event>, scratch: &mut Vec<String>) {
    raw.clear();
    scratch.clear();
    scratch.resize(events.len(), String::new());
    for (event, scratch) in events.iter().zip(scratch.iter_mut()) {
        let r = event.raw(scratch);
        let text = r.text.unwrap_or("");
        raw.push(Event { kind: r.kind, code: r.code, x: r.x, y: r.y, value: r.value, text: Str { ptr: text.as_ptr(), len: text.len() } });
    }
}

fn configuration(win: &mut host::Window) -> Configuration {
    let extent: host::Extent = win.extent();
    Configuration {
        logical_width: extent.logical.0, logical_height: extent.logical.1,
        physical_width: extent.drawable.0, physical_height: extent.drawable.1,
        format: Format::Bgra8Unorm,
        present_mode: if win.vsync() { PresentMode::Fifo } else { PresentMode::Immediate },
        max_acquired: 1, layer: win.token().ok(),
    }
}

fn output_text(value: Str) -> String {
    if value.len == 0 || value.ptr.is_null() { return String::new(); }
    let bytes = unsafe { std::slice::from_raw_parts(value.ptr, value.len) };
    std::str::from_utf8(bytes).unwrap_or_default().to_owned()
}

fn main() {
    let shot = env("PXUI_GALLERY_SHOT");
    let hidden = env("PXUI_GALLERY_HIDDEN").is_some();
    let exit_after = env("PXUI_GALLERY_EXIT_AFTER").and_then(|v| v.parse::<f64>().ok());
    let count = env("PXUI_GALLERY_COUNT").is_some();
    let theme_path = env("PXUI_GALLERY_THEME").unwrap_or_else(|| format!("{}/../../assets/themes/dusk.theme", env!("CARGO_MANIFEST_DIR")));
    let mut dusk = load_theme(&theme_path);
    let mut theme_stamp = modified(&theme_path);
    let mut next_theme_poll = 0.0;
    let mut s = Gallery::new();
    s.page = env("PXUI_GALLERY_PAGE").and_then(|v| v.parse().ok()).unwrap_or(0) % PAGES.len();
    s.dusk = env("PXUI_GALLERY_DUSK").is_some();
    (s.rtl, s.compact, s.host_dirty) = (env("PXUI_GALLERY_RTL").is_some(), env("PXUI_GALLERY_COMPACT").is_some(), true);
    let mut win = match host::open(WINDOW_W, WINDOW_H, hidden) {
        Ok(w) => w,
        Err(e) => { eprintln!("pxui_gallery: no window: {e}"); std::process::exit(1) }
    };
    let mut gpu = Gpu::open().unwrap_or_else(|e| { eprintln!("pxui_gallery: no GPU: {e}"); std::process::exit(1) });
    let surface: Option<Surface> = if hidden { None } else { Some(be::create_surface(&gpu.device, configuration(&mut win)).unwrap_or_else(|e| fail(e))) };
    let mut ui = Ui::create(if cfg!(target_os = "macos") { pxui::Platform::Apple } else { pxui::Platform::Other }, None, &[PRAGMASEVKA]);
    apply_theme(&mut ui, &s, &mut dusk, &theme_path);
    let mut renderer = gpu.renderer(&ui).unwrap_or_else(|e| { eprintln!("pxui_gallery: {e}"); std::process::exit(1) });
    s.image = Some(gpu.image(100, 60, &gpu::test_card(100, 60)));
    let mut extent = win.extent();
    let mut target = gpu.target(extent.drawable.0 as u32, extent.drawable.1 as u32);
    let mut a11y = host::a11y(&mut win);
    ui.set_a11y(a11y.is_some());
    let mut actions = Vec::with_capacity(16);
    let mut a11y_seen = 0;
    let mut events: Vec<HostEvent> = Vec::with_capacity(256);
    let mut raw_events = Vec::with_capacity(256);
    let mut event_text = Vec::new();
    let mut key_values = Vec::with_capacity(8);
    let mut button_values = Vec::with_capacity(4);
    let mut frame = input::frame::Frame { input: input::at_time(0.0), pixel_scale: (1.0, 1.0), fps: 60.0, mouse_delta: (0.0, 0.0) };
    frame.input.keys.reserve(8);
    frame.input.buttons.reserve(4);
    let (mut frames, mut draws, mut settled) = (0u64, 0u64, 0u64);
    let mut drawn_revision = u64::MAX;
    let mut last = 0.0;
    let mut cost = Cost::default();
    let mut text = String::with_capacity(160);
    let mut force = true;
    let started = Instant::now();
    mem_sys::local_reset();
    loop {
        events.clear();
        if !win.pump(&mut events) { break; }
        let now = win.time();
        if now >= next_theme_poll {
            next_theme_poll = now + 1.0;
            let stamp = modified(&theme_path);
            if stamp != theme_stamp {
                theme_stamp = stamp;
                s.reload = true;
            }
        }
        if s.reload {
            s.reload = false;
            dusk = load_theme(&theme_path);
            if s.dusk { s.host_dirty = true; }
            s.toast("Theme file reloaded");
            force = true;
        }
        if s.host_dirty {
            apply_theme(&mut ui, &s, &mut dusk, &theme_path);
            ui.set_dir(s.rtl);
            (s.page_rev, s.chrome_rev, s.host_dirty, force) = (s.page_rev + 1, s.chrome_rev + 1, false, true);
        }
        if let Some(bridge) = a11y.as_mut() {
            actions.clear();
            if !host::a11y_pump(bridge, &mut actions) {
                host::a11y_gone(&mut win);
                (a11y, a11y_seen) = (None, 0);
                ui.set_a11y(false);
            } else if !actions.is_empty() {
                ui.inject_a11y(&actions);
                force = true;
            }
        }
        if win.extent() != extent {
            extent = win.extent();
            if let Some(surface) = &surface { be::configure(surface, configuration(&mut win)).unwrap_or_else(|e| fail(e)); }
            be::destroy_texture(&target).unwrap_or_else(|e| fail(e));
            target = gpu.target(extent.drawable.0 as u32, extent.drawable.1 as u32);
            force = true;
        }
        encode_events(&events, &mut raw_events, &mut event_text);
        let built = force || ui.wants_frame(&raw_events, now);
        if built {
            force = false;
            if ui.output().platform.paste != 0 { host::paste(&mut win, &mut events); }
            let f = &mut frame.input;
            for e in &events {
                match e {
                    HostEvent::PointerMoved(p) => f.pointer = *p,
                    HostEvent::PointerPressed(b, _) if !f.buttons.contains(b) => f.buttons.push(*b),
                    HostEvent::PointerReleased(b, _) => f.buttons.retain(|x| x != b),
                    HostEvent::KeyPressed(k @ (Key::Shift | Key::Ctrl | Key::Alt | Key::Meta)) if !f.keys.contains(k) => f.keys.push(*k),
                    HostEvent::KeyReleased(k) => f.keys.retain(|x| x != k),
                    HostEvent::FocusLost => { f.keys.clear(); f.buttons.clear(); }
                    _ => {}
                }
            }
            (f.t, f.dt, f.frame, f.tick) = (now, now - last, frames as i64, frames as i64);
            f.size = extent.logical;
            f.events = std::mem::take(&mut events);
            frame.pixel_scale = (win.scale(), win.scale());
            last = now;
            key_values.clear();
            key_values.extend(f.keys.iter().copied().map(Key::code));
            button_values.clear();
            button_values.extend(f.buttons.iter().copied().map(input::MouseButton::code));
            encode_events(&f.events, &mut raw_events, &mut event_text);
            let input = Input {
                size: core::mem::size_of::<Input>() as u32, time: f.t, dt: f.dt, frame: f.frame, tick: f.tick,
                logical_size: [f.size.0, f.size.1], pointer: [f.pointer.0, f.pointer.1],
                keys: Slice { ptr: key_values.as_ptr(), len: key_values.len() },
                buttons: Slice { ptr: button_values.as_ptr(), len: button_values.len() },
                events: Slice { ptr: raw_events.as_ptr(), len: raw_events.len() },
                pixel_scale: [frame.pixel_scale.0, frame.pixel_scale.1], mouse_delta: [frame.mouse_delta.0, frame.mouse_delta.1], fps: frame.fps,
            };
            let a0 = mem_sys::local_alloc_count();
            let t0 = Instant::now();
            // the frame borrows the Ui until it is dropped, so the error text is read after it
            if let Err(code) = ui.input_frame(&input, &mut |ui: &mut Ui| app(ui, &mut s)).map(|_| ()) { eprintln!("pxui_gallery: frame failed ({code}): {}", ui.last_error()); std::process::exit(1) }
            cost.us = t0.elapsed().as_secs_f64() * 1e6;
            cost.allocs = Some(mem_sys::local_alloc_count() - a0);
            events = std::mem::take(&mut frame.input.events);
            let output = ui.output();
            (cost.nodes, cost.views, cost.laid_out) = (output.built_nodes as usize, output.rebuilt_views as usize, output.arranged_nodes as usize);
            frames += 1;
            cost.frames = frames;
            if started.elapsed().as_secs_f64() > 0.5 { settled += 1; }
            let out = ui.output().platform;
            host::cursor(&mut win, (out.has_cursor != 0).then_some(out.cursor));
            if out.has_clipboard != 0 { host::clipboard(&mut win, &output_text(out.clipboard)); }
            if out.has_background != 0 {
                let c = out.background;
                host::background(&mut win, (c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0));
            }
            if let Some(bridge) = a11y.as_mut() {
                let tree = ui.output().a11y;
                if tree.revision != a11y_seen {
                    a11y_seen = tree.revision;
                    host::a11y_update(bridge, &tree);
                }
            }
            readout(&mut text, &cost);
            if let Some(h) = s.readout { ui.patch_text(&h, &text); }
            if s.quit { break; }
        }
        let animating = s.page == 1 && !s.reduced;
        if animating {
            for h in &s.busy { ui.busy_step(h, now); }
        }
        if ui.draw_revision() != drawn_revision {
            drawn_revision = ui.draw_revision();
            draw(&mut gpu, &mut ui, &mut renderer, &target, surface.as_ref(), extent, win.scale() as f32);
            draws += 1;
            if let Some(path) = shot.as_deref().filter(|_| draws >= 3) {
                let (w, h) = (extent.drawable.0 as u32, extent.drawable.1 as u32);
                let pixels = be::read_texture(&target, (w * 4) as i64).unwrap_or_else(|e| fail(e));
                if let Err(e) = std::fs::write(path, png::encode(w, h, &pixels)) { eprintln!("pxui_gallery: {path}: {e}"); }
                break;
            }
        }
        if exit_after.is_some_and(|secs| now >= secs) { break; }
        let mut wake = ui.repaint_at().unwrap_or(f64::INFINITY).min(next_theme_poll);
        if animating { wake = wake.min(now + 1.0 / 60.0); }
        if shot.is_some() && draws < 3 { (wake, force) = (now, true); }
        if let Some(secs) = exit_after { wake = wake.min(secs); }
        if wake > win.time() { host::wait(&mut win, wake.is_finite().then_some(wake)); }
    }
    if count { println!("pxui_gallery: frames {frames} draws {draws} settled {settled}"); }
    s.image = None;
    drop(renderer);
    be::destroy_texture(&target).unwrap_or_else(|e| fail(e));
    if let Some(surface) = surface { be::destroy_surface(&surface).unwrap_or_else(|e| fail(e)); }
    drop(ui);
    gpu.close();
}

fn draw(gpu: &mut Gpu, ui: &mut Ui, renderer: &mut Renderer, target: &be::Texture, surface: Option<&Surface>, extent: host::Extent, scale: f32) {
    let ground = ui.theme().tokens.panel;
    let frame = match surface {
        Some(surface) => match be::acquire_sync(surface).unwrap_or_else(|e| fail(e)) { AcquireResult::Acquired(frame) => Some(frame), _ => return },
        None => None,
    };
    let commands = be::begin_commands(&gpu.queue).unwrap_or_else(|e| fail(e));
    gpu.prepare(renderer, &commands).unwrap_or_else(|e| { eprintln!("pxui_gallery: {e}"); std::process::exit(1) });
    let encoder = be::render_encoder(&commands, &RenderTarget {
        colors: vec![ColorAttachment { texture: target.clone(), resolve: None, load: Load::Clear, store: Store::Store, clear: (ground.r as f64 / 255.0, ground.g as f64 / 255.0, ground.b as f64 / 255.0, 1.0) }],
        depth: None,
    }).unwrap_or_else(|e| fail(e));
    gpu.draw(renderer, &commands, &encoder, [extent.drawable.0 as u32, extent.drawable.1 as u32], scale).unwrap_or_else(|e| { eprintln!("pxui_gallery: {e}"); std::process::exit(1) });
    be::end_render(&encoder).unwrap_or_else(|e| fail(e));
    let receipt = match frame {
        Some(frame) => be::commit_present(&commands, target, &frame).unwrap_or_else(|e| fail(e)),
        None => be::commit(&commands).unwrap_or_else(|e| fail(e)),
    };
    gpu.complete(&receipt);
}
