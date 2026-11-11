//! Kit and text widgets beyond the first set: styled buttons, grips, popups, inspector pieces and the line field.
use crate::interaction::write_signal;
use crate::kit::PxuiHintDesc;
use crate::state::{Build, MAX_TOTAL_TEXT_BYTES, PxuiUi, boxed, call_build, checked, engine, fail, get, to_abi_box};
use crate::text_inputs::TextValid;
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, PxuiSignal, Str};
use core::ffi::c_void;
use pxui_core::ui::{Axis, HeadAction, LineStep, Rect, Ui};
use pxui_types::{geom::{Point, Rgba}, ffi::{PxuiHeadActionDesc, PxuiHeadOut, PxuiHeaderDesc, PxuiPopupDesc, Slice}};
use std::rc::Rc;

const MAX_ACTIONS: usize = 256;

export!(pub unsafe fn pxui_ui_button_styled(ui: *mut PxuiUi, text: Str, bare: u8, icon: u8, at_end: u8, has_ink: u8, ink: Rgba, out: *mut u8) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(text) = (unsafe { checked(text) }).filter(|_| bare <= 1 && icon <= 1 && at_end <= 1 && has_ink <= 1 && !out.is_null()) else { return unsafe { fail(ui, "invalid button arguments") }; };
    unsafe { *out = e.button_styled(text, bare != 0, icon != 0, at_end != 0, (has_ink != 0).then_some(ink)) as u8; }
    0
});

// `axis` 0 is a row (a vertical grip), 1 a column; `rect` places the grip in its parent when `has_rect` is set.
export!(pub unsafe fn pxui_ui_grip(ui: *mut PxuiUi, label: Str, axis: u32, has_rect: u8, rect: Rect, out: *mut PxuiBox, signal: *mut PxuiSignal) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let axis = match axis { 0 => Axis::Row, 1 => Axis::Column, _ => return unsafe { fail(ui, "unknown grip axis") } };
    let Some(label) = (unsafe { checked(label) }).filter(|_| has_rect <= 1 && !out.is_null() && !signal.is_null() && crate::state::finite(&[rect.x, rect.y, rect.w, rect.h])) else { return unsafe { fail(ui, "invalid grip arguments") }; };
    let (handle, result) = unsafe { (&mut *state.engine).grip(format_args!("{label}"), axis, (has_rect != 0).then_some(rect)) };
    unsafe { *out = to_abi_box(handle); write_signal(state, &result, &mut *signal); }
    0
});

export!(pub unsafe fn pxui_ui_popup(ui: *mut PxuiUi, desc: *const PxuiPopupDesc, build: Build, user: *mut c_void, out: *mut u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() || out.is_null() { return unsafe { fail(ui, "null popup argument") }; }
    let Some(d) = read_desc!(desc, PxuiPopupDesc) else { return unsafe { fail(ui, "invalid popup descriptor") }; };
    if d.has_stroke > 1 || d.has_max_height > 1 || d.dismiss_initial > 1 || d.attached > 1
        || !crate::state::finite(&[d.x, d.y, d.width, d.height]) || (d.has_max_height != 0 && !d.max_height.is_finite()) {
        return unsafe { fail(ui, "invalid popup descriptor") };
    }
    let Some(label) = (unsafe { checked(d.label) }) else { return unsafe { fail(ui, "invalid popup label") }; };
    let shared = state.shared;
    let mut body = |engine: &mut Ui| call_build(shared, engine, build, user);
    let open = unsafe { (&mut *state.engine).popup((d.has_stroke != 0).then_some(d.stroke), (d.has_max_height != 0).then_some(d.max_height), d.dismiss_initial != 0, d.attached != 0, (d.x, d.y), d.width, d.height, label, &mut body) };
    unsafe { *out = open as u8; }
    0
});

export!(pub unsafe fn pxui_ui_inspector_bar(ui: *mut PxuiUi, hints: Slice<PxuiHintDesc>, count: i64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if hints.len > 4096 || (hints.len != 0 && hints.ptr.is_null()) { return unsafe { fail(ui, "invalid inspector bar hints") }; }
    let raw = if hints.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(hints.ptr, hints.len) } };
    // the strings are reused between frames, like the footer's, so a settled bar allocates nothing
    let mut rows = core::mem::take(&mut state.shared().hints);
    rows.truncate(raw.len());
    let mut total = 0usize;
    for (at, hint) in raw.iter().enumerate() {
        let (Some(key), Some(value)) = (unsafe { checked(hint.key) }, unsafe { checked(hint.value) }) else { state.shared().hints = rows; return unsafe { fail(ui, "invalid inspector bar hint") }; };
        total += key.len() + value.len();
        if total > MAX_TOTAL_TEXT_BYTES { state.shared().hints = rows; return unsafe { fail(ui, "inspector bar text limit exceeded") }; }
        match rows.get_mut(at) {
            Some(row) => { row.0.clear(); row.0.push_str(key); row.1.clear(); row.1.push_str(value); }
            None => rows.push((key.to_owned(), value.to_owned())),
        }
    }
    unsafe { (&mut *state.engine).inspector_bar(&rows, count); }
    state.shared().hints = rows;
    0
});

export!(pub unsafe fn pxui_ui_inspector_body(ui: *mut PxuiUi, build: Build, user: *mut c_void) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let shared = state.shared;
    unsafe { (&mut *state.engine).inspector_body(&mut |engine: &mut Ui| call_build(shared, engine, build, user)); }
    0
});

