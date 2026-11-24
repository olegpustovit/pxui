//! Against a running compositor (WSLg, weston, ...): a window, its token, a fatal-validation Vulkan swapchain,
//! three painted frames with one programmatic resize, cursor, IME rectangle and a clipboard round trip.
//! Skips with a note when WAYLAND_DISPLAY is unset.
mod common;

#[cfg(target_os = "linux")]
#[path = "../examples/support/gpu.rs"]
mod gpu;

#[cfg(target_os = "linux")]
fn window_wayland_present() {
    use input::Event;
    use pxui_render::Instance;
    use pxui_wayland::{CursorShape, Protocols, Window};
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        println!("RAYS_TEST_SKIP: pxui_wayland: WAYLAND_DISPLAY is unset, no compositor to test against");
        return;
    }
    let mut window = Window::open("pxui_wayland test", 320, 200).unwrap_or_else(|e| panic!("{e}"));
    let protocols = window.protocols().unwrap();
    let names: [(i32, &str); 10] = [
        (Protocols::SEAT, "wl_seat"), (Protocols::POINTER, "pointer"), (Protocols::KEYBOARD, "keyboard"),
        (Protocols::DATA_DEVICE, "wl_data_device"), (Protocols::VIEWPORTER, "wp_viewporter"),
        (Protocols::FRACTIONAL_SCALE, "wp_fractional_scale_v1"), (Protocols::CURSOR_SHAPE, "wp_cursor_shape_v1"),
        (Protocols::DECORATION, "zxdg_decoration_v1"), (Protocols::SERVER_DECORATED, "server-side decorated"),
        (Protocols::TEXT_INPUT, "zwp_text_input_v3"),
    ];
    let pick = |offered: bool| names.iter().filter(|(bit, _)| protocols.has(*bit) == offered).map(|(_, name)| *name).collect::<Vec<_>>();
    println!("pxui_wayland: offered {:?}; missing {:?}; scale {}", pick(true), pick(false), window.scale());
    assert_eq!(window.size(), (320, 200));
    assert!(window.scale() >= 1.0);

    let mut gpu = gpu::Gpu::open(&mut window, || ogpu_vulkan::backend::create_with_validation((true, true)));
    let mut events = Vec::new();
    let mut presented = 0;
    let mut resized = false;
    for frame in 0..60 {
        assert!(window.pump(&mut events), "window closed during the test");
        resized |= events.iter().any(|e| *e == Event::Resized((400, 260)));
        events.clear();
        if presented == 1 && window.size() != (400, 260) {
            window.resize(400, 260).unwrap();
            assert_eq!(window.size(), (400, 260), "a resize takes effect at once");
        }
        let (w, h) = window.size();
        let rects = [
            Instance::rect([8.0, 8.0, w as f32 / 2.0, h as f32 - 8.0], [220, 40, 40, 255]),
            Instance::rect([w as f32 / 2.0 + 4.0, 8.0, w as f32 - 8.0, 40.0 + frame as f32], [40, 80, 220, 255]).radii([6.0; 4]),
        ];
        presented += usize::from(gpu.frame(&mut window, &rects));
        if presented == 3 {
            break;
        }
        window.wait(Some(window.time() + 0.05));
    }
    assert_eq!(presented, 3, "three frames presented");
    assert!(resized, "the next pump announced the programmatic resize");

    window.set_cursor(CursorShape::Text).unwrap();
    window.set_cursor(CursorShape::HorizontalResize).unwrap();
    // without cursor-shape-v1 (WSLg) the cursor comes from the XCursor theme when one is installed
    let themes = ["/usr/share/icons/default/index.theme", "/usr/share/icons/Adwaita/cursors", "/usr/share/icons/DMZ-White/cursors"];
    if !window.protocols().unwrap().has(Protocols::CURSOR_SHAPE) && themes.iter().any(|t| std::path::Path::new(t).exists()) {
        assert!(window.protocols().unwrap().has(Protocols::CURSOR_THEME), "an installed cursor theme was not loaded");
    }
    window.set_text_input(Some(((10, 20, 120, 24), 30))).unwrap();
    window.set_surrounding_text(Some(("héllo wörld", 3, 1))).unwrap();
    assert!(window.set_surrounding_text(Some(("a\0b", 1, 1))).is_err(), "a NUL cannot be sent");
    window.set_surrounding_text(None).unwrap();
    window.set_text_input(None).unwrap();

    let text = "pxui_wayland clipboard \u{2713}";
    if protocols.has(Protocols::DATA_DEVICE) {
        window.clipboard_set_text(text).unwrap();
        // let the compositor take the selection and echo an offer before reading it back
        for _ in 0..3 {
            window.wait(Some(window.time() + 0.05));
            window.pump(&mut events);
        }
        events.clear();
        let source = window.request_paste().unwrap();
        let deadline = window.time() + 3.0;
        let pasted = loop {
            window.pump(&mut events);
            if let Some(Event::Paste(pasted)) = events.iter().find(|e| matches!(e, Event::Paste(_))) {
                break pasted.clone();
            }
            assert!(window.time() < deadline, "no Paste event within 3 s ({source:?})");
            window.wait(Some(window.time() + 0.05));
        };
        println!("pxui_wayland: paste via {source:?}");
        assert_eq!(pasted, text);
        assert_eq!(window.clipboard_get_text().unwrap(), text, "the synchronous read through this window");
        // a second window reads and sets the clipboard through its own connection, not the newest one's
        let mut other = Window::open("pxui_wayland second", 160, 100).unwrap();
        other.clipboard_set_text("second window").unwrap();
        for _ in 0..3 {
            other.wait(Some(other.time() + 0.05));
            other.pump(&mut events);
            window.pump(&mut events);
        }
        assert_eq!(other.clipboard_get_text().unwrap(), "second window");
        other.destroy().unwrap();
        assert!(other.clipboard_get_text().is_err(), "a destroyed window has no clipboard");
        events.clear();
    } else {
        assert!(window.clipboard_set_text(text).is_err());
    }

    // Relative mode: motion accumulates only while it is on (no pointer moves here, so none arrives).
    window.set_relative_mouse(true).unwrap();
    window.pump(&mut events);
    window.set_relative_mouse(false).unwrap();
    assert_eq!(window.take_motion(), (0.0, 0.0));
    // WSLg runs no desktop portal; elsewhere a real dialog would open, which this test cannot answer
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        assert_eq!(window.show_dialog(&[], None, pxui_wayland::DialogKind::OpenFile), Err(pxui_wayland::NO_FILE_DIALOG.to_owned()));
    }

    gpu.close();
    window.destroy().unwrap();
    assert!(window.clipboard_set_text("gone").is_err() && window.clipboard_get_text().is_err(), "a destroyed window has no clipboard");
    assert!(window.set_cursor(CursorShape::Default).is_err(), "a destroyed window refuses calls");
    let mut events = Vec::new();
    assert!(!window.pump(&mut events));
}

