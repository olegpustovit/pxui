//! Main-thread AppKit window visibility and one-shot Metal display-link callbacks.
use native_layer_token_sys::portable::Token;
use std::{ffi::c_void, ptr::NonNull, rc::Rc};

#[derive(Clone)]
pub struct Pacing(Rc<Native>);

struct Native(NonNull<c_void>);

/// Owns the update's drawable loan until the frame callback returns or unwinds.
pub struct Tick {
    pacing: Pacing,
    pub lead: f64,
}

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

    pub fn tick(&self, hz: u32) -> Option<Tick> {
        let mut lead = 0.0;
        if unsafe { pxui_sdl3_window_tick(self.0.0.as_ptr(), hz, &mut lead) } {
            Some(Tick { pacing: self.clone(), lead })
        } else {
            None
        }
    }
}

impl Drop for Tick {
    fn drop(&mut self) {
        unsafe { pxui_sdl3_window_release_drawable(self.pacing.0.0.as_ptr()); }
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
    fn pxui_sdl3_window_tick(pacing: *mut c_void, hz: u32, lead: *mut f64) -> bool;
    fn pxui_sdl3_window_release_drawable(pacing: *mut c_void);
}

#[cfg(feature = "test-util")]
pub fn test_order_front(layer: &Token, cover: bool) {
    unsafe extern "C" { fn pxui_sdl3_test_order_front(layer: *mut c_void, cover: bool); }
    if let Some(layer) = layer.borrow_raw(layer.owner_id(), layer.generation()) {
        unsafe { pxui_sdl3_test_order_front(layer.as_ptr(), cover); }
    }
}
