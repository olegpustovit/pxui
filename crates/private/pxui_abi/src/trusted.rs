//! The trusted entries: the bodies of the hot exports, minus what only an untrusted caller needs. The static wrapper's caller is Rust, so
//! its strings are UTF-8, its enums in range, its references non-null and a panic unwinds through these frames as it did through the engine
//! itself. A `pxui_*` export of the same name checks what C hands over (poison, null, UTF-8, tags) and then calls its entry here, so the
//! two paths run one body. What Rust types cannot promise stays checked in the body (a text past its role's cap, `state::MAX_*_BYTES`, a
//! stale box, a number that is not finite). Each entry counts itself as one call into the library. A builder entry is refused outside a
//! frame (`enter!`).
use crate::node::declare;
use crate::state::{get, Inner, MAX_KEY_BYTES, MAX_LABEL_BYTES, MAX_TEXT_BYTES, MAX_TOTAL_TEXT_BYTES, PxuiUi, boxed, fail, note_call, to_abi_box};
use crate::{PxuiBox, PxuiSignal, Str};
use pxui_types::ffi::{PxuiResponse, PxuiTextStyle};
use pxui_types::{text::LabelFields, TextGlyph, TextMetrics, TextSpans};
use pxui_core::ui::{Segment, SegmentedOpts, ToggleOpts, Ui};
use pxui_types::ui::{Checkbox, Overlay, Slider};
use pxui_types::{Axis, FrameIn, FrameOut, Node, Size, ffi::Input, geom::Edges, layout::{Along, Justify}, theme::{State, States, ThemeSnapshot}};
/// What the static wrapper's callbacks are given and build their handle from (`Borrowed`): the engine of the callback, and a
/// selection list or table row's record (`#[repr(C)]`, the wrapper's `CellState` byte for byte).
pub use crate::state::Borrowed;
pub use pxui_core::ui::Ui as Engine;
pub use pxui_core::ui_lists::ListCell;
/// What a plain button, icon button or toggle entry reports (`#[repr(C)]`): the static wrapper reads it as it is, a C export as
/// its `PxuiResponse` (`respond`).
pub use pxui_core::ui::Response as EngineResponse;
/// The engine's handle inside `EngineResponse` (`#[repr(C)]`, the wrapper's `Handle` byte for byte).
pub use pxui_core::ui::UiBox as EngineBox;


/// `value` as text the caller promises is UTF-8, at most `max` bytes (the role's cap the exports refuse above). Its pointer is never
/// null: a Rust `str`'s, or the pair an export checked (`state::pair`, an empty text included).
#[inline]
unsafe fn utf8<'a>(value: Str, max: usize) -> Option<&'a str> {
    if value.len > max { return None; }
    Some(unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(value.ptr, value.len)) })
}
#[inline]
unsafe fn engine<'a>(ui: *mut PxuiUi) -> &'a mut Ui { unsafe { &mut *(*ui).engine } }

#[inline]
pub unsafe fn pxui_ui_node(ui: *mut PxuiUi, label: Str, desc: &Node, out: *mut PxuiBox) -> i32 {
    let Some(label) = (unsafe { utf8(label, MAX_KEY_BYTES) }).filter(|l| !crate::state::unusable(l)) else { unsafe { note_call((*ui).shared) }; return unsafe { fail(ui, "invalid node label") } };
    unsafe { node(ui, label, desc, out) }
}

/// `Transition::valid`, out of line: a node without a transition (the most) pays one compare in its inline entry.
#[inline(never)]
fn valid_curve(curve: &pxui_types::node::Transition) -> bool { curve.valid() }

/// `pxui_ui_node` of a Rust label (`Key::label`): UTF-8 by its type, so only its length is checked (the static wrapper's entry).
#[inline(always)]
pub unsafe fn node_str(ui: *mut PxuiUi, label: &str, desc: &Node, out: *mut PxuiBox) -> i32 {
    if bad_key(label) { unsafe { note_call((*ui).shared) }; return unsafe { fail(ui, "invalid node label") } }
    unsafe { node(ui, label, desc, out) }
}

/// `node_str` of a label the static wrapper checked (`authored`: never below 0x20, so never a resolved key's form): only its length.
/// Inline, the common node (a live building handle, no transition) is one compare and the engine's call; anything else takes
/// `node_label_checked`'s way, out of line.
#[inline(always)]
pub unsafe fn node_label(ui: *mut PxuiUi, label: &str, desc: &Node, out: *mut PxuiBox) -> i32 {
    if ui.is_null() || label.len() > MAX_KEY_BYTES || unsafe { (*(*ui).shared).building } != ui || desc.transition.is_some() { return unsafe { node_label_checked(ui, label, desc, out) } }
    unsafe { (*(*ui).shared).calls = (*(*ui).shared).calls.wrapping_add(1) };
    declare(unsafe { engine(ui) }, Some(label), 0, desc, unsafe { &mut *out });
    0
}

/// `node_label` with every check and refusal.
#[inline(never)]
unsafe fn node_label_checked(ui: *mut PxuiUi, label: &str, desc: &Node, out: *mut PxuiBox) -> i32 {
    if !ui.is_null() && label.len() > MAX_KEY_BYTES { unsafe { note_call((*ui).shared) }; return unsafe { fail(ui, "invalid node label") } }
    unsafe { node(ui, label, desc, out) }
}

/// `pxui_ui_node` of a label already checked (UTF-8, at most `MAX_KEY_BYTES`): the export's entry, which checks it as C hands it over.
#[inline(always)]
pub(crate) unsafe fn node(ui: *mut PxuiUi, label: &str, desc: &Node, out: *mut PxuiBox) -> i32 {
    enter!(ui);
    if let pxui_types::node::OptTransition::Some(curve) = &desc.transition { if !valid_curve(curve) { return unsafe { fail(ui, "invalid transition curve") }; } }
    declare(unsafe { engine(ui) }, Some(label), 0, desc, unsafe { &mut *out });
    0
}

/// `pxui_ui_layout_node` of a checked node: a keyless layout container (see `Ui::layout_node`).
#[inline]
pub unsafe fn layout_node(ui: *mut PxuiUi, desc: &Node, out: *mut PxuiBox) -> i32 {
    enter!(ui);
    if let pxui_types::node::OptTransition::Some(curve) = &desc.transition { if !valid_curve(curve) { return unsafe { fail(ui, "invalid transition curve") }; } }
    declare(unsafe { engine(ui) }, None, 0, desc, unsafe { &mut *out });
    0
}

#[inline]
pub unsafe fn pxui_ui_node_keyed(ui: *mut PxuiUi, key: i64, desc: &Node, out: *mut PxuiBox) -> i32 {
    enter!(ui);
    if key == 0 || key == 1 { return unsafe { fail(ui, "reserved node key; derive integer keys with int_key") }; }
    if let pxui_types::node::OptTransition::Some(curve) = &desc.transition { if !valid_curve(curve) { return unsafe { fail(ui, "invalid transition curve") }; } }
    declare(unsafe { engine(ui) }, None, key, desc, unsafe { &mut *out });
    0
}

/// `pxui_part` of a node the caller's type vouches for (the static wrapper's `part`): the part's checks as the export makes them (a body
/// is running, or a `Part::HANDLE` in a reorderable row; a known part), then the node declared under the part's key as
/// `pxui_ui_node_keyed` declares one, counted as one call. The export keeps its own way for a C record (checked, grid tracks copied).
#[inline]
pub unsafe fn part(ui: *mut PxuiUi, part: u32, desc: &Node, out: *mut PxuiBox) -> i32 {
    use pxui_types::theme::Part;
    enter!(ui);
    let e = unsafe { engine(ui) };
    if unsafe { (*ui).shared() }.bodies.depth == 0 && !(part == Part::HANDLE.0 && e.in_movable_row()) { return unsafe { crate::state::fail_code(ui, "a part is built inside a body", crate::PXUI_ERR_STATE) } }
    if !Part::valid(part) { return unsafe { fail(ui, "invalid part number") } }
    if let pxui_types::node::OptTransition::Some(curve) = &desc.transition { if !valid_curve(curve) { return unsafe { fail(ui, "invalid transition curve") }; } }
    let key = e.own_part_key(Part(part));
    if key == 0 || key == 1 { return unsafe { fail(ui, "reserved node key; derive integer keys with int_key") }; }
    declare(e, None, key, desc, unsafe { &mut *out });
    if let Some(b) = crate::state::core_box(unsafe { *out }) { e.note_part(Part(part), b) }
    0
}

/// `pxui_ui_drop_target`'s answer for box `b` when nothing is carried or dropped this frame (the static wrapper's `drop_target` asks
/// first): `false` is that answer, no target, counted as the call; `true` (a `Ui` that is not usable, a box the export refuses as
/// stale, or a pending drop) sends the caller to the export, which checks the box and lends the payload.
#[inline]
pub unsafe fn drop_pending(ui: *mut PxuiUi, b: *const PxuiBox) -> bool {
    if ui.is_null() || b.is_null() || !unsafe { crate::state::usable(ui) } { return true; }
    let e = unsafe { engine(ui) };
    if e.drop_pending() || crate::state::live_box(e, unsafe { *b }).is_none() { return true; }
    unsafe { note_call((*ui).shared) };
    false
}

#[inline]
pub unsafe fn pxui_ui_text(ui: *mut PxuiUi, box_: *const PxuiBox, value: Str) -> i32 {
    let Some(value) = (unsafe { utf8(value, usize::MAX) }).map(clip) else { unsafe { note_call((*ui).shared) }; return unsafe { fail(ui, "invalid text") } };
    unsafe { text(ui, box_, value) }
}

