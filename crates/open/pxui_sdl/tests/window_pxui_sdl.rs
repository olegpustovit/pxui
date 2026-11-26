mod common;
use pxui_sdl::*;

fn window_completed_drop_spans_pumps_and_cancel_reuses_buffers() {
    use input::Event;
    let mut window = Window::open_with("drop", 100, 80, true, true, true).unwrap();
    let mut events = Vec::with_capacity(32);
    window.pump(&mut events);
    events.clear();
    assert!(sdl3_sys::test_util::push_drop(0));
    assert!(window.pump(&mut events));
    let start = events.iter().find_map(|event| if let Event::FileDragMoved(p)=event {Some(*p)} else {None}).unwrap();
    assert!(!events.iter().any(|event|matches!(event,Event::FilesDropped {..}|Event::FileDragEnded)));
    events.clear();
    assert!(sdl3_sys::test_util::push_drop(1));
    assert!(window.pump(&mut events));
    assert!(events.contains(&Event::FocusLost));
    let drops: Vec<_> = events.iter().filter(|event|matches!(event,Event::FilesDropped {..})).collect();
    assert_eq!(drops,[&Event::FilesDropped {position:(start.0*2.0,start.1*2.0),paths:Ok(vec!["/Music/first.flac".into(),"/Music/second.m4a".into()])}]);
    events.clear();
    window.pump(&mut events);
    assert!(!events.iter().any(|event|matches!(event,Event::FilesDropped {..})),"delivery is one-shot");
    for round in 0..8 {
        events.clear();
        assert!(sdl3_sys::test_util::push_drop(2));
        mem_sys::local_reset();
        window.pump(&mut events);
        if round>1 {assert_eq!(mem_sys::local_alloc_count(),0,"warm native cancel/pump allocates nothing in Rust");}
        assert!(events.iter().any(|event|matches!(event,Event::FileDragMoved(_))));
        assert!(events.contains(&Event::FileDragEnded));
        assert!(!events.iter().any(|event|matches!(event,Event::FilesDropped {..})));
    }
    window.destroy().unwrap();
}

#[cfg(target_os = "macos")]
fn wait_for_exposure(window: &mut Window, token: &Token) {
    // AppKit sends occlusion changes asynchronously after WindowServer orders the window.
    sdl3_sys::pacing::test_order_front(token, false);
    let mut events = Vec::new();
    let deadline = window.time() + 0.45;
    while !window.visible().unwrap() && window.time() < deadline {
        window.wait(Some(deadline));
        assert!(window.pump(&mut events));
        events.clear();
    }
    assert!(window.visible().unwrap(), "AppKit did not expose the window");
}

