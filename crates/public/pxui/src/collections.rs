use crate::{check, Ui};

/// A node of a preorder tree: its depth and whether it has children (not 0). The layout is the library's, so a slice of
/// these crosses the ABI as it is.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct TreeNode { pub depth: u16, pub children: u8 }

use pxui_types::ffi::PxuiTreeNode as TreeNodeDesc;
const _: () = assert!(core::mem::size_of::<TreeNode>() == core::mem::size_of::<TreeNodeDesc>() && core::mem::align_of::<TreeNode>() == core::mem::align_of::<TreeNodeDesc>());

// The C-callback side of the collections (the dynamic build): trampolines the library calls for a row's text.
#[cfg(not(feature = "static"))]
mod link {
    use super::*;
    use crate::composite::{defer_callback_panic, deferred};
    use core::{ffi::c_void, panic::AssertUnwindSafe};
    use pxui_types::{Str, ffi::{PxuiListViewWithDesc as ListViewWithDesc, PxuiTableViewWithDesc as TableViewWithDesc, PxuiTreeViewWithDesc as TreeViewWithDesc, PxuiCollectionEmit as Emit, PxuiF32MutSpan as F32MutSpan, PxuiListViewDesc as ListViewDesc, PxuiTableViewDesc as TableViewDesc, PxuiTreeViewDesc as TreeViewDesc, PxuiU32MutSpan as U32MutSpan, PxuiU8MutSpan as U8MutSpan, Slice}};

    #[link(name = "pxui")]
    unsafe extern "C" {
        fn pxui_ui_list_view(ui: *mut crate::sys::PxuiUi, desc: *const ListViewDesc, out: *mut usize) -> i32;
        fn pxui_ui_tree(ui: *mut crate::sys::PxuiUi, desc: *const TreeViewDesc, out: *mut usize) -> i32;
        fn pxui_ui_table_view(ui: *mut crate::sys::PxuiUi, desc: *const TableViewDesc, out: *mut usize) -> i32;
        fn pxui_ui_list_view_with(ui: *mut crate::sys::PxuiUi, desc: *const ListViewWithDesc, out: *mut usize) -> i32;
        fn pxui_ui_tree_with(ui: *mut crate::sys::PxuiUi, desc: *const TreeViewWithDesc, out: *mut usize) -> i32;
        fn pxui_ui_table_view_with(ui: *mut crate::sys::PxuiUi, desc: *const TableViewWithDesc, out: *mut usize) -> i32;
    }

