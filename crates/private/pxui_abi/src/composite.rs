use crate::{Build, PxuiDialogOut, PxuiUi, get, text};
use core::ffi::c_void;
use pxui_core::{ui::Ui, ui_widgets::DialogOut};
use pxui_types::{PxuiBox, Str, Size, ffi::{PxuiAnchorDesc, PxuiSizeDesc, Slice}};

const MAX_TEXT_BYTES: usize = 1 << 20;
const MAX_TOTAL_TEXT_BYTES: usize = 8 << 20;
const MAX_BUTTONS: usize = 256;

#[derive(Clone, Copy)]
struct CallbackCtx {
    error: *mut std::ffi::CString,
    returned: *mut Vec<std::ffi::CString>,
    a11y_nodes: *mut Vec<crate::FfiA11yNode>,
    grid_tracks: *mut std::collections::HashMap<i64, Vec<Box<[pxui_types::layout::Track]>>>,
    grid_updated: *mut std::collections::HashMap<i64, bool>,
}

fn size(value: PxuiSizeDesc) -> Option<Size> {
    if !value.value.is_finite() { return None; }
    Some(match value.kind {
        0 => Size::Px(value.value),
        1 => Size::Pct(value.value),
        2 => Size::Rel(value.relative?),
        3 => Size::Grow,
        4 => Size::Fit,
        _ => return None,
    })
}

unsafe fn strings(values: Slice<Str>) -> Option<Vec<&'static str>> {
    if values.len > MAX_BUTTONS || (values.len != 0 && values.ptr.is_null()) { return None; }
    let values = if values.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(values.ptr, values.len) } };
    let mut total = 0usize;
    values.iter().map(|value| {
        total = total.checked_add(value.len)?;
        if value.len > MAX_TEXT_BYTES || total > MAX_TOTAL_TEXT_BYTES { return None; }
        unsafe { text(*value) }
    }).collect()
}

unsafe fn checked_text(value: Str) -> Option<&'static str> {
    if value.len > MAX_TEXT_BYTES { None } else { unsafe { text(value) } }
}

unsafe fn call_build(ctx: CallbackCtx, engine: &mut Ui, callback: Build, user: *mut c_void) {
    let Some(callback) = callback else { return; };
    let mut borrowed = PxuiUi {
        engine: engine as *mut Ui,
        error: ctx.error,
        returned: ctx.returned,
        a11y_nodes: ctx.a11y_nodes,
        grid_tracks: ctx.grid_tracks,
        grid_updated: ctx.grid_updated,
        owner: false,
    };
    callback(user, &mut borrowed);
}

