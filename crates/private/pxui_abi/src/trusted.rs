//! The trusted entries: the bodies of the hot exports, minus what only an untrusted caller needs. The static wrapper's caller is Rust, so
//! its strings are UTF-8, its enums in range, its references non-null and a panic unwinds through these frames as it did through the engine
//! itself. A `pxui_*` export of the same name checks what C hands over (poison, null, UTF-8, tags) and then calls its entry here, so the
//! two paths run one body. What Rust types cannot promise stays checked in the body (a text longer than `MAX_TEXT_BYTES`, a stale box,
//! a number that is not finite). Each entry counts itself as one call into the library.
use crate::node::declare;
use crate::state::{get, Inner, MAX_TEXT_BYTES, MAX_TOTAL_TEXT_BYTES, PxuiUi, boxed, call_inner, fail, note_call, to_abi_box};
use crate::{PxuiBox, PxuiSignal, Str};
use pxui_types::ffi::PxuiResponse;
use pxui_core::ui::{SegmentedOpts, ToggleOpts, Ui};
use pxui_types::{FrameIn, FrameOut, Node, Size, ffi::Input, geom::Edges, layout::Justify, theme::{States, ThemeSnapshot}};


/// `value` as text the caller promises is UTF-8 (the checked length is the one the exports refuse above). Its pointer is never null:
/// a Rust `str`'s, or the pair an export checked (`state::pair`, an empty text included).
#[inline]
unsafe fn utf8<'a>(value: Str) -> Option<&'a str> {
    if value.len > MAX_TEXT_BYTES { return None; }
    Some(unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(value.ptr, value.len)) })
}
#[inline]
unsafe fn engine<'a>(ui: *mut PxuiUi) -> &'a mut Ui { unsafe { &mut *(*ui).engine } }

#[inline]
pub unsafe fn pxui_ui_node(ui: *mut PxuiUi, label: Str, desc: &Node, out: *mut PxuiBox) -> i32 {
    let Some(label) = (unsafe { utf8(label) }) else { note_call(); return unsafe { fail(ui, "invalid node label") } };
    unsafe { node(ui, label, desc, out) }
}

/// `pxui_ui_node` of a Rust label (`Key::label`): UTF-8 by its type, so only its length is checked (the static wrapper's entry).
#[inline(always)]
pub unsafe fn node_str(ui: *mut PxuiUi, label: &str, desc: &Node, out: *mut PxuiBox) -> i32 {
    if long(label) { note_call(); return unsafe { fail(ui, "invalid node label") } }
    unsafe { node(ui, label, desc, out) }
}

/// `pxui_ui_node` of a label already checked (UTF-8, at most `MAX_TEXT_BYTES`): the export's entry, which checks it as C hands it over.
#[inline(always)]
pub(crate) unsafe fn node(ui: *mut PxuiUi, label: &str, desc: &Node, out: *mut PxuiBox) -> i32 {
    enter!(ui);
    if let pxui_types::node::OptTransition::Some(curve) = &desc.transition { if !curve.valid() { return unsafe { fail(ui, "invalid transition curve") }; } }
    declare(unsafe { engine(ui) }, Some(label), 0, desc, unsafe { &mut *out });
    0
}

#[inline]
pub unsafe fn pxui_ui_node_keyed(ui: *mut PxuiUi, key: i64, desc: &Node, out: *mut PxuiBox) -> i32 {
    enter!(ui);
    if key == 0 || key == 1 { return unsafe { fail(ui, "reserved node key; derive integer keys with int_key") }; }
    if let pxui_types::node::OptTransition::Some(curve) = &desc.transition { if !curve.valid() { return unsafe { fail(ui, "invalid transition curve") }; } }
    declare(unsafe { engine(ui) }, None, key, desc, unsafe { &mut *out });
    0
}

#[inline]
pub unsafe fn pxui_ui_text(ui: *mut PxuiUi, box_: *const PxuiBox, value: Str) -> i32 {
    let Some(value) = (unsafe { utf8(value) }) else { note_call(); return unsafe { fail(ui, "invalid text") } };
    unsafe { text(ui, box_, value) }
}

/// `pxui_ui_text` of a value already checked (UTF-8, at most `MAX_TEXT_BYTES`), as `node`.
#[inline(always)]
pub(crate) unsafe fn text(ui: *mut PxuiUi, box_: *const PxuiBox, value: &str) -> i32 {
    enter!(ui);
    let raw = unsafe { *box_ };
    unsafe { engine(ui) }.text(&pxui_core::ui::UiBox { index: raw.index as usize, box_key: raw.box_key, box_slot: raw.box_slot as usize, generation: raw.generation }, value);
    0
}

#[inline]
pub unsafe fn pxui_label(ui: *mut PxuiUi, label: Str) -> i32 {
    enter!(ui);
    let Some(label) = (unsafe { utf8(label) }) else { return unsafe { fail(ui, "invalid label text") } };
    unsafe { engine(ui) }.label(label, pxui_core::ui::Label::DEFAULT);
    0
}

#[inline(always)]
pub unsafe fn pxui_ui_label(ui: *mut PxuiUi, text: Str, style: &pxui_types::Label, digits: bool) -> i32 {
    enter!(ui);
    let Some(text) = (unsafe { utf8(text) }) else { return unsafe { fail(ui, "invalid label text") } };
    if !unsafe { engine(ui) }.label_styled(text, style, digits) { return unsafe { fail(ui, "invalid label style: a size, line height, padding, width, wrap, align, ellipsis or feature no style has, or a font not loaded") }; }
    0
}

#[inline]
pub unsafe fn pxui_button(ui: *mut PxuiUi, label: Str, out: *mut u8) -> i32 {
    enter!(ui);
    let Some(label) = (unsafe { utf8(label) }) else { return unsafe { fail(ui, "invalid button arguments") } };
    unsafe { *out = engine(ui).button(label) as u8; }
    0
}

