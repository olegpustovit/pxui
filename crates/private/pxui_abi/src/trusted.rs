//! The trusted entries: the bodies of the hot exports, minus what only an untrusted caller needs. The static wrapper's caller is Rust, so
//! its strings are UTF-8, its enums in range, its references non-null and a panic unwinds through these frames as it did through the engine
//! itself. A `pxui_*` export of the same name checks what C hands over (poison, null, UTF-8, tags) and then calls its entry here, so the
//! two paths run one body. What Rust types cannot promise stays checked in the body (a text past its role's cap, `state::MAX_*_BYTES`, a
//! stale box, a number that is not finite). Each entry counts itself as one call into the library. A builder entry is refused outside a
//! frame (`enter!`).
use crate::node::declare;
use crate::state::{get, Inner, MAX_KEY_BYTES, MAX_LABEL_BYTES, MAX_TEXT_BYTES, MAX_TOTAL_TEXT_BYTES, PxuiUi, boxed, call_inner, fail, note_call, to_abi_box};
use crate::{PxuiBox, PxuiSignal, Str};
use pxui_types::ffi::{PxuiResponse, PxuiTextStyle};
use pxui_types::{text::LabelFields, TextGlyph, TextMetrics, TextSpans};
use pxui_core::ui::{Segment, SegmentedOpts, ToggleOpts, Ui};
use pxui_types::ui::{Checkbox, Choice, Dropdown, Overlay, SelectionList, Slider, TableResponse, TableView};
use pxui_types::{Axis, FrameIn, FrameOut, Node, Size, ffi::Input, geom::Edges, layout::{Along, Justify}, theme::{State, States, ThemeSnapshot}};


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
    let Some(label) = (unsafe { utf8(label, MAX_KEY_BYTES) }) else { unsafe { note_call((*ui).shared) }; return unsafe { fail(ui, "invalid node label") } };
    unsafe { node(ui, label, desc, out) }
}

/// `pxui_ui_node` of a Rust label (`Key::label`): UTF-8 by its type, so only its length is checked (the static wrapper's entry).
#[inline(always)]
pub unsafe fn node_str(ui: *mut PxuiUi, label: &str, desc: &Node, out: *mut PxuiBox) -> i32 {
    if long_key(label) { unsafe { note_call((*ui).shared) }; return unsafe { fail(ui, "invalid node label") } }
    unsafe { node(ui, label, desc, out) }
}

/// `pxui_ui_node` of a label already checked (UTF-8, at most `MAX_KEY_BYTES`): the export's entry, which checks it as C hands it over.
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

#[inline]
pub unsafe fn pxui_label(ui: *mut PxuiUi, label: Str) -> i32 {
    enter!(ui);
    let Some(label) = (unsafe { utf8(label, usize::MAX) }).map(clip) else { return unsafe { fail(ui, "invalid label text") } };
    unsafe { engine(ui) }.label(label, pxui_core::ui::Label::DEFAULT);
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

#[inline]
pub unsafe fn pxui_button(ui: *mut PxuiUi, label: Str, out: *mut u8) -> i32 {
    enter!(ui);
    let Some(label) = (unsafe { utf8(label, usize::MAX) }).map(clip) else { return unsafe { fail(ui, "invalid button arguments") } };
    unsafe { *out = engine(ui).button(label) as u8; }
    0
}

#[inline]
pub unsafe fn pxui_toggle(ui: *mut PxuiUi, label: Str, on: u8, out: *mut u8) -> i32 {
    enter!(ui);
    let Some(label) = (unsafe { utf8(label, usize::MAX) }).map(clip) else { return unsafe { fail(ui, "invalid toggle arguments") } };
    unsafe { *out = engine(ui).toggle(label, on != 0) as u8; }
    0
}

/// The plain kit toggle (`Toggle::DEFAULT`) keyed by `key`, `pxui_ui_toggle_plain`'s body: the engine's own default options, nothing built or checked
/// for them. A click is the flip of `on`, which the caller keeps.
#[inline(always)]
pub unsafe fn toggle_plain(ui: *mut PxuiUi, key: &str, text: &str, on: bool, out: &mut PxuiResponse) -> i32 {
    enter!(ui);
    let text = clip(text);
    if long_key(key) { return unsafe { fail(ui, "invalid toggle text") }; }
    let r = unsafe { engine(ui) }.toggle_opts(key, text, on, &ToggleOpts::DEFAULT);
    respond(&r, out);
    0
}

/// The kit toggle with options, the body of `pxui_ui_toggle_response` and `pxui_ui_toggle_plain`: `track` and `knob` in place of the theme's
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
    if long_key(key) { return unsafe { fail(ui, "invalid toggle text") }; }
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

/// The plain kit segmented control (`Segmented::DEFAULT`) keyed by `key`, named `name`, `pxui_ui_segmented_plain`'s body. `chosen` is the choice
/// (`selected` clamped to `options`, moved by a press or a key); `options` are within the engine's limits (`segments_ok`).
#[inline(always)]
pub unsafe fn segmented_plain(ui: *mut PxuiUi, key: &str, name: &str, options: &[Segment], selected: usize, out: &mut PxuiResponse, chosen: &mut usize) -> i32 {
    let (mut at, mut secondary) = (None, None);
    let rc = unsafe { segmented(ui, key, name, options, Some(selected), true, 0.0, false, None, 0.0, 8.0, 0.0, false, out, &mut at, &mut secondary) };
    *chosen = at.unwrap_or(0);
    rc
}

/// The kit segmented control with options, the body of `pxui_ui_segmented_result`, `pxui_ui_segmented_response` and `pxui_ui_segmented_plain`:
/// `selected` the choice (`None`: none; past the options: the last), `h` (0 the theme's control height), `grow`, `states` in place of the
/// theme's `segment`, `enabled`, `w` the strip's width (0: as `grow` says), `padding` the text's inset, `min_segment` a segment's least width
/// (each finite and not negative), `focus_on_click` a press on an option focusing it. `chosen` is the choice after this frame's input, `secondary` the option a secondary press began on.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn segmented(ui: *mut PxuiUi, key: &str, name: &str, options: &[Segment], selected: Option<usize>, enabled: bool, h: f32, grow: bool, states: Option<&States>,
    w: f32, padding: f32, min_segment: f32, focus_on_click: bool, out: &mut PxuiResponse, chosen: &mut Option<usize>, secondary: &mut Option<usize>) -> i32 {
    enter!(ui);
    let name = clip(name);
    let mut spare = Vec::new();
    let options = clip_segments(options, &mut spare);
    if long_key(key) || !segments_ok(options) { return unsafe { fail(ui, "invalid widget strings") }; }
    if ![h, w, padding, min_segment].iter().all(|v| (0.0..=f32::MAX).contains(v)) { return unsafe { fail(ui, "invalid segmented descriptor") }; }
    let mut opts = SegmentedOpts { enabled, h: (h > 0.0).then_some(h), grow, states: None, w: (w > 0.0).then_some(w), padding, min_segment, focus_on_click };
    if let Some(states) = states { opts.states = Some(*states); }
    let (r, at, pressed) = unsafe { engine(ui) }.segmented_opts(key, name, options, selected, &opts);
    respond(&r, out);
    (*chosen, *secondary) = (at, pressed);
    0
}

