use crate::node::size;
use crate::state::{Build, PxuiUi, checked, fail, get, strings};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox};
use core::ffi::c_void;
use pxui_types::ffi::{PxuiAnchorDesc, PxuiDialogDesc, PxuiDialogResult, PxuiSizeDesc};
use pxui_types::ui::{Dialog, DialogButton, DismissOpts, Chosen, Overlay, Presentation};
use pxui_types::ffi::PxuiKeyedDesc;
use pxui_types::Node;

const MAX_BUTTONS: usize = 64;

/// The buttons of `texts` with the flags and the details a dialog record carries (`PxuiDialogDesc`; `details` as many as `texts` or none):
/// `buf` holds them for the call.
fn dialog_buttons<'a, 'b>(texts: &[&'a str], details: &[&'a str], (default_at, cancel_at, danger, disabled): (i64, i64, u64, u64), buf: &'b mut [DialogButton<'a>; MAX_BUTTONS]) -> Option<&'b [DialogButton<'a>]> {
    let n = texts.len();
    let beyond = |mask: u64| n < MAX_BUTTONS && mask >> n != 0;
    if n > MAX_BUTTONS || !(-1..n as i64).contains(&default_at) || !(-1..n as i64).contains(&cancel_at) || beyond(danger) || beyond(disabled) { return None; }
    if !details.is_empty() && details.len() != n { return None; }
    for (i, (b, text)) in buf.iter_mut().zip(texts).enumerate() {
        *b = DialogButton { text, primary: i as i64 == default_at, cancel: i as i64 == cancel_at, danger: danger >> i & 1 != 0, enabled: disabled >> i & 1 == 0, detail: details.get(i).copied().unwrap_or("") };
    }
    Some(&buf[..n])
}

