use crate::{Handle, OverlayResponse};
use core::{any::Any, ffi::c_void, mem::ManuallyDrop, ptr::NonNull};
use pxui_types::{geom::Rect, layout::{Along, Placement, Side}, ffi::PxuiAnchorDesc};
#[cfg(not(feature = "static"))]
use crate::node::size;
use crate::{Ui, check};
#[cfg(not(feature = "static"))]
use pxui_types::ffi::PxuiKeyedDesc;
use pxui_types::ui::{Key, Overlay};
#[cfg(not(feature = "static"))]
use pxui_types::PxuiPanel;

// The wrapper's only state outside a `Ui`, all of it here (OS-grade criteria 2 and 4). Each is shared by every `Ui` of its thread,
// never handed to another thread, holds no app pointer past the call that filled it, and has a fixed bound:
// - `CALLBACK_PANIC` (below, thread-local): at most one deferred panic per thread, taken by the outermost call that ran the callback.
// - `PENDING` (below): the one process-wide value, an atomic count of the threads holding a deferred panic (at most the thread
//   count), read Relaxed from any thread so a settled process skips the thread-local lookup.
// - `paint::POOL` (thread-local): released canvas boxes, at most `POOLED` (64) per closure layout; the layouts are the app's distinct
//   canvas closure types, a fixed set per program.
// - `overlays::ROWS` (thread-local): one picker's rows, at most the library's 4096 rows and 16 MiB of text (a fill past that is
//   refused and its rows dropped).
// - `overlays::MENU_COLUMNS` (thread-local, dynamic link only): one long menu's columns, at most `MENU_ROWS` (4096) rows; `Ui::menu`
//   refuses a longer menu before copying a row.
// - `text_inputs::DESCRIPTION` (thread-local): one emptied description buffer per peak nested area call,
//   each retained only up to twice the 64 KiB display limit.
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

/// The C callback of a children closure: `user` points at the caller's `&mut dyn FnMut(&mut Ui)`, `raw` is the library's `Ui`.
pub(crate) extern "C" fn call(user: *mut c_void, raw: *mut c_void) {
    if raw.is_null() || deferred() { return; }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ui = ManuallyDrop::new(Ui(NonNull::new(raw.cast()).unwrap()));
        (unsafe { &mut *user.cast::<&mut dyn FnMut(&mut Ui)>() })(&mut ui);
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}

fn encode_placement(value: Placement) -> PxuiAnchorDesc {
    let side = match value.side { Side::Below => 0, Side::Above => 1, Side::Right => 2, Side::Left => 3 };
    let align = match value.align { Along::Start => 0, Along::Center => 1, Along::End => 2 };
    PxuiAnchorDesc { has_anchor: 1, rect: Rect::default(), side, align, gap: value.gap, margin: value.margin, flip: value.flip as u8, ..PxuiAnchorDesc::default() }
}


impl Ui {
    /// The one body of `modal`: linked statically, the library's trusted entry with the overlay; else the keyed export.
    #[track_caller]
    pub(crate) fn modal_key(&mut self, key: Key, overlay: &Overlay<'_>, mut build: &mut dyn FnMut(&mut Ui)) -> OverlayResponse {
        let mut id_label = [0u8; 17];
        let key = key.label(&mut id_label);
        let mut handle = Handle::default();
        #[cfg(feature = "static")]
        { let mut open = false; check(self, unsafe { let outer = self.0.as_ptr(); pxui_abi_link::trusted::modal(outer, key, overlay, inner!(outer, |ui: &mut Ui| build(ui)), &mut open, &mut handle) }); OverlayResponse { handle, open } }
        #[cfg(not(feature = "static"))]
        {
            let keyed = PxuiKeyedDesc::new(key, crate::clip(overlay.name.unwrap_or("")));
            let present = match overlay.present { crate::Presentation::Centered => 0, crate::Presentation::Bottom => 1, crate::Presentation::Full => 2, crate::Presentation::Top => 3 };
            let sheet = pxui_types::ffi::PxuiSheetDesc {
                size: core::mem::size_of::<pxui_types::ffi::PxuiSheetDesc>() as u32, has_width: overlay.w.is_some() as u8, present, reserved: [0; 2],
                width: overlay.w.unwrap_or_default(), dismiss_fraction: overlay.dismiss.fraction.unwrap_or_default(), dismiss_velocity: overlay.dismiss.velocity.unwrap_or_default(),
                top: overlay.at.1, max_h: overlay.max_h.unwrap_or_default(),
            };
            let mut out = 0;
            check(self, unsafe { keyed_sys::pxui_ui_modal(self.0.as_ptr(), &keyed, &sheet, Some(call), &mut build as *mut _ as *mut c_void, &mut out, &mut handle) });
            OverlayResponse { handle, open: out != 0 }
        }
    }

