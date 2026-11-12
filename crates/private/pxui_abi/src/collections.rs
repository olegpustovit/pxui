use crate::state::{PxuiUi, checked, fail, get, strings, text};
use crate::{PXUI_ERR_ARGUMENT, Str};
use core::{cell::Cell, ffi::c_void, mem::{align_of, offset_of, size_of}};
use pxui_core::ui_widgets::TreeNode;
pub use pxui_types::ffi::{PxuiListViewWithDesc, PxuiTableViewWithDesc, PxuiTreeViewWithDesc, PxuiCollectionEmit, PxuiCollectionText, PxuiF32MutSpan, PxuiListViewDesc, PxuiTableViewDesc, PxuiTreeNode, PxuiTreeViewDesc, PxuiU32MutSpan, PxuiU8MutSpan};

const MAX_ITEMS: usize = 1_000_000;

// The engine's node and the C node share a layout, so a caller's array is the engine's slice with no copy.
const _: () = assert!(size_of::<TreeNode>() == size_of::<PxuiTreeNode>() && align_of::<TreeNode>() == align_of::<PxuiTreeNode>()
    && offset_of!(TreeNode, depth) == offset_of!(PxuiTreeNode, depth) && offset_of!(TreeNode, children) == offset_of!(PxuiTreeNode, children));

struct EmitCtx { invalid: bool }
// The Rust callback target stays in a stack context and never crosses the ABI boundary.
struct EmitCtxWithTarget<'a> { ctx: EmitCtx, target: Option<&'a mut dyn FnMut(&str)> }
// What C emits is checked as text here; its lengths are checked by the entry the text goes to.
extern "C" fn emit_to_target(user: *mut c_void, value: Str) {
    let state = unsafe { &mut *user.cast::<EmitCtxWithTarget<'_>>() };
    let Some(text) = (unsafe { text(value) }) else { state.ctx.invalid = true; return; };
    if let Some(target) = state.target.as_deref_mut() { target(text); }
}

fn valid_span<T>(ptr: *mut T, len: usize, required: usize) -> bool {
    len == required && len <= MAX_ITEMS && (len == 0 || !ptr.is_null())
}
fn valid_output_span<T>(ptr: *mut T, len: usize, required: usize) -> bool {
    len >= required && len <= MAX_ITEMS && (len == 0 || !ptr.is_null())
}
/// Runs the C text callback for one row or cell, its emitted text going to `put`; false when it emitted text that is not UTF-8.
fn callback_text(callback: PxuiCollectionText, user: *mut c_void, row: usize, column: usize, put: &mut dyn FnMut(&str)) -> bool {
    if let Some(callback) = callback {
        let mut state = EmitCtxWithTarget { ctx: EmitCtx { invalid: false }, target: Some(put) };
        callback(user, row, column, Some(emit_to_target), (&mut state as *mut EmitCtxWithTarget<'_>).cast());
        !state.ctx.invalid
    } else {
        true
    }
}

export!(trusted pub unsafe fn pxui_ui_list_view(ui: *mut PxuiUi, desc: *const PxuiListViewDesc, out: *mut usize) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null list view argument") }; }
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid list view descriptor") }; };
    if desc.multi > 1 || desc.reserved != [0; 3] || desc.count > MAX_ITEMS || !valid_span(desc.selected.ptr, desc.selected.len, desc.count) {
        return unsafe { fail(ui, "invalid list view descriptor") };
    }
    let Some(label) = (unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid list view label") }; };
    // the engine reads a flag as not 0 and writes 0 or 1, so the caller's bytes are its flags as they are: no scan, no copy
    let selected: &mut [u8] = if desc.selected.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.selected.ptr, desc.selected.len) } };
    let bad_text = Cell::new(false);
    let mut activated = None;
    let rc = unsafe { crate::trusted::list_view(ui, label, desc.count, desc.multi != 0, selected, &mut |row, put| if !callback_text(desc.text, desc.user, row, 0, put) { bad_text.set(true); }, &mut activated) };
    if rc != 0 { return rc; }
    if bad_text.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    unsafe { *out = activated.unwrap_or(usize::MAX); }
    0
});