/// `pxui_ui_text` of a value already checked (UTF-8, at most `MAX_LABEL_BYTES`), as `node`.
#[inline(always)]
pub(crate) unsafe fn text(ui: *mut PxuiUi, box_: *const PxuiBox, value: &str) -> i32 {
    enter!(ui);
    let raw = unsafe { *box_ };
    unsafe { engine(ui) }.text(&pxui_core::ui::UiBox { index: raw.index as usize, box_key: raw.box_key, box_slot: raw.box_slot as usize, generation: raw.generation }, value);
    0
}

#[inline(always)]
pub unsafe fn pxui_ui_label(ui: *mut PxuiUi, text: Str, style: &pxui_types::text::LabelFields, digits: bool, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    let Some(text) = (unsafe { utf8(text, usize::MAX) }).map(clip) else { return unsafe { fail(ui, "invalid label text") } };
    let Some(node) = unsafe { engine(ui) }.label_styled(text, style, digits) else { return unsafe { fail(ui, "invalid label style: a size, line height, box height, padding, width, wrap, align, ellipsis or feature no style has, or a font not loaded") }; };
    *out = to_abi_box(node);
    0
}

/// The plain kit toggle (`Toggle::DEFAULT`) keyed by `key`, `pxui_ui_toggle_plain`'s body: the engine's own default options, nothing built or checked
/// for them. A click is the flip of `on`, which the caller keeps.
#[inline(always)]
pub unsafe fn toggle_plain(ui: *mut PxuiUi, key: &str, text: &str, on: bool, out: &mut EngineResponse) -> i32 {
    enter!(ui);
    let text = clip(text);
    if bad_key(key) { return unsafe { fail(ui, "invalid toggle text") }; }
    let r = unsafe { engine(ui) }.toggle_opts(key, text, on, &ToggleOpts::DEFAULT);
    *out = r;
    0
}

/// The kit toggle with options, the body of `pxui_ui_toggle`: `track` and `knob` in place of the theme's
/// `switch` and `knob`, `row_h`, `switch_w`, `switch_h` and `inset` (finite, non-negative; 0 the kit's), `press_row` (a press anywhere on the row
/// toggles), `detail` a line under the text, `label` the title's text style (`None`: the theme's toggle font; one that does not resolve is
/// refused), `enabled`. `detail` is only read for the call.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn toggle_with(ui: *mut PxuiUi, key: &str, text: &str, on: bool, track: Option<&States>, knob: Option<&States>, row_h: f32, switch_w: f32, switch_h: f32, inset: f32,
    press_row: bool, detail: &str, label: Option<&pxui_types::text::LabelFields>, enabled: bool, out: &mut PxuiResponse) -> i32 {
    enter!(ui);
    let text = clip(text);
    let detail = clip(detail);
    if bad_key(key) { return unsafe { fail(ui, "invalid toggle text") }; }
    // finite and not negative: NaN and infinity fail one of the two compares
    if ![row_h, switch_w, switch_h, inset].iter().all(|v| (0.0..=f32::MAX).contains(v)) { return unsafe { fail(ui, "invalid toggle descriptor") }; }
    if label.is_some_and(|l| unsafe { engine(ui) }.resolve(l).is_none()) { return unsafe { fail(ui, "invalid toggle label style") }; }
    // the looks are copied only when given
    // SAFETY: the engine reads `detail` while this call builds the row (it copies the text into its node); nothing keeps the reference
    let detail: &'static str = unsafe { core::mem::transmute::<&str, &'static str>(detail) };
    let mut opts = ToggleOpts { enabled, row_h: (row_h > 0.0).then_some(row_h), switch: (switch_w > 0.0 || switch_h > 0.0).then_some((switch_w, switch_h)), track: None, knob: None,
        detail, inset: (inset > 0.0).then_some(inset), press_row, label: label.copied() };
    if let Some(track) = track { opts.track = Some(*track); }
    if let Some(knob) = knob { opts.knob = Some(*knob); }
    let r = unsafe { engine(ui) }.toggle_opts(key, text, on, &opts);
    respond(&r, out);
    0
}

/// `segmented` for `pxui_ui_segmented`, which has checked the key, the name and every item as C hands them over (`checked_key`,
/// `checked`, `checked_items`): only the handle and the numbers are checked here. `item(i)` reads item `i` where it lies.
#[allow(clippy::too_many_arguments)]
#[inline]
pub(crate) unsafe fn segmented_checked<'a>(ui: *mut PxuiUi, key: &str, name: &str, count: usize, item: &dyn Fn(usize) -> Segment<'a>, selected: Option<usize>, enabled: bool, h: f32, grow: bool,
    states: Option<&States>, w: f32, padding: f32, min_segment: f32, focus_on_click: bool, out: &mut PxuiResponse, chosen: &mut Option<usize>, secondary: &mut Option<usize>) -> i32 {
    enter!(ui);
    let mut r = EngineResponse::default();
    let rc = unsafe { segmented_body(ui, key, name, count, item, selected, enabled, h, grow, states, w, padding, min_segment, focus_on_click, &mut r, chosen, secondary) };
    if rc == 0 { respond(&r, out); }
    rc
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
unsafe fn segmented_body<'a>(ui: *mut PxuiUi, key: &str, name: &str, count: usize, item: &dyn Fn(usize) -> Segment<'a>, selected: Option<usize>, enabled: bool, h: f32, grow: bool,
    states: Option<&States>, w: f32, padding: f32, min_segment: f32, focus_on_click: bool, out: &mut EngineResponse, chosen: &mut Option<usize>, secondary: &mut Option<usize>) -> i32 {
    if ![h, w, padding, min_segment].iter().all(|v| (0.0..=f32::MAX).contains(v)) { return unsafe { fail(ui, "invalid segmented descriptor") }; }
    let mut opts = SegmentedOpts { enabled, h: (h > 0.0).then_some(h), grow, states: None, w: (w > 0.0).then_some(w), padding, min_segment, focus_on_click };
    if let Some(states) = states { opts.states = Some(*states); }
    let (r, at, pressed) = unsafe { engine(ui) }.segmented_options(key, name, count, item, selected, &opts);
    *out = r;
    (*chosen, *secondary) = (at, pressed);
    0
}

/// A text field keyed by `key`, named `name` (unnamed when empty), `traits` its keyboard: `changed` takes the new text (borrowed until the
/// next frame) when the user changed it; `out` what happened to its box. `pxui_ui_text_field`'s body; the key is the caller's checked one.
#[inline]
pub unsafe fn text_field(ui: *mut PxuiUi, key: &str, name: &str, value: &str, traits: pxui_types::TextTraits, changed: &mut Option<Str>, out: &mut PxuiResponse) -> i32 {
    enter!(ui);
    let name = clip(name);
    if bad_key(key) || value.len() > MAX_TEXT_BYTES { return unsafe { fail(ui, "invalid text field text") }; }
    *changed = None;
    if let Some(text) = unsafe { engine(ui) }.text_field_keyed(key, name, value, traits) {
        let Ok(text) = (unsafe { &*ui }).give(text) else { return crate::PXUI_ERR_STATE };
        *changed = Some(text);
    }
    unsafe { input_response(ui, None, out) }
}

/// A Rust `&str` the static wrapper lent as a `Str` (UTF-8 by its type).
unsafe fn lent<'a>(value: Str) -> &'a str {
    if value.len == 0 { "" } else { unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(value.ptr, value.len)) } }
}

/// `pxui_ui_value_field` for the static wrapper: the descriptor's strings are Rust `&str`s (UTF-8 by their type, display text
/// clipped by the wrapper) and the callbacks Rust closures the engine calls directly. `changed` takes the new text (the engine's, borrowed
/// until the next call on the `Ui`), `editing` whether a text editor is open, `out` what happened to the field's box.
#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn value_field(ui: *mut PxuiUi, d: &pxui_types::ffi::PxuiValueFieldDesc, valid: &dyn Fn(&str) -> bool, slide: Option<&dyn Fn(f32) -> String>, scrub: Option<&dyn Fn(&str, f32, bool) -> String>,
    name: &str, changed: &mut Option<Str>, editing: &mut bool, out: &mut PxuiResponse) -> i32 {
    enter!(ui);
    let (label, value) = unsafe { (lent(d.label), lent(d.value)) };
    if bad_label(label) || value.len() > MAX_TEXT_BYTES || !crate::state::finite(&[d.x, d.y, d.width, d.height, d.fraction, d.tracking]) || d.width < 0.0 || d.height < 0.0 || d.font_size < 0 {
        return unsafe { fail(ui, "invalid value field arguments") };
    }
    let opts = unsafe { crate::text_inputs::field_opts(d, lent(d.display), lent(d.placeholder), lent(d.lead), lent(d.trail)) };
    let (text, open) = unsafe { engine(ui) }.value_field((d.x, d.y), d.width, d.height, &opts, valid, slide, scrub, label, value);
    *changed = text.map(|t| Str { ptr: t.as_ptr(), len: t.len() });
    *editing = open;
    unsafe { input_response(ui, Some(clip(name)), out) }
}

/// Whether a text input's label (its identity, display-text sized: `checked_label`) is refused.
fn bad_label(label: &str) -> bool { label.len() > MAX_LABEL_BYTES || crate::state::unusable(label) }