    /// The one body of `panel`: linked statically, the library's trusted entry with the overlay; else the overlay as the panel record of
    /// the keyed export.
    #[track_caller]
    #[inline]
    pub(crate) fn panel_key(&mut self, key: Key, overlay: &Overlay<'_>, mut f: &mut dyn FnMut(&mut Ui)) -> OverlayResponse {
        let mut id_label = [0u8; 17];
        let key = key.label(&mut id_label);
        let mut handle = Handle::default();
        #[cfg(feature = "static")]
        let rc = unsafe { let outer = self.0.as_ptr(); pxui_abi_link::trusted::panel(outer, key, overlay, inner!(outer, |ui: &mut Ui| f(ui)), &mut handle) };
        #[cfg(not(feature = "static"))]
        let rc = {
            // the options a panel usually has are written as they are; each optional part only when it is there
            let mut panel = PxuiPanel { x: overlay.at.0, y: overlay.at.1, width: overlay.w.unwrap_or(280.0), padding_points: overlay.padding, padding: if overlay.padding.is_finite() && overlay.padding >= 0.0 { overlay.padding.round() as i64 } else { -1 }, window: overlay.window as u8, ..PxuiPanel::default() };
            if let Some(height) = overlay.h { (panel.has_height, panel.height) = (1, height); }
            if let Some(height) = overlay.max_h { (panel.has_max_height, panel.max_height) = (1, height); }
            if let Some(stroke) = overlay.stroke { (panel.has_stroke, panel.stroke) = (1, stroke); }
            // the encoded look lives to the end of the call, which reads it where it lies; a plain panel only tests the option
            let encoded;
            if let Some(look) = overlay.look { encoded = crate::node::look(*look); panel.look = &encoded; }
            let keyed = PxuiKeyedDesc::new(key, crate::clip(overlay.name.unwrap_or("")));
            unsafe { keyed_sys::pxui_ui_panel(self.0.as_ptr(), &keyed, &panel, Some(call), &mut f as *mut _ as *mut c_void, &mut handle) }
        };
        check(self, rc);
        OverlayResponse { handle, open: true }
    }

    /// The one body of `scroll_view`: keyed by `key`, unnamed, laid out by `node`.
    #[track_caller]
    pub(crate) fn scroll_view_node(&mut self, key: Key, node: pxui_types::Node, mut build: &mut dyn FnMut(&mut Ui)) -> Handle {
        let mut id_label = [0u8; 17];
        let key = key.label(&mut id_label);
        let mut out = Handle::default();
        #[cfg(feature = "static")]
        check(self, unsafe { let outer = self.0.as_ptr(); pxui_abi_link::trusted::scroll_view(outer, key, "", &node, inner!(outer, |ui: &mut Ui| build(ui)), &mut out) });
        #[cfg(not(feature = "static"))]
        check(self, unsafe { keyed_sys::pxui_ui_scroll_view(self.0.as_ptr(), &PxuiKeyedDesc::new(key, ""), &node, Some(call), &mut build as *mut _ as *mut c_void, &mut out) });
        out
    }