export!(trusted pub unsafe fn pxui_ui_tree(ui: *mut PxuiUi, desc: *const PxuiTreeViewDesc, out: *mut usize) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null tree argument") }; }
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid tree descriptor") }; };
    if desc.nodes.len > MAX_ITEMS || (desc.nodes.len != 0 && desc.nodes.ptr.is_null()) || !valid_span(desc.open.ptr, desc.open.len, desc.nodes.len)
        || !valid_output_span(desc.visible.ptr, desc.visible.len, desc.nodes.len) || desc.selected.is_null() || desc.visible_len.is_null() {
        return unsafe { fail(ui, "invalid tree descriptor") };
    }
    let Some(label) = (unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid tree label") }; };
    let raw_nodes = if desc.nodes.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(desc.nodes.ptr, desc.nodes.len) } };
    // any byte is a valid flag (not 0: has children), so the nodes and the open flags need no scan
    let open: &mut [u8] = if desc.open.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.open.ptr, desc.open.len) } };
    let mut selected = (unsafe { *desc.selected } != usize::MAX).then_some(unsafe { *desc.selected });
    // the engine writes the rows into the caller's buffer itself (at least a row per node, checked above): nothing is copied
    let visible: &mut [u32] = if desc.visible.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.visible.ptr, desc.visible.len) } };
    let bad_text = Cell::new(false);
    let mut result = (None, 0);
    let rc = unsafe { crate::trusted::tree(ui, label, raw_nodes, open, &mut selected, visible, &mut |row, put| if !callback_text(desc.text, desc.user, row, 0, put) { bad_text.set(true); }, &mut result) };
    if rc != 0 { return rc; }
    if bad_text.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    unsafe { *desc.visible_len = result.1; *desc.selected = selected.unwrap_or(usize::MAX); *out = result.0.unwrap_or(usize::MAX); }
    0
});

export!(trusted pub unsafe fn pxui_ui_table_view(ui: *mut PxuiUi, desc: *const PxuiTableViewDesc, out: *mut usize) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null table argument") }; }
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid table descriptor") }; };
    if desc.has_sort > 1 || desc.ascending > 1 || desc.reserved != [0; 2] || desc.rows > MAX_ITEMS || desc.titles.len > 4096
        || desc.widths.len != desc.titles.len || !valid_span(desc.widths.ptr, desc.widths.len, desc.titles.len) || desc.selected.is_null() {
        return unsafe { fail(ui, "invalid table descriptor") };
    }
    let mut titles = state.shared().take_strs();
    let Some(label) = (unsafe { checked(desc.label) }).filter(|_| unsafe { strings(desc.titles, 4096, &mut titles) }) else {
        state.shared().put_strs(titles);
        return unsafe { fail(ui, "invalid table strings") };
    };
    let widths: &mut [f32] = if desc.widths.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.widths.ptr, desc.widths.len) } };
    let sort = (desc.has_sort != 0).then_some((desc.sort_column, desc.ascending != 0));
    let mut selected = (unsafe { *desc.selected } != usize::MAX).then_some(unsafe { *desc.selected });
    let bad_text = Cell::new(false);
    let mut activated = None;
    let rc = unsafe { crate::trusted::table_view(ui, label, &titles, widths, desc.rows, sort, &mut selected, &mut |row, col, put| if !callback_text(desc.text, desc.user, row, col, put) { bad_text.set(true); }, &mut activated) };
    state.shared().put_strs(titles);
    if rc != 0 { return rc; }
    if bad_text.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    unsafe { *desc.selected = selected.unwrap_or(usize::MAX); *out = activated.unwrap_or(usize::MAX); }
    0
});

