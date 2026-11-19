use crate::{check, Ui};
use pxui_types::ui::{Key, ListResponse, SectionPart, SelectionList, TreeResponse};
#[cfg(not(feature = "static"))]
use pxui_types::ffi::PxuiCellState;
use pxui_types::ffi::{PxuiColumn, PxuiColumnWidth, PxuiColumnWidthSpan, PxuiHeadResult, PxuiListResult, PxuiResponse, PxuiSelectionListDesc, PxuiTableHeadDesc, PxuiTableDesc, PxuiTableResult, PxuiU8MutSpan, Slice};
use pxui_types::theme::BodyKind;
use crate::author::{CellState, Column, HeadResponse, ListState, Sort, Table, TableHead, TableResponse, TableState};
#[cfg(not(feature = "static"))]
use pxui_types::Str;

/// A node of a preorder tree: its depth and whether it has children (not 0). The layout is the library's, so a slice of
/// these crosses the ABI as it is.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct TreeNode {
    /// Depth in the preorder tree, zero at the root.
    pub depth: u16,
    /// Whether the row has child rows, stored as one or zero.
    pub children: u8,
}

impl TreeNode {
    /// Declares a row in a preorder tree.
    ///
    /// Depth is relative to the root row. The tree calls its cell builder for the displayed content.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const ROW: pxui::TreeNode = pxui::TreeNode::new(0, true);
    /// ```
    pub const fn new(depth: u16, has_children: bool) -> TreeNode { TreeNode { depth, children: has_children as u8 } }
}

use pxui_types::ffi::PxuiTreeNode as TreeNodeDesc;
const _: () = assert!(core::mem::size_of::<TreeNode>() == core::mem::size_of::<TreeNodeDesc>() && core::mem::align_of::<TreeNode>() == core::mem::align_of::<TreeNodeDesc>());

// The C-callback side of the collections (the dynamic build): trampolines the library calls for a row's text.
#[cfg(not(feature = "static"))]
mod link {
    use super::*;
    use crate::composite::{defer_callback_panic, deferred};
    use core::{ffi::c_void, panic::AssertUnwindSafe};
    use pxui_types::{Str, ffi::{PxuiTreeViewDesc, PxuiU32MutSpan as U32MutSpan, PxuiU8MutSpan as U8MutSpan}};

