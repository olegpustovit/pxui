use crate::Ui;
use core::{any::Any, ffi::c_void, panic::AssertUnwindSafe};
use pxui_types::{Str, ffi::Slice};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct TreeNode { pub depth: u16, pub children: bool }
#[repr(C)]
#[derive(Clone, Copy, Default)]
#[cfg(not(feature = "static"))]
struct TreeNodeDesc { depth: u16, children: u8, reserved: u8 }

#[repr(C)]
#[derive(Clone, Copy, Default)]
#[cfg(not(feature = "static"))]
struct U8MutSpan { ptr: *mut u8, len: usize }
#[repr(C)]
#[derive(Clone, Copy, Default)]
#[cfg(not(feature = "static"))]
struct F32MutSpan { ptr: *mut f32, len: usize }
#[repr(C)]
#[derive(Clone, Copy, Default)]
#[cfg(not(feature = "static"))]
struct U32MutSpan { ptr: *mut u32, len: usize }

#[cfg(feature = "static")]
use pxui_abi_link::PxuiCollectionEmit as Emit;
#[cfg(not(feature = "static"))]
type Emit = Option<extern "C" fn(*mut c_void, Str)>;
#[cfg(not(feature = "static"))]
type Text = Option<extern "C" fn(*mut c_void, usize, usize, Emit, *mut c_void)>;

#[cfg(feature = "static")]
use pxui_abi_link::{
    PxuiF32MutSpan as F32MutSpan, PxuiListViewDesc as ListViewDesc,
    PxuiTableViewDesc as TableViewDesc, PxuiTreeNode as TreeNodeDesc,
    PxuiTreeViewDesc as TreeViewDesc, PxuiU32MutSpan as U32MutSpan,
    PxuiU8MutSpan as U8MutSpan,
};
#[cfg(not(feature = "static"))]
#[repr(C)]
struct ListViewDesc { label: Str, count: usize, multi: u8, reserved: [u8; 7], selected: U8MutSpan, text: Text, user: *mut c_void }
#[cfg(not(feature = "static"))]
#[repr(C)]
struct TreeViewDesc { label: Str, nodes: Slice<TreeNodeDesc>, open: U8MutSpan, selected: *mut usize, visible: U32MutSpan, visible_len: *mut usize, text: Text, user: *mut c_void }
#[cfg(not(feature = "static"))]
#[repr(C)]
struct TableViewDesc { label: Str, titles: Slice<Str>, widths: F32MutSpan, rows: usize, has_sort: u8, ascending: u8, reserved: [u8; 6], sort_column: usize, selected: *mut usize, text: Text, user: *mut c_void }

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{pxui_ui_list_view, pxui_ui_table_view, pxui_ui_tree};
}
#[cfg(not(feature = "static"))]
mod sys {
    use super::{ListViewDesc, TableViewDesc, TreeViewDesc};
    use crate::sys::PxuiUi;
    #[link(name = "pxui")]
    unsafe extern "C" {
        pub fn pxui_ui_list_view(ui: *mut PxuiUi, desc: *const ListViewDesc, out: *mut usize) -> i32;
        pub fn pxui_ui_tree(ui: *mut PxuiUi, desc: *const TreeViewDesc, out: *mut usize) -> i32;
        pub fn pxui_ui_table_view(ui: *mut PxuiUi, desc: *const TableViewDesc, out: *mut usize) -> i32;
    }
}

fn as_str(value: &str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }
fn status(rc: i32) -> Result<(), i32> { if rc == 0 { Ok(()) } else { Err(rc) } }

struct Callback<F> { f: F, panic: Option<Box<dyn Any + Send>> }
extern "C" fn call_list<F: FnMut(usize, &mut dyn FnMut(&str))>(user: *mut c_void, row: usize, _column: usize, emit: Emit, emit_user: *mut c_void) {
    let state = unsafe { &mut *user.cast::<Callback<F>>() };
    if state.panic.is_some() { return; }
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut put = |s: &str| { if let Some(emit) = emit { emit(emit_user, as_str(s)); } };
        (state.f)(row, &mut put);
    }));
    if let Err(panic) = result { state.panic = Some(panic); }
}
extern "C" fn call_table<F: FnMut(usize, usize, &mut dyn FnMut(&str))>(user: *mut c_void, row: usize, column: usize, emit: Emit, emit_user: *mut c_void) {
    let state = unsafe { &mut *user.cast::<Callback<F>>() };
    if state.panic.is_some() { return; }
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut put = |s: &str| { if let Some(emit) = emit { emit(emit_user, as_str(s)); } };
        (state.f)(row, column, &mut put);
    }));
    if let Err(panic) = result { state.panic = Some(panic); }
}

