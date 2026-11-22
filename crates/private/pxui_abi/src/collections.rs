//! Measured lists require an active frame, count at most 1,000,000, finite positive estimate and
//! finite estimated total. Callbacks use only the current borrowed UI and may repeat as measurement
//! settles. Validate descriptors before mutation and preserve callback recovery, hit/accessibility
//! geometry and bounded warm allocation behavior.
//!
use pxui_types::ffi::{PxuiKeyedDesc, PxuiRowKey};
use crate::state::{PxuiUi, checked, fail, get};
use crate::{PXUI_ERR_ARGUMENT, Str};
use core::{mem::{align_of, offset_of, size_of}};
use pxui_core::ui_widgets::TreeNode;
pub use pxui_types::ffi::{PxuiTreeViewDesc, PxuiF32MutSpan, PxuiTreeNode, PxuiU32MutSpan, PxuiU8MutSpan};

const MAX_ITEMS: usize = 1_000_000;

// The engine's node and the C node share a layout, so a caller's array is the engine's slice with no copy.
const _: () = assert!(size_of::<TreeNode>() == size_of::<PxuiTreeNode>() && align_of::<TreeNode>() == align_of::<PxuiTreeNode>()
    && offset_of!(TreeNode, depth) == offset_of!(PxuiTreeNode, depth) && offset_of!(TreeNode, children) == offset_of!(PxuiTreeNode, children));

fn valid_span<T>(ptr: *mut T, len: usize, required: usize) -> bool {
    len == required && len <= MAX_ITEMS && (len == 0 || !ptr.is_null())
}
fn valid_output_span<T>(ptr: *mut T, len: usize, required: usize) -> bool {
    len >= required && len <= MAX_ITEMS && (len == 0 || !ptr.is_null())
}
// ABI 1.49: the kit's text of the collection item being built, from inside a `selection_list`/`tree` row or a `table` cell callback: a list row's text, a tree row's text naming the row, a table cell's text naming the cell. Refused elsewhere.
export!(trusted pub unsafe fn pxui_ui_item_text(ui: *mut PxuiUi, text: Str) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(text) = (unsafe { checked(text) }) else { return unsafe { fail(ui, "invalid item text") } };
    unsafe { crate::trusted::item_text(ui, text) }
});

export!(trusted pub unsafe fn pxui_ui_tree(ui: *mut PxuiUi, desc: *const PxuiTreeViewDesc, out: *mut usize) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null tree argument") }; }
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid tree descriptor") }; };
    if desc.has_revision > 1 || desc.reserved != [0; 3] || desc.nodes.len > MAX_ITEMS || (desc.nodes.len != 0 && desc.nodes.ptr.is_null()) || !valid_span(desc.open.ptr, desc.open.len, desc.nodes.len)
        || !valid_output_span(desc.visible.ptr, desc.visible.len, desc.nodes.len) || desc.selected.is_null() || desc.visible_len.is_null() {
        return unsafe { fail(ui, "invalid tree descriptor") };
    }
    let Some(label) = (unsafe { crate::state::checked_key(desc.label) }) else { return unsafe { fail(ui, "invalid tree label") }; };
    let raw_nodes = if desc.nodes.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(desc.nodes.ptr, desc.nodes.len) } };
    let open: &mut [u8] = if desc.open.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.open.ptr, desc.open.len) } };
    let mut selected = (unsafe { *desc.selected } != usize::MAX).then_some(unsafe { *desc.selected });
    let visible: &mut [u32] = if desc.visible.len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.visible.ptr, desc.visible.len) } };
    let mut result = (None, 0);
    let mut row = unsafe { crate::state::row_handle(ui) };
    let rc = unsafe { crate::trusted::tree(ui, label, raw_nodes, open, &mut selected, visible, (desc.has_revision != 0).then_some(desc.revision), &mut |engine: &mut pxui_core::ui::Ui, node| if let Some(build) = desc.build { debug_assert!(core::ptr::eq(engine, row.engine)); let h: *mut PxuiUi = &mut row; let _current = crate::state::Current::set(row.shared, h); build(desc.user, h.cast(), node, 0) }, &mut result) };
    if rc != 0 { return rc; }
    unsafe { *desc.visible_len = result.1; *desc.selected = selected.unwrap_or(usize::MAX); *out = result.0.unwrap_or(usize::MAX); }
    0
});

// ABI 1.87 (U2b.1): one export a widget, the record checked in one pass before any effect, the whole result in one library-written
// record. A non-empty keyed name names the list (the table's frame).

