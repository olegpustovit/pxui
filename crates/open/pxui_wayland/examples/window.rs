//! The PXUI host loop on Wayland, frame-paced: pump, draw only when something changed and the compositor's frame
//! callback invites a frame, present only the damage, sleep until the next event or due frame.
//! `cargo run -p pxui_wayland --example window` under a Wayland session; a click flips the colours.
//! `PXUI_WAYLAND_ANIMATE=0` moves a box at the display rate (every frame callback), `=N` at N frames per second (an
//! ambient rate); `PXUI_WAYLAND_EXIT_AFTER=S` quits after S seconds and prints the frame counters.
#[cfg(target_os = "linux")]
#[path = "support/gpu.rs"]
#[allow(dead_code)] // the compositor test uses the rest
mod gpu;

#[cfg(target_os = "linux")]
fn main() {
    use ogpu_core::render_pass::Rect;
    use pxui_render::Instance;
    let env = |name: &str| std::env::var(name).ok().and_then(|v| v.parse::<f64>().ok());
    let animate = env("PXUI_WAYLAND_ANIMATE"); // None idle, Some(0) display rate, Some(hz) ambient
    let exit_after = env("PXUI_WAYLAND_EXIT_AFTER");
    let mut window = pxui_wayland::Window::open("pxui_wayland", 480, 320).unwrap_or_else(|e| panic!("{e}"));
    let mut gpu = gpu::Gpu::open(&mut window, ogpu_vulkan::backend::create);
    let mut events = Vec::with_capacity(256);
    let (mut flipped, mut dirty, mut draws) = (false, true, 0u64);
    let (mut next_tick, mut phase, mut boxed): (f64, u32, Option<Rect>) = (0.0, 0, None);
    while window.pump(&mut events) {
        let now = window.time();
        if events.iter().any(|e| matches!(e, input::Event::PointerPressed(..))) { flipped ^= true; dirty = true; }
        dirty |= events.iter().any(|e| matches!(e, input::Event::Resized(..) | input::Event::PixelSizeChanged(..)));
        events.clear();
        let ticking = animate.is_some_and(|_| now >= next_tick);
        if exit_after.is_some_and(|s| now >= s) { break; }
        if (dirty || ticking) && window.frame_ready() {
            let (w, h) = window.size();
            let scale = window.scale();
            let (a, b) = if flipped { ([40, 80, 220, 255], [220, 40, 40, 255]) } else { ([220, 40, 40, 255], [40, 80, 220, 255]) };
            if ticking { phase = phase.wrapping_add(1); }
            // a 24-point box sliding along the bottom; its old and new place are this frame's damage
            let x = 16.0 + (phase % 200) as f64 * ((w as f64 - 56.0) / 200.0);
            let cell = [x as f32, h as f32 - 48.0, x as f32 + 24.0, h as f32 - 24.0];
            let px = |v: f32| (v as f64 * scale) as i64;
            let now_box = Rect { x: px(cell[0]) - 1, y: px(cell[1]) - 1, width: px(cell[2]) - px(cell[0]) + 3, height: px(cell[3]) - px(cell[1]) + 3 };
            let damage = match (dirty, boxed) {
                (false, Some(old)) => Some(ogpu_core::damage::union(old, now_box)),
                _ => None,
            };
            let rects = [
                Instance::rect([16.0, 16.0, w as f32 / 2.0 - 8.0, h as f32 - 64.0], a).radii([8.0; 4]),
                Instance::rect([w as f32 / 2.0 + 8.0, 16.0, w as f32 - 16.0, h as f32 - 64.0], b).radii([8.0; 4]).border(2.0, [240, 240, 240, 255]),
                Instance::rect(cell, [240, 200, 60, 255]).radii([4.0; 4]),
            ];
            if gpu.frame_damage(&mut window, &rects, damage) {
                draws += 1;
                dirty = false;
                boxed = Some(now_box);
                if ticking { next_tick = if animate == Some(0.0) { now } else { next_tick.max(now - 1.0) + 1.0 / animate.unwrap_or(1.0) }; }
            }
        }
        // Wanting a frame but waiting on the callback: its arrival wakes the wait. Otherwise sleep to the next tick.
        let want = dirty || animate.is_some();
        let deadline = match (want && !window.frame_ready(), animate) {
            (true, _) => exit_after,
            (false, Some(_)) => Some(next_tick),
            (false, None) => exit_after,
        };
        window.wait(deadline.map(|d| exit_after.map_or(d, |e| d.min(e))));
    }
    if exit_after.is_some() {
        let stats = window.frame_stats();
        println!("pxui_wayland: draws {draws} {stats:?} present {:?}", ogpu_core::backend::present_stats(gpu.surface()));
    }
    gpu.close();
}

#[cfg(not(target_os = "linux"))]
fn main() {}
