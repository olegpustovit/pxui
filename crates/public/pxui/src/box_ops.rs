//! Boxes and what hangs off them, with the engine's names and shapes (see `ui_state` for how failures surface).
use crate::composite::{call, defer_callback_panic, deferred};
use crate::node::{look, mark, size};
use crate::opts::{BoxOpts, HitFn};
use crate::{as_str, as_text, check, Ui};
use core::ffi::c_void;
use pxui_types::{geom::{Point, Rect}, ffi::{PxuiBoxDesc, PxuiLookDesc, PxuiMarkDesc, PxuiSizeDesc, Slice}, theme::Look, Dir, Size, Mark, PxuiBox, Str};
use std::{alloc::Layout, fmt::{Arguments, Write}};

library!(lib {
    fn pxui_ui_text_layout_count(ui: *mut PxuiUi, out: *mut u64) -> i32;
    fn pxui_ui_reveal(ui: *mut PxuiUi, list: *const PxuiBox, index: usize) -> i32;
    fn pxui_ui_dismiss(ui: *mut PxuiUi, box_: *const PxuiBox, request: *const pxui_types::ffi::PxuiDismissIn, out: *mut pxui_types::ffi::PxuiDismissOut) -> i32;
    fn pxui_ui_swipe_actions(ui: *mut PxuiUi, box_: *const PxuiBox, request: *const pxui_types::ffi::PxuiSwipeIn) -> i32;
    fn pxui_ui_gesture(ui: *mut PxuiUi, box_: *const PxuiBox, request: *const pxui_types::ffi::PxuiGestureIn, commands: u32, out: *mut pxui_types::ffi::PxuiGestureOut) -> i32;
    fn pxui_ui_box(ui: *mut PxuiUi, label: Str, desc: *const PxuiBoxDesc, out: *mut PxuiBox) -> i32;
    fn pxui_ui_set_at(ui: *mut PxuiUi, box_: *const PxuiBox, x: f32, y: f32) -> i32;
    fn pxui_ui_node_fields(ui: *mut PxuiUi, box_: *const PxuiBox, out: *mut pxui_types::ffi::PxuiNodeFields) -> i32;
    fn pxui_ui_node_layout(ui: *mut PxuiUi, box_: *const PxuiBox, out: *mut pxui_types::Node) -> i32;
    fn pxui_ui_set_node_layout(ui: *mut PxuiUi, box_: *const PxuiBox, node: *const pxui_types::Node) -> i32;
    fn pxui_ui_set_node_fields(ui: *mut PxuiUi, box_: *const PxuiBox, fields: *const pxui_types::ffi::PxuiNodeFields) -> i32;
    fn pxui_ui_patch_look(ui: *mut PxuiUi, box_: *const PxuiBox, value: *const PxuiLookDesc, out: *mut u8) -> i32;
    fn pxui_ui_patch_rect(ui: *mut PxuiUi, box_: *const PxuiBox, value: Rect, out: *mut u8) -> i32;
    fn pxui_ui_patch_mark(ui: *mut PxuiUi, box_: *const PxuiBox, value: *const PxuiMarkDesc, out: *mut u8) -> i32;
    fn pxui_ui_local_rect(ui: *mut PxuiUi, box_: *const PxuiBox, out: *mut Rect) -> i32;
    fn pxui_ui_scroll_offset(ui: *mut PxuiUi, box_: *const PxuiBox, which: u32, out: *mut f32) -> i32;
    fn pxui_ui_set_scroll_offset(ui: *mut PxuiUi, box_: *const PxuiBox, value: f32) -> i32;
    fn pxui_ui_text_state(ui: *mut PxuiUi, box_: *const PxuiBox, present: *mut u8, out: *mut Str) -> i32;
    fn pxui_ui_set_text_state(ui: *mut PxuiUi, box_: *const PxuiBox, present: u8, value: Str) -> i32;
    fn pxui_ui_cached_label(ui: *mut PxuiUi, box_: *const PxuiBox, index: u64, text: Str, out: *mut Str) -> i32;
    fn pxui_ui_to_front(ui: *mut PxuiUi, order: i64, box_: *const PxuiBox) -> i32;
    fn pxui_ui_carrying(ui: *mut PxuiUi, present: *mut u8, kind: *mut Str, value: *mut Str) -> i32;
    fn pxui_ui_carry(ui: *mut PxuiUi, has_from: u8, from: *const PxuiBox, kind: Str, value: Str) -> i32;
    fn pxui_ui_transfer_carry(ui: *mut PxuiUi, to: *const PxuiBox, out: *mut u8) -> i32;
    fn pxui_ui_cancel_carry(ui: *mut PxuiUi) -> i32;
    fn pxui_ui_drop_target(ui: *mut PxuiUi, box_: *const PxuiBox, tag: *mut u32, kind: *mut Str, value: *mut Str) -> i32;
    fn pxui_ui_context_at(ui: *mut PxuiUi, box_: *const PxuiBox, present: *mut u8, out: *mut Point) -> i32;
    fn pxui_ui_hovered(ui: *mut PxuiUi, box_: *const PxuiBox, within: u8, out: *mut u8) -> i32;
    fn pxui_ui_press_keys(ui: *mut PxuiUi, box_: *const PxuiBox, ptr: *mut *const u32, len: *mut usize) -> i32;
    fn pxui_ui_root_at(ui: *mut PxuiUi, x: f32, y: f32, roots: Slice<i64>, present: *mut u8, out: *mut i64) -> i32;
    fn pxui_ui_view_flat(ui: *mut PxuiUi, label: Str, revision: u64, width: PxuiSizeDesc, height: PxuiSizeDesc, build: pxui_types::ffi::UiBuild, user: *mut c_void, out: *mut PxuiBox) -> i32;
    fn pxui_ui_view_offset(ui: *mut PxuiUi, box_: *const PxuiBox, dx: f32, dy: f32) -> i32;
    fn pxui_ui_view_opacity(ui:*mut PxuiUi,box_:*const PxuiBox,opacity:f32)->i32;
    fn pxui_ui_view_scale(ui:*mut PxuiUi,box_:*const PxuiBox,scale:f32)->i32;
    fn pxui_ui_view_clip(ui:*mut PxuiUi,box_:*const PxuiBox,clip:*const Rect)->i32;
    fn pxui_ui_view_transform_about(ui: *mut PxuiUi, box_: *const PxuiBox, scale: f32, transform: *const f32, px: f32, py: f32) -> i32;
    fn pxui_ui_view_backdrop_blur(ui: *mut PxuiUi, box_: *const PxuiBox, sigma: f32) -> i32;
    fn pxui_ui_view_backdrop_saturation(ui: *mut PxuiUi, box_: *const PxuiBox, saturation: f32) -> i32;
});

/// A label written from `format_args!`: up to 128 bytes on the stack, longer ones on the heap.
pub(crate) struct Label {
    stack: [u8; 128],
    len: usize,
    heap: String,
}

