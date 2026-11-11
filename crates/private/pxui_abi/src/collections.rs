use crate::{get, text, PxuiUi};
use core::{cell::Cell, ffi::c_void};
use pxui_core::ui_widgets::TreeNode;
use pxui_types::{Str, ffi::Slice};

const MAX_ITEMS: usize = 1_000_000;
const MAX_TEXT_BYTES: usize = 1 << 20;
const MAX_TOTAL_TEXT_BYTES: usize = 8 << 20;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiU8MutSpan { pub ptr: *mut u8, pub len: usize }
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiF32MutSpan { pub ptr: *mut f32, pub len: usize }
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiU32MutSpan { pub ptr: *mut u32, pub len: usize }

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiTreeNode { pub depth: u16, pub children: u8, pub reserved: u8 }

pub type PxuiCollectionEmit = Option<extern "C" fn(*mut c_void, Str)>;
pub type PxuiCollectionText = Option<extern "C" fn(*mut c_void, usize, usize, PxuiCollectionEmit, *mut c_void)>;

#[repr(C)]
pub struct PxuiListViewDesc {
    pub label: Str,
    pub count: usize,
    pub multi: u8,
    pub reserved: [u8; 7],
    pub selected: PxuiU8MutSpan,
    pub text: PxuiCollectionText,
    pub user: *mut c_void,
}

#[repr(C)]
pub struct PxuiTreeViewDesc {
    pub label: Str,
    pub nodes: Slice<PxuiTreeNode>,
    pub open: PxuiU8MutSpan,
    pub selected: *mut usize,
    pub visible: PxuiU32MutSpan,
    pub visible_len: *mut usize,
    pub text: PxuiCollectionText,
    pub user: *mut c_void,
}