#[inline]
pub unsafe fn pxui_toggle(ui: *mut PxuiUi, label: Str, on: u8, out: *mut u8) -> i32 {
    enter!(ui);
    let Some(label) = (unsafe { utf8(label) }) else { return unsafe { fail(ui, "invalid toggle arguments") } };
    unsafe { *out = engine(ui).toggle(label, on != 0) as u8; }
    0
}

/// The plain kit toggle (`Toggle::DEFAULT`) keyed by `key`, `pxui_ui_toggle_plain`'s body: the engine's own default options, nothing built or checked
/// for them. A click is the flip of `on`, which the caller keeps.
#[inline(always)]
pub unsafe fn toggle_plain(ui: *mut PxuiUi, key: &str, text: &str, on: bool, out: &mut PxuiResponse) -> i32 {
    enter!(ui);
    if long(key) || long(text) { return unsafe { fail(ui, "invalid toggle text") }; }
    let r = unsafe { engine(ui) }.toggle_opts(key, text, on, &ToggleOpts::DEFAULT);
    respond(&r, out);
    0
}

/// The kit toggle with options, the body of `pxui_ui_toggle_response` and `pxui_ui_toggle_plain`: `track` and `knob` in place of the theme's
/// `switch` and `knob`, `row_h`, `switch_w`, `switch_h` and `inset` (finite, non-negative; 0 the kit's), `press_row` (a press anywhere on the row
/// toggles), `detail` a line under the text, `enabled`. `detail` is only read for the call.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn toggle_with(ui: *mut PxuiUi, key: &str, text: &str, on: bool, track: Option<&States>, knob: Option<&States>, row_h: f32, switch_w: f32, switch_h: f32, inset: f32,
    press_row: bool, detail: &str, enabled: bool, out: &mut PxuiResponse) -> i32 {
    enter!(ui);
    if long(key) || long(text) || long(detail) { return unsafe { fail(ui, "invalid toggle text") }; }
    // finite and not negative: NaN and infinity fail one of the two compares
    if ![row_h, switch_w, switch_h, inset].iter().all(|v| (0.0..=f32::MAX).contains(v)) { return unsafe { fail(ui, "invalid toggle descriptor") }; }
    // the looks are copied only when given
    // SAFETY: the engine reads `detail` while this call builds the row (it copies the text into its node); nothing keeps the reference
    let detail: &'static str = unsafe { core::mem::transmute::<&str, &'static str>(detail) };
    let mut opts = ToggleOpts { enabled, row_h: (row_h > 0.0).then_some(row_h), switch: (switch_w > 0.0 || switch_h > 0.0).then_some((switch_w, switch_h)), track: None, knob: None,
        detail, inset: (inset > 0.0).then_some(inset), press_row };
    if let Some(track) = track { opts.track = Some(*track); }
    if let Some(knob) = knob { opts.knob = Some(*knob); }
    let r = unsafe { engine(ui) }.toggle_opts(key, text, on, &opts);
    respond(&r, out);
    0
}

/// The plain kit segmented control (`Segmented::DEFAULT`) keyed by `key`, named `name`, `pxui_ui_segmented_plain`'s body. `chosen` is the choice
/// (`selected` clamped to `options`, moved by a press or a key); `options` are within the engine's limits (`options_ok`).
#[inline(always)]
pub unsafe fn segmented_plain(ui: *mut PxuiUi, key: &str, name: &str, options: &[&str], selected: usize, out: &mut PxuiResponse, chosen: &mut usize) -> i32 {
    enter!(ui);
    if long(key) || long(name) || !options_ok(options) { return unsafe { fail(ui, "invalid widget strings") }; }
    let (r, at) = unsafe { engine(ui) }.segmented_opts(key, name, options, selected, &SegmentedOpts::DEFAULT);
    respond(&r, out);
    *chosen = at;
    0
}

/// The kit segmented control with options, the body of `pxui_ui_segmented_response` and `pxui_ui_segmented_plain`: `h` (finite, non-negative; 0 the
/// theme's control height), `grow`, `states` in place of the theme's `segment`, `enabled`.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn segmented_with(ui: *mut PxuiUi, key: &str, name: &str, options: &[&str], selected: usize, enabled: bool, h: f32, grow: bool, states: Option<&States>,
    out: &mut PxuiResponse, chosen: &mut usize) -> i32 {
    enter!(ui);
    if long(key) || long(name) || !options_ok(options) { return unsafe { fail(ui, "invalid widget strings") }; }
    if !(0.0..=f32::MAX).contains(&h) { return unsafe { fail(ui, "invalid segmented descriptor") }; }
    let mut opts = SegmentedOpts { enabled, h: (h > 0.0).then_some(h), grow, states: None };
    if let Some(states) = states { opts.states = Some(*states); }
    let (r, at) = unsafe { engine(ui) }.segmented_opts(key, name, options, selected, &opts);
    respond(&r, out);
    *chosen = at;
    0
}

/// A text field: `out_changed` says whether the user changed the text, `changed` then holds the new text (borrowed until the next frame).
#[inline]
pub unsafe fn pxui_text_field(ui: *mut PxuiUi, label: Str, value: Str, changed: *mut Str, out_changed: *mut u8) -> i32 {
    enter!(ui);
    let (Some(label), Some(value)) = (unsafe { utf8(label) }, unsafe { utf8(value) }) else { return unsafe { fail(ui, "invalid text field arguments") } };
    unsafe { *out_changed = 0; *changed = Str::default(); }
    if let Some(value) = unsafe { engine(ui) }.text_field(label, value) { unsafe { *changed = (*ui).give(value); *out_changed = 1; } }
    0
}

/// The theme in effect, lent.
#[inline]
pub unsafe fn pxui_ui_theme(ui: *mut PxuiUi) -> *const ThemeSnapshot {
    enter!(ui, core::ptr::null());
    (unsafe { engine(ui) }.theme() as *const pxui_core::Theme).cast()
}

// ---- containers: their children run through `Inner`