impl Label {
    pub(crate) fn new(args: Arguments) -> Label {
        let mut label = Label { stack: [0; 128], len: 0, heap: String::new() };
        let _ = label.write_fmt(args);
        label
    }
    pub(crate) fn as_str(&self) -> &str {
        if self.heap.is_empty() { core::str::from_utf8(&self.stack[..self.len]).unwrap_or("") } else { &self.heap }
    }
}

impl Write for Label {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        if self.heap.is_empty() && self.len + s.len() <= self.stack.len() {
            self.stack[self.len..self.len + s.len()].copy_from_slice(s.as_bytes());
            self.len += s.len();
        } else {
            if self.heap.is_empty() { self.heap.push_str(core::str::from_utf8(&self.stack[..self.len]).unwrap_or("")); }
            self.heap.push_str(s);
        }
        Ok(())
    }
}

// The box's hit shape is an app closure: it crosses as a callback, the closure's `Rc` boxed behind the user pointer
// and released by the library exactly once.
extern "C" fn hit_call(user: *mut c_void, rect: Rect) -> Rect {
    if user.is_null() || deferred() { return rect; }
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (unsafe { &*user.cast::<HitFn>() })(rect))) {
        Ok(hit) => hit,
        Err(panic) => { defer_callback_panic(panic); rect }
    }
}
extern "C" fn hit_release(user: *mut c_void) {
    crate::paint::dropped(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { core::ptr::drop_in_place(user.cast::<HitFn>()) })));
    crate::paint::pool_give(Layout::new::<HitFn>(), user.cast());
}
fn hit_box(hit: HitFn) -> *mut c_void {
    let at = crate::paint::pool_take(Layout::new::<HitFn>()).cast::<HitFn>();
    unsafe { at.write(hit); }
    at.cast()
}

/// The payload a drag or key carry holds, borrowed from the library until the next mutable call on the `Ui`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PayloadRef<'a> {
    /// What the payload is, as the carrier named it (`"song"`, `"row"`).
    pub kind: &'a str,
    /// Which one it is, as the carrier named it (an id).
    pub value: &'a str,
}
/// What a drag looks like to a drop target, with the payload borrowed as for `PayloadRef`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropRef<'a> {
    /// A payload is held over the target or one of its descendants.
    Hover(PayloadRef<'a>),
    /// The payload was released on the target in this frame.
    Dropped(PayloadRef<'a>),
}

/// Touch gestures a node takes (touch only: a mouse or trackpad never starts one).
///
/// A `Copy` value built from `GestureOpts::DEFAULT` (no gesture) with const builders.
///
/// ```
/// use pxui::{DismissOpts, GestureOpts};
/// const ROW: GestureOpts = GestureOpts::DEFAULT.long_press().row_swipe(72.0).dismiss(DismissOpts::DEFAULT.fraction(0.25));
/// assert_ne!(ROW, GestureOpts::DEFAULT);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GestureOpts {
    long_press: bool,
    row_swipe: Option<f32>,
    edge_back: bool,
    refresh: Option<f32>,
    swipe_actions: Option<SwipeActions>,
    dismiss: Option<DismissOpts>,
    touch_drag: bool,
}

impl GestureOpts {
    /// No gesture.
    pub const DEFAULT: GestureOpts = GestureOpts { long_press: false, row_swipe: None, edge_back: false, refresh: None, swipe_actions: None, dismiss: None, touch_drag: false };
    /// A delayed highlight after 0.1 s and a long press after 0.5 s, within 10 pt of the touch-down.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{DismissOpts, GestureOpts, SwipeActions};
    /// let _ = GestureOpts::DEFAULT.long_press();
    /// ```
    pub const fn long_press(self) -> GestureOpts { GestureOpts { long_press: true, ..self } }
    /// A horizontal row swipe revealing actions `width` points wide (8 pt axis lock; vertical goes to scroll).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{DismissOpts, GestureOpts, SwipeActions};
    /// let _ = GestureOpts::DEFAULT.row_swipe(72.0);
    /// ```
    pub const fn row_swipe(self, width: f32) -> GestureOpts { GestureOpts { row_swipe: Some(width), ..self } }
    /// An interactive back from the leading 20 pt of the node.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{DismissOpts, GestureOpts, SwipeActions};
    /// let _ = GestureOpts::DEFAULT.edge_back();
    /// ```
    pub const fn edge_back(self) -> GestureOpts { GestureOpts { edge_back: true, ..self } }
    /// Pull to refresh on a scroll node at UIKit's pull distance (60 points).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::GestureOpts;
    /// let _ = GestureOpts::DEFAULT.pull_to_refresh();
    /// ```
    pub const fn pull_to_refresh(self) -> GestureOpts { GestureOpts { refresh: Some(0.0), ..self } }
    /// Pull to refresh on a scroll node: the pull distance in `points` that fires it.
    /// Allocates: never.
    /// Panics: when `points` is not finite and above 0 (`pull_to_refresh` is the default distance).
    ///
    /// ```
    /// use pxui::GestureOpts;
    /// let _ = GestureOpts::DEFAULT.refresh(48.0);
    /// ```
    pub const fn refresh(self, points: f32) -> GestureOpts {
        assert!(points > 0.0 && points < f32::INFINITY, "pxui: the refresh distance must be finite and above 0");
        GestureOpts { refresh: Some(points), ..self }
    }
    /// A row swipe with actions on both sides and a full swipe (UIKit swipeActions); replaces `row_swipe`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{DismissOpts, GestureOpts, SwipeActions};
    /// let _ = GestureOpts::DEFAULT.swipe_actions(SwipeActions::DEFAULT.trailing(80.0));
    /// ```
    pub const fn swipe_actions(self, actions: SwipeActions) -> GestureOpts { GestureOpts { swipe_actions: Some(actions), ..self } }
    /// A vertical drag-to-dismiss for a full-screen player or bottom sheet.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{DismissOpts, GestureOpts, SwipeActions};
    /// let _ = GestureOpts::DEFAULT.dismiss(DismissOpts::DEFAULT);
    /// ```
    pub const fn dismiss(self, dismiss: DismissOpts) -> GestureOpts { GestureOpts { dismiss: Some(dismiss), ..self } }
    /// Takes the touch at once as a held press and drag (an index strip, a scrubber): reported like a held mouse button, never scrolling an enclosing scroll view.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{DismissOpts, GestureOpts, SwipeActions};
    /// let _ = GestureOpts::DEFAULT.touch_drag();
    /// ```
    pub const fn touch_drag(self) -> GestureOpts { GestureOpts { touch_drag: true, ..self } }
}
impl Default for GestureOpts {
    fn default() -> GestureOpts { GestureOpts::DEFAULT }
}

/// Drag-to-dismiss thresholds; a threshold left unset takes the UIKit sheet's (a quarter of the node height, 600 points/s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DismissOpts {
    fraction: Option<f32>,
    velocity: Option<f32>,
}

