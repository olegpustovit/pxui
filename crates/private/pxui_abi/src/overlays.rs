use crate::state::{MAX_TOTAL_TEXT_BYTES, PxuiUi, checked, core_box, fail, finite, get, strings};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str};
use core::{cell::Cell, ffi::c_void};
use pxui_core::ui_text::{MenuOpts, MenuResult, Pick, PickerOpts};
use pxui_types::ffi::Slice;

const MAX_ROWS: usize = 4096;

pub type PickerRows = Option<extern "C" fn(*mut c_void, Str, *mut Slice<Str>)>;

export!(pub unsafe fn pxui_ui_tip(ui: *mut PxuiUi, box_: *const PxuiBox, label: Str) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if box_.is_null() { return unsafe { fail(ui, "null tooltip box") }; }
    let (Some(label), Some(box_)) = (unsafe { checked(label) }, core_box(unsafe { *box_ })) else { return unsafe { fail(ui, "invalid tooltip arguments") }; };
    unsafe { (&mut *state.engine).tip(&box_, label); }
    0
});

export!(pub unsafe fn pxui_ui_toasts(ui: *mut PxuiUi, label: Str, items: Slice<Str>, seconds: f64, out_index: *mut i64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out_index.is_null() || !seconds.is_finite() || seconds < 0.0 { return unsafe { fail(ui, "invalid toast arguments") }; }
    let mut list = state.shared().take_strs();
    let Some(label) = (unsafe { checked(label) }).filter(|_| unsafe { strings(items, MAX_ROWS, &mut list) }) else {
        state.shared().put_strs(list);
        return unsafe { fail(ui, "invalid toast strings") };
    };
    let value = unsafe { (&mut *state.engine).toasts(label, &list, seconds) };
    state.shared().put_strs(list);
    unsafe { *out_index = value.map_or(-1, |index| index as i64); }
    0
});

export!(pub unsafe fn pxui_ui_context_menu(
    ui: *mut PxuiUi, x: f32, y: f32, label: Str, items: Slice<Str>, enabled: Slice<u8>,
    has_width: u8, width: f32, has_selected: u8, selected: i64,
    out_tag: *mut u32, out_index: *mut i64,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out_tag.is_null() || out_index.is_null() || !finite(&[x, y]) || has_width > 1 || has_selected > 1
        || (has_width != 0 && (!width.is_finite() || width < 0.0)) || items.len > MAX_ROWS
        || enabled.len != items.len || (enabled.len != 0 && enabled.ptr.is_null()) {
        return unsafe { fail(ui, "invalid context menu arguments") };
    }
    let mut list = state.shared().take_strs();
    let Some(label) = (unsafe { checked(label) }).filter(|_| unsafe { strings(items, MAX_ROWS, &mut list) }) else {
        state.shared().put_strs(list);
        return unsafe { fail(ui, "invalid context menu strings") };
    };
    let enabled = if enabled.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(enabled.ptr, enabled.len) } };
    if enabled.iter().any(|value| *value > 1) {
        state.shared().put_strs(list);
        return unsafe { fail(ui, "invalid context menu enabled flags") };
    }
    // the engine takes owned rows; the buffers are reused, so an open menu allocates only while it grows
    let mut menu = core::mem::take(&mut state.shared().menu);
    menu.truncate(list.len());
    for (at, (label, enabled)) in list.iter().zip(enabled).enumerate() {
        match menu.get_mut(at) {
            Some(row) => { row.0.clear(); row.0.push_str(label); row.1 = *enabled != 0; }
            None => menu.push(((*label).to_owned(), *enabled != 0)),
        }
    }
    let opts = MenuOpts { width: (has_width != 0).then_some(width), selected: (has_selected != 0).then_some(selected), ..MenuOpts::default() };
    let result = unsafe { (&mut *state.engine).context_menu((x, y), &opts, label, &menu) };
    state.shared().menu = menu;
    state.shared().put_strs(list);
    let (tag, index) = match result { MenuResult::Open => (0, -1), MenuResult::Pick(index) => (1, index), MenuResult::Dismiss => (2, -1) };
    unsafe { *out_tag = tag; *out_index = index; }
    0
});

export!(pub unsafe fn pxui_ui_picker(
    ui: *mut PxuiUi, limit: i64, slash: u8, at_rest: u8, label: Str, query: Str,
    rows_cb: PickerRows, user: *mut c_void, out_query: *mut Str, out_tag: *mut u32, out_index: *mut i64,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out_query.is_null() || out_tag.is_null() || out_index.is_null() || !(1..=1024).contains(&limit) || slash > 1 || at_rest > 1 {
        return unsafe { fail(ui, "invalid picker arguments") };
    }
    let (Some(label), Some(query)) = (unsafe { checked(label) }, unsafe { checked(query) }) else {
        return unsafe { fail(ui, "invalid picker strings") };
    };
    let invalid = Cell::new(false);
    // The engine keeps the rows between frames; each call replaces them in place, reusing their strings.
    let fill = |query: &str, rows: &mut Vec<(String, String)>| {
        let mut span = Slice { ptr: core::ptr::null(), len: 0 };
        if let Some(callback) = rows_cb { callback(user, Str { ptr: query.as_ptr(), len: query.len() }, &mut span); }
        if span.len > MAX_ROWS * 2 || span.len % 2 != 0 || (span.len != 0 && span.ptr.is_null()) { invalid.set(true); return rows.clear(); }
        let values = if span.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(span.ptr, span.len) } };
        let mut total = 0usize;
        for (at, pair) in values.chunks_exact(2).enumerate() {
            total = total.saturating_add(pair[0].len).saturating_add(pair[1].len);
            let texts = (unsafe { checked(pair[0]) }, unsafe { checked(pair[1]) });
            let (Some(label), Some(detail)) = texts else { invalid.set(true); return rows.clear(); };
            if total > MAX_TOTAL_TEXT_BYTES { invalid.set(true); return rows.clear(); }
            match rows.get_mut(at) {
                Some(row) => { row.0.clear(); row.0.push_str(label); row.1.clear(); row.1.push_str(detail); }
                None => rows.push((label.to_owned(), detail.to_owned())),
            }
        }
        rows.truncate(values.len() / 2);
    };
    let opts = PickerOpts { limit, slash: slash != 0, at_rest: at_rest != 0, ..PickerOpts::default() };
    let (query, pick) = unsafe { (&mut *state.engine).picker_filled(&opts, label, query, &mut { fill }) };
    if invalid.get() { return unsafe { fail(ui, "invalid picker rows") }; }
    unsafe { *out_query = state.give(query); }
    let (tag, index) = match pick { Pick::None => (0, -1), Pick::Pick(index) => (1, index), Pick::Delete(index) => (2, index), Pick::Submit => (3, -1), Pick::Back => (4, -1), Pick::Cancel => (5, -1) };
    unsafe { *out_tag = tag; *out_index = index; }
    0
});
