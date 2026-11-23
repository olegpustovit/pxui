//! Main-thread Apple preferences, cached by notifications and read without querying the OS per frame.
use std::ffi::c_void;
use std::io;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::rc::Rc;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// 0 unknown, 1 light, 2 dark, matching the frame input and persisted appearance codes.
    pub appearance: u32,
    pub reduced_motion: u32,
    /// 1 when the system's scrollers are overlay (iOS always; macOS by its "Show scroll bars" setting and pointing device).
    pub overlay_scrollers: u32,
}

pub struct Preferences {
    raw: NonNull<c_void>,
    _main: PhantomData<Rc<()>>,
}
impl Preferences {
    /// Create after the window and drop before it, so notifications can safely wake its event loop.
    pub fn open(window: u32, wake: extern "C" fn()) -> io::Result<Self> {
        let raw = unsafe { pxui_sdl3_preferences_create(window, wake) };
        NonNull::new(raw)
            .map(|raw| Self {
                raw,
                _main: PhantomData,
            })
            .ok_or_else(|| {
                io::Error::other("System preferences need a live Apple window on its main thread")
            })
    }
    pub fn snapshot(&self) -> Snapshot {
        let mut value = Snapshot::default();
        unsafe { pxui_sdl3_preferences_read(self.raw.as_ptr(), &mut value) };
        value
    }
}
impl Drop for Preferences {
    fn drop(&mut self) {
        unsafe { pxui_sdl3_preferences_release(self.raw.as_ptr()) };
    }
}
unsafe extern "C" {
    fn pxui_sdl3_preferences_create(window: u32, wake: extern "C" fn()) -> *mut c_void;
    fn pxui_sdl3_preferences_read(environment: *mut c_void, state: *mut Snapshot);
    fn pxui_sdl3_preferences_release(environment: *mut c_void);
}

#[cfg(all(feature = "test-util", target_os = "macos"))]
pub fn verify_notifications() -> bool {
    unsafe extern "C" {
        fn pxui_sdl3_preferences_test_notifications() -> i32;
    }
    unsafe { pxui_sdl3_preferences_test_notifications() != 0 }
}

#[cfg(test)]
mod tests {
    #[test]
    fn system_preferences_reject_worker_thread_construction() {
        extern "C" fn wake() {}
        std::thread::spawn(|| assert!(super::Preferences::open(0, wake).is_err()))
            .join()
            .unwrap();
    }
}