impl DismissOpts {
    /// The UIKit sheet's thresholds.
    pub const DEFAULT: DismissOpts = DismissOpts { fraction: None, velocity: None };
    /// The fraction of the node height a release past commits.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{DismissOpts, GestureOpts, SwipeActions};
    /// let _ = DismissOpts::DEFAULT.fraction(0.25);
    /// ```
    pub const fn fraction(self, fraction: f32) -> DismissOpts { DismissOpts { fraction: Some(fraction), ..self } }
    /// The downward release speed in points per second that commits.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{DismissOpts, GestureOpts, SwipeActions};
    /// let _ = DismissOpts::DEFAULT.velocity(600.0);
    /// ```
    pub const fn velocity(self, velocity: f32) -> DismissOpts { DismissOpts { velocity: Some(velocity), ..self } }
}
impl Default for DismissOpts {
    fn default() -> DismissOpts { DismissOpts::DEFAULT }
}

/// Row-swipe actions in points; a side left unset has no actions, and leading and trailing follow the layout direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwipeActions {
    leading: Option<f32>,
    trailing: Option<f32>,
    full: Option<f32>,
}

impl SwipeActions {
    /// No actions on either side and no full swipe.
    pub const DEFAULT: SwipeActions = SwipeActions { leading: None, trailing: None, full: None };
    /// Actions `width` points wide on the leading side.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{DismissOpts, GestureOpts, SwipeActions};
    /// let _ = SwipeActions::DEFAULT.leading(60.0);
    /// ```
    pub const fn leading(self, width: f32) -> SwipeActions { SwipeActions { leading: Some(width), ..self } }
    /// Actions `width` points wide on the trailing side.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{DismissOpts, GestureOpts, SwipeActions};
    /// let _ = SwipeActions::DEFAULT.trailing(80.0);
    /// ```
    pub const fn trailing(self, width: f32) -> SwipeActions { SwipeActions { trailing: Some(width), ..self } }
    /// The finger travel in points that commits the edge action on release; unset: no full swipe.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{DismissOpts, GestureOpts, SwipeActions};
    /// let _ = SwipeActions::DEFAULT.full(120.0);
    /// ```
    pub const fn full(self, travel: f32) -> SwipeActions { SwipeActions { full: Some(travel), ..self } }
}
impl Default for SwipeActions {
    fn default() -> SwipeActions { SwipeActions::DEFAULT }
}

/// What the gestures did to a node. The `*_fired`/`long_pressed`/`back_*` bits last one frame; read
/// before layout, scroll-derived `pull` is the previous frame's painted value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GestureState {
    /// contact held past the highlight delay, or a quick tap's one-frame flash
    pub highlighted: bool,
    /// the contact held past 0.5 s without moving (one frame)
    pub long_pressed: bool,
    /// a row swipe is under the finger or settling
    pub swiping: bool,
    /// the row rests open
    pub swipe_open: bool,
    /// the leading side's actions rest open (with `swipe_open`)
    pub swipe_open_leading: bool,
    /// the trailing side's actions rest open (with `swipe_open`)
    pub swipe_open_trailing: bool,
    /// a full swipe committed the leading edge action on release (one frame; the row settles closed)
    pub swipe_commit_leading: bool,
    /// a full swipe committed the trailing edge action on release (one frame; the row settles closed)
    pub swipe_commit_trailing: bool,
    /// signed shift for the swiped row's content: positive reveals the actions on its left, negative on its right (the physical side; leading is left in left-to-right)
    pub swipe: f32,
    /// an edge back is under the finger or settling
    pub back_active: bool,
    /// the back finished committing: pop the screen now (`back` is already 0)
    pub back_committed: bool,
    /// the back was cancelled and sprang back (one frame)
    pub back_cancelled: bool,
    /// back progress 0..1 while the finger drags or the commit/cancel settles
    pub back: f32,
    /// the pull crossed the threshold and was released: start loading, `refreshing` holds until `end_refresh`
    pub refresh_fired: bool,
    /// a pull to refresh is held, from the release until `end_refresh`
    pub refreshing: bool,
    /// points the content is pulled down past the top
    pub pull: f32,
    /// a drag-to-dismiss is under the finger or settling
    pub dismissing: bool,
    /// the release dismissed (this frame): close now; `dismiss` settles to the node height
    pub dismiss_committed: bool,
    /// the release sprang back (this frame)
    pub dismiss_cancelled: bool,
    /// points to translate the node down by (negative while rubber-banding upward)
    pub dismiss: f32,
}

impl Ui {
    fn gesture_call(&mut self, node: &PxuiBox, request: Option<pxui_types::ffi::PxuiGestureIn>, commands: u32) -> GestureState {
        let mut out = pxui_types::ffi::PxuiGestureOut::default();
        let request_ptr = request.as_ref().map_or(core::ptr::null(), |r| r as *const _);
        crate::live(self, unsafe { lib::pxui_ui_gesture(self.0.as_ptr(), node, request_ptr, commands, &mut out) });
        let f = out.flags;
        GestureState {
            highlighted: f & 1 != 0, long_pressed: f & 2 != 0, swiping: f & 4 != 0, swipe_open: f & 8 != 0, swipe: out.swipe,
            back_active: f & 16 != 0, back_committed: f & 32 != 0, back_cancelled: f & 64 != 0, back: out.back,
            refresh_fired: f & 128 != 0, refreshing: f & 256 != 0, pull: out.pull,
            dismissing: false, dismiss_committed: false, dismiss_cancelled: false, dismiss: 0.0,
            swipe_commit_leading: f & 512 != 0, swipe_commit_trailing: f & 1024 != 0, swipe_open_leading: f & 2048 != 0, swipe_open_trailing: f & 4096 != 0,
        }
    }
    /// Takes the touch gestures `opts` on `node` (long press, row swipe, edge back, pull to refresh, swipe actions, drag to dismiss) and returns what they did to it.
    /// Call it every build; what `opts` leaves out is not taken. Gestures are touch only: a mouse or trackpad never starts one. The one-frame bits of `GestureState` (`long_pressed`, `*_committed`, `refresh_fired`, `*_cancelled`) read true only in the frame they happen. A stale handle reads as rest (every field zero or false).
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`, or a non-finite width or threshold in `opts`).
    ///
    /// ```
    /// use pxui::{ui::Size, GestureOpts, Node};
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let row = ui.node("row".into(), Node { h: Size::Px(44.0), ..Node::DEFAULT });
    ///     let state = ui.gesture(&row, GestureOpts::DEFAULT.long_press().row_swipe(80.0));
    ///     assert!(!state.long_pressed && !state.swiping);
    /// });
    /// ```
    #[track_caller]
    pub fn gesture(&mut self, node: &PxuiBox, opts: GestureOpts) -> GestureState {
        let kinds = u32::from(opts.long_press) | u32::from(opts.row_swipe.is_some()) << 1 | u32::from(opts.edge_back) << 2 | u32::from(opts.refresh.is_some()) << 3 | u32::from(opts.touch_drag) << 5;
        // the C record reads a 0 threshold as the default (UIKit's 60 points)
        let request = pxui_types::ffi::PxuiGestureIn { kinds, reveal: opts.row_swipe.unwrap_or(0.0), threshold: opts.refresh.unwrap_or(0.0), ..Default::default() };
        let state = self.gesture_call(node, Some(request), 0);
        if let Some(a) = opts.swipe_actions {
            let swipe = pxui_types::ffi::PxuiSwipeIn { leading: a.leading.unwrap_or(0.0), trailing: a.trailing.unwrap_or(0.0), full: a.full.unwrap_or(0.0), ..Default::default() };
            crate::live(self, unsafe { lib::pxui_ui_swipe_actions(self.0.as_ptr(), node, &swipe) });
        }
        let Some(d) = opts.dismiss else { return state };
        let (request, mut out) = (pxui_types::ffi::PxuiDismissIn { fraction: d.fraction.unwrap_or(0.0), velocity: d.velocity.unwrap_or(0.0), ..Default::default() }, pxui_types::ffi::PxuiDismissOut::default());
        crate::live(self, unsafe { lib::pxui_ui_dismiss(self.0.as_ptr(), node, &request, &mut out) });
        GestureState { dismissing: out.flags & 8192 != 0, dismiss_committed: out.flags & 16384 != 0, dismiss_cancelled: out.flags & 32768 != 0, dismiss: out.offset, ..state }
    }
    /// Reads `node`'s gesture state without changing which gestures it takes; for code that runs before or apart from the `gesture` call.
    /// A stale handle reads as rest.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let row = ui.node("row".into(), pxui::Node::DEFAULT);
    ///     assert!(!ui.gesture_state(&row).swipe_open);
    /// });
    /// ```
    #[track_caller]
    pub fn gesture_state(&mut self, node: &PxuiBox) -> GestureState { self.gesture_call(node, None, 0) }
    /// Closes a swiped-open row `node`: the content slides back to rest.
    /// A row that is not open, or a stale handle, does nothing.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let row = ui.node("row".into(), pxui::Node::DEFAULT);
    ///     ui.close_swipe(&row);
    /// });
    /// ```
    #[track_caller]
    pub fn close_swipe(&mut self, node: &PxuiBox) { self.gesture_call(node, None, 1); }
    /// Ends a held pull to refresh on the scroll node `node`: `refreshing` drops and the content returns to the top.
    /// Call it when the load that `refresh_fired` started has finished. A node that is not refreshing, or a stale handle, does nothing.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let list = ui.node("list".into(), pxui::Node::DEFAULT);
    ///     ui.end_refresh(&list);
    /// });
    /// ```
    #[track_caller]
    pub fn end_refresh(&mut self, node: &PxuiBox) { self.gesture_call(node, None, 2); }
}

