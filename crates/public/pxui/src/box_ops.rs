//! Boxes and what hangs off them, with the engine's names and shapes (see `ui_state` for how failures surface).
use crate::composite::{call, defer_callback_panic, deferred};
use crate::node::{look, mark, size};
use crate::opts::{BoxOpts, HitFn};
use crate::{as_str, check, Ui};
use core::ffi::c_void;
use pxui_types::{geom::{Point, Rect}, ffi::{PxuiBoxDesc, PxuiLookDesc, PxuiMarkDesc, PxuiSizeDesc, Slice}, theme::Look, Dir, Size, ui::{Drop, Payload}, Mark, PxuiBox, Str};
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

/// A carry query borrowed until the next mutable UI call, without owned string copies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PayloadRef<'a> {
    pub kind: &'a str,
    pub value: &'a str,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropRef<'a> {
    Hover(PayloadRef<'a>),
    Dropped(PayloadRef<'a>),
}

/// Touch gestures a node takes (touch only: a mouse or trackpad never starts one).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GestureOpts {
    /// delayed highlight after 0.1 s and a long press after 0.5 s, within 10 pt of the touch-down
    pub long_press: bool,
    /// horizontal row swipe revealing actions this many points wide (8 pt axis lock; vertical goes to scroll)
    pub row_swipe: Option<f32>,
    /// interactive back from the leading 20 pt of the node
    pub edge_back: bool,
    /// pull to refresh on a scroll node: the pull distance in points (0 picks 60)
    pub refresh: Option<f32>,
    /// row swipe with actions on both sides and a full swipe (UIKit swipeActions); replaces `row_swipe`
    pub swipe_actions: Option<SwipeActions>,
    /// vertical drag-to-dismiss for a full-screen player or bottom sheet
    pub dismiss: Option<DismissOpts>,
    /// take the touch at once as a held press and drag (an index strip, a scrubber): reported like a held mouse button, never scrolling an enclosing scroll view
    pub touch_drag: bool,
}

/// Drag-to-dismiss thresholds; 0 picks the default (UIKit sheet: a quarter of the node height, 600 points/s).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DismissOpts {
    /// fraction of the node height a release past commits
    pub fraction: f32,
    /// downward release speed in points per second that commits
    pub velocity: f32,
}

/// Row-swipe actions in points. A side of 0 has no actions; leading and trailing follow the layout direction.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SwipeActions {
    pub leading: f32,
    pub trailing: f32,
    /// finger travel that commits the edge action on release (0: no full swipe)
    pub full: f32,
}

/// What the gestures did to a node. The `*_fired`/`long_pressed`/`back_*` bits last one frame; read
/// before layout, scroll-derived `pull` is the previous frame's painted value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GestureState {
    /// contact held past the highlight delay, or a quick tap's one-frame flash
    pub highlighted: bool,
    pub long_pressed: bool,
    pub swiping: bool,
    pub swipe_open: bool,
    /// which side rests open (with `swipe_open`), and a full swipe's commit, which lasts one frame; the row settles closed
    pub swipe_open_leading: bool,
    pub swipe_open_trailing: bool,
    pub swipe_commit_leading: bool,
    pub swipe_commit_trailing: bool,
    /// signed shift for the swiped row's content: positive reveals the actions on its left, negative on its right (the physical side; leading is left in left-to-right)
    pub swipe: f32,
    pub back_active: bool,
    /// the back finished committing: pop the screen now (`back` is already 0)
    pub back_committed: bool,
    pub back_cancelled: bool,
    /// back progress 0..1 while the finger drags or the commit/cancel settles
    pub back: f32,
    /// the pull crossed the threshold and was released: start loading, `refreshing` holds until `end_refresh`
    pub refresh_fired: bool,
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
    /// Take these gestures on `node` (call every build) and read what they did. A stale handle reads as rest.
    #[track_caller]
    pub fn gesture(&mut self, node: &PxuiBox, opts: GestureOpts) -> GestureState {
        let kinds = u32::from(opts.long_press) | u32::from(opts.row_swipe.is_some()) << 1 | u32::from(opts.edge_back) << 2 | u32::from(opts.refresh.is_some()) << 3 | u32::from(opts.touch_drag) << 5;
        let request = pxui_types::ffi::PxuiGestureIn { kinds, reveal: opts.row_swipe.unwrap_or(0.0), threshold: opts.refresh.unwrap_or(0.0), ..Default::default() };
        let state = self.gesture_call(node, Some(request), 0);
        if let Some(a) = opts.swipe_actions {
            let swipe = pxui_types::ffi::PxuiSwipeIn { leading: a.leading, trailing: a.trailing, full: a.full, ..Default::default() };
            crate::live(self, unsafe { lib::pxui_ui_swipe_actions(self.0.as_ptr(), node, &swipe) });
        }
        let Some(d) = opts.dismiss else { return state };
        let (request, mut out) = (pxui_types::ffi::PxuiDismissIn { fraction: d.fraction, velocity: d.velocity, ..Default::default() }, pxui_types::ffi::PxuiDismissOut::default());
        crate::live(self, unsafe { lib::pxui_ui_dismiss(self.0.as_ptr(), node, &request, &mut out) });
        GestureState { dismissing: out.flags & 8192 != 0, dismiss_committed: out.flags & 16384 != 0, dismiss_cancelled: out.flags & 32768 != 0, dismiss: out.offset, ..state }
    }
    /// Read a node's gesture state without changing what it takes.
    #[track_caller]
    pub fn gesture_state(&mut self, node: &PxuiBox) -> GestureState { self.gesture_call(node, None, 0) }
    /// Close a swiped-open row.
    #[track_caller]
    pub fn close_swipe(&mut self, node: &PxuiBox) { self.gesture_call(node, None, 1); }
    /// End a held refresh: the content returns to the top.
    #[track_caller]
    pub fn end_refresh(&mut self, node: &PxuiBox) { self.gesture_call(node, None, 2); }
}

