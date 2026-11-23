//! UIKit owns the run loop; the framework owns display pacing, idle deadlines and callback lifetime.
use super::Window;
pub use super::app::{Callback, Schedule, Signal, run};

/// Runs once on the initial thread. On quit, calls Finish and drops captured app resources before
/// destroying the window. The native loop exits the process after cleanup; startup rejection returns Err.
/// Control signals service inactive worker wakes/deadlines without a display-link frame. NextFrame
/// defers drawing until foreground; WaitUntil can schedule another control callback while inactive.
pub(super) fn run_with_density(title: &str, width: i64, height: i64, high_density: bool, callback: Box<Callback>) -> Result<(), String> {
    use sdl3::ios::Step;
    let mut window = None;
    let mut callback = Some(callback);
    let mut failure = None;
    let status = {
        let mut invoke = |native| {
            let signal = match native {
                sdl3::ios::Signal::Initialize => Signal::Initialize,
                sdl3::ios::Signal::Frame => Signal::Frame,
                sdl3::ios::Signal::Paused => Signal::Paused,
                sdl3::ios::Signal::Resumed => Signal::Resumed,
                sdl3::ios::Signal::LowMemory => Signal::LowMemory,
                sdl3::ios::Signal::Finish => Signal::Finish,
                sdl3::ios::Signal::Control => Signal::Control,
            };
            if signal == Signal::Initialize {
                match Window::open_with(title, width, height, true, false, high_density) {
                    Ok(created) => window = Some(created),
                    Err(error) => {
                        failure = Some(error);
                        return Step::exit(false);
                    }
                }
            }
            if signal == Signal::Finish {
                if let Some(mut app) = callback.take() {
                    if let Some(window) = &mut window
                        && let Err(error) = app(window, signal)
                    {
                        failure.get_or_insert(error);
                    }
                    drop(app);
                }
                if let Some(mut window) = window.take()
                    && let Err(error) = window.destroy()
                {
                    failure.get_or_insert(error);
                }
                if let Some(error) = &failure {
                    eprintln!("PXUI iOS: {error}");
                }
                return Step::exit(failure.is_none());
            }
            let Some(window) = &mut window else {
                return Step::exit(false);
            };
            let Some(app) = &mut callback else {
                return Step::exit(false);
            };
            let schedule = match app(window, signal) {
                Ok(schedule) => schedule,
                Err(error) => {
                    failure.get_or_insert(error);
                    return Step::exit(false);
                }
            };
            if schedule == Schedule::Exit {
                return Step::exit(true);
            }
            if signal == Signal::Initialize {
                if matches!(schedule, Schedule::WaitUntil(Some(at)) if !at.is_finite()) {
                    failure.get_or_insert("iOS repaint deadline must be finite".into());
                    return Step::exit(false);
                }
                return match window.handle.id() {
                    Ok(id) => Step::initialize(id),
                    Err(error) => {
                        failure.get_or_insert(error.to_string());
                        Step::exit(false)
                    }
                };
            }
            match schedule {
                Schedule::NextFrame => Step::next(),
                Schedule::WaitUntil(None) => Step::wait(f64::INFINITY),
                Schedule::WaitUntil(Some(at)) if at.is_finite() => {
                    Step::wait((at - window.time()).max(0.0))
                }
                Schedule::WaitUntil(_) => {
                    failure.get_or_insert("iOS repaint deadline must be finite".into());
                    Step::exit(false)
                }
                Schedule::Exit => Step::exit(true),
            }
        };
        sdl3::ios::run(&mut invoke)
    };
    drop(callback.take());
    if let Some(mut window) = window
        && let Err(error) = window.destroy()
    {
        failure.get_or_insert(error);
    }
    match failure {
        Some(error) => Err(error),
        None if status == 0 => Ok(()),
        None => Err(format!(
            "Could not run the iOS loop on the process main thread ({status})"
        )),
    }
}

/// Snapshot in logical window points. Native timestamps are mapped by the framework host.
pub fn environment(window: &Window) -> Result<sdl3::ios::Environment, String> {
    window.live()?;
    let mut value = sdl3::ios::environment(window.handle.id().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    for rect in [
        &mut value.safe_area,
        &mut value.keyboard_frame,
        &mut value.keyboard_from,
        &mut value.keyboard_current,
    ] {
        for number in rect {
            *number /= window.unit as f32;
        }
    }
    Ok(value)
}
