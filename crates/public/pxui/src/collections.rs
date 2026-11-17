use crate::{check, Ui};
#[cfg(not(feature = "static"))]
use pxui_types::Str;

/// A node of a preorder tree: its depth and whether it has children (not 0). The layout is the library's, so a slice of
/// these crosses the ABI as it is.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct TreeNode {
    pub depth: u16,
    pub children: u8,
}

use pxui_types::ffi::PxuiTreeNode as TreeNodeDesc;
const _: () = assert!(core::mem::size_of::<TreeNode>() == core::mem::size_of::<TreeNodeDesc>() && core::mem::align_of::<TreeNode>() == core::mem::align_of::<TreeNodeDesc>());

// The C-callback side of the collections (the dynamic build): trampolines the library calls for a row's text.
#[cfg(not(feature = "static"))]
mod link {
    use super::*;
    use crate::composite::{defer_callback_panic, deferred};
    use core::{ffi::c_void, panic::AssertUnwindSafe};
    use pxui_types::{Str, ffi::{PxuiListViewWithDesc as ListViewWithDesc, PxuiTableViewWithDesc as TableViewWithDesc, PxuiTreeViewWithDesc as TreeViewWithDesc, PxuiF32MutSpan as F32MutSpan, PxuiU32MutSpan as U32MutSpan, PxuiU8MutSpan as U8MutSpan, Slice}};

    #[link(name = "pxui")]
    unsafe extern "C" {
        fn pxui_ui_list_view_with(ui: *mut crate::sys::PxuiUi, desc: *const ListViewWithDesc, out: *mut usize) -> i32;
        fn pxui_ui_tree_with(ui: *mut crate::sys::PxuiUi, desc: *const TreeViewWithDesc, out: *mut usize) -> i32;
        fn pxui_ui_table_view_with(ui: *mut crate::sys::PxuiUi, desc: *const TableViewWithDesc, out: *mut usize) -> i32;
    }

    /// The cell callback: the caller's closure behind a trait object, so one trampoline serves all three collections.
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