impl Ui {
    /// Declares a box under the current parent by `label` and returns its handle, valid this frame; children go in with `within`.
    ///
    /// `opts` sets its flags, size, axis, padding, scroll step and optional free position and hit shape (see `BoxOpts`).
    /// Allocates: never on a warm frame; a new identity takes a slot the first time, and a `hit` closure takes a pooled box once per call.
    /// Identity: `label` hashes under the parent's seed as `key_of` does; two boxes with one label in one parent collide and the second takes a `#dup` key, so label boxes that can reorder by something unique.
    /// Panics: when the library refuses the box (outside a frame, an invalid size or a `label` over the text limit).
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
    /// The rectangle of `b` as its view lays it out, in the space `patch_rect` takes (`rect` is where the window shows it); empty for a stale handle.
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
    /// The settled scroll offset of `b` in points, without elastic overscroll (`scroll_position` is the painted one).
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui` or a stale handle).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.scroll_view("s", pxui::ui::Size::Px(100.0), pxui::ui::Size::Px(100.0), &mut |_| {});
    ///     assert_eq!(ui.scroll_offset(&b), 0.0);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn scroll_offset(&self, b: &PxuiBox) -> f32 {
        let mut out = 0.0;
        check(self, unsafe { lib::pxui_ui_scroll_offset(self.0.as_ptr(), b, 0, &mut out) });
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
    /// The painted scroll offset of `b` in points, elastic overscroll included (`scroll_offset` is the settled one).
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui` or a stale handle).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.scroll_view("s", pxui::ui::Size::Px(100.0), pxui::ui::Size::Px(100.0), &mut |_| {});
    ///     assert_eq!(ui.scroll_position(&b), 0.0);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn scroll_position(&self, b: &PxuiBox) -> f32 {
        let mut out = 0.0;
        check(self, unsafe { lib::pxui_ui_scroll_offset(self.0.as_ptr(), b, 1, &mut out) });
        out
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
        check(self, unsafe { lib::pxui_ui_set_text_state(self.0.as_ptr(), b, value.is_some() as u8, as_str(text)) })
    }
    /// The text kept in `b`'s slot number `index`, replaced by `text` when it differs: stable text for paint callbacks without a per-frame `String`.
    ///
    /// Borrowed from the library until the next call on this box or the next frame. The slots follow the node and go with it.
    /// Allocates: when `text` differs from the kept text (a new string); never for an unchanged one.
    /// Panics: when the library refuses the call (a stale handle or text over the limit).
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

    #[track_caller]
    pub fn carrying(&self) -> Option<Payload> {
        self.carrying_ref().map(|p| Payload { kind: p.kind.to_owned(), value: p.value.to_owned() })
    }
    #[track_caller]
    pub fn carrying_ref(&self) -> Option<PayloadRef<'_>> {
        let (mut present, mut kind, mut value) = (0, Str::default(), Str::default());
        check(self, unsafe { lib::pxui_ui_carrying(self.0.as_ptr(), &mut present, &mut kind, &mut value) });
        (present != 0).then(|| PayloadRef { kind: crate::lent_str(kind), value: crate::lent_str(value) })
    }
    #[track_caller]
    pub fn carry(&mut self, from: Option<&PxuiBox>, kind: &str, value: &str) {
        let raw = from.map_or(core::ptr::null(), |b| b as *const PxuiBox);
        check(self, unsafe { lib::pxui_ui_carry(self.0.as_ptr(), from.is_some() as u8, raw, as_str(kind), as_str(value)) })
    }
    /// Move an active pointer carry's capture to a live hit ancestor, retaining it when virtual
    /// source rows disappear. Removing that ancestor, losing capture or cancellation still ends it.
    /// Scroll containers follow the pointer at their edges. Returns false for key carries,
    /// stale/unrelated targets or no active pointer carry.
    #[track_caller]
    pub fn transfer_carry(&mut self, to: &PxuiBox) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_transfer_carry(self.0.as_ptr(), to, &mut out) });
        out != 0
    }
    #[inline] #[track_caller] pub fn cancel_carry(&mut self) { check(self, unsafe { lib::pxui_ui_cancel_carry(self.0.as_ptr()) }) }
    #[track_caller]
    pub fn drop_target(&mut self, b: &PxuiBox) -> Option<Drop> {
        self.drop_target_ref(b).map(|drop| match drop {
            DropRef::Hover(p) => Drop::Hover(Payload { kind: p.kind.to_owned(), value: p.value.to_owned() }),
            DropRef::Dropped(p) => Drop::Dropped(Payload { kind: p.kind.to_owned(), value: p.value.to_owned() }),
        })
    }
    #[track_caller]
    pub fn drop_target_ref(&mut self, b: &PxuiBox) -> Option<DropRef<'_>> {
        let (mut tag, mut kind, mut value) = (0, Str::default(), Str::default());
        check(self, unsafe { lib::pxui_ui_drop_target(self.0.as_ptr(), b, &mut tag, &mut kind, &mut value) });
        let payload = || PayloadRef { kind: crate::lent_str(kind), value: crate::lent_str(value) };
        match tag { 1 => Some(DropRef::Hover(payload())), 2 => Some(DropRef::Dropped(payload())), _ => None }
    }
    /// The released right-click in this box or a descendant, through shared capture.
    #[track_caller]
    pub fn context_at(&mut self, b: &PxuiBox) -> Option<(f32, f32)> {
        let (mut present, mut out) = (0, Point::default());
        check(self, unsafe { lib::pxui_ui_context_at(self.0.as_ptr(), b, &mut present, &mut out) });
        (present != 0).then_some((out.x, out.y))
    }
    #[inline]
    #[track_caller]
    pub fn hovered_exactly(&self, b: &PxuiBox) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_hovered(self.0.as_ptr(), b, 0, &mut out) });
        out != 0
    }
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
    /// Panics: when the library refuses the call (outside a frame, an invalid size, or a `label` over the text limit).
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
    /// The last root pressed by a pointer press among `events`, by the previous hit tree.
    #[track_caller]
    pub fn last_press_within(&self, events: &[pxui_types::Event], roots: &[i64]) -> Option<i64> {
        let mut last = None;
        for event in events.iter().filter(|event| event.kind == pxui_types::Event::POINTER_PRESSED) {
            if let Some(root) = self.root_at((event.x as f32, event.y as f32), roots) { last = Some(root); }
        }
        last
    }
    #[inline] #[track_caller] pub fn view_offset(&mut self, b: &PxuiBox, dx: f32, dy: f32) { check(self, unsafe { lib::pxui_ui_view_offset(self.0.as_ptr(), b, dx, dy) }) }
    /// Multiply each instance's alpha by a finite 0..1 value; overlaps remain per-instance, not group opacity.
    #[inline] #[track_caller] pub fn view_opacity(&mut self,b:&PxuiBox,opacity:f32) { check(self,unsafe { lib::pxui_ui_view_opacity(self.0.as_ptr(),b,opacity) }) }
    /// Uniform scale about the view box's centre. Zero hides it; negative values reflect both axes.
    #[inline] #[track_caller] pub fn view_scale(&mut self,b:&PxuiBox,scale:f32) { check(self,unsafe { lib::pxui_ui_view_scale(self.0.as_ptr(),b,scale) }) }
    /// Maps the view `b`'s output through `pivot + scale * transform * (p - pivot)` about the window point `pivot`, which stays put when the view moves.
    /// `transform` is a row-major 2x2 matrix (`[1.0, 0.0, 0.0, 1.0]` is the identity; rotation, skew and flips go here); one table entry, no rebuild. Unchanged values change nothing, and a scale of 1 leaves every point exact.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a stale or non-view box, or a non-finite value).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let v = ui.view("v", 1, pxui::ui::Size::Px(40.0), pxui::ui::Size::Px(40.0), &mut |_| {});
    ///     ui.view_transform_about(&v, 1.5, [1.0, 0.0, 0.0, 1.0], (20.0, 20.0));
    /// });
    /// ```
    #[inline] #[track_caller] pub fn view_clip(&mut self,b:&PxuiBox,clip:Option<Rect>) { check(self,unsafe { lib::pxui_ui_view_clip(self.0.as_ptr(),b,clip.as_ref().map_or(core::ptr::null(),|r|r)) }) }
    #[inline]
    #[track_caller]
    pub fn view_transform_about(&mut self, b: &PxuiBox, scale: f32, transform: [f32; 4], pivot: (f32, f32)) {
        check(self, unsafe { lib::pxui_ui_view_transform_about(self.0.as_ptr(), b, scale, transform.as_ptr(), pivot.0, pivot.1) })
    }
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

/// What `Ui::node_mut` edits of a node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeFields {
    pub look: Look,
    pub mark: Mark,
    pub inset: pxui_types::geom::Edges,
}
