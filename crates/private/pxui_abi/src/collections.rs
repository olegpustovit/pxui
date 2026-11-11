use crate::state::{MAX_TEXT_BYTES, MAX_TOTAL_TEXT_BYTES, PxuiUi, checked, fail, get, strings, text};
use crate::{PXUI_ERR_ARGUMENT, Str};
use core::{cell::Cell, ffi::c_void, mem::{align_of, offset_of, size_of}};
use pxui_core::ui_widgets::TreeNode;
pub use pxui_types::ffi::{PxuiCollectionEmit, PxuiCollectionText, PxuiF32MutSpan, PxuiListViewDesc, PxuiTableViewDesc, PxuiTreeNode, PxuiTreeViewDesc, PxuiU32MutSpan, PxuiU8MutSpan};

const MAX_ITEMS: usize = 1_000_000;

// The engine's node and the C node share a layout, so a caller's array is the engine's slice with no copy.
const _: () = assert!(size_of::<TreeNode>() == size_of::<PxuiTreeNode>() && align_of::<TreeNode>() == align_of::<PxuiTreeNode>()
    && offset_of!(TreeNode, depth) == offset_of!(PxuiTreeNode, depth) && offset_of!(TreeNode, children) == offset_of!(PxuiTreeNode, children));

struct EmitCtx<'a> { invalid: bool, bytes: usize, total: &'a Cell<usize> }
// The Rust callback target stays in a stack context and never crosses the ABI boundary.
struct EmitCtxWithTarget<'a> { ctx: EmitCtx<'a>, target: Option<&'a mut dyn FnMut(&str)> }
extern "C" fn emit_to_target(user: *mut c_void, value: Str) {
    let state = unsafe { &mut *user.cast::<EmitCtxWithTarget<'_>>() };
    if value.len > MAX_TEXT_BYTES || state.ctx.bytes.saturating_add(value.len) > MAX_TEXT_BYTES
        || state.ctx.total.get().saturating_add(value.len) > MAX_TOTAL_TEXT_BYTES {
        state.ctx.invalid = true;
        return;
    }
    let Some(text) = (unsafe { text(value) }) else { state.ctx.invalid = true; return; };
    state.ctx.bytes += text.len();
    state.ctx.total.set(state.ctx.total.get() + text.len());
    if let Some(target) = state.target.as_deref_mut() { target(text); }
}

fn valid_span<T>(ptr: *mut T, len: usize, required: usize) -> bool {
    len == required && len <= MAX_ITEMS && (len == 0 || !ptr.is_null())
}
fn valid_output_span<T>(ptr: *mut T, len: usize, required: usize) -> bool {
    len >= required && len <= MAX_ITEMS && (len == 0 || !ptr.is_null())
}
fn callback_text(callback: PxuiCollectionText, user: *mut c_void, row: usize, column: usize, total: &Cell<usize>, put: &mut dyn FnMut(&str)) -> bool {
    if let Some(callback) = callback {
        let mut state = EmitCtxWithTarget { ctx: EmitCtx { invalid: false, bytes: 0, total }, target: Some(put) };
        callback(user, row, column, Some(emit_to_target), (&mut state as *mut EmitCtxWithTarget<'_>).cast());
        !state.ctx.invalid
    } else {
        true
    }
}

export!(pub unsafe fn pxui_ui_list_view(ui: *mut PxuiUi, desc: *const PxuiListViewDesc, out: *mut usize) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null list view argument") }; }
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid list view descriptor") }; };
    if desc.multi > 1 || desc.reserved != [0; 3] || desc.count > MAX_ITEMS || !valid_span(desc.selected.ptr, desc.selected.len, desc.count) {
        return unsafe { fail(ui, "invalid list view descriptor") };
    }
    let Some(label) = (unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid list view label") }; };
    let flags: &mut [u8] = if desc.selected.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.selected.ptr, desc.selected.len) } };
    if flags.iter().any(|v| *v > 1) { return unsafe { fail(ui, "invalid list selection flags") }; }
    // every byte is 0 or 1 now, so the caller's flags are the engine's `&mut [bool]` with no copy
    let selected: &mut [bool] = unsafe { core::slice::from_raw_parts_mut(flags.as_mut_ptr().cast::<bool>(), flags.len()) };
    let bad_text = Cell::new(false);
    let text_budget = Cell::new(0);
    let activated = unsafe { (&mut *state.engine).list_view(label, desc.count, desc.multi != 0, selected, &mut |i, put| if !callback_text(desc.text, desc.user, i, 0, &text_budget, put) { bad_text.set(true); }) };
    if bad_text.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    unsafe { *out = activated.unwrap_or(usize::MAX); }
    0
});

