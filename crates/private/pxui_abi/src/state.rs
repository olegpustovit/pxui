//! The state behind a `PxuiUi` handle and the helpers every export shares: string and box decoding,
//! validation, the poison flag and the per-frame buffers that keep a settled frame allocation-free.
use crate::{PXUI_ERR_ARGUMENT, Str};
use core::ffi::c_void;
use pxui_core::ui::{Ui, UiBox};
use pxui_types::{Event as FfiEvent, PxuiBox, ffi::{A11yNode, Output, Slice}, layout::Track};
use std::{collections::HashMap, rc::Rc};

/// Calls into the library since the process started (`pxui_counter`): one plain load and store, for the one thread the contract allows.
static CALLS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
pub(crate) fn note_call() {
    use core::sync::atomic::Ordering::Relaxed;
    CALLS.store(CALLS.load(Relaxed).wrapping_add(1), Relaxed);
}
pub(crate) fn calls() -> u64 { CALLS.load(core::sync::atomic::Ordering::Relaxed) }

/// One string at most 256 MiB: a whole source file is text one widget edits; a longer one is refused with an error, never cut.
pub(crate) const MAX_TEXT_BYTES: usize = 1 << 28;
pub(crate) const MAX_TOTAL_TEXT_BYTES: usize = 8 << 20;

/// One signal's key events and the joined text some of them carry, kept between frames so a frame that
/// reports keys allocates only while the buffers grow.
#[derive(Default)]
pub(crate) struct KeyChunk { pub events: Vec<FfiEvent>, pub text: String }

