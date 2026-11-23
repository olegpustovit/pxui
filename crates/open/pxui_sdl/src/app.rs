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
    Control,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Schedule {
    NextFrame,
    WaitUntil(Option<f64>),
    Exit,
}
/// Shortest wait for a deadline while the window cannot be seen (SDL reports no end of occlusion, so this bounds it).
#[cfg(not(target_os = "ios"))]
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
            loop {
                match callback(&mut window, Signal::Frame)? {
                    Schedule::Exit => return Ok(()),
                    Schedule::NextFrame => {}
                    Schedule::WaitUntil(at) => {
                        if at.is_some_and(|at| !at.is_finite()) {
                            return Err("Repaint deadline must be finite".into());
                        }
                        // Nobody sees a hidden, minimized or covered window: its deadlines (animation, progress) wait at
                        // least HIDDEN_WAIT, while input and worker wakes still arrive at once.
                        let at = if window.visible()? { at } else { at.map(|at| at.max(window.time() + HIDDEN_WAIT)) };
                        window.wait(at);
                    }
                }
            }
        })();
        let finish = callback(&mut window, Signal::Finish);
        drop(callback);
        let destroyed = window.destroy();
        result.and(finish.map(|_| ())).and(destroyed)
    }
}