// Window visibility requires the initial thread and reports a missing window explicitly.
fn window_visibility() {
    let mut window = match Window::open_with("pxui_sdl", 32, 24, true, true, true) {
        Ok(window) => window,
        Err(e) => {
            println!("RAYS_TEST_SKIP: pxui_sdl visibility: skipped ({e})");
            assert!(std::env::var("RAYS_ALLOW_SKIP").as_deref() == Ok("1"), "missing required GPU/display/audio support; RAYS_ALLOW_SKIP=1 explicitly permits skipping");
            return;
        }
    };
    for area in [((1,2,0,12),7),((1,2,20,-1),7),((1,2,20,12),-1)] {
        assert!(window.set_text_input(Some(area)).is_err());
    }
    window.set_text_input(Some(((1, 2, 20, 12), 7))).unwrap();
    let options=pxui_sdl::TextInputOptions {autocorrect:0,capitalization:0,action:2};
    for bad in [
        pxui_sdl::TextInputOptions {autocorrect:2,..options},
        pxui_sdl::TextInputOptions {capitalization:4,..options},
        pxui_sdl::TextInputOptions {action:3,..options},
    ] {assert!(window.set_text_input_with_options(Some(((1,2,20,12),7)),bad).is_err());}
    window.set_text_input_with_options(Some(((1,2,20,12),7)),options).unwrap();
    mem_sys::local_reset();
    for _ in 0..100 {window.set_text_input_with_options(Some(((1,2,20,12),7)),options).unwrap();}
    assert_eq!(mem_sys::local_alloc_count(),0,"unchanged native keyboard options must reuse platform state");
    mem_sys::local_reset();
    for _ in 0..100 { window.set_text_input(Some(((1, 2, 20, 12), 7))).unwrap(); }
    assert_eq!(mem_sys::local_alloc_count(), 0, "unchanged IME output must reuse its area");
    window.set_surrounding_text(Some(("text", 2, 2))).unwrap();
    window.set_text_input(None).unwrap();
    window.set_text_input(None).unwrap();
    assert!(!window.visible().unwrap(), "native window was not initially hidden");
    window.show().unwrap();
    #[cfg(target_os = "macos")]
    {
        let token = window.token().unwrap();
        wait_for_exposure(&mut window, &token);
    }
    assert!(window.visible().unwrap(), "show did not expose the window");
    // a window hands out a layer token, a size and a scale, and tracks resize
    window.token().unwrap();
    assert!(window.size().0 > 0 && window.scale() >= 1.);
    window.resize(40, 30).unwrap();
    assert_eq!(window.size().0, 40);
    window.set_cursor(CursorShape::Text).unwrap();
    window.set_cursor(CursorShape::Text).unwrap();
    // an idle pump reports an open window and no close event
    let mut events = Vec::new();
    assert!(window.pump(&mut events));
    assert!(!events.iter().any(|e| matches!(e, input::Event::Closed)));
    assert!(window.time() >= 0.);
    // wait sleeps to its deadline when nothing arrives (drain what the show and resize queued first)
    for _ in 0..20 {
        window.pump(&mut events);
    }
    // the resize's extent re-reads succeeded: nothing to report
    assert!(window.take_failure().is_none(), "an extent re-read failed");
    let start = window.time();
    window.wait(Some(start + 0.05));
    let slept = window.time() - start;
    assert!(slept < 1.0, "wait overslept its deadline: {slept} s");
    window.wait(Some(start - 1.0)); // a passed deadline returns at once
    let woke = std::thread::spawn(wake).join().unwrap();
    assert!(woke, "worker could not wake SDL");
    let start = window.time();
    window.wait(Some(start + 1.0));
    assert!(window.time() - start < 0.5, "worker wake was lost");
    events.clear();
    assert!(window.pump(&mut events));
    assert!(events.is_empty(), "a worker wake must not become UI input");
    window.destroy().unwrap();
    assert!(window.show().is_err());
    println!("pxui_sdl visibility: hidden/show flags passed");
}

fn window_numeric_input_zero_alloc() {
    let mut window = Window::open_with("input allocations", 32, 24, false, true, true).unwrap();
    assert!(sdl3_sys::test_util::touch_mouse_emulation_disabled());
    let mut events = Vec::with_capacity(256);
    for _ in 0..8 {
        events.clear();
        assert!(sdl3_sys::test_util::push_numeric_trace());
        assert!(window.pump(&mut events));
    }
    mem_sys::local_reset();
    for _ in 0..100 {
        events.clear();
        assert!(sdl3_sys::test_util::push_numeric_trace());
        assert!(window.pump(&mut events));
        assert!(events.iter().any(|event| matches!(event, input::Event::PointerMoved(_))));
        assert!(events.iter().any(|event| matches!(event, input::Event::PointerPressed(..))));
        assert!(events.iter().any(|event| matches!(event, input::Event::PointerReleased(..))));
        assert!(events.iter().any(|event| matches!(event, input::Event::Scrolled(_))));
        assert!(events.iter().any(|event| matches!(event, input::Event::KeyPressed(_))));
        assert!(events.iter().any(|event| matches!(event, input::Event::Resized(_))));
        assert!(events.iter().any(|event| matches!(event, input::Event::Pinched(_))));
    }
    let allocations = mem_sys::local_alloc_count();
    assert_eq!(allocations, 0, "real SDL input pump allocated after warming");
    window.destroy().unwrap();
}

