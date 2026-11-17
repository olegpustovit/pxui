use crate::Handle;
use core::{any::Any, ffi::c_void, mem::ManuallyDrop, ptr::NonNull};
use pxui_types::{Size, geom::Rect, layout::{Along, Placement, Side}, ffi::PxuiAnchorDesc};
#[cfg(not(feature = "static"))]
use crate::node::size;
use crate::{Ui, check};
#[cfg(not(feature = "static"))]
use pxui_types::ffi::PxuiKeyedDesc;
use pxui_types::ui::{Key, Overlay};
#[cfg(not(feature = "static"))]
use pxui_types::PxuiPanel;

// Per-thread state of the wrapper, by design (OS-grade criterion 2): each lives in a `thread_local`, is shared by every `Ui` on
// that thread, is never touched from another thread and holds no app pointer past the call that filled it. Bounded:
// - `CALLBACK_PANIC` (below): at most one deferred panic per thread, taken by the outermost call; `PENDING` counts the threads
//   holding one, so a settled process skips the lookup.
// - `paint::POOL`: released closure boxes, at most `POOLED` (64) per closure layout; layouts are the app's distinct closure types.
// - `overlays::ROWS` / `MENU_COLUMNS`: a picker's rows and a long menu's columns, grown to the largest one built on the thread.
// - `ORPHANS` (lib.rs): each `Ui` dropped before its renderers, until a renderer drop on this thread destroys it.
// None of them is the process allocator: allocation counting is `mem_sys`'s opt-in `counting` feature (test/perf builds).

// A panic in an app callback must not unwind through the library. The first one is kept here and every later callback
// of the call is skipped; the outermost call that can run callbacks (frame, prepare, draw) resumes it after it returns.
thread_local! { static CALLBACK_PANIC: std::cell::RefCell<Option<Box<dyn Any + Send>>> = const { std::cell::RefCell::new(None) }; }
/// How many threads hold a deferred panic: while none does (every call of a settled app), a callback skips the thread-local lookup.
static PENDING: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
pub(crate) fn take_callback_panic() -> Option<Box<dyn Any + Send>> {
    if PENDING.load(core::sync::atomic::Ordering::Relaxed) == 0 { return None; }
    let taken = CALLBACK_PANIC.with(|slot| slot.borrow_mut().take());
    if taken.is_some() { PENDING.fetch_sub(1, core::sync::atomic::Ordering::Relaxed); }
    taken
}
#[inline]
pub(crate) fn deferred() -> bool { PENDING.load(core::sync::atomic::Ordering::Relaxed) != 0 && CALLBACK_PANIC.with(|slot| slot.borrow().is_some()) }
#[inline]
pub(crate) fn resume_deferred() { if let Some(panic) = take_callback_panic() { std::panic::resume_unwind(panic) } }

pub(crate) fn defer_callback_panic(panic: Box<dyn Any + Send>) {
    CALLBACK_PANIC.with(|slot| { let mut slot = slot.borrow_mut(); if slot.is_none() { *slot = Some(panic); PENDING.fetch_add(1, core::sync::atomic::Ordering::Relaxed); } });
}

pub(crate) extern "C" fn call<F: FnMut(&mut Ui)>(user: *mut c_void, raw: *mut c_void) {
    if raw.is_null() || deferred() { return; }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ui = ManuallyDrop::new(Ui(NonNull::new(raw.cast()).unwrap()));
        (unsafe { &mut *user.cast::<F>() })(&mut ui);
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}

fn encode_placement(value: Placement) -> PxuiAnchorDesc {
    let side = match value.side { Side::Below => 0, Side::Above => 1, Side::Right => 2, Side::Left => 3 };
    let align = match value.align { Along::Start => 0, Along::Center => 1, Along::End => 2 };
    PxuiAnchorDesc { has_anchor: 1, reserved: [0; 3], rect: Rect::default(), side, align, gap: value.gap, margin: value.margin, flip: value.flip as u8, reserved_tail: [0; 3] }
}


