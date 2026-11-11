use crate::{composite::{defer_callback_panic, deferred, resume_deferred}, check, Strs, Ui};
use core::{ffi::c_void, panic::AssertUnwindSafe};
use pxui_types::{Str, ffi::Slice};

/// A node of a preorder tree: its depth and whether it has children. The layout is the library's, so a slice of
/// these crosses the ABI as it is.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct TreeNode { pub depth: u16, pub children: bool }

use pxui_types::ffi::{PxuiCollectionEmit as Emit, PxuiF32MutSpan as F32MutSpan, PxuiListViewDesc as ListViewDesc, PxuiTableViewDesc as TableViewDesc, PxuiTreeNode as TreeNodeDesc, PxuiTreeViewDesc as TreeViewDesc, PxuiU32MutSpan as U32MutSpan, PxuiU8MutSpan as U8MutSpan};

const _: () = assert!(core::mem::size_of::<TreeNode>() == core::mem::size_of::<TreeNodeDesc>() && core::mem::align_of::<TreeNode>() == core::mem::align_of::<TreeNodeDesc>());

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

#[inline]
fn as_str(value: &str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }

// The context hands out its trampoline and its user pointer from one method, so the two cannot name different types.
struct Callback<F> { f: F }
type TextFn = Option<extern "C" fn(*mut c_void, usize, usize, Emit, *mut c_void)>;
impl<F: FnMut(usize, &mut dyn FnMut(&str))> Callback<F> {
    fn list(&mut self) -> (TextFn, *mut c_void) { (Some(call_list::<F>), (self as *mut Self).cast()) }
}
struct TableCallback<F> { f: F }
impl<F: FnMut(usize, usize, &mut dyn FnMut(&str))> TableCallback<F> {
    fn table(&mut self) -> (TextFn, *mut c_void) { (Some(call_table::<F>), (self as *mut Self).cast()) }
}
extern "C" fn call_list<F: FnMut(usize, &mut dyn FnMut(&str))>(user: *mut c_void, row: usize, _column: usize, emit: Emit, emit_user: *mut c_void) {
    if deferred() { return; }
    let state = unsafe { &mut *user.cast::<Callback<F>>() };
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut put = |s: &str| { if let Some(emit) = emit { emit(emit_user, as_str(s)); } };
        (state.f)(row, &mut put);
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}
extern "C" fn call_table<F: FnMut(usize, usize, &mut dyn FnMut(&str))>(user: *mut c_void, row: usize, column: usize, emit: Emit, emit_user: *mut c_void) {
    if deferred() { return; }
    let state = unsafe { &mut *user.cast::<TableCallback<F>>() };
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut put = |s: &str| { if let Some(emit) = emit { emit(emit_user, as_str(s)); } };
        (state.f)(row, column, &mut put);
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}