#[inline]
pub unsafe fn within(ui: *mut PxuiUi, box_: *const PxuiBox, f: Inner) -> i32 {
    enter!(ui);
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    let shared = unsafe { (*ui).shared };
    e.within(&b, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

#[inline]
pub unsafe fn scope(ui: *mut PxuiUi, label: &str, f: Inner) -> i32 {
    enter!(ui);
    if label.len() > MAX_TEXT_BYTES { return unsafe { fail(ui, "invalid scope label") }; }
    let shared = unsafe { (*ui).shared };
    unsafe { engine(ui) }.scope(label, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

#[inline]
pub unsafe fn col(ui: *mut PxuiUi, padding: f32, label: &str, f: Inner) -> i32 {
    enter!(ui);
    if label.len() > MAX_TEXT_BYTES || !padding.is_finite() { return unsafe { fail(ui, "invalid column arguments") }; }
    let shared = unsafe { (*ui).shared };
    unsafe { engine(ui) }.col(padding, label, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

#[inline]
pub unsafe fn view(ui: *mut PxuiUi, label: &str, revision: u64, width: Size, height: Size, flat: bool, f: Inner, out: *mut PxuiBox) -> i32 {
    enter!(ui);
    if label.len() > MAX_TEXT_BYTES || !finite(width) || !finite(height) { return unsafe { fail(ui, "invalid view arguments") }; }
    let shared = unsafe { (*ui).shared };
    let engine = unsafe { engine(ui) };
    let mut build = |engine: &mut Ui| call_inner(shared, engine, f);
    let handle = if flat { engine.view_flat(label, revision, width, height, &mut build) } else { engine.view(label, revision, width, height, &mut build) };
    unsafe { *out = to_abi_box(handle); }
    0
}

/// A size whose length is a number (`size` of the descriptor refuses the same).
fn finite(size: Size) -> bool { !matches!(size, Size::Px(v) | Size::Pct(v) if !v.is_finite()) }

/// One frame over `input` (`Ui::frame`).
#[inline]
pub unsafe fn frame(ui: *mut PxuiUi, input: &FrameIn, f: Inner, out: &mut FrameOut) -> i32 { unsafe { crate::ui::frame_body(ui, input, f, out) } }

/// One frame over the host's whole input record (`Ui::input_frame`).
#[inline]
pub unsafe fn input_frame(ui: *mut PxuiUi, input: &Input, f: Inner, out: &mut FrameOut) -> i32 { unsafe { crate::ui::input_frame_body(ui, input, f, out) } }

// ---- widgets: Rust types in, so the wrapper passes its own `&str` and slices (no `Str` span to build, nothing to decode)

pub(crate) const MAX_OPTIONS: usize = 4096;

#[inline]
fn long(value: &str) -> bool { value.len() > MAX_TEXT_BYTES }

/// Whether `options` are within the engine's limits: at most `MAX_OPTIONS` of them, at most `MAX_TOTAL_TEXT_BYTES` in all.
#[inline]
fn options_ok(options: &[&str]) -> bool {
    options.len() <= MAX_OPTIONS && options.iter().fold(0usize, |total, o| total.saturating_add(o.len())) <= MAX_TOTAL_TEXT_BYTES && options.iter().all(|o| !long(o))
}

#[inline]
pub unsafe fn checkbox(ui: *mut PxuiUi, label: &str, value: bool, out: &mut bool) -> i32 {
    enter!(ui);
    if long(label) { return unsafe { fail(ui, "invalid checkbox label") }; }
    *out = unsafe { engine(ui) }.checkbox(label, value);
    0
}

/// `kind`: 0 radio, 1 tabs, 2 segmented, 3 dropdown.
#[inline]
pub unsafe fn choice(ui: *mut PxuiUi, kind: u32, label: &str, options: &[&str], selected: usize, out: &mut usize) -> i32 {
    enter!(ui);
    if long(label) || !options_ok(options) { return unsafe { fail(ui, "invalid widget strings") }; }
    let engine = unsafe { engine(ui) };
    *out = match kind { 0 => engine.radio(label, options, selected), 1 => engine.tabs(label, options, selected), 2 => engine.segmented(label, options, selected), _ => engine.dropdown(label, options, selected) };
    0
}

#[inline]
pub unsafe fn slider(ui: *mut PxuiUi, label: &str, value: f32, min: f32, max: f32, out: &mut f32) -> i32 {
    enter!(ui);
    if !value.is_finite() || !min.is_finite() || !max.is_finite() || min > max || long(label) { return unsafe { fail(ui, "invalid slider arguments") }; }
    *out = unsafe { engine(ui) }.slider(label, value, min..=max);
    0
}

#[inline]
pub unsafe fn progress(ui: *mut PxuiUi, label: &str, value: Option<f32>, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    if long(label) || value.is_some_and(|v| !v.is_finite()) { return unsafe { fail(ui, "invalid progress arguments") }; }
    *out = to_abi_box(unsafe { engine(ui) }.progress(label, value));
    0
}

#[inline]
pub unsafe fn spinner(ui: *mut PxuiUi, label: &str, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    if long(label) { return unsafe { fail(ui, "invalid spinner label") }; }
    *out = to_abi_box(unsafe { engine(ui) }.spinner(label));
    0
}

/// A button size `Button` and `IconButton` allow: the kit's (`Fit`), finite non-negative points or filling the line.
fn button_size(size: Size) -> bool { match size { Size::Fit | Size::Grow => true, Size::Px(v) => (0.0..=f32::MAX).contains(&v), _ => false } }

/// The engine's response as the record the wrapper's `Response` is read from.
#[inline(always)]
fn respond(r: &pxui_core::ui::Response, out: &mut PxuiResponse) {
    out.node = to_abi_box(r.handle);
    out.flags = r.clicked as u32 * PxuiResponse::CLICKED | r.hovered as u32 * PxuiResponse::HOVERED | r.held as u32 * PxuiResponse::HELD
        | r.focused as u32 * PxuiResponse::FOCUSED | r.activated as u32 * PxuiResponse::ACTIVATED;
}

/// The kit button with options, the body of `pxui_ui_button_with`, `pxui_ui_button_response` and `pxui_ui_button_plain`: `states` in
/// place of the theme's button, `w` and `h` (`button_size`), `justify` its place in the line, `pad` (finite, non-negative) inside the
/// box, `content` built in the box in place of `text`. What happened goes to `out`; with `signal`, the box's whole signal too.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn button_with(ui: *mut PxuiUi, key: &str, text: &str, name: Option<&str>, states: Option<&States>, w: Size, h: Size, justify: Justify, pad: Edges,
    enabled: bool, selected: bool, danger: bool, content: Option<Inner>, out: &mut PxuiResponse, signal: Option<&mut PxuiSignal>) -> i32 {
    enter!(ui);
    if long(key) || long(text) || name.is_some_and(long) { return unsafe { fail(ui, "invalid button text") }; }
    if !button_size(w) || !button_size(h) || ![pad.l, pad.t, pad.r, pad.b].iter().all(|v| (0.0..=f32::MAX).contains(v)) { return unsafe { fail(ui, "invalid button descriptor") }; }
    let (e, shared) = (unsafe { engine(ui) }, unsafe { (*ui).shared });
    let ink = if danger { Some(e.theme().tokens.invalid) } else { None };
    // the looks are copied only when given: `states.copied()` copies the whole record either way
    let mut opts = pxui_core::ui::ButtonOpts { states: None, w, h, enabled, selected, justify, padding: pad, ink, bare: false, icon: false };
    if let Some(states) = states { opts.states = Some(*states); }
    let mut run = content.map(|f| move |engine: &mut Ui| call_inner(shared, engine, &mut *f));
    let body = run.as_mut().map(|f| f as &mut dyn FnMut(&mut Ui));
    let r = match signal {
        None => e.button_opts(key, text, name, &opts, body, None),
        Some(signal) => {
            let mut read = pxui_core::ui::Signal::default();
            let r = e.button_opts(key, text, name, &opts, body, Some(&mut read));
            crate::interaction::write_borrowed_signal(unsafe { &*ui }, &read, unsafe { engine(ui) }.key_events_ref(&r.handle), signal);
            r
        }
    };
    respond(&r, out);
    0
}

/// The plain kit button (`Button::DEFAULT`) keyed by `key`, `pxui_ui_button_plain`'s body: `button_with` with the engine's own
/// default options, so nothing is built or checked for them.
#[inline(always)]
pub unsafe fn button_plain(ui: *mut PxuiUi, key: &str, text: &str, out: &mut PxuiResponse) -> i32 {
    enter!(ui);
    if long(key) || long(text) { return unsafe { fail(ui, "invalid button text") }; }
    let r = unsafe { engine(ui) }.button_opts(key, text, None, &pxui_core::ui::ButtonOpts::DEFAULT, None, None);
    respond(&r, out);
    0
}

/// The plain kit icon button (`IconButton::DEFAULT`) showing the glyph `glyph`, `pxui_ui_icon_button_plain`'s body: as
/// `button_plain`.
#[inline(always)]
pub unsafe fn icon_button_plain(ui: *mut PxuiUi, key: &str, glyph: &str, name: &str, out: &mut PxuiResponse) -> i32 {
    enter!(ui);
    if long(key) || long(glyph) || long(name) { return unsafe { fail(ui, "invalid icon button text") }; }
    let r = unsafe { engine(ui) }.icon_button_text(key, glyph, None, name, &pxui_core::ui::IconButton::DEFAULT, None);
    respond(&r, out);
    0
}

/// The kit icon button with options, the body of `pxui_ui_icon_button_with`, `_response` and `_plain`: the `glyph`, or the app's
/// drawing `draw` (a C callback and its `user`), named `name`; `states` in place of the theme's icon button, `w` and `h`
/// (`button_size`), `press_scale` finite and positive. What happened goes to `out`; with `signal`, the box's whole signal too.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn icon_button_with(ui: *mut PxuiUi, key: &str, glyph: &str, draw: pxui_types::ffi::PxuiIconDraw, user: *mut core::ffi::c_void, name: &str, states: Option<&States>,
    w: Size, h: Size, enabled: bool, press_scale: f32, out: &mut PxuiResponse, signal: Option<&mut PxuiSignal>) -> i32 {
    enter!(ui);
    if long(key) || long(glyph) || long(name) { return unsafe { fail(ui, "invalid icon button text") }; }
    if !button_size(w) || !button_size(h) || !press_scale.is_finite() || press_scale <= 0.0 { return unsafe { fail(ui, "invalid icon button descriptor") }; }
    let draw = draw.map(|f| (crate::paint::icon_bridge as pxui_core::ui::IconDraw, [f as usize, user as usize, unsafe { (*ui).shared } as usize]));
    let mut button = pxui_core::ui::IconButton { states: None, w, h, enabled, press_scale };
    if let Some(states) = states { button.states = Some(*states); }
    let e = unsafe { engine(ui) };
    let r = match signal {
        None => e.icon_button_text(key, glyph, draw, name, &button, None),
        Some(signal) => {
            let mut read = pxui_core::ui::Signal::default();
            let r = e.icon_button_text(key, glyph, draw, name, &button, Some(&mut read));
            crate::interaction::write_borrowed_signal(unsafe { &*ui }, &read, unsafe { engine(ui) }.key_events_ref(&r.handle), signal);
            r
        }
    };
    respond(&r, out);
    0
}

#[inline]
pub unsafe fn icon_button(ui: *mut PxuiUi, label: &str, icon: &str, out: &mut bool) -> i32 {
    enter!(ui);
    if long(label) || long(icon) { return unsafe { fail(ui, "invalid icon button text") }; }
    *out = unsafe { engine(ui) }.icon_button_text(label, icon, None, label, &pxui_core::ui::IconButton::DEFAULT, None).clicked;
    0
}

#[inline]
pub unsafe fn badge(ui: *mut PxuiUi, label: &str, value: &str, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    if long(label) || long(value) { return unsafe { fail(ui, "invalid badge text") }; }
    *out = to_abi_box(unsafe { engine(ui) }.badge(label, value));
    0
}

/// The text a search field changed to, or none; kept by the library until the next frame.
#[inline]
pub unsafe fn search_field(ui: *mut PxuiUi, label: &str, icon: &str, value: &str, placeholder: &str, out: &mut Option<Str>) -> i32 {
    enter!(ui);
    if long(label) || long(icon) || long(value) || long(placeholder) { return unsafe { fail(ui, "invalid search field text") }; }
    let found = unsafe { engine(ui) }.search_field(label, icon, value, placeholder);
    *out = found.map(|text| unsafe { get(ui) }.map_or(Str::default(), |state| state.lend(text)));
    0
}

/// `Ui::menu` of Rust rows: the position and the widths are finite by the checks below, the rows and the key are Rust values. The
/// static wrapper's entry; the `pxui_ui_context_menu` export decodes its columns into rows and calls this.
#[inline]
pub unsafe fn menu(ui: *mut PxuiUi, key: pxui_types::ui::Key, at: (f32, f32), items: &[pxui_types::ui::MenuItem], menu: &pxui_types::ui::Menu, out: &mut pxui_types::ui::MenuResponse) -> i32 {
    enter!(ui);
    let width_ok = menu.width.is_none_or(|width| width.is_finite() && width >= 0.0);
    let row_ok = menu.row_h.is_none_or(|row| row.is_finite() && row > 0.0);
    if !at.0.is_finite() || !at.1.is_finite() || !width_ok || !row_ok { return unsafe { fail(ui, "invalid context menu arguments") }; }
    *out = unsafe { engine(ui) }.menu(key, at, items, menu);
    0
}

#[inline]
pub unsafe fn tip(ui: *mut PxuiUi, box_: &PxuiBox, label: &str) -> i32 {
    enter!(ui);
    if long(label) { return unsafe { fail(ui, "invalid tooltip arguments") }; }
    unsafe { engine(ui) }.tip(&unbox(box_), label);
    0
}

#[inline]
fn unbox(raw: &PxuiBox) -> pxui_core::ui::UiBox { pxui_core::ui::UiBox { index: raw.index as usize, box_key: raw.box_key, box_slot: raw.box_slot as usize, generation: raw.generation } }

#[inline]
pub unsafe fn rect(ui: *mut PxuiUi, box_: &PxuiBox, out: &mut pxui_types::geom::Rect) -> i32 {
    enter!(ui);
    *out = unsafe { engine(ui) }.rect(&unbox(box_));
    0
}

#[inline]
pub unsafe fn box_of_key(ui: *mut PxuiUi, key: i64, out: &mut PxuiBox) -> bool {
    enter!(ui, false);
    let Some(found) = (unsafe { engine(ui) }).box_of_key(key) else { return false };
    *out = to_abi_box(found);
    true
}

// ---- the inspector rows and the footer

#[inline]
pub unsafe fn inspector_toggle(ui: *mut PxuiUi, key: &str, label: &str, value: bool, out: &mut bool) -> i32 {
    enter!(ui);
    if long(key) || long(label) { return unsafe { fail(ui, "invalid inspector toggle text") }; }
    *out = unsafe { engine(ui) }.inspector_toggle(key, label, value);
    0
}

#[inline]
pub unsafe fn inspector_button(ui: *mut PxuiUi, key: &str, label: &str, out: &mut bool) -> i32 {
    enter!(ui);
    if long(key) || long(label) { return unsafe { fail(ui, "invalid inspector button text") }; }
    *out = unsafe { engine(ui) }.inspector_button(key, label);
    0
}

#[inline]
pub unsafe fn inspector_readout(ui: *mut PxuiUi, width: Option<f32>, key: &str, label: &str, value: &str) -> i32 {
    enter!(ui);
    if long(key) || long(label) || long(value) || width.is_some_and(|w| !w.is_finite() || w < 0.0) { return unsafe { fail(ui, "invalid inspector readout text") }; }
    unsafe { engine(ui) }.inspector_readout(width, key, label, value);
    0
}

#[inline]
pub unsafe fn inspector_message(ui: *mut PxuiUi, key: &str, message: &str) -> i32 {
    enter!(ui);
    if long(key) || long(message) { return unsafe { fail(ui, "invalid inspector message text") }; }
    unsafe { engine(ui) }.inspector_message(key, message);
    0
}

/// The row's box and its control's x, y and width.
#[inline]
pub unsafe fn inspector_row(ui: *mut PxuiUi, width: Option<f32>, pin: Option<bool>, key: &str, label: &str, out: &mut (PxuiBox, f32, f32, f32)) -> i32 {
    enter!(ui);
    if long(key) || long(label) || width.is_some_and(|w| !w.is_finite() || w < 0.0) { return unsafe { fail(ui, "invalid inspector row text") }; }
    let (row, x, y, w) = unsafe { engine(ui) }.inspector_row(width, pin, key, label);
    *out = (to_abi_box(row), x, y, w);
    0
}

#[inline]
pub unsafe fn inspector_section(ui: *mut PxuiUi, key: &str, expanded: bool, set_expanded: Option<bool>, label: &str, f: Inner, out: &mut bool) -> i32 {
    enter!(ui);
    if long(key) || long(label) { return unsafe { fail(ui, "invalid inspector section text") }; }
    let shared = unsafe { (*ui).shared };
    *out = unsafe { engine(ui) }.inspector_section(key, expanded, set_expanded, label, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

/// The footer over the caller's borrowed rows: nothing is copied.
#[inline]
pub unsafe fn footer(ui: *mut PxuiUi, right: Option<&str>, hints: &[(&str, &str)]) -> i32 {
    enter!(ui);
    let total = right.map_or(0, str::len) + hints.iter().fold(0usize, |total, (k, v)| total.saturating_add(k.len()).saturating_add(v.len()));
    if hints.len() > 4096 || total > MAX_TOTAL_TEXT_BYTES || right.is_some_and(long) { return unsafe { fail(ui, "invalid footer text") }; }
    unsafe { engine(ui) }.footer(right, hints);
    0
}

// ---- containers with options

#[inline]
pub unsafe fn panel(ui: *mut PxuiUi, opts: pxui_types::ui::PanelOpts, label: &str, f: Inner) -> i32 {
    enter!(ui);
    if long(label) || !crate::state::finite(&[opts.x, opts.y, opts.width, opts.height.unwrap_or_default(), opts.max_height.unwrap_or_default()]) { return unsafe { fail(ui, "invalid panel descriptor") }; }
    let shared = unsafe { (*ui).shared };
    unsafe { engine(ui) }.panel(opts, label, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

#[inline]
pub unsafe fn scroll_view(ui: *mut PxuiUi, label: &str, width: Size, height: Size, f: Inner, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    if long(label) || !finite(width) || !finite(height) { return unsafe { fail(ui, "invalid scroll view arguments") }; }
    let shared = unsafe { (*ui).shared };
    *out = to_abi_box(unsafe { engine(ui) }.scroll_view(label, width, height, &mut |engine: &mut Ui| call_inner(shared, engine, f)));
    0
}

// ---- collections: the engine asks `text` for a row's text and the entry checks what it hands back

const MAX_ITEMS: usize = 1_000_000;


/// `put` that refuses (and records) a text over `MAX_TEXT_BYTES`, or past `MAX_TOTAL_TEXT_BYTES` in all, instead of passing it on.
#[inline]
fn bounded(put: &mut dyn FnMut(&str), bad: &core::cell::Cell<bool>, total: &core::cell::Cell<usize>, value: &str) {
    let sum = total.get().saturating_add(value.len());
    if long(value) || sum > MAX_TOTAL_TEXT_BYTES { bad.set(true); return; }
    total.set(sum);
    put(value);
}

/// The C text-row `list_view` (`pxui_ui_list_view`): each row's text from `text`, the kit's row label (rows are not named).
#[inline]
pub unsafe fn list_view_text(ui: *mut PxuiUi, label: &str, count: usize, multi: bool, selected: &mut [u8], text: &mut dyn FnMut(usize, &mut dyn FnMut(&str)), out: &mut Option<usize>) -> i32 {
    enter!(ui);
    if long(label) || count > MAX_ITEMS || selected.len() != count { return unsafe { fail(ui, "invalid list view descriptor") }; }
    let (bad, total, text) = (core::cell::Cell::new(false), core::cell::Cell::new(0), core::cell::RefCell::new(text));
    *out = unsafe { engine(ui) }.list_view(label, count, multi, selected, &mut |ui, row| (text.borrow_mut())(row, &mut |value| bounded(&mut |t| { ui.row_text(t); }, &bad, &total, value)));
    if bad.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    0
}

/// The C text-row `tree` (`pxui_ui_tree`): a preorder tree; `visible` holds a row per node, `open` and `nodes` one flag and one node each.
#[inline]
pub unsafe fn tree_text(ui: *mut PxuiUi, label: &str, nodes: &[pxui_types::ffi::PxuiTreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], text: &mut dyn FnMut(usize, &mut dyn FnMut(&str)), out: &mut (Option<usize>, usize)) -> i32 {
    enter!(ui);
    if long(label) || nodes.len() > MAX_ITEMS || open.len() != nodes.len() || visible.len() < nodes.len() { return unsafe { fail(ui, "invalid tree descriptor") }; }
    // the engine's node and the C node share a layout (asserted where the exports are), so the caller's array is the engine's slice
    let nodes = unsafe { core::slice::from_raw_parts(nodes.as_ptr().cast::<pxui_core::ui_widgets::TreeNode>(), nodes.len()) };
    let (bad, total, text) = (core::cell::Cell::new(false), core::cell::Cell::new(0), core::cell::RefCell::new(text));
    *out = unsafe { engine(ui) }.tree(label, nodes, open, selected, visible, &mut |ui, row| (text.borrow_mut())(row, &mut |value| bounded(&mut |t| ui.item_text(t), &bad, &total, value)));
    if bad.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    0
}

/// The C text-cell `table_view` (`pxui_ui_table_view`): each cell's text from `cell`, naming the cell.
#[inline]
pub unsafe fn table_view_text(ui: *mut PxuiUi, label: &str, titles: &[&str], widths: &mut [f32], rows: usize, sort: Option<(usize, bool)>, selected: &mut Option<usize>, cell: &mut dyn FnMut(usize, usize, &mut dyn FnMut(&str)), out: &mut Option<usize>) -> i32 {
    enter!(ui);
    if long(label) || rows > MAX_ITEMS || titles.len() > 4096 || widths.len() != titles.len() || !options_ok(titles) { return unsafe { fail(ui, "invalid table descriptor") }; }
    if widths.iter().any(|w| !w.is_finite() || *w < 0.0) || sort.is_some_and(|(column, _)| column >= titles.len()) { return unsafe { fail(ui, "invalid table widths or sort column") }; }
    let (bad, total, cell) = (core::cell::Cell::new(false), core::cell::Cell::new(0), core::cell::RefCell::new(cell));
    *out = unsafe { engine(ui) }.table_view(label, titles, widths, rows, sort, selected, &mut |ui, row, column| (cell.borrow_mut())(row, column, &mut |value| bounded(&mut |t| ui.cell_text(t), &bad, &total, value)));
    if bad.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    0
}

// ---- more containers and callbacks

/// Measured rows share the existing borrowed-handle callback protocol with virtual grids.
#[inline]
pub unsafe fn list_measured(ui:*mut PxuiUi,label:&str,count:usize,estimate_h:f32,f:&mut dyn FnMut(*mut PxuiUi,usize),out:&mut PxuiBox)->i32 {
    enter!(ui);
    if !unsafe { (*(*ui).shared).in_frame } { return crate::PXUI_ERR_STATE; }
    if long(label) || count>MAX_ITEMS || !estimate_h.is_finite() || estimate_h<=0.0 || !(count as f32*estimate_h).is_finite() { return unsafe { fail(ui,"invalid measured list arguments") }; }
    let shared=unsafe { (*ui).shared };
    *out=to_abi_box(unsafe { engine(ui) }.list_measured(label,count,estimate_h,&mut |engine:&mut Ui,index| {
        let mut borrowed=PxuiUi::borrowed(engine,shared);
        let _current=crate::state::Current::set(shared,&mut borrowed);
        f(&mut borrowed,index);
    }));
    0
}

/// One fixed-height section, sharing the existing borrowed grid callback protocol.
#[inline]
pub unsafe fn section_list(ui:*mut PxuiUi,label:&str,section:&pxui_types::ui::SectionList,key:Option<&dyn Fn(usize)->i64>,f:&mut dyn FnMut(*mut PxuiUi,usize),out:&mut PxuiBox)->i32 {
    enter!(ui);
    if !unsafe { (*(*ui).shared).in_frame } {return crate::PXUI_ERR_STATE;}
    let total=section.leading_height+section.header_height+section.count as f32*section.row_height+section.trailing_height;
    if long(label) || section.count>MAX_ITEMS || !section.row_height.is_finite() || section.row_height<=0.0
        || !total.is_finite() || [section.leading_height,section.header_height,section.trailing_height].iter().any(|v|!v.is_finite() || *v<0.0) {return unsafe {fail(ui,"invalid section list geometry")};}
    let shared=unsafe {(*ui).shared};
    *out=to_abi_box(unsafe {engine(ui)}.section_list(label,section,key,&mut |engine:&mut Ui,index| {
        let mut borrowed=PxuiUi::borrowed(engine,shared);
        let _current=crate::state::Current::set(shared,&mut borrowed);
        f(&mut borrowed,index);
    }));
    0
}

/// A section with measured rows and their logical content positions.
#[inline]
pub unsafe fn section_list_measured(ui:*mut PxuiUi,label:&str,section:&pxui_types::ui::SectionList,key:Option<&dyn Fn(usize)->i64>,f:&mut dyn FnMut(*mut PxuiUi,usize,f32),out:&mut PxuiBox)->i32 {
    enter!(ui);
    if !unsafe { (*(*ui).shared).in_frame } {return crate::PXUI_ERR_STATE;}
    let total=section.leading_height+section.header_height+section.count as f32*section.row_height+section.trailing_height;
    if long(label) || section.count>MAX_ITEMS || !section.row_height.is_finite() || section.row_height<=0.0
        || !total.is_finite() || [section.leading_height,section.header_height,section.trailing_height].iter().any(|v|!v.is_finite() || *v<0.0) {return unsafe {fail(ui,"invalid section list geometry")};}
    let shared=unsafe {(*ui).shared};
    *out=to_abi_box(unsafe {engine(ui)}.section_list_measured(label,section,key,&mut |engine:&mut Ui,index,y| {
        let mut borrowed=PxuiUi::borrowed(engine,shared);
        let _current=crate::state::Current::set(shared,&mut borrowed);
        f(&mut borrowed,index,y);
    }));
    0
}

/// A grid of `count` cells, built on demand: `f` gets the borrowed handle and the cell's index.
#[inline]
pub unsafe fn grid_list(ui: *mut PxuiUi, label: &str, count: usize, cell_w: f32, cell_h: f32, f: &mut dyn FnMut(*mut PxuiUi, usize), out: &mut PxuiBox) -> i32 {
    let shared = unsafe { (*ui).shared };
    unsafe { grid_cells(ui, label, count, cell_w, cell_h, &mut |engine: &mut Ui, index| { let mut borrowed = PxuiUi::borrowed(engine, shared); let _current = crate::state::Current::set(shared, &mut borrowed); f(&mut borrowed, index) }, out) }
}

/// `grid_list` with the engine's own cell closure: the export's makes the borrowed handle and calls C's callback itself, one
/// indirect call a cell fewer than through `f`.
#[inline(always)]
pub(crate) unsafe fn grid_cells(ui: *mut PxuiUi, label: &str, count: usize, cell_w: f32, cell_h: f32, cell: &mut dyn FnMut(&mut Ui, usize), out: &mut PxuiBox) -> i32 {
    enter!(ui);
    if long(label) || count > MAX_ITEMS || !cell_w.is_finite() || !cell_h.is_finite() || cell_w <= 0.0 || cell_h <= 0.0 { return unsafe { fail(ui, "invalid grid list descriptor") }; }
    *out = to_abi_box(unsafe { engine(ui) }.grid_list(label, count, cell_w, cell_h, cell));
    0
}

/// Two panes in a row (`axis` 0) or a column (1); `changed` is the new ratio when the user moved the divider.
#[inline]
pub unsafe fn split(ui: *mut PxuiUi, label: &str, axis: pxui_types::Axis, ratio: f32, min_a: f32, min_b: f32, a: Inner, b: Inner, out: &mut Option<f32>) -> i32 {
    enter!(ui);
    if long(label) || ![ratio, min_a, min_b].iter().all(|v| v.is_finite()) || min_a < 0.0 || min_b < 0.0 || axis == pxui_types::Axis::Stack { return unsafe { fail(ui, "invalid split descriptor") }; }
    let shared = unsafe { (*ui).shared };
    *out = unsafe { engine(ui) }.split(label, axis, ratio, min_a, min_b, &mut |engine: &mut Ui| call_inner(shared, engine, a), &mut |engine: &mut Ui| call_inner(shared, engine, b));
    0
}

#[inline]
pub unsafe fn anchored(ui: *mut PxuiUi, label: &str, anchor: pxui_types::geom::Rect, placement: pxui_types::layout::Placement, width: Size, height: Size, f: Inner, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    if long(label) || !finite(width) || !finite(height) || anchor.w < 0.0 || anchor.h < 0.0 || placement.margin < 0.0
        || ![anchor.x, anchor.y, anchor.w, anchor.h, placement.gap, placement.margin].iter().all(|v| v.is_finite()) { return unsafe { fail(ui, "invalid anchored arguments") }; }
    let shared = unsafe { (*ui).shared };
    *out = to_abi_box(unsafe { engine(ui) }.anchored(label, anchor, placement, width, height, &mut |engine: &mut Ui| call_inner(shared, engine, f)));
    0
}

#[inline]
pub unsafe fn modal(ui: *mut PxuiUi, label: &str, width: Option<f32>, f: Inner, out: &mut bool) -> i32 {
    enter!(ui);
    if long(label) || width.is_some_and(|w| !w.is_finite() || w < 0.0) { return unsafe { fail(ui, "invalid modal arguments") }; }
    let shared = unsafe { (*ui).shared };
    *out = unsafe { engine(ui) }.modal(width, label, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

/// A table of text cells at `at`; the clicked row, if any. `cell` writes a cell's text through `put`.
#[inline]
pub unsafe fn table(ui: *mut PxuiUi, at: (f32, f32), width: f32, height: f32, headers: &[&str], rows: i64, cell: &mut dyn FnMut(i64, i64, &mut dyn FnMut(&str)), label: &str, out: &mut Option<i64>) -> i32 {
    enter!(ui);
    if long(label) || headers.len() > 256 || rows < 0 || rows as usize > MAX_ITEMS || !width.is_finite() || !height.is_finite() || width < 0.0 || height < 0.0 || !at.0.is_finite() || !at.1.is_finite() || !options_ok(headers) {
        return unsafe { fail(ui, "invalid table descriptor") };
    }
    let (bad, total) = (core::cell::Cell::new(false), core::cell::Cell::new(0));
    *out = unsafe { engine(ui) }.table_with(at, width, height, headers, rows, &mut |row, column, put| cell(row, column, &mut |value| bounded(put, &bad, &total, value)), label).0;
    if bad.get() { return unsafe { fail(ui, "invalid table cell callback text") }; }
    0
}

/// `pxui_slot` as the static wrapper calls it: the caller's content runs inside, and a panic of it unwinds to the app as in every trusted entry.
/// The kit's text of the collection item being built (`Ui::item_text`); refused outside a `list_view`/`tree` row or `table_view` cell.
#[inline]
pub unsafe fn item_text(ui: *mut PxuiUi, text: &str) -> i32 {
    enter!(ui);
    if long(text) { return unsafe { fail(ui, "invalid item text") }; }
    if !unsafe { engine(ui) }.in_item() { return unsafe { fail(ui, "item_text outside a collection row or cell") }; }
    unsafe { engine(ui) }.item_text(text);
    0
}

#[inline]
pub unsafe fn pxui_slot(ui: *mut PxuiUi, slot: pxui_types::ffi::PxuiSlot, out: *mut u8) -> i32 { unsafe { crate::bodies::slot(ui, slot, out) } }

/// `list_view`: `cell(ui, row, 0)` builds the children of row `row`, `ui` the borrowed handle.
#[inline]
pub unsafe fn list_view(ui: *mut PxuiUi, label: &str, count: usize, multi: bool, selected: &mut [u8], cell: &mut dyn FnMut(*mut PxuiUi, usize, usize), out: &mut Option<usize>) -> i32 {
    enter!(ui);
    if long(label) || count > MAX_ITEMS || selected.len() != count { return unsafe { fail(ui, "invalid list view descriptor") }; }
    let shared = unsafe { (*ui).shared };
    *out = unsafe { engine(ui) }.list_view(label, count, multi, selected, &mut |engine, row| { let mut borrowed = PxuiUi::borrowed(engine, shared); let _current = crate::state::Current::set(shared, &mut borrowed); cell(&mut borrowed, row, 0) });
    0
}

/// `tree`: `cell(ui, node, 0)` builds the children of the row that shows tree node `node`.
#[inline]
pub unsafe fn tree(ui: *mut PxuiUi, label: &str, nodes: &[pxui_types::ffi::PxuiTreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], cell: &mut dyn FnMut(*mut PxuiUi, usize, usize), out: &mut (Option<usize>, usize)) -> i32 {
    enter!(ui);
    if long(label) || nodes.len() > MAX_ITEMS || open.len() != nodes.len() || visible.len() < nodes.len() { return unsafe { fail(ui, "invalid tree descriptor") }; }
    let nodes = unsafe { core::slice::from_raw_parts(nodes.as_ptr().cast::<pxui_core::ui_widgets::TreeNode>(), nodes.len()) };
    let shared = unsafe { (*ui).shared };
    *out = unsafe { engine(ui) }.tree(label, nodes, open, selected, visible, &mut |engine, node| { let mut borrowed = PxuiUi::borrowed(engine, shared); let _current = crate::state::Current::set(shared, &mut borrowed); cell(&mut borrowed, node, 0) });
    0
}

/// `table_view`: `cell(ui, row, column)` builds the children of one cell.
#[inline]
pub unsafe fn table_view(ui: *mut PxuiUi, label: &str, titles: &[&str], widths: &mut [f32], rows: usize, sort: Option<(usize, bool)>, selected: &mut Option<usize>, cell: &mut dyn FnMut(*mut PxuiUi, usize, usize), out: &mut Option<usize>) -> i32 {
    enter!(ui);
    if long(label) || rows > MAX_ITEMS || titles.len() > 4096 || widths.len() != titles.len() || !options_ok(titles) { return unsafe { fail(ui, "invalid table descriptor") }; }
    if widths.iter().any(|w| !w.is_finite() || *w < 0.0) || sort.is_some_and(|(column, _)| column >= titles.len()) { return unsafe { fail(ui, "invalid table widths or sort column") }; }
    let shared = unsafe { (*ui).shared };
    *out = unsafe { engine(ui) }.table_view(label, titles, widths, rows, sort, selected, &mut |engine, row, column| { let mut borrowed = PxuiUi::borrowed(engine, shared); let _current = crate::state::Current::set(shared, &mut borrowed); cell(&mut borrowed, row, column) });
    0
}