impl Ui {
    /// The one body of `modal`: linked statically, the library's trusted entry with the overlay; else the keyed export.
    #[track_caller]
    pub(crate) fn modal_key(&mut self, key: Key, overlay: &Overlay<'_>, mut build: &mut dyn FnMut(&mut Ui)) -> bool {
        let mut id_label = [0u8; 17];
        let key = key.label(&mut id_label);
        #[cfg(feature = "static")]
        { let mut out = false; check(self, unsafe { pxui_abi_link::trusted::modal(self.0.as_ptr(), key, overlay, inner!(|ui: &mut Ui| build(ui)), &mut out) }); out }
        #[cfg(not(feature = "static"))]
        {
            let keyed = PxuiKeyedDesc::new(key, crate::clip(overlay.name.unwrap_or("")));
            let width = overlay.w;
            let mut out = 0;
            check(self, unsafe { keyed_sys::pxui_ui_modal_keyed(self.0.as_ptr(), &keyed, width.is_some() as u8, width.unwrap_or_default(), Some(call::<&mut dyn FnMut(&mut Ui)>), &mut build as *mut _ as *mut c_void, &mut out) });
            out != 0
        }
    }

    /// The one body of `panel`: linked statically, the library's trusted entry with the overlay; else the overlay as the panel record of
    /// the keyed export.
    #[track_caller]
    pub(crate) fn panel_key(&mut self, key: Key, overlay: &Overlay<'_>, mut f: &mut dyn FnMut(&mut Ui)) {
        let mut id_label = [0u8; 17];
        let key = key.label(&mut id_label);
        #[cfg(feature = "static")]
        let rc = unsafe { pxui_abi_link::trusted::panel(self.0.as_ptr(), key, overlay, inner!(|ui: &mut Ui| f(ui))) };
        #[cfg(not(feature = "static"))]
        let rc = {
            // the options a panel usually has are written as they are; each optional part only when it is there
            let mut panel = PxuiPanel { x: overlay.at.0, y: overlay.at.1, width: overlay.w.unwrap_or(280.0), padding: overlay.padding as i64, window: overlay.window as u8, ..PxuiPanel::default() };
            if let Some(height) = overlay.h { (panel.has_height, panel.height) = (1, height); }
            if let Some(height) = overlay.max_h { (panel.has_max_height, panel.max_height) = (1, height); }
            if let Some(stroke) = overlay.stroke { (panel.has_stroke, panel.stroke) = (1, stroke); }
            if let Some(look) = overlay.look { panel_look(&mut panel, look); }
            let keyed = PxuiKeyedDesc::new(key, crate::clip(overlay.name.unwrap_or("")));
            unsafe { keyed_sys::pxui_ui_panel_keyed(self.0.as_ptr(), &keyed, &panel, Some(call::<&mut dyn FnMut(&mut Ui)>), &mut f as *mut _ as *mut c_void) }
        };
        check(self, rc)
    }