    /// The cell callback of the `_with` forms: the caller's closure behind a trait object, so one trampoline serves all three.
    type Cell<'a> = &'a mut dyn FnMut(&mut Ui, usize, usize);
    extern "C" fn call_cell(user: *mut c_void, raw: *mut c_void, row: usize, column: usize) {
        if raw.is_null() || deferred() { return; }
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let mut ui = core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new(raw.cast()).unwrap()));
            (unsafe { &mut *user.cast::<Cell>() })(&mut ui, row, column);
        }));
        if let Err(panic) = result { defer_callback_panic(panic); }
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
        pub(super) fn list_view_with_c(&mut self, label: &str, count: usize, multi: bool, selected: &mut [u8], mut cell: Cell) -> Option<usize> {
            let desc = ListViewWithDesc {
                size: core::mem::size_of::<ListViewWithDesc>() as u32, multi: multi as u8, reserved: [0; 3], label: as_str(label), count,
                selected: U8MutSpan { ptr: selected.as_mut_ptr(), len: selected.len() }, build: Some(call_cell), user: (&mut cell as *mut Cell).cast(),
            };
            let mut out = usize::MAX;
            let rc = unsafe { pxui_ui_list_view_with(self.0.as_ptr(), &desc, &mut out) };
            crate::composite::resume_deferred();
            check(self, rc);
            (out != usize::MAX).then_some(out)
        }

        pub(super) fn tree_with_c(&mut self, label: &str, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], mut cell: Cell) -> (Option<usize>, usize) {
            let mut rows_len = 0usize;
            let mut selected_raw = selected.unwrap_or(usize::MAX);
            let desc = TreeViewWithDesc {
                size: core::mem::size_of::<TreeViewWithDesc>() as u32, reserved: 0, label: as_str(label), nodes: Slice { ptr: nodes.as_ptr().cast::<TreeNodeDesc>(), len: nodes.len() },
                open: U8MutSpan { ptr: open.as_mut_ptr(), len: open.len() }, selected: &mut selected_raw,
                visible: U32MutSpan { ptr: visible.as_mut_ptr(), len: visible.len() }, visible_len: &mut rows_len,
                build: Some(call_cell), user: (&mut cell as *mut Cell).cast(),
            };
            let mut out = usize::MAX;
            let rc = unsafe { pxui_ui_tree_with(self.0.as_ptr(), &desc, &mut out) };
            crate::composite::resume_deferred();
            check(self, rc);
            *selected = (selected_raw != usize::MAX).then_some(selected_raw);
            ((out != usize::MAX).then_some(out), rows_len)
        }

        pub(super) fn table_view_with_c(&mut self, label: &str, titles: &[&str], widths: &mut [f32], rows: usize, sort: Option<(usize, bool)>, selected: &mut Option<usize>, mut cell: Cell) -> Option<usize> {
            let titles = crate::Strs::new(titles);
            let mut selected_raw = selected.unwrap_or(usize::MAX);
            let desc = TableViewWithDesc {
                size: core::mem::size_of::<TableViewWithDesc>() as u32, label: as_str(label), titles: titles.span(),
                widths: F32MutSpan { ptr: widths.as_mut_ptr(), len: widths.len() }, rows,
                has_sort: sort.is_some() as u8, ascending: sort.map_or(0, |(_, ascending)| ascending as u8), reserved: [0; 2],
                sort_column: sort.map_or(0, |(column, _)| column), selected: &mut selected_raw,
                build: Some(call_cell), user: (&mut cell as *mut Cell).cast(),
            };
            let mut out = usize::MAX;
            let rc = unsafe { pxui_ui_table_view_with(self.0.as_ptr(), &desc, &mut out) };
            crate::composite::resume_deferred();
            check(self, rc);
            *selected = (selected_raw != usize::MAX).then_some(selected_raw);
            (out != usize::MAX).then_some(out)
        }

        pub(super) fn list_view_c<F: FnMut(usize, &mut dyn FnMut(&str))>(&mut self, label: &str, count: usize, multi: bool, selected: &mut [u8], text: F) -> Option<usize> {
            let mut context = Callback { f: text };
            let (text, user) = context.list();
            let desc = ListViewDesc {
                size: core::mem::size_of::<ListViewDesc>() as u32, label: as_str(label), count, multi: multi as u8, reserved: [0; 3],
                selected: U8MutSpan { ptr: selected.as_mut_ptr(), len: selected.len() },
                text, user,
            };
            let mut out = usize::MAX;
            let rc = unsafe { pxui_ui_list_view(self.0.as_ptr(), &desc, &mut out) };
            crate::composite::resume_deferred();
            check(self, rc);
            (out != usize::MAX).then_some(out)
        }

        pub(super) fn tree_c<F: FnMut(usize, &mut dyn FnMut(&str))>(&mut self, label: &str, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], text: F) -> (Option<usize>, usize) {
            let mut rows_len = 0usize;
            let mut selected_raw = selected.unwrap_or(usize::MAX);
            let mut context = Callback { f: text };
            let (text, user) = context.list();
            let desc = TreeViewDesc {
                size: core::mem::size_of::<TreeViewDesc>() as u32, reserved: 0, label: as_str(label), nodes: Slice { ptr: nodes.as_ptr().cast::<TreeNodeDesc>(), len: nodes.len() },
                open: U8MutSpan { ptr: open.as_mut_ptr(), len: open.len() }, selected: &mut selected_raw,
                visible: U32MutSpan { ptr: visible.as_mut_ptr(), len: visible.len() }, visible_len: &mut rows_len,
                text, user,
            };
            let mut out = usize::MAX;
            let rc = unsafe { pxui_ui_tree(self.0.as_ptr(), &desc, &mut out) };
            crate::composite::resume_deferred();
            check(self, rc);
            *selected = (selected_raw != usize::MAX).then_some(selected_raw);
            ((out != usize::MAX).then_some(out), rows_len)
        }

        pub(super) fn table_view_c<F: FnMut(usize, usize, &mut dyn FnMut(&str))>(&mut self, label: &str, titles: &[&str], widths: &mut [f32], rows: usize, sort: Option<(usize, bool)>, selected: &mut Option<usize>, cell: F) -> Option<usize> {
            let titles = crate::Strs::new(titles);
            let mut context = TableCallback { f: cell };
            let (text, user) = context.table();
            let mut selected_raw = selected.unwrap_or(usize::MAX);
            let desc = TableViewDesc {
                size: core::mem::size_of::<TableViewDesc>() as u32, label: as_str(label), titles: titles.span(),
                widths: F32MutSpan { ptr: widths.as_mut_ptr(), len: widths.len() }, rows,
                has_sort: sort.is_some() as u8, ascending: sort.map_or(0, |(_, ascending)| ascending as u8), reserved: [0; 2],
                sort_column: sort.map_or(0, |(column, _)| column), selected: &mut selected_raw,
                text, user,
            };
            let mut out = usize::MAX;
            let rc = unsafe { pxui_ui_table_view(self.0.as_ptr(), &desc, &mut out) };
            crate::composite::resume_deferred();
            check(self, rc);
            *selected = (selected_raw != usize::MAX).then_some(selected_raw);
            (out != usize::MAX).then_some(out)
        }
    }
}

