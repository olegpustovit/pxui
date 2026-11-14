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
pub type Callback = dyn FnMut(&mut Window, Signal) -> Result<Schedule, String>;

/// Finish and captured resource destruction precede window destruction, including callback errors.
/// On iOS UIKit owns execution; native shutdown exits the process after cleanup.
pub fn run(title: &str, width: i64, height: i64, callback: Box<Callback>) -> Result<(), String> {
    #[cfg(target_os = "ios")]
    {
        super::ios::run(title, width, height, callback)
    }
    #[cfg(not(target_os = "ios"))]
    {
        let mut window = Window::open(title, width, height)?;
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