    #[link(name = "pxui")]
    unsafe extern "C" {
        fn pxui_ui_tree(ui: *mut crate::sys::PxuiUi, desc: *const PxuiTreeViewDesc, out: *mut usize) -> i32;
        fn pxui_ui_section_list(ui: *mut crate::sys::PxuiUi, keyed: *const pxui_types::ffi::PxuiKeyedDesc, desc: *const pxui_types::ffi::PxuiSectionListDesc, key: pxui_types::ffi::PxuiRowKey, build: pxui_types::ffi::PxuiListCell, user: *mut c_void, out: *mut PxuiListResult) -> i32;
        fn pxui_ui_selection_list(ui: *mut crate::sys::PxuiUi, keyed: *const pxui_types::ffi::PxuiKeyedDesc, desc: *const PxuiSelectionListDesc, key: pxui_types::ffi::PxuiRowKey, cell: pxui_types::ffi::PxuiListCell, user: *mut c_void, out: *mut PxuiListResult) -> i32;
        fn pxui_ui_table_head(ui: *mut crate::sys::PxuiUi, keyed: *const pxui_types::ffi::PxuiKeyedDesc, desc: *const PxuiTableHeadDesc, out: *mut PxuiHeadResult) -> i32;
        fn pxui_ui_table(ui: *mut crate::sys::PxuiUi, keyed: *const pxui_types::ffi::PxuiKeyedDesc, desc: *const PxuiTableDesc, key: pxui_types::ffi::PxuiRowKey, cell: pxui_types::ffi::PxuiTableCellBuild, user: *mut c_void, out: *mut PxuiTableResult) -> i32;
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

    /// The section and selection list context: the part builder and the row ids, behind the one `user` the call carries.
    pub(super) struct ListRows<'a> { pub cell: &'a mut dyn FnMut(&mut Ui, SectionPart, CellState), pub id: &'a dyn Fn(usize) -> i64 }
    pub(super) extern "C" fn call_list_cell(user: *mut c_void, raw: *mut c_void, index: usize, cell: *const PxuiCellState) {
        if raw.is_null() || cell.is_null() || deferred() { return; }
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let mut ui = core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new(raw.cast()).unwrap()));
            (unsafe { &mut *user.cast::<ListRows>() }.cell)(&mut ui, part_of(index), cell_of(unsafe { &*cell }));
        }));
        if let Err(panic) = result { defer_callback_panic(panic); }
    }
    pub(super) extern "C" fn call_list_id(user: *mut c_void, row: usize) -> i64 {
        if deferred() { return row as i64; }
        let rows = unsafe { &*user.cast::<ListRows>() };
        match std::panic::catch_unwind(AssertUnwindSafe(|| (rows.id)(row))) { Ok(id) => id, Err(panic) => { defer_callback_panic(panic); row as i64 } }
    }

    /// The table context: the cell builder and the row ids.
    pub(super) struct TableRows<'a> { pub cell: &'a mut dyn FnMut(&mut Ui, usize, usize, CellState), pub id: &'a dyn Fn(usize) -> i64 }
    pub(super) extern "C" fn call_table_cell(user: *mut c_void, raw: *mut c_void, row: usize, column: usize, cell: *const PxuiCellState) {
        if raw.is_null() || cell.is_null() || deferred() { return; }
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let mut ui = core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new(raw.cast()).unwrap()));
            (unsafe { &mut *user.cast::<TableRows>() }.cell)(&mut ui, row, column, cell_of(unsafe { &*cell }));
        }));
        if let Err(panic) = result { defer_callback_panic(panic); }
    }
    pub(super) extern "C" fn call_table_id(user: *mut c_void, row: usize) -> i64 {
        if deferred() { return row as i64; }
        let rows = unsafe { &*user.cast::<TableRows>() };
        match std::panic::catch_unwind(AssertUnwindSafe(|| (rows.id)(row))) { Ok(id) => id, Err(panic) => { defer_callback_panic(panic); row as i64 } }
    }

    impl Ui {
        /// `section_list`'s C route: the part builder and the row ids as a selection list's (`ListRows`), one trampoline for both.
        pub(crate) fn section_list_c(&mut self, label: &str, desc: &pxui_types::ffi::PxuiSectionListDesc, id: &dyn Fn(usize) -> i64, cell: &mut dyn FnMut(&mut Ui, SectionPart, CellState), out: &mut PxuiListResult) {
            let mut rows = ListRows { cell, id };
            let rc = unsafe { pxui_ui_section_list(self.0.as_ptr(), &pxui_types::ffi::PxuiKeyedDesc::new(label, ""), desc, Some(call_list_id), Some(call_list_cell), (&mut rows as *mut ListRows).cast(), out) };
            crate::composite::resume_deferred();
            check(self, rc);
        }
        pub(super) fn selection_list_c(&mut self, label: &str, desc: &PxuiSelectionListDesc, rows: &mut ListRows, out: &mut PxuiListResult) {
            let rc = unsafe { pxui_ui_selection_list(self.0.as_ptr(), &pxui_types::ffi::PxuiKeyedDesc::new(label, ""), desc, Some(call_list_id), Some(call_list_cell), (rows as *mut ListRows).cast(), out) };
            crate::composite::resume_deferred();
            check(self, rc);
        }
        pub(super) fn table_c(&mut self, label: &str, desc: &PxuiTableDesc, rows: &mut TableRows, out: &mut PxuiTableResult) {
            let rc = unsafe { pxui_ui_table(self.0.as_ptr(), &pxui_types::ffi::PxuiKeyedDesc::new(label, ""), desc, Some(call_table_id), Some(call_table_cell), (rows as *mut TableRows).cast(), out) };
            crate::composite::resume_deferred();
            check(self, rc);
        }
        pub(super) fn table_head_c(&mut self, label: &str, desc: &PxuiTableHeadDesc, out: &mut PxuiHeadResult) {
            let rc = unsafe { pxui_ui_table_head(self.0.as_ptr(), &pxui_types::ffi::PxuiKeyedDesc::new(label, ""), desc, out) };
            check(self, rc);
        }
    }

    #[inline]
    fn as_str(value: &str) -> Str { crate::as_str(value) }

    impl Ui {
        pub(super) fn tree_c(&mut self, label: &str, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], mut cell: Cell) -> (Option<usize>, usize) {
            let mut rows_len = 0usize;
            let mut selected_raw = selected.unwrap_or(usize::MAX);
            let desc = PxuiTreeViewDesc {
                size: core::mem::size_of::<PxuiTreeViewDesc>() as u32, reserved: 0, label: as_str(label), nodes: Slice { ptr: nodes.as_ptr().cast::<TreeNodeDesc>(), len: nodes.len() },
                open: U8MutSpan { ptr: open.as_mut_ptr(), len: open.len() }, selected: &mut selected_raw,
                visible: U32MutSpan { ptr: visible.as_mut_ptr(), len: visible.len() }, visible_len: &mut rows_len,
                build: Some(call_cell), user: (&mut cell as *mut Cell).cast(),
            };
            let mut out = usize::MAX;
            let rc = unsafe { pxui_ui_tree(self.0.as_ptr(), &desc, &mut out) };
            crate::composite::resume_deferred();
            check(self, rc);
            *selected = (selected_raw != usize::MAX).then_some(selected_raw);
            ((out != usize::MAX).then_some(out), rows_len)
        }
    }
}

