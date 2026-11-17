use std::ffi::{CString, c_char};

/// Drives UIKit's real SDL text field and checks native event ownership/limits.
#[cfg(target_os = "ios")]
pub fn ios_text_step(step: i32) -> bool {unsafe {super::ffi::pxui_sdl3_ios_test_text_step(step)}}

pub fn touch_mouse_emulation_disabled() -> bool {
    unsafe { super::ffi::pxui_sdl3_test_touch_mouse_emulation_disabled() }
}

pub fn push_finger(phase: i32, x: f64, y: f64) -> bool {
    if !(0..=3).contains(&phase) || !x.is_finite() || !y.is_finite() { return false; }
    unsafe { super::ffi::pxui_sdl3_test_push_finger(phase, x, y) }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use file_access_sys::test_util::{selection as file_selection, status as file_selection_status};

#[cfg(target_os = "ios")]
pub fn ios_picker_ready() -> bool { unsafe { super::ffi::pxui_dialog_ios_test_ready() } }

#[cfg(target_os = "ios")]
pub fn ios_picker_idle() -> bool { unsafe { super::ffi::pxui_dialog_ios_test_idle() } }

/// Drives the actual native delegate: 0 cancels, 1 chooses a file URL, 2 returns an invalid URL.
#[cfg(target_os = "ios")]
pub fn ios_picker_finish(mode: i32, path: &str) -> usize {
    let Ok(path) = CString::new(path) else { return 0; };
    unsafe { super::ffi::pxui_dialog_ios_test_finish(mode, path.as_ptr()) }
}

#[cfg(target_os = "ios")]
pub fn ios_picker_close() -> bool { unsafe { super::ffi::pxui_dialog_ios_test_close() } }

pub fn push_event_trace() -> bool {
    // SAFETY: the helper only pushes SDL-owned test events into the active queue.
    unsafe { super::ffi::pxui_sdl3_test_push_event_trace() }
}

/// 0 starts a drop with one file; 1 completes it with another; 2 cancels an empty drag.
pub fn push_drop(phase: i32) -> bool {
    unsafe { super::ffi::pxui_sdl3_test_push_drop(phase) }
}

pub fn mutate_event_sources() {
    // SAFETY: this mutates static test fixture buffers after their events are copied.
    unsafe { super::ffi::pxui_sdl3_test_mutate_event_sources() }
}

pub fn polling_event_queue() -> bool {
    // SAFETY: reports whether the current native event pump is active.
    unsafe { super::ffi::pxui_sdl3_test_polling_event_queue() }
}

pub fn queue_resize_during_poll() {
    // SAFETY: arms one synthetic pixel-size event for the next native event pump.
    unsafe { super::ffi::pxui_sdl3_test_queue_resize_during_poll() }
}

pub fn push_resize_burst() -> bool {
    // SAFETY: this pushes a fixed sequence of synthetic SDL resize events.
    unsafe { super::ffi::pxui_sdl3_test_push_resize_burst() }
}

/// Queues one event naming SDL window `window`: 0 key A down, 1 pointer motion, 2 focus gained,
/// 3 focus lost, 4 close requested, 5 text input, 6 quit (names no window).
pub fn push_window_event(window: u32, kind: i32) -> bool {
    // SAFETY: the helper pushes one SDL-owned test event with a static payload.
    unsafe { super::ffi::pxui_sdl3_test_push_window_event(window, kind) }
}

pub fn push_numeric_trace() -> bool {
    // SAFETY: the helper queues fixed pointer, keyboard, resize, wheel and pinch events.
    unsafe { super::ffi::pxui_sdl3_test_push_numeric_trace() }
}

/// Reserves a test dialog opened by SDL window `window`: its outcome is routed to that window.
pub fn dialog_reserve_for(window: u32) -> i32 {
    // SAFETY: the helper reserves one internal dialog test slot and records its owner.
    unsafe { super::ffi::pxui_sdl3_test_dialog_reserve_for(window) }
}

pub fn dialog_reserve() -> i32 {
    // SAFETY: the helper reserves one internal dialog test slot.
    unsafe { super::ffi::pxui_sdl3_test_dialog_reserve() }
}

pub fn dialog_abandon(id: i32) {
    // SAFETY: this releases the matching test slot if it is still open.
    unsafe { super::ffi::pxui_sdl3_test_dialog_abandon(id) }
}

pub fn dialog_finish(id: i32, paths: Option<&[&str]>) -> bool {
    let strings: Option<Vec<_>> = paths.map(|paths| paths.iter().map(|path| CString::new(*path).ok()).collect());
    let Some(strings) = strings else { return false; };
    if strings.iter().any(Option::is_none) { return false; }
    let strings: Vec<_> = strings.into_iter().map(Option::unwrap).collect();
    let pointers: Vec<*const c_char> = strings.iter().map(|path| path.as_ptr()).collect();
    // SAFETY: the pointers remain valid until the callback thread has finished copying them.
    unsafe {
        super::ffi::pxui_sdl3_test_dialog_finish(id, pointers.as_ptr(), pointers.len() as i32, false)
    }
}

pub fn dialog_fail(id: i32) -> bool {
    // SAFETY: the helper completes the matching slot with a copied failure message.
    unsafe { super::ffi::pxui_sdl3_test_dialog_finish(id, std::ptr::null(), 0, true) }
}

pub fn dialog_finish_huge(id: i32) -> bool {
    // SAFETY: the helper passes a bounded 5000-entry list to exercise native selection limits.
    unsafe { super::ffi::pxui_sdl3_test_dialog_finish_huge(id) }
}

pub fn post_scroll(points: i32, phase: i32) {
    // SAFETY: the helper posts one native trackpad event; it is a no-op off macOS.
    unsafe { super::ffi::pxui_sdl3_test_post_scroll(points, phase) }
}

pub fn push_touch_trace(window:u32)->bool {
    // SAFETY: the helper posts numeric SDL events for a checked live window ID.
    unsafe {super::ffi::pxui_sdl3_test_push_touch_trace(window)}
}