    impl Ui {
        pub(super) fn list_view_c(&mut self, label: &str, count: usize, multi: bool, selected: &mut [u8], mut cell: Cell) -> Option<usize> {
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

        pub(super) fn tree_c(&mut self, label: &str, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], mut cell: Cell) -> (Option<usize>, usize) {
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

        pub(super) fn table_view_c(&mut self, label: &str, titles: &[&str], widths: &mut [f32], rows: usize, sort: Option<(usize, bool)>, selected: &mut Option<usize>, mut cell: Cell) -> Option<usize> {
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

    }
}

impl Ui {
    /// A selectable virtual list of `count` rows filling its parent: `cell(ui, row)` builds the rows in view inside the row node the
    /// `list_view` body shapes (`item_text` for the kit's text row; any widgets otherwise). Returns the row a double click or Enter
    /// activated.
    ///
    /// `selected` holds one flag a row (not 0: selected), the library's own storage for the call: a click selects a row alone; with
    /// `multi`, Command (Ctrl) toggles one and Shift selects a run. One Tab stop; arrows, Page keys, Home and End move the keyboard's row.
    /// Allocates: never on a warm frame.
    /// Identity: `label` keys the list under its parent; rows are keyed by index.
    /// Panics: when the library refuses the call (outside a frame, `selected` shorter than `count`), or when `cell` panics.
    ///
    /// ```
    /// let mut selected = [0u8; 3];
    /// pxui::doc::frames(1, &mut |ui| { ui.list_view("Files", 3, true, &mut selected, &mut |ui, row| ui.item_text(["a", "b", "c"][row])); });
    /// ```
    #[track_caller] pub fn list_view(&mut self, label: &str, count: usize, multi: bool, selected: &mut [u8], cell: &mut dyn FnMut(&mut Ui, usize)) -> Option<usize> {
        #[cfg(feature = "static")]
        {
            let mut out = None;
            check(self, unsafe { pxui_abi_link::trusted::list_view(self.0.as_ptr(), label, count, multi, selected, &mut |raw, row, _| (&mut *cell)(&mut *core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new_unchecked(raw))), row), &mut out) });
            out
        }
        #[cfg(not(feature = "static"))]
        self.list_view_c(label, count, multi, selected, &mut |ui, row, _| cell(ui, row))
    }

    /// A preorder virtual tree: `cell(ui, node)` builds the row that shows tree node `node`, after the disclosure mark (`item_text` for
    /// the kit's text row, which names the row). The library writes the shown rows into the start of `visible` (at least a row per
    /// node); returns the node a double click or Enter activated and the number of rows shown.
    ///
    /// `open` holds one flag a node (not 0: expanded); `selected` is the selected node, both updated in place.
    /// Allocates: never on a warm frame.
    /// Identity: `label` keys the tree under its parent; rows are keyed by index.
    /// Panics: when the library refuses the call (outside a frame, `open` or `visible` too short), or when `cell` panics.
    ///
    /// ```
    /// use pxui::collections::TreeNode;
    /// let nodes = [TreeNode { depth: 0, children: 1 }, TreeNode { depth: 1, children: 0 }];
    /// let (mut open, mut selected, mut visible) = ([1u8, 0], None, [0u32; 2]);
    /// pxui::doc::frames(1, &mut |ui| { ui.tree("Outline", &nodes, &mut open, &mut selected, &mut visible, &mut |ui, node| ui.item_text(["root", "leaf"][node])); });
    /// assert_eq!(visible, [0, 1]);
    /// ```
    #[track_caller] pub fn tree(&mut self, label: &str, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], cell: &mut dyn FnMut(&mut Ui, usize)) -> (Option<usize>, usize) {
        #[cfg(feature = "static")]
        {
            let mut out = (None, 0);
            let nodes = unsafe { core::slice::from_raw_parts(nodes.as_ptr().cast::<TreeNodeDesc>(), nodes.len()) };
            check(self, unsafe { pxui_abi_link::trusted::tree(self.0.as_ptr(), label, nodes, open, selected, visible, &mut |raw, node, _| (&mut *cell)(&mut *core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new_unchecked(raw))), node), &mut out) });
            out
        }
        #[cfg(not(feature = "static"))]
        self.tree_c(label, nodes, open, selected, visible, &mut |ui, node, _| cell(ui, node))
    }

    /// A virtual table: a header of `titles` over `rows` rows, columns `widths` points wide; `cell(ui, row, column)` builds one cell
    /// inside the cell node (`item_text` for the kit's text cell, which names the cell). Widths and selection update in place; returns
    /// the column whose header was clicked (the sort intent).
    ///
    /// `sort` marks the sorted column with a chevron (up when ascending); dragging a header grip resizes its column.
    /// Allocates: never on a warm frame.
    /// Identity: `label` keys the table under its parent; rows are keyed by index.
    /// Panics: when the library refuses the call (outside a frame, `widths` not one a title, a sort column past them), or when `cell`
    /// panics.
    ///
    /// ```
    /// let (mut widths, mut selected) = ([80.0f32, 60.0], None);
    /// pxui::doc::frames(1, &mut |ui| { ui.table_view("Points", &["Name", "Tag"], &mut widths, 2, None, &mut selected, &mut |ui, row, column| ui.item_text(["a", "b", "c", "d"][row * 2 + column])); });
    /// ```
    #[allow(clippy::too_many_arguments)]
    #[track_caller] pub fn table_view(&mut self, label: &str, titles: &[&str], widths: &mut [f32], rows: usize, sort: Option<(usize, bool)>, selected: &mut Option<usize>, cell: &mut dyn FnMut(&mut Ui, usize, usize)) -> Option<usize> {
        #[cfg(feature = "static")]
        {
            let mut out = None;
            check(self, unsafe { pxui_abi_link::trusted::table_view(self.0.as_ptr(), label, titles, widths, rows, sort, selected, &mut |raw, row, column| (&mut *cell)(&mut *core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new_unchecked(raw))), row, column), &mut out) });
            out
        }
        #[cfg(not(feature = "static"))]
        self.table_view_c(label, titles, widths, rows, sort, selected, cell)
    }

    /// The kit's text of the collection item `cell` is building: a `list_view` row's label, a `tree` row's (naming the row) or a
    /// `table_view` cell's (naming the cell), in the item's look, through its row body.
    ///
    /// Call it from a `list_view`, `tree` or `table_view` cell callback; the text is the item's accessible name where it names one.
    /// Allocates: never on a warm frame (text it has not shaped before is shaped once).
    /// Panics: outside a collection item, on invalid text, or when the `Ui` has failed.
    ///
    /// ```
    /// let mut selected = [0u8; 2];
    /// pxui::doc::frames(1, &mut |ui| { ui.list_view("Files", 2, false, &mut selected, &mut |ui, row| ui.item_text(["notes.txt", "song.flac"][row])); });
    /// ```
    #[inline]
    #[track_caller] pub fn item_text(&mut self, text: &str) {
        // static: the trusted entry takes the checked UTF-8 text as it is, one call a row
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::item_text(self.0.as_ptr(), text) });
        #[cfg(not(feature = "static"))]
        check(self, unsafe { item::pxui_ui_item_text(self.0.as_ptr(), Str { ptr: text.as_ptr(), len: text.len() }) })
    }
}

