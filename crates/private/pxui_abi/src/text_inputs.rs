use crate::state::{PxuiUi, checked, fail, finite, get};
use crate::{PXUI_ERR_ARGUMENT, Str};
use core::{cell::Cell, ffi::c_void};
use pxui_core::ui_text::{TextAreaOpts, ValueFieldOpts};

pub type TextValid = Option<extern "C" fn(*mut c_void, Str) -> u8>;

export!(pub unsafe fn pxui_ui_text_area_submit(
    ui: *mut PxuiUi, at: *const f32, width: f32, height: f32, wrap: u8, label: Str, value: Str,
    out_text: *mut Str, out_changed: *mut u8, out_submitted: *mut u8,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if at.is_null() || out_text.is_null() || out_changed.is_null() || out_submitted.is_null() || wrap > 1 {
        return unsafe { fail(ui, "invalid text area arguments") };
    }
    let (x, y) = unsafe { (*at, *at.add(1)) };
    if !finite(&[x, y, width, height]) || width < 0.0 || height < 0.0 {
        return unsafe { fail(ui, "invalid text area arguments") };
    }
    unsafe { *out_text = Str::default(); *out_changed = 0; *out_submitted = 0; }
    let (Some(label), Some(value)) = (unsafe { checked(label) }, unsafe { checked(value) }) else {
        return unsafe { fail(ui, "invalid text area strings") };
    };
    let opts = TextAreaOpts { wrap: wrap != 0, ..TextAreaOpts::default() };
    let (text, submitted) = unsafe { (&mut *state.engine).text_area_submit((x, y), width, height, &opts, label, value) };
    unsafe { *out_changed = (text != value) as u8; *out_submitted = submitted as u8; }
    unsafe { *out_text = state.give(text); }
    0
});

export!(pub unsafe fn pxui_ui_value_field(
    ui: *mut PxuiUi, at: *const f32, width: f32, height: f32, label: Str, value: Str,
    valid: TextValid, user: *mut c_void, out_text: *mut Str, out_changed: *mut u8, out_editing: *mut u8,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if at.is_null() || out_text.is_null() || out_changed.is_null() || out_editing.is_null() {
        return unsafe { fail(ui, "invalid value field arguments") };
    }
    let (x, y) = unsafe { (*at, *at.add(1)) };
    if !finite(&[x, y, width, height]) || width < 0.0 || height < 0.0 {
        return unsafe { fail(ui, "invalid value field arguments") };
    }
    unsafe { *out_text = Str::default(); *out_changed = 0; *out_editing = 0; }
    let (Some(label), Some(value)) = (unsafe { checked(label) }, unsafe { checked(value) }) else {
        return unsafe { fail(ui, "invalid value field strings") };
    };
    let opts = ValueFieldOpts::default();
    let invalid_result = Cell::new(false);
    let accept = |text: &str| {
        let Some(valid) = valid else { return true; };
        let result = valid(user, Str { ptr: text.as_ptr(), len: text.len() });
        if result > 1 { invalid_result.set(true); }
        result == 1
    };
    let (text, editing) = unsafe { (&mut *state.engine).value_field((x, y), width, height, &opts, &accept, label, value) };
    if invalid_result.get() { return unsafe { fail(ui, "invalid value field validator result") }; }
    unsafe { *out_changed = (text != value) as u8; *out_editing = editing as u8; }
    unsafe { *out_text = state.give(text); }
    0
});