fn window_dialog_capabilities_are_taken_once_and_expire_on_the_next_pump() {
    let mut window = Window::open_with("dialog capabilities", 32, 24, false, true, true).unwrap();
    let mut events = Vec::with_capacity(32);
    assert!(window.pump(&mut events));
    let id = sdl3_sys::test_util::dialog_reserve_for(window.id());
    assert!(id > 0);
    assert!(sdl3_sys::test_util::dialog_finish(id, Some(&["/tmp/名前.flac", "/tmp/Björk.mp3"])));
    let expired = sdl3_sys::test_util::dialog_reserve_for(window.id());
    assert!(expired > 0);
    assert!(sdl3_sys::test_util::dialog_finish(expired, Some(&["/tmp/unused"])));
    events.clear();
    assert!(window.pump(&mut events));
    assert!(events.iter().any(|event| matches!(event, input::Event::FileDialog {id: chosen, result: Ok(paths)} if *chosen == i64::from(id) && paths.len() == 2)));
    assert!(events.iter().any(|event| matches!(event, input::Event::FileDialog {id: chosen, result: Ok(paths)} if *chosen == i64::from(expired) && paths.len() == 1)));
    assert!(window.take_dialog_selection(-1).is_err());
    let selected = window.take_dialog_selection(i64::from(id)).unwrap();
    assert_eq!(selected[0].path(), std::path::Path::new("/tmp/名前.flac"));
    assert!(selected[0].bookmark().unwrap().is_none());
    assert!(window.take_dialog_selection(i64::from(id)).is_err());
    events.clear();
    assert!(window.pump(&mut events));
    assert!(window.take_dialog_selection(i64::from(expired)).is_err());
    window.destroy().unwrap();
    assert!(window.take_dialog_selection(i64::from(id)).is_err());
}

