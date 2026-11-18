//! The state behind a `PxuiUi` handle and the helpers every export shares: string and box decoding,
//! validation, the poison flag and the per-frame buffers that keep a settled frame allocation-free.
use crate::{PXUI_ERR_ARGUMENT, Str};
use core::ffi::c_void;
use pxui_core::ui::{Ui, UiBox};
use pxui_types::{Event as FfiEvent, PxuiBox, ffi::{A11yNode, Output, Slice}, layout::Track};
use std::{collections::HashMap, rc::Rc};

/// Counts one call into the library on the `Ui` behind `shared` (`pxui_ui_calls`): a plain field of that `Ui`, which one thread uses
/// at a time, so no counter is shared between `Ui`s or threads. Calls without a `Ui` (version, text service, theme objects) are not counted.
#[inline(always)]
pub(crate) unsafe fn note_call(shared: *mut Shared) { if !shared.is_null() { unsafe { (*shared).calls = (*shared).calls.wrapping_add(1); } } }

// Per-role caps on one string from the caller (docs/abi.md, Limits). A longer one is refused with an error, never cut.
/// An identifier: a key, a node or widget label used as identity, a body, theme or font name.
pub(crate) const MAX_KEY_BYTES: usize = 256;
/// Display text: a label, a title, an accessible name or value, a tip, a menu row.
pub(crate) const MAX_LABEL_BYTES: usize = 64 << 10;
/// Editor text (a text field's or area's value, a paste, a text measured or shaped by the text service) and one event's text.
pub(crate) const MAX_TEXT_BYTES: usize = 16 << 20;
/// All the strings of one call's span (options, rows, hints) together, and all the event text of one frame.
pub(crate) const MAX_TOTAL_TEXT_BYTES: usize = 16 << 20;
/// Strings the library hands back (`lend`, `give`) between two frames: at most this many, and `MAX_RETURNED_BYTES` in all; a call
/// past either is refused with `PXUI_ERR_STATE`, so a caller looping outside a frame cannot grow them without bound.
pub(crate) const MAX_RETURNED: usize = 4096;
pub(crate) const MAX_RETURNED_BYTES: usize = 64 << 20;

/// One signal's key events and the joined text some of them carry, kept between frames so a frame that
/// reports keys allocates only while the buffers grow.
#[derive(Default)]
pub(crate) struct KeyChunk { pub events: Vec<FfiEvent>, pub text: String, pub touches:Vec<pxui_types::Touch> }

