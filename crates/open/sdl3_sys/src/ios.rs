//! Scoped UIKit loop callbacks. Native calls stop before the borrowed callback can leave scope.
use std::ffi::c_void;

#[cfg(feature = "test-util")]
pub fn test_nested_wake() -> bool {
    unsafe extern "C" {
        fn rays_sdl3_ios_test_nested_wake() -> i32;
    }
    unsafe { rays_sdl3_ios_test_nested_wake() != 0 }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Signal {
    Initialize,
    Frame,
    Paused,
    Resumed,
    LowMemory,
    Finish,
    /// An inactive app may service controls and workers here; drawing must stay paused.
    Control,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Step {
    window: u32,
    action: i32,
    delay: f64,
}
impl Step {
    pub fn initialize(window: u32) -> Self {
        Self {
            window,
            action: 0,
            delay: 0.0,
        }
    }
    pub fn next() -> Self {
        Self {
            window: 0,
            action: 0,
            delay: 0.0,
        }
    }
    pub fn wait(delay: f64) -> Self {
        Self {
            window: 0,
            action: 1,
            delay,
        }
    }
    pub fn exit(success: bool) -> Self {
        Self {
            window: 0,
            action: if success { 2 } else { 3 },
            delay: 0.0,
        }
    }
}

unsafe extern "C" {
    fn rays_sdl3_run_ios(
        callback: extern "C" fn(*mut c_void, i32) -> Step,
        user: *mut c_void,
    ) -> i32;
}

/// A callback panic aborts at the C boundary; it cannot unwind through UIKit.
pub fn run(callback: &mut dyn FnMut(Signal) -> Step) -> i32 {
    extern "C" fn invoke(user: *mut c_void, signal: i32) -> Step {
        let signal = match signal {
            0 => Signal::Initialize,
            1 => Signal::Frame,
            2 => Signal::Paused,
            3 => Signal::Resumed,
            4 => Signal::LowMemory,
            5 => Signal::Finish,
            6 => Signal::Control,
            _ => return Step::exit(false),
        };
        // The native loop calls only on main and unregisters all sources before returning.
        let callback = unsafe { &mut *user.cast::<&mut dyn FnMut(Signal) -> Step>() };
        callback(signal)
    }
    let mut callback = callback;
    // UIKit owns the nested run loop; this stack slot stays live until SDL_RunApp returns.
    unsafe {
        rays_sdl3_run_ios(
            invoke,
            (&mut callback as *mut &mut dyn FnMut(Signal) -> Step).cast(),
        )
    }
}

/// Geometry is in SDL window coordinates; timestamps use the native monotonic clock.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Environment {
    pub safe_area: [f32; 4],
    pub keyboard_frame: [f32; 4],
    pub keyboard_from: [f32; 4],
    pub keyboard_started: f64,
    pub keyboard_duration: f64,
    pub timestamp: f64,
    pub keyboard_curve: u32,
    pub keyboard_visible: u32,
    pub keyboard_current: [f32; 4],
    pub keyboard_flags: u32,
}
unsafe extern "C" {
    fn rays_sdl3_ios_environment(window: u32, out: *mut Environment) -> i32;
}
pub fn environment(window: u32) -> Result<Environment, super::Error> {
    let mut out = Environment::default();
    if unsafe { rays_sdl3_ios_environment(window, &mut out) } != 1 {
        return Err(super::sdl_error(
            super::ErrorKind::Sdl,
            "could not read the iOS environment",
        ));
    }
    Ok(out)
}
