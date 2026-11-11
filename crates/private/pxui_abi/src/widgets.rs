use crate::{get, text, PxuiUi};
use pxui_core::ui::UiBox;
use pxui_types::{PxuiBox, Str, ffi::Slice};
use std::ffi::CString;

const MAX_OPTIONS: usize = 4096;
const MAX_TEXT_BYTES: usize = 1 << 20;
const MAX_TOTAL_TEXT_BYTES: usize = 8 << 20;

unsafe fn checked_text(value: Str) -> Option<&'static str> {
    if value.len > MAX_TEXT_BYTES { return None; }
    unsafe { text(value) }
}

unsafe fn strings(values: Slice<Str>) -> Option<Vec<&'static str>> {
    if values.len > MAX_OPTIONS || (values.len != 0 && values.ptr.is_null()) { return None; }
    let values = if values.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(values.ptr, values.len) } };
    let mut total = 0usize;
    values.iter().map(|value| {
        total = total.checked_add(value.len)?;
        if total > MAX_TOTAL_TEXT_BYTES { return None; }
        unsafe { checked_text(*value) }
    }).collect()
}

fn boxed(value: UiBox) -> PxuiBox { PxuiBox { index: value.index as u64, key: value.box_key, slot: value.box_slot as u64, generation: value.generation } }

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_checkbox(ui: *mut PxuiUi, label: Str, value: u8, out: *mut u8) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return -1; };
    if out.is_null() || value > 1 { return unsafe { crate::fail(ui as *mut PxuiUi, "invalid checkbox arguments") }; }
    let Some(label) = (unsafe { checked_text(label) }) else { return unsafe { crate::fail(ui as *mut PxuiUi, "invalid checkbox label") }; };
    unsafe { *out = (&mut *ui.engine).checkbox(label, value != 0) as u8; }
    0
}

unsafe fn choice(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize, kind: u32) -> i32 {
    let Some(ui_state) = (unsafe { get(ui) }) else { return -1; };
    if out.is_null() { return unsafe { crate::fail(ui_state as *mut PxuiUi, "null widget output") }; }
    let (Some(label), Some(options)) = (unsafe { checked_text(label) }, unsafe { strings(options) }) else { return unsafe { crate::fail(ui_state as *mut PxuiUi, "invalid widget strings") }; };
    let engine = unsafe { &mut *ui_state.engine };
    unsafe { *out = match kind { 0 => engine.radio(label, &options, selected), 1 => engine.tabs(label, &options, selected), 2 => engine.segmented(label, &options, selected), _ => engine.dropdown(label, &options, selected) }; }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_radio(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { choice(ui, label, options, selected, out, 0) } }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_tabs(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { choice(ui, label, options, selected, out, 1) } }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_segmented(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { choice(ui, label, options, selected, out, 2) } }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_dropdown(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { choice(ui, label, options, selected, out, 3) } }

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_slider(ui: *mut PxuiUi, label: Str, value: f32, min: f32, max: f32, out: *mut f32) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return -1; };
    if out.is_null() || !value.is_finite() || !min.is_finite() || !max.is_finite() || min > max { return unsafe { crate::fail(ui as *mut PxuiUi, "invalid slider arguments") }; }
    let Some(label) = (unsafe { checked_text(label) }) else { return unsafe { crate::fail(ui as *mut PxuiUi, "invalid slider label") }; };
    unsafe { *out = (&mut *ui.engine).slider(label, value, min..=max); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_progress(ui: *mut PxuiUi, label: Str, has_value: u8, value: f32, out: *mut PxuiBox) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return -1; };
    if out.is_null() || has_value > 1 || (has_value != 0 && !value.is_finite()) { return unsafe { crate::fail(ui as *mut PxuiUi, "invalid progress arguments") }; }
    let Some(label) = (unsafe { checked_text(label) }) else { return unsafe { crate::fail(ui as *mut PxuiUi, "invalid progress label") }; };
    unsafe { *out = boxed((&mut *ui.engine).progress(label, (has_value != 0).then_some(value))); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_spinner(ui: *mut PxuiUi, label: Str, out: *mut PxuiBox) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return -1; };
    if out.is_null() { return unsafe { crate::fail(ui as *mut PxuiUi, "null spinner output") }; }
    let Some(label) = (unsafe { checked_text(label) }) else { return unsafe { crate::fail(ui as *mut PxuiUi, "invalid spinner label") }; };
    unsafe { *out = boxed((&mut *ui.engine).spinner(label)); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_icon_button(ui: *mut PxuiUi, label: Str, icon: Str, out: *mut u8) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return -1; };
    if out.is_null() { return unsafe { crate::fail(ui as *mut PxuiUi, "null icon button output") }; }
    let (Some(label), Some(icon)) = (unsafe { checked_text(label) }, unsafe { checked_text(icon) }) else { return unsafe { crate::fail(ui as *mut PxuiUi, "invalid icon button text") }; };
    unsafe { *out = (&mut *ui.engine).icon_button(label, icon) as u8; }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_badge(ui: *mut PxuiUi, label: Str, value: Str, out: *mut PxuiBox) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return -1; };
    if out.is_null() { return unsafe { crate::fail(ui as *mut PxuiUi, "null badge output") }; }
    let (Some(label), Some(value)) = (unsafe { checked_text(label) }, unsafe { checked_text(value) }) else { return unsafe { crate::fail(ui as *mut PxuiUi, "invalid badge text") }; };
    unsafe { *out = boxed((&mut *ui.engine).badge(label, value)); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_search_field(ui: *mut PxuiUi, label: Str, icon: Str, value: Str, placeholder: Str, changed: *mut u8, out: *mut Str) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return -1; };
    if changed.is_null() || out.is_null() { return unsafe { crate::fail(ui as *mut PxuiUi, "null search field output") }; }
    unsafe { *changed = 0; *out = Str::default(); }
    let (Some(label), Some(icon), Some(value), Some(placeholder)) = (
        unsafe { checked_text(label) }, unsafe { checked_text(icon) }, unsafe { checked_text(value) }, unsafe { checked_text(placeholder) },
    ) else { return unsafe { crate::fail(ui as *mut PxuiUi, "invalid search field text") }; };
    let Some(value) = (unsafe { &mut *ui.engine }).search_field(label, icon, value, placeholder) else {
        unsafe { *changed = 0; *out = Str::default(); }
        return 0;
    };
    let Ok(value) = CString::new(value) else { return unsafe { crate::fail(ui as *mut PxuiUi, "search value contains NUL") }; };
    unsafe { (&mut *ui.returned).push(value); }
    let bytes = unsafe { (&*ui.returned).last().unwrap().as_bytes() };
    unsafe { *changed = 1; *out = Str { ptr: bytes.as_ptr(), len: bytes.len() }; }
    0
}