/// Everything a `Ui` owns besides the engine. Borrowed callback handles point at the same value. `repr(C)`: the fields every call
/// reads (`enter!`: the count, the current handle, the frame flag) come first, each one load from the `Shared` pointer.
#[repr(C)]
pub(crate) struct Shared {
    /// calls into the library on this `Ui` since it was created (`pxui_ui_calls`)
    pub calls: u64,
    /// the one `PxuiUi` handle that may be used right now: the owner outside a frame; in a frame the innermost borrowed handle (the
    /// engine it holds is the live `&mut Ui` of that callback), or null while no callback runs. Any other handle reaches the engine
    /// through a pointer the live `&mut Ui` was not derived from, so every export refuses it (`usable`).
    pub current: *mut PxuiUi,
    pub in_frame: bool,
    pub poisoned: bool,
    /// a paint call was refused (a span left open, a library call under an open span): the frame that ran the callback reports it
    pub paint_error: bool,
    /// the word `pxui_ui_icon_word` gave the next icon button call's drawing; that call takes it (back to 0)
    pub icon_word: u32,
    pub error: String,
    /// a frame ended since the output was last built: its next build refreshes the accessibility records too
    pub frame_ended: bool,
    /// A renderer or caller acknowledged a layer since the output was last read.
    pub draw_acknowledged: bool,
    /// renderers created on this `Ui`: each holds a raw pointer to the engine, so it outlives none of them
    pub renderers: u32,
    /// text handed to C (an edited field, the picker query), valid until the next frame
    pub returned: Vec<String>,
    /// The text area options of the call in progress: refilled in place each call, so its data (errors, notes, spans, splices, chips) reuses its storage.
    pub area: pxui_core::ui_text::TextAreaOpts,
    /// The text area's adapters to the caller's functions, made once and rebound each call.
    pub area_ad: Option<crate::text_inputs::AreaAdapters>,
    /// Copies lent to C, with buffers reused only after the next frame begins.
    pub lent: Vec<String>,
    pub lent_used: usize,
    /// bytes of the strings in `returned` and the first `lent_used` of `lent`
    pub returned_bytes: usize,
    /// grid tracks the engine's nodes borrow, one shared slice per keyed node that has a grid (equal lists share one)
    pub grid_tracks: HashMap<i64, Rc<[Track]>>,
    /// the stored track lists by their hash (`node::tracks_hash`), so a grid node finds an equal list in one lookup, not a scan of them
    /// all; an entry no box holds any more goes at the end of a frame
    pub grid_shared: HashMap<u64, Rc<[Track]>>,
    /// replaced track slices, freed at the end of the frame (the old node may still borrow them until then)
    pub grid_retired: Vec<Rc<[Track]>>,
    /// the output of the last frame, built into these buffers when it is read (`pxui_ui_output`)
    pub output: Output,
    pub nodes: Vec<A11yNode>,
    pub layers: Vec<pxui_types::ffi::PxuiLayerInfo>,
    pub dirty: Vec<pxui_types::ffi::PxuiDirty>,
    pub last_children: Vec<u32>,
    pub chunks: Vec<KeyChunk>,
    pub chunks_used: usize,
    /// the engine's input record of the last frame, kept for its vectors' capacity: a frame's events refill it without allocating
    pub spare_input: Option<Box<input::frame::Frame>>,
    /// the events `pxui_ui_wants_frame` decodes, kept for their capacity (empty between calls)
    pub wants: Vec<input::Event>,
    /// key codes of a box's held keys, valid until the next frame
    pub key_codes: Vec<u32>,
    /// a menu's rows and its submenus' rows borrowed from the caller for one call: empty between calls
    pub menu: Vec<pxui_types::ui::MenuItem<'static>>,
    pub menu_sub: Vec<pxui_types::ui::MenuItem<'static>>,
    /// the input frame `pxui_ui_input` last handed out, and its wire form (the events point into the frame's own strings)
    pub routed: Option<input::frame::Frame>,
    pub routed_events: Vec<FfiEvent>,
    pub routed_text: Vec<String>,
    pub routed_touches:Vec<pxui_types::Touch>,
    pub routed_keys: Vec<u32>,
    pub routed_buttons: Vec<u32>,
    /// a footer's or inspector bar's rows borrowed from the caller for one call: empty between calls
    pub hints: Vec<(&'static str, &'static str)>,
    /// hit callbacks the engine holds or has let go of, kept to reuse (see `box_ops::hit_closure`)
    pub hits: Vec<(Rc<dyn Fn(pxui_types::geom::Rect) -> pxui_types::geom::Rect>, Rc<crate::box_ops::HitSlot>)>,
    /// an inspector head's actions, reused between frames like the hints
    pub actions: Vec<pxui_core::ui::HeadAction>,
    /// the name validator of an inspector head: one closure over a cell the call fills, so a settled head allocates none
    pub rename: Option<(Rc<core::cell::Cell<(crate::text_inputs::TextValid, usize)>>, Rc<dyn Fn(&str) -> bool>)>,
    /// the value field's callback context and the two closures over it, made once: a field call fills the context instead of allocating closures
    pub field: Option<crate::text_inputs::FieldCallbacks>,
    pub strs: Vec<Vec<&'static str>>,
    /// a segmented control's options borrowed from the caller for one call: a pool, as `strs`
    pub segments: Vec<Vec<pxui_types::ui::Segment<'static>>>,
    /// the glyph list `rasterize` composites from, kept for its capacity
    pub raster_glyphs: Vec<pxui_text::Glyph>,
    pub menus: Vec<(&'static str, &'static [&'static str])>,
    /// the bodies the app registered and the slots of the body calls in progress (`bodies.rs`)
    pub bodies: crate::bodies::BodyState,
    /// this `Ui`'s native-image identities (`pxui_ui_image`, `pxui_paint_image`), bounded; no other `Ui` shares them
    pub images: crate::image::Handles,
    /// the 1.x integer projection of the theme in effect that `pxui_ui_theme` lends C (held inline: filled per call, no allocation)
    pub legacy_theme: Option<pxui_types::theme::ThemeSnapshot>,
}

impl Shared {
    /// The string scratch, taken so that a callback running meanwhile takes another rather than clobbers it; give it back with `put_strs`.
    /// A pool, not one buffer: a widget built inside another's callback (a radio in a dialog's content, a dialog in a dialog) takes its own,
    /// and every one comes back, so a settled frame allocates none whatever the nesting.
    pub fn take_strs(&mut self) -> Vec<&'static str> { self.strs.pop().unwrap_or_default() }
    pub fn put_strs(&mut self, mut strs: Vec<&'static str>) { strs.clear(); self.strs.push(strs); }
    /// The segment scratch, a pool as `take_strs`; give it back with `put_segments`.
    pub fn take_segments(&mut self) -> Vec<pxui_types::ui::Segment<'static>> { self.segments.pop().unwrap_or_default() }
    pub fn put_segments(&mut self, mut segments: Vec<pxui_types::ui::Segment<'static>>) { segments.clear(); self.segments.push(segments); }
    pub fn new() -> Self {
        Self {
            error: String::new(), poisoned: false, calls: 0, paint_error: false, icon_word: 0, in_frame: false, current: core::ptr::null_mut(), frame_ended: false, draw_acknowledged: false, renderers: 0, returned: Vec::new(), area: Default::default(), area_ad: None, lent: Vec::new(), lent_used: 0, returned_bytes: 0,
            grid_tracks: HashMap::new(), grid_shared: HashMap::new(), grid_retired: Vec::new(), output: Output::default(), nodes: Vec::new(),
            layers: Vec::new(), dirty: Vec::new(), last_children: Vec::new(), chunks: Vec::new(), chunks_used: 0, spare_input: None, wants: Vec::new(), key_codes: Vec::new(), menu: Vec::new(), menu_sub: Vec::new(), routed: None, routed_events: Vec::new(), routed_text: Vec::new(),routed_touches:Vec::new(), routed_keys: Vec::new(), routed_buttons: Vec::new(),
            hints: Vec::new(), hits: Vec::new(), actions: Vec::new(), rename: None, field: None, strs: Vec::new(), segments: Vec::new(), raster_glyphs: Vec::new(), menus: Vec::new(), bodies: Default::default(), images: crate::image::Handles::new(), legacy_theme: None,
        }
    }
}

/// The opaque `PxuiUi`: the engine plus the shared state. A borrowed handle (`owner == false`) is what a
/// frame callback receives: its engine is the live `&mut Ui` of the frame being built.
#[repr(C)]
pub struct PxuiUi {
    pub(crate) engine: *mut Ui,
    pub(crate) shared: *mut Shared,
    pub(crate) owner: bool,
}

impl PxuiUi {
    pub(crate) fn borrowed(engine: &mut Ui, shared: *mut Shared) -> Self { Self { engine: engine as *mut Ui, shared, owner: false } }
    pub(crate) fn shared<'a>(&self) -> &'a mut Shared { unsafe { &mut *self.shared } }
    /// A copy of `value` valid until the next frame, in a buffer reused after it; refused (`PXUI_ERR_STATE`, recorded) past
    /// `MAX_RETURNED` strings or `MAX_RETURNED_BYTES` since the frame began.
    pub(crate) fn lend(&self, value: &str) -> Result<Str, i32> {
        let shared = self.shared();
        unsafe { room(self, shared, value.len()) }?;
        if shared.lent_used == shared.lent.len() { shared.lent.push(String::new()); }
        let slot = &mut shared.lent[shared.lent_used];
        slot.clear(); slot.push_str(value);
        shared.lent_used += 1;
        Ok(Str { ptr: slot.as_ptr(), len: slot.len() })
    }
    /// Keeps `value` alive until the next frame and returns it as a `Str`; refused as `lend`.
    pub(crate) fn give(&self, value: String) -> Result<Str, i32> {
        let shared = self.shared();
        unsafe { room(self, shared, value.len()) }?;
        shared.returned.push(value);
        let value = shared.returned.last().map_or("", String::as_str);
        Ok(Str { ptr: value.as_ptr(), len: value.len() })
    }
}

/// Counts a string of `len` bytes about to be handed back, or refuses it past the caps (`MAX_RETURNED`, `MAX_RETURNED_BYTES`).
#[inline]
unsafe fn room(ui: &PxuiUi, shared: &mut Shared, len: usize) -> Result<(), i32> {
    let bytes = shared.returned_bytes.saturating_add(len);
    if shared.returned.len() + shared.lent_used >= MAX_RETURNED || bytes > MAX_RETURNED_BYTES {
        return Err(unsafe { fail_code(ui as *const PxuiUi as *mut PxuiUi, "too many strings handed back since the frame began: read them and run a frame", crate::PXUI_ERR_STATE) });
    }
    shared.returned_bytes = bytes;
    Ok(())
}

/// What a trusted entry runs for a container's children: it gets the borrowed handle of the engine being built. The C exports wrap
/// their `Build` and user pointer in one (`|ui| build(user, ui.cast())`); the static wrapper passes its own closure.
pub type Inner<'a> = &'a mut dyn FnMut(*mut PxuiUi);

/// Makes a handle the current one (`Shared::current`) until dropped, then restores the previous: also when a callback unwinds.
pub(crate) struct Current { shared: *mut Shared, prev: *mut PxuiUi }
impl Current {
    #[inline(always)]
    pub fn set(shared: *mut Shared, handle: *mut PxuiUi) -> Self { Self { shared, prev: unsafe { core::mem::replace(&mut (*shared).current, handle) } } }
}
// a poisoned `Ui` stays without a current handle until its next frame (`panicked` cleared it): every call takes `refuse`'s way out
impl Drop for Current { #[inline(always)] fn drop(&mut self) { unsafe { if !(*self.shared).poisoned { (*self.shared).current = self.prev; } } } }

/// Whether `ui` is the handle an export may use now (one load and compare; see `Shared::current`).
#[inline(always)]
pub(crate) unsafe fn usable(ui: *mut PxuiUi) -> bool { unsafe { (*(*ui).shared).current == ui } }

/// What a call on a handle that is not `Shared::current` returns: `PXUI_ERR_PANIC` on a poisoned `Ui` (whose error stays as the panic
/// left it; poison clears `current`, so one compare covers both), else `PXUI_ERR_STATE` with the reason recorded. Out of line, as `fail`.
#[cold]
#[inline(never)]
pub(crate) unsafe fn refuse(shared: *mut Shared) -> i32 {
    if !shared.is_null() {
        if unsafe { (*shared).poisoned } { return crate::PXUI_ERR_PANIC; }
        let error = unsafe { &mut (*shared).error };
        error.clear();
        error.push_str("this handle is not the one a frame callback may use now: use the handle of the innermost callback, and the owner only outside a frame");
    }
    crate::PXUI_ERR_STATE
}

/// What a builder call (`enter!`) on the usable handle outside a frame returns: `PXUI_ERR_STATE`, the reason recorded. A builder
/// declares nodes of the frame being built, so it runs only in a frame callback.
#[cold]
#[inline(never)]
pub(crate) unsafe fn outside_frame(shared: *mut Shared) -> i32 {
    let error = unsafe { &mut (*shared).error };
    error.clear();
    error.push_str("this call builds the frame: make it from a frame callback, with that callback's handle");
    crate::PXUI_ERR_STATE
}

/// Calls `f` with a borrowed handle over `engine`.
pub(crate) fn call_inner(shared: *mut Shared, engine: &mut Ui, f: Inner) {
    let mut borrowed = PxuiUi::borrowed(engine, shared);
    let handle: *mut PxuiUi = &mut borrowed;
    let _current = Current::set(shared, handle);
    f(handle);
}

/// Calls `build` with a borrowed handle over `engine`.
pub(crate) fn call_build(shared: *mut Shared, engine: &mut Ui, build: Build, user: *mut c_void) {
    if let Some(build) = build { call_inner(shared, engine, &mut |ui| build(user, ui.cast())); }
}

pub use pxui_types::ffi::UiBuild as Build;

/// Where an export finds the shared state of its first argument, so the export macro can poison it.
/// `usable` says whether the export may touch the engine through the handle: a `PxuiUi` must be `Shared::current`; a renderer holds the
/// engine for its own calls, so it is refused inside a frame or another renderer's callback; a paint handle reaches the
/// painter of the one paint callback it was made for, and no callback nests in another, so it has no second live path.
pub(crate) trait Handle { fn shared(self) -> *mut Shared; fn usable(self, shared: *mut Shared) -> bool; }
impl Handle for *mut PxuiUi {
    fn shared(self) -> *mut Shared { if self.is_null() { core::ptr::null_mut() } else { unsafe { (*self).shared } } }
    #[inline(always)] fn usable(self, shared: *mut Shared) -> bool { shared.is_null() || unsafe { (*shared).current == self } }
}
impl Handle for *const PxuiUi {
    fn shared(self) -> *mut Shared { Handle::shared(self as *mut PxuiUi) }
    #[inline(always)] fn usable(self, shared: *mut Shared) -> bool { shared.is_null() || unsafe { (*shared).current == self as *mut PxuiUi } }
}
impl Handle for *mut crate::PxuiRenderer {
    fn shared(self) -> *mut Shared { if self.is_null() { core::ptr::null_mut() } else { unsafe { (*self).shared } } }
    #[inline(always)] fn usable(self, shared: *mut Shared) -> bool { shared.is_null() || unsafe { !(*shared).in_frame && !(*shared).current.is_null() } }
}
impl Handle for *mut crate::paint::PxuiPaint {
    fn shared(self) -> *mut Shared { if self.is_null() { core::ptr::null_mut() } else { unsafe { (*self).shared } } }
    #[inline(always)] fn usable(self, _: *mut Shared) -> bool { true }
}

/// Where an `object` export (macros.rs) finds the poison flag of its first argument: null for a null object.
pub(crate) trait Poison { fn poison(self) -> *mut bool; }
impl Poison for *mut crate::text::PxuiShaper { fn poison(self) -> *mut bool { if self.is_null() { core::ptr::null_mut() } else { unsafe { &raw mut (*self).poisoned } } } }
impl Poison for *mut crate::theme::PxuiTheme { fn poison(self) -> *mut bool { if self.is_null() { core::ptr::null_mut() } else { unsafe { &raw mut (*self).poisoned } } } }
impl Poison for *mut crate::a11y::PxuiA11y { fn poison(self) -> *mut bool { if self.is_null() { core::ptr::null_mut() } else { unsafe { &raw mut (*self).poisoned } } } }

/// Records a caught panic: the `Ui` fails every call until its next frame; a panic of the frame itself ends the frame.
pub(crate) unsafe fn panicked(shared: *mut Shared, payload: Box<dyn core::any::Any + Send>, leave_frame: bool) {
    if shared.is_null() { return; }
    let shared = unsafe { &mut *shared };
    shared.poisoned = true;
    shared.current = core::ptr::null_mut();
    if leave_frame { shared.in_frame = false; }
    let message = payload.downcast_ref::<&str>().copied().or_else(|| payload.downcast_ref::<String>().map(String::as_str)).unwrap_or("non-string panic payload");
    shared.error.clear();
    shared.error.push_str("panic: ");
    shared.error.push_str(message);
}

/// The engine behind `ui`.
pub(crate) unsafe fn engine<'a>(ui: *mut PxuiUi) -> Option<&'a mut Ui> { unsafe { get(ui) }.map(|state| unsafe { &mut *state.engine }) }

pub(crate) unsafe fn get<'a>(ui: *mut PxuiUi) -> Option<&'a mut PxuiUi> {
    if ui.is_null() { None } else { Some(unsafe { &mut *ui }) }
}

/// Records `message` as the last error and returns `PXUI_ERR_ARGUMENT`. Out of line, as `fail_code`: an export's checks stay a branch each.
#[cold]
#[inline(never)]
pub(crate) unsafe fn fail(ui: *mut PxuiUi, message: &str) -> i32 { unsafe { fail_code(ui, message, PXUI_ERR_ARGUMENT) } }

/// Records `message` as the last error and returns `code`.
#[cold]
#[inline(never)]
pub(crate) unsafe fn fail_code(ui: *mut PxuiUi, message: &str, code: i32) -> i32 {
    // a poisoned `Ui` keeps the panic's message
    if let Some(ui) = unsafe { get(ui) }.filter(|ui| !ui.shared().poisoned) {
        let error = &mut ui.shared().error;
        error.clear();
        error.push_str(message);
    }
    code
}

/// UTF-8 text from ptr+len. NUL is ordinary text.
#[inline(always)]
pub(crate) unsafe fn text(value: Str) -> Option<&'static str> {
    if value.len == 0 { return Some(""); }
    if value.ptr.is_null() { return None; }
    let bytes: &'static [u8] = unsafe { std::slice::from_raw_parts(value.ptr, value.len) };
    // most labels are ASCII: checked inline, the answer in a register; anything else takes the full check
    if ascii(bytes) { Some(unsafe { std::str::from_utf8_unchecked(bytes) }) } else { std::str::from_utf8(bytes).ok() }
}

/// Whether `bytes` are ASCII. Up to 16 of them (a label) are two loads that may overlap, up to 64 (a tip, a sentence) four or eight,
/// not a loop; longer ones `is_ascii`.
#[inline(always)]
pub(crate) fn ascii(bytes: &[u8]) -> bool {
    let (n, p) = (bytes.len(), bytes.as_ptr());
    let word = |at: usize| unsafe { p.add(at).cast::<u64>().read_unaligned() };
    let half = |at: usize| unsafe { p.add(at).cast::<u32>().read_unaligned() };
    // the first and the last `k` words cover all `n` bytes when `n <= 16k`
    let ends = |k: usize| (0..k).fold(0, |all, i| all | word(8 * i) | word(n - 8 * (i + 1)));
    if n >= 8 {
        if n <= 16 { (word(0) | word(n - 8)) & 0x8080_8080_8080_8080 == 0 }
        else if n <= 32 { ends(2) & 0x8080_8080_8080_8080 == 0 }
        else if n <= 64 { ends(4) & 0x8080_8080_8080_8080 == 0 }
        else { bytes.is_ascii() }
    }
    else if n >= 4 { (half(0) | half(n - 4)) & 0x8080_8080 == 0 }
    // one to three bytes: the first, the middle and the last are all of them
    else { n == 0 || unsafe { *p | *p.add(n / 2) | *p.add(n - 1) } & 0x80 == 0 }
}

/// `value` as the pair C hands over. An export passes its checked text on this way, not the caller's pair again, so nothing is
/// kept in a register across the UTF-8 check.
#[inline(always)]
pub(crate) fn pair(value: &str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }

/// `text` limited to `MAX_LABEL_BYTES`: display text, the default role. The limit is compared past 16 bytes only, where `ascii`
/// branches anyway: a label pays nothing for it.
#[inline(always)]
pub(crate) unsafe fn checked(value: Str) -> Option<&'static str> {
    if value.len <= 16 { unsafe { text(value) } } else if value.len > MAX_LABEL_BYTES { None } else { unsafe { text(value) } }
}

/// `text` cut to `MAX_LABEL_BYTES` at a character boundary: display text from the Rust wrapper, which clips rather than panics on user
/// data. The C exports never clip (they refuse, `checked`); a trusted entry clips what only the static wrapper can pass.
pub(crate) fn clip(text: &str) -> &str {
    if text.len() <= MAX_LABEL_BYTES { return text; }
    let mut end = MAX_LABEL_BYTES;
    while !text.is_char_boundary(end) { end -= 1; }
    &text[..end]
}

/// `text` limited to `MAX_KEY_BYTES`: an identifier. A label that begins with a 0x01 byte must be a usable resolved key (`Key::Resolved`):
/// one not of a 0x01 byte and 16 lowercase hex digits, or one carrying the reserved 0 or 1, is refused.
#[inline(always)]
pub(crate) unsafe fn checked_key(value: Str) -> Option<&'static str> {
    let key = if value.len <= 16 { unsafe { text(value) } } else if value.len > MAX_KEY_BYTES { None } else { unsafe { text(value) } }?;
    if unusable(key) { None } else { Some(key) }
}

/// `checked` (display-text cap) of a label that is also an identity, a legacy label-keyed call's: refused as `checked_key` refuses a
/// 0x01-led label that is not a usable resolved key.
#[inline(always)]
pub(crate) unsafe fn checked_label(value: Str) -> Option<&'static str> {
    unsafe { checked(value) }.filter(|label| !unusable(label))
}

/// Whether `key` begins with the 0x01 byte of a resolved key but is not a usable one (see `usable_resolved`).
#[inline(always)]
pub(crate) fn unusable(key: &str) -> bool { key.as_bytes().first() == Some(&1) && !usable_resolved(key) }

/// Whether a 0x01-led label is a resolved key the engine may take as it is: 16 lowercase hex digits after the byte, carrying neither 0
/// nor 1 (the key tables' empty and tombstone marks).
#[cold]
#[inline(never)]
fn usable_resolved(key: &str) -> bool {
    let digits = &key.as_bytes()[1..];
    digits.len() == 16 && digits.iter().all(|d| matches!(d, b'0'..=b'9' | b'a'..=b'f')) && (digits[..15].iter().any(|d| *d != b'0') || digits[15] > b'1')
}

/// `text` limited to `MAX_TEXT_BYTES`: editor text.
#[inline(always)]
pub(crate) unsafe fn checked_text(value: Str) -> Option<&'static str> {
    if value.len > MAX_TEXT_BYTES { None } else { unsafe { text(value) } }
}