/// Everything a `Ui` owns besides the engine. Borrowed callback handles point at the same value.
pub(crate) struct Shared {
    pub error: String,
    pub poisoned: bool,
    /// a paint call was refused (a span left open, a library call under an open span): the frame that ran the callback reports it
    pub paint_error: bool,
    pub in_frame: bool,
    /// renderers created on this `Ui`: each holds a raw pointer to the engine, so it outlives none of them
    pub renderers: u32,
    /// text handed to C (an edited field, the picker query), valid until the next frame
    pub returned: Vec<String>,
    /// grid tracks the engine's nodes borrow, one shared slice per keyed node that has a grid (equal lists share one)
    pub grid_tracks: HashMap<i64, Rc<[Track]>>,
    /// replaced track slices, freed at the end of the frame (the old node may still borrow them until then)
    pub grid_retired: Vec<Rc<[Track]>>,
    /// the output of the last frame, built once in `finish_frame` into these buffers
    pub output: Output,
    pub nodes: Vec<A11yNode>,
    pub layers: Vec<pxui_types::ffi::PxuiLayerInfo>,
    pub dirty: Vec<pxui_types::ffi::PxuiDirty>,
    pub last_children: Vec<u32>,
    pub chunks: Vec<KeyChunk>,
    pub chunks_used: usize,
    /// the engine's input record of the last frame, kept for its vectors' capacity: a frame's events refill it without allocating
    pub spare_input: Option<input::Frame>,
    /// key codes of a box's held keys, valid until the next frame
    pub key_codes: Vec<u32>,
    pub menu: Vec<(String, bool)>,
    /// the input frame `pxui_ui_input` last handed out, and its wire form (the events point into the frame's own strings)
    pub routed: Option<input::frame::Frame>,
    pub routed_events: Vec<FfiEvent>,
    pub routed_text: Vec<String>,
    pub routed_keys: Vec<u32>,
    pub routed_buttons: Vec<u32>,
    pub hints: Vec<(String, String)>,
    /// hit callbacks the engine holds or has let go of, kept to reuse (see `box_ops::hit_closure`)
    pub hits: Vec<(Rc<dyn Fn(pxui_types::geom::Rect) -> pxui_types::geom::Rect>, Rc<crate::box_ops::HitSlot>)>,
    /// an inspector head's actions, reused between frames like the hints
    pub actions: Vec<pxui_core::ui::HeadAction>,
    /// the name validator of an inspector head: one closure over a cell the call fills, so a settled head allocates none
    pub rename: Option<(Rc<core::cell::Cell<(crate::text_inputs::TextValid, usize)>>, Rc<dyn Fn(&str) -> bool>)>,
    pub strs: Vec<&'static str>,
    pub menus: Vec<(&'static str, &'static [&'static str])>,
}

impl Shared {
    /// The string scratch, taken so that a callback running meanwhile finds it empty rather than clobbers it; give it back with `put_strs`.
    pub fn take_strs(&mut self) -> Vec<&'static str> { core::mem::take(&mut self.strs) }
    pub fn put_strs(&mut self, mut strs: Vec<&'static str>) { strs.clear(); self.strs = strs; }
    pub fn new() -> Self {
        Self {
            error: String::new(), poisoned: false, paint_error: false, in_frame: false, renderers: 0, returned: Vec::new(),
            grid_tracks: HashMap::new(), grid_retired: Vec::new(), output: Output::default(), nodes: Vec::new(),
            layers: Vec::new(), dirty: Vec::new(), last_children: Vec::new(), chunks: Vec::new(), chunks_used: 0, spare_input: None, key_codes: Vec::new(), menu: Vec::new(), routed: None, routed_events: Vec::new(), routed_text: Vec::new(), routed_keys: Vec::new(), routed_buttons: Vec::new(),
            hints: Vec::new(), hits: Vec::new(), actions: Vec::new(), rename: None, strs: Vec::new(), menus: Vec::new(),
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
    /// Keeps `value` alive until the next frame and returns it as a `Str`.
    pub(crate) fn give(&self, value: String) -> Str {
        let returned = &mut self.shared().returned;
        returned.push(value);
        let value = returned.last().map_or("", String::as_str);
        Str { ptr: value.as_ptr(), len: value.len() }
    }
}

/// Calls `build` with a borrowed handle over `engine`.
pub(crate) fn call_build(shared: *mut Shared, engine: &mut Ui, build: Build, user: *mut c_void) {
    if let Some(build) = build {
        let mut borrowed = PxuiUi::borrowed(engine, shared);
        build(user, (&mut borrowed as *mut PxuiUi).cast());
    }
}

pub use pxui_types::ffi::UiBuild as Build;

/// Where an export finds the shared state of its first argument, so the export macro can poison it.
pub(crate) trait Handle { fn shared(self) -> *mut Shared; }
impl Handle for *mut PxuiUi { fn shared(self) -> *mut Shared { if self.is_null() { core::ptr::null_mut() } else { unsafe { (*self).shared } } } }
impl Handle for *const PxuiUi { fn shared(self) -> *mut Shared { Handle::shared(self as *mut PxuiUi) } }
impl Handle for *mut crate::PxuiRenderer { fn shared(self) -> *mut Shared { if self.is_null() { core::ptr::null_mut() } else { unsafe { (*self).shared } } } }
impl Handle for *mut crate::paint::PxuiPaint { fn shared(self) -> *mut Shared { if self.is_null() { core::ptr::null_mut() } else { unsafe { (*self).shared } } } }

pub(crate) unsafe fn poisoned(shared: *mut Shared) -> bool { !shared.is_null() && unsafe { (*shared).poisoned } }

/// Records a caught panic: the `Ui` fails every call until its next frame; a panic of the frame itself ends the frame.
pub(crate) unsafe fn panicked(shared: *mut Shared, payload: Box<dyn core::any::Any + Send>, leave_frame: bool) {
    if shared.is_null() { return; }
    let shared = unsafe { &mut *shared };
    shared.poisoned = true;
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

/// Records `message` as the last error and returns `PXUI_ERR_ARGUMENT`.
pub(crate) unsafe fn fail(ui: *mut PxuiUi, message: &str) -> i32 { unsafe { fail_code(ui, message, PXUI_ERR_ARGUMENT) } }

/// Records `message` as the last error and returns `code`.
pub(crate) unsafe fn fail_code(ui: *mut PxuiUi, message: &str, code: i32) -> i32 {
    if let Some(ui) = unsafe { get(ui) } {
        let error = &mut ui.shared().error;
        error.clear();
        error.push_str(message);
    }
    code
}

/// UTF-8 text from ptr+len. NUL is ordinary text.
pub(crate) unsafe fn text(value: Str) -> Option<&'static str> {
    if value.len == 0 { return Some(""); }
    if value.ptr.is_null() { return None; }
    let bytes = unsafe { std::slice::from_raw_parts(value.ptr, value.len) };
    std::str::from_utf8(bytes).ok().map(|s| unsafe { &*(s as *const str) })
}

/// `text` limited to `MAX_TEXT_BYTES`.
pub(crate) unsafe fn checked(value: Str) -> Option<&'static str> {
    if value.len > MAX_TEXT_BYTES { None } else { unsafe { text(value) } }
}

/// Decodes a span of strings into `out` (cleared first): at most `max` of them, at most eight megabytes in all.
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