// Two windows share SDL's one process queue: each pump returns only events naming its own window,
// whichever window drains the queue first; focus, close and quit are per window.
fn two_windows_take_only_their_own_events() {
    use input::{Event, Key};
    use sdl3_sys::test_util::push_window_event as push;
    let (key, motion, focus, unfocus, close, text, quit) = (0, 1, 2, 3, 4, 5, 6);
    let mut a = Window::open_with("router a", 40, 30, false, true, true).unwrap();
    let mut b = Window::open_with("router b", 40, 30, false, true, true).unwrap();
    assert_ne!(a.id(), b.id());
    let (mut ea, mut eb) = (Vec::with_capacity(64), Vec::with_capacity(64));
    a.pump(&mut ea);
    b.pump(&mut eb);
    let pumped = |window: &mut Window, events: &mut Vec<Event>| { events.clear(); window.pump(events) };
    let key_a = |events: &[Event]| events.iter().any(|e| *e == Event::KeyPressed(Key::KeyChar('a')));
    let moved = |events: &[Event]| events.iter().any(|e| matches!(e, Event::PointerMoved(_)));
    let typed = |events: &[Event]| events.iter().any(|e| matches!(e, Event::TextInput(_)));

    // b pumps first and drains the queue: a's events wait for a
    assert!(push(a.id(), key) && push(b.id(), motion) && push(a.id(), text) && push(b.id(), close));
    assert!(!pumped(&mut b, &mut eb), "b's close request closes b");
    assert!(moved(&eb) && eb.contains(&Event::Closed) && !key_a(&eb) && !typed(&eb), "b got {eb:?}");
    assert!(pumped(&mut a, &mut ea), "b's close request must not close a");
    assert!(key_a(&ea) && typed(&ea) && !moved(&ea) && !ea.contains(&Event::Closed), "a got {ea:?}");

    // focus is per window, and an event naming no window goes to the focused one
    assert!(push(b.id(), focus) && push(0, key));
    assert!(pumped(&mut a, &mut ea));
    assert!(ea.is_empty(), "a got {ea:?}");
    assert!(pumped(&mut b, &mut eb));
    assert_eq!(eb, [Event::FocusGained, Event::KeyPressed(Key::KeyChar('a'))]);
    assert!(push(b.id(), unfocus) && push(a.id(), focus) && push(0, motion));
    assert!(pumped(&mut b, &mut eb) && eb == [Event::FocusLost], "b got {eb:?}");
    assert!(pumped(&mut a, &mut ea) && ea.contains(&Event::FocusGained) && moved(&ea), "a got {ea:?}");

    // a dialog's outcome returns to the window that opened it, even while another has focus
    assert!(push(a.id(), unfocus) && push(b.id(), focus));
    assert!(pumped(&mut a, &mut ea) && pumped(&mut b, &mut eb));
    let dialog = sdl3_sys::test_util::dialog_reserve_for(a.id());
    assert!(dialog > 0 && sdl3_sys::test_util::dialog_finish(dialog, Some(&["/tmp/opened-by-a"])));
    let chose = |events: &[Event]| events.iter().any(|e| matches!(e, Event::FileDialog { id, .. } if *id == i64::from(dialog)));
    assert!(pumped(&mut b, &mut eb) && !chose(&eb), "b got a's dialog: {eb:?}");
    assert!(pumped(&mut a, &mut ea) && chose(&ea), "a got {ea:?}");
    assert_eq!(a.take_dialog_selection(i64::from(dialog)).unwrap().len(), 1);

    // with two windows and none focused a window-less event reaches neither, and both report it;
    // past the queue cap events are counted, a close request is still delivered, and the two losses
    // are reported together rather than one overwriting the other
    assert!(push(b.id(), unfocus) && push(0, key));
    for _ in 0..4100 { assert!(push(a.id(), key)); }
    assert!(push(a.id(), close));
    assert!(pumped(&mut b, &mut eb) && eb == [Event::FocusLost], "b got {eb:?}");
    assert!(!pumped(&mut a, &mut ea), "a's close request survives the full queue");
    assert_eq!((ea.len(), ea.last()), (4097, Some(&Event::Closed)));
    let report = a.take_failure().unwrap();
    assert!(report.contains("4 events dropped") && report.contains("1 window-less events"), "{report}");
    assert!(b.take_failure().unwrap().contains("1 window-less events"));
    assert!(a.take_failure().is_none(), "the report is taken once");

    // quit closes every window
    assert!(push(0, quit));
    assert!(!pumped(&mut a, &mut ea) && !pumped(&mut b, &mut eb));

    // events for a closed window are dropped, not handed to the survivor
    let closed = b.id();
    b.destroy().unwrap();
    assert!(push(closed, key) && push(closed, motion));
    assert!(pumped(&mut a, &mut ea) && ea.is_empty(), "a got {ea:?}");
    // warm routing allocates nothing
    for round in 0..8 {
        assert!(push(a.id(), key) && push(a.id(), motion));
        mem_sys::local_reset();
        assert!(pumped(&mut a, &mut ea) && key_a(&ea) && moved(&ea));
        if round > 1 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm routed pump allocated"); }
    }
    a.destroy().unwrap();
}

#[cfg(target_os = "macos")]
fn original_url_bookmarks_refuse_the_initial_thread() {
    let (selected, _) = sdl3_sys::test_util::file_selection(&std::env::temp_dir()).unwrap();
    assert!(selected.bookmark().unwrap_err().to_string().contains("worker thread"));
    let (_, calls, starts, stops) = sdl3_sys::test_util::file_selection_status();
    assert_eq!((calls, starts, stops), (0, 0, 0));
}

#[inline(never)]
fn overflow(n: usize) -> usize {
    let recurse = std::hint::black_box(overflow as fn(usize) -> usize);
    n.wrapping_add(recurse(n.wrapping_add(1)))
}