#[repr(C)]
pub struct PxuiTableViewDesc {
    pub label: Str,
    pub titles: Slice<Str>,
    pub widths: PxuiF32MutSpan,
    pub rows: usize,
    pub has_sort: u8,
    pub ascending: u8,
    pub reserved: [u8; 6],
    pub sort_column: usize,
    pub selected: *mut usize,
    pub text: PxuiCollectionText,
    pub user: *mut c_void,
}

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
unsafe fn checked(value: Str) -> Option<&'static str> {
    if value.len > MAX_TEXT_BYTES { return None; }
    unsafe { text(value) }
}
unsafe fn strings(values: Slice<Str>) -> Option<Vec<&'static str>> {
    if values.len > 4096 || (values.len != 0 && values.ptr.is_null()) { return None; }
    let values = if values.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(values.ptr, values.len) } };
    let mut total = 0usize;
    values.iter().map(|v| {
        total = total.checked_add(v.len)?;
        if total > MAX_TOTAL_TEXT_BYTES { return None; }
        unsafe { checked(*v) }
    }).collect()
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_list_view(ui: *mut PxuiUi, desc: *const PxuiListViewDesc, out: *mut usize) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if desc.is_null() || out.is_null() { return unsafe { crate::fail(ui, "null list view argument") }; }
    let desc = unsafe { &*desc };
    if desc.multi > 1 || desc.reserved != [0; 7] || desc.count > MAX_ITEMS || !valid_span(desc.selected.ptr, desc.selected.len, desc.count) {
        return unsafe { crate::fail(ui, "invalid list view descriptor") };
    }
    let Some(label) = (unsafe { checked(desc.label) }) else { return unsafe { crate::fail(ui, "invalid list view label") }; };
    let flags: &mut [u8] = if desc.selected.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.selected.ptr, desc.selected.len) } };
    if flags.iter().any(|v| *v > 1) { return unsafe { crate::fail(ui, "invalid list selection flags") }; }
    let mut selected: Vec<bool> = flags.iter().map(|v| *v != 0).collect();
    let bad_text = Cell::new(false);
    let text_budget = Cell::new(0);
    let activated = unsafe { (&mut *state.engine).list_view(label, desc.count, desc.multi != 0, &mut selected, &mut |i, put| if !callback_text(desc.text, desc.user, i, 0, &text_budget, put) { bad_text.set(true); }) };
    if bad_text.get() { return unsafe { crate::fail(ui, "invalid collection callback text") }; }
    for (out_flag, value) in flags.iter_mut().zip(selected) { *out_flag = value as u8; }
    unsafe { *out = activated.unwrap_or(usize::MAX); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_tree(ui: *mut PxuiUi, desc: *const PxuiTreeViewDesc, out: *mut usize) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if desc.is_null() || out.is_null() { return unsafe { crate::fail(ui, "null tree argument") }; }
    let desc = unsafe { &*desc };
    if desc.nodes.len > MAX_ITEMS || (desc.nodes.len != 0 && desc.nodes.ptr.is_null()) || !valid_span(desc.open.ptr, desc.open.len, desc.nodes.len)
        || !valid_output_span(desc.visible.ptr, desc.visible.len, desc.nodes.len) || desc.selected.is_null() || desc.visible_len.is_null() {
        return unsafe { crate::fail(ui, "invalid tree descriptor") };
    }
    let Some(label) = (unsafe { checked(desc.label) }) else { return unsafe { crate::fail(ui, "invalid tree label") }; };
    let raw_nodes = if desc.nodes.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(desc.nodes.ptr, desc.nodes.len) } };
    if raw_nodes.iter().any(|n| n.children > 1 || n.reserved != 0) { return unsafe { crate::fail(ui, "invalid tree node") }; }
    let nodes: Vec<_> = raw_nodes.iter().map(|n| TreeNode { depth: n.depth, children: n.children != 0 }).collect();
    let raw_open: &mut [u8] = if desc.open.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.open.ptr, desc.open.len) } };
    if raw_open.iter().any(|v| *v > 1) { return unsafe { crate::fail(ui, "invalid tree open flags") }; }
    let mut open: Vec<bool> = raw_open.iter().map(|v| *v != 0).collect();
    let mut selected = (unsafe { *desc.selected } != usize::MAX).then_some(unsafe { *desc.selected });
    let mut visible = Vec::with_capacity(desc.visible.len);
    let bad_text = Cell::new(false);
    let text_budget = Cell::new(0);
    let activated = unsafe { (&mut *state.engine).tree(label, &nodes, &mut open, &mut selected, &mut visible, &mut |i, put| if !callback_text(desc.text, desc.user, i, 0, &text_budget, put) { bad_text.set(true); }) };
    if bad_text.get() { return unsafe { crate::fail(ui, "invalid collection callback text") }; }
    for (dst, value) in raw_open.iter_mut().zip(open) { *dst = value as u8; }
    if visible.len() > desc.visible.len { return unsafe { crate::fail(ui, "tree visible output buffer too small") }; }
    if !visible.is_empty() { unsafe { core::ptr::copy_nonoverlapping(visible.as_ptr(), desc.visible.ptr, visible.len()) }; }
    unsafe { *desc.visible_len = visible.len(); *desc.selected = selected.unwrap_or(usize::MAX); *out = activated.unwrap_or(usize::MAX); }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_table_view(ui: *mut PxuiUi, desc: *const PxuiTableViewDesc, out: *mut usize) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if desc.is_null() || out.is_null() { return unsafe { crate::fail(ui, "null table argument") }; }
    let desc = unsafe { &*desc };
    if desc.has_sort > 1 || desc.ascending > 1 || desc.reserved != [0; 6] || desc.rows > MAX_ITEMS || desc.titles.len > 4096
        || desc.widths.len != desc.titles.len || !valid_span(desc.widths.ptr, desc.widths.len, desc.titles.len) || desc.selected.is_null() {
        return unsafe { crate::fail(ui, "invalid table descriptor") };
    }
    let (Some(label), Some(titles)) = (unsafe { checked(desc.label) }, unsafe { strings(desc.titles) }) else { return unsafe { crate::fail(ui, "invalid table strings") }; };
    let widths: &mut [f32] = if desc.widths.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.widths.ptr, desc.widths.len) } };
    if widths.iter().any(|w| !w.is_finite() || *w < 0.0) { return unsafe { crate::fail(ui, "invalid table widths") }; }
    let mut selected = (unsafe { *desc.selected } != usize::MAX).then_some(unsafe { *desc.selected });
    let sort = (desc.has_sort != 0).then_some((desc.sort_column, desc.ascending != 0));
    if sort.is_some_and(|(column, _)| column >= titles.len()) { return unsafe { crate::fail(ui, "invalid table sort column") }; }
    let bad_text = Cell::new(false);
    let text_budget = Cell::new(0);
    let activated = unsafe { (&mut *state.engine).table_view(label, &titles, widths, desc.rows, sort, &mut selected, &|row, col, put| if !callback_text(desc.text, desc.user, row, col, &text_budget, put) { bad_text.set(true); }) };
    if bad_text.get() { return unsafe { crate::fail(ui, "invalid collection callback text") }; }
    unsafe { *desc.selected = selected.unwrap_or(usize::MAX); *out = activated.unwrap_or(usize::MAX); }
    0
}