/// `pxui_ui_text_area` for the static wrapper: the descriptor's strings are Rust `&str`s and its spans Rust slices (UTF-8 and in
/// bounds by their types; their contents are checked as the export checks them), `node` a Rust value. `changed` takes the new text (the
/// engine's, borrowed until the next call on the `Ui`), `submitted` whether it was submitted, `out` what happened to the area's box.
#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn text_area(ui: *mut PxuiUi, d: &pxui_types::ffi::PxuiTextAreaDesc, node: &Node, cbs: &pxui_types::ffi::PxuiAreaCallbacks, name: &str,
    changed: &mut Option<Str>, submitted: &mut bool, out: &mut PxuiResponse) -> i32 {
    enter!(ui);
    let (label, value) = unsafe { (lent(d.label), lent(d.value)) };
    let bad_curve = matches!(&node.transition, pxui_types::node::OptTransition::Some(curve) if !curve.valid());
    if bad_label(label) || value.len() > MAX_TEXT_BYTES || node.grid.is_some() || bad_curve || d.flags & !pxui_types::ffi::PxuiTextAreaDesc::KNOWN != 0 {
        return unsafe { fail(ui, "invalid text area arguments") };
    }
    let shared = unsafe { (*ui).shared() };
    let (opts, ad) = (&mut shared.area, shared.area_ad.get_or_insert_with(crate::text_inputs::area_adapters));
    if unsafe { crate::text_inputs::area_opts(d, Some(cbs), opts, ad) }.is_none() { return unsafe { fail(ui, "invalid text area options") }; }
    let (text, done) = unsafe { engine(ui) }.text_area_submit(*node, opts, label, value);
    *changed = text.map(|t| Str { ptr: t.as_ptr(), len: t.len() });
    *submitted = done;
    // the language goes back to its adapters for the next call
    if let Some(lang) = opts.language.take() { ad.language = Some(lang); }
    unsafe { input_response(ui, Some(clip(name)), out) }
}

/// What happened to the text input the widget call just built (`Ui::input_response`), the box named `name` when given: what a text
/// input export reports. `PXUI_ERR_STATE` when the call built none.
#[inline]
pub(crate) unsafe fn input_response(ui: *mut PxuiUi, name: Option<&str>, out: &mut PxuiResponse) -> i32 {
    match unsafe { engine(ui) }.input_response(name) { Some(r) => { respond(&r, out); 0 } None => crate::PXUI_ERR_STATE }
}

/// The theme in effect, lent.
#[inline]
pub unsafe fn pxui_ui_theme(ui: *mut PxuiUi) -> *const ThemeSnapshot {
    enter!(any ui, core::ptr::null());
    let theme = (unsafe { engine(ui) }.theme() as *const pxui_core::Theme).cast();
    // cached in the handle: the theme a usable handle sees cannot change (see `PxuiUi`)
    unsafe { (*ui).theme = theme; }
    theme
}

// ---- containers: their children run through `Inner`

#[inline]
pub unsafe fn within(ui: *mut PxuiUi, box_: *const PxuiBox, f: Inner) -> i32 {
    enter!(ui);
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    e.within(&b, f);
    0
}

#[inline]
pub unsafe fn scope(ui: *mut PxuiUi, label: &str, f: Inner) -> i32 {
    enter!(ui);
    if bad_key(label) { return unsafe { fail(ui, "invalid scope label") }; }
    unsafe { engine(ui) }.scope(label, f);
    0
}

/// The body of `pxui_ui_view`: the engine's `view_of` (sized by its parent).
#[inline]
pub unsafe fn view_of(ui: *mut PxuiUi, key: &str, revision: u64, f: Inner, out: *mut PxuiBox) -> i32 {
    enter!(ui);
    if bad_key(key) { return unsafe { fail(ui, "invalid view arguments") }; }
    let handle = unsafe { engine(ui) }.view_of(key, &revision, f);
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

/// Whether display text is past its cap (`MAX_LABEL_BYTES`).
#[inline]
fn long(value: &str) -> bool { value.len() > MAX_LABEL_BYTES }
/// Display text from Rust clipped to `MAX_LABEL_BYTES` at a character boundary (`state::clip`): user data shown in a label must not
/// panic the wrapper. The C exports refuse longer text before they reach an entry, so this cuts only what the static wrapper passes.
use crate::state::clip;
/// `list` with each text clipped: the list itself unless one is past the cap (a rare caller, which pays one vector in `spare`).
fn clip_strs<'a>(list: &'a [&'a str], spare: &'a mut Vec<&'a str>) -> &'a [&'a str] {
    if list.iter().all(|o| !long(o)) { return list; }
    spare.extend(list.iter().map(|o| clip(o)));
    spare
}
/// `clip_strs` of segments, their texts and names.
fn clip_segments<'a>(list: &'a [Segment<'a>], spare: &'a mut Vec<Segment<'a>>) -> &'a [Segment<'a>] {
    if list.iter().all(|o| !long(o.text) && !o.name.is_some_and(long) && !long(o.detail)) { return list; }
    spare.extend(list.iter().map(|o| Segment { text: clip(o.text), name: o.name.map(clip), detail: clip(o.detail), ..*o }));
    spare
}
/// `overlay` with its name clipped (`clip`): the overlay itself unless its name is past the cap (a rare caller, which pays one copy).
#[inline(always)]
fn clipped_overlay<'a>(overlay: &'a Overlay<'a>, spare: &'a mut Option<Overlay<'a>>) -> &'a Overlay<'a> {
    if !overlay.name.is_some_and(long) { return overlay; }
    spare.insert(Overlay { name: overlay.name.map(clip), ..*overlay })
}
/// Segments as `clip_segments` gives them and whether `segments_ok` holds, in one pass when their texts together are within one
/// text's cap (then none is past it).
#[inline(always)]
fn checked_segments<'a>(list: &'a [Segment<'a>], spare: &'a mut Vec<Segment<'a>>) -> (&'a [Segment<'a>], bool) {
    let total = list.iter().fold(0usize, |total, o| total.saturating_add(o.text.len()).saturating_add(o.name.map_or(0, str::len)).saturating_add(o.detail.len()));
    if total <= MAX_LABEL_BYTES { return (list, list.len() <= MAX_OPTIONS); }
    let list = clip_segments(list, spare);
    (list, segments_ok(list))
}
/// Whether an identifier is refused: past its cap (`MAX_KEY_BYTES`), or a 0x01-led label that is not a usable resolved key.
#[inline]
fn bad_key(value: &str) -> bool { value.len() > MAX_KEY_BYTES || crate::state::unusable(value) }

/// Whether `options` are within the engine's limits: at most `MAX_OPTIONS` of them, at most `MAX_TOTAL_TEXT_BYTES` in all.
#[inline]
fn options_ok(options: &[&str]) -> bool {
    options.len() <= MAX_OPTIONS && options.iter().fold(0usize, |total, o| total.saturating_add(o.len())) <= MAX_TOTAL_TEXT_BYTES
}

/// Whether segmented `options` are within the engine's limits: as `options_ok`, their names counted with their texts.
#[inline]
fn segments_ok(options: &[Segment]) -> bool {
    let total = options.iter().fold(0usize, |total, o| total.saturating_add(o.text.len()).saturating_add(o.name.map_or(0, str::len)).saturating_add(o.detail.len()));
    options.len() <= MAX_OPTIONS && total <= MAX_TOTAL_TEXT_BYTES
}

/// The kit checkbox keyed by `key`, showing and named `text`, its style's variant given to the body; a click is the flip of `on`,
/// which the caller keeps.
#[inline]
pub unsafe fn checkbox(ui: *mut PxuiUi, key: &str, text: &str, on: bool, checkbox: &Checkbox, out: &mut EngineResponse) -> i32 {
    enter!(ui);
    let text = clip(text);
    if bad_key(key) { return unsafe { fail(ui, "invalid checkbox text") }; }
    // finite and not negative: NaN and infinity fail the compare
    if checkbox.size.is_some_and(|side| !(0.0..=f32::MAX).contains(&side)) { return unsafe { fail(ui, "invalid checkbox size") }; }
    *out = unsafe { engine(ui) }.checkbox(key, text, on, checkbox);
    0
}

/// A pick-one control of `items` keyed by `key`: `kind` 0 a radio group and 1 a tab bar (`name` their accessible name; `focus` makes a
/// press on a tab focus the bar), 3 a dropdown (`name` its shown and accessible text); an item with an id is keyed by it. `chosen` is
/// the choice (none: none), `secondary` the item a secondary press began on. Any other kind is refused.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn pick(ui: *mut PxuiUi, kind: u32, key: &str, name: &str, items: &[Segment], selected: Option<usize>, focus: bool, out: &mut EngineResponse, chosen: &mut Option<usize>, secondary: &mut Option<usize>) -> i32 {
    enter!(ui);
    let name = clip(name);
    let mut spare = Vec::new();
    let (items, ok) = checked_segments(items, &mut spare);
    if bad_key(key) || !ok { return unsafe { fail(ui, "invalid widget strings") }; }
    unsafe { pick_body(ui, kind, key, name, items.len(), &|i| items[i], selected, focus, out, chosen, secondary) }
}

/// `pick` for `pxui_ui_pick` and `pxui_ui_tabs`, which has checked the key, the name and every item as C hands them over (`checked_key`, `checked`,
/// `checked_items`): only the handle is checked here, so nothing is checked twice. `item(i)` reads item `i` where it lies.
#[allow(clippy::too_many_arguments)]
#[inline]
pub(crate) unsafe fn pick_checked<'a>(ui: *mut PxuiUi, kind: u32, key: &str, name: &str, count: usize, item: &dyn Fn(usize) -> Segment<'a>, selected: Option<usize>, focus: bool, out: &mut PxuiResponse, chosen: &mut Option<usize>, secondary: &mut Option<usize>) -> i32 {
    enter!(ui);
    let mut r = EngineResponse::default();
    let rc = unsafe { pick_body(ui, kind, key, name, count, item, selected, focus, &mut r, chosen, secondary) };
    if rc == 0 { respond(&r, out); }
    rc
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
unsafe fn pick_body<'a>(ui: *mut PxuiUi, kind: u32, key: &str, name: &str, count: usize, item: &dyn Fn(usize) -> Segment<'a>, selected: Option<usize>, focus: bool, out: &mut EngineResponse, chosen: &mut Option<usize>, secondary: &mut Option<usize>) -> i32 {
    let engine = unsafe { engine(ui) };
    let (r, at, pressed) = match kind {
        0 => engine.pick_radio(key, name, count, item, selected, 0),
        1 => engine.tabs(key, name, count, item, selected, 0, focus),
        3 => engine.pick_dropdown(key, name, count, item, selected, 0),
        _ => return unsafe { fail(ui, "invalid pick kind") },
    };
    *out = r;
    (*chosen, *secondary) = (at, pressed);
    0
}

/// What the engine's slider reports: its response, its value, the held preview and the commit.
pub type SliderResult = (EngineResponse, f32, Option<f32>, Option<f32>);

/// The kit slider, reporting the engine's own record (`slider_result`'s body; the static wrapper reads it as it is).
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn slider_engine(ui: *mut PxuiUi, key: &str, text: &str, value: f32, min: f32, max: f32, slider: &Slider, out: &mut SliderResult) -> i32 {
    enter!(ui);
    let text = clip(text);
    if !value.is_finite() || !min.is_finite() || !max.is_finite() || min > max || bad_key(key) { return unsafe { fail(ui, "invalid slider arguments") }; }
    *out = unsafe { engine(ui) }.slider_result(key, text, value, min..=max, slider);
    0
}

/// The kit slider with its phases: `scrub` gets the held preview and the commit (flags 1 and 2), the value in `preview` when neither.
#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn slider_result(ui: *mut PxuiUi, key: &str, text: &str, value: f32, min: f32, max: f32, slider: &Slider, out: &mut PxuiResponse, scrub: &mut pxui_types::ffi::PxuiScrubOut) -> i32 {
    let mut result = SliderResult::default();
    let rc = unsafe { slider_engine(ui, key, text, value, min, max, slider, &mut result) };
    if rc != 0 { return rc; }
    let (r, v, preview, commit) = result;
    respond(&r, out);
    *scrub = pxui_types::ffi::PxuiScrubOut { flags: u32::from(preview.is_some()) | u32::from(commit.is_some()) << 1 | u32::from(r.held) << 2, node: to_abi_box(r.handle),
        preview: preview.unwrap_or(v), commit: commit.unwrap_or_default(), ..Default::default() };
    0
}