fn fatal_child(mode: &str) -> ! {
    configure_native_process().unwrap();
    sdl3::init::init(&[sdl3::init::Subsystem::Events]).unwrap();
    println!("SDL alive before fatal failure");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    match mode {
        "overflow" => {
            // Use the normal Rust worker stack, not the application's enlarged main stack.
            std::thread::Builder::new().name("fatal-overflow".into())
                .spawn(|| std::hint::black_box(overflow(1))).unwrap().join().unwrap();
            panic!("recursive worker returned");
        }
        "abort" => std::process::abort(),
        _ => panic!("unknown fatal child mode"),
    }
}

fn native_fatal_exit(mode: &str) {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let mut child = Command::new(std::env::current_exe().unwrap())
        .env("RAYS_RUNTIME_FATAL_CHILD", mode)
        .env_remove("RUST_MIN_STACK")
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    // Leave cleanup time inside the runner's existing five-second deadline.
    let deadline = Instant::now() + Duration::from_secs(3);
    let exited = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            Ok(None) => break Err("fatal child did not terminate promptly".to_string()),
            Err(error) => break Err(error.to_string()),
        }
    };
    if exited.is_err() {
        let _ = child.kill();
    }
    // Always reap the child, including timeout/error paths, before any assertion.
    let output = child.wait_with_output().unwrap();
    let status = exited.unwrap_or_else(|error| panic!("{mode}: {error}"));
    assert!(!status.success(), "{mode}: fatal child exited successfully");
    assert!(String::from_utf8_lossy(&output.stdout).contains("SDL alive before fatal failure"),
        "{mode}: SDL startup did not finish: {}", String::from_utf8_lossy(&output.stderr));
    if mode == "overflow" {
        assert!(String::from_utf8_lossy(&output.stderr).contains("has overflowed its stack"),
            "child did not reach a real Rust stack overflow: {}", String::from_utf8_lossy(&output.stderr));
    }
    // The supervising process is still usable, including its own SDL lifecycle.
    sdl3::init::init(&[sdl3::init::Subsystem::Events]).unwrap();
    sdl3::event::poll_coalesced().unwrap();
    sdl3::init::quit_subsystems(&[sdl3::init::Subsystem::Events]).unwrap();
}

fn native_stack_overflow_exits() { native_fatal_exit("overflow"); }
fn native_abort_exits() { native_fatal_exit("abort"); }

fn native_startup_requires_initial_domain() {
    let error = std::thread::spawn(configure_native_process).join().unwrap().unwrap_err();
    assert_eq!(error.kind(), sdl3::ErrorKind::WrongDomain);
    assert_eq!(error.operation(), "SDL3.Init.configure_native_process");
}

#[cfg(target_os = "macos")]
fn app_loop_opens_a_point_sized_drawable_when_requested() {
    use pxui_sdl::app::{Schedule, Signal, run_with_density};
    run_with_density("point-sized drawable", 320, 240, false, Box::new(|window, signal| {
        if signal == Signal::Initialize {
            let extent = window.extent();
            assert_eq!(extent.logical, (320, 240));
            assert_eq!(extent.drawable, extent.logical);
            assert_eq!(window.scale(), 1.0);
        }
        Ok(Schedule::Exit)
    })).unwrap();
}