/// The most columns a table or heading takes.
const MAX_COLUMNS: usize = 64;
const COLUMN_EMPTY: PxuiColumn = PxuiColumn { size: core::mem::size_of::<PxuiColumn>() as u32, sortable: 0, grip: 0, has_id: 0, reserved: 0, title: pxui_types::Str { ptr: core::ptr::null(), len: 0 }, id: 0, share: 0.0, w: 0.0 };
const HEAD_EMPTY: PxuiHeadResult = PxuiHeadResult { size: core::mem::size_of::<PxuiHeadResult>() as u32, sorted: 0, resized: 0, has_sort: 0, ascending: 0, sort_id: 0, response: PxuiResponse::EMPTY };

/// A build index as the list part it names.
pub(crate) fn part_of(index: usize) -> SectionPart {
    match index { usize::MAX => SectionPart::Leading, i if i == usize::MAX - 1 => SectionPart::Header, i if i == usize::MAX - 2 => SectionPart::Trailing, i => SectionPart::Row(i) }
}

/// The library's cell record as the row's `CellState`.
/// The engine's row record as the wrapper's: the same bytes (`#[repr(C)]` both, checked below), read as they are.
#[cfg(feature = "static")]
#[inline(always)]
pub(crate) fn cell_from(c: &pxui_abi_link::trusted::ListCell) -> CellState { unsafe { *(c as *const pxui_abi_link::trusted::ListCell).cast::<CellState>() } }
#[cfg(feature = "static")]
const _: () = {
    use core::mem::{align_of, offset_of, size_of};
    use pxui_abi_link::trusted::ListCell as L;
    assert!(size_of::<CellState>() == size_of::<L>() && align_of::<CellState>() == align_of::<L>());
    assert!(offset_of!(CellState, selected) == offset_of!(L, selected) && offset_of!(CellState, enabled) == offset_of!(L, enabled) && offset_of!(CellState, focused) == offset_of!(L, focused) && offset_of!(CellState, held) == offset_of!(L, held) && offset_of!(CellState, y) == offset_of!(L, y));
    assert!(offset_of!(CellState, handle.index) == offset_of!(L, handle.index) && offset_of!(CellState, handle.box_key) == offset_of!(L, handle.box_key) && offset_of!(CellState, handle.box_slot) == offset_of!(L, handle.box_slot) && offset_of!(CellState, handle.generation) == offset_of!(L, handle.generation));
    assert!(size_of::<usize>() == size_of::<u64>());
};

