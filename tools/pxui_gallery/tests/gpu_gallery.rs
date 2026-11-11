//! Gallery goldens and its idle budget, using the same public PXUI surface as the window app.
#[path = "../../t/png.rs"]
mod png;
#[path = "../../t/run.rs"]
mod run;

use pxui::{Renderer, Ui};
use pxui_gallery::gpu::{self, Gpu};
use pxui_gallery::*;
use pxui_types::{Event, Str, Size, ffi::{Input, Slice, ThemeSource}};
use std::time::Instant;

const DEPARTURE_MONO: &[u8] = include_bytes!("../../../assets/fonts/DepartureMono-Regular.otf");
const DUSK: &str = include_str!("../../../assets/themes/dusk.theme");

fn theme(source: &str) -> ThemeSource {
    ThemeSource { size: core::mem::size_of::<ThemeSource>() as u32, flags: 0, source: Str { ptr: source.as_ptr(), len: source.len() } }
}

fn pointer_event() -> Event {
    Event { kind: Event::POINTER_MOVED, code: 0, x: -100.0, y: -100.0, value: 0.0, text: Str::default() }
}

fn frame_at(count: i64, size: (i64, i64), density: i64, events: &[Event]) -> Input {
    Input {
        size: core::mem::size_of::<Input>() as u32, time: count as f64 / 60.0, dt: 1.0 / 60.0,
        frame: count, tick: count, logical_size: [size.0, size.1], pointer: [-100.0, -100.0],
        keys: Slice { ptr: core::ptr::null(), len: 0 }, buttons: Slice { ptr: core::ptr::null(), len: 0 },
        events: Slice { ptr: events.as_ptr(), len: events.len() }, pixel_scale: [density as f64; 2], mouse_delta: [0.0; 2], fps: 60.0,
    }
}

fn panel(theme: &pxui_types::theme::ThemeSnapshot) -> [u8; 4] {
    let c = theme.tokens.panel;
    [c.r, c.g, c.b, 255]
}

/// Build one page twice to warm its retained view, then render it through the public renderer.
fn shoot(gpu: &mut Gpu, theme: &ThemeSource, rtl: bool, size: (i64, i64), density: i64, ground: Option<[u8; 4]>, build: &mut dyn FnMut(&mut Ui)) -> Vec<u8> {
    let mut ui = Ui::create(pxui::Platform::Apple, None, &[DEPARTURE_MONO]);
    ui.set_theme(theme).unwrap_or_else(|code| panic!("theme failed ({code}): {}", ui.last_error()));
    ui.set_dir(rtl);
    let events = [pointer_event()];
    for count in 0..2 {
        let input = frame_at(count, size, density, &events);
        ui.input_frame(&input, &mut |ui: &mut Ui| { ui.view("page", 1, Size::Grow, Size::Grow, &mut *build); }).expect("frame");
    }
    let clear = ground.unwrap_or_else(|| panel(&ui.theme()));
    let mut renderer: Renderer = gpu.renderer(&ui).expect("PXUI renderer");
    let pixels = gpu.render(&mut renderer, (size.0 as u32, size.1 as u32), density as u32, clear);
    drop(renderer);
    pixels
}

/// The A0 ABI baseline: one warm forced page rebuild over 1,000 headless frames at 1x.
fn abi_baseline_pages() {
    let events = [pointer_event()];
    let theme = theme("");
    for (index, name) in PAGES.iter().enumerate() {
        let mut state = Gallery::new();
        let mut ui = Ui::create(pxui::Platform::Apple, None, &[DEPARTURE_MONO]);
        ui.set_theme(&theme).expect("default theme");
        let (mut times, mut allocations) = (Vec::with_capacity(1000), Vec::with_capacity(1000));
        for count in 0..1010 {
            let input = frame_at(count, (PAGE_W, PAGE_H), 1, &events);
            let revision = count as u64;
            let allocated = gpu::allocations();
            let started = Instant::now();
            ui.input_frame(&input, &mut |ui: &mut Ui| {
                ui.view("page", revision, Size::Grow, Size::Grow, &mut |ui: &mut Ui| page(ui, &mut state, index));
            }).expect("frame");
            if count >= 10 {
                times.push(started.elapsed().as_secs_f64() * 1e6);
                allocations.push(gpu::allocations() - allocated);
            }
        }
        times.sort_by(f64::total_cmp);
        allocations.sort_unstable();
        println!("PXUI ABI baseline page={name} median_us={:.2} allocations/frame={}", times[500], allocations[500]);
    }
}

