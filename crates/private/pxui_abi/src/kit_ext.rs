//! Kit and text widgets beyond the first set: styled buttons, grips, popups, inspector pieces and the line field.
use crate::interaction::write_signal;
use crate::kit::PxuiHintDesc;
use crate::state::{Build, MAX_TOTAL_TEXT_BYTES, PxuiUi, boxed, call_build, checked, engine, fail, get, to_abi_box};
use crate::text_inputs::TextValid;
use crate::trusted;
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, PxuiSignal, Str};
use core::{cell::Cell, ffi::c_void};
use pxui_core::ui::{Axis, Edges, HeadAction, Justify, LineStep, Rect, Size, States, Ui};
use pxui_types::{geom::{Point, Rgba}, ffi::{PxuiButtonDesc, PxuiIconButtonDesc, PxuiResponse, PxuiHeadActionDesc, PxuiHeadOut, PxuiHeaderDesc, PxuiPopupDesc, Slice}};
use std::rc::Rc;

const MAX_ACTIONS: usize = 256;

export!(pub unsafe fn pxui_ui_button_styled(ui: *mut PxuiUi, text: Str, bare: u8, icon: u8, at_end: u8, has_ink: u8, ink: Rgba, out: *mut u8) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(text) = (unsafe { checked(text) }).filter(|_| bare <= 1 && icon <= 1 && at_end <= 1 && has_ink <= 1 && !out.is_null()) else { return unsafe { fail(ui, "invalid button arguments") }; };
    unsafe { *out = e.button_styled(text, bare != 0, icon != 0, at_end != 0, (has_ink != 0).then_some(ink)) as u8; }
    0
});

// The kit button with options; `build` (when set) replaces its text, run inside the box. `states` is null (the theme's button) or six looks.
export!(trusted pub unsafe fn pxui_ui_button_with(ui: *mut PxuiUi, desc: *const PxuiButtonDesc, build: Build, user: *mut c_void, out: *mut PxuiBox, signal: *mut PxuiSignal, focused: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if desc.is_null() || out.is_null() || signal.is_null() || focused.is_null() { return unsafe { fail(ui, "null button argument") }; }
    let mut response = PxuiResponse::EMPTY;
    let rc = unsafe { button_call(ui, desc, build, user, &mut response, Some(&mut *signal)) };
    if rc == 0 { unsafe { (*out, *focused) = (response.node, (response.flags & PxuiResponse::FOCUSED != 0) as u8); } }
    rc
});

