//! UIKit owns the run loop; the framework owns display pacing, idle deadlines and callback lifetime.
use super::Window;
pub use sdl3::ios::Signal;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Schedule {
    NextFrame,
    WaitUntil(Option<f64>),
    Exit,
}
pub type Callback = dyn FnMut(&mut Window, Signal) -> Result<Schedule, String>;

/// Runs once on the initial thread. On quit, calls Finish and drops captured app resources before
/// destroying the window. The native loop exits the process after cleanup; startup rejection returns Err.
pub fn run(title: &str, width: i64, height: i64, callback: Box<Callback>) -> Result<(), String> {
    use sdl3::ios::Step;
    let mut window = None;
    let mut callback = Some(callback);
    let mut failure = None;
    let status = {
        let mut invoke = |signal| {
            if signal == Signal::Initialize {
                match Window::open(title, width, height) {
                    Ok(created) => window = Some(created),
                    Err(error) => {
                        failure = Some(error);
                        return Step::exit(false);
                    }
                }
            }
            if signal == Signal::Finish {
                if let Some(mut app) = callback.take() {
                    if let Some(window) = &mut window {
                        if let Err(error) = app(window, signal) {
                            failure.get_or_insert(error);
                        }
                    }
                    drop(app);
                }
                if let Some(mut window) = window.take() {
                    if let Err(error) = window.destroy() {
                        failure.get_or_insert(error);
                    }
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
    if let Some(mut window) = window {
        if let Err(error) = window.destroy() {
            failure.get_or_insert(error);
        }
    }
    match failure {
        Some(error) => Err(error),
        None if status == 0 => Ok(()),
        None => Err(format!(
            "Could not run the iOS loop on the process main thread ({status})"
        )),
    }
}