#[cfg(not(feature = "static"))]
fn cell_of(c: &PxuiCellState) -> CellState {
    CellState { selected: c.selected != 0, enabled: c.enabled != 0, focused: c.focused != 0, held: c.held != 0, handle: c.handle, y: c.y }
}

/// The library's list record as a `ListResponse`.
pub(crate) fn list_response(out: &PxuiListResult) -> ListResponse {
    let at = |i: usize| (i != usize::MAX).then_some(i);
    let row = (out.activated != usize::MAX || out.secondary != usize::MAX).then_some(out.row);
    let moved = (out.moved_from != usize::MAX && out.moved_to != usize::MAX).then_some((out.moved_from, out.moved_to));
    ListResponse { handle: out.handle, activated: at(out.activated), selected: at(out.selected), secondary: at(out.secondary), row, moved, drop_at: at(out.drop_at), focused: out.focused != 0 }
}

/// The columns as C records (on the stack) and `state`'s widths by id, one a column (more than 64 columns panic: the library refuses them).
fn column_records(columns: &[Column<'_>], state: &mut TableState) -> ([PxuiColumn; MAX_COLUMNS], [PxuiColumnWidth; MAX_COLUMNS]) {
    assert!(columns.len() <= MAX_COLUMNS, "pxui: a table takes at most 64 columns");
    let (mut specs, mut widths) = ([COLUMN_EMPTY; MAX_COLUMNS], [PxuiColumnWidth::default(); MAX_COLUMNS]);
    for (i, c) in columns.iter().enumerate() {
        specs[i] = PxuiColumn { sortable: c.sortable as u8, grip: c.grip as u8, has_id: c.id.is_some() as u8, title: crate::as_str(c.title), id: c.id.unwrap_or(0), share: c.share, w: c.w, ..COLUMN_EMPTY };
    }
    state.widths.resize(columns.len(), (0, 0.0));
    for (w, s) in widths.iter_mut().zip(&state.widths) { *w = PxuiColumnWidth { id: s.0, w: s.1, reserved: 0 }; }
    (specs, widths)
}

/// `TableState::sort` as the records' fields.
fn sort_fields(sort: Option<(i64, Sort)>) -> (u8, u8, i64) {
    sort.map_or((0, 0, 0), |(id, s)| (1, (s == Sort::Ascending) as u8, id))
}

/// The heading's record into `state` (widths, sort) and the `HeadResponse`.
fn head_response(out: &PxuiHeadResult, widths: &[PxuiColumnWidth], state: &mut TableState) -> HeadResponse {
    for (s, w) in state.widths.iter_mut().zip(widths) { *s = (w.id, w.w); }
    state.sort = (out.has_sort != 0).then_some((out.sort_id, if out.ascending != 0 { Sort::Ascending } else { Sort::Descending }));
    HeadResponse { response: crate::kit_ext::response(&out.response), sorted: out.sorted != 0, resized: out.resized != 0 }
}

impl Ui {
    /// The one body of `selection_list`: keyed by `key`; the list record, `state`'s flags (sized to `count`) and anchor cross in one
    /// call (`pxui_ui_selection_list`), the whole `ListResponse` comes back in one record.
    #[track_caller]
    pub(crate) fn selection_list_key(&mut self, key: Key, count: usize, row_key: &dyn Fn(usize) -> i64, state: &mut ListState, list: &SelectionList, cell: &mut dyn FnMut(&mut Ui, SectionPart, CellState)) -> ListResponse {
        // shrink then grow: truncating first means rows that come back start unflagged (no stale SELECTED); capacity is kept
        state.rows.resize(count, 0);
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        // linked statically the drop look goes to the library as it is, never encoded
        #[cfg(feature = "static")]
        let drop_look = pxui_types::ffi::PxuiLookDesc::default();
        #[cfg(not(feature = "static"))]
        let drop_look = crate::node::look(list.drop_look.unwrap_or(crate::theme::Look::NONE));
        let desc = PxuiSelectionListDesc { size: core::mem::size_of::<PxuiSelectionListDesc>() as u32, multi: list.multi as u8, fit: list.fit as u8, open_on_click: list.open_on_click as u8,
            overlay_scrollbar: list.overlay_scrollbar as u8, measured: list.measured as u8, reorder: list.reorder as u8, has_drop_look: list.drop_look.is_some() as u8, bare: list.bare as u8, has_revision: state.revision.is_some() as u8, reserved: [0; 3], count, row_height: list.row_height, leading_height: list.leading_height,
            header_height: list.header_height, trailing_height: list.trailing_height, rows: PxuiU8MutSpan { ptr: state.rows.as_mut_ptr(), len: count }, anchor: state.anchor.unwrap_or(usize::MAX),
            drop_look, revision: state.revision.unwrap_or(0) };
        let mut out = PxuiListResult::EMPTY;
        if list.variant != 0 { self.set_variant(BodyKind::selection_list, list.variant); }
        #[cfg(feature = "static")]
        let outer = self.0.as_ptr();
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::selection_list(outer, label, &desc, list.drop_look, Some(row_key), &mut |engine: &mut pxui_abi_link::trusted::Engine, index, c| {
            let mut slot = None;
            let handle = pxui_abi_link::trusted::Borrowed::handle(engine, outer, &mut slot);
            cell(&mut *core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new_unchecked(handle))), part_of(index), cell_from(c))
        }, &mut out) });
        #[cfg(not(feature = "static"))]
        self.selection_list_c(label, &desc, &mut link::ListRows { cell, id: row_key }, &mut out);
        if list.variant != 0 { self.set_variant(BodyKind::selection_list, 0); }
        state.anchor = (out.anchor != usize::MAX).then_some(out.anchor);
        list_response(&out)
    }

    /// The one body of `tree`: keyed by `key` (the export's label is identity only).
    #[allow(clippy::too_many_arguments)]
    #[track_caller]
    pub(crate) fn tree_key(&mut self, key: Key, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], cell: &mut dyn FnMut(&mut Ui, usize)) -> TreeResponse {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        #[cfg(feature = "static")]
        let (activated, shown) = {
            let mut out = (None, 0);
            let nodes = unsafe { core::slice::from_raw_parts(nodes.as_ptr().cast::<TreeNodeDesc>(), nodes.len()) };
            let outer = self.0.as_ptr();
            check(self, unsafe { pxui_abi_link::trusted::tree(outer, label, nodes, open, selected, visible, &mut |engine: &mut pxui_abi_link::trusted::Engine, node| {
                let mut slot = None;
                let handle = pxui_abi_link::trusted::Borrowed::handle(engine, outer, &mut slot);
                (&mut *cell)(&mut *core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new_unchecked(handle))), node)
            }, &mut out) });
            out
        };
        #[cfg(not(feature = "static"))]
        let (activated, shown) = self.tree_c(label, nodes, open, selected, visible, &mut |ui, node, _| cell(ui, node));
        TreeResponse { activated, shown }
    }

    /// The one body of `table`: keyed by `key`; the columns, `state` (widths, sort, flags, anchor) and the geometry cross in one call
    /// (`pxui_ui_table`), both results come back in one record.
    #[allow(clippy::too_many_arguments)]
    #[track_caller]
    pub(crate) fn table_key(&mut self, key: Key, columns: &[Column<'_>], rows: usize, row_key: &dyn Fn(usize) -> i64, state: &mut TableState, table: &Table, cell: &mut dyn FnMut(&mut Ui, usize, usize, CellState)) -> TableResponse {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let (mut specs, mut widths) = column_records(columns, state);
        state.selection.rows.resize(rows, 0);
        let n = columns.len();
        let (has_sort, ascending, sort_id) = sort_fields(state.sort);
        let desc = PxuiTableDesc { size: core::mem::size_of::<PxuiTableDesc>() as u32, multi: table.multi as u8, has_sort, ascending, has_revision: state.selection.revision.is_some() as u8,
            columns: Slice { ptr: specs.as_mut_ptr(), len: n }, widths: PxuiColumnWidthSpan { ptr: widths.as_mut_ptr(), len: n }, sort_id, rows, row_h: table.row_h, head_h: table.head.h,
            flags: PxuiU8MutSpan { ptr: state.selection.rows.as_mut_ptr(), len: rows }, anchor: state.selection.anchor.unwrap_or(usize::MAX), head_variant: table.head.variant, reserved2: [0; 6], revision: state.selection.revision.unwrap_or(0) };
        let mut out = PxuiTableResult { size: core::mem::size_of::<PxuiTableResult>() as u32, reserved: 0, rows: PxuiListResult::EMPTY, head: HEAD_EMPTY };
        if table.variant != 0 { self.set_variant(BodyKind::table_row, table.variant); }
        #[cfg(feature = "static")]
        let outer = self.0.as_ptr();
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::table(outer, label, &desc, Some(row_key), &mut |engine: &mut pxui_abi_link::trusted::Engine, row, column, c| {
            let mut slot = None;
            let handle = pxui_abi_link::trusted::Borrowed::handle(engine, outer, &mut slot);
            cell(&mut *core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new_unchecked(handle))), row, column, cell_from(c))
        }, &mut out) });
        #[cfg(not(feature = "static"))]
        self.table_c(label, &desc, &mut link::TableRows { cell, id: row_key }, &mut out);
        if table.variant != 0 { self.set_variant(BodyKind::table_row, 0); }
        state.selection.anchor = (out.rows.anchor != usize::MAX).then_some(out.rows.anchor);
        let head = head_response(&out.head, &widths[..n], state);
        TableResponse { rows: list_response(&out.rows), head }
    }

    /// The one body of `table_head`: keyed by `key`; the columns and `state`'s widths and sort cross in one call (`pxui_ui_table_head`).
    #[track_caller]
    pub(crate) fn table_head_key(&mut self, key: Key, columns: &[Column<'_>], state: &mut TableState, head: &TableHead) -> HeadResponse {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let (mut specs, mut widths) = column_records(columns, state);
        let n = columns.len();
        let (has_sort, ascending, sort_id) = sort_fields(state.sort);
        let desc = PxuiTableHeadDesc { size: core::mem::size_of::<PxuiTableHeadDesc>() as u32, has_sort, ascending, reserved: [0; 2], columns: Slice { ptr: specs.as_mut_ptr(), len: n },
            widths: PxuiColumnWidthSpan { ptr: widths.as_mut_ptr(), len: n }, sort_id, h: head.h, reserved2: 0 };
        let mut out = HEAD_EMPTY;
        if head.variant != 0 { self.set_variant(BodyKind::table_head, head.variant); }
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::table_head(self.0.as_ptr(), label, &desc, &mut out) });
        #[cfg(not(feature = "static"))]
        self.table_head_c(label, &desc, &mut out);
        if head.variant != 0 { self.set_variant(BodyKind::table_head, 0); }
        head_response(&out, &widths[..n], state)
    }

    /// The kit's text of the collection item `cell` is building: a `selection_list` row's label, a `tree` row's (naming the row) or a
    /// `table` cell's (naming the cell), in the item's look, through its row body.
    ///
    /// Call it from a `selection_list`, `tree` or `table` cell callback; the text is the item's accessible name where it names one.
    /// Allocates: never on a warm frame (text it has not shaped before is shaped once).
    /// Panics: outside a collection item, on invalid text, or when the `Ui` has failed. Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::ui::SectionPart;
    /// let mut state = pxui::ListState::new();
    /// pxui::doc::frames(1, &mut |ui| { ui.selection_list("files", 2, &|i| i as i64, &mut state, &pxui::SelectionList::DEFAULT, &mut |ui, part, _| if let SectionPart::Row(row) = part { ui.item_text(["notes.txt", "song.flac"][row]) }); });
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
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.set_a11y(true);
        let nodes = [TreeNode { depth: 0, children: 0 }];
        let (mut list, mut open, mut selected, mut visible, mut table) = (crate::ListState::new(), [0u8; 1], None, [0u32; 1], crate::TableState::new());
        let input = crate::FrameIn::new(400.0, 300.0);
        for _ in 0..2 {
            ui.frame(&input, &mut |ui: &mut Ui| {
                let half = ui.node("lists", crate::Node { h: crate::ui::Size::Px(80.0), ..crate::Node::DEFAULT });
                ui.within(&half, &mut |ui| { ui.selection_list("list", 1, &|i| i as i64, &mut list, &crate::SelectionList::DEFAULT, &mut |ui, part, _| if let crate::ui::SectionPart::Row(_) = part { ui.item_text("list row") }); });
                let half = ui.node("trees", crate::Node { h: crate::ui::Size::Px(80.0), ..crate::Node::DEFAULT });
                ui.within(&half, &mut |ui| { ui.tree("tree", &nodes, &mut open, &mut selected, &mut visible, &mut |ui, _| ui.item_text("tree row")); });
                let half = ui.node("tables", crate::Node { h: crate::ui::Size::Px(80.0), ..crate::Node::DEFAULT });
                ui.within(&half, &mut |ui| { ui.table("table", &[crate::Column::new("A").w_px(80.0)], 1, &|i| i as i64, &mut table, &crate::Table::DEFAULT, &mut |ui, _, _, _| ui.item_text("table cell")); });
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
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        let mut list = crate::ListState::new();
        let nodes: Vec<TreeNode> = (0..ROWS).map(|i| TreeNode { depth: [0, 1, 2, 2, 1, 2][i % 6], children: matches!(i % 6, 0 | 1 | 4) as u8 }).collect();
        let mut open: Vec<u8> = (0..ROWS).map(|i| (i < 6) as u8).collect();
        let (mut selected, mut visible, mut table) = (Some(2usize), vec![0u32; ROWS], crate::TableState::new());
        table.sort = Some((0, crate::Sort::Ascending));
        const COLUMNS: [crate::Column; 3] = [crate::Column::new("Name").sortable(true).w_px(96.0), crate::Column::new("Group").w_px(72.0), crate::Column::new("Tag").w_px(72.0)];
        let (texts, cells, mut shown) = (core::cell::Cell::new(0usize), core::cell::Cell::new(0usize), 0);
        for count in 0..3 {
            let input = crate::FrameIn::new(640.0, 464.0).time(count as f64 / 60.0);
            ui.frame(&input, &mut |ui: &mut Ui| {
                ui.view_of("page", &count, |ui: &mut Ui| {
                    let ratio = ui.split("split", 0.34, &crate::Split::DEFAULT.min_a(120.0).min_b(200.0), &mut |ui: &mut Ui| {
                        let _ = ui.selection_list("files", ROWS, &|i| i as i64, &mut list, &crate::SelectionList::DEFAULT.multi(), &mut |ui, part, _| if let crate::ui::SectionPart::Row(i) = part { texts.set(texts.get() + 1); ui.item_text(&format!("file {i}")); });
                    }, &mut |ui: &mut Ui| {
                        shown = ui.tree("outline", &nodes, &mut open, &mut selected, &mut visible, &mut |ui, i| { texts.set(texts.get() + 1); ui.item_text(&format!("node {i}")); }).shown;
                        let _ = ui.table("points", &COLUMNS, ROWS, &|i| i as i64, &mut table, &crate::Table::DEFAULT, &mut |ui, r, c, _| { cells.set(cells.get() + 1); ui.item_text(&format!("{r}:{c}")); });
                    });
                    assert!(ratio.ratio.is_none());
                });
            }).expect("frame");
        }
        assert!(texts.get() > 0 && cells.get() > 0, "the virtual rows were asked for their text");
        assert!(shown > 0 && shown <= nodes.len(), "{shown}");
        assert_eq!(visible[0], 0);
    }
}