#[cfg(not(feature = "static"))]
mod item {
    use pxui_types::Str;
    #[link(name = "pxui")]
    unsafe extern "C" { pub fn pxui_ui_item_text(ui: *mut crate::sys::PxuiUi, text: Str) -> i32; }
}

#[cfg(test)]
mod item_text_tests {
    use super::TreeNode;
    use crate::Ui;

    /// `item_text` puts the kit's text in each collection's item: tree rows and table cells are named by it (as the C text-row
    /// exports name them), list rows are not; outside a collection item it is refused.
    #[test]
    fn item_text_names_tree_rows_and_table_cells_and_is_refused_outside_an_item() {
        let mut ui = Ui::new().expect("library");
        ui.set_a11y(true);
        let nodes = [TreeNode { depth: 0, children: 0 }];
        let (mut list, mut open, mut selected, mut visible, mut widths, mut cell) = ([0u8; 1], [0u8; 1], None, [0u32; 1], [80.0f32], None);
        let input = crate::FrameIn { width: 400.0, height: 300.0, ..crate::FrameIn::default() };
        for _ in 0..2 {
            ui.frame(&input, &mut |ui: &mut Ui| {
                let half = ui.node("lists".into(), crate::Node { h: crate::ui::Size::Px(80.0), ..crate::Node::DEFAULT });
                ui.within(&half, &mut |ui| { ui.list_view("list", 1, false, &mut list, &mut |ui, _| ui.item_text("list row")); });
                let half = ui.node("trees".into(), crate::Node { h: crate::ui::Size::Px(80.0), ..crate::Node::DEFAULT });
                ui.within(&half, &mut |ui| { ui.tree("tree", &nodes, &mut open, &mut selected, &mut visible, &mut |ui, _| ui.item_text("tree row")); });
                let half = ui.node("tables".into(), crate::Node { h: crate::ui::Size::Px(80.0), ..crate::Node::DEFAULT });
                ui.within(&half, &mut |ui| { ui.table_view("table", &["A"], &mut widths, 1, None, &mut cell, &mut |ui, _, _| ui.item_text("table cell")); });
            }).expect("frame");
        }
        let names: Vec<String> = ui.a11y_nodes().iter().map(|node| ui.a11y_name_of(node).to_owned()).collect();
        assert!(names.iter().any(|name| name == "tree row"), "{names:?}");
        assert!(names.iter().any(|name| name == "table cell"), "{names:?}");
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { let _ = ui.frame(&input, &mut |ui: &mut Ui| ui.item_text("loose")); })).is_err());
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
                        let _ = ui.list_view("Files", ROWS, true, &mut list, &mut |ui, i| { texts.set(texts.get() + 1); ui.item_text(&format!("file {i}")); });
                    }, &mut |ui: &mut Ui| {
                        shown = ui.tree("Outline", &nodes, &mut open, &mut selected, &mut visible, &mut |ui, i| { texts.set(texts.get() + 1); ui.item_text(&format!("node {i}")); }).1;
                        let _ = ui.table_view("Points", &["Name", "Group", "Tag"], &mut widths, ROWS, Some((0, true)), &mut table_sel, &mut |ui, r, c| { cells.set(cells.get() + 1); ui.item_text(&format!("{r}:{c}")); });
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