    /// Keeps the current view's positioned labels when an instant root theme changes only colors.
    ///
    /// Call at the end of a `view_of` containing only `label_at` calls. Their text, placement and
    /// styles must be independent of theme colors and system appearance. Set each label's ink by
    /// token, or use the theme's label ink. Views with canvases or scoped themes rebuild normally.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     ui.view_of("copy", &0, |ui| {
    ///         ui.label_at("title".into(), pxui::Rect::new(0.0, 0.0, 80.0, 20.0), "Albums", pxui::Label::BODY.line_height(20.0)).unwrap();
    ///         ui.retain_palette_labels();
    ///     });
    /// });
    /// ```
    #[inline]
    pub fn retain_palette_labels(&mut self) {
        #[cfg(feature = "static")]
        let _ = unsafe { pxui_abi_link::pxui_ui_retain_palette_labels(self.0.as_ptr()) };
        #[cfg(not(feature = "static"))]
        let _ = unsafe { keyed_sys::pxui_ui_retain_palette_labels(self.0.as_ptr()) };
    }

    /// Lets the current view recolor positioned labels and canvas shapes on an instant palette swap.
    ///
    /// Call at the start of a root-themed view. Keep layout, node looks and interaction independent of
    /// palette values. Pass `Rgba::token` into shape fills or strokes, including in custom icon art.
    /// Canvas output must use these shapes and may not read concrete theme colors. Nested views need
    /// the same contract. Views with unsupported node looks or active node tweens rebuild.
    /// Views with more than 16 canvas color ranges rebuild.
    /// Allocates: never once the retained view storage is warm.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{Fill, Node, Rgba, theme::ColorToken};
    /// pxui::doc::frames(1, &mut |ui| {
    ///     ui.view_of("badge", &0, |ui| {
    ///         ui.retain_palette_paint();
    ///         let badge = ui.node("plate", Node::column().w_px(50.0).h_px(20.0));
    ///         ui.canvas(&badge, |paint, rect| paint.fill(rect, Fill::new(Rgba::token(ColorToken::ACCENT))));
    ///     });
    /// });
    /// ```
    #[inline]
    pub fn retain_palette_paint(&mut self) {
        #[cfg(feature = "static")]
        let _ = unsafe { pxui_abi_link::pxui_ui_retain_palette_paint(self.0.as_ptr()) };
        #[cfg(not(feature = "static"))]
        let _ = unsafe { keyed_sys::pxui_ui_retain_palette_paint(self.0.as_ptr()) };
    }

    /// Lets the current view keep its build when the window resizes and the view's layout fits.
    ///
    /// Use inside a `view_of` whose build depends on the window size only through its local
    /// bounds or explicit revision. A change to the view's own bounds still triggers layout and
    /// a size change rebuilds its contents. Device density changes always invalidate the view.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     ui.column(pxui::Node::column().w_px(120.0).h_px(40.0), |ui| {
    ///         ui.view_of("transport", &0, |ui| {
    ///             ui.label("Playback", pxui::Label::BODY);
    ///             ui.retain_view_on_resize();
    ///         });
    ///     });
    /// });
    /// ```
    #[inline]
    pub fn retain_view_on_resize(&mut self) {
        #[cfg(feature = "static")]
        let _ = unsafe { pxui_abi_link::pxui_ui_retain_view_on_resize(self.0.as_ptr()) };
        #[cfg(not(feature = "static"))]
        let _ = unsafe { keyed_sys::pxui_ui_retain_view_on_resize(self.0.as_ptr()) };
    }

