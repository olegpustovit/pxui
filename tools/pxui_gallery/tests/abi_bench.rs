//! The A7 performance gate (`RAYS_SLOW=1 cargo t -p pxui_gallery abi_bench`, in each link mode; `tools/validate.sh` runs both).
//! The gallery's own pages are built three ways: through the engine directly (`abi_bench`, a tool that names `pxui_core`), through
//! the wrapper linked statically and through the wrapper against the built `libpxui`; this binary is whichever of the last two the
//! build selected. The limits are the plan's; none is relaxed to pass. Each case prints its numbers; `docs/abi-baseline.md`
//! records them.
#[path = "../../t/run.rs"]
mod run;

use abi_bench::{gallery as direct_pages, pxui as engine};
use input::{frame::Frame, Event as HostEvent, Platform};
use pxui_gallery::{page, Gallery, PAGES, PAGE_H, PAGE_W};
use pxui_types::{geom::Rgba, Event, FrameIn, Size, Str};
use std::{io::{BufRead, BufReader}, process::{Command, Stdio}, time::{Duration, Instant}};

const DEPARTURE_MONO: &[u8] = include_bytes!("../../../assets/fonts/DepartureMono-Regular.otf");
const STATIC: bool = cfg!(feature = "static");
const MODE: &str = if STATIC { "static" } else { "dynamic" };
/// Samples per page and side, interleaved so drift and thermal state hit both alike.
const SAMPLES: usize = 600;
/// `pxui_abi`'s smallest span: a drawing of N shapes asks for a new one at most every this many.
const SPAN: u64 = 256;
/// Draw calls per page at 1x in the default theme: the A0 baseline (the engine's, before the migration).
const DRAW_CALLS: [u64; 5] = [1, 2, 1, 1, 1];