/// Decodes a span of strings into `out` (cleared first): at most `max` of them, each a label (`checked`), `MAX_TOTAL_TEXT_BYTES` in all.
pub(crate) unsafe fn strings(values: Slice<Str>, max: usize, out: &mut Vec<&'static str>) -> bool {
    out.clear();
    unsafe { append_strings(values, max, out) }
}

/// `strings` that keeps what `out` holds.
pub(crate) unsafe fn append_strings(values: Slice<Str>, max: usize, out: &mut Vec<&'static str>) -> bool {
    if values.len > max || (values.len != 0 && values.ptr.is_null()) { return false; }
    let values = if values.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(values.ptr, values.len) } };
    let mut total = 0usize;
    for value in values {
        total = match total.checked_add(value.len) { Some(total) if total <= MAX_TOTAL_TEXT_BYTES => total, _ => return false };
        let Some(value) = (unsafe { checked(*value) }) else { return false };
        out.push(value);
    }
    true
}

pub(crate) fn finite(values: &[f32]) -> bool { values.iter().all(|v| v.is_finite()) }

pub(crate) fn to_abi_box(b: UiBox) -> PxuiBox { PxuiBox { index: b.index as u64, box_key: b.box_key, box_slot: b.box_slot as u64, generation: b.generation } }
pub(crate) fn core_box(raw: PxuiBox) -> Option<UiBox> {
    Some(UiBox { index: usize::try_from(raw.index).ok()?, box_key: raw.box_key, box_slot: usize::try_from(raw.box_slot).ok()?, generation: raw.generation })
}
/// The engine and the live box a call names, or the error code after recording why.
pub(crate) unsafe fn boxed<'a>(ui: *mut PxuiUi, raw: *const PxuiBox) -> Result<(&'a mut Ui, UiBox), i32> {
    let Some(engine) = (unsafe { engine(ui) }) else { return Err(PXUI_ERR_ARGUMENT) };
    if raw.is_null() { return Err(unsafe { fail(ui, "null box") }); }
    match live_box(engine, unsafe { *raw }) { Some(b) => Ok((engine, b)), None => Err(unsafe { fail_code(ui, "stale box", crate::PXUI_ERR_STALE) }) }
}

/// A box naming a slot the engine has. A node that has gone since the handle was made is not refused here: the engine's own calls
/// tolerate it as it does (a patch does nothing, a query answers a default, a scroll or state call reads the slot), and the few that
/// need a live node say so themselves.
pub(crate) fn live_box(engine: &Ui, raw: PxuiBox) -> Option<UiBox> {
    let b = core_box(raw)?;
    engine.has_slot(&b).then_some(b)
}

#[cfg(test)]
mod tests {
    /// `ascii` never calls a text with a byte past 0x7f ASCII (an export would then take it unchecked as UTF-8), at every length its
    /// overlapping loads cover and every position of that byte, and accepts all-ASCII text.
    #[test]
    fn ascii_sees_every_byte_of_every_length() {
        let mut bytes = [b'a'; 80];
        for n in 0..=80 {
            assert!(super::ascii(&bytes[..n]), "{n} ASCII bytes");
            for at in 0..n {
                bytes[at] = 0xc3;
                assert!(!super::ascii(&bytes[..n]), "byte {at} of {n}");
                bytes[at] = b'a';
            }
        }
    }
}