impl Ui {
    /// Builds a virtual list. `selected` has one flag per row; the result is an activated row. The flags are the
    /// library's own storage for the call: nothing is copied.
    #[track_caller] pub fn list_view<F: FnMut(usize, &mut dyn FnMut(&str))>(&mut self, label: &str, count: usize, multi: bool, selected: &mut [bool], text: F) -> Option<usize> {
        let mut context = Callback { f: text };
        let (text, user) = context.list();
        let desc = ListViewDesc {
            label: as_str(label), count, multi: multi as u8, reserved: [0; 7],
            // a `bool` is a byte; the library rejects any value above 1 before it writes
            selected: U8MutSpan { ptr: selected.as_mut_ptr().cast::<u8>(), len: selected.len() },
            text, user,
        };
        let mut out = usize::MAX;
        let rc = unsafe { sys::pxui_ui_list_view(self.0.as_ptr(), &desc, &mut out) };
        resume_deferred();
        check(self, rc);
        (out != usize::MAX).then_some(out)
    }

    /// Builds a preorder virtual tree. `visible` is replaced by the current flattened row indices (its capacity is reused).
    #[track_caller] pub fn tree<F: FnMut(usize, &mut dyn FnMut(&str))>(&mut self, label: &str, nodes: &[TreeNode], open: &mut [bool], selected: &mut Option<usize>, visible: &mut Vec<u32>, text: F) -> Option<usize> {
        visible.clear();
        visible.reserve(nodes.len());
        let mut rows_len = 0usize;
        let mut selected_raw = selected.unwrap_or(usize::MAX);
        let mut context = Callback { f: text };
        let (text, user) = context.list();
        let desc = TreeViewDesc {
            label: as_str(label), nodes: Slice { ptr: nodes.as_ptr().cast::<TreeNodeDesc>(), len: nodes.len() },
            open: U8MutSpan { ptr: open.as_mut_ptr().cast::<u8>(), len: open.len() }, selected: &mut selected_raw,
            // the library writes at most `nodes.len()` rows into the vector's spare capacity
            visible: U32MutSpan { ptr: visible.as_mut_ptr(), len: nodes.len() }, visible_len: &mut rows_len,
            text, user,
        };
        let mut out = usize::MAX;
        let rc = unsafe { sys::pxui_ui_tree(self.0.as_ptr(), &desc, &mut out) };
        resume_deferred();
        check(self, rc);
        *selected = (selected_raw != usize::MAX).then_some(selected_raw);
        assert!(rows_len <= nodes.len(), "pxui: the library reported more visible rows than there are nodes");
        // the library wrote `rows_len` rows into the spare capacity
        unsafe { visible.set_len(rows_len); }
        (out != usize::MAX).then_some(out)
    }

    /// Builds a virtual table. Widths and selection are updated in place; the result is a sorted column.
    #[track_caller] pub fn table_view<F: FnMut(usize, usize, &mut dyn FnMut(&str))>(&mut self, label: &str, titles: &[&str], widths: &mut [f32], rows: usize, sort: Option<(usize, bool)>, selected: &mut Option<usize>, cell: F) -> Option<usize> {
        let titles = Strs::new(titles);
        let mut context = TableCallback { f: cell };
        let (text, user) = context.table();
        let mut selected_raw = selected.unwrap_or(usize::MAX);
        let desc = TableViewDesc {
            label: as_str(label), titles: titles.span(),
            widths: F32MutSpan { ptr: widths.as_mut_ptr(), len: widths.len() }, rows,
            has_sort: sort.is_some() as u8, ascending: sort.map_or(0, |(_, ascending)| ascending as u8), reserved: [0; 6],
            sort_column: sort.map_or(0, |(column, _)| column), selected: &mut selected_raw,
            text, user,
        };
        let mut out = usize::MAX;
        let rc = unsafe { sys::pxui_ui_table_view(self.0.as_ptr(), &desc, &mut out) };
        resume_deferred();
        check(self, rc);
        *selected = (selected_raw != usize::MAX).then_some(selected_raw);
        (out != usize::MAX).then_some(out)
    }
}

#[cfg(all(test, feature = "static"))]
mod tests {
    use super::*;

    /// The panel of the gallery's collections page, headless: a split of a list and a tree over a table, 100,000 rows each.
    /// It used to crash (SIGSEGV) because `split` handed the library a pointer to its closure where the trampoline read the closure.
    #[test]
    fn collections_page_builds_through_the_library_and_updates_state() {
        const ROWS: usize = 100_000;
        let mut ui = Ui::new().expect("library");
        let mut list = vec![false; ROWS];
        let nodes: Vec<TreeNode> = (0..ROWS).map(|i| TreeNode { depth: [0, 1, 2, 2, 1, 2][i % 6], children: matches!(i % 6, 0 | 1 | 4) }).collect();
        let mut open: Vec<bool> = (0..ROWS).map(|i| i < 6).collect();
        let (mut selected, mut visible, mut widths, mut table_sel) = (Some(2usize), Vec::with_capacity(ROWS), [96.0f32, 72.0, 72.0], Some(1usize));
        let (texts, cells) = (core::cell::Cell::new(0usize), core::cell::Cell::new(0usize));
        for count in 0..3 {
            let input = pxui_types::FrameIn { width: 640.0, height: 464.0, time: count as f64 / 60.0, ..Default::default() };
            ui.frame(&input, &mut |ui: &mut Ui| {
                ui.view("page", count, pxui_types::Size::Grow, pxui_types::Size::Grow, &mut |ui: &mut Ui| {
                    let ratio = ui.split("split", pxui_types::Axis::Row, 0.34, 120.0, 200.0, &mut |ui: &mut Ui| {
                        let _ = ui.list_view("Files", ROWS, true, &mut list, |i, put| { texts.set(texts.get() + 1); put(&format!("file {i}")); });
                    }, &mut |ui: &mut Ui| {
                        let _ = ui.tree("Outline", &nodes, &mut open, &mut selected, &mut visible, |i, put| { texts.set(texts.get() + 1); put(&format!("node {i}")); });
                        let _ = ui.table_view("Points", &["Name", "Group", "Tag"], &mut widths, ROWS, Some((0, true)), &mut table_sel, |r, c, put| { cells.set(cells.get() + 1); put(&format!("{r}:{c}")); });
                    });
                    assert!(ratio.is_none() || ratio.is_some());
                });
            }).expect("frame");
        }
        assert!(texts.get() > 0 && cells.get() > 0, "the virtual rows were asked for their text");
        assert!(!visible.is_empty() && visible.len() <= nodes.len(), "{}", visible.len());
        assert_eq!(visible[0], 0);
    }
}