fn slow() -> bool {
    let on = std::env::var("RAYS_SLOW").as_deref() == Ok("1");
    if !on { println!("PXUI abi_bench: skipped (set RAYS_SLOW=1)"); }
    on
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

fn pointer() -> Event {
    Event { kind: Event::POINTER_MOVED, code: 0, x: -100.0, y: -100.0, value: 0.0, text: Str::default() }
}

use pxui_gallery::gpu::allocations;

struct Wrapped { ui: pxui::Ui, state: Gallery, events: [Event; 1], moved: bool }
struct Direct { ui: engine::Ui, state: direct_pages::Gallery, frame: Frame, moved: bool }

fn wrapped() -> Wrapped {
    let mut ui = pxui::Ui::create(pxui::Platform::Apple, None, &[DEPARTURE_MONO]);
    ui.set_theme(&pxui_types::ffi::ThemeSource { size: core::mem::size_of::<pxui_types::ffi::ThemeSource>() as u32, flags: 0, source: Str::default() }).expect("default theme");
    Wrapped { ui, state: Gallery::new(), events: [pointer()], moved: true }
}

fn direct() -> Direct {
    let mut input = input::at_time(0.0);
    input.size = (PAGE_W, PAGE_H);
    input.pointer = (-100.0, -100.0);
    input.events = vec![HostEvent::PointerMoved((-100.0, -100.0))];
    let frame = Frame { input, pixel_scale: (1.0, 1.0), fps: 60.0, mouse_delta: (0.0, 0.0) };
    Direct { ui: engine::Ui::create(Platform::Apple, None, engine::text::Text::new(&[DEPARTURE_MONO])), state: direct_pages::Gallery::new(), frame, moved: true }
}

/// One frame of page `index` whose view has revision `revision` (a new one rebuilds the page, the same one settles).
fn wrapped_frame(w: &mut Wrapped, index: usize, revision: u64, count: u64) {
    let input = FrameIn { width: PAGE_W as f32, height: PAGE_H as f32, scale: 1.0, time: count as f64 / 60.0, events: w.events.as_ptr(), event_count: w.moved as usize, ..FrameIn::default() };
    let state = &mut w.state;
    w.ui.frame(&input, &mut |ui: &mut pxui::Ui| { ui.view("page", revision, Size::Grow, Size::Grow, &mut |ui: &mut pxui::Ui| page(ui, state, index)); }).expect("frame");
}

fn direct_frame(d: &mut Direct, index: usize, revision: u64, count: u64) {
    d.frame.input.t = count as f64 / 60.0;
    d.frame.input.events.clear();
    if d.moved { d.frame.input.events.push(HostEvent::PointerMoved((-100.0, -100.0))); }
    let state = &mut d.state;
    d.ui.frame(&d.frame, &mut |ui: &mut engine::Ui| { ui.view("page", revision, Size::Grow, Size::Grow, &mut |ui: &mut engine::Ui| direct_pages::page(ui, state, index)); });
}

fn timed(f: &mut dyn FnMut()) -> f64 {
    let started = Instant::now();
    f();
    started.elapsed().as_secs_f64() * 1e6
}

/// Page build time (median over interleaved forced rebuilds), allocations per rebuild and per settled frame, nodes built.
fn abi_bench_pages() {
    if !slow() { return; }
    let (mut worst_per_thousand, mut failures) = (f64::MIN, Vec::<String>::new());
    for (index, name) in PAGES.iter().enumerate() {
        let (mut w, mut d) = (wrapped(), direct());
        let (mut tw, mut td) = (Vec::with_capacity(SAMPLES), Vec::with_capacity(SAMPLES));
        let (mut aw, mut ad) = (0, 0);
        let mut nodes = 0;
        for count in 0..(20 + SAMPLES as u64) {
            let revision = count + 1;
            // alternate who goes first, so neither side always inherits the other's cache
            let (mut build_w, mut build_d) = (|| wrapped_frame(&mut w, index, revision, count), || direct_frame(&mut d, index, revision, count));
            let (before_w, before_d);
            let (us_w, us_d);
            if count % 2 == 0 {
                before_w = allocations(); us_w = timed(&mut build_w); aw = allocations() - before_w;
                before_d = allocations(); us_d = timed(&mut build_d); ad = allocations() - before_d;
            } else {
                before_d = allocations(); us_d = timed(&mut build_d); ad = allocations() - before_d;
                before_w = allocations(); us_w = timed(&mut build_w); aw = allocations() - before_w;
            }
            if count >= 20 { tw.push(us_w); td.push(us_d); }
            nodes = w.ui.output().built_nodes as i64;
        }
        // settled: the same revision again, with no input, builds nothing and allocates nothing, on either side
        (w.moved, d.moved) = (false, false);
        let revision = SAMPLES as u64 + 21;
        let (mut sw, mut sd) = (0, 0);
        for round in 0..12 {
            let count = revision + round;
            let before = allocations();
            wrapped_frame(&mut w, index, revision, count);
            if round >= 4 { sw = sw.max(allocations() - before); }
            let before = allocations();
            direct_frame(&mut d, index, revision, count);
            if round >= 4 { sd = sd.max(allocations() - before); }
        }
        let (mw, md) = (median(&mut tw), median(&mut td));
        let per_thousand = (mw - md) / (nodes.max(1) as f64 / 1000.0);
        worst_per_thousand = worst_per_thousand.max(per_thousand);
        println!("PXUI abi_bench {MODE} page={name} direct_us={md:.2} wrapper_us={mw:.2} ratio={:.4} delta_us_per_1000_nodes={per_thousand:.2} nodes={nodes} rebuild_allocs direct={ad} wrapper={aw} settled_allocs direct={sd} wrapper={sw}", mw / md);
        // ponytail: the Layout page's anchored popup is a root box, which the engine rebuilds the view for on every frame (by design, see
        // `Ui::escape`), so that page never settles in any build; there the wrapper must only match the engine. Move the popup out of
        // the cached view to gate it at zero too.
        let settles = index != 4;
        if (settles && (sd, sw) != (0, 0)) || sw > sd { failures.push(format!("{name}: a settled frame allocates nothing in any build (direct {sd}, wrapper {sw})")); }
        if STATIC && mw > md * 1.01 { failures.push(format!("{name}: wrapper static build {mw:.2} us is over 1% above the direct engine's {md:.2} us")); }
        if !STATIC && (mw > md * 1.03 || per_thousand >= 5.0) { failures.push(format!("{name}: wrapper dynamic build {mw:.2} us against direct {md:.2} us ({per_thousand:.2} us per 1000 nodes) is over 3% or 5 us per 1000 nodes")); }
    }
    println!("PXUI abi_bench {MODE} worst delta {worst_per_thousand:.2} us per 1000 nodes");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The wrapper's drawing is the engine's: same layers, same instances, same breaks (static link only, where both are one binary).
#[cfg(feature = "static")]
fn abi_bench_instances() {
    if !slow() { return; }
    for (index, name) in PAGES.iter().enumerate() {
        let (mut w, mut d) = (wrapped(), direct());
        for count in 0..3 {
            wrapped_frame(&mut w, index, 1, count);
            direct_frame(&mut d, index, 1, count);
        }
        let from_wrapper = unsafe { &*w.ui.engine_ptr().cast::<engine::Ui>() };
        let (a, b) = (from_wrapper.output().draw, d.ui.output().draw);
        assert_eq!(a.len(), b.len(), "{name}: layer count");
        let mut bytes = 0;
        for (la, lb) in a.iter().zip(b) {
            assert_eq!(la.instances, lb.instances, "{name}: instances differ");
            assert_eq!(la.breaks.len(), lb.breaks.len(), "{name}: breaks differ");
            bytes += la.instances.len() * core::mem::size_of::<pxui_types::draw::Instance>();
        }
        println!("PXUI abi_bench static page={name} layers={} instance_bytes={bytes} identical", a.len());
    }
}

/// The dynamic build cannot see the engine behind the library; the static build's identical instances stand for it.
#[cfg(not(feature = "static"))]
fn abi_bench_instances() {}

/// Draw calls and submits per rendered frame, through the app's own GPU, in this build's link mode.
fn abi_bench_gpu() {
    if !slow() { return; }
    let mut gpu = pxui_gallery::gpu::Gpu::open().expect("a GPU backend");
    let image = gpu.image(100, 60, &pxui_gallery::gpu::test_card(100, 60));
    for (index, name) in PAGES.iter().enumerate() {
        let mut w = wrapped();
        w.state.image = Some(image);
        for count in 0..2 { wrapped_frame(&mut w, index, 1, count); }
        let mut renderer = gpu.renderer(&w.ui).expect("PXUI renderer");
        let clear = [0, 0, 0, 255];
        gpu.render(&mut renderer, (PAGE_W as u32, PAGE_H as u32), 1, clear);
        let (calls, epoch) = (renderer.draw_calls(), gpu.epoch.get());
        gpu.render(&mut renderer, (PAGE_W as u32, PAGE_H as u32), 1, clear);
        let submits = gpu.epoch.get() - epoch;
        println!("PXUI abi_bench {MODE} page={name} draw_calls={calls} submits_per_render={submits}");
        assert_eq!(calls, DRAW_CALLS[index], "{name}: draw calls differ from the engine's baseline");
        assert_eq!(submits, 1, "{name}: a rendered frame is exactly the app's one submit");
        drop(renderer);
    }
    gpu.close();
}

/// A paint closure of N shapes and no text costs at most N / span + 1 more library calls than one of none: the plan's bound, with a
/// partly filled span counted as a span (the first span is asked for, the last one committed).
fn abi_bench_paint_calls() {
    if !slow() { return; }
    let mut ui = pxui::Ui::create(pxui::Platform::Apple, None, &[DEPARTURE_MONO]);
    let ink = Rgba { r: 20, g: 120, b: 220, a: 255 };
    let mut calls = |shapes: u64, count: u64| {
        let input = FrameIn { width: 300.0, height: 200.0, scale: 1.0, time: count as f64 / 60.0, ..FrameIn::default() };
        let before = pxui::library_counter(pxui::COUNTER_CALLS);
        ui.frame(&input, &mut |ui: &mut pxui::Ui| {
            let b = ui.node("canvas", pxui::Node { w: Size::Px(300.0), h: Size::Px(200.0), ..pxui::Node::DEFAULT });
            ui.canvas(&b, move |p, _| for i in 0..shapes { p.fill((i % 300) as f32, (i / 300) as f32, 1.0, 1.0, ink); });
        }).expect("frame");
        pxui::library_counter(pxui::COUNTER_CALLS) - before - 1
    };
    for count in 0..4 { calls(0, count); }
    let empty = calls(0, 4);
    let mut failures = Vec::new();
    for shapes in [100u64, 1_000, 10_000] {
        let drawn = calls(shapes, 5);
        let extra = drawn - empty;
        println!("PXUI abi_bench {MODE} paint shapes={shapes} library_calls={drawn} extra={extra} limit={}", shapes.div_ceil(SPAN) + 1);
        if extra > shapes.div_ceil(SPAN) + 1 { failures.push(format!("{shapes} shapes cost {extra} library calls, over {} (N / span, rounded up, + 1)", shapes.div_ceil(SPAN) + 1)); }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn gallery(env: &[(&str, &str)]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pxui_gallery"));
    command.env("PXUI_GALLERY_HIDDEN", "1");
    for (key, value) in env { command.env(key, value); }
    command
}

/// The idle gallery makes no library call once it has settled.
fn abi_bench_idle() {
    if !slow() { return; }
    let out = gallery(&[("PXUI_GALLERY_EXIT_AFTER", "2.5"), ("PXUI_GALLERY_COUNT", "1")]).output().expect("run the gallery");
    let text = String::from_utf8_lossy(&out.stdout);
    let field = |name: &str| -> u64 {
        let at = text.find(&format!("{name} ")).unwrap_or_else(|| panic!("no {name} in {text:?}"));
        text[at + name.len() + 1..].split_whitespace().next().and_then(|v| v.parse().ok()).expect("a count")
    };
    println!("PXUI abi_bench {MODE} idle: {} frames after settling, {} library calls", field("settled"), field("idle_calls"));
    assert_eq!((field("settled"), field("idle_calls")), (0, 0), "an idle gallery calls the library");
}

/// Resident memory (median) and the time to the first presented frame (fastest) of a one-window, one-label app over 31 runs. The dynamic build
/// is compared with the static build's numbers (left in the build directory by its own run) when they are there.
fn abi_bench_start() {
    if !slow() { return; }
    let (mut times, mut rss) = (Vec::new(), Vec::new());
    for _ in 0..31 {
        let started = Instant::now();
        let mut child = gallery(&[("PXUI_GALLERY_LABEL", "1"), ("PXUI_GALLERY_EXIT_AFTER", "5")]).stdout(Stdio::piped()).spawn().expect("run the gallery");
        let mut lines = BufReader::new(child.stdout.take().expect("stdout"));
        let mut line = String::new();
        while !line.contains("first-frame") {
            line.clear();
            assert!(lines.read_line(&mut line).expect("read") > 0, "the label app exited before its first frame");
        }
        times.push(started.elapsed().as_secs_f64() * 1e3);
        std::thread::sleep(Duration::from_millis(100));
        let ps = Command::new("ps").args(["-o", "rss=", "-p", &child.id().to_string()]).output().expect("ps");
        rss.push(String::from_utf8_lossy(&ps.stdout).trim().parse::<f64>().expect("an rss in KiB"));
        let _ = child.kill();
        let _ = child.wait();
    }
    // start-up noise (other processes, the page cache) only ever adds, so the fastest run is the one to compare; the median is printed
    let first_ms = times.iter().copied().fold(f64::MAX, f64::min);
    let (typical_ms, rss_kib) = (median(&mut times), median(&mut rss));
    println!("PXUI abi_bench {MODE} label app: first frame {first_ms:.1} ms fastest of 31 ({typical_ms:.1} median), resident {rss_kib:.0} KiB");
    let dir = std::env::current_exe().expect("exe").ancestors().nth(2).expect("target dir").to_path_buf();
    std::fs::write(dir.join(format!("abi_bench_{MODE}.txt")), format!("{first_ms} {rss_kib}\n")).expect("record");
    if !STATIC {
        let Ok(text) = std::fs::read_to_string(dir.join("abi_bench_static.txt")) else { println!("PXUI abi_bench: no static numbers to compare with (run the static build first)"); return };
        let numbers: Vec<f64> = text.split_whitespace().filter_map(|v| v.parse().ok()).collect();
        println!("PXUI abi_bench static label app: first frame {:.1} ms, resident {:.0} KiB", numbers[0], numbers[1]);
        assert!(first_ms <= numbers[0] + 2.0, "dynamic first frame {first_ms:.1} ms is over 2 ms past static {:.1} ms", numbers[0]);
        assert!(rss_kib <= numbers[1], "dynamic resident {rss_kib:.0} KiB is above static {:.0} KiB", numbers[1]);
    }
}

fn main() {
    run::run(&[
        ("abi_bench_pages", abi_bench_pages),
        ("abi_bench_instances", abi_bench_instances),
        ("abi_bench_gpu", abi_bench_gpu),
        ("abi_bench_paint_calls", abi_bench_paint_calls),
        ("abi_bench_idle", abi_bench_idle),
        ("abi_bench_start", abi_bench_start),
    ]);
}
