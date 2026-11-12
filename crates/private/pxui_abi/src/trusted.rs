//! The trusted entries: the bodies of the hot exports, minus what only an untrusted caller needs. The static wrapper's caller is Rust, so
//! its strings are UTF-8, its enums in range, its references non-null and a panic unwinds through these frames as it did through the engine
//! itself. A `pxui_*` export of the same name checks what C hands over (poison, null, UTF-8, tags) and then calls its entry here, so the
//! two paths run one body. What Rust types cannot promise stays checked in the body (a text longer than `MAX_TEXT_BYTES`, a stale box,
//! a number that is not finite). Each entry counts itself as one call into the library.
use crate::node::declare;
use crate::state::{Inner, MAX_TEXT_BYTES, MAX_TOTAL_TEXT_BYTES, PxuiUi, boxed, call_inner, fail, note_call, to_abi_box};
use crate::{PxuiBox, Str};
use pxui_core::ui::Ui;
use pxui_types::{FrameIn, FrameOut, Node, Size, ffi::Input, theme::ThemeSnapshot};


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

/// `pxui_ui_node` of a label already checked (UTF-8, at most `MAX_TEXT_BYTES`): the export's entry, which checks it as C hands it over.
#[inline(always)]
pub(crate) unsafe fn node(ui: *mut PxuiUi, label: &str, desc: &Node, out: *mut PxuiBox) -> i32 {
    note_call();
    declare(unsafe { engine(ui) }, Some(label), 0, desc, unsafe { &mut *out });
    0
}

#[inline]
pub unsafe fn pxui_ui_node_keyed(ui: *mut PxuiUi, key: i64, desc: &Node, out: *mut PxuiBox) -> i32 {
    note_call();
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
    note_call();
    let raw = unsafe { *box_ };
    unsafe { engine(ui) }.text(&pxui_core::ui::UiBox { index: raw.index as usize, box_key: raw.box_key, box_slot: raw.box_slot as usize, generation: raw.generation }, value);
    0
}

#[inline]
pub unsafe fn pxui_label(ui: *mut PxuiUi, label: Str) -> i32 {
    note_call();
    let Some(label) = (unsafe { utf8(label) }) else { return unsafe { fail(ui, "invalid label text") } };
    unsafe { engine(ui) }.label(label);
    0
}

#[inline]
pub unsafe fn pxui_button(ui: *mut PxuiUi, label: Str, out: *mut u8) -> i32 {
    note_call();
    let Some(label) = (unsafe { utf8(label) }) else { return unsafe { fail(ui, "invalid button arguments") } };
    unsafe { *out = engine(ui).button(label) as u8; }
    0
}

#[inline]
pub unsafe fn pxui_toggle(ui: *mut PxuiUi, label: Str, on: u8, out: *mut u8) -> i32 {
    note_call();
    let Some(label) = (unsafe { utf8(label) }) else { return unsafe { fail(ui, "invalid toggle arguments") } };
    unsafe { *out = engine(ui).toggle(label, on != 0) as u8; }
    0
}

/// A text field: `out_changed` says whether the user changed the text, `changed` then holds the new text (borrowed until the next frame).
#[inline]
pub unsafe fn pxui_text_field(ui: *mut PxuiUi, label: Str, value: Str, changed: *mut Str, out_changed: *mut u8) -> i32 {
    note_call();
    let (Some(label), Some(value)) = (unsafe { utf8(label) }, unsafe { utf8(value) }) else { return unsafe { fail(ui, "invalid text field arguments") } };
    unsafe { *out_changed = 0; *changed = Str::default(); }
    if let Some(value) = unsafe { engine(ui) }.text_field(label, value) { unsafe { *changed = (*ui).give(value); *out_changed = 1; } }
    0
}

/// The theme in effect, lent.
#[inline]
pub unsafe fn pxui_ui_theme(ui: *mut PxuiUi) -> *const ThemeSnapshot {
    note_call();
    (unsafe { engine(ui) }.theme() as *const pxui_core::Theme).cast()
}

// ---- containers: their children run through `Inner`