// ABI 1.48: `pxui_ui_button_with` reporting into one `PxuiResponse` (no `PxuiSignal`).
export!(trusted pub unsafe fn pxui_ui_button_response(ui: *mut PxuiUi, desc: *const PxuiButtonDesc, build: Build, user: *mut c_void, out: *mut PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if desc.is_null() { return unsafe { fail(ui, "null button argument") }; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short button response") } };
    unsafe { button_call(ui, desc, build, user, out, None) }
});

// ABI 1.48: the plain kit button (`Button::DEFAULT`: the theme's look, the kit's size, at the start of its line) keyed by `key`.
export!(trusted pub unsafe fn pxui_ui_button_plain(ui: *mut PxuiUi, key: Str, text: Str, out: *mut PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short button response") } };
    let (Some(key), Some(text)) = (unsafe { checked(key) }, unsafe { checked(text) }) else { return unsafe { fail(ui, "invalid button text") } };
    unsafe { trusted::button_plain(ui, key, text, out) }
});

/// The checked `PxuiButtonDesc` behind `desc` (not null) through the trusted entry: what happened to `out`, the whole signal to
/// `signal` when given.
unsafe fn button_call(ui: *mut PxuiUi, desc: *const PxuiButtonDesc, build: Build, user: *mut c_void, out: &mut PxuiResponse, signal: Option<&mut PxuiSignal>) -> i32 {
    let Some(d) = read_desc!(desc, PxuiButtonDesc) else { return unsafe { fail(ui, "invalid button descriptor") }; };
    let justify = match d.justify { 0 => Some(Justify::Start), 1 => Some(Justify::Center), 2 => Some(Justify::End), _ => None };
    let (Some(w), Some(h), Some(justify)) = (size(d.w_kind, d.w), size(d.h_kind, d.h), justify) else { return unsafe { fail(ui, "invalid button descriptor") }; };
    if d.enabled > 1 || d.selected > 1 || d.danger > 1 || d.has_name > 1 || d.reserved != 0 { return unsafe { fail(ui, "invalid button descriptor") }; }
    let (Some(key), Some(text), Some(name)) = (unsafe { checked(d.key) }, unsafe { checked(d.text) }, unsafe { checked(d.name) }) else { return unsafe { fail(ui, "invalid button text") }; };
    let Ok(looks) = (unsafe { looks(d.states) }) else { return unsafe { fail(ui, "invalid button looks") } };
    let mut content = |ui: *mut PxuiUi| if let Some(build) = build { build(user, ui.cast()) };
    let pad = Edges { l: d.padding[0], t: d.padding[1], r: d.padding[2], b: d.padding[3] };
    unsafe { trusted::button_with(ui, key, text, (d.has_name != 0).then_some(name), looks.as_ref(), w, h, justify, pad, d.enabled != 0, d.selected != 0, d.danger != 0,
        if build.is_some() { Some(&mut content) } else { None }, out, signal) }
}

// The kit icon button with options; `states` is null (the theme's icon button) or six looks.
export!(trusted pub unsafe fn pxui_ui_icon_button_with(ui: *mut PxuiUi, desc: *const PxuiIconButtonDesc, out: *mut PxuiBox, signal: *mut PxuiSignal, focused: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if desc.is_null() || out.is_null() || signal.is_null() || focused.is_null() { return unsafe { fail(ui, "null icon button argument") }; }
    let mut response = PxuiResponse::EMPTY;
    let rc = unsafe { icon_button_call(ui, desc, &mut response, Some(&mut *signal)) };
    if rc == 0 { unsafe { (*out, *focused) = (response.node, (response.flags & PxuiResponse::FOCUSED != 0) as u8); } }
    rc
});

// ABI 1.48: `pxui_ui_icon_button_with` reporting into one `PxuiResponse` (no `PxuiSignal`).
export!(trusted pub unsafe fn pxui_ui_icon_button_response(ui: *mut PxuiUi, desc: *const PxuiIconButtonDesc, out: *mut PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if desc.is_null() { return unsafe { fail(ui, "null icon button argument") }; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short icon button response") } };
    unsafe { icon_button_call(ui, desc, out, None) }
});

// ABI 1.48: the plain kit icon button (`IconButton::DEFAULT`: the theme's look, the kit's square) showing the glyph `icon`, named `name`.
export!(trusted pub unsafe fn pxui_ui_icon_button_plain(ui: *mut PxuiUi, key: Str, icon: Str, name: Str, out: *mut PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short icon button response") } };
    let (Some(key), Some(icon), Some(name)) = (unsafe { checked(key) }, unsafe { checked(icon) }, unsafe { checked(name) }) else { return unsafe { fail(ui, "invalid icon button text") } };
    unsafe { trusted::icon_button_plain(ui, key, icon, name, out) }
});

/// The checked `PxuiIconButtonDesc` behind `desc` (not null) through the trusted entry, as `button_call`.
unsafe fn icon_button_call(ui: *mut PxuiUi, desc: *const PxuiIconButtonDesc, out: &mut PxuiResponse, signal: Option<&mut PxuiSignal>) -> i32 {
    let Some(d) = read_desc!(desc, PxuiIconButtonDesc) else { return unsafe { fail(ui, "invalid icon button descriptor") }; };
    let (Some(w), Some(h)) = (size(d.w_kind, d.w), size(d.h_kind, d.h)) else { return unsafe { fail(ui, "invalid icon button descriptor") }; };
    if d.enabled > 1 || d.reserved != 0 { return unsafe { fail(ui, "invalid icon button descriptor") }; }
    let (Some(key), Some(name), Some(icon)) = (unsafe { checked(d.key) }, unsafe { checked(d.name) }, unsafe { checked(d.icon) }) else { return unsafe { fail(ui, "invalid icon button text") }; };
    let Ok(looks) = (unsafe { looks(d.states) }) else { return unsafe { fail(ui, "invalid icon button looks") } };
    unsafe { trusted::icon_button_with(ui, key, icon, d.draw, d.draw_user, name, looks.as_ref(), w, h, d.enabled != 0, d.press_scale, out, signal) }
}

/// A button size as a descriptor writes it (kind 0 the kit's, 1 points, 2 fill); the points are the trusted entry's to check.
fn size(kind: u8, value: f32) -> Option<Size> { match kind { 0 => Some(Size::Fit), 1 => Some(Size::Px(value)), 2 => Some(Size::Grow), _ => None } }

/// The six looks behind `states` (none when null); `Err` when one does not decode.
unsafe fn looks(states: *const pxui_types::ffi::PxuiLookDesc) -> Result<Option<States>, ()> {
    if states.is_null() { return Ok(None); }
    let raw = unsafe { core::slice::from_raw_parts(states, 6) };
    let mut it = raw.iter().copied().map(crate::style::decode_look);
    let mut next = || it.next().flatten();
    match (next(), next(), next(), next(), next(), next()) {
        (Some(idle), Some(hover), Some(pressed), Some(focus), Some(disabled), Some(checked)) => Ok(Some(States { idle, hover, pressed, focus, disabled, checked })),
        _ => Err(()),
    }
}

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
    if !raw.iter().all(|hint| hint.size as usize == core::mem::size_of_val(hint)) { return unsafe { fail(ui, "invalid inspector bar hints") }; }
    // the rows borrow the caller's strings for this call, in a vector reused between frames, like the footer's: a settled bar allocates nothing
    let mut rows = core::mem::take(&mut state.shared().hints);
    let mut total = 0usize;
    for hint in raw {
        let (Some(key), Some(value)) = (unsafe { checked(hint.key) }, unsafe { checked(hint.value) }) else { rows.clear(); state.shared().hints = rows; return unsafe { fail(ui, "invalid inspector bar hint") }; };
        total += key.len() + value.len();
        if total > MAX_TOTAL_TEXT_BYTES { rows.clear(); state.shared().hints = rows; return unsafe { fail(ui, "inspector bar text limit exceeded") }; }
        rows.push((key, value));
    }
    unsafe { (&mut *state.engine).inspector_bar(&rows, count); }
    rows.clear();
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
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "invalid inspector header output") }; };
    if desc.is_null() { return unsafe { fail(ui, "null inspector header argument") }; }
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
    if !raw.iter().all(|a| a.size as usize == core::mem::size_of_val(a)) { return unsafe { fail(ui, "invalid inspector header action") }; }
    // the strings are reused between frames, like the hints': a settled head allocates nothing
    let mut actions = core::mem::take(&mut state.shared().actions);
    actions.truncate(raw.len());
    for (at, a) in raw.iter().enumerate() {
        let (Some(caption), Some(keycap)) = (unsafe { checked(a.caption) }, unsafe { checked(a.keycap) }) else { state.shared().actions = actions; return unsafe { fail(ui, "invalid inspector header action") }; };
        if a.active > 1 || a.usable > 1 { state.shared().actions = actions; return unsafe { fail(ui, "invalid inspector header action flag") }; }
        match actions.get_mut(at) {
            Some(row) => { row.caption.clear(); row.caption.push_str(caption); row.keycap.clear(); row.keycap.push_str(keycap); row.active = a.active != 0; row.usable = a.usable != 0; }
            None => actions.push(HeadAction { caption: caption.to_owned(), keycap: keycap.to_owned(), active: a.active != 0, usable: a.usable != 0 }),
        }
    }
    let valid = rename.map(|rename| {
        let shared = state.shared();
        let (cell, valid) = shared.rename.get_or_insert_with(|| {
            let cell = Rc::new(Cell::new((None, 0usize)));
            let read = Rc::clone(&cell);
            (cell, Rc::new(move |name: &str| { let (rename, user) = read.get(); rename.is_some_and(|rename| rename(user as *mut c_void, Str { ptr: name.as_ptr(), len: name.len() }) != 0) }))
        });
        cell.set((Some(rename), user as usize));
        Rc::clone(valid)
    });
    let head = unsafe { (&mut *state.engine).inspector_header(key, kind, badge, index, valid, &actions, reset, d.reset_enabled != 0, title, detail) };
    state.shared().actions = actions;
    let renamed = state.give(head.renamed);
    (out.renamed, out.chosen, out.has_chosen, out.reset_pressed) = (renamed, head.chosen.unwrap_or(0), head.chosen.is_some() as u8, head.reset_pressed as u8);
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