/// A text field: `out_changed` says whether the user changed the text, `changed` then holds the new text (borrowed until the next frame).
#[inline]
pub unsafe fn pxui_text_field(ui: *mut PxuiUi, label: Str, value: Str, changed: *mut Str, out_changed: *mut u8) -> i32 {
    enter!(ui);
    let (Some(label), Some(value)) = (unsafe { utf8(label, usize::MAX) }.map(clip), unsafe { utf8(value, MAX_TEXT_BYTES) }) else { return unsafe { fail(ui, "invalid text field arguments") } };
    unsafe { *out_changed = 0; *changed = Str::default(); }
    if let Some(value) = unsafe { engine(ui) }.text_field(label, value) { let Ok(value) = (unsafe { &*ui }).give(value) else { return crate::PXUI_ERR_STATE }; unsafe { *changed = value; *out_changed = 1; } }
    0
}

/// The theme in effect, lent.
#[inline]
pub unsafe fn pxui_ui_theme(ui: *mut PxuiUi) -> *const ThemeSnapshot {
    enter!(any ui, core::ptr::null());
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
    if long_key(label) { return unsafe { fail(ui, "invalid scope label") }; }
    let shared = unsafe { (*ui).shared };
    unsafe { engine(ui) }.scope(label, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

#[inline]
pub unsafe fn col(ui: *mut PxuiUi, padding: f32, label: &str, f: Inner) -> i32 {
    enter!(ui);
    if long_key(label) || !padding.is_finite() { return unsafe { fail(ui, "invalid column arguments") }; }
    let shared = unsafe { (*ui).shared };
    unsafe { engine(ui) }.col(padding, label, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

#[inline]
pub unsafe fn view(ui: *mut PxuiUi, label: &str, revision: u64, width: Size, height: Size, flat: bool, f: Inner, out: *mut PxuiBox) -> i32 {
    enter!(ui);
    if long_key(label) || !finite(width) || !finite(height) { return unsafe { fail(ui, "invalid view arguments") }; }
    let shared = unsafe { (*ui).shared };
    let engine = unsafe { engine(ui) };
    let mut build = |engine: &mut Ui| call_inner(shared, engine, f);
    let handle = if flat { engine.view_flat(label, revision, width, height, &mut build) } else { engine.view(label, revision, width, height, &mut build) };
    unsafe { *out = to_abi_box(handle); }
    0
}

/// The body of `pxui_ui_view_keyed`: the engine's `view_of` (sized by its parent).
#[inline]
pub unsafe fn view_of(ui: *mut PxuiUi, key: &str, revision: u64, f: Inner, out: *mut PxuiBox) -> i32 {
    enter!(ui);
    if long_key(key) { return unsafe { fail(ui, "invalid view arguments") }; }
    let shared = unsafe { (*ui).shared };
    let handle = unsafe { engine(ui) }.view_of(key, &revision, &mut |engine: &mut Ui| call_inner(shared, engine, f));
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
/// Whether an identifier is past its cap (`MAX_KEY_BYTES`).
#[inline]
fn long_key(value: &str) -> bool { value.len() > MAX_KEY_BYTES }

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
pub unsafe fn checkbox(ui: *mut PxuiUi, key: &str, text: &str, on: bool, checkbox: &Checkbox, out: &mut PxuiResponse) -> i32 {
    enter!(ui);
    let text = clip(text);
    if long_key(key) { return unsafe { fail(ui, "invalid checkbox text") }; }
    let r = unsafe { engine(ui) }.checkbox(key, text, on, checkbox);
    respond(&r, out);
    0
}

/// A kit choice keyed by `key`: `kind` 0 a radio group and 1 tabs (`name` their accessible name), 2 a segmented control of plain
/// options (the 1.x `pxui_ui_segmented`), 3 a dropdown (`name` its shown and accessible text); `chosen` is the choice. Any other kind
/// is refused.
#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn choice(ui: *mut PxuiUi, kind: u32, key: &str, name: &str, options: &[&str], selected: usize, variant: u16, out: &mut PxuiResponse, chosen: &mut usize) -> i32 {
    enter!(ui);
    let name = clip(name);
    let mut spare = Vec::new();
    let options = clip_strs(options, &mut spare);
    if long_key(key) || !options_ok(options) { return unsafe { fail(ui, "invalid widget strings") }; }
    let engine = unsafe { engine(ui) };
    let (r, at) = match kind {
        0 => engine.radio_legacy(key, options, selected, &Choice { name, variant }),
        1 => engine.tabs(key, options, selected, &Choice { name, variant }),
        2 => { let (r, at, _) = engine.segmented_options(key, name, options.len(), &|i| Segment::new(options[i]), Some(selected), &SegmentedOpts::DEFAULT); (r, at.unwrap_or(0)) }
        3 => engine.dropdown_legacy(key, name, options, selected, &Dropdown { variant }),
        _ => return unsafe { fail(ui, "invalid choice kind") },
    };
    respond(&r, out);
    *chosen = at;
    0
}

/// A pick-one control of `items` keyed by `key`: `kind` 0 a radio group (`name` its accessible name), 3 a dropdown (`name` its shown
/// and accessible text); an item with an id is keyed by it. `chosen` is the choice (none: none), `secondary` the item a secondary
/// press began on. Any other kind is refused.
#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn pick(ui: *mut PxuiUi, kind: u32, key: &str, name: &str, items: &[Segment], selected: Option<usize>, out: &mut PxuiResponse, chosen: &mut Option<usize>, secondary: &mut Option<usize>) -> i32 {
    enter!(ui);
    let name = clip(name);
    let mut spare = Vec::new();
    let items = clip_segments(items, &mut spare);
    if long_key(key) || !segments_ok(items) { return unsafe { fail(ui, "invalid widget strings") }; }
    let engine = unsafe { engine(ui) };
    let (r, at, pressed) = match kind {
        0 => engine.pick_radio(key, name, items.len(), &|i| items[i], selected, 0),
        3 => engine.pick_dropdown(key, name, items.len(), &|i| items[i], selected, 0),
        _ => return unsafe { fail(ui, "invalid pick kind") },
    };
    respond(&r, out);
    (*chosen, *secondary) = (at, pressed);
    0
}

/// The kit slider with its phases: `scrub` gets the held preview and the commit (flags 1 and 2), the value in `preview` when neither.
#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn slider_result(ui: *mut PxuiUi, key: &str, text: &str, value: f32, min: f32, max: f32, slider: &Slider, out: &mut PxuiResponse, scrub: &mut pxui_types::ffi::PxuiScrubOut) -> i32 {
    enter!(ui);
    let text = clip(text);
    if !value.is_finite() || !min.is_finite() || !max.is_finite() || min > max || long_key(key) { return unsafe { fail(ui, "invalid slider arguments") }; }
    let (r, v, preview, commit) = unsafe { engine(ui) }.slider_result(key, text, value, min..=max, slider);
    respond(&r, out);
    *scrub = pxui_types::ffi::PxuiScrubOut { flags: u32::from(preview.is_some()) | u32::from(commit.is_some()) << 1 | u32::from(r.held) << 2, node: to_abi_box(r.handle),
        preview: preview.unwrap_or(v), commit: commit.unwrap_or_default(), ..Default::default() };
    0
}

/// The kit slider keyed by `key`, showing and named `text`; `value_out` is the value, held in the range.
#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn slider(ui: *mut PxuiUi, key: &str, text: &str, value: f32, min: f32, max: f32, slider: &Slider, out: &mut PxuiResponse, value_out: &mut f32) -> i32 {
    enter!(ui);
    let text = clip(text);
    if !value.is_finite() || !min.is_finite() || !max.is_finite() || min > max || long_key(key) { return unsafe { fail(ui, "invalid slider arguments") }; }
    let (r, v) = unsafe { engine(ui) }.slider_legacy(key, text, value, min..=max, slider);
    respond(&r, out);
    *value_out = v;
    0
}

/// A progress row keyed by `key`, showing and named `text`.
#[inline]
pub unsafe fn progress(ui: *mut PxuiUi, key: &str, text: &str, value: Option<f32>, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    let text = clip(text);
    if long_key(key) || value.is_some_and(|v| !v.is_finite()) { return unsafe { fail(ui, "invalid progress arguments") }; }
    *out = to_abi_box(unsafe { engine(ui) }.progress(key, text, value));
    0
}

/// A spinner keyed by `key`, named `name`.
#[inline]
pub unsafe fn spinner(ui: *mut PxuiUi, key: &str, name: &str, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    let name = clip(name);
    if long_key(key) { return unsafe { fail(ui, "invalid spinner text") }; }
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

/// The kit button with options, the body of `pxui_ui_button_with`, `pxui_ui_button_response`, `pxui_ui_button_content` and
/// `pxui_ui_button_plain`: `states` in place of the theme's button, `w` and `h` (`button_size`), `justify` its place in the line, `pad`
/// (finite, non-negative) inside the box, `centered` the box exactly its size with its text centred, `label` the text's style (one that
/// resolves), `axis`, `gap` (finite, non-negative) and `content_align` the contents' layout, `focus_on_click` a press focusing it,
/// `lead` the leading icon's glyph or `lead_draw` (the app's C drawing with its `lead_user` and `lead_word`, as `icon_button_with` takes
/// them), `detail` and `accessory` (empty: none; clipped as the text is) in `detail_label` and `accessory_label` (ones that resolve), `content` built in the box in place of `text` and given the button's state. What happened goes to `out`; with `signal`, the box's
/// whole signal too.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn button_with(ui: *mut PxuiUi, key: &str, text: &str, name: Option<&str>, states: Option<&States>, w: Size, h: Size, justify: Justify, pad: Edges,
    enabled: bool, selected: bool, danger: bool, centered: bool, label: Option<&pxui_types::text::LabelFields>, axis: Option<Axis>, gap: f32, content_align: Option<Along>, focus_on_click: bool,
    lead: &str, lead_draw: pxui_types::ffi::PxuiIconDraw, lead_user: *mut core::ffi::c_void, lead_word: u32, detail: &str, accessory: &str,
    detail_label: Option<&pxui_types::text::LabelFields>, accessory_label: Option<&pxui_types::text::LabelFields>,
    content: Option<&mut dyn FnMut(*mut PxuiUi, State)>, out: &mut PxuiResponse, signal: Option<&mut PxuiSignal>) -> i32 {
    enter!(ui);
    let text = clip(text);
    let name = name.map(clip);
    let parts = [clip(lead), clip(detail), clip(accessory)];
    if long_key(key) { return unsafe { fail(ui, "invalid button text") }; }
    if !button_size(w) || !button_size(h) || ![pad.l, pad.t, pad.r, pad.b, gap].iter().all(|v| (0.0..=f32::MAX).contains(v)) { return unsafe { fail(ui, "invalid button descriptor") }; }
    let (e, shared) = (unsafe { engine(ui) }, unsafe { (*ui).shared });
    if [label, detail_label, accessory_label].iter().any(|l| l.is_some_and(|l| e.resolve(l).is_none())) { return unsafe { fail(ui, "invalid button label") }; }
    let ink = if danger { Some(e.theme().tokens.invalid) } else { None };
    let lead_draw = lead_draw.map(|f| (crate::paint::icon_bridge as pxui_core::ui::IconDraw, [f as usize, lead_user as usize, shared as usize, lead_word as usize]));
    // the looks and the label are copied only when given: `copied()` copies the whole record either way
    let mut opts = pxui_core::ui::ButtonOpts { states: None, w, h, enabled, selected, justify, padding: pad, ink, bare: false, icon: false, centered, label: None, axis, gap, content_align, focus_on_click,
        lead_draw, detail_label: detail_label.copied(), accessory_label: accessory_label.copied() };
    if let Some(states) = states { opts.states = Some(*states); }
    if let Some(label) = label { opts.label = Some(*label); }
    let mut run = content.map(|f| move |engine: &mut Ui, state: State| call_inner(shared, engine, &mut |raw| f(raw, state)));
    let body = run.as_mut().map(|f| f as &mut dyn FnMut(&mut Ui, State));
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
pub unsafe fn button_plain(ui: *mut PxuiUi, key: &str, text: &str, out: &mut PxuiResponse) -> i32 {
    enter!(ui);
    let text = clip(text);
    if long_key(key) { return unsafe { fail(ui, "invalid button text") }; }
    let r = unsafe { engine(ui) }.button_opts(key, text, None, &pxui_core::ui::ButtonOpts::DEFAULT, [""; 3], None, None);
    respond(&r, out);
    0
}

/// The plain kit icon button (`IconButton::DEFAULT`) showing the glyph `glyph`, `pxui_ui_icon_button_plain`'s body: as
/// `button_plain`.
#[inline(always)]
pub unsafe fn icon_button_plain(ui: *mut PxuiUi, key: &str, glyph: &str, name: &str, out: &mut PxuiResponse) -> i32 {
    enter!(ui);
    let glyph = clip(glyph);
    let name = clip(name);
    if long_key(key) { return unsafe { fail(ui, "invalid icon button text") }; }
    let r = unsafe { engine(ui) }.icon_button_text(key, glyph, None, name, &pxui_core::ui::IconButton::DEFAULT, None);
    respond(&r, out);
    0
}

/// The kit icon button with options, the body of `pxui_ui_icon_button_with`, `_response` and `_plain`: the `glyph`, or the app's
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
    if long_key(key) { return unsafe { fail(ui, "invalid icon button text") }; }
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

#[inline]
pub unsafe fn icon_button(ui: *mut PxuiUi, label: &str, icon: &str, out: &mut bool) -> i32 {
    enter!(ui);
    let label = clip(label);
    let icon = clip(icon);
    *out = unsafe { engine(ui) }.icon_button_text(label, icon, None, label, &pxui_core::ui::IconButton::DEFAULT, None).clicked;
    0
}

/// A badge keyed by `key`, named `name`, `value` its text.
#[inline]
pub unsafe fn badge(ui: *mut PxuiUi, key: &str, name: &str, value: &str, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    let name = clip(name);
    let value = clip(value);
    if long_key(key) { return unsafe { fail(ui, "invalid badge text") }; }
    *out = to_abi_box(unsafe { engine(ui) }.badge(key, name, value));
    0
}

/// The text a search field changed to, or none; kept by the library until the next frame.
#[inline]
pub unsafe fn search_field(ui: *mut PxuiUi, label: &str, icon: &str, value: &str, placeholder: &str, out: &mut Option<Str>) -> i32 {
    enter!(ui);
    let label = clip(label);
    let icon = clip(icon);
    let placeholder = clip(placeholder);
    if value.len() > MAX_TEXT_BYTES { return unsafe { fail(ui, "invalid search field text") }; }
    let found = unsafe { engine(ui) }.search_field(label, icon, value, placeholder);
    *out = match found { None => None, Some(text) => match unsafe { get(ui) } { Some(state) => match state.lend(text) { Ok(text) => Some(text), Err(code) => return code }, None => Some(Str::default()) } };
    0
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
    if long_key(key) { return unsafe { fail(ui, "invalid inspector toggle text") }; }
    *out = unsafe { engine(ui) }.inspector_toggle(key, label, value);
    0
}

#[inline]
pub unsafe fn inspector_button(ui: *mut PxuiUi, key: &str, label: &str, out: &mut bool) -> i32 {
    enter!(ui);
    let label = clip(label);
    if long_key(key) { return unsafe { fail(ui, "invalid inspector button text") }; }
    *out = unsafe { engine(ui) }.inspector_button(key, label);
    0
}

#[inline]
pub unsafe fn inspector_readout(ui: *mut PxuiUi, width: Option<f32>, key: &str, label: &str, value: &str) -> i32 {
    enter!(ui);
    let label = clip(label);
    let value = clip(value);
    if long_key(key) || width.is_some_and(|w| !w.is_finite() || w < 0.0) { return unsafe { fail(ui, "invalid inspector readout text") }; }
    unsafe { engine(ui) }.inspector_readout(width, key, label, value);
    0
}

#[inline]
pub unsafe fn inspector_message(ui: *mut PxuiUi, key: &str, message: &str) -> i32 {
    enter!(ui);
    let message = clip(message);
    if long_key(key) { return unsafe { fail(ui, "invalid inspector message text") }; }
    unsafe { engine(ui) }.inspector_message(key, message);
    0
}

/// The row's box and its control's x, y and width.
#[inline]
pub unsafe fn inspector_row(ui: *mut PxuiUi, width: Option<f32>, pin: Option<bool>, key: &str, label: &str, out: &mut (PxuiBox, f32, f32, f32)) -> i32 {
    enter!(ui);
    let label = clip(label);
    if long_key(key) || width.is_some_and(|w| !w.is_finite() || w < 0.0) { return unsafe { fail(ui, "invalid inspector row text") }; }
    let (row, x, y, w) = unsafe { engine(ui) }.inspector_row(width, pin, key, label);
    *out = (to_abi_box(row), x, y, w);
    0
}

#[inline]
pub unsafe fn inspector_section(ui: *mut PxuiUi, key: &str, expanded: bool, set_expanded: Option<bool>, label: &str, f: Inner, out: &mut bool) -> i32 {
    enter!(ui);
    let label = clip(label);
    if long_key(key) { return unsafe { fail(ui, "invalid inspector section text") }; }
    let shared = unsafe { (*ui).shared };
    *out = unsafe { engine(ui) }.inspector_section(key, expanded, set_expanded, label, &mut |engine: &mut Ui| call_inner(shared, engine, f));
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
fn overlay_ok(overlay: &Overlay) -> bool {
    [overlay.at.0, overlay.at.1, overlay.max_h.unwrap_or_default()].iter().all(|v| v.is_finite())
        && [overlay.w.unwrap_or_default(), overlay.h.unwrap_or_default(), overlay.padding].iter().all(|v| (0.0..=f32::MAX).contains(v))
}

/// A panel keyed by `key`, placed, dressed and named by `overlay`.
#[inline]
pub unsafe fn panel(ui: *mut PxuiUi, key: &str, overlay: &Overlay, f: Inner, root: &mut PxuiBox) -> i32 {
    enter!(ui);
    let overlay = &Overlay { name: overlay.name.map(clip), ..*overlay };
    if long_key(key) || !overlay_ok(overlay) { return unsafe { fail(ui, "invalid panel descriptor") }; }
    let shared = unsafe { (*ui).shared };
    *root = to_abi_box(unsafe { engine(ui) }.panel_root(key, overlay, &mut |engine: &mut Ui| call_inner(shared, engine, f)));
    0
}

/// A popup keyed by `key`, placed, dressed and named by `overlay`; `out` says whether it is open, `root` its root (empty: dismissed).
#[inline]
pub unsafe fn popup(ui: *mut PxuiUi, key: &str, overlay: &Overlay, f: Inner, out: &mut bool, root: &mut PxuiBox) -> i32 {
    enter!(ui);
    let overlay = &Overlay { name: overlay.name.map(clip), ..*overlay };
    if long_key(key) || !overlay_ok(overlay) { return unsafe { fail(ui, "invalid popup descriptor") }; }
    let shared = unsafe { (*ui).shared };
    let open = unsafe { engine(ui) }.popup_root(key, overlay, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    (*out, *root) = (open.is_some(), open.map_or_else(PxuiBox::default, to_abi_box));
    0
}

/// A scroll view keyed by `key`, named `name` (empty: none), laid out by `node` (no grid; a transition must be valid).
#[inline]
pub unsafe fn scroll_view(ui: *mut PxuiUi, key: &str, name: &str, node: &Node, f: Inner, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    let name = clip(name);
    let bad_curve = matches!(&node.transition, pxui_types::node::OptTransition::Some(curve) if !curve.valid());
    if long_key(key) || !finite(node.w) || !finite(node.h) || node.grid.is_some() || bad_curve { return unsafe { fail(ui, "invalid scroll view arguments") }; }
    let shared = unsafe { (*ui).shared };
    let engine = unsafe { engine(ui) };
    let b = engine.scroll_view(key, *node, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    if !name.is_empty() { engine.a11y_name(&b, name); }
    *out = to_abi_box(b);
    0
}

// ---- collections: the engine asks `text` for a row's text and the entry checks what it hands back

const MAX_ITEMS: usize = 1_000_000;


/// `put` of a row's text clipped (`clip`), refusing (and recording) one past `MAX_TOTAL_TEXT_BYTES` in all instead of passing it on.
#[inline]
fn bounded(put: &mut dyn FnMut(&str), bad: &core::cell::Cell<bool>, total: &core::cell::Cell<usize>, value: &str) {
    let value = clip(value);
    let sum = total.get().saturating_add(value.len());
    if sum > MAX_TOTAL_TEXT_BYTES { bad.set(true); return; }
    total.set(sum);
    put(value);
}

/// The C text-row `list_view` (`pxui_ui_list_view`): each row's text from `text`, the kit's row label (rows are not named).
#[inline]
pub unsafe fn list_view_text(ui: *mut PxuiUi, label: &str, count: usize, multi: bool, selected: &mut [u8], text: &mut dyn FnMut(usize, &mut dyn FnMut(&str)), out: &mut Option<usize>) -> i32 {
    enter!(ui);
    if long_key(label) || count > MAX_ITEMS || selected.len() != count { return unsafe { fail(ui, "invalid list view descriptor") }; }
    let (bad, total, text) = (core::cell::Cell::new(false), core::cell::Cell::new(0), core::cell::RefCell::new(text));
    *out = unsafe { engine(ui) }.selection_list(label, count, selected, &SelectionList { multi }, &mut |ui, row| (text.borrow_mut())(row, &mut |value| bounded(&mut |t| { ui.row_text(t); }, &bad, &total, value))).activated;
    if bad.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    0
}

/// The C text-row `tree` (`pxui_ui_tree`): a preorder tree; `visible` holds a row per node, `open` and `nodes` one flag and one node each.
#[inline]
pub unsafe fn tree_text(ui: *mut PxuiUi, label: &str, nodes: &[pxui_types::ffi::PxuiTreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], text: &mut dyn FnMut(usize, &mut dyn FnMut(&str)), out: &mut (Option<usize>, usize)) -> i32 {
    enter!(ui);
    if long_key(label) || nodes.len() > MAX_ITEMS || open.len() != nodes.len() || visible.len() < nodes.len() { return unsafe { fail(ui, "invalid tree descriptor") }; }
    // the engine's node and the C node share a layout (asserted where the exports are), so the caller's array is the engine's slice
    let nodes = unsafe { core::slice::from_raw_parts(nodes.as_ptr().cast::<pxui_core::ui_widgets::TreeNode>(), nodes.len()) };
    let (bad, total, text) = (core::cell::Cell::new(false), core::cell::Cell::new(0), core::cell::RefCell::new(text));
    let r = unsafe { engine(ui) }.tree(label, nodes, open, selected, visible, &mut |ui, row| (text.borrow_mut())(row, &mut |value| bounded(&mut |t| ui.item_text(t), &bad, &total, value)));
    *out = (r.activated, r.shown);
    if bad.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    0
}

/// The C text-cell `table_view` (`pxui_ui_table_view`): each cell's text from `cell`, naming the cell.
#[inline]
pub unsafe fn table_view_text(ui: *mut PxuiUi, label: &str, titles: &[&str], widths: &mut [f32], rows: usize, sort: Option<(usize, bool)>, selected: &mut Option<usize>, cell: &mut dyn FnMut(usize, usize, &mut dyn FnMut(&str)), out: &mut Option<usize>) -> i32 {
    enter!(ui);
    let mut spare = Vec::new();
    let titles = clip_strs(titles, &mut spare);
    if long_key(label) || rows > MAX_ITEMS || titles.len() > 4096 || widths.len() != titles.len() || !options_ok(titles) { return unsafe { fail(ui, "invalid table descriptor") }; }
    if widths.iter().any(|w| !w.is_finite() || *w < 0.0) || sort.is_some_and(|(column, _)| column >= titles.len()) { return unsafe { fail(ui, "invalid table widths or sort column") }; }
    let (bad, total, cell) = (core::cell::Cell::new(false), core::cell::Cell::new(0), core::cell::RefCell::new(cell));
    let engine = unsafe { engine(ui) };
    let r = engine.table_view(label, titles, widths, rows, selected, &TableView { sort, text: None }, &mut |ui, row, column| (cell.borrow_mut())(row, column, &mut |value| bounded(&mut |t| ui.cell_text(t), &bad, &total, value)));
    name_table(engine, label, pxui_types::ui::shown(label));
    *out = r.sort;
    if bad.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    0
}

// ---- more containers and callbacks

/// Measured rows share the existing borrowed-handle callback protocol with virtual grids.
#[inline]
pub unsafe fn list_measured(ui:*mut PxuiUi,label:&str,count:usize,estimate_h:f32,f:&mut dyn FnMut(*mut PxuiUi,usize),out:&mut PxuiBox)->i32 {
    enter!(ui);
    if !unsafe { (*(*ui).shared).in_frame } { return crate::PXUI_ERR_STATE; }
    if long_key(label) || count>MAX_ITEMS || !estimate_h.is_finite() || estimate_h<=0.0 || !(count as f32*estimate_h).is_finite() { return unsafe { fail(ui,"invalid measured list arguments") }; }
    let shared=unsafe { (*ui).shared };
    *out=to_abi_box(unsafe { engine(ui) }.list_measured(label,count,estimate_h,&mut |engine:&mut Ui,index| {
        let mut borrowed=PxuiUi::borrowed(engine,shared);
        let _current=crate::state::Current::set(shared,&mut borrowed);
        f(&mut borrowed,index);
    }));
    0
}

/// A section list keyed by `key` (fixed or measured rows, as `section.measured`); `f` gets the borrowed handle, the part's index and its
/// logical content Y.
#[inline]
pub unsafe fn section_list(ui:*mut PxuiUi,key:&str,section:&pxui_types::ui::SectionList,id:Option<&dyn Fn(usize)->i64>,f:&mut dyn FnMut(*mut PxuiUi,usize,f32),out:&mut PxuiBox)->i32 {
    enter!(ui);
    if !unsafe { (*(*ui).shared).in_frame } {return crate::PXUI_ERR_STATE;}
    let total=section.leading_height+section.header_height+section.count as f32*section.row_height+section.trailing_height;
    if long_key(key) || section.count>MAX_ITEMS || !section.row_height.is_finite() || section.row_height<=0.0
        || !total.is_finite() || [section.leading_height,section.header_height,section.trailing_height].iter().any(|v|!v.is_finite() || *v<0.0) {return unsafe {fail(ui,"invalid section list geometry")};}
    let shared=unsafe {(*ui).shared};
    *out=to_abi_box(unsafe {engine(ui)}.section_list(key,section,id,&mut |engine:&mut Ui,index,y| {
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
    if long_key(label) || count > MAX_ITEMS || !cell_w.is_finite() || !cell_h.is_finite() || cell_w <= 0.0 || cell_h <= 0.0 { return unsafe { fail(ui, "invalid grid list descriptor") }; }
    *out = to_abi_box(unsafe { engine(ui) }.grid_list(label, count, cell_w, cell_h, cell));
    0
}

/// Two panes in a row (`axis` 0) or a column (1); `response` is the grip's, `moved` whether it moved along the axis and `out` the
/// ratio under the pointer while the grip is held and on its release.
#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn split(ui: *mut PxuiUi, label: &str, axis: pxui_types::Axis, ratio: f32, min_a: f32, min_b: f32, a: Inner, b: Inner, response: &mut PxuiResponse, moved: &mut bool, out: &mut Option<f32>) -> i32 {
    enter!(ui);
    if long_key(label) || ![ratio, min_a, min_b].iter().all(|v| v.is_finite()) || min_a < 0.0 || min_b < 0.0 || axis == pxui_types::Axis::Stack { return unsafe { fail(ui, "invalid split descriptor") }; }
    let shared = unsafe { (*ui).shared };
    let (r, m, at) = unsafe { engine(ui) }.split_result(label, axis, ratio, min_a, min_b, &mut |engine: &mut Ui| call_inner(shared, engine, a), &mut |engine: &mut Ui| call_inner(shared, engine, b));
    respond(&r, response);
    (*moved, *out) = (m, at);
    0
}

/// A sheet keyed by `key` by `anchor` at `overlay`'s placement and size.
#[inline]
pub unsafe fn anchored(ui: *mut PxuiUi, key: &str, anchor: pxui_types::geom::Rect, overlay: &Overlay, f: Inner, out: &mut PxuiBox) -> i32 {
    enter!(ui);
    let overlay = &Overlay { name: overlay.name.map(clip), ..*overlay };
    let placement = overlay.place;
    if long_key(key) || !overlay_ok(overlay) || anchor.w < 0.0 || anchor.h < 0.0 || placement.margin < 0.0
        || ![anchor.x, anchor.y, anchor.w, anchor.h, placement.gap, placement.margin].iter().all(|v| v.is_finite()) { return unsafe { fail(ui, "invalid anchored arguments") }; }
    let shared = unsafe { (*ui).shared };
    *out = to_abi_box(unsafe { engine(ui) }.anchored(key, anchor, overlay, &mut |engine: &mut Ui| call_inner(shared, engine, f)));
    0
}

/// A modal keyed by `key`, `overlay`'s width and name; `out` says whether it is open.
#[inline]
pub unsafe fn modal(ui: *mut PxuiUi, key: &str, overlay: &Overlay, f: Inner, out: &mut bool, root: &mut PxuiBox) -> i32 {
    enter!(ui);
    let overlay = &Overlay { name: overlay.name.map(clip), ..*overlay };
    if long_key(key) || !overlay_ok(overlay) { return unsafe { fail(ui, "invalid modal arguments") }; }
    let shared = unsafe { (*ui).shared };
    let open = unsafe { engine(ui) }.modal_root(key, overlay, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    (*out, *root) = (open.is_some(), open.map_or_else(PxuiBox::default, to_abi_box));
    0
}

/// Toasts keyed by `key`, each by its item id; `out` is the index of one clicked away or expired.
#[inline]
pub unsafe fn toasts(ui: *mut PxuiUi, key: &str, items: &[pxui_types::ui::Toast], seconds: f64, out: &mut Option<usize>) -> i32 {
    enter!(ui);
    if long_key(key) || items.len() > MAX_OPTIONS || !seconds.is_finite() || seconds < 0.0 { return unsafe { fail(ui, "invalid toasts") }; }
    let spare: Vec<pxui_types::ui::Toast>;
    let items = if items.iter().all(|t| !long(t.text)) { items } else { spare = items.iter().map(|t| pxui_types::ui::Toast::new(t.id, clip(t.text))).collect(); &spare };
    *out = unsafe { engine(ui) }.toasts_index(key, items, seconds);
    0
}

/// A modal dialog: `buttons` (at most 64) as `Ui::dialog` takes them, `style` its sheet; what the frame did goes to `out`.
#[inline]
pub unsafe fn dialog(ui: *mut PxuiUi, key: &str, title: &str, buttons: &[pxui_types::ui::DialogButton], style: &pxui_types::ui::Dialog, f: Inner, out: &mut pxui_types::ui::Chosen) -> i32 {
    enter!(ui);
    let title = clip(title);
    let mut spare = Vec::new();
    let buttons = if buttons.iter().all(|b| !long(b.text) && !long(b.detail)) { buttons } else { spare.extend(buttons.iter().map(|b| pxui_types::ui::DialogButton { text: clip(b.text), detail: clip(b.detail), ..*b })); &spare[..] };
    if long_key(key) || buttons.len() > 64 || !style.w.is_finite() || style.w < 0.0 { return unsafe { fail(ui, "invalid dialog arguments") }; }
    let shared = unsafe { (*ui).shared };
    *out = unsafe { engine(ui) }.dialog(key, title, buttons, style, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

/// The 1.x text table (`pxui_ui_table`) at `at`, as `table_view` with `TableView::text`: `cell` writes a cell's text through `put`;
/// the clicked row, if any.
#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn table(ui: *mut PxuiUi, at: (f32, f32), width: f32, height: f32, headers: &[&str], rows: i64, cell: &mut dyn FnMut(i64, i64, &mut dyn FnMut(&str)), label: &str, out: &mut Option<i64>) -> i32 {
    enter!(ui);
    let mut spare = Vec::new();
    let headers = clip_strs(headers, &mut spare);
    if long_key(label) || headers.len() > 256 || rows < 0 || rows as usize > MAX_ITEMS || !width.is_finite() || !height.is_finite() || width < 0.0 || height < 0.0 || !at.0.is_finite() || !at.1.is_finite() || !options_ok(headers) {
        return unsafe { fail(ui, "invalid table descriptor") };
    }
    let (bad, total) = (core::cell::Cell::new(false), core::cell::Cell::new(0));
    let text = TableView::text(pxui_types::geom::Rect::new(at.0, at.1, width, height));
    *out = unsafe { engine(ui) }.table_view(label, headers, &mut [], rows as usize, &mut None, &text, &mut |ui, row, column| cell(row as i64, column as i64, &mut |value| bounded(&mut |t| ui.item_text(t), &bad, &total, value))).row.map(|row| row as i64);
    if bad.get() { return unsafe { fail(ui, "invalid table cell callback text") }; }
    0
}

/// `pxui_slot` as the static wrapper calls it: the caller's content runs inside, and a panic of it unwinds to the app as in every trusted entry.
/// The kit's text of the collection item being built (`Ui::item_text`); refused outside a `list_view`/`tree` row or `table_view` cell.
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

/// `selection_list` keyed by `key`: `cell(ui, row, 0)` builds the children of row `row`, `ui` the borrowed handle.
#[inline]
pub unsafe fn list_view(ui: *mut PxuiUi, key: &str, count: usize, multi: bool, selected: &mut [u8], cell: &mut dyn FnMut(*mut PxuiUi, usize, usize), out: &mut Option<usize>) -> i32 {
    enter!(ui);
    if long_key(key) || count > MAX_ITEMS || selected.len() != count { return unsafe { fail(ui, "invalid list view descriptor") }; }
    let shared = unsafe { (*ui).shared };
    *out = unsafe { engine(ui) }.selection_list(key, count, selected, &SelectionList { multi }, &mut |engine, row| { let mut borrowed = PxuiUi::borrowed(engine, shared); let _current = crate::state::Current::set(shared, &mut borrowed); cell(&mut borrowed, row, 0) }).activated;
    0
}

/// `tree` keyed by `key`: `cell(ui, node, 0)` builds the children of the row that shows tree node `node`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn tree(ui: *mut PxuiUi, key: &str, nodes: &[pxui_types::ffi::PxuiTreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], cell: &mut dyn FnMut(*mut PxuiUi, usize, usize), out: &mut (Option<usize>, usize)) -> i32 {
    enter!(ui);
    if long_key(key) || nodes.len() > MAX_ITEMS || open.len() != nodes.len() || visible.len() < nodes.len() { return unsafe { fail(ui, "invalid tree descriptor") }; }
    let nodes = unsafe { core::slice::from_raw_parts(nodes.as_ptr().cast::<pxui_core::ui_widgets::TreeNode>(), nodes.len()) };
    let shared = unsafe { (*ui).shared };
    let r = unsafe { engine(ui) }.tree(key, nodes, open, selected, visible, &mut |engine, node| { let mut borrowed = PxuiUi::borrowed(engine, shared); let _current = crate::state::Current::set(shared, &mut borrowed); cell(&mut borrowed, node, 0) });
    *out = (r.activated, r.shown);
    0
}

/// `table_view` keyed by `key`, named `name` (empty: none): `cell(ui, row, column)` builds the children of one cell (with
/// `TableView::text`, gives its text to `item_text`); `out` the clicked row (text table) or header (sort intent).
#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn table_view(ui: *mut PxuiUi, key: &str, name: &str, titles: &[&str], widths: &mut [f32], rows: usize, selected: &mut Option<usize>, table: &TableView, cell: &mut dyn FnMut(*mut PxuiUi, usize, usize), out: &mut TableResponse) -> i32 {
    enter!(ui);
    let name = clip(name);
    let mut spare = Vec::new();
    let titles = clip_strs(titles, &mut spare);
    if long_key(key) || rows > MAX_ITEMS || titles.len() > 4096 || !options_ok(titles) { return unsafe { fail(ui, "invalid table descriptor") }; }
    match table.text {
        Some(r) => if titles.len() > 256 || ![r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()) || r.w < 0.0 || r.h < 0.0 { return unsafe { fail(ui, "invalid text table rectangle") }; },
        None => if widths.len() != titles.len() || widths.iter().any(|w| !w.is_finite() || *w < 0.0) || table.sort.is_some_and(|(column, _)| column >= titles.len()) { return unsafe { fail(ui, "invalid table widths or sort column") }; },
    }
    let shared = unsafe { (*ui).shared };
    let engine = unsafe { engine(ui) };
    *out = engine.table_view(key, titles, widths, rows, selected, table, &mut |engine, row, column| { let mut borrowed = PxuiUi::borrowed(engine, shared); let _current = crate::state::Current::set(shared, &mut borrowed); cell(&mut borrowed, row, column) });
    if !name.is_empty() { name_table(engine, key, name); }
    0
}

/// Names the table keyed `key` under the current parent `name` (the 1.x exports named it by their label).
fn name_table(engine: &mut Ui, key: &str, name: &str) {
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
    unsafe { crate::text::pxui_ui_text_spans(ui, box_, value, &style, spans) }
}

#[inline(always)]
pub unsafe fn label_text_line_box(ui: *mut PxuiUi, box_: *const PxuiBox, value: Str, style: &LabelFields, spans: *const TextSpans) -> i32 {
    let Some(style) = (unsafe { resolved(ui, style) }) else { return unsafe { fail(ui, "label style does not resolve") } };
    unsafe { crate::text::pxui_ui_text_line_box(ui, box_, value, &style, spans) }
}

/// `spans` null: the plain measurement (`pxui_text_width`); else `pxui_text_width_spans`.
#[inline(always)]
pub unsafe fn label_measure(ui: *mut PxuiUi, value: Str, style: &LabelFields, max_width: f32, spans: *const TextSpans, out: *mut TextMetrics) -> i32 {
    let Some(style) = (unsafe { resolved(ui, style) }) else { return crate::PXUI_ERR_ARGUMENT };
    if spans.is_null() { unsafe { crate::text::pxui_text_width(ui, 0, value, &style, max_width, out) } }
    else { unsafe { crate::text::pxui_text_width_spans(ui, 0, value, &style, max_width, spans, out) } }
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn label_quads(ui: *mut PxuiUi, value: Str, style: &LabelFields, max_width: f32, x: f32, y: f32, scale: f32, out: *mut TextGlyph, capacity: usize) -> usize {
    let Some(style) = (unsafe { resolved(ui, style) }) else { return usize::MAX };
    unsafe { crate::text::pxui_text_quads(ui, 0, value, &style, max_width, x, y, scale, out, capacity) }
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub unsafe fn label_rasterize_rgba(ui: *mut PxuiUi, value: Str, style: &LabelFields, max_width: f32, scale: f32, color_r: u8, color_g: u8, color_b: u8,
    color_a: u8, width: *mut u32, height: *mut u32, out: *mut u8, capacity: usize) -> usize {
    let Some(style) = (unsafe { resolved(ui, style) }) else { return usize::MAX };
    unsafe { crate::text::pxui_text_rasterize_rgba(ui, 0, value, &style, max_width, scale, color_r, color_g, color_b, color_a, width, height, out, capacity) }
}

#[inline(always)]
pub unsafe fn label_prewarm(ui: *mut PxuiUi, value: Str, style: &LabelFields, max_width: f32) -> i32 {
    let Some(style) = (unsafe { resolved(ui, style) }) else { return crate::PXUI_ERR_ARGUMENT };
    unsafe { crate::text::pxui_text_prewarm(ui, value, &style, max_width) }
}