fn box_(value: pxui_core::ui::UiBox) -> PxuiBox {
    PxuiBox { index: value.index as u64, key: value.box_key, slot: value.box_slot as u64, generation: value.generation }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_modal(ui: *mut PxuiUi, label: Str, has_width: u8, width: f32, callback: Build, user: *mut c_void, out: *mut u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if out.is_null() || has_width > 1 || (has_width != 0 && (!width.is_finite() || width < 0.0)) {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid modal arguments") };
    }
    let Some(label) = (unsafe { checked_text(label) }) else { return unsafe { crate::fail(state as *mut PxuiUi, "invalid modal label") }; };
    let ctx = CallbackCtx { error: state.error, returned: state.returned, a11y_nodes: state.a11y_nodes, grid_tracks: state.grid_tracks, grid_updated: state.grid_updated };
    let mut build = |engine: &mut Ui| unsafe { call_build(ctx, engine, callback, user) };
    let open = unsafe { (&mut *state.engine).modal((has_width != 0).then_some(width), label, &mut build) };
    unsafe { *out = open as u8; }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_scroll_view(ui: *mut PxuiUi, label: Str, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if out.is_null() { return unsafe { crate::fail(state as *mut PxuiUi, "null scroll view output") }; }
    let (Some(label), Some(width), Some(height)) = (unsafe { checked_text(label) }, size(width), size(height)) else {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid scroll view arguments") };
    };
    let ctx = CallbackCtx { error: state.error, returned: state.returned, a11y_nodes: state.a11y_nodes, grid_tracks: state.grid_tracks, grid_updated: state.grid_updated };
    let mut build = |engine: &mut Ui| unsafe { call_build(ctx, engine, callback, user) };
    let handle = unsafe { (&mut *state.engine).scroll_view(label, width, height, &mut build) };
    unsafe { *out = box_(handle); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_view(ui: *mut PxuiUi, label: Str, revision: u64, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if out.is_null() { return unsafe { crate::fail(state as *mut PxuiUi, "null view output") }; }
    let (Some(label), Some(width), Some(height)) = (unsafe { checked_text(label) }, size(width), size(height)) else {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid view arguments") };
    };
    let ctx = CallbackCtx { error: state.error, returned: state.returned, a11y_nodes: state.a11y_nodes, grid_tracks: state.grid_tracks, grid_updated: state.grid_updated };
    let mut build = |engine: &mut Ui| unsafe { call_build(ctx, engine, callback, user) };
    let handle = unsafe { (&mut *state.engine).view(label, revision, width, height, &mut build) };
    unsafe { *out = box_(handle); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_anchored(ui: *mut PxuiUi, label: Str, anchor: *const PxuiAnchorDesc, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if anchor.is_null() || out.is_null() { return unsafe { crate::fail(state as *mut PxuiUi, "null anchored argument") }; }
    let (Some(label), Some(width), Some(height)) = (unsafe { checked_text(label) }, size(width), size(height)) else {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid anchored arguments") };
    };
    let anchor = unsafe { *anchor };
    if anchor.has_anchor != 1 || anchor.flip > 1 || anchor.side > 3 || anchor.align > 4
        || anchor.rect.w < 0.0 || anchor.rect.h < 0.0 || anchor.margin < 0.0
        || ![anchor.rect.x, anchor.rect.y, anchor.rect.w, anchor.rect.h, anchor.gap, anchor.margin].iter().all(|v| v.is_finite()) {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid anchor descriptor") };
    }
    let side = match anchor.side { 0 => pxui_types::layout::Side::Below, 1 => pxui_types::layout::Side::Above, 2 => pxui_types::layout::Side::Right, 3 => pxui_types::layout::Side::Left, _ => unreachable!() };
    let align = match anchor.align { 0 => pxui_types::layout::Align::Start, 1 => pxui_types::layout::Align::Center, 2 => pxui_types::layout::Align::End, 3 => pxui_types::layout::Align::Stretch, 4 => pxui_types::layout::Align::Baseline, _ => unreachable!() };
    let placement = pxui_types::layout::Placement { side, align, gap: anchor.gap, margin: anchor.margin, flip: anchor.flip != 0 };
    let ctx = CallbackCtx { error: state.error, returned: state.returned, a11y_nodes: state.a11y_nodes, grid_tracks: state.grid_tracks, grid_updated: state.grid_updated };
    let mut build = |engine: &mut Ui| unsafe { call_build(ctx, engine, callback, user) };
    let handle = unsafe { (&mut *state.engine).anchored(label, anchor.rect, placement, width, height, &mut build) };
    unsafe { *out = box_(handle); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_dialog(ui: *mut PxuiUi, title: Str, buttons: Slice<Str>, callback: Build, user: *mut c_void, out: *mut PxuiDialogOut) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if out.is_null() { return unsafe { crate::fail(state as *mut PxuiUi, "null dialog output") }; }
    let (Some(title), Some(buttons)) = (unsafe { checked_text(title) }, unsafe { strings(buttons) }) else {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid dialog arguments") };
    };
    let ctx = CallbackCtx { error: state.error, returned: state.returned, a11y_nodes: state.a11y_nodes, grid_tracks: state.grid_tracks, grid_updated: state.grid_updated };
    let mut body = |engine: &mut Ui| unsafe { call_build(ctx, engine, callback, user) };
    let result = unsafe { (&mut *state.engine).dialog(title, &buttons, &mut body) };
    unsafe { *out = match result { DialogOut::Open => PxuiDialogOut { tag: 0, index: 0 }, DialogOut::Closed => PxuiDialogOut { tag: 1, index: 0 }, DialogOut::Button(index) => PxuiDialogOut { tag: 2, index } }; }
    0
}
