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