// ABI 1.86: the one modal route with a presentation: a modal keyed by `keyed.key`, named `keyed.name`, presented as `sheet` says
// (centred, a bottom sheet, full screen or top-anchored at `sheet.top`; a bottom or full-screen sheet takes a downward drag-to-dismiss with `sheet`'s thresholds) and
// `sheet.width` wide when `has_width` is 1. Every field of `sheet` is checked before the build: a short record, `has_width` or `present`
// out of range, a non-zero `reserved`, or a width or threshold that is negative or not finite is refused. Writes `out` (1 while open)
// and the modal's root to `root` (the empty box when it is dismissed).
export!(trusted pub unsafe fn pxui_ui_modal(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, sheet: *const pxui_types::ffi::PxuiSheetDesc, callback: Build, user: *mut c_void, out: *mut u8, root: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() || root.is_null() { return unsafe { fail(ui, "null modal output") }; }
    let Some((key, name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    let Some(sheet) = sized!(sheet) else { return unsafe { fail(ui, "invalid sheet descriptor") } };
    let finite = |v: f32| (0.0..=f32::MAX).contains(&v);
    if sheet.has_width > 1 || sheet.present > 3 || sheet.reserved != [0; 2] || ![sheet.width, sheet.dismiss_fraction, sheet.dismiss_velocity, sheet.top, sheet.max_h].into_iter().all(finite) {
        return unsafe { fail(ui, "invalid sheet descriptor") };
    }
    let present = [Presentation::Centered, Presentation::Bottom, Presentation::Full, Presentation::Top][sheet.present as usize];
    let dismiss = DismissOpts { fraction: (sheet.dismiss_fraction > 0.0).then_some(sheet.dismiss_fraction), velocity: (sheet.dismiss_velocity > 0.0).then_some(sheet.dismiss_velocity) };
    let overlay = Overlay { w: (sheet.has_width != 0).then_some(sheet.width), at: (0.0, sheet.top), max_h: (sheet.max_h > 0.0).then_some(sheet.max_h), name: Some(name), present, dismiss, ..Overlay::DEFAULT };
    let mut open = false;
    let rc = unsafe { crate::trusted::modal(ui, key, &overlay, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, callback, user), &mut open, &mut *root) };
    unsafe { *out = open as u8; }
    rc
});

// ABI 1.80: a scroll view keyed by `keyed.key`, named `keyed.name` (empty: none), laid out by the checked `node` (no grid); the frame
// adds its scroll flags, look, thumbs, scroll step and role, and insets the content one point inside the node's padding.
export!(trusted pub unsafe fn pxui_ui_scroll_view(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, node: *const Node, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some((key, name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    let Some(node) = (unsafe { crate::node::checked_node(node) }) else { return unsafe { fail(ui, "invalid scroll view node") } };
    if out.is_null() { return unsafe { fail(ui, "null scroll view output") }; }
    unsafe { crate::trusted::scroll_view(ui, key, name, node, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, callback, user), &mut *out) }
});

// ABI 1.72: the authoring view keyed by `keyed.key` (a view has no name: a non-empty `keyed.name` is refused). It fills its parent,
// or takes its content's size on the main axis of a row or column that is `Fit` there; `revision` is the hash of the inputs.
export!(trusted pub unsafe fn pxui_ui_view(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, revision: u64, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null view output") }; }
    let Some((key, "")) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { crate::trusted::view_of(ui, key, revision, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, callback, user), out) }
});

/// `anchor`'s placement, or `None` for an invalid descriptor. `align` 3 and 4 (PXUI_ALIGN_STRETCH, _BASELINE) are accepted for 1.x
/// and align as Start, as they always did; 2.0 refuses them (abi2_cut: validation tightened).
fn placement(anchor: &PxuiAnchorDesc) -> Option<pxui_types::layout::Placement> {
    if anchor.has_anchor != 1 || anchor.flip > 1 || anchor.side > 3 || anchor.align > 4 { return None; }
    let side = match anchor.side { 0 => pxui_types::layout::Side::Below, 1 => pxui_types::layout::Side::Above, 2 => pxui_types::layout::Side::Right, _ => pxui_types::layout::Side::Left };
    let align = match anchor.align { 1 => pxui_types::layout::Along::Center, 2 => pxui_types::layout::Along::End, _ => pxui_types::layout::Along::Start };
    Some(pxui_types::layout::Placement { side, align, gap: anchor.gap, margin: anchor.margin, flip: anchor.flip != 0 })
}

// ABI 1.68: a sheet by its anchor keyed by `keyed.key`, named `keyed.name` (empty: none); `width` and `height` in points or fit.
export!(trusted pub unsafe fn pxui_ui_anchored(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, anchor: *const PxuiAnchorDesc, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some((key, name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    if anchor.is_null() || out.is_null() { return unsafe { fail(ui, "null anchored argument") }; }
    let points = |s: Option<pxui_types::Size>| match s { Some(pxui_types::Size::Fit) => Some(None), Some(pxui_types::Size::Px(v)) => Some(Some(v)), _ => None };
    let (Some(w), Some(h)) = (points(size(width)), points(size(height))) else { return unsafe { fail(ui, "invalid anchored size: points or fit") }; };
    let anchor = unsafe { *anchor };
    let Some(place) = placement(&anchor) else { return unsafe { fail(ui, "invalid anchor descriptor") }; };
    let overlay = Overlay { w, h, place, name: (!name.is_empty()).then_some(name), ..Overlay::DEFAULT };
    unsafe { crate::trusted::anchored(ui, key, anchor.rect, &overlay, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, callback, user), &mut *out) }
});

// ABI 1.53: a dialog with flagged buttons (`PxuiDialogDesc`) answering a `PxuiDialogResult`.
export!(trusted pub unsafe fn pxui_ui_dialog(ui: *mut PxuiUi, desc: *const PxuiDialogDesc, callback: Build, user: *mut c_void, out: *mut PxuiDialogResult) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() { return unsafe { fail(ui, "null dialog argument") }; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short dialog result") } };
    // the 88-byte record of 1.53 and the whole one; an 88-byte record's `details` are zero (none)
    let Some((d, _)) = read_desc!(desc, PxuiDialogDesc, [88]) else { return unsafe { fail(ui, "invalid dialog descriptor") } };
    let mut list = state.shared().take_strs();
    let mut buf = [DialogButton::new(""); MAX_BUTTONS];
    // the labels, then the details after them in the same scratch
    let read = unsafe { strings(d.buttons, MAX_BUTTONS, &mut list) && crate::state::append_strings(d.details, MAX_BUTTONS, &mut list) };
    let decoded = match (unsafe { crate::state::checked_key(d.key) }, unsafe { checked(d.title) }) {
        (Some(key), Some(title)) if read => { let (texts, details) = list.split_at(d.buttons.len); dialog_buttons(texts, details, (d.default_at, d.cancel_at, d.danger, d.disabled), &mut buf).map(|b| (key, title, b)) }
        _ => None,
    };
    let Some((key, title, buttons)) = decoded else {
        state.shared().put_strs(list);
        return unsafe { fail(ui, "invalid dialog descriptor") };
    };
    let mut result = Chosen { chosen: None, chosen_id: None, dismissed: false };
    let rc = unsafe { crate::trusted::dialog(ui, key, title, buttons, &Dialog::DEFAULT.w_px(d.width), &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, callback, user), &mut result) };
    state.shared().put_strs(list);
    if rc == 0 {
        (out.has_chosen, out.chosen, out.dismissed) = (result.chosen.is_some() as u8, result.chosen.unwrap_or(0) as u64, result.dismissed as u8);
    }
    rc
});
