use crate::node::size;
use crate::state::{Build, PxuiUi, checked, fail, get, strings};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, PxuiDialogOut, Str};
use core::ffi::c_void;
use pxui_types::ffi::{PxuiAnchorDesc, PxuiDialogDesc, PxuiDialogResult, PxuiSizeDesc, Slice};
use pxui_types::ui::{Dialog, DialogButton, DialogResponse};

const MAX_BUTTONS: usize = 64;

/// The buttons of `texts` with the flags a dialog record carries (`PxuiDialogDesc`): `buf` holds them for the call.
fn dialog_buttons<'a, 'b>(texts: &[&'a str], (default_at, cancel_at, danger, disabled): (i64, i64, u64, u64), buf: &'b mut [DialogButton<'a>; MAX_BUTTONS]) -> Option<&'b [DialogButton<'a>]> {
    let n = texts.len();
    let beyond = |mask: u64| n < MAX_BUTTONS && mask >> n != 0;
    if n > MAX_BUTTONS || !(-1..n as i64).contains(&default_at) || !(-1..n as i64).contains(&cancel_at) || beyond(danger) || beyond(disabled) { return None; }
    for (i, (b, text)) in buf.iter_mut().zip(texts).enumerate() {
        *b = DialogButton { text, default: i as i64 == default_at, cancel: i as i64 == cancel_at, danger: danger >> i & 1 != 0, enabled: disabled >> i & 1 == 0 };
    }
    Some(&buf[..n])
}

export!(trusted pub unsafe fn pxui_ui_modal(ui: *mut PxuiUi, label: Str, has_width: u8, width: f32, callback: Build, user: *mut c_void, out: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() || has_width > 1 { return unsafe { fail(ui, "invalid modal arguments") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid modal label") }; };
    let mut open = false;
    let rc = unsafe { crate::trusted::modal(ui, label, (has_width != 0).then_some(width), &mut |ui| if let Some(callback) = callback { callback(user, ui.cast()) }, &mut open) };
    unsafe { *out = open as u8; }
    rc
});

export!(trusted pub unsafe fn pxui_ui_scroll_view(ui: *mut PxuiUi, label: Str, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null scroll view output") }; }
    let (Some(label), Some(width), Some(height)) = (unsafe { checked(label) }, size(width), size(height)) else {
        return unsafe { fail(ui, "invalid scroll view arguments") };
    };
    unsafe { crate::trusted::scroll_view(ui, label, width, height, &mut |ui| if let Some(callback) = callback { callback(user, ui.cast()) }, &mut *out) }
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

export!(trusted pub unsafe fn pxui_ui_anchored(ui: *mut PxuiUi, label: Str, anchor: *const PxuiAnchorDesc, width: PxuiSizeDesc, height: PxuiSizeDesc, callback: Build, user: *mut c_void, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if anchor.is_null() || out.is_null() { return unsafe { fail(ui, "null anchored argument") }; }
    let (Some(label), Some(width), Some(height)) = (unsafe { checked(label) }, size(width), size(height)) else {
        return unsafe { fail(ui, "invalid anchored arguments") };
    };
    let anchor = unsafe { *anchor };
    if anchor.has_anchor != 1 || anchor.flip > 1 || anchor.side > 3 || anchor.align > 4 { return unsafe { fail(ui, "invalid anchor descriptor") }; }
    let side = match anchor.side { 0 => pxui_types::layout::Side::Below, 1 => pxui_types::layout::Side::Above, 2 => pxui_types::layout::Side::Right, 3 => pxui_types::layout::Side::Left, _ => unreachable!() };
    let align = match anchor.align { 0 => pxui_types::layout::Align::Start, 1 => pxui_types::layout::Align::Center, 2 => pxui_types::layout::Align::End, 3 => pxui_types::layout::Align::Stretch, 4 => pxui_types::layout::Align::Baseline, _ => unreachable!() };
    let placement = pxui_types::layout::Placement { side, align, gap: anchor.gap, margin: anchor.margin, flip: anchor.flip != 0 };
    unsafe { crate::trusted::anchored(ui, label, anchor.rect, placement, width, height, &mut |ui| if let Some(callback) = callback { callback(user, ui.cast()) }, &mut *out) }
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
        Some(title) if unsafe { strings(buttons, MAX_BUTTONS, &mut list) } => dialog_buttons(&list, flags, &mut buf).map(|b| (title, b)),
        _ => None,
    };
    let Some((title, buttons)) = decoded else {
        state.shared().put_strs(list);
        return unsafe { fail(ui, "invalid dialog arguments") };
    };
    let mut result = DialogResponse { chosen: None, dismissed: false };
    let rc = unsafe { crate::trusted::dialog(ui, title, title, buttons, &Dialog::DEFAULT, &mut |ui| if let Some(callback) = callback { callback(user, ui.cast()) }, &mut result) };
    state.shared().put_strs(list);
    if rc == 0 {
        unsafe { *out = match result { DialogResponse { chosen: Some(index), .. } => PxuiDialogOut { tag: 2, index }, DialogResponse { dismissed: true, .. } => PxuiDialogOut { tag: 1, index: 0 }, _ => PxuiDialogOut { tag: 0, index: 0 } }; }
    }
    rc
});

// ABI 1.53: a dialog with flagged buttons (`PxuiDialogDesc`) answering a `PxuiDialogResult`.
export!(trusted pub unsafe fn pxui_ui_dialog_response(ui: *mut PxuiUi, desc: *const PxuiDialogDesc, callback: Build, user: *mut c_void, out: *mut PxuiDialogResult) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() { return unsafe { fail(ui, "null dialog argument") }; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short dialog result") } };
    let Some(d) = read_desc!(desc, PxuiDialogDesc) else { return unsafe { fail(ui, "invalid dialog descriptor") } };
    let mut list = state.shared().take_strs();
    let mut buf = [DialogButton::new(""); MAX_BUTTONS];
    let decoded = match (unsafe { checked(d.key) }, unsafe { checked(d.title) }) {
        (Some(key), Some(title)) if unsafe { strings(d.buttons, MAX_BUTTONS, &mut list) } => dialog_buttons(&list, (d.default_at, d.cancel_at, d.danger, d.disabled), &mut buf).map(|b| (key, title, b)),
        _ => None,
    };
    let Some((key, title, buttons)) = decoded else {
        state.shared().put_strs(list);
        return unsafe { fail(ui, "invalid dialog descriptor") };
    };
    let mut result = DialogResponse { chosen: None, dismissed: false };
    let rc = unsafe { crate::trusted::dialog(ui, key, title, buttons, &Dialog::DEFAULT.width(d.width), &mut |ui| if let Some(callback) = callback { callback(user, ui.cast()) }, &mut result) };
    state.shared().put_strs(list);
    if rc == 0 {
        (out.has_chosen, out.chosen, out.dismissed) = (result.chosen.is_some() as u8, result.chosen.unwrap_or(0) as u64, result.dismissed as u8);
    }
    rc
});