impl Ui {
    /// Builds a virtual list. `selected` has one flag per row (not 0: selected); the result is an activated row. The flags are
    /// the library's own storage for the call: nothing is copied or scanned.
    #[track_caller] pub fn list_view<F: FnMut(usize, &mut dyn FnMut(&str))>(&mut self, label: &str, count: usize, multi: bool, selected: &mut [u8], text: F) -> Option<usize> {
        #[cfg(feature = "static")]
        {
            let (mut text, mut out) = (text, None);
            check(self, unsafe { pxui_abi_link::trusted::list_view(self.0.as_ptr(), label, count, multi, selected, &mut text, &mut out) });
            out
        }
        #[cfg(not(feature = "static"))]
        self.list_view_c(label, count, multi, selected, text)
    }

    /// Builds a preorder virtual tree. The library writes the shown rows into the start of `visible` (at least a row per node);
    /// the result is an activated node and the number of rows shown.
    #[track_caller] pub fn tree<F: FnMut(usize, &mut dyn FnMut(&str))>(&mut self, label: &str, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], text: F) -> (Option<usize>, usize) {
        #[cfg(feature = "static")]
        {
            let (mut text, mut out) = (text, (None, 0));
            let nodes = unsafe { core::slice::from_raw_parts(nodes.as_ptr().cast::<TreeNodeDesc>(), nodes.len()) };
            check(self, unsafe { pxui_abi_link::trusted::tree(self.0.as_ptr(), label, nodes, open, selected, visible, &mut text, &mut out) });
            out
        }
        #[cfg(not(feature = "static"))]
        self.tree_c(label, nodes, open, selected, visible, text)
    }

    /// Builds a virtual table. Widths and selection are updated in place; the result is a sorted column.
    #[track_caller] pub fn table_view<F: FnMut(usize, usize, &mut dyn FnMut(&str))>(&mut self, label: &str, titles: &[&str], widths: &mut [f32], rows: usize, sort: Option<(usize, bool)>, selected: &mut Option<usize>, cell: F) -> Option<usize> {
        #[cfg(feature = "static")]
        {
            let (mut cell, mut out) = (cell, None);
            check(self, unsafe { pxui_abi_link::trusted::table_view(self.0.as_ptr(), label, titles, widths, rows, sort, selected, &mut cell, &mut out) });
            out
        }
        #[cfg(not(feature = "static"))]
        self.table_view_c(label, titles, widths, rows, sort, selected, cell)
    }
}