    /// The one body of `scroll_view`: keyed by `key`, unnamed.
    #[track_caller]
    pub(crate) fn scroll_view_key(&mut self, key: Key, width: Size, height: Size, mut build: &mut dyn FnMut(&mut Ui)) -> Handle {
        let mut id_label = [0u8; 17];
        let key = key.label(&mut id_label);
        let mut out = Handle::default();
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::scroll_view(self.0.as_ptr(), key, "", width, height, inner!(|ui: &mut Ui| build(ui)), &mut out) });
        #[cfg(not(feature = "static"))]
        check(self, unsafe { keyed_sys::pxui_ui_scroll_view_keyed(self.0.as_ptr(), &PxuiKeyedDesc::new(key, ""), size(width), size(height), Some(call::<&mut dyn FnMut(&mut Ui)>), &mut build as *mut _ as *mut c_void, &mut out) });
        out
    }

    /// The one body of `view_of`: the view keyed by `key` under the parent, sized by its parent, kept while `revision` is unchanged.
    #[inline]
    #[track_caller]
    pub(crate) fn view_key(&mut self, key: Key, revision: u64, mut build: &mut dyn FnMut(&mut Ui)) -> Handle {
        let mut id_label = [0u8; 17];
        let key = key.label(&mut id_label);
        let mut out = Handle::default();
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::view_of(self.0.as_ptr(), key, revision, inner!(|ui: &mut Ui| build(ui)), &mut out) });
        #[cfg(not(feature = "static"))]
        check(self, unsafe { keyed_sys::pxui_ui_view_keyed(self.0.as_ptr(), &PxuiKeyedDesc::new(key, ""), revision, Some(call::<&mut dyn FnMut(&mut Ui)>), &mut build as *mut _ as *mut c_void, &mut out) });
        out
    }

    /// The one body of `anchored`: keyed by `key`, the overlay's placement and size (each its content's when unset), named by its name.
    #[track_caller]
    pub(crate) fn anchored_key(&mut self, key: Key, anchor: Rect, overlay: &Overlay<'_>, mut build: &mut dyn FnMut(&mut Ui)) -> Handle {
        let mut id_label = [0u8; 17];
        let key = key.label(&mut id_label);
        let mut out = Handle::default();
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::anchored(self.0.as_ptr(), key, anchor, overlay, inner!(|ui: &mut Ui| build(ui)), &mut out) });
        #[cfg(not(feature = "static"))]
        {
            let mut descriptor = encode_placement(overlay.place);
            descriptor.rect = anchor;
            let (width, height) = (overlay.w.map_or(Size::Fit, Size::Px), overlay.h.map_or(Size::Fit, Size::Px));
            check(self, unsafe { keyed_sys::pxui_ui_anchored_keyed(self.0.as_ptr(), &PxuiKeyedDesc::new(key, crate::clip(overlay.name.unwrap_or(""))), &descriptor, size(width), size(height), Some(call::<&mut dyn FnMut(&mut Ui)>), &mut build as *mut _ as *mut c_void, &mut out) });
        }
        out
    }
}

/// A panel descriptor's look, out of line: inlined, its encoding made the plain panel's descriptor a select of every field.
#[cfg(not(feature = "static"))]
#[cold]
#[inline(never)]
fn panel_look(panel: &mut PxuiPanel, look: pxui_types::theme::Look) { (panel.has_look, panel.look) = (1, crate::node::look(look)); }


// The keyed container exports (ABI 1.68): key and name apart in a `PxuiKeyedDesc`.
#[cfg(not(feature = "static"))]
mod keyed_sys {
    use core::ffi::c_void;
    use pxui_types::{PxuiBox, PxuiPanel, ffi::{PxuiAnchorDesc, PxuiKeyedDesc, PxuiSizeDesc, UiBuild}};
    use crate::sys::PxuiUi;
    #[link(name = "pxui")]
    unsafe extern "C" {
        pub(super) fn pxui_ui_modal_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, has_width: u8, width: f32, build: UiBuild, user: *mut c_void, out: *mut u8) -> i32;
        pub(super) fn pxui_ui_panel_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, panel: *const PxuiPanel, build: UiBuild, user: *mut c_void) -> i32;
        pub(super) fn pxui_ui_view_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, revision: u64, build: UiBuild, user: *mut c_void, out: *mut PxuiBox) -> i32;
        pub(super) fn pxui_ui_scroll_view_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, width: PxuiSizeDesc, height: PxuiSizeDesc, build: UiBuild, user: *mut c_void, out: *mut PxuiBox) -> i32;
        pub(super) fn pxui_ui_anchored_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, anchor: *const PxuiAnchorDesc, width: PxuiSizeDesc, height: PxuiSizeDesc, build: UiBuild, user: *mut c_void, out: *mut PxuiBox) -> i32;
    }
}