/// The kit section index keyed by `key`, named `name`, its `letters` sharing the strip's height; `chosen` is the letter chosen this frame
/// (`None` when nothing moved it).
#[inline]
pub unsafe fn section_index(ui: *mut PxuiUi, key: &str, name: &str, letters: &[&str], out: &mut PxuiResponse, chosen: &mut Option<usize>) -> i32 {
    enter!(ui);
    let name = clip(name);
    let mut spare = Vec::new();
    let letters = clip_strs(letters, &mut spare);
    if bad_key(key) || !options_ok(letters) { return unsafe { fail(ui, "invalid widget strings") }; }
    unsafe { section_index_body(ui, key, name, letters, out, chosen) }
}

/// `section_index` for `pxui_ui_section_index`, which has checked the key, the name and every letter as C hands them over.
#[inline]
pub(crate) unsafe fn section_index_checked(ui: *mut PxuiUi, key: &str, name: &str, letters: &[&str], out: &mut PxuiResponse, chosen: &mut Option<usize>) -> i32 {
    enter!(ui);
    unsafe { section_index_body(ui, key, name, letters, out, chosen) }
}

#[inline(always)]
unsafe fn section_index_body(ui: *mut PxuiUi, key: &str, name: &str, letters: &[&str], out: &mut PxuiResponse, chosen: &mut Option<usize>) -> i32 {
    let (r, at) = unsafe { engine(ui) }.section_index(key, name, letters);
    respond(&r, out);
    *chosen = at;
    0
}

/// The disclosure keyed by `key` showing `text`: `open` is the caller's value, `f` builds the content while open; `out` is the header's
/// response (`changed` when this frame's input toggled it).
#[inline]
pub unsafe fn disclosure(ui: *mut PxuiUi, key: &str, text: &str, open: bool, f: Inner, out: &mut PxuiResponse) -> i32 {
    enter!(ui);
    let text = clip(text);
    if bad_key(key) { return unsafe { fail(ui, "invalid disclosure text") }; }
    let r = unsafe { engine(ui) }.disclosure(key, text, open, f);
    respond(&r, out);
    0
}

/// A progress row keyed by `key`, showing and named `text`.
#[inline]
pub unsafe fn progress(ui: *mut PxuiUi, key: &str, text: &str, value: Option<f32>, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    let text = clip(text);
    if bad_key(key) || value.is_some_and(|v| !v.is_finite()) { return unsafe { fail(ui, "invalid progress arguments") }; }
    *out = to_abi_box(unsafe { engine(ui) }.progress(key, text, value));
    0
}

/// A spinner keyed by `key`, named `name`.
#[inline]
pub unsafe fn spinner(ui: *mut PxuiUi, key: &str, name: &str, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    let name = clip(name);
    if bad_key(key) { return unsafe { fail(ui, "invalid spinner text") }; }
    *out = to_abi_box(unsafe { engine(ui) }.spinner(key, name));
    0
}

/// A button size `Button` and `IconButton` allow: the kit's (`Fit`), finite non-negative points or filling the line.
fn button_size(size: Size) -> bool { match size { Size::Fit | Size::Grow => true, Size::Px(v) => (0.0..=f32::MAX).contains(&v), _ => false } }

/// The engine's response as the record the wrapper's `Response` is read from.
#[inline(always)]
pub(crate) fn respond(r: &pxui_core::ui::Response, out: &mut PxuiResponse) {
    out.node = to_abi_box(r.handle);
    out.flags = r.clicked as u32 * PxuiResponse::CLICKED | r.hovered as u32 * PxuiResponse::HOVERED | r.held as u32 * PxuiResponse::HELD
        | r.focused as u32 * PxuiResponse::FOCUSED | r.activated as u32 * PxuiResponse::ACTIVATED | r.secondary as u32 * PxuiResponse::SECONDARY;
}