fn compare(path: &str, size: (i64, i64), density: i64, pixels: &[u8], refresh: bool) -> i32 {
    let (w, h) = ((size.0 * density) as u32, (size.1 * density) as u32);
    if refresh && std::env::var("RAYS_UPDATE_FIXTURES").as_deref() == Ok("1") {
        std::fs::write(path, png::encode(w, h, pixels)).expect("refresh golden");
    }
    let (gw, gh, g) = png::decode(&std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}")));
    assert!((gw, gh) == (w, h), "{path}: golden is {gw}x{gh}, render is {w}x{h}");
    g.chunks(4).zip(pixels.chunks(4)).map(|(p, q)| (0..4).map(|k| (p[k] as i32 - q[k] as i32).abs()).max().unwrap_or(0)).max().unwrap_or(0)
}

fn gpu_gallery_pages() {
    let mut gpu = Gpu::open().expect("a GPU backend");
    let image = gpu.image(100, 60, &gpu::test_card(100, 60));
    let shots: Vec<_> = PAGES.iter().enumerate().map(|(i, p)| (i, p.to_lowercase(), false))
        .chain([(4, "layout_rtl".to_owned(), true)]).collect();
    let rays = theme("");
    let dusk = theme(DUSK);
    for (name, theme) in [("rays", &rays), ("dusk", &dusk)] {
        for (i, page_name, rtl) in &shots {
            for density in [1, 2] {
                let mut s = Gallery::new();
                s.image = Some(image);
                (s.dialog, s.menu_at) = (*i == 3, (*i == 3).then_some((360.0, 40.0)));
                let pixels = shoot(&mut gpu, theme, *rtl, (PAGE_W, PAGE_H), density, None, &mut |ui: &mut Ui| {
                    ui.view("page", 1, Size::Grow, Size::Grow, &mut |ui: &mut Ui| page(ui, &mut s, *i));
                });
                let path = format!("{}/fixtures/gallery_{page_name}_{name}_{density}x.png", env!("CARGO_MANIFEST_DIR"));
                let worst = compare(&path, (PAGE_W, PAGE_H), density, &pixels, true);
                println!("PXUI gallery {page_name} {name} {density}x: max channel difference {worst}, submits 1");
                assert!(worst <= 2, "gallery {page_name} under {name} at {density}x drifted: largest channel difference {worst}");
            }
        }
    }
    gpu.close();
    println!("PXUI gallery: {} pages x 2 themes x 2 densities match their goldens", shots.len());
}

fn abi_kit_panel() {
    let mut gpu = Gpu::open().expect("a GPU backend");
    let theme = theme("");
    for density in [1, 2] {
        let mut s = Gallery::new();
        let pixels = shoot(&mut gpu, &theme, false, (320, 300), density, Some([228, 139, 161, 255]), &mut |ui| kit_page(ui, &mut s));
        let path = format!("{}/../../fixtures/pxui_core/kit_panel_{density}x.png", env!("CARGO_MANIFEST_DIR"));
        let worst = compare(&path, (320, 300), density, &pixels, false);
        let limit = if cfg!(target_os = "macos") { 0 } else { 2 };
        assert!(worst <= limit, "the kit page at {density}x differs from kit_panel_{density}x.png by {worst}");
        println!("PXUI gallery kit page = kit_panel_{density}x.png: max channel difference {worst}");
    }
    gpu.close();
}