// `rename` (when set) says whether a typed name is valid; it is called only during this call.
export!(pub unsafe fn pxui_ui_inspector_header(ui: *mut PxuiUi, desc: *const PxuiHeaderDesc, rename: TextValid, user: *mut c_void, out: *mut PxuiHeadOut) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() || out.is_null() { return unsafe { fail(ui, "null inspector header argument") }; }
    let Some(d) = read_desc!(desc, PxuiHeaderDesc) else { return unsafe { fail(ui, "invalid inspector header descriptor") }; };
    if [d.has_kind, d.has_badge, d.has_index, d.has_reset, d.reset_enabled].iter().any(|v| *v > 1)
        || d.actions.len > MAX_ACTIONS || (d.actions.len != 0 && d.actions.ptr.is_null()) {
        return unsafe { fail(ui, "invalid inspector header descriptor") };
    }
    let optional = |flag: u8, value: Str| -> Result<Option<&'static str>, ()> { if flag == 0 { Ok(None) } else { unsafe { checked(value) }.map(Some).ok_or(()) } };
    let (Some(key), Some(title), Some(detail), Ok(kind), Ok(badge), Ok(index), Ok(reset)) = (
        unsafe { checked(d.key) }, unsafe { checked(d.title) }, unsafe { checked(d.detail) },
        optional(d.has_kind, d.kind), optional(d.has_badge, d.badge), optional(d.has_index, d.index), optional(d.has_reset, d.reset),
    ) else { return unsafe { fail(ui, "invalid inspector header text") }; };
    let raw: &[PxuiHeadActionDesc] = if d.actions.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(d.actions.ptr, d.actions.len) } };
    let mut actions = Vec::with_capacity(raw.len());
    for a in raw {
        let (Some(caption), Some(keycap)) = (unsafe { checked(a.caption) }, unsafe { checked(a.keycap) }) else { return unsafe { fail(ui, "invalid inspector header action") }; };
        if a.active > 1 || a.usable > 1 { return unsafe { fail(ui, "invalid inspector header action flag") }; }
        actions.push(HeadAction { caption: caption.to_owned(), keycap: keycap.to_owned(), active: a.active != 0, usable: a.usable != 0 });
    }
    let valid: Option<Rc<dyn Fn(&str) -> bool>> = rename.map(|rename| Rc::new(move |name: &str| rename(user, Str { ptr: name.as_ptr(), len: name.len() }) != 0) as Rc<dyn Fn(&str) -> bool>);
    let head = unsafe { (&mut *state.engine).inspector_header(key, kind, badge, index, valid, &actions, reset, d.reset_enabled != 0, title, detail) };
    let renamed = state.give(head.renamed);
    unsafe { *out = PxuiHeadOut { renamed, chosen: head.chosen.unwrap_or(0), has_chosen: head.chosen.is_some() as u8, reset_pressed: head.reset_pressed as u8, reserved: [0; 6] }; }
    0
});

export!(pub unsafe fn pxui_ui_inspector_section_view(ui: *mut PxuiUi, key: Str, revision: u64, expanded: u8, has_set: u8, set_expanded: u8, label: Str, build: Build, user: *mut c_void, out: *mut u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let (Some(key), Some(label)) = (unsafe { checked(key) }, unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid inspector section text") }; };
    if out.is_null() || expanded > 1 || has_set > 1 || set_expanded > 1 { return unsafe { fail(ui, "invalid inspector section arguments") }; }
    let shared = state.shared;
    let mut body = |engine: &mut Ui| call_build(shared, engine, build, user);
    let open = unsafe { (&mut *state.engine).inspector_section_view(key, revision, expanded != 0, (has_set != 0).then_some(set_expanded != 0), label, &mut body) };
    unsafe { *out = open as u8; }
    0
});

export!(pub unsafe fn pxui_ui_inspector_toggle_value(ui: *mut PxuiUi, key: Str, x: f32, y: f32, value: u8, out: *mut u8) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(key) = (unsafe { checked(key) }).filter(|_| value <= 1 && !out.is_null() && crate::state::finite(&[x, y])) else { return unsafe { fail(ui, "invalid toggle arguments") }; };
    unsafe { *out = e.inspector_toggle_value(key, (x, y), value != 0) as u8; }
    0
});

// `step`: 0 none, 1 next, 2 previous, 3 close. The text is borrowed until the next frame.
export!(pub unsafe fn pxui_ui_line_field(ui: *mut PxuiUi, x: f32, y: f32, w: f32, h: f32, grab: u8, placeholder: Str, label: Str, value: Str, text: *mut Str, step: *mut u32) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let (Some(placeholder), Some(label), Some(value)) = (unsafe { checked(placeholder) }, unsafe { checked(label) }, unsafe { checked(value) }) else { return unsafe { fail(ui, "invalid line field text") }; };
    if text.is_null() || step.is_null() || grab > 1 || !crate::state::finite(&[x, y, w, h]) { return unsafe { fail(ui, "invalid line field arguments") }; }
    let (edited, next) = unsafe { (&mut *state.engine).line_field((x, y), w, h, grab != 0, placeholder, label, value) };
    unsafe {
        *text = state.give(edited);
        *step = match next { LineStep::None => 0, LineStep::Next => 1, LineStep::Previous => 2, LineStep::Close => 3 };
    }
    0
});

export!(pub unsafe fn pxui_ui_field_drag(ui: *mut PxuiUi, box_: *const PxuiBox, present: *mut u8, out: *mut Point) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if present.is_null() || out.is_null() { return unsafe { fail(ui, "null field drag output") }; }
    let drag = e.field_drag(&b);
    unsafe { *present = drag.is_some() as u8; *out = drag.map_or(Point::default(), |(x, y)| Point { x, y }); }
    0
});
