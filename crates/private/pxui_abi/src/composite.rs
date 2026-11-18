use crate::node::size;
use crate::state::{Build, PxuiUi, checked, fail, get, strings};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, PxuiDialogOut, Str};
use core::ffi::c_void;
use pxui_types::ffi::{PxuiAnchorDesc, PxuiDialogDesc, PxuiDialogResult, PxuiSizeDesc, Slice};
use pxui_types::ui::{shown, Dialog, DialogButton, Chosen, Overlay};
use pxui_types::ffi::PxuiKeyedDesc;

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

/// The modal of a width flag and width named `name`, the body of `pxui_ui_modal` and `pxui_ui_modal_keyed`.
unsafe fn modal(ui: *mut PxuiUi, key: &str, name: &str, has_width: u8, width: f32, callback: Build, user: *mut c_void, out: *mut u8, root: Option<&mut PxuiBox>) -> i32 {
    if out.is_null() || has_width > 1 { return unsafe { fail(ui, "invalid modal arguments") }; }
    let overlay = Overlay { w: (has_width != 0).then_some(width), name: Some(name), ..Overlay::DEFAULT };
    let (mut open, mut node) = (false, PxuiBox::default());
    let rc = unsafe { crate::trusted::modal(ui, key, &overlay, &mut |ui| if let Some(callback) = callback { callback(user, ui.cast()) }, &mut open, &mut node) };
    unsafe { *out = open as u8; }
    if let Some(root) = root { *root = node; }
    rc
}

// Legacy (cut at 2.0): keyed by its label, named up to a `##`; `pxui_ui_modal_keyed` keys apart.
export!(trusted pub unsafe fn pxui_ui_modal(ui: *mut PxuiUi, label: Str, has_width: u8, width: f32, callback: Build, user: *mut c_void, out: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid modal label") }; };
    unsafe { modal(ui, label, shown(label), has_width, width, callback, user, out, None) }
});

