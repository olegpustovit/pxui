use crate::{PxuiUi, get, text};
use core::{cell::Cell, ffi::c_void};
use pxui_core::{ui::UiBox, ui_text::{MenuOpts, MenuResult, Pick, PickerOpts}};
use pxui_types::{PxuiBox, Str, ffi::Slice};
use std::ffi::CString;

const MAX_ROWS: usize = 4096;
const MAX_TEXT_BYTES: usize = 1 << 20;
const MAX_TOTAL_TEXT_BYTES: usize = 8 << 20;

pub type PickerRows = Option<extern "C" fn(*mut c_void, Str, *mut Slice<Str>)>;

unsafe fn checked_text(value: Str) -> Option<&'static str> {
    if value.len > MAX_TEXT_BYTES { None } else { unsafe { text(value) } }
}

unsafe fn strings(values: Slice<Str>) -> Option<Vec<&'static str>> {
    if values.len > MAX_ROWS || (values.len != 0 && values.ptr.is_null()) { return None; }
    let values = if values.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(values.ptr, values.len) } };
    let mut total = 0usize;
    values.iter().map(|value| {
        total = total.checked_add(value.len)?;
        if value.len > MAX_TEXT_BYTES || total > MAX_TOTAL_TEXT_BYTES { return None; }
        unsafe { text(*value) }
    }).collect()
}

fn core_box(raw: PxuiBox) -> UiBox { UiBox { index: raw.index as usize, box_key: raw.key, box_slot: raw.slot as usize, generation: raw.generation } }

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_tip(ui: *mut PxuiUi, box_: *const PxuiBox, label: Str) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if box_.is_null() { return unsafe { crate::fail(state as *mut PxuiUi, "null tooltip box") }; }
    let Some(label) = (unsafe { checked_text(label) }) else { return unsafe { crate::fail(state as *mut PxuiUi, "invalid tooltip text") }; };
    unsafe { (&mut *state.engine).tip(&core_box(*box_), label); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_toasts(ui: *mut PxuiUi, label: Str, items: Slice<Str>, seconds: f64, out_index: *mut i64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if out_index.is_null() || !seconds.is_finite() || seconds < 0.0 { return unsafe { crate::fail(state as *mut PxuiUi, "invalid toast arguments") }; }
    let (Some(label), Some(items)) = (unsafe { checked_text(label) }, unsafe { strings(items) }) else {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid toast strings") };
    };
    let value = unsafe { (&mut *state.engine).toasts(label, &items, seconds) };
    unsafe { *out_index = value.map_or(-1, |index| index as i64); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_context_menu(
    ui: *mut PxuiUi, x: f32, y: f32, label: Str, items: Slice<Str>, enabled: Slice<u8>,
    has_width: u8, width: f32, has_selected: u8, selected: i64,
    out_tag: *mut u32, out_index: *mut i64,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if out_tag.is_null() || out_index.is_null() || !x.is_finite() || !y.is_finite() || has_width > 1 || has_selected > 1
        || (has_width != 0 && (!width.is_finite() || width < 0.0)) || items.len > MAX_ROWS
        || enabled.len != items.len || (enabled.len != 0 && enabled.ptr.is_null()) {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid context menu arguments") };
    }
    let (Some(label), Some(labels)) = (unsafe { checked_text(label) }, unsafe { strings(items) }) else {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid context menu strings") };
    };
    let enabled = if enabled.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(enabled.ptr, enabled.len) } };
    if enabled.iter().any(|value| *value > 1) { return unsafe { crate::fail(state as *mut PxuiUi, "invalid context menu enabled flags") }; }
    let items: Vec<_> = labels.into_iter().zip(enabled).map(|(label, enabled)| (label.to_owned(), *enabled != 0)).collect();
    let opts = MenuOpts { width: (has_width != 0).then_some(width), selected: (has_selected != 0).then_some(selected), ..MenuOpts::default() };
    let result = unsafe { (&mut *state.engine).context_menu((x, y), &opts, label, &items) };
    let (tag, index) = match result { MenuResult::Open => (0, -1), MenuResult::Pick(index) => (1, index), MenuResult::Dismiss => (2, -1) };
    unsafe { *out_tag = tag; *out_index = index; }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_picker(
    ui: *mut PxuiUi, limit: i64, slash: u8, at_rest: u8, label: Str, query: Str,
    rows: PickerRows, user: *mut c_void, out_query: *mut Str, out_tag: *mut u32, out_index: *mut i64,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if out_query.is_null() || out_tag.is_null() || out_index.is_null() || !(1..=1024).contains(&limit) || slash > 1 || at_rest > 1 {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid picker arguments") };
    }
    let (Some(label), Some(query)) = (unsafe { checked_text(label) }, unsafe { checked_text(query) }) else {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid picker strings") };
    };
    let invalid = Cell::new(false);
    let rows_of = |query: &str| -> Vec<(String, String)> {
        let Some(callback) = rows else { return Vec::new(); };
        let mut span = Slice { ptr: core::ptr::null(), len: 0 };
        callback(user, Str { ptr: query.as_ptr(), len: query.len() }, &mut span);
        if span.len > MAX_ROWS * 2 || span.len % 2 != 0 || (span.len != 0 && span.ptr.is_null()) {
            invalid.set(true);
            return Vec::new();
        }
        let values = if span.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(span.ptr, span.len) } };
        let mut total = 0usize;
        let mut result = Vec::with_capacity(values.len() / 2);
        for pair in values.chunks_exact(2) {
            total = match total.checked_add(pair[0].len).and_then(|n| n.checked_add(pair[1].len)) { Some(total) => total, None => { invalid.set(true); return Vec::new(); } };
            if total > MAX_TOTAL_TEXT_BYTES || pair[0].len > MAX_TEXT_BYTES || pair[1].len > MAX_TEXT_BYTES {
                invalid.set(true);
                return Vec::new();
            }
            let (Some(label), Some(detail)) = (unsafe { text(pair[0]) }, unsafe { text(pair[1]) }) else {
                invalid.set(true);
                return Vec::new();
            };
            result.push((label.to_owned(), detail.to_owned()));
        }
        result
    };
    let opts = PickerOpts { limit, slash: slash != 0, at_rest: at_rest != 0, ..PickerOpts::default() };
    let (query, pick) = unsafe { (&mut *state.engine).picker(&opts, label, query, &rows_of) };
    if invalid.get() { return unsafe { crate::fail(state as *mut PxuiUi, "invalid picker rows") }; }
    let Ok(query) = CString::new(query) else { return unsafe { crate::fail(state as *mut PxuiUi, "picker query contains NUL") }; };
    unsafe { (&mut *state.returned).push(query); }
    let value = unsafe { (&*state.returned).last().unwrap() };
    unsafe { *out_query = Str { ptr: value.as_ptr().cast(), len: value.as_bytes().len() }; }
    let (tag, index) = match pick { Pick::None => (0, -1), Pick::Pick(index) => (1, index), Pick::Delete(index) => (2, index), Pick::Submit => (3, -1), Pick::Back => (4, -1), Pick::Cancel => (5, -1) };
    unsafe { *out_tag = tag; *out_index = index; }
    0
}