/// The kit button with options, the body of `pxui_ui_button`: `states` in place of the theme's button, `w` and `h` (`button_size`), `justify` its place in the line, `pad`
/// (finite, non-negative) inside the box, `centered` the box exactly its size with its text centred, `label` the text's style (one that
/// resolves), `axis`, `gap` (finite, non-negative) and `content_align` the contents' layout, `focus_on_click` a press focusing it,
/// `lead` the leading icon's glyph or `lead_draw` (the app's C drawing with its `lead_user` and `lead_word`, as `icon_button_with` takes
/// them), `detail` and `accessory` (empty: none; clipped as the text is) in `detail_label` and `accessory_label` (ones that resolve), `content` built in the box in place of `text` and given the button's state. What happened goes to `out`; with `signal`, the box's
/// whole signal too.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn button_with(ui: *mut PxuiUi, key: &str, text: &str, name: Option<&str>, states: Option<&States>, w: Size, h: Size, justify: Justify, pad: Edges,
    enabled: bool, selected: bool, danger: bool, centered: bool, label: Option<&pxui_types::text::LabelFields>, axis: Option<Axis>, gap: f32, content_align: Option<Along>, focus_on_click: bool,
    lead: &str, lead_draw: pxui_types::ffi::PxuiIconDraw, lead_user: *mut core::ffi::c_void, lead_word: u32, icon_size: f32, detail: &str, accessory: &str,
    detail_label: Option<&pxui_types::text::LabelFields>, accessory_label: Option<&pxui_types::text::LabelFields>,
    content: Option<&mut dyn FnMut(&mut Ui, State)>, out: &mut PxuiResponse, signal: Option<&mut PxuiSignal>) -> i32 {
    enter!(ui);
    let text = clip(text);
    let name = name.map(clip);
    let parts = [clip(lead), clip(detail), clip(accessory)];
    if bad_key(key) { return unsafe { fail(ui, "invalid button text") }; }
    if !button_size(w) || !button_size(h) || ![pad.l, pad.t, pad.r, pad.b, gap, icon_size].iter().all(|v| (0.0..=f32::MAX).contains(v)) { return unsafe { fail(ui, "invalid button descriptor") }; }
    let (e, shared) = (unsafe { engine(ui) }, unsafe { (*ui).shared });
    if [label, detail_label, accessory_label].iter().any(|l| l.is_some_and(|l| e.resolve(l).is_none())) { return unsafe { fail(ui, "invalid button label") }; }
    let ink = if danger { Some(e.theme().tokens.invalid) } else { None };
    let lead_draw = lead_draw.map(|f| (crate::paint::icon_bridge as pxui_core::ui::IconDraw, [f as usize, lead_user as usize, shared as usize, lead_word as usize]));
    // the looks and the label are copied only when given: `copied()` copies the whole record either way
    let mut opts = pxui_core::ui::ButtonOpts { states: None, w, h, enabled, selected, justify, padding: pad, ink, bare: false, icon: false, centered, label: None, axis, gap, content_align, focus_on_click,
        lead_draw, icon_size, detail_label: detail_label.copied(), accessory_label: accessory_label.copied() };
    if let Some(states) = states { opts.states = Some(*states); }
    if let Some(label) = label { opts.label = Some(*label); }
    let body = content;
    let r = match signal {
        None => e.button_opts(key, text, name, &opts, parts, body, None),
        Some(signal) => {
            let mut read = pxui_core::ui::Signal::default();
            let r = e.button_opts(key, text, name, &opts, parts, body, Some(&mut read));
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
pub unsafe fn button_plain(ui: *mut PxuiUi, key: &str, text: &str, out: &mut EngineResponse) -> i32 {
    enter!(ui);
    let text = clip(text);
    if bad_key(key) { return unsafe { fail(ui, "invalid button text") }; }
    let r = unsafe { engine(ui) }.button_opts(key, text, None, &pxui_core::ui::ButtonOpts::DEFAULT, [""; 3], None, None);
    *out = r;
    0
}

/// The plain kit icon button (`IconButton::DEFAULT`) showing the glyph `glyph`, `pxui_ui_icon_button_plain`'s body: as
/// `button_plain`.
#[inline(always)]
pub unsafe fn icon_button_plain(ui: *mut PxuiUi, key: &str, glyph: &str, name: &str, out: &mut EngineResponse) -> i32 {
    enter!(ui);
    let glyph = clip(glyph);
    let name = clip(name);
    if bad_key(key) { return unsafe { fail(ui, "invalid icon button text") }; }
    let r = unsafe { engine(ui) }.icon_button_text(key, glyph, None, name, &pxui_core::ui::IconButton::DEFAULT, None);
    *out = r;
    0
}

/// The kit icon button with options, the body of `pxui_ui_icon_button`: the `glyph`, or the app's
/// drawing `draw` (a C callback, its `user` and the icon's `word`, read in it with `pxui_paint_icon_word`), named `name`; `states` in place of the theme's icon button, `w` and `h`
/// (`button_size`), `press_scale` finite and positive, `press` the valid curves in and out (`None`: the kit's), `selected` an on/off key's
/// state (`None`: a plain button). What happened goes to `out`; with `signal`, the box's whole signal too.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn icon_button_with(ui: *mut PxuiUi, key: &str, glyph: &str, draw: pxui_types::ffi::PxuiIconDraw, user: *mut core::ffi::c_void, word: u32, name: &str, states: Option<&States>,
    w: Size, h: Size, enabled: bool, press_scale: f32, press: Option<&'static [pxui_types::Curve; 2]>, selected: Option<bool>, out: &mut PxuiResponse, signal: Option<&mut PxuiSignal>) -> i32 {
    enter!(ui);
    let glyph = clip(glyph);
    let name = clip(name);
    if bad_key(key) { return unsafe { fail(ui, "invalid icon button text") }; }
    if !button_size(w) || !button_size(h) || !press_scale.is_finite() || press_scale <= 0.0 { return unsafe { fail(ui, "invalid icon button descriptor") }; }
    let draw = draw.map(|f| (crate::paint::icon_bridge as pxui_core::ui::IconDraw, [f as usize, user as usize, unsafe { (*ui).shared } as usize, word as usize]));
    if press.is_some_and(|p| !p[0].valid() || !p[1].valid()) { return unsafe { fail(ui, "invalid icon button press curve") }; }
    let mut button = pxui_core::ui::IconButton { states: None, w, h, enabled, press_scale, press, selected };
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

/// A badge keyed by `key`, named `name`, `value` its text.
#[inline]
pub unsafe fn badge(ui: *mut PxuiUi, key: &str, name: &str, value: &str, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    let name = clip(name);
    let value = clip(value);
    if bad_key(key) { return unsafe { fail(ui, "invalid badge text") }; }
    *out = to_abi_box(unsafe { engine(ui) }.badge(key, name, value));
    0
}

/// The text a search field changed to, or none; kept by the library until the next frame.
#[inline]
pub unsafe fn search_field(ui: *mut PxuiUi, label: &str, icon: &str, value: &str, placeholder: &str, traits: pxui_types::TextTraits, out: &mut Option<Str>, name: &str, response: &mut PxuiResponse) -> i32 {
    enter!(ui);
    let label = clip(label);
    let icon = clip(icon);
    let placeholder = clip(placeholder);
    if value.len() > MAX_TEXT_BYTES { return unsafe { fail(ui, "invalid search field text") }; }
    let found = unsafe { engine(ui) }.search_field(label, icon, value, placeholder, traits);
    *out = match found { None => None, Some(text) => match unsafe { get(ui) } { Some(state) => match state.lend(text) { Ok(text) => Some(text), Err(code) => return code }, None => Some(Str::default()) } };
    unsafe { input_response(ui, Some(clip(name)), response) }
}

/// `Ui::menu` of Rust rows: the position and the widths are finite by the checks below, the rows and the key are Rust values. The
/// static wrapper's entry; the `pxui_ui_context_menu` export decodes its columns into rows and calls this.
#[inline]
pub unsafe fn menu(ui: *mut PxuiUi, key: pxui_types::ui::Key, at: (f32, f32), items: &[pxui_types::ui::MenuItem], menu: &pxui_types::ui::Menu, out: &mut pxui_types::ui::Chosen) -> i32 {
    enter!(ui);
    let width_ok = menu.w.is_none_or(|width| width.is_finite() && width >= 0.0);
    let row_ok = menu.row_h.is_none_or(|row| row.is_finite() && row > 0.0);
    let menu = &pxui_types::ui::Menu { name: menu.name.map(clip), ..*menu };
    if !at.0.is_finite() || !at.1.is_finite() || !width_ok || !row_ok || !(0.0..=f32::MAX).contains(&menu.padding) { return unsafe { fail(ui, "invalid context menu arguments") }; }
    *out = unsafe { engine(ui) }.menu(key, pxui_types::geom::Point::new(at.0, at.1), items, menu);
    0
}

#[inline]
pub unsafe fn tooltip(ui: *mut PxuiUi, box_: &PxuiBox, label: &str) -> i32 {
    enter!(ui);
    let label = clip(label);
    unsafe { engine(ui) }.tooltip(&unbox(box_), label);
    0
}

#[inline]
fn unbox(raw: &PxuiBox) -> pxui_core::ui::UiBox { pxui_core::ui::UiBox { index: raw.index as usize, box_key: raw.box_key, box_slot: raw.box_slot as usize, generation: raw.generation } }

#[inline]
pub unsafe fn rect(ui: *mut PxuiUi, box_: &PxuiBox, out: &mut pxui_types::geom::Rect) -> i32 {
    enter!(any ui);
    *out = unsafe { engine(ui) }.rect(&unbox(box_));
    0
}

#[inline]
pub unsafe fn box_of_key(ui: *mut PxuiUi, key: i64, out: &mut PxuiBox) -> bool {
    enter!(any ui, false);
    let Some(found) = (unsafe { engine(ui) }).box_of_key(key) else { return false };
    *out = to_abi_box(found);
    true
}

// ---- the inspector rows and the footer

#[inline]
pub unsafe fn inspector_toggle(ui: *mut PxuiUi, key: &str, label: &str, value: bool, out: &mut bool) -> i32 {
    enter!(ui);
    let label = clip(label);
    if bad_key(key) { return unsafe { fail(ui, "invalid inspector toggle text") }; }
    *out = unsafe { engine(ui) }.inspector_toggle(key, label, value);
    0
}

#[inline]
pub unsafe fn inspector_button(ui: *mut PxuiUi, key: &str, label: &str, out: &mut bool) -> i32 {
    enter!(ui);
    let label = clip(label);
    if bad_key(key) { return unsafe { fail(ui, "invalid inspector button text") }; }
    *out = unsafe { engine(ui) }.inspector_button(key, label);
    0
}

#[inline]
pub unsafe fn inspector_readout(ui: *mut PxuiUi, width: Option<f32>, key: &str, label: &str, value: &str) -> i32 {
    enter!(ui);
    let label = clip(label);
    let value = clip(value);
    if bad_key(key) || width.is_some_and(|w| !w.is_finite() || w < 0.0) { return unsafe { fail(ui, "invalid inspector readout text") }; }
    unsafe { engine(ui) }.inspector_readout(width, key, label, value);
    0
}

#[inline]
pub unsafe fn inspector_message(ui: *mut PxuiUi, key: &str, message: &str) -> i32 {
    enter!(ui);
    let message = clip(message);
    if bad_key(key) { return unsafe { fail(ui, "invalid inspector message text") }; }
    unsafe { engine(ui) }.inspector_message(key, message);
    0
}

/// The row's box and its control's x, y and width.
#[inline]
pub unsafe fn inspector_row(ui: *mut PxuiUi, width: Option<f32>, pin: Option<bool>, key: &str, label: &str, out: &mut (PxuiBox, f32, f32, f32)) -> i32 {
    enter!(ui);
    let label = clip(label);
    if bad_key(key) || width.is_some_and(|w| !w.is_finite() || w < 0.0) { return unsafe { fail(ui, "invalid inspector row text") }; }
    let (row, x, y, w) = unsafe { engine(ui) }.inspector_row(width, pin, key, label);
    *out = (to_abi_box(row), x, y, w);
    0
}

#[inline]
pub unsafe fn inspector_section(ui: *mut PxuiUi, key: &str, expanded: bool, set_expanded: Option<bool>, label: &str, f: Inner, out: &mut bool) -> i32 {
    enter!(ui);
    let label = clip(label);
    if bad_key(key) { return unsafe { fail(ui, "invalid inspector section text") }; }
    *out = unsafe { engine(ui) }.inspector_section(key, expanded, set_expanded, label, f);
    0
}

/// The footer over the caller's borrowed rows: nothing is copied.
#[inline]
pub unsafe fn footer(ui: *mut PxuiUi, right: Option<&str>, hints: &[(&str, &str)]) -> i32 {
    enter!(ui);
    let total = right.map_or(0, str::len) + hints.iter().fold(0usize, |total, (k, v)| total.saturating_add(k.len()).saturating_add(v.len()));
    let right = right.map(clip);
    if hints.len() > 4096 || total > MAX_TOTAL_TEXT_BYTES { return unsafe { fail(ui, "invalid footer text") }; }
    unsafe { engine(ui) }.footer(right, hints);
    0
}

// ---- containers with options

/// Whether every number of `overlay` is finite (and its width, height and padding not negative).
/// On the bits, one compare a group: a finite number's bits less the sign are below infinity's; one in `0.0..=f32::MAX` is a
/// positive finite number's bits or -0.0 (its bits less the sign 0); NaN is neither.
#[inline(always)]
fn overlay_ok(overlay: &Overlay) -> bool {
    let finite = [overlay.at.0, overlay.at.1, overlay.max_h.unwrap_or_default()].iter().fold(0, |most, v| most.max(v.to_bits() & 0x7fff_ffff)) < 0x7f80_0000;
    let sized = [overlay.w.unwrap_or_default(), overlay.h.unwrap_or_default(), overlay.padding, overlay.dismiss.fraction.unwrap_or_default(), overlay.dismiss.velocity.unwrap_or_default()]
        .iter().fold(0, |most, v| { let bits = v.to_bits(); most.max(if bits == 0x8000_0000 { 0 } else { bits }) }) <= f32::MAX.to_bits();
    finite & sized
}

/// A panel keyed by `key`, placed, dressed and named by `overlay`.
#[inline]
pub unsafe fn panel(ui: *mut PxuiUi, key: &str, overlay: &Overlay, f: Inner, root: &mut PxuiBox) -> i32 {
    enter!(ui);
    let mut clipped = None;
    let overlay = clipped_overlay(overlay, &mut clipped);
    if bad_key(key) || !overlay_ok(overlay) { return unsafe { fail(ui, "invalid panel descriptor") }; }
    *root = to_abi_box(unsafe { engine(ui) }.panel_root(key, overlay, f));
    0
}

/// A popup keyed by `key`, placed, dressed and named by `overlay`; `out` says whether it is open, `root` its root (empty: dismissed).
#[inline]
pub unsafe fn popup(ui: *mut PxuiUi, key: &str, overlay: &Overlay, f: Inner, out: &mut bool, root: &mut PxuiBox) -> i32 {
    enter!(ui);
    let mut clipped = None;
    let overlay = clipped_overlay(overlay, &mut clipped);
    if bad_key(key) || !overlay_ok(overlay) { return unsafe { fail(ui, "invalid popup descriptor") }; }
    let open = unsafe { engine(ui) }.popup_root(key, overlay, f);
    (*out, *root) = (open.is_some(), open.map_or_else(PxuiBox::default, to_abi_box));
    0
}

/// A scroll view keyed by `key`, named `name` (empty: none), laid out by `node` (no grid; a transition must be valid).
#[inline]
pub unsafe fn scroll_view(ui: *mut PxuiUi, key: &str, name: &str, node: &Node, f: Inner, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    let name = clip(name);
    let bad_curve = matches!(&node.transition, pxui_types::node::OptTransition::Some(curve) if !curve.valid());
    if bad_key(key) || !finite(node.w) || !finite(node.h) || node.grid.is_some() || bad_curve { return unsafe { fail(ui, "invalid scroll view arguments") }; }
    let engine = unsafe { engine(ui) };
    let b = engine.scroll_view(key, *node, f);
    if !name.is_empty() { engine.a11y_name(&b, name); }
    *out = to_abi_box(b);
    0
}

// ---- collections

const MAX_ITEMS: usize = 1_000_000;

// ---- more containers and callbacks


/// A grid of `count` cells, built on demand: `cell` gets the engine and the cell's index, and makes its own borrowed handle.
#[inline]
pub unsafe fn grid_list(ui: *mut PxuiUi, label: &str, count: usize, cell_w: f32, cell_h: f32, cell: &mut dyn FnMut(&mut Ui, usize), out: &mut PxuiBox) -> i32 {
    enter!(ui);
    if bad_key(label) || count > MAX_ITEMS || !cell_w.is_finite() || !cell_h.is_finite() || cell_w <= 0.0 || cell_h <= 0.0 { return unsafe { fail(ui, "invalid grid list descriptor") }; }
    *out = to_abi_box(unsafe { engine(ui) }.grid_list(label, count, cell_w, cell_h, cell));
    0
}

/// Two panes in a row (`axis` 0) or a column (1); `response` is the grip's and `out` the ratio under the pointer while the grip is held
/// and on its release.
#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn split(ui: *mut PxuiUi, label: &str, axis: pxui_types::Axis, ratio: f32, min_a: f32, min_b: f32, a: Inner, b: Inner, response: &mut PxuiResponse, out: &mut Option<f32>) -> i32 {
    enter!(ui);
    if bad_key(label) || ![ratio, min_a, min_b].iter().all(|v| v.is_finite()) || min_a < 0.0 || min_b < 0.0 || axis == pxui_types::Axis::Stack { return unsafe { fail(ui, "invalid split descriptor") }; }
    let (r, _, at) = unsafe { engine(ui) }.split_result(label, axis, ratio, min_a, min_b, a, b);
    respond(&r, response);
    *out = at;
    0
}

/// A sheet keyed by `key` by `anchor` at `overlay`'s placement and size.
#[inline]
pub unsafe fn anchored(ui: *mut PxuiUi, key: &str, anchor: pxui_types::geom::Rect, overlay: &Overlay, f: Inner, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    let mut clipped = None;
    let overlay = clipped_overlay(overlay, &mut clipped);
    let placement = overlay.place;
    if bad_key(key) || !overlay_ok(overlay) || anchor.w < 0.0 || anchor.h < 0.0 || placement.margin < 0.0
        || ![anchor.x, anchor.y, anchor.w, anchor.h, placement.gap, placement.margin].iter().all(|v| v.is_finite()) { return unsafe { fail(ui, "invalid anchored arguments") }; }
    *out = to_abi_box(unsafe { engine(ui) }.anchored(key, anchor, overlay, f));
    0
}

/// A modal keyed by `key`, `overlay`'s width and name; `out` says whether it is open.
#[inline]
pub unsafe fn modal(ui: *mut PxuiUi, key: &str, overlay: &Overlay, f: Inner, out: &mut bool, root: &mut PxuiBox) -> i32 {
    enter!(ui);
    let mut clipped = None;
    let overlay = clipped_overlay(overlay, &mut clipped);
    if bad_key(key) || !overlay_ok(overlay) { return unsafe { fail(ui, "invalid modal arguments") }; }
    let open = unsafe { engine(ui) }.modal_root(key, overlay, f);
    (*out, *root) = (open.is_some(), open.map_or_else(PxuiBox::default, to_abi_box));
    0
}

/// Toasts keyed by `key`, each by its item id; `out` is the index of one clicked away or expired.
#[inline]
pub unsafe fn toasts(ui: *mut PxuiUi, key: &str, items: &[pxui_types::ui::Toast], seconds: f64, out: &mut Option<usize>) -> i32 {
    enter!(ui);
    if bad_key(key) || items.len() > MAX_OPTIONS || !seconds.is_finite() || seconds < 0.0 { return unsafe { fail(ui, "invalid toasts") }; }
    let spare: Vec<pxui_types::ui::Toast>;
    let items = if items.iter().all(|t| !long(t.text)) { items } else { spare = items.iter().map(|t| pxui_types::ui::Toast::new(t.id, clip(t.text))).collect(); &spare };
    *out = unsafe { engine(ui) }.toasts(key, items, seconds).chosen;
    0
}

/// A modal dialog: `buttons` (at most 64) as `Ui::dialog` takes them, `style` its sheet; what the frame did goes to `out`.
#[inline]
pub unsafe fn dialog(ui: *mut PxuiUi, key: &str, title: &str, buttons: &[pxui_types::ui::DialogButton], style: &pxui_types::ui::Dialog, f: Inner, out: &mut pxui_types::ui::Chosen) -> i32 {
    enter!(ui);
    let title = clip(title);
    let mut spare = Vec::new();
    let buttons = if buttons.iter().all(|b| !long(b.text) && !long(b.detail)) { buttons } else { spare.extend(buttons.iter().map(|b| pxui_types::ui::DialogButton { text: clip(b.text), detail: clip(b.detail), ..*b })); &spare[..] };
    if bad_key(key) || buttons.len() > 64 || !style.w.is_finite() || style.w < 0.0 { return unsafe { fail(ui, "invalid dialog arguments") }; }
    *out = unsafe { engine(ui) }.dialog(key, title, buttons, style, f);
    0
}


/// `pxui_slot` as the static wrapper calls it: the caller's content runs inside, and a panic of it unwinds to the app as in every trusted entry.
/// The kit's text of the collection item being built (`Ui::item_text`); refused outside a `selection_list`/`tree` row or `table` cell.
#[inline]
pub unsafe fn item_text(ui: *mut PxuiUi, text: &str) -> i32 {
    enter!(ui);
    let text = clip(text);
    if !unsafe { engine(ui) }.in_item() { return unsafe { fail(ui, "item_text outside a collection row or cell") }; }
    unsafe { engine(ui) }.item_text(text);
    0
}

#[inline]
pub unsafe fn pxui_slot(ui: *mut PxuiUi, slot: pxui_types::ffi::PxuiSlot, out: *mut u8) -> i32 { unsafe { crate::bodies::slot(ui, slot, out) } }

/// `tree` keyed by `key`: `cell(ui, node, 0)` builds the children of the row that shows tree node `node`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn tree(ui: *mut PxuiUi, key: &str, nodes: &[pxui_types::ffi::PxuiTreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], cell: &mut dyn FnMut(&mut Ui, usize), out: &mut (Option<usize>, usize)) -> i32 {
    enter!(ui);
    if bad_key(key) || nodes.len() > MAX_ITEMS || open.len() != nodes.len() || visible.len() < nodes.len() { return unsafe { fail(ui, "invalid tree descriptor") }; }
    let nodes = unsafe { core::slice::from_raw_parts(nodes.as_ptr().cast::<pxui_core::ui_widgets::TreeNode>(), nodes.len()) };
    let r = unsafe { engine(ui) }.tree(key, nodes, open, selected, visible, cell);
    *out = (r.activated, r.shown);
    0
}


/// Names the table keyed `key` under the current parent `name` (the 1.x exports named it by their label).
pub(crate) fn name_table(engine: &mut Ui, key: &str, name: &str) {
    if let Some(frame) = engine.box_of_key(pxui_types::ui::key_of(engine.current_seed(), key)) { engine.a11y_name(&frame, name); }
}

// ABI 1.65: the text calls that take a label style and resolve it in the same call (no `pxui_ui_resolve_label` round trip): the
// style resolves against the Ui's theme as `pxui_ui_resolve_label` resolves it, then the call runs as its resolved-style twin.

/// The resolved C style of `style` on `ui`; `None` for a gone Ui or a style that does not resolve.
#[inline(always)]
unsafe fn resolved(ui: *mut PxuiUi, style: &LabelFields) -> Option<PxuiTextStyle> { unsafe { crate::state::engine(ui) }?.resolve(style) }

#[inline(always)]
pub unsafe fn label_text_spans(ui: *mut PxuiUi, box_: *const PxuiBox, value: Str, style: &LabelFields, spans: *const TextSpans) -> i32 {
    let Some(style) = (unsafe { resolved(ui, style) }) else { return unsafe { fail(ui, "label style does not resolve") } };
    unsafe { crate::text::ui_text_spans(ui, box_, value, &style, spans) }
}

#[inline(always)]
pub unsafe fn label_text_line_box(ui: *mut PxuiUi, box_: *const PxuiBox, value: Str, style: &LabelFields, spans: *const TextSpans) -> i32 {
    let Some(style) = (unsafe { resolved(ui, style) }) else { return unsafe { fail(ui, "label style does not resolve") } };
    unsafe { crate::text::ui_text_line_box(ui, box_, value, &style, spans) }
}

/// The measurement of `value` in a label style, with `spans` when not null (the body of `pxui_ui_label_measure`).
#[inline(always)]
pub unsafe fn label_measure(ui: *mut PxuiUi, value: Str, style: &LabelFields, max_width: f32, spans: *const TextSpans, out: *mut TextMetrics) -> i32 {
    let Some(style) = (unsafe { resolved(ui, style) }) else { return crate::PXUI_ERR_ARGUMENT };
    if spans.is_null() { unsafe { crate::text::text_width(ui, 0, value, &style, max_width, out) } }
    else { unsafe { crate::text::text_width_spans(ui, 0, value, &style, max_width, spans, out) } }
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn label_quads(ui: *mut PxuiUi, value: Str, style: &LabelFields, max_width: f32, x: f32, y: f32, scale: f32, out: *mut TextGlyph, capacity: usize) -> usize {
    let Some(style) = (unsafe { resolved(ui, style) }) else { return usize::MAX };
    unsafe { crate::text::text_quads(ui, 0, value, &style, max_width, x, y, scale, out, capacity) }
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn label_rasterize_rgba(ui: *mut PxuiUi, value: Str, style: &LabelFields, max_width: f32, scale: f32, color_r: u8, color_g: u8, color_b: u8,
    color_a: u8, width: *mut u32, height: *mut u32, out: *mut u8, capacity: usize) -> usize {
    let Some(style) = (unsafe { resolved(ui, style) }) else { return usize::MAX };
    unsafe { crate::text::text_rasterize_rgba(ui, 0, value, &style, max_width, scale, color_r, color_g, color_b, color_a, width, height, out, capacity) }
}

#[inline(always)]
pub unsafe fn label_prewarm(ui: *mut PxuiUi, value: Str, style: &LabelFields, max_width: f32) -> i32 {
    let Some(style) = (unsafe { resolved(ui, style) }) else { return crate::PXUI_ERR_ARGUMENT };
    unsafe { crate::text::text_prewarm(ui, value, &style, max_width) }
}

// ---- U2b.1: selection list, table and heading, section list with its trailer (one validation pass each, the whole result in one record)

use pxui_core::ui_lists::{ColumnSpec, ListResult, MAX_COLUMNS};
use pxui_types::ffi::{PxuiCellState, PxuiColumn, PxuiColumnWidth, PxuiHeadResult, PxuiSectionListDesc, PxuiListResult, PxuiSelectionListDesc, PxuiTableHeadDesc, PxuiTableDesc, PxuiTableResult};

/// A section geometry the engine takes: a positive finite row height, finite non-negative part heights, a finite total, at most
/// `MAX_ITEMS` rows.
fn geometry_ok(count: usize, row: f32, parts: [f32; 3]) -> bool {
    let total = parts.iter().sum::<f32>() + count as f32 * row;
    count <= MAX_ITEMS && row.is_finite() && row > 0.0 && total.is_finite() && parts.iter().all(|v| v.is_finite() && *v >= 0.0)
}

/// The library-written cell record of `c`.
pub(crate) fn cell_state(c: &ListCell) -> PxuiCellState {
    PxuiCellState { size: core::mem::size_of::<PxuiCellState>() as u32, selected: c.selected as u8, enabled: c.enabled as u8, focused: c.focused as u8, held: c.held as u8, handle: to_abi_box(c.handle), y: c.y, reserved: 0 }
}

/// `r` and `anchor` written to `out` (its size already checked by the caller).
fn write_list(r: &ListResult, anchor: Option<usize>, out: &mut PxuiListResult) {
    let none = usize::MAX;
    (out.focused, out.reserved, out.handle, out.row) = (r.focused as u8, [0; 3], to_abi_box(r.handle), r.row.map_or(PxuiBox::default(), to_abi_box));
    (out.activated, out.selected, out.secondary, out.anchor) = (r.activated.unwrap_or(none), r.selected.unwrap_or(none), r.secondary.unwrap_or(none), anchor.unwrap_or(none));
    out.drop_at = r.drop_at.unwrap_or(none);
    (out.moved_from, out.moved_to) = r.moved.unwrap_or((none, none));
}

/// The columns of `span` checked in one pass before any effect into `out`: each record's size, flags and reserved byte, its title
/// (a label, 64 KiB at most), a finite non-negative share and width, ids (an index without one) unique; at most `MAX_COLUMNS`.
/// `None`: refused.
pub(crate) unsafe fn read_columns<'a>(span: pxui_types::ffi::Slice<PxuiColumn>, out: &mut [ColumnSpec; MAX_COLUMNS], titles: &mut [&'a str; MAX_COLUMNS]) -> Option<usize> {
    if span.len > MAX_COLUMNS || (span.len > 0 && span.ptr.is_null()) { return None; }
    for i in 0..span.len {
        let c = unsafe { span.ptr.add(i) };
        if (unsafe { c.cast::<u32>().read_unaligned() } as usize) < core::mem::size_of::<PxuiColumn>() { return None; }
        let c = unsafe { &*c };
        let finite = |v: f32| v.is_finite() && v >= 0.0;
        if c.sortable > 1 || c.grip > 1 || c.has_id > 1 || c.reserved != 0 || !finite(c.share) || !finite(c.w) { return None; }
        let title = unsafe { crate::state::checked(c.title) }?;
        let id = if c.has_id != 0 { c.id } else { i as i64 };
        if out[..i].iter().any(|o| o.id == id) { return None; }
        titles[i] = title;
        out[i] = ColumnSpec { id, share: c.share, w: c.w, grip: c.grip != 0, sortable: c.sortable != 0 };
    }
    Some(span.len)
}

/// The widths span of a heading or table: one a column, writable.
unsafe fn widths_of<'a>(ptr: *mut PxuiColumnWidth, len: usize, columns: usize) -> Option<&'a mut [PxuiColumnWidth]> {
    if len != columns || (len > 0 && ptr.is_null()) { return None; }
    Some(if len == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(ptr, len) } })
}