// ABI 1.68: a modal keyed by `keyed.key`, named `keyed.name`.
export!(trusted pub unsafe fn pxui_ui_modal_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, has_width: u8, width: f32, callback: Build, user: *mut c_void, out: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some((key, name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { modal(ui, key, name, has_width, width, callback, user, out, None) }
});

// ABI 1.76: `pxui_ui_modal_keyed` that also writes the modal's root to `root` (the empty box when it is dismissed).
export!(trusted pub unsafe fn pxui_ui_modal_root(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, has_width: u8, width: f32, callback: Build, user: *mut c_void, out: *mut u8, root: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if root.is_null() { return unsafe { fail(ui, "null modal root") }; }
    let Some((key, name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { modal(ui, key, name, has_width, width, callback, user, out, Some(&mut *root)) }
});

/// The scroll view named `name`, the body of `pxui_ui_scroll_view` and `pxui_ui_scroll_view_keyed`.
unsafe fn scroll_view(ui: *mut PxuiUi, key: &str, name: &str, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    if out.is_null() { return unsafe { fail(ui, "null scroll view output") }; }
    let (Some(width), Some(height)) = (size(width), size(height)) else { return unsafe { fail(ui, "invalid scroll view arguments") }; };
    unsafe { crate::trusted::scroll_view(ui, key, name, width, height, &mut |ui| if let Some(callback) = callback { callback(user, ui.cast()) }, &mut *out) }
}

// Legacy (cut at 2.0): keyed by its label, named up to a `##`; `pxui_ui_scroll_view_keyed` keys apart.
export!(trusted pub unsafe fn pxui_ui_scroll_view(ui: *mut PxuiUi, label: Str, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid scroll view arguments") }; };
    unsafe { scroll_view(ui, label, shown(label), width, height, callback, user, out) }
});

// ABI 1.68: a scroll view keyed by `keyed.key`, named `keyed.name` (empty: none).
export!(trusted pub unsafe fn pxui_ui_scroll_view_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some((key, name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { scroll_view(ui, key, name, width, height, callback, user, out) }
});

unsafe fn view_with(ui: *mut PxuiUi, label: Str, revision: u64, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox, flat: bool) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null view output") }; }
    let (Some(label), Some(width), Some(height)) = (unsafe { checked(label) }, size(width), size(height)) else {
        return unsafe { fail(ui, "invalid view arguments") };
    };
    unsafe { crate::trusted::view(ui, label, revision, width, height, flat, &mut |ui| if let Some(callback) = callback { callback(user, ui.cast()) }, out) }
}

export!(trusted pub unsafe fn pxui_ui_view(ui: *mut PxuiUi, label: Str, revision: u64, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    unsafe { view_with(ui, label, revision, width, height, callback, user, out, false) }
});

// A view that takes no entry of its own in the painter view table: its instances ride its parent entry.
export!(trusted pub unsafe fn pxui_ui_view_flat(ui: *mut PxuiUi, label: Str, revision: u64, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    unsafe { view_with(ui, label, revision, width, height, callback, user, out, true) }
});

// ABI 1.72: the authoring view keyed by `keyed.key` (a view has no name: a non-empty `keyed.name` is refused). It fills its parent,
// or takes its content's size on the main axis of a row or column that is `Fit` there; `revision` is the hash of the inputs.
export!(trusted pub unsafe fn pxui_ui_view_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, revision: u64, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null view output") }; }
    let Some((key, "")) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { crate::trusted::view_of(ui, key, revision, &mut |ui| if let Some(callback) = callback { callback(user, ui.cast()) }, out) }
});

/// `anchor`'s placement, or `None` for an invalid descriptor. `align` 3 and 4 (PXUI_ALIGN_STRETCH, _BASELINE) are accepted for 1.x
/// and align as Start, as they always did; 2.0 refuses them (abi2_cut: validation tightened).
fn placement(anchor: &PxuiAnchorDesc) -> Option<pxui_types::layout::Placement> {
    if anchor.has_anchor != 1 || anchor.flip > 1 || anchor.side > 3 || anchor.align > 4 { return None; }
    let side = match anchor.side { 0 => pxui_types::layout::Side::Below, 1 => pxui_types::layout::Side::Above, 2 => pxui_types::layout::Side::Right, _ => pxui_types::layout::Side::Left };
    let align = match anchor.align { 1 => pxui_types::layout::Along::Center, 2 => pxui_types::layout::Along::End, _ => pxui_types::layout::Along::Start };
    Some(pxui_types::layout::Placement { side, align, gap: anchor.gap, margin: anchor.margin, flip: anchor.flip != 0 })
}

// Legacy (cut at 2.0): keyed by its label (identity only); `pxui_ui_anchored_keyed` keys apart and names.
export!(build pub unsafe fn pxui_ui_anchored(ui: *mut PxuiUi, label: Str, anchor: *const PxuiAnchorDesc, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if anchor.is_null() || out.is_null() { return unsafe { fail(ui, "null anchored argument") }; }
    let (Some(label), Some(width), Some(height)) = (unsafe { checked(label) }, size(width), size(height)) else { return unsafe { fail(ui, "invalid anchored arguments") }; };
    let anchor = unsafe { *anchor };
    let Some(place) = placement(&anchor) else { return unsafe { fail(ui, "invalid anchor descriptor") }; };
    let r = anchor.rect;
    if label.len() > crate::state::MAX_TEXT_BYTES || r.w < 0.0 || r.h < 0.0 || place.margin < 0.0 || ![r.x, r.y, r.w, r.h, place.gap, place.margin].iter().all(|v| v.is_finite()) { return unsafe { fail(ui, "invalid anchored arguments") }; }
    let shared = state.shared;
    let b = unsafe { (&mut *state.engine).anchored_legacy(label, r, place, width, height, &mut |engine| crate::state::call_build(shared, engine, callback, user)) };
    unsafe { *out = crate::state::to_abi_box(b); }
    0
});

// ABI 1.68: a sheet by its anchor keyed by `keyed.key`, named `keyed.name` (empty: none); `width` and `height` in points or fit.
export!(trusted pub unsafe fn pxui_ui_anchored_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, anchor: *const PxuiAnchorDesc, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some((key, name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    if anchor.is_null() || out.is_null() { return unsafe { fail(ui, "null anchored argument") }; }
    let points = |s: Option<pxui_types::Size>| match s { Some(pxui_types::Size::Fit) => Some(None), Some(pxui_types::Size::Px(v)) => Some(Some(v)), _ => None };
    let (Some(w), Some(h)) = (points(size(width)), points(size(height))) else { return unsafe { fail(ui, "invalid anchored size: points or fit") }; };
    let anchor = unsafe { *anchor };
    let Some(place) = placement(&anchor) else { return unsafe { fail(ui, "invalid anchor descriptor") }; };
    let overlay = Overlay { w, h, place, name: (!name.is_empty()).then_some(name), ..Overlay::DEFAULT };
    unsafe { crate::trusted::anchored(ui, key, anchor.rect, &overlay, &mut |ui| if let Some(callback) = callback { callback(user, ui.cast()) }, &mut *out) }
});

// A dialog with the plain buttons of its labels: the last is the default, none cancels, the sheet is keyed by the title.
export!(trusted pub unsafe fn pxui_ui_dialog(ui: *mut PxuiUi, title: Str, buttons: Slice<Str>, callback: Build, user: *mut c_void, out: *mut PxuiDialogOut) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null dialog output") }; }
    let mut list = state.shared().take_strs();
    let mut buf = [DialogButton::new(""); MAX_BUTTONS];
    let n = buttons.len as i64;
    let flags = (n - 1, -1, 0, 0);
    let decoded = match unsafe { checked(title) } {
        Some(title) if unsafe { strings(buttons, MAX_BUTTONS, &mut list) } => dialog_buttons(&list, &[], flags, &mut buf).map(|b| (title, b)),
        _ => None,
    };
    let Some((title, buttons)) = decoded else {
        state.shared().put_strs(list);
        return unsafe { fail(ui, "invalid dialog arguments") };
    };
    let mut result = Chosen { chosen: None, chosen_id: None, dismissed: false };
    let rc = unsafe { crate::trusted::dialog(ui, title, title, buttons, &Dialog::DEFAULT, &mut |ui| if let Some(callback) = callback { callback(user, ui.cast()) }, &mut result) };
    state.shared().put_strs(list);
    if rc == 0 {
        unsafe { *out = match result { Chosen { chosen: Some(index), .. } => PxuiDialogOut { tag: 2, index }, Chosen { dismissed: true, .. } => PxuiDialogOut { tag: 1, index: 0 }, _ => PxuiDialogOut { tag: 0, index: 0 } }; }
    }
    rc
});

// ABI 1.53: a dialog with flagged buttons (`PxuiDialogDesc`) answering a `PxuiDialogResult`.
export!(trusted pub unsafe fn pxui_ui_dialog_response(ui: *mut PxuiUi, desc: *const PxuiDialogDesc, callback: Build, user: *mut c_void, out: *mut PxuiDialogResult) -> i32 {
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
    let rc = unsafe { crate::trusted::dialog(ui, key, title, buttons, &Dialog::DEFAULT.w_px(d.width), &mut |ui| if let Some(callback) = callback { callback(user, ui.cast()) }, &mut result) };
    state.shared().put_strs(list);
    if rc == 0 {
        (out.has_chosen, out.chosen, out.dismissed) = (result.chosen.is_some() as u8, result.chosen.unwrap_or(0) as u64, result.dismissed as u8);
    }
    rc
});