/// The compositor side of `window_wayland_damage_pacing`, run in a child process with WAYLAND_DEBUG=client so the
/// parent reads every request it sends. Markers on stderr split the protocol log into phases.
#[cfg(target_os = "linux")]
fn damage_pacing_child() {
    use ogpu_core::backend as be;
    use ogpu_core::render_pass::Rect;
    use pxui_render::Instance;
    use pxui_wayland::Window;
    let mut window = Window::open("pxui_wayland pacing", 320, 200).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(window.scale(), 1.0, "weston --backend=headless runs at scale 1");
    let mut gpu = gpu::Gpu::open(&mut window, || ogpu_vulkan::backend::create_with_validation((true, true)));
    let (w, h) = (320usize, 200usize);
    gpu::ok(be::capture_presents(gpu.surface(), (w * h * 4) as u64));
    let mut events = Vec::new();
    let deadline = window.time() + 3.0;
    // Frame-paced: block (on events, which include frame callbacks) until the compositor invites a frame.
    let mut wait_ready = |window: &mut Window| {
        while !window.frame_ready() {
            assert!(window.time() < deadline, "no frame callback within 3 s");
            window.wait(Some(window.time() + 0.1));
            assert!(window.pump(&mut events), "window closed");
            events.clear();
        }
    };
    let scene = |x: f32| [
        Instance::rect([8.0, 8.0, 150.0, 150.0], [220, 40, 40, 255]).radii([8.0; 4]),
        Instance::rect([160.0, 8.0, 312.0, 150.0], [40, 80, 220, 255]).radii([8.0; 4]).border(2.0, [240, 240, 240, 255]),
        Instance::rect([x, 160.0, x + 24.0, 184.0], [240, 200, 60, 255]).radii([4.0; 4]),
    ];
    let place = |x: f32| Rect { x: x as i64 - 1, y: 159, width: 27, height: 27 };
    let check = |gpu: &gpu::Gpu, frame: usize| {
        let mut shown = vec![0u8; w * h * 4];
        gpu::ok(be::read_capture(gpu.surface(), &mut shown));
        let canvas = gpu.canvas();
        for (i, (s, c)) in shown.chunks_exact(4).zip(canvas.chunks_exact(4)).enumerate() {
            let bgra = [c[2], c[1], c[0], c[3]];
            assert!(s.iter().zip(bgra).all(|(a, b)| a.abs_diff(b) <= 2), "frame {frame}: presented pixel {i} {s:?} is not the canvas {bgra:?}");
        }
        canvas
    };
    wait_ready(&mut window);
    eprintln!("R8MARK full");
    assert!(gpu.frame_damage(&mut window, &scene(10.0), None));
    check(&gpu, 0);
    let mut x = 10.0;
    for frame in 1..=6 {
        wait_ready(&mut window);
        let damage = ogpu_core::damage::union(place(x), place(x + 12.0));
        x += 12.0;
        eprintln!("R8MARK damage {} {} {} {}", damage.x, damage.y, damage.width, damage.height);
        assert!(gpu.frame_damage(&mut window, &scene(x), Some(damage)));
        check(&gpu, frame);
    }
    // partial_equals_full: the damage-only canvas equals a whole redraw of the same scene.
    let partial = gpu.canvas();
    wait_ready(&mut window);
    eprintln!("R8MARK full");
    assert!(gpu.frame_damage(&mut window, &scene(x), None));
    let full = check(&gpu, 7);
    let worst = partial.iter().zip(&full).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
    assert!(worst <= 2, "the damage-only canvas differs from a whole redraw by {worst}");
    let stats = gpu::ok(be::present_stats(gpu.surface()));
    assert!(stats.partial >= 4 && stats.regions >= 6, "damaged frames copied and announced only their damage: {stats:?}");
    // Idle: nothing changed, so nothing is drawn or committed.
    wait_ready(&mut window);
    eprintln!("R8MARK idle");
    let until = window.time() + 0.3;
    while window.time() < until {
        window.wait(Some(until));
        window.pump(&mut events);
        events.clear();
    }
    // Hidden: a minimised surface gets no frame callbacks, so a frame-paced animation stops after one frame.
    eprintln!("R8MARK hidden");
    window.minimize().unwrap();
    let (until, mut hidden_draws) = (window.time() + 0.3, 0);
    while window.time() < until {
        if window.frame_ready() {
            x = if x > 280.0 { 10.0 } else { x + 12.0 };
            hidden_draws += usize::from(gpu.frame_damage(&mut window, &scene(x), Some(place(x))));
        }
        window.wait(Some(until));
        window.pump(&mut events);
        events.clear();
    }
    eprintln!("R8MARK end");
    let frames = window.frame_stats();
    println!("pxui_wayland pacing: hidden draws {hidden_draws}, {frames:?}, {stats:?}");
    assert!(frames.requests >= 8 && frames.callbacks >= 7, "frames were paced by callbacks: {frames:?}");
    gpu::ok(be::capture_presents(gpu.surface(), 0));
    gpu.close();
    window.destroy().unwrap();
}