/// The heading's results into `out`.
fn write_head(h: &pxui_core::ui_lists::HeadResult, sort: Option<(i64, bool)>, out: &mut PxuiHeadResult) {
    respond(&h.response, &mut out.response);
    out.response.size = core::mem::size_of::<PxuiResponse>() as u32;
    (out.sorted, out.resized, out.has_sort) = (h.sorted as u8, h.resized as u8, sort.is_some() as u8);
    (out.ascending, out.sort_id) = (sort.is_some_and(|s| s.1) as u8, sort.map_or(0, |s| s.0));
}

/// `pxui_ui_section_list`'s body (and the static wrapper's `section_list`): `desc` checked, then the list keyed by `key` with
/// its trailer; `f` gets the borrowed handle, the part's index (`usize::MAX - 2` the trailer) and its cell: a row's box (the lead-in,
/// heading and trailer the empty box) and the part's logical content Y, enabled, nothing selected, focused or held.
pub unsafe fn section_list(ui: *mut PxuiUi, key: &str, desc: &PxuiSectionListDesc, ids: Option<&dyn Fn(usize) -> i64>, f: &mut dyn FnMut(&mut Ui, usize, &ListCell), out: &mut PxuiListResult) -> i32 {
    enter!(ui);
    if bad_key(key) || desc.overlay_scrollbar > 1 || desc.measured > 1 || desc.reorder > 1 || desc.reserved != 0
        || !geometry_ok(desc.count, desc.row_height, [desc.leading_height, desc.header_height, desc.trailing_height]) { return unsafe { fail(ui, "invalid section list descriptor") }; }
    let section = pxui_types::ui::SectionList { count: desc.count, row_height: desc.row_height, leading_height: desc.leading_height, header_height: desc.header_height, trailing_height: desc.trailing_height, overlay_scrollbar: desc.overlay_scrollbar != 0, measured: desc.measured != 0, reorder: desc.reorder != 0 };
    // a row builds inside its box (the entered node); the other parts get the empty box
    let body = unsafe { engine(ui) }.section_list(key, &section, ids, true, &mut |e, index, y| {
        let handle = if index < usize::MAX - 2 { e.entered() } else { Default::default() };
        f(e, index, &ListCell { enabled: true, handle, y, ..ListCell::default() })
    });
    let (drop_at, moved) = { let e = unsafe { engine(ui) }; (e.list_dropped(), e.list_moved()) };
    write_list(&ListResult { handle: body, drop_at, moved, ..ListResult::default() }, None, out);
    0
}