    /// The one body of `view_of`: the view keyed by `key` under the parent, sized by its parent, kept while `revision` is unchanged.
    #[track_caller]
    #[inline]
    pub(crate) fn view_key(&mut self, key: Key, revision: u64, mut build: &mut dyn FnMut(&mut Ui)) -> Handle {
        let mut id_label = [0u8; 17];
        let key = key.label(&mut id_label);
        let mut out = Handle::default();
        #[cfg(feature = "static")]
        check(self, unsafe { let outer = self.0.as_ptr(); pxui_abi_link::trusted::view_of(outer, key, revision, inner!(outer, |ui: &mut Ui| build(ui)), &mut out) });
        #[cfg(not(feature = "static"))]
        check(self, unsafe { keyed_sys::pxui_ui_view(self.0.as_ptr(), &PxuiKeyedDesc::new(key, ""), revision, Some(call), &mut build as *mut _ as *mut c_void, &mut out) });
        out
    }

    /// The one body of `anchored`: keyed by `key`, the overlay's placement and size (each its content's when unset), named by its name.
    #[track_caller]
    pub(crate) fn anchored_key(&mut self, key: Key, anchor: Rect, overlay: &Overlay<'_>, mut build: &mut dyn FnMut(&mut Ui)) -> OverlayResponse {
        let mut id_label = [0u8; 17];
        let key = key.label(&mut id_label);
        let mut out = Handle::default();
        #[cfg(feature = "static")]
        check(self, unsafe { let outer = self.0.as_ptr(); pxui_abi_link::trusted::anchored(outer, key, anchor, overlay, inner!(outer, |ui: &mut Ui| build(ui)), &mut out) });
        #[cfg(not(feature = "static"))]
        {
            let mut descriptor = encode_placement(overlay.place);
            descriptor.rect = anchor;
            let (width, height) = { use pxui_types::Size; (overlay.w.map_or(Size::Fit, Size::Px), overlay.h.map_or(Size::Fit, Size::Px)) };
            check(self, unsafe { keyed_sys::pxui_ui_anchored(self.0.as_ptr(), &PxuiKeyedDesc::new(key, crate::clip(overlay.name.unwrap_or(""))), &descriptor, size(width), size(height), Some(call), &mut build as *mut _ as *mut c_void, &mut out) });
        }
        OverlayResponse { handle: out, open: true }
    }
}


// The keyed container exports (ABI 1.68): key and name apart in a `PxuiKeyedDesc`.
#[cfg(not(feature = "static"))]
mod keyed_sys {
    use core::ffi::c_void;
    use pxui_types::{PxuiBox, PxuiPanel, ffi::{PxuiAnchorDesc, PxuiKeyedDesc, PxuiSizeDesc, UiBuild}};
    use crate::sys::PxuiUi;
    #[link(name = "pxui")]
    unsafe extern "C" {
        pub(super) fn pxui_ui_modal(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, sheet: *const pxui_types::ffi::PxuiSheetDesc, build: UiBuild, user: *mut c_void, out: *mut u8, root: *mut PxuiBox) -> i32;
        pub(super) fn pxui_ui_panel(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, panel: *const PxuiPanel, build: UiBuild, user: *mut c_void, root: *mut PxuiBox) -> i32;
        pub(super) fn pxui_ui_retain_palette_labels(ui: *mut PxuiUi) -> i32;
        pub(super) fn pxui_ui_retain_palette_paint(ui: *mut PxuiUi) -> i32;
        pub(super) fn pxui_ui_retain_view_on_resize(ui: *mut PxuiUi) -> i32;
        pub(super) fn pxui_ui_view(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, revision: u64, build: UiBuild, user: *mut c_void, out: *mut PxuiBox) -> i32;
        pub(super) fn pxui_ui_scroll_view(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, node: *const pxui_types::Node, build: UiBuild, user: *mut c_void, out: *mut PxuiBox) -> i32;
        pub(super) fn pxui_ui_anchored(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, anchor: *const PxuiAnchorDesc, width: PxuiSizeDesc, height: PxuiSizeDesc, build: UiBuild, user: *mut c_void, out: *mut PxuiBox) -> i32;
    }
}