/// A Control callback that pumps events can drain the wake its window's visibility change posted (LocalWave does,
/// to keep its workers moving): the loop must still draw the now visible window, not sleep until another event.
#[cfg(target_os = "macos")]
fn app_loop_draws_a_window_that_became_visible_during_a_control_callback() {
    use pxui_sdl::app::{Schedule, Signal, run_with_density};
    use std::sync::atomic::{AtomicBool, Ordering};
    static RESCUED: AtomicBool = AtomicBool::new(false);
    let (mut controls, mut drained, mut drawn_after) = (0, false, f64::NAN);
    let mut returned_at = 0.0;
    run_with_density("visibility wake", 64, 48, false, Box::new(move |window, signal| {
        match signal {
            Signal::Initialize => {
                // Only the first callback can see the window hidden; any later Control is the hidden-start path.
                sdl3_sys::pacing::test_order_front(&window.token()?, false);
                Ok(Schedule::NextFrame)
            }
            Signal::Control if !drained => {
                controls += 1;
                // Pump until AppKit reports the window visible, consuming its wake as an app's own pump would.
                let deadline = window.time() + 2.0;
                while window.presentation().is_none() && window.time() < deadline {
                    window.pump(&mut Vec::new());
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                if window.presentation().is_none() {
                    return Err("AppKit did not expose the window".into());
                }
                drained = true;
                returned_at = window.time();
                // A lost wake would sleep here forever; this thread ends the wait after 3 s so the test fails instead.
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    RESCUED.store(true, Ordering::SeqCst);
                    pxui_sdl::wake();
                });
                Ok(Schedule::WaitUntil(None))
            }
            Signal::Control => Ok(Schedule::WaitUntil(None)),
            Signal::Frame => {
                drawn_after = window.time() - returned_at;
                Ok(Schedule::Exit)
            }
            Signal::Paused | Signal::Resumed | Signal::LowMemory => Ok(Schedule::WaitUntil(None)),
            Signal::Finish => {
                if controls == 0 {
                    println!("visibility wake: the window was visible at once; the hidden start did not occur this run");
                    return Ok(Schedule::Exit);
                }
                assert!(!RESCUED.load(Ordering::SeqCst) && drawn_after < 1.0, "drew {drawn_after} s after the window became visible");
                Ok(Schedule::Exit)
            }
        }
    })).unwrap();
}

fn app_loop_cleans_up_callbacks_before_the_window_on_success_and_errors() {
    use pxui_sdl::app::{Schedule, Signal};
    use std::{cell::RefCell, rc::Rc};
    struct Guard {
        token: Option<pxui_sdl::Token>,
        stages: Rc<RefCell<Vec<Signal>>>,
        exposure_deadline: f64,
    }
    impl Guard {
        fn signal(&mut self, window: &mut Window, signal: Signal, failure: u8) -> Result<Schedule, String> {
            if signal == Signal::Control {
                assert_eq!(self.stages.borrow().as_slice(), &[Signal::Initialize]);
                // AppKit reports exposure asynchronously, and the app loop owns its wait.
                assert!(window.pump(&mut Vec::new()));
                if window.time() >= self.exposure_deadline {
                    return Err("AppKit did not expose the window".into());
                }
                return Ok(Schedule::WaitUntil(Some(self.exposure_deadline)));
            }
            self.stages.borrow_mut().push(signal);
            if signal == Signal::Initialize {
                let token = window.token()?;
                #[cfg(target_os = "macos")]
                if failure != 1 && failure != 4 {
                    sdl3_sys::pacing::test_order_front(&token, false);
                    self.exposure_deadline = window.time() + 0.45;
                }
                self.token = Some(token);
            }
            if (failure == 1 && signal == Signal::Initialize) || (failure == 2 && signal == Signal::Frame) {
                return Err("injected callback failure".into());
            }
            Ok(match signal {
                Signal::Initialize if failure == 4 => Schedule::WaitUntil(Some(f64::NAN)),
                Signal::Initialize => Schedule::NextFrame,
                Signal::Frame if failure == 3 => Schedule::WaitUntil(Some(f64::NAN)),
                _ => Schedule::Exit,
            })
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            assert!(self.token.as_ref().unwrap().alive());
            assert_eq!(self.stages.borrow().last(), Some(&Signal::Finish));
        }
    }
    for failure in 0..5 {
        let stages = Rc::new(RefCell::new(Vec::with_capacity(3)));
        let mut guard = Guard { token: None, stages: stages.clone(), exposure_deadline: 0.0 };
        let result = pxui_sdl::app::run("app callback cleanup", 32, 24, Box::new(move |window, signal| guard.signal(window, signal, failure)));
        assert_eq!(result.is_err(), failure != 0);
        let expected = if failure == 1 || failure == 4 { &[Signal::Initialize, Signal::Finish][..] } else { &[Signal::Initialize, Signal::Frame, Signal::Finish][..] };
        assert_eq!(stages.borrow().as_slice(), expected);
    }
}