export!(trusted pub unsafe fn pxui_ui_list_view_with(ui: *mut PxuiUi, desc: *const PxuiListViewWithDesc, out: *mut usize) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null list view argument") }; }
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid list view descriptor") }; };
    if desc.multi > 1 || desc.reserved != [0; 3] || desc.count > MAX_ITEMS || !valid_span(desc.selected.ptr, desc.selected.len, desc.count) {
        return unsafe { fail(ui, "invalid list view descriptor") };
    }
    let Some(label) = (unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid list view label") }; };
    let selected: &mut [u8] = if desc.selected.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.selected.ptr, desc.selected.len) } };
    let mut activated = None;
    let rc = unsafe { crate::trusted::list_view_with(ui, label, desc.count, desc.multi != 0, selected, &mut |ui, row, column| if let Some(build) = desc.build { build(desc.user, ui.cast(), row, column) }, &mut activated) };
    if rc != 0 { return rc; }
    unsafe { *out = activated.unwrap_or(usize::MAX); }
    0
});

export!(trusted pub unsafe fn pxui_ui_tree_with(ui: *mut PxuiUi, desc: *const PxuiTreeViewWithDesc, out: *mut usize) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null tree argument") }; }
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid tree descriptor") }; };
    if desc.nodes.len > MAX_ITEMS || (desc.nodes.len != 0 && desc.nodes.ptr.is_null()) || !valid_span(desc.open.ptr, desc.open.len, desc.nodes.len)
        || !valid_output_span(desc.visible.ptr, desc.visible.len, desc.nodes.len) || desc.selected.is_null() || desc.visible_len.is_null() {
        return unsafe { fail(ui, "invalid tree descriptor") };
    }
    let Some(label) = (unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid tree label") }; };
    let raw_nodes = if desc.nodes.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(desc.nodes.ptr, desc.nodes.len) } };
    let open: &mut [u8] = if desc.open.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.open.ptr, desc.open.len) } };
    let mut selected = (unsafe { *desc.selected } != usize::MAX).then_some(unsafe { *desc.selected });
    let visible: &mut [u32] = if desc.visible.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.visible.ptr, desc.visible.len) } };
    let mut result = (None, 0);
    let rc = unsafe { crate::trusted::tree_with(ui, label, raw_nodes, open, &mut selected, visible, &mut |ui, node, column| if let Some(build) = desc.build { build(desc.user, ui.cast(), node, column) }, &mut result) };
    if rc != 0 { return rc; }
    unsafe { *desc.visible_len = result.1; *desc.selected = selected.unwrap_or(usize::MAX); *out = result.0.unwrap_or(usize::MAX); }
    0
});

export!(trusted pub unsafe fn pxui_ui_table_view_with(ui: *mut PxuiUi, desc: *const PxuiTableViewWithDesc, out: *mut usize) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null table argument") }; }
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid table descriptor") }; };
    if desc.has_sort > 1 || desc.ascending > 1 || desc.reserved != [0; 2] || desc.rows > MAX_ITEMS || desc.titles.len > 4096
        || desc.widths.len != desc.titles.len || !valid_span(desc.widths.ptr, desc.widths.len, desc.titles.len) || desc.selected.is_null() {
        return unsafe { fail(ui, "invalid table descriptor") };
    }
    let mut titles = state.shared().take_strs();
    let Some(label) = (unsafe { checked(desc.label) }).filter(|_| unsafe { strings(desc.titles, 4096, &mut titles) }) else {
        state.shared().put_strs(titles);
        return unsafe { fail(ui, "invalid table strings") };
    };
    let widths: &mut [f32] = if desc.widths.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.widths.ptr, desc.widths.len) } };
    let sort = (desc.has_sort != 0).then_some((desc.sort_column, desc.ascending != 0));
    let mut selected = (unsafe { *desc.selected } != usize::MAX).then_some(unsafe { *desc.selected });
    let mut activated = None;
    let rc = unsafe { crate::trusted::table_view_with(ui, label, &titles, widths, desc.rows, sort, &mut selected, &mut |ui, row, column| if let Some(build) = desc.build { build(desc.user, ui.cast(), row, column) }, &mut activated) };
    state.shared().put_strs(titles);
    if rc != 0 { return rc; }
    unsafe { *desc.selected = selected.unwrap_or(usize::MAX); *out = activated.unwrap_or(usize::MAX); }
    0
});