/// A `Ui` and its renderer drop in either order without leaving the `Ui` allocated (the library refuses to destroy a `Ui` under a live
/// renderer; the wrapper destroys it when the renderer goes).
fn ui_and_renderer_drop_in_any_order() {
    let gpu = Gpu::open().expect("a GPU backend");
    let round = |gpu: &Gpu, ui_first: bool| {
        let ui = Ui::create(pxui::Platform::Apple, None, &[DEPARTURE_MONO]);
        let renderer = gpu.renderer(&ui).expect("PXUI renderer");
        if ui_first { drop(ui); drop(renderer); } else { drop(renderer); drop(ui); }
    };
    round(&gpu, true);
    round(&gpu, false);
    let base = mem_sys::allocated_bytes();
    for round_at in 0..6 { round(&gpu, round_at % 2 == 0); }
    let grown = mem_sys::allocated_bytes().saturating_sub(base);
    println!("PXUI six Ui/renderer pairs dropped in both orders: {grown} bytes left allocated");
    // a leaked `Ui` holds its font system and atlases: megabytes each
    assert!(grown < 1 << 20, "a Ui leaked: {grown} bytes grew over six rounds");
    gpu.close();
}

/// A settled frame drawn again through the renderer allocates nothing: its layers' breaks live on the stack, its views and dirty
/// ranges are the retained ones, and the app's command buffer and pass are adopted into the objects of the frame before.
fn renderer_draw_settled() {
    let mut gpu = Gpu::open().expect("a GPU backend");
    let theme = theme("");
    let mut state = Gallery::new();
    let mut ui = Ui::create(pxui::Platform::Apple, None, &[DEPARTURE_MONO]);
    ui.set_theme(&theme).expect("default theme");
    let events = [pointer_event()];
    for count in 0..3 {
        let input = frame_at(count, (PAGE_W, PAGE_H), 1, &events);
        ui.input_frame(&input, &mut |ui: &mut Ui| { ui.view("page", 1, Size::Grow, Size::Grow, &mut |ui: &mut Ui| page(ui, &mut state, 0)); }).expect("frame");
    }
    let clear = panel(&ui.theme());
    let mut renderer: Renderer = gpu.renderer(&ui).expect("PXUI renderer");
    let (mut prepare, mut draw) = (0, 0);
    for round in 0..8 {
        gpu.render(&mut renderer, (PAGE_W as u32, PAGE_H as u32), 1, clear);
        if round >= 4 { (prepare, draw) = (prepare.max(gpu.prepare_allocations.get()), draw.max(gpu.draw_allocations.get())); }
    }
    drop(renderer);
    gpu.close();
    println!("PXUI renderer prepare and draw of a settled frame: {prepare} and {draw} allocations");
    // the app's command buffer and pass are adopted once and pointed at each frame's (`readopt_*`): nothing is made per frame
    assert_eq!((prepare, draw), (0, 0), "a settled frame's renderer prepare and draw allocate nothing");
}

fn gpu_gallery_idle() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_pxui_gallery"))
        .env("PXUI_GALLERY_HIDDEN", "1").env("PXUI_GALLERY_EXIT_AFTER", "2.5").env("PXUI_GALLERY_COUNT", "1")
        .output().expect("run the gallery");
    let text = String::from_utf8_lossy(&out.stdout);
    let field = |name: &str| -> u64 {
        let at = text.find(&format!("{name} ")).unwrap_or_else(|| panic!("no {name} in {text:?} ({})", String::from_utf8_lossy(&out.stderr)));
        text[at + name.len() + 1..].split_whitespace().next().and_then(|v| v.parse().ok()).expect("a count")
    };
    assert!(out.status.success(), "the gallery failed: {}", String::from_utf8_lossy(&out.stderr));
    let (frames, settled) = (field("frames"), field("settled"));
    assert!(frames >= 1 && settled == 0, "idle gallery: {frames} frames, {settled} after it settled");
    println!("PXUI gallery idle: {frames} frames in the first half second, {settled} in the next two");
}

fn main() {
    run::run(&[
        ("abi_kit_panel", abi_kit_panel),
        ("gpu_gallery_pages", gpu_gallery_pages),
        ("gpu_gallery_idle", gpu_gallery_idle),
        ("abi_baseline_pages", abi_baseline_pages),
        ("renderer_draw_settled", renderer_draw_settled),
        ("ui_and_renderer_drop_in_any_order", ui_and_renderer_drop_in_any_order),
    ]);
}