impl Ui {
    /// Builds a virtual list. `selected` has one flag per row; the result is an activated row.
    pub fn list_view<F: FnMut(usize, &mut dyn FnMut(&str))>(&mut self, label: &str, count: usize, multi: bool, selected: &mut [bool], text: F) -> Result<Option<usize>, i32> {
        let mut flags: Vec<u8> = selected.iter().map(|v| *v as u8).collect();
        let mut callback = Callback { f: text, panic: None };
        let desc = ListViewDesc {
            label: as_str(label), count, multi: multi as u8, reserved: [0; 7],
            selected: U8MutSpan { ptr: flags.as_mut_ptr(), len: flags.len() },
            text: Some(call_list::<F>), user: (&mut callback as *mut Callback<F>).cast::<c_void>(),
        };
        let mut out = usize::MAX;
        let rc = unsafe { sys::pxui_ui_list_view(self.0.as_ptr(), &desc, &mut out) };
        if let Some(panic) = callback.panic.take() { std::panic::resume_unwind(panic); }
        status(rc)?;
        for (dst, src) in selected.iter_mut().zip(flags) { *dst = src != 0; }
        Ok((out != usize::MAX).then_some(out))
    }

    /// Builds a preorder virtual tree. `visible` is replaced by the current flattened row indices.
    pub fn tree<F: FnMut(usize, &mut dyn FnMut(&str))>(&mut self, label: &str, nodes: &[TreeNode], open: &mut [bool], selected: &mut Option<usize>, visible: &mut Vec<u32>, text: F) -> Result<Option<usize>, i32> {
        let mut flags: Vec<u8> = open.iter().map(|v| *v as u8).collect();
        let raw_nodes: Vec<_> = nodes.iter().map(|node| TreeNodeDesc { depth: node.depth, children: node.children as u8, reserved: 0 }).collect();
        let mut rows = vec![0u32; nodes.len()];
        let mut rows_len = 0usize;
        let mut selected_raw = selected.unwrap_or(usize::MAX);
        let mut callback = Callback { f: text, panic: None };
        let desc = TreeViewDesc {
            label: as_str(label), nodes: Slice { ptr: raw_nodes.as_ptr(), len: raw_nodes.len() },
            open: U8MutSpan { ptr: flags.as_mut_ptr(), len: flags.len() }, selected: &mut selected_raw,
            visible: U32MutSpan { ptr: rows.as_mut_ptr(), len: rows.len() }, visible_len: &mut rows_len,
            text: Some(call_list::<F>), user: (&mut callback as *mut Callback<F>).cast::<c_void>(),
        };
        let mut out = usize::MAX;
        let rc = unsafe { sys::pxui_ui_tree(self.0.as_ptr(), &desc, &mut out) };
        if let Some(panic) = callback.panic.take() { std::panic::resume_unwind(panic); }
        status(rc)?;
        for (dst, src) in open.iter_mut().zip(flags) { *dst = src != 0; }
        *selected = (selected_raw != usize::MAX).then_some(selected_raw);
        if rows_len > rows.len() { return Err(-1); }
        visible.clear();
        visible.extend_from_slice(&rows[..rows_len]);
        Ok((out != usize::MAX).then_some(out))
    }

    /// Builds a virtual table. Widths and selection are updated in place; the result is a sorted column.
    pub fn table_view<F: FnMut(usize, usize, &mut dyn FnMut(&str))>(&mut self, label: &str, titles: &[&str], widths: &mut [f32], rows: usize, sort: Option<(usize, bool)>, selected: &mut Option<usize>, cell: F) -> Result<Option<usize>, i32> {
        let title_values: Vec<Str> = titles.iter().map(|s| as_str(s)).collect();
        let mut callback = Callback { f: cell, panic: None };
        let mut selected_raw = selected.unwrap_or(usize::MAX);
        let desc = TableViewDesc {
            label: as_str(label), titles: Slice { ptr: title_values.as_ptr(), len: title_values.len() },
            widths: F32MutSpan { ptr: widths.as_mut_ptr(), len: widths.len() }, rows,
            has_sort: sort.is_some() as u8, ascending: sort.map_or(0, |(_, ascending)| ascending as u8), reserved: [0; 6],
            sort_column: sort.map_or(0, |(column, _)| column), selected: &mut selected_raw,
            text: Some(call_table::<F>), user: (&mut callback as *mut Callback<F>).cast::<c_void>(),
        };
        let mut out = usize::MAX;
        let rc = unsafe { sys::pxui_ui_table_view(self.0.as_ptr(), &desc, &mut out) };
        if let Some(panic) = callback.panic.take() { std::panic::resume_unwind(panic); }
        status(rc)?;
        *selected = (selected_raw != usize::MAX).then_some(selected_raw);
        Ok((out != usize::MAX).then_some(out))
    }
}
