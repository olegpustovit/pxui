//! Shared application callback contract; platform hosts own window, waits and callback cleanup.
use super::Window;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Signal {
    Initialize,
    Frame,
    Paused,
    Resumed,
    LowMemory,
    Finish,
    /// Services workers and deadlines while drawing is suspended.
    Control,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Schedule {
    NextFrame,
    WaitUntil(Option<f64>),
    Exit,
}
/// Throttles hidden deadlines on desktop hosts without native occlusion notifications.
#[cfg(not(any(target_os = "ios", target_os = "macos")))]
const HIDDEN_WAIT: f64 = 0.5;

pub type Callback = dyn FnMut(&mut Window, Signal) -> Result<Schedule, String>;

/// Finish and captured resource destruction precede window destruction, including callback errors.
/// On iOS UIKit owns execution; native shutdown exits the process after cleanup.
pub fn run(title: &str, width: i64, height: i64, callback: Box<Callback>) -> Result<(), String> {
    run_with_density(title, width, height, true, callback)
}

/// Runs an application with the drawable density choice from [`Window::open_with`].
/// Pass false for point-sized captures on Apple displays.
pub fn run_with_density(title: &str, width: i64, height: i64, high_density: bool, callback: Box<Callback>) -> Result<(), String> {
    #[cfg(target_os = "ios")]
    {
        super::ios::run_with_density(title, width, height, high_density, callback)
    }
    #[cfg(not(target_os = "ios"))]
    {
        let mut window = Window::open_with(title, width, height, true, false, high_density)?;
        let mut callback = callback;
        let result = (|| {
            let initial = callback(&mut window, Signal::Initialize)?;
            if matches!(initial, Schedule::WaitUntil(Some(at)) if !at.is_finite()) {
                return Err("Repaint deadline must be finite".into());
            }
            if initial == Schedule::Exit {
                return Ok(());
            }
            #[cfg(target_os = "macos")]
            {
                let timer_pacing = std::env::var_os("PXUI_TIMER_PACING").as_deref() == Some(std::ffi::OsStr::new("1"));
                let pacing = window.pacing.clone().filter(|_| !timer_pacing);
                let mut next = Schedule::NextFrame;
                loop {
                    let at = deadline(next)?;
                    let hz = window.repaint_hz;
                    let ready_for_link = pacing.is_some() && link_due(at, window.time(), hz);
                    let woke_for_event = if matches!(next, Schedule::WaitUntil(_)) && !ready_for_link {
                        if let Some(pacing) = &pacing { pacing.pause(); }
                        window.wait_for_event(at)
                    } else {
                        false
                    };
                    let update = if window.presentation().is_some() {
                        pacing.as_ref().and_then(|pacing| pacing.tick(hz))
                    } else {
                        None
                    };
                    if update.is_some()
                        && !woke_for_event
                        && !frame_due(next, window.time())
                        && !window.wait_for_event(Some(0.0))
                    {
                        continue;
                    }
                    if ready_for_link && update.is_none() {
                        window.wait(at);
                    }
                    let visible = window.presentation().is_some();
                    if let Some(update) = &update {
                        window.mark_frame_time(update.lead);
                    }
                    next = callback(&mut window, if visible { Signal::Frame } else { Signal::Control })?;
                    if next == Schedule::Exit { return Ok(()); }
                    if !visible && matches!(next, Schedule::NextFrame) {
                        next = Schedule::WaitUntil(None);
                    }
                }
            }
            #[cfg(not(target_os = "macos"))]
            loop {
                match callback(&mut window, Signal::Frame)? {
                    Schedule::Exit => return Ok(()),
                    Schedule::NextFrame => {}
                    schedule @ Schedule::WaitUntil(_) => {
                        let at = deadline(schedule)?;
                        let at = if window.visible()? { at } else { at.map(|at| at.max(window.time() + HIDDEN_WAIT)) };
                        window.wait(at);
                    }
                }
            }
        })();
        #[cfg(target_os = "macos")]
        if let Some(pacing) = &window.pacing { pacing.pause(); }
        let finish = callback(&mut window, Signal::Finish);
        drop(callback);
        let destroyed = window.destroy();
        result.and(finish.map(|_| ())).and(destroyed)
    }
}

#[cfg(not(target_os = "ios"))]
fn deadline(schedule: Schedule) -> Result<Option<f64>, String> {
    match schedule {
        Schedule::WaitUntil(Some(at)) if !at.is_finite() => Err("Repaint deadline must be finite".into()),
        Schedule::WaitUntil(at) => Ok(at),
        _ => Ok(None),
    }
}

#[cfg(target_os = "macos")]
fn link_due(at: Option<f64>, now: f64, hz: u32) -> bool {
    // Immediate display work stays armed, while ambient work can arm one update ahead.
    let lead = if hz == 0 { 0.0 } else { 1.0 / f64::from(hz.clamp(20, 30)) + 0.001 };
    at.is_some_and(|at| at - now <= lead)
}

#[cfg(target_os = "macos")]
fn frame_due(schedule: Schedule, now: f64) -> bool {
    schedule == Schedule::NextFrame || matches!(schedule, Schedule::WaitUntil(Some(at)) if now >= at)
}

#[cfg(all(test, target_os = "macos"))]
#[test]
fn the_display_link_arms_for_due_work_and_nearby_ambient_deadlines() {
    assert!(link_due(Some(5.05), 5.0, 20));
    assert!(link_due(Some(4.99), 5.0, 20));
    assert!(!link_due(Some(5.06), 5.0, 20));
    assert!(!link_due(Some(5.05), 5.0, 30));
    assert!(!link_due(Some(5.5), 5.0, 1));
    assert!(!link_due(None, 5.0, 20));
    assert!(link_due(Some(5.0), 5.0, 0));
    assert!(link_due(Some(4.99), 5.0, 0));
    assert!(!link_due(Some(5.01), 5.0, 0));
}

#[cfg(all(test, target_os = "macos"))]
#[test]
fn a_60_hz_callback_stream_respects_20_hz_repaint_deadlines() {
    let interval = 1.0 / 20.0;
    let mut next = Schedule::WaitUntil(Some(interval));
    let mut draws = 0;
    for tick in 1..=600 {
        let now = f64::from(tick) / 60.0;
        if frame_due(next, now) {
            draws += 1;
            next = Schedule::WaitUntil(Some(now + interval));
        }
        assert!(draws <= tick / 3);
    }
    assert!(draws > 0);
    assert!(!frame_due(Schedule::WaitUntil(None), 10.0));
    assert!(!frame_due(Schedule::WaitUntil(Some(10.0005)), 10.0));
    assert!(frame_due(Schedule::WaitUntil(Some(10.0)), 10.0));
}