/// `pxui_ui_selection_list`'s body (and the static wrapper's `selection_list`): `desc` checked (flags, geometry, a flag span of
/// `count` bytes at least), then the list keyed by `key`; `cell` gets the borrowed handle, the part's index and its cell record.
/// `drop_look` is the record's drop look as a typed value (the export decodes and checks `has_drop_look` and the look; the static
/// wrapper passes its own, so it is never encoded and decoded).
pub unsafe fn selection_list(ui: *mut PxuiUi, key: &str, desc: &PxuiSelectionListDesc, drop_look: Option<pxui_types::theme::Look>, ids: Option<&dyn Fn(usize) -> i64>, cell: &mut dyn FnMut(&mut Ui, usize, &ListCell), out: &mut PxuiListResult) -> i32 {
    enter!(ui);
    if bad_key(key) || [desc.multi, desc.fit, desc.open_on_click, desc.overlay_scrollbar, desc.measured, desc.reorder, desc.bare, desc.has_revision].iter().any(|f| *f > 1) || desc.reserved != [0; 3]
        || !geometry_ok(desc.count, desc.row_height, [desc.leading_height, desc.header_height, desc.trailing_height])
        || desc.rows.len < desc.count || (desc.count > 0 && desc.rows.ptr.is_null()) { return unsafe { fail(ui, "invalid selection list descriptor") }; }
    let flags: &mut [u8] = if desc.count == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.rows.ptr, desc.count) } };
    let mut anchor = (desc.anchor < desc.count).then_some(desc.anchor);
    let list = pxui_types::ui::SelectionList { multi: desc.multi != 0, row_height: desc.row_height, leading_height: desc.leading_height, header_height: desc.header_height, trailing_height: desc.trailing_height,
        overlay_scrollbar: desc.overlay_scrollbar != 0, measured: desc.measured != 0, fit: desc.fit != 0, open_on_click: desc.open_on_click != 0,
        reorder: desc.reorder != 0, drop_look, bare: desc.bare != 0, ..pxui_types::ui::SelectionList::DEFAULT };
    let revision = (desc.has_revision != 0).then_some(desc.revision);
    let r = unsafe { engine(ui) }.selection_rows(key, desc.count, ids.unwrap_or(&|i| i as i64), flags, &mut anchor, revision, &list, cell);
    write_list(&r, anchor, out);
    0
}