/// Against its own `weston --backend=headless`: damage-only presents send one wl_surface.damage_buffer per damage
/// rect, the presented images equal the canvas, frame callbacks pace the commits, an idle window commits nothing and
/// a minimised one stops. Skips without weston.
#[cfg(target_os = "linux")]
fn window_wayland_damage_pacing() {
    use std::process::{Command, Stdio};
    let Some(weston) = std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join("weston")).find(|w| w.is_file())) else {
        println!("RAYS_TEST_SKIP: pxui_wayland: weston is not installed");
        return;
    };
    let dir = std::env::temp_dir().join(format!("pxui-wayland-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let mut compositor = Command::new(weston).args(["--backend=headless", "--socket=pxui-test", "--width=640", "--height=480", "--idle-time=0"])
        .env("XDG_RUNTIME_DIR", &dir).env_remove("WAYLAND_DISPLAY").env_remove("DISPLAY").stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let started = std::time::Instant::now();
    while !dir.join("pxui-test.lock").exists() || !dir.join("pxui-test").exists() {
        assert!(started.elapsed().as_secs_f64() < 2.0, "weston did not open its socket");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let child = Command::new(std::env::current_exe().unwrap()).env("PXUI_WAYLAND_PACING_CHILD", "1").env("WAYLAND_DEBUG", "client")
        .env("XDG_RUNTIME_DIR", &dir).env("WAYLAND_DISPLAY", "pxui-test").output();
    let _ = compositor.kill();
    let _ = compositor.wait();
    let _ = std::fs::remove_dir_all(&dir);
    let child = child.unwrap();
    let log = String::from_utf8_lossy(&child.stderr);
    print!("{}", String::from_utf8_lossy(&child.stdout));
    assert!(child.status.success(), "the pacing child failed: {}", log.lines().filter(|l| !l.starts_with('[')).collect::<Vec<_>>().join("\n"));
    // The toplevel is the first surface made; follow its requests phase by phase.
    let surface = log.lines().find_map(|l| l.split("create_surface(new id ").nth(1)).and_then(|r| r.split(')').next()).expect("a surface").to_owned();
    let (mut phase, mut phases): (String, Vec<(String, usize, Vec<[i64; 4]>)>) = (String::new(), Vec::new());
    for line in log.lines() {
        if let Some(marker) = line.strip_prefix("R8MARK ") {
            phase = marker.to_owned();
            phases.push((phase.clone(), 0, Vec::new()));
        } else if let Some(entry) = phases.last_mut().filter(|_| line.contains(&format!(" -> {surface}."))) {
            if line.ends_with(".commit()") {
                entry.1 += 1;
            } else if let Some(args) = line.split(".damage_buffer(").nth(1) {
                let v: Vec<i64> = args.trim_end_matches(')').split(", ").map(|n| n.parse().unwrap()).collect();
                entry.2.push([v[0], v[1], v[2], v[3]]);
            }
        }
    }
    let _ = phase;
    let damaged: Vec<_> = phases.iter().filter(|p| p.0.starts_with("damage ")).collect();
    assert_eq!(damaged.len(), 6, "six damaged frames: {phases:?}");
    for (marker, commits, rects) in damaged {
        let want: Vec<i64> = marker[7..].split(' ').map(|n| n.parse().unwrap()).collect();
        assert_eq!(*commits, 1, "{marker}: one commit");
        assert_eq!(rects, &vec![[want[0], want[1], want[2], want[3]]], "{marker}: one damage_buffer of exactly the frame's damage");
    }
    for (marker, commits, rects) in phases.iter().filter(|p| p.0 == "full") {
        assert_eq!((*commits, rects.as_slice()), (1, &[[0, 0, i64::from(i32::MAX), i64::from(i32::MAX)]][..]), "{marker}: a whole frame damages everything");
    }
    let count = |name: &str| phases.iter().find(|p| p.0 == name).map(|p| p.1);
    assert_eq!(count("idle"), Some(0), "an idle window commits nothing");
    assert!(count("hidden").is_some_and(|c| c <= 1), "a minimised window stops after the frame already invited: {phases:?}");
}

fn main() {
    #[cfg(target_os = "linux")]
    if std::env::var_os("PXUI_WAYLAND_PACING_CHILD").is_some() {
        return damage_pacing_child();
    }
    #[cfg(target_os = "linux")]
    common::run(&[("window_wayland_present", window_wayland_present), ("window_wayland_damage_pacing", window_wayland_damage_pacing)]);
    #[cfg(not(target_os = "linux"))]
    common::run(&[]);
}