#[cfg(target_os = "macos")]
fn window_background_preserves_native_fallback_pixels_and_releases_providers() {
    extern "C" fn clock() -> u64 { mem_sys::profile::thread_cpu_time_ns().unwrap_or(u64::MAX) }
    let mut times = [0; 2];
    assert!(sdl3_sys::chrome::check_native_background(clock, &mut times));
    assert!(times.iter().all(|&ns|ns>0));
    println!("native background: samples=100 mean_update_cpu_us before={:.3} after={:.3}",times[0] as f64/100_000.0,times[1] as f64/100_000.0);
}

#[cfg(target_os = "macos")]
fn window_background_api_uses_the_full_metal_view_and_preserves_resize() {
    let mut window = Window::open_with("native background API", 320, 240, false, true, true).unwrap();
    window.set_resizable(true).unwrap();
    let token = window.token().unwrap();
    window.set_background((0.2, 0.3, 0.4)).unwrap();
    assert!(!sdl3_sys::chrome::background_state(&token).unwrap().1);
    let chrome = Chrome::open(token.clone(), (200.0, 100.0)).unwrap();
    for (width, height) in [(320, 240), (480, 320), (200, 100)] {
        window.resize(width, height).unwrap();
        for rgb in [(0.9,0.9,0.9),(0.1,0.1,0.1),(0.2,0.3,0.4)] {
            mem_sys::local_reset();
            window.set_background(rgb).unwrap();
            let (actual, active) = sdl3_sys::chrome::background_state(&token).unwrap();
            assert!(active, "the root opaque Metal view uses the bounded fallback");
            assert!((actual[0]-rgb.0).abs()<1e-6 && (actual[1]-rgb.1).abs()<1e-6 && (actual[2]-rgb.2).abs()<1e-6);
            assert_eq!(mem_sys::local_alloc_count(), 0);
        }
        assert!(chrome.snapshot().unwrap().full_size_content != 0);
    }
    window.destroy().unwrap();
    assert!(sdl3_sys::chrome::background_state(&token).is_none());
}

fn main() {
    if let Ok(mode) = std::env::var("RAYS_RUNTIME_FATAL_CHILD") {
        fatal_child(&mode);
    }
    common::run(&[
        #[cfg(target_os = "macos")]
        ("window_background_api_uses_the_full_metal_view_and_preserves_resize", window_background_api_uses_the_full_metal_view_and_preserves_resize),
        #[cfg(target_os = "macos")]
        ("window_background_preserves_native_fallback_pixels_and_releases_providers", window_background_preserves_native_fallback_pixels_and_releases_providers),
        ("window_completed_drop_spans_pumps_and_cancel_reuses_buffers", window_completed_drop_spans_pumps_and_cancel_reuses_buffers),
        ("window_visibility", window_visibility),
        ("window_numeric_input_zero_alloc", window_numeric_input_zero_alloc),
        ("two_windows_take_only_their_own_events", two_windows_take_only_their_own_events),
        ("window_dialog_capabilities_are_taken_once_and_expire_on_the_next_pump", window_dialog_capabilities_are_taken_once_and_expire_on_the_next_pump),
        #[cfg(target_os = "macos")]
        ("original_url_bookmarks_refuse_the_initial_thread", original_url_bookmarks_refuse_the_initial_thread),
        ("native_stack_overflow_exits", native_stack_overflow_exits),
        ("native_abort_exits", native_abort_exits),
        ("native_startup_requires_initial_domain", native_startup_requires_initial_domain),
        #[cfg(target_os = "macos")]
        ("app_loop_opens_a_point_sized_drawable_when_requested", app_loop_opens_a_point_sized_drawable_when_requested),
        ("app_loop_cleans_up_callbacks_before_the_window_on_success_and_errors", app_loop_cleans_up_callbacks_before_the_window_on_success_and_errors),
        #[cfg(target_os = "macos")]
        ("app_loop_draws_a_window_that_became_visible_during_a_control_callback", app_loop_draws_a_window_that_became_visible_during_a_control_callback),
    ]);
}