impl Ui {
    /// `list_view` with the caller's own row content: `cell(ui, row)` builds the children of row `row` (any widgets) inside the row node
    /// the `list_view` body shapes. The rows' accessible names are the caller's.
    #[track_caller] pub fn list_view_with(&mut self, label: &str, count: usize, multi: bool, selected: &mut [u8], cell: &mut dyn FnMut(&mut Ui, usize)) -> Option<usize> {
        #[cfg(feature = "static")]
        {
            let mut out = None;
            check(self, unsafe { pxui_abi_link::trusted::list_view_with(self.0.as_ptr(), label, count, multi, selected, &mut |raw, row, _| (&mut *cell)(&mut *core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new_unchecked(raw))), row), &mut out) });
            out
        }
        #[cfg(not(feature = "static"))]
        self.list_view_with_c(label, count, multi, selected, &mut |ui, row, _| cell(ui, row))
    }

    /// `tree` with the caller's own row content: `cell(ui, node)` builds the children of the row that shows tree node `node`, after
    /// the disclosure mark.
    #[track_caller] pub fn tree_with(&mut self, label: &str, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], cell: &mut dyn FnMut(&mut Ui, usize)) -> (Option<usize>, usize) {
        #[cfg(feature = "static")]
        {
            let mut out = (None, 0);
            let nodes = unsafe { core::slice::from_raw_parts(nodes.as_ptr().cast::<TreeNodeDesc>(), nodes.len()) };
            check(self, unsafe { pxui_abi_link::trusted::tree_with(self.0.as_ptr(), label, nodes, open, selected, visible, &mut |raw, node, _| (&mut *cell)(&mut *core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new_unchecked(raw))), node), &mut out) });
            out
        }
        #[cfg(not(feature = "static"))]
        self.tree_with_c(label, nodes, open, selected, visible, &mut |ui, node, _| cell(ui, node))
    }

    /// `table_view` with the caller's own cells: `cell(ui, row, column)` builds the children of one cell (any widgets) inside the cell node.
    #[track_caller] pub fn table_view_with(&mut self, label: &str, titles: &[&str], widths: &mut [f32], rows: usize, sort: Option<(usize, bool)>, selected: &mut Option<usize>, cell: &mut dyn FnMut(&mut Ui, usize, usize)) -> Option<usize> {
        #[cfg(feature = "static")]
        {
            let mut out = None;
            check(self, unsafe { pxui_abi_link::trusted::table_view_with(self.0.as_ptr(), label, titles, widths, rows, sort, selected, &mut |raw, row, column| (&mut *cell)(&mut *core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new_unchecked(raw))), row, column), &mut out) });
            out
        }
        #[cfg(not(feature = "static"))]
        self.table_view_with_c(label, titles, widths, rows, sort, selected, cell)
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
        let mut list = vec![0u8; ROWS];
        let nodes: Vec<TreeNode> = (0..ROWS).map(|i| TreeNode { depth: [0, 1, 2, 2, 1, 2][i % 6], children: matches!(i % 6, 0 | 1 | 4) as u8 }).collect();
        let mut open: Vec<u8> = (0..ROWS).map(|i| (i < 6) as u8).collect();
        let (mut selected, mut visible, mut widths, mut table_sel) = (Some(2usize), vec![0u32; ROWS], [96.0f32, 72.0, 72.0], Some(1usize));
        let (texts, cells, mut shown) = (core::cell::Cell::new(0usize), core::cell::Cell::new(0usize), 0);
        for count in 0..3 {
            let input = pxui_types::FrameIn { width: 640.0, height: 464.0, time: count as f64 / 60.0, ..Default::default() };
            ui.frame(&input, &mut |ui: &mut Ui| {
                ui.view("page", count, pxui_types::Size::Grow, pxui_types::Size::Grow, &mut |ui: &mut Ui| {
                    let ratio = ui.split("split", pxui_types::Axis::Row, 0.34, 120.0, 200.0, &mut |ui: &mut Ui| {
                        let _ = ui.list_view("Files", ROWS, true, &mut list, |i, put| { texts.set(texts.get() + 1); put(&format!("file {i}")); });
                    }, &mut |ui: &mut Ui| {
                        shown = ui.tree("Outline", &nodes, &mut open, &mut selected, &mut visible, |i, put| { texts.set(texts.get() + 1); put(&format!("node {i}")); }).1;
                        let _ = ui.table_view("Points", &["Name", "Group", "Tag"], &mut widths, ROWS, Some((0, true)), &mut table_sel, |r, c, put| { cells.set(cells.get() + 1); put(&format!("{r}:{c}")); });
                    });
                    assert!(ratio.is_none() || ratio.is_some());
                });
            }).expect("frame");
        }
        assert!(texts.get() > 0 && cells.get() > 0, "the virtual rows were asked for their text");
        assert!(shown > 0 && shown <= nodes.len(), "{shown}");
        assert_eq!(visible[0], 0);
    }
}