/// Names the box `handle` (when live) `name`.
unsafe fn name_box(ui: *mut PxuiUi, handle: &crate::PxuiBox, name: &str) {
    if name.is_empty() { return; }
    if let Ok((engine, b)) = unsafe { crate::state::boxed(ui, handle) } { engine.a11y_name(&b, name); }
}

export!(trusted pub unsafe fn pxui_ui_section_list(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, desc: *const pxui_types::ffi::PxuiSectionListDesc, key: PxuiRowKey, build: pxui_types::ffi::PxuiListCell, user: *mut core::ffi::c_void, out: *mut pxui_types::ffi::PxuiListResult) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "invalid list result") } };
    let Some((label, name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid section list descriptor") } };
    let ids = key.map(|key| move |row| key(user, row));
    let mut row = unsafe { crate::state::row_handle(ui) };
    let rc = unsafe { crate::trusted::section_list(ui, label, desc, ids.as_ref().map(|ids| ids as &dyn Fn(usize) -> i64), &mut |engine: &mut pxui_core::ui::Ui, index, c| if let Some(build) = build { debug_assert!(core::ptr::eq(engine, row.engine)); let h: *mut PxuiUi = &mut row; let _current = crate::state::Current::set(row.shared, h); build(user, h.cast(), index, crate::trusted::cell_state(c)) }, out) };
    if rc == 0 { unsafe { name_box(ui, &out.handle, name) }; }
    rc
});

export!(trusted pub unsafe fn pxui_ui_selection_list(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, desc: *const pxui_types::ffi::PxuiSelectionListDesc, key: PxuiRowKey, cell: pxui_types::ffi::PxuiListCell, user: *mut core::ffi::c_void, out: *mut pxui_types::ffi::PxuiListResult) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "invalid list result") } };
    let Some((label, name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid selection list descriptor") } };
    let drop_look = match desc.has_drop_look { 0 => None, 1 => crate::style::decode_look(desc.drop_look), _ => None };
    if desc.has_drop_look > 1 || (desc.has_drop_look == 1 && drop_look.is_none()) { enter!(ui); return unsafe { fail(ui, "invalid selection list descriptor") } }
    let ids = key.map(|key| move |row| key(user, row));
    let mut row = unsafe { crate::state::row_handle(ui) };
    let rc = with_variant!(ui, pxui_types::theme::BodyKind::selection_list, unsafe { (*keyed).variant }, unsafe { crate::trusted::selection_list(ui, label, desc, drop_look, ids.as_ref().map(|ids| ids as &dyn Fn(usize) -> i64), &mut |engine: &mut pxui_core::ui::Ui, index, c| if let Some(cell) = cell { debug_assert!(core::ptr::eq(engine, row.engine)); let h: *mut PxuiUi = &mut row; let _current = crate::state::Current::set(row.shared, h); cell(user, h.cast(), index, crate::trusted::cell_state(c)) }, out) });
    if rc == 0 { unsafe { name_box(ui, &out.handle, name) }; }
    rc
});

export!(trusted pub unsafe fn pxui_ui_table_head(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, desc: *const pxui_types::ffi::PxuiTableHeadDesc, out: *mut pxui_types::ffi::PxuiHeadResult) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "invalid heading result") } };
    let Some((label, _)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid table heading descriptor") } };
    with_variant!(ui, pxui_types::theme::BodyKind::table_head, unsafe { (*keyed).variant }, unsafe { crate::trusted::table_head(ui, label, desc, out) })
});