#[inline]
pub unsafe fn within(ui: *mut PxuiUi, box_: *const PxuiBox, f: Inner) -> i32 {
    note_call();
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    let shared = unsafe { (*ui).shared };
    e.within(&b, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

#[inline]
pub unsafe fn scope(ui: *mut PxuiUi, label: &str, f: Inner) -> i32 {
    note_call();
    if label.len() > MAX_TEXT_BYTES { return unsafe { fail(ui, "invalid scope label") }; }
    let shared = unsafe { (*ui).shared };
    unsafe { engine(ui) }.scope(label, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

#[inline]
pub unsafe fn col(ui: *mut PxuiUi, padding: f32, label: &str, f: Inner) -> i32 {
    note_call();
    if label.len() > MAX_TEXT_BYTES || !padding.is_finite() { return unsafe { fail(ui, "invalid column arguments") }; }
    let shared = unsafe { (*ui).shared };
    unsafe { engine(ui) }.col(padding, label, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

#[inline]
pub unsafe fn view(ui: *mut PxuiUi, label: &str, revision: u64, width: Size, height: Size, flat: bool, f: Inner, out: *mut PxuiBox) -> i32 {
    note_call();
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

const MAX_OPTIONS: usize = 4096;

#[inline]
fn long(value: &str) -> bool { value.len() > MAX_TEXT_BYTES }

/// Whether `options` are within the engine's limits: at most `MAX_OPTIONS` of them, at most `MAX_TOTAL_TEXT_BYTES` in all.
#[inline]
fn options_ok(options: &[&str]) -> bool {
    options.len() <= MAX_OPTIONS && options.iter().fold(0usize, |total, o| total.saturating_add(o.len())) <= MAX_TOTAL_TEXT_BYTES && options.iter().all(|o| !long(o))
}

#[inline]
pub unsafe fn checkbox(ui: *mut PxuiUi, label: &str, value: bool, out: &mut bool) -> i32 {
    note_call();
    if long(label) { return unsafe { fail(ui, "invalid checkbox label") }; }
    *out = unsafe { engine(ui) }.checkbox(label, value);
    0
}

/// `kind`: 0 radio, 1 tabs, 2 segmented, 3 dropdown.
#[inline]
pub unsafe fn choice(ui: *mut PxuiUi, kind: u32, label: &str, options: &[&str], selected: usize, out: &mut usize) -> i32 {
    note_call();
    if long(label) || !options_ok(options) { return unsafe { fail(ui, "invalid widget strings") }; }
    let engine = unsafe { engine(ui) };
    *out = match kind { 0 => engine.radio(label, options, selected), 1 => engine.tabs(label, options, selected), 2 => engine.segmented(label, options, selected), _ => engine.dropdown(label, options, selected) };
    0
}

#[inline]
pub unsafe fn slider(ui: *mut PxuiUi, label: &str, value: f32, min: f32, max: f32, out: &mut f32) -> i32 {
    note_call();
    if !value.is_finite() || !min.is_finite() || !max.is_finite() || min > max || long(label) { return unsafe { fail(ui, "invalid slider arguments") }; }
    *out = unsafe { engine(ui) }.slider(label, value, min..=max);
    0
}

#[inline]
pub unsafe fn progress(ui: *mut PxuiUi, label: &str, value: Option<f32>, out: &mut PxuiBox) -> i32 {
    note_call();
    if long(label) || value.is_some_and(|v| !v.is_finite()) { return unsafe { fail(ui, "invalid progress arguments") }; }
    *out = to_abi_box(unsafe { engine(ui) }.progress(label, value));
    0
}

#[inline]
pub unsafe fn spinner(ui: *mut PxuiUi, label: &str, out: &mut PxuiBox) -> i32 {
    note_call();
    if long(label) { return unsafe { fail(ui, "invalid spinner label") }; }
    *out = to_abi_box(unsafe { engine(ui) }.spinner(label));
    0
}

#[inline]
pub unsafe fn icon_button(ui: *mut PxuiUi, label: &str, icon: &str, out: &mut bool) -> i32 {
    note_call();
    if long(label) || long(icon) { return unsafe { fail(ui, "invalid icon button text") }; }
    *out = unsafe { engine(ui) }.icon_button(label, icon);
    0
}

#[inline]
pub unsafe fn badge(ui: *mut PxuiUi, label: &str, value: &str, out: &mut PxuiBox) -> i32 {
    note_call();
    if long(label) || long(value) { return unsafe { fail(ui, "invalid badge text") }; }
    *out = to_abi_box(unsafe { engine(ui) }.badge(label, value));
    0
}

/// The text a search field changed to, or none.
#[inline]
pub unsafe fn search_field(ui: *mut PxuiUi, label: &str, icon: &str, value: &str, placeholder: &str, out: &mut Option<String>) -> i32 {
    note_call();
    if long(label) || long(icon) || long(value) || long(placeholder) { return unsafe { fail(ui, "invalid search field text") }; }
    *out = unsafe { engine(ui) }.search_field(label, icon, value, placeholder);
    0
}

#[inline]
pub unsafe fn tip(ui: *mut PxuiUi, box_: &PxuiBox, label: &str) -> i32 {
    note_call();
    if long(label) { return unsafe { fail(ui, "invalid tooltip arguments") }; }
    unsafe { engine(ui) }.tip(&unbox(box_), label);
    0
}

#[inline]
fn unbox(raw: &PxuiBox) -> pxui_core::ui::UiBox { pxui_core::ui::UiBox { index: raw.index as usize, box_key: raw.box_key, box_slot: raw.box_slot as usize, generation: raw.generation } }

#[inline]
pub unsafe fn rect(ui: *mut PxuiUi, box_: &PxuiBox, out: &mut pxui_types::geom::Rect) -> i32 {
    note_call();
    *out = unsafe { engine(ui) }.rect(&unbox(box_));
    0
}

#[inline]
pub unsafe fn box_of_key(ui: *mut PxuiUi, key: i64, out: &mut PxuiBox) -> bool {
    note_call();
    let Some(found) = (unsafe { engine(ui) }).box_of_key(key) else { return false };
    *out = to_abi_box(found);
    true
}

// ---- the inspector rows and the footer

#[inline]
pub unsafe fn inspector_toggle(ui: *mut PxuiUi, key: &str, label: &str, value: bool, out: &mut bool) -> i32 {
    note_call();
    if long(key) || long(label) { return unsafe { fail(ui, "invalid inspector toggle text") }; }
    *out = unsafe { engine(ui) }.inspector_toggle(key, label, value);
    0
}

#[inline]
pub unsafe fn inspector_button(ui: *mut PxuiUi, key: &str, label: &str, out: &mut bool) -> i32 {
    note_call();
    if long(key) || long(label) { return unsafe { fail(ui, "invalid inspector button text") }; }
    *out = unsafe { engine(ui) }.inspector_button(key, label);
    0
}

#[inline]
pub unsafe fn inspector_readout(ui: *mut PxuiUi, width: Option<f32>, key: &str, label: &str, value: &str) -> i32 {
    note_call();
    if long(key) || long(label) || long(value) || width.is_some_and(|w| !w.is_finite() || w < 0.0) { return unsafe { fail(ui, "invalid inspector readout text") }; }
    unsafe { engine(ui) }.inspector_readout(width, key, label, value);
    0
}

#[inline]
pub unsafe fn inspector_message(ui: *mut PxuiUi, key: &str, message: &str) -> i32 {
    note_call();
    if long(key) || long(message) { return unsafe { fail(ui, "invalid inspector message text") }; }
    unsafe { engine(ui) }.inspector_message(key, message);
    0
}

/// The row's box and its control's x, y and width.
#[inline]
pub unsafe fn inspector_row(ui: *mut PxuiUi, width: Option<f32>, pin: Option<bool>, key: &str, label: &str, out: &mut (PxuiBox, f32, f32, f32)) -> i32 {
    note_call();
    if long(key) || long(label) || width.is_some_and(|w| !w.is_finite() || w < 0.0) { return unsafe { fail(ui, "invalid inspector row text") }; }
    let (row, x, y, w) = unsafe { engine(ui) }.inspector_row(width, pin, key, label);
    *out = (to_abi_box(row), x, y, w);
    0
}

#[inline]
pub unsafe fn inspector_section(ui: *mut PxuiUi, key: &str, expanded: bool, set_expanded: Option<bool>, label: &str, f: Inner, out: &mut bool) -> i32 {
    note_call();
    if long(key) || long(label) { return unsafe { fail(ui, "invalid inspector section text") }; }
    let shared = unsafe { (*ui).shared };
    *out = unsafe { engine(ui) }.inspector_section(key, expanded, set_expanded, label, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

/// The footer: the engine keeps the hint rows it is given, so the caller's own rows go straight in.
#[inline]
pub unsafe fn footer(ui: *mut PxuiUi, right: Option<&str>, hints: &[(String, String)]) -> i32 {
    note_call();
    let total = right.map_or(0, str::len) + hints.iter().fold(0usize, |total, (k, v)| total.saturating_add(k.len()).saturating_add(v.len()));
    if hints.len() > 4096 || total > MAX_TOTAL_TEXT_BYTES || right.is_some_and(long) { return unsafe { fail(ui, "invalid footer text") }; }
    unsafe { engine(ui) }.footer(right, hints);
    0
}

// ---- containers with options

#[inline]
pub unsafe fn panel(ui: *mut PxuiUi, opts: pxui_types::ui::PanelOpts, label: &str, f: Inner) -> i32 {
    note_call();
    if long(label) || !crate::state::finite(&[opts.x, opts.y, opts.width, opts.height.unwrap_or_default(), opts.max_height.unwrap_or_default()]) { return unsafe { fail(ui, "invalid panel descriptor") }; }
    let shared = unsafe { (*ui).shared };
    unsafe { engine(ui) }.panel(opts, label, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

#[inline]
pub unsafe fn scroll_view(ui: *mut PxuiUi, label: &str, width: Size, height: Size, f: Inner, out: &mut PxuiBox) -> i32 {
    note_call();
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

#[inline]
pub unsafe fn list_view(ui: *mut PxuiUi, label: &str, count: usize, multi: bool, selected: &mut [u8], text: &mut dyn FnMut(usize, &mut dyn FnMut(&str)), out: &mut Option<usize>) -> i32 {
    note_call();
    if long(label) || count > MAX_ITEMS || selected.len() != count { return unsafe { fail(ui, "invalid list view descriptor") }; }
    let (bad, total, text) = (core::cell::Cell::new(false), core::cell::Cell::new(0), core::cell::RefCell::new(text));
    *out = unsafe { engine(ui) }.list_view(label, count, multi, selected, &|row, put| (text.borrow_mut())(row, &mut |value| bounded(put, &bad, &total, value)));
    if bad.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    0
}

/// A preorder tree; `visible` holds a row per node, `open` and `nodes` one flag and one node each.
#[inline]
pub unsafe fn tree(ui: *mut PxuiUi, label: &str, nodes: &[pxui_types::ffi::PxuiTreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], text: &mut dyn FnMut(usize, &mut dyn FnMut(&str)), out: &mut (Option<usize>, usize)) -> i32 {
    note_call();
    if long(label) || nodes.len() > MAX_ITEMS || open.len() != nodes.len() || visible.len() < nodes.len() { return unsafe { fail(ui, "invalid tree descriptor") }; }
    // the engine's node and the C node share a layout (asserted where the exports are), so the caller's array is the engine's slice
    let nodes = unsafe { core::slice::from_raw_parts(nodes.as_ptr().cast::<pxui_core::ui_widgets::TreeNode>(), nodes.len()) };
    let (bad, total, text) = (core::cell::Cell::new(false), core::cell::Cell::new(0), core::cell::RefCell::new(text));
    *out = unsafe { engine(ui) }.tree(label, nodes, open, selected, visible, &|row, put| (text.borrow_mut())(row, &mut |value| bounded(put, &bad, &total, value)));
    if bad.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    0
}

#[inline]
pub unsafe fn table_view(ui: *mut PxuiUi, label: &str, titles: &[&str], widths: &mut [f32], rows: usize, sort: Option<(usize, bool)>, selected: &mut Option<usize>, cell: &mut dyn FnMut(usize, usize, &mut dyn FnMut(&str)), out: &mut Option<usize>) -> i32 {
    note_call();
    if long(label) || rows > MAX_ITEMS || titles.len() > 4096 || widths.len() != titles.len() || !options_ok(titles) { return unsafe { fail(ui, "invalid table descriptor") }; }
    if widths.iter().any(|w| !w.is_finite() || *w < 0.0) || sort.is_some_and(|(column, _)| column >= titles.len()) { return unsafe { fail(ui, "invalid table widths or sort column") }; }
    let (bad, total, cell) = (core::cell::Cell::new(false), core::cell::Cell::new(0), core::cell::RefCell::new(cell));
    *out = unsafe { engine(ui) }.table_view(label, titles, widths, rows, sort, selected, &|row, column, put| (cell.borrow_mut())(row, column, &mut |value| bounded(put, &bad, &total, value)));
    if bad.get() { return unsafe { fail(ui, "invalid collection callback text") }; }
    0
}

// ---- more containers and callbacks

/// A grid of `count` cells, built on demand: `f` gets the borrowed handle and the cell's index.
#[inline]
pub unsafe fn grid_list(ui: *mut PxuiUi, label: &str, count: usize, cell_w: f32, cell_h: f32, f: &mut dyn FnMut(*mut PxuiUi, usize), out: &mut PxuiBox) -> i32 {
    let shared = unsafe { (*ui).shared };
    unsafe { grid_cells(ui, label, count, cell_w, cell_h, &mut |engine: &mut Ui, index| { let mut borrowed = PxuiUi::borrowed(engine, shared); f(&mut borrowed, index) }, out) }
}

/// `grid_list` with the engine's own cell closure: the export's makes the borrowed handle and calls C's callback itself, one
/// indirect call a cell fewer than through `f`.
#[inline(always)]
pub(crate) unsafe fn grid_cells(ui: *mut PxuiUi, label: &str, count: usize, cell_w: f32, cell_h: f32, cell: &mut dyn FnMut(&mut Ui, usize), out: &mut PxuiBox) -> i32 {
    note_call();
    if long(label) || count > MAX_ITEMS || !cell_w.is_finite() || !cell_h.is_finite() || cell_w <= 0.0 || cell_h <= 0.0 { return unsafe { fail(ui, "invalid grid list descriptor") }; }
    *out = to_abi_box(unsafe { engine(ui) }.grid_list(label, count, cell_w, cell_h, cell));
    0
}

/// Two panes in a row (`axis` 0) or a column (1); `changed` is the new ratio when the user moved the divider.
#[inline]
pub unsafe fn split(ui: *mut PxuiUi, label: &str, axis: pxui_types::Axis, ratio: f32, min_a: f32, min_b: f32, a: Inner, b: Inner, out: &mut Option<f32>) -> i32 {
    note_call();
    if long(label) || ![ratio, min_a, min_b].iter().all(|v| v.is_finite()) || min_a < 0.0 || min_b < 0.0 || axis == pxui_types::Axis::Stack { return unsafe { fail(ui, "invalid split descriptor") }; }
    let shared = unsafe { (*ui).shared };
    *out = unsafe { engine(ui) }.split(label, axis, ratio, min_a, min_b, &mut |engine: &mut Ui| call_inner(shared, engine, a), &mut |engine: &mut Ui| call_inner(shared, engine, b));
    0
}

#[inline]
pub unsafe fn anchored(ui: *mut PxuiUi, label: &str, anchor: pxui_types::geom::Rect, placement: pxui_types::layout::Placement, width: Size, height: Size, f: Inner, out: &mut PxuiBox) -> i32 {
    note_call();
    if long(label) || !finite(width) || !finite(height) || anchor.w < 0.0 || anchor.h < 0.0 || placement.margin < 0.0
        || ![anchor.x, anchor.y, anchor.w, anchor.h, placement.gap, placement.margin].iter().all(|v| v.is_finite()) { return unsafe { fail(ui, "invalid anchored arguments") }; }
    let shared = unsafe { (*ui).shared };
    *out = to_abi_box(unsafe { engine(ui) }.anchored(label, anchor, placement, width, height, &mut |engine: &mut Ui| call_inner(shared, engine, f)));
    0
}

#[inline]
pub unsafe fn modal(ui: *mut PxuiUi, label: &str, width: Option<f32>, f: Inner, out: &mut bool) -> i32 {
    note_call();
    if long(label) || width.is_some_and(|w| !w.is_finite() || w < 0.0) { return unsafe { fail(ui, "invalid modal arguments") }; }
    let shared = unsafe { (*ui).shared };
    *out = unsafe { engine(ui) }.modal(width, label, &mut |engine: &mut Ui| call_inner(shared, engine, f));
    0
}

/// A table of text cells at `at`; the clicked row, if any. `cell` writes a cell's text through `put`.
#[inline]
pub unsafe fn table(ui: *mut PxuiUi, at: (f32, f32), width: f32, height: f32, headers: &[&str], rows: i64, cell: &mut dyn FnMut(i64, i64, &mut dyn FnMut(&str)), label: &str, out: &mut Option<i64>) -> i32 {
    note_call();
    if long(label) || headers.len() > 256 || rows < 0 || rows as usize > MAX_ITEMS || !width.is_finite() || !height.is_finite() || width < 0.0 || height < 0.0 || !at.0.is_finite() || !at.1.is_finite() || !options_ok(headers) {
        return unsafe { fail(ui, "invalid table descriptor") };
    }
    let (bad, total) = (core::cell::Cell::new(false), core::cell::Cell::new(0));
    *out = unsafe { engine(ui) }.table_with(at, width, height, headers, rows, &mut |row, column, put| cell(row, column, &mut |value| bounded(put, &bad, &total, value)), label).0;
    if bad.get() { return unsafe { fail(ui, "invalid table cell callback text") }; }
    0
}