/// `pxui_ui_table_head`'s body (and the static wrapper's `table_head`): the columns read in one pass, the widths span one a column,
/// then the heading keyed by `key`.
pub unsafe fn table_head(ui: *mut PxuiUi, key: &str, desc: &PxuiTableHeadDesc, out: &mut PxuiHeadResult) -> i32 {
    enter!(ui);
    let (mut columns, mut titles) = ([ColumnSpec::default(); MAX_COLUMNS], [""; MAX_COLUMNS]);
    let n = if bad_key(key) || desc.has_sort > 1 || desc.ascending > 1 || desc.reserved != [0; 2] || desc.reserved2 != 0 || !desc.h.is_finite() || desc.h <= 0.0 { None } else { unsafe { read_columns(desc.columns, &mut columns, &mut titles) } };
    let Some(n) = n else { return unsafe { fail(ui, "invalid table heading descriptor") } };
    let Some(widths) = (unsafe { widths_of(desc.widths.ptr, desc.widths.len, n) }) else { return unsafe { fail(ui, "invalid table heading widths") } };
    let mut sort = (desc.has_sort != 0).then_some((desc.sort_id, desc.ascending != 0)).filter(|(id, _)| columns[..n].iter().any(|c| c.id == *id));
    let h = unsafe { engine(ui) }.table_head(key, &columns[..n], &titles[..n], widths, &mut sort, desc.h);
    write_head(&h, sort, out);
    0
}

/// `pxui_ui_table`'s body (and the static wrapper's `table`): the columns read in one pass, widths, flags and geometry checked, then
/// the table keyed by `key`; `cell` gets the borrowed handle, the row, the column and the row's cell record.
pub unsafe fn table(ui: *mut PxuiUi, key: &str, desc: &PxuiTableDesc, ids: Option<&dyn Fn(usize) -> i64>, cell: &mut dyn FnMut(&mut Ui, usize, usize, &ListCell), out: &mut PxuiTableResult) -> i32 {
    enter!(ui);
    let (mut columns, mut titles) = ([ColumnSpec::default(); MAX_COLUMNS], [""; MAX_COLUMNS]);
    let positive = |v: f32| v.is_finite() && v > 0.0;
    let n = if bad_key(key) || desc.multi > 1 || desc.has_sort > 1 || desc.ascending > 1 || desc.has_revision > 1 || desc.reserved2 != [0; 6] || !positive(desc.head_h)
        || !geometry_ok(desc.rows, desc.row_h, [0.0; 3]) || desc.flags.len < desc.rows || (desc.rows > 0 && desc.flags.ptr.is_null()) { None } else { unsafe { read_columns(desc.columns, &mut columns, &mut titles) } };
    let Some(n) = n else { return unsafe { fail(ui, "invalid table descriptor") } };
    let Some(widths) = (unsafe { widths_of(desc.widths.ptr, desc.widths.len, n) }) else { return unsafe { fail(ui, "invalid table widths") } };
    let flags: &mut [u8] = if desc.rows == 0 { &mut [] } else { unsafe { core::slice::from_raw_parts_mut(desc.flags.ptr, desc.rows) } };
    let mut anchor = (desc.anchor < desc.rows).then_some(desc.anchor);
    let mut sort = (desc.has_sort != 0).then_some((desc.sort_id, desc.ascending != 0)).filter(|(id, _)| columns[..n].iter().any(|c| c.id == *id));
    let revision = (desc.has_revision != 0).then_some(desc.revision);
    let (r, h) = unsafe { engine(ui) }.table(key, &columns[..n], &titles[..n], widths, &mut sort, desc.rows, ids.unwrap_or(&|i| i as i64), flags, &mut anchor, revision, desc.multi != 0, desc.row_h, desc.head_h, desc.head_variant, cell);
    out.reserved = 0;
    out.rows.size = core::mem::size_of::<PxuiListResult>() as u32;
    write_list(&r, anchor, &mut out.rows);
    out.head.size = core::mem::size_of::<PxuiHeadResult>() as u32;
    write_head(&h, sort, &mut out.head);
    0
}

#[cfg(test)]
mod segment_tests {
    use super::{MAX_LABEL_BYTES, MAX_OPTIONS, Overlay, Segment, checked_segments, overlay_ok};

    /// Short segments pass as they are in one pass; one past the display cap takes the clipping pass (clipped, still accepted); more
    /// than `MAX_OPTIONS` are refused.
    #[test]
    fn segments_are_checked_in_one_pass_and_clipped_only_when_long() {
        let short = [Segment::new("a"), Segment::new("b")];
        let mut spare = Vec::new();
        let (list, ok) = checked_segments(&short, &mut spare);
        assert!(ok && core::ptr::eq(list, &short[..]));
        let long = "é".repeat(MAX_LABEL_BYTES);
        let one = [Segment::new(&long)];
        let mut spare = Vec::new();
        let (list, ok) = checked_segments(&one, &mut spare);
        assert!(ok && list[0].text.len() <= MAX_LABEL_BYTES && long.starts_with(list[0].text));
        let many = vec![Segment::new("x"); MAX_OPTIONS + 1];
        let mut spare = Vec::new();
        assert!(!checked_segments(&many, &mut spare).1);
    }

    /// `overlay_ok` on the bits takes what the float compares took: finite places, sizes in `0.0..=f32::MAX` (-0.0 included).
    #[test]
    fn overlay_ok_takes_finite_places_and_sizes_from_zero_to_max() {
        let base = Overlay::DEFAULT;
        assert!(overlay_ok(&base));
        assert!(overlay_ok(&Overlay { at: (-5.0, f32::MAX), w: Some(-0.0), h: Some(f32::MAX), max_h: Some(-1.0), ..base }));
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(!overlay_ok(&Overlay { at: (bad, 0.0), ..base }));
            assert!(!overlay_ok(&Overlay { max_h: Some(bad), ..base }));
        }
        for bad in [f32::NAN, f32::INFINITY, -1.0, -f32::MIN_POSITIVE] {
            assert!(!overlay_ok(&Overlay { w: Some(bad), ..base }));
            assert!(!overlay_ok(&Overlay { padding: bad, ..base }));
        }
    }
}
/// The kit segmented control with options, the static wrapper's `segmented` entry:
/// `selected` the choice (`None`: none; past the options: the last), `h` (0 the theme's control height), `grow`, `states` in place of the
/// theme's `segment`, `enabled`, `w` the strip's width (0: as `grow` says), `padding` the text's inset, `min_segment` a segment's least width
/// (each finite and not negative), `focus_on_click` a press on an option focusing it. `chosen` is the choice after this frame's input, `secondary` the option a secondary press began on.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn segmented(ui: *mut PxuiUi, key: &str, name: &str, options: &[Segment], selected: Option<usize>, enabled: bool, h: f32, grow: bool, states: Option<&States>,
    w: f32, padding: f32, min_segment: f32, focus_on_click: bool, out: &mut EngineResponse, chosen: &mut Option<usize>, secondary: &mut Option<usize>) -> i32 {
    enter!(ui);
    let name = clip(name);
    let mut spare = Vec::new();
    let (options, ok) = checked_segments(options, &mut spare);
    if bad_key(key) || !ok { return unsafe { fail(ui, "invalid widget strings") }; }
    unsafe { segmented_body(ui, key, name, options.len(), &|i| options[i], selected, enabled, h, grow, states, w, padding, min_segment, focus_on_click, out, chosen, secondary) }
}