export!(trusted pub unsafe fn pxui_ui_table(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, desc: *const pxui_types::ffi::PxuiTableDesc, key: PxuiRowKey, cell: pxui_types::ffi::PxuiTableCellBuild, user: *mut core::ffi::c_void, out: *mut pxui_types::ffi::PxuiTableResult) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "invalid table result") } };
    let Some((label, name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid table descriptor") } };
    let ids = key.map(|key| move |row| key(user, row));
    let mut handle = unsafe { crate::state::row_handle(ui) };
    let rc = with_variant!(ui, pxui_types::theme::BodyKind::table_row, unsafe { (*keyed).variant }, unsafe { crate::trusted::table(ui, label, desc, ids.as_ref().map(|ids| ids as &dyn Fn(usize) -> i64), &mut |engine: &mut pxui_core::ui::Ui, row, column, c| if let Some(cell) = cell { debug_assert!(core::ptr::eq(engine, handle.engine)); let h: *mut PxuiUi = &mut handle; let _current = crate::state::Current::set(handle.shared, h); cell(user, h.cast(), row, column, crate::trusted::cell_state(c)) }, out) });
    if rc == 0 && !name.is_empty() { if let Some(engine) = unsafe { crate::state::engine(ui) } { crate::trusted::name_table(engine, label, name); } }
    rc
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FrameIn, FrameOut, pxui_frame, pxui_last_error, pxui_ui_destroy};
    use core::ffi::c_void;

    /// `f` run in one frame of `ui` (its return code).
    fn in_frame(ui: *mut PxuiUi, f: &mut dyn FnMut(*mut PxuiUi) -> i32) -> i32 {
        extern "C" fn build(user: *mut c_void, ui: *mut c_void) {
            let (f, rc) = unsafe { &mut *user.cast::<(&mut dyn FnMut(*mut PxuiUi) -> i32, i32)>() };
            *rc = f(ui.cast());
        }
        let mut state = (f, i32::MIN);
        let mut out = FrameOut::default();
        let input = FrameIn::new(320.0, 240.0);
        assert_eq!(unsafe { pxui_frame(ui, &input, Some(build), (&mut state as *mut (&mut dyn FnMut(*mut PxuiUi) -> i32, i32)).cast(), &mut out) }, 0);
        state.1
    }
    fn error(ui: *mut PxuiUi) -> String { let e = unsafe { pxui_last_error(ui) }; String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(e.ptr, e.len) }).into_owned() }

    /// Review finding 14: a kept tree is a trust boundary. A C caller's `has_revision` past 1 and non-zero `reserved` bytes are
    /// refused before any effect; a caller that breaks its revision promise (writes `visible` past the nodes under the same revision)
    /// gets its rows re-derived, never a panic or an error.
    #[test]
    fn a_kept_tree_refuses_bad_flags_and_re_derives_a_broken_promise() {
        let ui = unsafe { crate::ui::create_test_ui() };
        // ten roots, each with one child; root 0 open: 11 rows shown
        let nodes: Vec<PxuiTreeNode> = (0..20).map(|i| PxuiTreeNode { depth: (i % 2) as u16, children: (i % 2 == 0) as u8 }).collect();
        let (mut open, mut visible, mut selected, mut shown) = (vec![0u8; 20], vec![0u32; 20], usize::MAX, 0usize);
        open[0] = 1;
        let label = { let mut record = Str::default(); record.ptr = b"tree".as_ptr(); record.len = 4; record };
        let call = |ui: *mut PxuiUi, has_revision: u8, reserved: [u8; 3], open: &mut [u8], visible: &mut [u32], selected: &mut usize, shown: &mut usize| {
            let desc = PxuiTreeViewDesc { size: core::mem::size_of::<PxuiTreeViewDesc>() as u32, has_revision, reserved, label, nodes: pxui_types::ffi::Slice { ptr: nodes.as_ptr(), len: nodes.len() },
                open: PxuiU8MutSpan { ptr: open.as_mut_ptr(), len: open.len() }, selected, visible: PxuiU32MutSpan { ptr: visible.as_mut_ptr(), len: visible.len() }, visible_len: shown,
                build: None, user: core::ptr::null_mut(), revision: 7 };
            let mut out = usize::MAX;
            unsafe { pxui_ui_tree(ui, &desc, &mut out) }
        };
        for (has_revision, reserved) in [(2u8, [0u8; 3]), (255, [0; 3]), (1, [1, 0, 0]), (0, [0, 0, 9])] {
            *visible.first_mut().unwrap() = 42;
            let rc = in_frame(ui, &mut |ui| call(ui, has_revision, reserved, &mut open, &mut visible, &mut selected, &mut shown));
            assert_eq!(rc, crate::PXUI_ERR_ARGUMENT, "has_revision {has_revision}, reserved {reserved:?}");
            assert!(error(ui).contains("invalid tree descriptor"), "{}", error(ui));
            assert_eq!(visible[0], 42, "a refused tree writes nothing");
        }
        // kept rows, then the promise broken: rows past the nodes written under the same revision
        // frame 2 breaks the keyboard's row (read before the rows are built), frame 4 only a row in view (read while building them)
        for frame in 0..6 {
            if frame == 2 { visible[..11].fill(1_000_000); }
            if frame == 4 { visible[5] = 1_000_000; }
            let rc = in_frame(ui, &mut |ui| call(ui, 1, [0; 3], &mut open, &mut visible, &mut selected, &mut shown));
            assert_eq!(rc, 0, "frame {frame}: {}", error(ui));
            assert_eq!(shown, 11, "frame {frame}");
            assert_eq!((&visible[..3], visible[5]), (&[0, 1, 2][..], 8), "frame {frame}: the rows re-derived");
        }
        assert!(!unsafe { (*(*ui).shared).poisoned });
        unsafe { pxui_ui_destroy(ui); }
    }
}
