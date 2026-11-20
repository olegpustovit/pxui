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
    /// `current` while a frame builds, else null: the handle a builder call (`enter!`) may use, so its checks are one compare
    pub building: *mut PxuiUi,
    pub in_frame: bool,
    /// the surface registered on this `Ui` (0: none). shortcut: one surface per Ui, upgrade when the engine keeps per-surface
    /// focus/a11y/IME state (then a small fixed table of ids)
    pub surface: u64,
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
    /// an inspector head's actions, reused between frames like the hints
    pub actions: Vec<pxui_core::ui::HeadAction>,
    /// the name validator of an inspector head: one closure over a cell the call fills, so a settled head allocates none
    pub rename: Option<(Rc<core::cell::Cell<(crate::text_inputs::TextValid, usize)>>, Rc<dyn Fn(&str) -> bool>)>,
    pub strs: Vec<Vec<&'static str>>,
    /// a segmented control's options borrowed from the caller for one call: a pool, as `strs`
    /// the glyph list `rasterize` composites from, kept for its capacity
    pub raster_glyphs: Vec<pxui_text::Glyph>,
    pub menus: Vec<(&'static str, &'static [&'static str])>,
    /// the bodies the app registered and the slots of the body calls in progress (`bodies.rs`)
    pub bodies: crate::bodies::BodyState,
    /// this `Ui`'s native-image identities (`pxui_ui_image`, `pxui_paint_image`), bounded; no other `Ui` shares them
    pub images: crate::image::Handles,
}

impl Shared {
    /// The string scratch, taken so that a callback running meanwhile takes another rather than clobbers it; give it back with `put_strs`.
    /// A pool, not one buffer: a widget built inside another's callback (a radio in a dialog's content, a dialog in a dialog) takes its own,
    /// and every one comes back, so a settled frame allocates none whatever the nesting.
    pub fn take_strs(&mut self) -> Vec<&'static str> { self.strs.pop().unwrap_or_default() }
    pub fn put_strs(&mut self, mut strs: Vec<&'static str>) { strs.clear(); self.strs.push(strs); }
    pub fn new() -> Self {
        Self {
            error: String::new(), poisoned: false, calls: 0, paint_error: false, icon_word: 0, in_frame: false, surface: 0, current: core::ptr::null_mut(), building: core::ptr::null_mut(), frame_ended: false, draw_acknowledged: false, renderers: 0, returned: Vec::new(), area: Default::default(), area_ad: None, lent: Vec::new(), lent_used: 0, returned_bytes: 0,
            grid_tracks: HashMap::new(), grid_shared: HashMap::new(), grid_retired: Vec::new(), output: Output::default(), nodes: Vec::new(),
            layers: Vec::new(), dirty: Vec::new(), last_children: Vec::new(), chunks: Vec::new(), chunks_used: 0, spare_input: None, wants: Vec::new(), key_codes: Vec::new(), menu: Vec::new(), menu_sub: Vec::new(), routed: None, routed_events: Vec::new(), routed_text: Vec::new(),routed_touches:Vec::new(), routed_keys: Vec::new(), routed_buttons: Vec::new(),
            hints: Vec::new(), actions: Vec::new(), rename: None, strs: Vec::new(), raster_glyphs: Vec::new(), menus: Vec::new(), bodies: Default::default(), images: crate::image::Handles::new(),
        }
    }
}

/// The `PxuiUi`: the engine plus the shared state. A borrowed handle (`owner == false`) is what a
/// frame callback receives: its engine is the live `&mut Ui` of the frame being built. Its first field is public (`include/pxui.h`):
/// the theme in effect for the handle, null until a theme read (`pxui_ui_theme`) fills it, so a caller reads the theme by one
/// load. The theme a handle sees cannot change while it is usable (a swap is refused in a frame, a `themed` scope builds through
/// a handle of its own) and the engine keeps each scope's theme in place, so the pointer stays valid for the handle's life.
#[repr(C)]
pub struct PxuiUi {
    pub theme: *const pxui_types::theme::ThemeSnapshot,
    pub(crate) engine: *mut Ui,
    pub(crate) shared: *mut Shared,
    pub(crate) owner: bool,
}

// the public head `include/pxui.h` declares: the theme pointer first
const _: () = assert!(core::mem::offset_of!(PxuiUi, theme) == 0);

impl PxuiUi {
    pub(crate) fn borrowed(engine: &mut Ui, shared: *mut Shared) -> Self { Self { theme: core::ptr::null(), engine: engine as *mut Ui, shared, owner: false } }
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

/// What a trusted entry runs for a container's children: the engine's own closure, passed through unchanged, which makes the
/// borrowed handle of the engine being built itself. The C exports run their `Build` and user pointer in one (`call_build`); the
/// static wrapper makes its handle with `Borrowed`, one closure a callback as the engine's own callers.
pub type Inner<'a> = &'a mut dyn FnMut(&mut Ui);

/// The borrowed handle of a callback the static wrapper runs itself (`Inner`): `enter` makes it current (and building) until it is
/// dropped, also when the callback unwinds. It stays where it is from `enter` on.
pub struct Borrowed { handle: PxuiUi, prev: *mut PxuiUi }
impl Borrowed {
    /// A handle over `engine`, the engine of a callback that a call on the handle `outer` runs (its state is `outer`'s).
    /// # Safety
    /// `outer` is the live handle whose call is running the callback; `enter` is called before the value is dropped.
    #[inline(always)]
    /// It sees `outer`'s theme (and takes its cached pointer, so its first theme read is a load, not a call): no static wrapper
    /// callback is a `themed` scope's, which builds through a handle of its own.
    pub unsafe fn new(engine: &mut Ui, outer: *mut PxuiUi) -> Self {
        let mut handle = PxuiUi::borrowed(engine, unsafe { (*outer).shared });
        handle.theme = unsafe { (*outer).theme };
        Self { handle, prev: core::ptr::null_mut() }
    }
    /// The handle a static wrapper callback on `engine` builds through: `outer` itself when it is the building handle of `engine`
    /// (every container's callback: the call on `outer` runs it, and Rust keeps `outer`'s own `Ui` borrowed meanwhile), so nothing is
    /// made or restored; else (the frame's own callback, which no handle builds before) a borrowed one `slot` holds, current until
    /// `slot` is dropped. `building` and `current` are set together (`enter`, `Current::set`, `run_frame`), so `outer` is current too.
    /// # Safety
    /// `outer` is the live handle whose call is running the callback; `slot` stays where it is until dropped.
    #[inline(always)]
    pub unsafe fn handle(engine: &mut Ui, outer: *mut PxuiUi, slot: &mut Option<Borrowed>) -> *mut PxuiUi {
        if unsafe { Self::reusable(engine, outer) } { return outer; }
        unsafe { slot.insert(Self::new(engine, outer)).enter() }
    }
    /// Whether a callback on `engine` that a call on `outer` runs builds through `outer` itself (`handle`).
    /// # Safety
    /// As `handle`.
    #[inline(always)]
    pub unsafe fn reusable(engine: &Ui, outer: *mut PxuiUi) -> bool {
        unsafe { (*(*outer).shared).building == outer && core::ptr::eq((*outer).engine, engine) }
    }
    /// Makes the handle current and returns it.
    /// # Safety
    /// Called once, before the callback uses the handle; the value does not move until it is dropped.
    #[inline(always)]
    pub unsafe fn enter(&mut self) -> *mut PxuiUi {
        let (handle, shared): (*mut PxuiUi, _) = (&mut self.handle, self.handle.shared);
        unsafe { (*shared).building = handle; self.prev = core::mem::replace(&mut (*shared).current, handle); }
        handle
    }
}
/// The handle a C collection's row callbacks build through, made once per call over `outer`'s engine and state (its theme cached as
/// `Borrowed::new` caches it) and made current for each row with `Current::set`: one handle a list, not one a row. A row's handle
/// is no more valid in the next row than before (the per-row handle sat at the same stack address too).
/// # Safety
/// `outer` is the live handle whose collection call runs the row callbacks.
#[inline(always)]
pub(crate) unsafe fn row_handle(outer: *mut PxuiUi) -> PxuiUi {
    unsafe { PxuiUi { theme: (*outer).theme, engine: (*outer).engine, shared: (*outer).shared, owner: false } }
}
// as `Current`: a poisoned `Ui` keeps no current handle
impl Drop for Borrowed { #[inline(always)] fn drop(&mut self) { let shared = self.handle.shared; unsafe { if !(*shared).poisoned { (*shared).current = self.prev; (*shared).building = self.prev; } } } }

/// Makes a handle the current one (`Shared::current`) until dropped, then restores the previous: also when a callback unwinds.
pub(crate) struct Current { shared: *mut Shared, prev: *mut PxuiUi }
impl Current {
    /// Makes `handle` current (and the building one) for a callback of the frame being built.
    #[inline(always)]
    pub fn set(shared: *mut Shared, handle: *mut PxuiUi) -> Self {
        unsafe { (*shared).building = handle; }
        Self { shared, prev: unsafe { core::mem::replace(&mut (*shared).current, handle) } }
    }
}
// a poisoned `Ui` stays without a current handle until its next frame (`panicked` cleared it): every call takes `refuse`'s way out.
// In a frame `building` is `current`, so it follows it back.
impl Drop for Current { #[inline(always)] fn drop(&mut self) { unsafe { if !(*self.shared).poisoned { (*self.shared).current = self.prev; (*self.shared).building = self.prev; } } } }
/// No handle current and none building (a renderer callback) until it is dropped, which restores both.
pub(crate) struct NoCurrent { shared: *mut Shared, prev: *mut PxuiUi, building: *mut PxuiUi }
impl NoCurrent {
    pub fn set(shared: *mut Shared) -> Self {
        let (prev, building) = unsafe { (core::mem::replace(&mut (*shared).current, core::ptr::null_mut()), core::mem::replace(&mut (*shared).building, core::ptr::null_mut())) };
        Self { shared, prev, building }
    }
}
impl Drop for NoCurrent { fn drop(&mut self) { unsafe { if !(*self.shared).poisoned { (*self.shared).current = self.prev; (*self.shared).building = self.building; } } } }

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

/// What a builder call (`enter!`) on a handle that is not `Shared::building` returns, the call counted: `refuse`'s answer for a handle
/// that is not the current one, else `outside_frame`'s.
#[cold]
#[inline(never)]
pub(crate) unsafe fn not_building(ui: *mut PxuiUi) -> i32 {
    let shared = unsafe { (*ui).shared };
    unsafe { note_call(shared) };
    if unsafe { usable(ui) } { unsafe { outside_frame(shared) } } else { unsafe { refuse(shared) } }
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


/// Calls `build` with a borrowed handle over `engine`. The handle sees the theme of the call running it (the current handle), so it
/// takes that handle's cached theme pointer (`call_build_seeing`).
#[inline(always)]
pub(crate) fn call_build(shared: *mut Shared, engine: &mut Ui, build: Build, user: *mut c_void) {
    let current = unsafe { (*shared).current };
    unsafe { call_build_seeing(shared, engine, if current.is_null() { core::ptr::null() } else { (*current).theme }, build, user) }
}

/// `call_build` whose handle starts with `theme` cached: the theme it sees, or null to read it on first use (a `themed` scope's
/// callback; the frame's own callback gives the owner's).
/// # Safety
/// `theme` is null or the theme in effect while `build` runs.
#[inline(always)]
pub(crate) unsafe fn call_build_seeing(shared: *mut Shared, engine: &mut Ui, theme: *const pxui_types::theme::ThemeSnapshot, build: Build, user: *mut c_void) {
    if let Some(build) = build {
        let mut borrowed = PxuiUi::borrowed(engine, shared);
        borrowed.theme = theme;
        let handle: *mut PxuiUi = &mut borrowed; let _current = Current::set(shared, handle); build(user, handle.cast());
    }
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
    shared.building = core::ptr::null_mut();
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
    if ascii(bytes) { Some(unsafe { std::str::from_utf8_unchecked(bytes) }) } else { non_ascii(bytes) }
}

/// The full UTF-8 check of text that is not all ASCII, out of line: a caller whose texts are ASCII (the most) keeps no registers for it.
#[cold]
#[inline(never)]
fn non_ascii(bytes: &'static [u8]) -> Option<&'static str> { std::str::from_utf8(bytes).ok() }

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

pub(crate) fn to_abi_box(b: UiBox) -> PxuiBox { PxuiBox { index: b.index as u64, box_key: b.box_key, box_slot: b.box_slot as u64, generation: b.generation, ui: b.ui } }
/// The engine's handle a caller's `raw` names, or `None` when it is not one of `engine`'s: the serial of another `Ui`, or zero (a zeroed
/// handle), is refused here, one compare, before any slot is read.
#[inline(always)]
pub(crate) fn core_box(engine: &Ui, raw: PxuiBox) -> Option<UiBox> {
    if raw.ui != engine.serial() { return None; }
    unconfirmed_box(raw)
}
/// The engine's handle `raw` spells, its serial not yet compared: its caller compares it, or the library itself wrote it.
#[inline(always)]
pub(crate) fn unconfirmed_box(raw: PxuiBox) -> Option<UiBox> {
    #[cfg(target_pointer_width = "64")]
    { Some(unsafe { core::mem::transmute::<PxuiBox, UiBox>(raw) }) }
    #[cfg(not(target_pointer_width = "64"))]
    { Some(UiBox { index: usize::try_from(raw.index).ok()?, box_key: raw.box_key, box_slot: usize::try_from(raw.box_slot).ok()?, generation: raw.generation, ui: raw.ui }) }
}
/// On a 64-bit target the engine's handle is `PxuiBox` byte for byte, so it crosses as one copy.
#[cfg(target_pointer_width = "64")]
const _: () = {
    use core::mem::{offset_of, size_of};
    assert!(size_of::<UiBox>() == size_of::<PxuiBox>() && offset_of!(UiBox, index) == offset_of!(PxuiBox, index) && offset_of!(UiBox, box_key) == offset_of!(PxuiBox, box_key)
        && offset_of!(UiBox, box_slot) == offset_of!(PxuiBox, box_slot) && offset_of!(UiBox, generation) == offset_of!(PxuiBox, generation) && offset_of!(UiBox, ui) == offset_of!(PxuiBox, ui));
};
/// `core_box` against the engine behind `ui` (none: a handle that has none).
pub(crate) unsafe fn core_box_of(ui: *mut PxuiUi, raw: PxuiBox) -> Option<UiBox> { core_box(unsafe { engine(ui) }?, raw) }
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
    let b = core_box(engine, raw)?;
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
