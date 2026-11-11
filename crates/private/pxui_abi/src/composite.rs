use crate::node::size;
use crate::state::{Build, PxuiUi, call_build, checked, fail, get, strings, to_abi_box};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, PxuiDialogOut, Str};
use core::ffi::c_void;
use pxui_core::{ui::Ui, ui_widgets::DialogOut};
use pxui_types::ffi::{PxuiAnchorDesc, PxuiSizeDesc, Slice};

const MAX_BUTTONS: usize = 256;

export!(pub unsafe fn pxui_ui_modal(ui: *mut PxuiUi, label: Str, has_width: u8, width: f32, callback: Build, user: *mut c_void, out: *mut u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() || has_width > 1 || (has_width != 0 && (!width.is_finite() || width < 0.0)) {
        return unsafe { fail(ui, "invalid modal arguments") };
    }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid modal label") }; };
    let shared = state.shared;
    let mut build = |engine: &mut Ui| call_build(shared, engine, callback, user);
    let open = unsafe { (&mut *state.engine).modal((has_width != 0).then_some(width), label, &mut build) };
    unsafe { *out = open as u8; }
    0
});

export!(pub unsafe fn pxui_ui_scroll_view(ui: *mut PxuiUi, label: Str, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null scroll view output") }; }
    let (Some(label), Some(width), Some(height)) = (unsafe { checked(label) }, size(width), size(height)) else {
        return unsafe { fail(ui, "invalid scroll view arguments") };
    };
    let shared = state.shared;
    let mut build = |engine: &mut Ui| call_build(shared, engine, callback, user);
    let handle = unsafe { (&mut *state.engine).scroll_view(label, width, height, &mut build) };
    unsafe { *out = to_abi_box(handle); }
    0
});

unsafe fn view_with(ui: *mut PxuiUi, label: Str, revision: u64, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox, flat: bool) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null view output") }; }
    let (Some(label), Some(width), Some(height)) = (unsafe { checked(label) }, size(width), size(height)) else {
        return unsafe { fail(ui, "invalid view arguments") };
    };
    let shared = state.shared;
    let mut build = |engine: &mut Ui| call_build(shared, engine, callback, user);
    let engine = unsafe { &mut *state.engine };
    let handle = if flat { engine.view_flat(label, revision, width, height, &mut build) } else { engine.view(label, revision, width, height, &mut build) };
    unsafe { *out = to_abi_box(handle); }
    0
}

export!(pub unsafe fn pxui_ui_view(ui: *mut PxuiUi, label: Str, revision: u64, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    unsafe { view_with(ui, label, revision, width, height, callback, user, out, false) }
});

// A view that takes no entry of its own in the painter view table: its instances ride its parent entry.
export!(pub unsafe fn pxui_ui_view_flat(ui: *mut PxuiUi, label: Str, revision: u64, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    unsafe { view_with(ui, label, revision, width, height, callback, user, out, true) }
});

export!(pub unsafe fn pxui_ui_anchored(ui: *mut PxuiUi, label: Str, anchor: *const PxuiAnchorDesc, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if anchor.is_null() || out.is_null() { return unsafe { fail(ui, "null anchored argument") }; }
    let (Some(label), Some(width), Some(height)) = (unsafe { checked(label) }, size(width), size(height)) else {
        return unsafe { fail(ui, "invalid anchored arguments") };
    };
    let anchor = unsafe { *anchor };
    if anchor.has_anchor != 1 || anchor.flip > 1 || anchor.side > 3 || anchor.align > 4
        || anchor.rect.w < 0.0 || anchor.rect.h < 0.0 || anchor.margin < 0.0
        || ![anchor.rect.x, anchor.rect.y, anchor.rect.w, anchor.rect.h, anchor.gap, anchor.margin].iter().all(|v| v.is_finite()) {
        return unsafe { fail(ui, "invalid anchor descriptor") };
    }
    let side = match anchor.side { 0 => pxui_types::layout::Side::Below, 1 => pxui_types::layout::Side::Above, 2 => pxui_types::layout::Side::Right, 3 => pxui_types::layout::Side::Left, _ => unreachable!() };
    let align = match anchor.align { 0 => pxui_types::layout::Align::Start, 1 => pxui_types::layout::Align::Center, 2 => pxui_types::layout::Align::End, 3 => pxui_types::layout::Align::Stretch, 4 => pxui_types::layout::Align::Baseline, _ => unreachable!() };
    let placement = pxui_types::layout::Placement { side, align, gap: anchor.gap, margin: anchor.margin, flip: anchor.flip != 0 };
    let shared = state.shared;
    let mut build = |engine: &mut Ui| call_build(shared, engine, callback, user);
    let handle = unsafe { (&mut *state.engine).anchored(label, anchor.rect, placement, width, height, &mut build) };
    unsafe { *out = to_abi_box(handle); }
    0
});

export!(pub unsafe fn pxui_ui_dialog(ui: *mut PxuiUi, title: Str, buttons: Slice<Str>, callback: Build, user: *mut c_void, out: *mut PxuiDialogOut) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null dialog output") }; }
    let mut list = state.shared().take_strs();
    let Some(title) = (unsafe { checked(title) }).filter(|_| unsafe { strings(buttons, MAX_BUTTONS, &mut list) }) else {
        state.shared().put_strs(list);
        return unsafe { fail(ui, "invalid dialog arguments") };
    };
    let shared = state.shared;
    let result = unsafe { (&mut *state.engine).dialog(title, &list, &mut |engine: &mut Ui| call_build(shared, engine, callback, user)) };
    state.shared().put_strs(list);
    unsafe { *out = match result { DialogOut::Open => PxuiDialogOut { tag: 0, index: 0 }, DialogOut::Closed => PxuiDialogOut { tag: 1, index: 0 }, DialogOut::Button(index) => PxuiDialogOut { tag: 2, index } }; }
    0
});