impl Ui {
    /// Declares a box under the current parent by `label` and returns its handle, valid this frame; children go in with `within`.
    ///
    /// `opts` sets its flags, size, axis, padding, scroll step and optional free position and hit shape (see `BoxOpts`).
    /// Allocates: never on a warm frame; a new identity takes a slot the first time, and a `hit` closure takes a pooled box once per call.
    /// Identity: `label` hashes under the parent's seed as `key_of` does; two boxes with one label in one parent collide and the second takes a `#dup` key, so label boxes that can reorder by something unique.
    /// Panics: when the library refuses the box (outside a frame, an invalid size or a `label` over the text limit). A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// use pxui::ui::BoxOpts;
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let panel = ui.box_("panel", BoxOpts::default().padding(4.0));
    ///     ui.within(&panel, &mut |ui| { ui.label("inside", pxui::Label::DEFAULT); });
    /// });
    /// ```
    #[track_caller]
    pub fn box_(&mut self, label: &str, opts: BoxOpts) -> PxuiBox {
        let (hit, hit_user, hit_release_fn) = match &opts.hit {
            Some(hit) => (Some(hit_call as extern "C" fn(*mut c_void, Rect) -> Rect), hit_box(hit.clone()), Some(hit_release as extern "C" fn(*mut c_void))),
            None => (None, core::ptr::null_mut(), None),
        };
        let desc = PxuiBoxDesc {
            size: core::mem::size_of::<PxuiBoxDesc>() as u32, axis: opts.axis as u32, flags: opts.flags.0, w: size(opts.w), h: size(opts.h),
            max_h: opts.max_h, padding: opts.padding, scroll_step: opts.scroll_step, has_at: opts.at.is_some() as u8,
            has_hit_rect: opts.hit_rect.is_some() as u8, reserved: [0; 2], at: opts.at.map_or([0.0; 2], |(x, y)| [x, y]),
            hit_rect: opts.hit_rect.unwrap_or_default(), hit, hit_user, hit_release: hit_release_fn,
        };
        let mut out = PxuiBox::default();
        check(self, unsafe { lib::pxui_ui_box(self.0.as_ptr(), as_str(label), &desc, &mut out) });
        out
    }
    /// `box_` with the label written by `format_args!`, so a per-row label needs no `String`.
    ///
    /// The label is formatted into a 128-byte stack buffer; a longer one goes to the heap.
    /// Allocates: nothing for a label up to 128 bytes, one `String` for a longer one; otherwise as `box_`.
    /// Identity: as `box_` for the formatted text: `format_args!("row {}", id)` keys the box by `id`.
    /// Panics: as `box_`.
    ///
    /// ```
    /// use pxui::ui::BoxOpts;
    /// pxui::doc::frames(1, &mut |ui| {
    ///     for id in 0..3 { ui.box_fmt(format_args!("row {id}"), BoxOpts::default()); }
    /// });
    /// ```
    #[track_caller]
    pub fn box_fmt(&mut self, label: Arguments, opts: BoxOpts) -> PxuiBox { self.box_(Label::new(label).as_str(), opts) }
    /// Between builds, gives `b` the look `value` and repaints only its instances; true when the change is applied or held for the next paint, false for a stale handle or when it must wait for the next frame.
    ///
    /// Inside a build the look is recorded and the frame paints it; the owner's next declaration of the node wins over a patch.
    /// Allocates: never when the node's instance count is unchanged; the draw layer may grow when it changes.
    /// Panics: when the library refuses the call (a null `Ui` or a look with a non-finite number).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     assert!(ui.patch_look(&b, &pxui::ui::Look::NONE));
    /// });
    /// ```
    #[inline] #[track_caller] pub fn set_at(&mut self, b: &PxuiBox, at: (f32, f32)) { check(self, unsafe { lib::pxui_ui_set_at(self.0.as_ptr(), b, at.0, at.1) }) }
    #[track_caller]
    pub fn patch_look(&mut self, b: &PxuiBox, value: &Look) -> bool {
        let mut out = 0;
        let rc = unsafe { lib::pxui_ui_patch_look(self.0.as_ptr(), b, &look(*value), &mut out) };
        crate::live(self, rc) && out != 0
    }
    /// Between builds, moves and resizes `b`'s look and text to `value`, a rectangle in the space `local_rect` reports; true as for `patch_look`.
    ///
    /// Children, canvases and the hit area stay where the last frame put them until the next frame.
    /// Allocates: as `patch_look`.
    /// Panics: when the library refuses the call (a null `Ui` or a non-finite rectangle).
    ///
    /// ```
    /// use pxui::ui::Rect;
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     assert!(ui.patch_rect(&b, Rect::new(0.0, 0.0, 10.0, 10.0)));
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn patch_rect(&mut self, b: &PxuiBox, value: Rect) -> bool {
        let mut out = 0;
        let rc = unsafe { lib::pxui_ui_patch_rect(self.0.as_ptr(), b, value, &mut out) };
        crate::live(self, rc) && out != 0
    }
    /// Between builds, gives `b` the mark `value` (a chevron's direction, a line's rising edge) and repaints it; true as for `patch_look`.
    /// Allocates: as `patch_look`.
    /// Panics: when the library refuses the call (a null `Ui` or a non-finite line width).
    ///
    /// ```
    /// use pxui::ui::{Dir, Mark};
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     assert!(ui.patch_mark(&b, Mark::Chevron(Dir::Down)));
    /// });
    /// ```
    #[track_caller]
    pub fn patch_mark(&mut self, b: &PxuiBox, value: Mark) -> bool {
        let mut out = 0;
        let rc = unsafe { lib::pxui_ui_patch_mark(self.0.as_ptr(), b, &mark(value), &mut out) };
        crate::live(self, rc) && out != 0
    }
    /// The rectangle of `b` as its view lays it out, in the space `patch_rect` takes (`Ui::rect` is where the window shows it); empty for a stale handle.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// let mut b = None;
    /// let ui = pxui::doc::frames(2, &mut |ui| b = Some(ui.node("n".into(), pxui::Node { w: pxui::ui::Size::Px(30.0), h: pxui::ui::Size::Px(10.0), ..pxui::Node::DEFAULT })));
    /// let r = ui.local_rect(&b.unwrap());
    /// assert_eq!((r.w, r.h), (30.0, 10.0));
    /// ```
    #[inline]
    #[track_caller]
    pub fn local_rect(&self, b: &PxuiBox) -> Rect {
        let mut out = Rect::default();
        let rc = unsafe { lib::pxui_ui_local_rect(self.0.as_ptr(), b, &mut out) };
        if !crate::live(self, rc) { return Rect::default(); }
        out
    }
    /// The cumulative number of text layouts the cache had to compute, for warm-up checks; an unchanged layout does not advance it.
    /// Read between frames. It counts layout work, not native allocations or glyph rasterization.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("warm", pxui::Label::DEFAULT); });
    /// let warm = ui.text_layout_count();
    /// pxui::testing::frame(&mut ui, &[], &mut |ui| { ui.label("warm", pxui::Label::DEFAULT); });
    /// assert_eq!(ui.text_layout_count(), warm);
    /// ```
    #[track_caller]
    pub fn text_layout_count(&self) -> u64 {
        let mut out=0;
        check(self, unsafe { lib::pxui_ui_text_layout_count(self.0.as_ptr(), &mut out) });
        out
    }
    /// Scrolls the list `list` by the least amount that shows item (or grid cell) `index`.
    /// Unvisited measured rows use the list's estimates and settle once measured. A stale handle or an index out of range does nothing.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     ui.reveal(&b, 7); // not a list: nothing to scroll
    /// });
    /// ```
    #[track_caller]
    pub fn reveal(&mut self, list: &PxuiBox, index: usize) {
        crate::live(self, unsafe { lib::pxui_ui_reveal(self.0.as_ptr(), list, index) });
    }
    /// The scroll offset of `b` in points: where it has settled and where it is painted now.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui` or a stale handle).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.scroll_view("s", pxui::ui::Size::Px(100.0), pxui::ui::Size::Px(100.0), |_| {});
    ///     assert_eq!(ui.scroll(&b), pxui::ui::Scroll { settled: 0.0, painted: 0.0 });
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn scroll(&self, b: &PxuiBox) -> Scroll {
        let (mut settled, mut painted) = (0.0, 0.0);
        check(self, unsafe { lib::pxui_ui_scroll_offset(self.0.as_ptr(), b, 0, &mut settled) });
        check(self, unsafe { lib::pxui_ui_scroll_offset(self.0.as_ptr(), b, 1, &mut painted) });
        Scroll { settled, painted }
    }
    #[inline] #[track_caller] pub fn set_scroll_offset(&mut self, b: &PxuiBox, value: f32) { check(self, unsafe { lib::pxui_ui_set_scroll_offset(self.0.as_ptr(), b, value) }) }
    /// The text kept in the box's slot, or none when it holds none; borrowed from the library, valid until the
    /// next call on `ui`.
    ///
    /// Allocates: nothing in the wrapper.
    /// Panics: when the `Ui` has failed, like every `Ui` call.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new().unwrap();
    /// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     assert_eq!(ui.text_state(&b), None);
    ///     ui.set_text_state(&b, Some("kept".into()));
    ///     assert_eq!(ui.text_state(&b), Some("kept"));
    /// }).unwrap();
    /// ```
    #[track_caller]
    pub fn text_state(&self, b: &PxuiBox) -> Option<&str> {
        let (mut present, mut out) = (0, Str::default());
        check(self, unsafe { lib::pxui_ui_text_state(self.0.as_ptr(), b, &mut present, &mut out) });
        (present != 0).then(|| crate::lent_str(out))
    }
    /// Keeps `value` in `b`'s text slot (`None` clears it); it lives as long as the node and shows through `text_state`.
    /// Allocates: the library copies `value` into the slot when it is longer than what the slot held.
    /// Panics: when the library refuses the call (a stale handle or text over the limit).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     ui.set_text_state(&b, Some("draft"));
    ///     assert_eq!(ui.text_state(&b), Some("draft"));
    ///     ui.set_text_state(&b, None);
    ///     assert_eq!(ui.text_state(&b), None);
    /// });
    /// ```
    #[track_caller]
    pub fn set_text_state(&mut self, b: &PxuiBox, value: Option<&str>) {
        let text = value.unwrap_or("");
        check(self, unsafe { lib::pxui_ui_set_text_state(self.0.as_ptr(), b, value.is_some() as u8, as_text(text)) })
    }
    /// The text kept in `b`'s slot number `index`, replaced by `text` when it differs: stable text for paint callbacks without a per-frame `String`.
    ///
    /// Borrowed from the library until the next call on this box or the next frame. The slots follow the node and go with it.
    /// Allocates: when `text` differs from the kept text (a new string); never for an unchanged one.
    /// Panics: when the library refuses the call (a stale handle or text over the limit). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     assert_eq!(ui.cached_label(&b, 0, "12 songs"), "12 songs");
    /// });
    /// ```
    #[track_caller]
    pub fn cached_label(&mut self, b: &PxuiBox, index: usize, text: &str) -> &str {
        let mut out = Str::default();
        check(self, unsafe { lib::pxui_ui_cached_label(self.0.as_ptr(), b, index as u64, as_str(text), &mut out) });
        if out.len == 0 { return ""; }
        unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(out.ptr, out.len)) }
    }
    #[inline] #[track_caller] pub fn to_front(&mut self, order: i64, b: &PxuiBox) { check(self, unsafe { lib::pxui_ui_to_front(self.0.as_ptr(), order, b) }) }

    /// The payload a drag or key carry is holding, borrowed from the library until the next mutable call on the `Ui`, or `None` when nothing is carried.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     assert!(ui.carrying().is_none());
    ///     ui.carry(None, "song", "42");
    ///     let held = ui.carrying().unwrap();
    ///     assert_eq!((held.kind, held.value), ("song", "42"));
    /// });
    /// ```
    #[track_caller]
    pub fn carrying(&self) -> Option<PayloadRef<'_>> {
        let (mut present, mut kind, mut value) = (0, Str::default(), Str::default());
        check(self, unsafe { lib::pxui_ui_carrying(self.0.as_ptr(), &mut present, &mut kind, &mut value) });
        (present != 0).then(|| PayloadRef { kind: crate::lent_str(kind), value: crate::lent_str(value) })
    }
    /// Holds a payload of `kind` and `value` for a drag or a key-driven move; drop targets read it with `drop_target` and everyone with `carrying`.
    /// With `from` (the box that owns the pressed left button) the payload is held once the pointer has moved 4 points from the press, so a widget calls it every frame while pressed. With `None` (a key) it is held at once, with no capture, and `cancel_carry` ends it. A new payload replaces the one held.
    /// Allocates: copies `kind` and `value` into storage the `Ui` reuses between carries; allocates only when a string outgrows it.
    /// Panics: when the library refuses the call (a stale `from`, or text over the limit). A key or identity label over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     ui.carry(None, "row", "7");
    ///     assert!(ui.carrying().is_some());
    ///     ui.cancel_carry();
    ///     assert!(ui.carrying().is_none());
    /// });
    /// ```
    #[track_caller]
    pub fn carry(&mut self, from: Option<&PxuiBox>, kind: &str, value: &str) {
        let raw = from.map_or(core::ptr::null(), |b| b as *const PxuiBox);
        check(self, unsafe { lib::pxui_ui_carry(self.0.as_ptr(), from.is_some() as u8, raw, as_str(kind), as_str(value)) })
    }
    /// Moves an active pointer carry's capture to `to`, a live hit ancestor of the box that started it, so a virtualized source row can scroll away without ending the drag.
    /// Scroll containers follow the pointer at their edges. Removing `to`, losing the capture or cancelling still ends the carry. False for a key carry, a stale or unrelated `to`, or no pointer carry.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let list = ui.node("list".into(), pxui::Node::DEFAULT);
    ///     assert!(!ui.transfer_carry(&list)); // nothing is being carried
    /// });
    /// ```
    #[track_caller]
    pub fn transfer_carry(&mut self, to: &PxuiBox) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_transfer_carry(self.0.as_ptr(), to, &mut out) });
        out != 0
    }
    /// Ends a held carry: `carrying` reads `None` from now on and the drop targets see no hover.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     ui.carry(None, "row", "7");
    ///     ui.cancel_carry();
    ///     assert!(ui.carrying().is_none());
    /// });
    /// ```
    #[inline] #[track_caller] pub fn cancel_carry(&mut self) { check(self, unsafe { lib::pxui_ui_cancel_carry(self.0.as_ptr()) }) }
    /// What a drag looks like to the drop target `b`: `Hover` while a payload is held over it or one of its descendants, `Dropped` in the frame the payload was released there, `None` otherwise.
    /// A drop stays readable by the target and its ancestors for the whole frame it happened in; the payload is borrowed from the library until the next mutable call on the `Ui`, so no string is copied.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let slot = ui.node("slot".into(), pxui::Node::DEFAULT);
    ///     assert!(ui.drop_target(&slot).is_none());
    /// });
    /// ```
    #[track_caller]
    pub fn drop_target(&mut self, b: &PxuiBox) -> Option<DropRef<'_>> {
        let (mut tag, mut kind, mut value) = (0, Str::default(), Str::default());
        check(self, unsafe { lib::pxui_ui_drop_target(self.0.as_ptr(), b, &mut tag, &mut kind, &mut value) });
        let payload = || PayloadRef { kind: crate::lent_str(kind), value: crate::lent_str(value) };
        match tag { 1 => Some(DropRef::Hover(payload())), 2 => Some(DropRef::Dropped(payload())), _ => None }
    }
    /// Where the right button was released in `b` or a descendant this frame, in window points, or `None`.
    /// Reads through the shared capture, so a press that began in `b` and was released over it counts. It reports the release, not the press; `context_clicked` also checks that the pointer did not travel between them.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("b".into(), pxui::Node::DEFAULT);
    ///     assert_eq!(ui.context_at(&b), None);
    /// });
    /// ```
    #[track_caller]
    pub fn context_at(&mut self, b: &PxuiBox) -> Option<(f32, f32)> {
        let (mut present, mut out) = (0, Point::default());
        check(self, unsafe { lib::pxui_ui_context_at(self.0.as_ptr(), b, &mut present, &mut out) });
        (present != 0).then_some((out.x, out.y))
    }
    /// Whether `b` itself, none of its children, is the topmost box under the pointer; always false while the last input was a touch.
    /// Reads the hover the frame's input routing settled; the answer is for this frame. A stale handle reads false.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// use pxui::{ui::Size, Event, Node};
    /// let mut b = None;
    /// let mut build = |ui: &mut pxui::Ui| b = Some(ui.node("n".into(), Node { flags: pxui::ui::Flags::CLICKABLE, w: Size::Px(50.0), h: Size::Px(50.0), ..Node::DEFAULT }));
    /// let mut ui = pxui::doc::frames(2, &mut build);
    /// let moved = [Event { kind: Event::POINTER_MOVED, x: 10.0, y: 10.0, ..Default::default() }];
    /// pxui::testing::frame(&mut ui, &moved, &mut build);
    /// assert!(ui.hovered_exactly(&b.unwrap()));
    /// ```
    #[inline]
    #[track_caller]
    pub fn hovered_exactly(&self, b: &PxuiBox) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_hovered(self.0.as_ptr(), b, 0, &mut out) });
        out != 0
    }
    /// Whether `b` or one of its hit descendants owns the pointer's hover in the shared hit tree.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// use pxui::{ui::Size, Event, Node};
    /// let mut b = None;
    /// let mut build = |ui: &mut pxui::Ui| b = Some(ui.node("n".into(), Node { flags: pxui::ui::Flags::CLICKABLE, w: Size::Px(50.0), h: Size::Px(50.0), ..Node::DEFAULT }));
    /// let mut ui = pxui::doc::frames(2, &mut build);
    /// let moved = [Event { kind: Event::POINTER_MOVED, x: 10.0, y: 10.0, ..Default::default() }];
    /// pxui::testing::frame(&mut ui, &moved, &mut build);
    /// assert!(ui.hovered_within(&b.unwrap()));
    /// ```
    #[inline]
    #[track_caller]
    pub fn hovered_within(&mut self, b: &PxuiBox) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_hovered(self.0.as_ptr(), b, 1, &mut out) });
        out != 0
    }
    /// The key codes held at the box's captured press, retained through release; borrowed from the
    /// library, valid until the next call on `ui`.
    ///
    /// Allocates: nothing.
    /// Panics: when the `Ui` has failed, like every `Ui` call.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new().unwrap();
    /// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     assert!(ui.press_keys(&b).is_empty());
    /// }).unwrap();
    /// ```
    #[track_caller]
    pub fn press_keys(&self, b: &PxuiBox) -> &[u32] {
        let (mut ptr, mut len) = (core::ptr::null(), 0);
        check(self, unsafe { lib::pxui_ui_press_keys(self.0.as_ptr(), b, &mut ptr, &mut len) });
        if len == 0 || ptr.is_null() { &[] } else { unsafe { core::slice::from_raw_parts(ptr, len) } }
    }
    /// The first of `roots` (box keys) that is the topmost box under `point`, or one of its ancestors, in the previous frame's hit tree; `None` when none is.
    /// Window points; call it while handling input, before this frame's tree replaces the last one.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// let mut key = 0;
    /// let ui = pxui::doc::frames(2, &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node { flags: pxui::ui::Flags::CLICKABLE, w: pxui::ui::Size::Px(50.0), h: pxui::ui::Size::Px(50.0), ..pxui::Node::DEFAULT });
    ///     key = b.box_key;
    /// });
    /// assert_eq!(ui.root_at((10.0, 10.0), &[key]), Some(key));
    /// assert_eq!(ui.root_at((200.0, 200.0), &[key]), None);
    /// ```
    #[track_caller]
    pub fn root_at(&self, point: (f32, f32), roots: &[i64]) -> Option<i64> {
        let (mut present, mut out) = (0, 0);
        check(self, unsafe { lib::pxui_ui_root_at(self.0.as_ptr(), point.0, point.1, Slice { ptr: roots.as_ptr(), len: roots.len() }, &mut present, &mut out) });
        (present != 0).then_some(out)
    }
    /// `view` that takes no painter table entry of its own: its instances ride the parent's, so many small views that move with their parent (one per list item) cost no table row each.
    ///
    /// `revision` and `build` behave as for `view`: the view's build is kept while `revision` is unchanged. `view_offset` and the other table changes on it take effect by a rebuild instead.
    /// Allocates: never on a warm frame; a new identity takes a slot the first time.
    /// Identity: `label` keys the view under its parent as for `box_`; two views with one label in one parent collide.
    /// Panics: when the library refuses the call (outside a frame, an invalid size, or a `label` over the text limit). A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     ui.view_flat("row-7", 1, pxui::ui::Size::Grow, pxui::ui::Size::Px(24.0), &mut |ui| { ui.label("seven", pxui::Label::DEFAULT); });
    /// });
    /// ```
    #[track_caller]
    pub fn view_flat(&mut self, label: &str, revision: u64, w: Size, h: Size, mut build: &mut dyn FnMut(&mut Ui)) -> PxuiBox {
        let mut out = PxuiBox::default();
        check(self, unsafe { lib::pxui_ui_view_flat(self.0.as_ptr(), as_str(label), revision, size(w), size(h), Some(call::<&mut dyn FnMut(&mut Ui)>), &mut build as *mut _ as *mut c_void, &mut out) });
        out
    }
    /// The last of `roots` (box keys) that a pointer press among `events` landed on or under, by the previous frame's hit tree; `None` when no press hit one.
    /// Window points; call it before the frame that takes `events` replaces the tree, to see which panel a press belongs to.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// use pxui::{ui::Size, Event, Node};
    /// let mut key = 0;
    /// let ui = pxui::doc::frames(2, &mut |ui| key = ui.node("n".into(), Node { flags: pxui::ui::Flags::CLICKABLE, w: Size::Px(50.0), h: Size::Px(50.0), ..Node::DEFAULT }).box_key);
    /// let press = |x| [Event { kind: Event::POINTER_PRESSED, x, y: 10.0, ..Default::default() }];
    /// assert_eq!(ui.last_press_within(&press(10.0), &[key]), Some(key));
    /// assert_eq!(ui.last_press_within(&press(200.0), &[key]), None);
    /// ```
    #[track_caller]
    pub fn last_press_within(&self, events: &[pxui_types::Event], roots: &[i64]) -> Option<i64> {
        let mut last = None;
        for event in events.iter().filter(|event| event.kind == pxui_types::Event::POINTER_PRESSED) {
            if let Some(root) = self.root_at((event.x as f32, event.y as f32), roots) { last = Some(root); }
        }
        last
    }
    /// Sets the view `b`'s table entry to `view`: the fields it sets take effect, the ones it leaves unset keep the entry's current value.
    /// One table entry, no rebuild; unchanged values change nothing, and a scale of 1 leaves every point exact.
    /// Every field is checked before any is applied, so a refused call changes nothing.
    /// Allocates: never.
    /// Panics: when an offset, scale or matrix value is not finite, the opacity is outside 0 to 1, or the library refuses the call (a stale or non-view box).
    ///
    /// ```
    /// use pxui::ui::{Size, ViewTransform};
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let v = ui.view("v", 1, Size::Px(40.0), Size::Px(40.0), &mut |_| {});
    ///     ui.set_view(&v, &ViewTransform::DEFAULT.offset(4.0, 0.0).opacity(0.5).about(1.5, [1.0, 0.0, 0.0, 1.0], (20.0, 20.0)));
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn set_view(&mut self, b: &PxuiBox, view: &ViewTransform) {
        let finite = |values: &[f32]| values.iter().all(|v| v.is_finite());
        assert!(view.offset.is_none_or(|(x, y)| finite(&[x, y])), "pxui: the view offset must be finite");
        assert!(view.opacity.is_none_or(|o| (0.0..=1.0).contains(&o)), "pxui: the view opacity must be in 0 to 1");
        assert!(view.scale.is_none_or(|s| finite(&[s])), "pxui: the view scale must be finite");
        assert!(view.about.is_none_or(|a| finite(&[a.scale, a.matrix[0], a.matrix[1], a.matrix[2], a.matrix[3], a.pivot.0, a.pivot.1])), "pxui: the view transform must be finite");
        let ui = self.0.as_ptr();
        if let Some((dx, dy)) = view.offset { check(self, unsafe { lib::pxui_ui_view_offset(ui, b, dx, dy) }) }
        if let Some(opacity) = view.opacity { check(self, unsafe { lib::pxui_ui_view_opacity(ui, b, opacity) }) }
        if let Some(scale) = view.scale { check(self, unsafe { lib::pxui_ui_view_scale(ui, b, scale) }) }
        if let Some(about) = view.about { check(self, unsafe { lib::pxui_ui_view_transform_about(ui, b, about.scale, about.matrix.as_ptr(), about.pivot.0, about.pivot.1) }) }
    }
    /// Clips the view `b` to `clip` in window points, for painting and hit testing (`None` removes the clip).
    /// One table entry, no rebuild; an unchanged clip changes nothing.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a stale or non-view box, or a rectangle that is not finite or has a negative size).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let v = ui.view("v", 1, pxui::ui::Size::Px(40.0), pxui::ui::Size::Px(40.0), &mut |_| {});
    ///     ui.view_clip(&v, Some(pxui::ui::Rect::new(0.0, 0.0, 20.0, 20.0)));
    ///     ui.view_clip(&v, None);
    /// });
    /// ```
    #[inline] #[track_caller] pub fn view_clip(&mut self,b:&PxuiBox,clip:Option<Rect>) { check(self,unsafe { lib::pxui_ui_view_clip(self.0.as_ptr(),b,clip.as_ref().map_or(core::ptr::null(),|r|r)) }) }
    /// Edits what a widget finishes of a node it declared this frame: `edit` gets the node's look, mark and inset, and what it leaves there is written back; returns what `edit` returns.
    /// `edit` must not call the `Ui`. For a stale handle the library refuses.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a stale handle, or a look or inset with a non-finite number).
    ///
    /// ```
    /// use pxui::ui::Mark;
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     let had_mark = ui.node_mut(&b, |fields| { let had = fields.mark != Mark::None; fields.mark = Mark::None; had });
    ///     assert!(!had_mark);
    /// });
    /// ```
    #[track_caller]
    pub fn node_mut<R>(&mut self, b: &PxuiBox, edit: impl FnOnce(&mut NodeFields) -> R) -> R {
        let mut raw = pxui_types::ffi::PxuiNodeFields { size: core::mem::size_of::<pxui_types::ffi::PxuiNodeFields>() as u32, reserved: 0, look: Default::default(), mark: Default::default(), inset: Default::default() };
        check(self, unsafe { lib::pxui_ui_node_fields(self.0.as_ptr(), b, &mut raw) });
        let direction = |d: u32| match d { 0 => Dir::Down, 1 => Dir::Up, 2 => Dir::Left, _ => Dir::Right };
        let mut fields = NodeFields {
            look: raw.look.into(), inset: raw.inset,
            mark: match raw.mark.kind { 1 => Mark::Chevron(direction(raw.mark.direction)), 2 => Mark::Line { width: raw.mark.width, rising: raw.mark.rising != 0 }, _ => Mark::None },
        };
        let result = edit(&mut fields);
        raw.look = fields.look.into();
        raw.mark = mark(fields.mark);
        raw.inset = fields.inset;
        check(self, unsafe { lib::pxui_ui_set_node_fields(self.0.as_ptr(), b, &raw) });
        result
    }
    /// The node `b` as the engine holds it this frame: change its layout fields and give it back to `set_node_layout`.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a stale handle or outside a frame).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node { w: pxui::ui::Size::Px(5.0), ..pxui::Node::DEFAULT });
    ///     assert!(matches!(ui.node_layout(&b).w, pxui::ui::Size::Px(5.0)));
    /// });
    /// ```
    #[track_caller]
    pub fn node_layout(&self, b: &PxuiBox) -> pxui_types::Node {
        let mut out = pxui_types::Node::DEFAULT;
        check(self, unsafe { lib::pxui_ui_node_layout(self.0.as_ptr(), b, &mut out) });
        out
    }
    /// Sets the layout of a node its widget declared this frame, as a body sizes the node it was given.
    ///
    /// Only `node`'s axis, justify, align, wrap, size, minimum and maximum, grow, shrink, aspect, padding, margin, gaps, own alignment and offset are taken; its other fields are ignored (behaviour keeps the flags and role, the look is `node_mut`'s).
    /// Allocates: never.
    /// Panics: when the library refuses the call (a stale handle, or a layout field that is not finite).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     let wide = pxui::Node { w: pxui::ui::Size::Px(80.0), ..ui.node_layout(&b) };
    ///     ui.set_node_layout(&b, &wide);
    ///     assert!(matches!(ui.node_layout(&b).w, pxui::ui::Size::Px(80.0)));
    /// });
    /// ```
    #[track_caller]
    pub fn set_node_layout(&mut self, b: &PxuiBox, node: &pxui_types::Node) {
        check(self, unsafe { lib::pxui_ui_set_node_layout(self.0.as_ptr(), b, node) });
    }
    #[inline] #[track_caller] pub fn view_backdrop_blur(&mut self, b: &PxuiBox, sigma: f32) { check(self, unsafe { lib::pxui_ui_view_backdrop_blur(self.0.as_ptr(), b, sigma) }) }
    /// Blurred backdrop saturation: 0 is grayscale, 1 preserves colour (the default).
    #[inline] #[track_caller] pub fn view_backdrop_saturation(&mut self, b: &PxuiBox, saturation: f32) { check(self, unsafe { lib::pxui_ui_view_backdrop_saturation(self.0.as_ptr(), b, saturation) }) }
}