export!(pub unsafe fn pxui_ui_tree(ui: *mut PxuiUi, desc: *const PxuiTreeViewDesc, out: *mut usize) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null tree argument") }; }
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid tree descriptor") }; };
    if desc.nodes.len > MAX_ITEMS || (desc.nodes.len != 0 && desc.nodes.ptr.is_null()) || !valid_span(desc.open.ptr, desc.open.len, desc.nodes.len)
        || !valid_output_span(desc.visible.ptr, desc.visible.len, desc.nodes.len) || desc.selected.is_null() || desc.visible_len.is_null() {
        return unsafe { fail(ui, "invalid tree descriptor") };
    }
    let Some(label) = (unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid tree label") }; };
    let raw_nodes = if desc.nodes.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(desc.nodes.ptr, desc.nodes.len) } };
    if raw_nodes.iter().any(|n| n.children > 1) { return unsafe { fail(ui, "invalid tree node") }; }
    let nodes: &[TreeNode] = unsafe { core::slice::from_raw_parts(raw_nodes.as_ptr().cast::<TreeNode>(), raw_nodes.len()) };
    let open_flags: &mut [u8] = if desc.open.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.open.ptr, desc.open.len) } };
    if open_flags.iter().any(|v| *v > 1) { return unsafe { fail(ui, "invalid tree open flags") }; }
    let open: &mut [bool] = unsafe { core::slice::from_raw_parts_mut(open_flags.as_mut_ptr().cast::<bool>(), open_flags.len()) };
    let mut selected = (unsafe { *desc.selected } != usize::MAX).then_some(unsafe { *desc.selected });
    // the engine fills a `Vec`; its buffer lives on the Ui, taken so a nested call cannot reach it
    let mut visible = core::mem::take(&mut state.shared().visible);
    visible.clear();
    let bad_text = Cell::new(false);
    let text_budget = Cell::new(0);
    let activated = unsafe { (&mut *state.engine).tree(label, nodes, open, &mut selected, &mut visible, &mut |i, put| if !callback_text(desc.text, desc.user, i, 0, &text_budget, put) { bad_text.set(true); }) };
    let fits = visible.len() <= desc.visible.len;
    if !bad_text.get() && fits {
        if !visible.is_empty() { unsafe { core::ptr::copy_nonoverlapping(visible.as_ptr(), desc.visible.ptr, visible.len()) }; }
        unsafe { *desc.visible_len = visible.len(); *desc.selected = selected.unwrap_or(usize::MAX); *out = activated.unwrap_or(usize::MAX); }
    }
    state.shared().visible = visible;
    if bad_text.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    if !fits { return unsafe { fail(ui, "tree visible output buffer too small") }; }
    0
});

export!(pub unsafe fn pxui_ui_table_view(ui: *mut PxuiUi, desc: *const PxuiTableViewDesc, out: *mut usize) -> i32 {
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
    if widths.iter().any(|w| !w.is_finite() || *w < 0.0) || sort.is_some_and(|(column, _)| column >= titles.len()) {
        state.shared().put_strs(titles);
        return unsafe { fail(ui, "invalid table widths or sort column") };
    }
    let mut selected = (unsafe { *desc.selected } != usize::MAX).then_some(unsafe { *desc.selected });
    let bad_text = Cell::new(false);
    let text_budget = Cell::new(0);
    let activated = unsafe { (&mut *state.engine).table_view(label, &titles, widths, desc.rows, sort, &mut selected, &|row, col, put| if !callback_text(desc.text, desc.user, row, col, &text_budget, put) { bad_text.set(true); }) };
    state.shared().put_strs(titles);
    if bad_text.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    unsafe { *desc.selected = selected.unwrap_or(usize::MAX); *out = activated.unwrap_or(usize::MAX); }
    0
});
