use crate::{PxuiUi, get, text};
use core::{cell::Cell, ffi::c_void};
use pxui_core::ui_text::{TextAreaOpts, ValueFieldOpts};
use pxui_types::Str;
use std::ffi::CString;

const MAX_TEXT_BYTES: usize = 1 << 20;
pub type TextValid = Option<extern "C" fn(*mut c_void, Str) -> u8>;

unsafe fn checked(value: Str) -> Option<&'static str> {
    if value.len > MAX_TEXT_BYTES { None } else { unsafe { text(value) } }
}

unsafe fn return_text(ui: &mut PxuiUi, value: String, out: *mut Str) -> bool {
    let Ok(value) = CString::new(value) else { return false; };
    unsafe { (&mut *ui.returned).push(value); }
    let value = unsafe { (&*ui.returned).last().unwrap() };
    unsafe { *out = Str { ptr: value.as_ptr().cast(), len: value.as_bytes().len() }; }
    true
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_text_area_submit(
    ui: *mut PxuiUi, at: *const f32, width: f32, height: f32, wrap: u8, label: Str, value: Str,
    out_text: *mut Str, out_changed: *mut u8, out_submitted: *mut u8,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if at.is_null() || out_text.is_null() || out_changed.is_null() || out_submitted.is_null() || wrap > 1 {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid text area arguments") };
    }
    let (x, y) = unsafe { (*at, *at.add(1)) };
    if ![x, y, width, height].iter().all(|v| v.is_finite()) || width < 0.0 || height < 0.0 {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid text area arguments") };
    }
    unsafe { *out_text = Str::default(); *out_changed = 0; *out_submitted = 0; }
    let (Some(label), Some(value)) = (unsafe { checked(label) }, unsafe { checked(value) }) else {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid text area strings") };
    };
    let opts = TextAreaOpts { wrap: wrap != 0, ..TextAreaOpts::default() };
    let (text, submitted) = unsafe { (&mut *state.engine).text_area_submit((x, y), width, height, &opts, label, value) };
    unsafe { *out_changed = (text != value) as u8; *out_submitted = submitted as u8; }
    if !unsafe { return_text(state, text, out_text) } { return unsafe { crate::fail(state as *mut PxuiUi, "edited text contains NUL") }; }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_value_field(
    ui: *mut PxuiUi, at: *const f32, width: f32, height: f32, label: Str, value: Str,
    valid: TextValid, user: *mut c_void, out_text: *mut Str, out_changed: *mut u8, out_editing: *mut u8,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if at.is_null() || out_text.is_null() || out_changed.is_null() || out_editing.is_null() {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid value field arguments") };
    }
    let (x, y) = unsafe { (*at, *at.add(1)) };
    if ![x, y, width, height].iter().all(|v| v.is_finite()) || width < 0.0 || height < 0.0 {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid value field arguments") };
    }
    unsafe { *out_text = Str::default(); *out_changed = 0; *out_editing = 0; }
    let (Some(label), Some(value)) = (unsafe { checked(label) }, unsafe { checked(value) }) else {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid value field strings") };
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
    if invalid_result.get() { return unsafe { crate::fail(state as *mut PxuiUi, "invalid value field validator result") }; }
    unsafe { *out_changed = (text != value) as u8; *out_editing = editing as u8; }
    if !unsafe { return_text(state, text, out_text) } { return unsafe { crate::fail(state as *mut PxuiUi, "edited value contains NUL") }; }
    0
}