/// A string the library lent: copied, since it only lives until the next frame.

/// The scroll offset of a box in points, as `Ui::scroll` reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scroll {
    /// Where the scroll has settled, without elastic overscroll.
    pub settled: f32,
    /// Where the content is painted now, elastic overscroll included.
    pub painted: f32,
}

/// What a retained view's table entry applies to its output, for `Ui::set_view`: a `Copy` value built from `ViewTransform::DEFAULT` (nothing set) with const builders.
///
/// A field left unset keeps the entry's current value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransform {
    offset: Option<(f32, f32)>,
    opacity: Option<f32>,
    scale: Option<f32>,
    about: Option<About>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct About {
    scale: f32,
    matrix: [f32; 4],
    pivot: (f32, f32),
}

impl ViewTransform {
    /// Nothing set.
    pub const DEFAULT: ViewTransform = ViewTransform { offset: None, opacity: None, scale: None, about: None };
    /// Shifts the view by `(dx, dy)` points.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ui::ViewTransform;
    /// let _ = ViewTransform::DEFAULT.offset(4.0, 0.0);
    /// ```
    pub const fn offset(self, dx: f32, dy: f32) -> ViewTransform { ViewTransform { offset: Some((dx, dy)), ..self } }
    /// Multiplies each instance's alpha by a finite 0..1 value; overlaps remain per-instance, not group opacity.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ui::ViewTransform;
    /// let _ = ViewTransform::DEFAULT.opacity(0.5);
    /// ```
    pub const fn opacity(self, opacity: f32) -> ViewTransform { ViewTransform { opacity: Some(opacity), ..self } }
    /// A uniform scale about the view box's centre. Zero hides it; negative values reflect both axes.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ui::ViewTransform;
    /// let _ = ViewTransform::DEFAULT.scale(2.0);
    /// ```
    pub const fn scale(self, scale: f32) -> ViewTransform { ViewTransform { scale: Some(scale), ..self } }
    /// Maps the output through `pivot + scale * matrix * (p - pivot)` about the window point `pivot`, which stays put when the view moves.
    /// `matrix` is a row-major 2x2 (`[1.0, 0.0, 0.0, 1.0]` is the identity; rotation, skew and flips go here).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ui::ViewTransform;
    /// let _ = ViewTransform::DEFAULT.about(1.5, [1.0, 0.0, 0.0, 1.0], (20.0, 20.0));
    /// ```
    pub const fn about(self, scale: f32, matrix: [f32; 4], pivot: (f32, f32)) -> ViewTransform { ViewTransform { about: Some(About { scale, matrix, pivot }), ..self } }
}
impl Default for ViewTransform {
    fn default() -> ViewTransform { ViewTransform::DEFAULT }
}

/// What `Ui::node_mut` edits of a node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeFields {
    pub look: Look,
    pub mark: Mark,
    pub inset: pxui_types::geom::Edges,
}
