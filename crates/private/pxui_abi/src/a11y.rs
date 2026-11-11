use crate::state::{PxuiUi, checked, core_box, fail, get};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str};

export!(pub unsafe fn pxui_ui_a11y_name(ui: *mut PxuiUi, box_: *const PxuiBox, name: Str) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if box_.is_null() { return unsafe { fail(ui, "null accessibility name box") }; }
    let Some(name) = (unsafe { checked(name) }) else { return unsafe { fail(ui, "invalid accessibility name") }; };
    let Some(box_) = core_box(unsafe { *box_ }) else { return unsafe { fail(ui, "invalid accessibility box") }; };
    unsafe { (&mut *state.engine).a11y_name(&box_, name); }
    0
});

/// The accessibility bridge of a window the app owns (Linux: AT-SPI2 on the accessibility bus). The app feeds it each frame's tree
/// (`FrameOut`/`Output.a11y`) and takes the actions assistive technology asked for; elsewhere the platform's own bridge lives in
/// the host and `pxui_a11y_start` returns null.
pub struct PxuiA11y {
    #[cfg(target_os = "linux")]
    bridge: pxui_atspi::Bridge,
    #[cfg(target_os = "linux")]
    actions: Vec<pxui_types::ffi::A11yAction>,
}

// Connects to the accessibility bus as `name`; null when there is none (the reason is not an error: no assistive technology).
export!(bare pub unsafe fn pxui_a11y_start(name: Str) -> *mut PxuiA11y {
    #[cfg(target_os = "linux")]
    {
        let Some(name) = (unsafe { checked(name) }) else { return core::ptr::null_mut() };
        return match pxui_atspi::Bridge::connect(name) { Ok(bridge) => Box::into_raw(Box::new(PxuiA11y { bridge, actions: Vec::new() })), Err(_) => core::ptr::null_mut() };
    }
    #[cfg(not(target_os = "linux"))]
    { let _ = name; core::ptr::null_mut() }
});

// The descriptor the bridge's events wake: the app adds it to its event loop.
export!(bare pub unsafe fn pxui_a11y_fd(a11y: *mut PxuiA11y) -> i32 {
    #[cfg(target_os = "linux")]
    { return unsafe { a11y.as_ref() }.map_or(-1, |a| a.bridge.fd()); }
    #[cfg(not(target_os = "linux"))]
    { let _ = a11y; -1 }
});

// Publishes `tree` (a frame's `Output.a11y`, borrowed for the call).
export!(bare pub unsafe fn pxui_a11y_update(a11y: *mut PxuiA11y, tree: *const pxui_types::ffi::A11yTree) -> i32 {
    #[cfg(target_os = "linux")]
    {
        let (Some(a), Some(tree)) = (unsafe { a11y.as_mut() }, unsafe { tree.as_ref() }) else { return PXUI_ERR_ARGUMENT };
        return if unsafe { a.bridge.update_ffi(tree) }.is_ok() { 0 } else { crate::PXUI_ERR_STATE };
    }
    #[cfg(not(target_os = "linux"))]
    { let _ = (a11y, tree); PXUI_ERR_ARGUMENT }
});

// Handles the bus's pending traffic. 1: the connection is alive, 0: it is gone (stop the bridge). The actions assistive technology
// asked for are in `out` (borrowed until the next pump), for `Ui::inject_a11y`.
export!(bare pub unsafe fn pxui_a11y_pump(a11y: *mut PxuiA11y, out: *mut pxui_types::ffi::A11yActionSpan) -> i32 {
    #[cfg(target_os = "linux")]
    {
        let (Some(a), false) = (unsafe { a11y.as_mut() }, out.is_null()) else { return PXUI_ERR_ARGUMENT };
        a.actions.clear();
        let alive = a.bridge.pump_ffi(&mut a.actions);
        unsafe { *out = pxui_types::ffi::Slice { ptr: a.actions.as_ptr(), len: a.actions.len() }; }
        return alive as i32;
    }
    #[cfg(not(target_os = "linux"))]
    { let _ = (a11y, out); PXUI_ERR_ARGUMENT }
});

export!(bare pub unsafe fn pxui_a11y_stop(a11y: *mut PxuiA11y) {
    #[cfg(target_os = "linux")]
    if !a11y.is_null() { drop(unsafe { Box::from_raw(a11y) }); }
    #[cfg(not(target_os = "linux"))]
    { let _ = a11y; }
});
