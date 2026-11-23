//! Main-thread AppKit window visibility and one-shot Metal display-link callbacks.
use native_layer_token_sys::portable::Token;
use std::{ffi::c_void, panic::{self, AssertUnwindSafe}, ptr::NonNull, rc::Rc};

#[derive(Clone)]
pub struct Pacing(Rc<Native>);

struct Native(NonNull<c_void>);

impl Pacing {
    pub fn open(layer: &Token) -> Option<Self> {
        let layer = layer.borrow_raw(layer.owner_id(), layer.generation())?;
        NonNull::new(unsafe { pxui_sdl3_window_pacing_open(layer.as_ptr()) })
            .map(|raw| Self(Rc::new(Native(raw))))
    }

    pub fn pause(&self) {
        unsafe { pxui_sdl3_window_pause_link(self.0.0.as_ptr()); }
    }

    pub fn refresh_visibility(&self) {
        unsafe { pxui_sdl3_window_refresh_visibility(self.0.0.as_ptr()); }
    }

    pub fn presentation(&self) -> Option<u64> {
        let generation = unsafe { pxui_sdl3_window_presentation(self.0.0.as_ptr()) };
        (generation != 0).then_some(generation)
    }

    pub fn tick(&self, hz: u32, callback: &mut dyn FnMut(f64)) -> bool {
        struct Invocation<'a> {
            call: &'a mut dyn FnMut(f64),
            panic: Option<Box<dyn std::any::Any + Send>>,
        }
        extern "C" fn invoke(user: *mut c_void, lead: f64) {
            // The native loop removes the callback before the invocation can leave scope.
            let invocation = unsafe { &mut *user.cast::<Invocation<'_>>() };
            invocation.panic = panic::catch_unwind(AssertUnwindSafe(|| (invocation.call)(lead))).err();
        }
        let mut invocation = Invocation { call: callback, panic: None };
        let served = unsafe {
            pxui_sdl3_window_tick(self.0.0.as_ptr(), hz, invoke, (&mut invocation as *mut Invocation<'_>).cast())
        };
        if let Some(panic) = invocation.panic {
            panic::resume_unwind(panic);
        }
        served
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        unsafe { pxui_sdl3_window_pacing_close(self.0.as_ptr()); }
    }
}

unsafe extern "C" {
    fn pxui_sdl3_window_pacing_open(layer: *mut c_void) -> *mut c_void;
    fn pxui_sdl3_window_pacing_close(pacing: *mut c_void);
    fn pxui_sdl3_window_pause_link(pacing: *mut c_void);
    fn pxui_sdl3_window_refresh_visibility(pacing: *mut c_void);
    fn pxui_sdl3_window_presentation(pacing: *mut c_void) -> u64;
    fn pxui_sdl3_window_tick(pacing: *mut c_void, hz: u32, callback: extern "C" fn(*mut c_void, f64), user: *mut c_void) -> bool;
}

#[cfg(feature = "test-util")]
pub fn test_order_front(layer: &Token, cover: bool) {
    unsafe extern "C" { fn pxui_sdl3_test_order_front(layer: *mut c_void, cover: bool); }
    if let Some(layer) = layer.borrow_raw(layer.owner_id(), layer.generation()) {
        unsafe { pxui_sdl3_test_order_front(layer.as_ptr(), cover); }
    }
}
