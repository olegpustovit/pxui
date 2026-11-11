//! Boxes and what hangs off them, with the engine's names and shapes (see `ui_state` for how failures surface).
use crate::composite::{call, defer_callback_panic, deferred};
use crate::node::{look, mark, size};
use crate::{as_str, check, Ui};
use core::ffi::c_void;
use pxui_types::{geom::{Point, Rect}, ffi::{PxuiBoxDesc, PxuiLookDesc, PxuiMarkDesc, PxuiSizeDesc, Slice}, theme::Look, Size, ui::{BoxOpts, Drop, HitFn, Payload}, Mark, PxuiBox, Str};
use std::fmt::{Arguments, Write};

library!(lib {
    fn pxui_ui_box(ui: *mut PxuiUi, label: Str, desc: *const PxuiBoxDesc, out: *mut PxuiBox) -> i32;
    fn pxui_ui_set_at(ui: *mut PxuiUi, box_: *const PxuiBox, x: f32, y: f32) -> i32;
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
    fn pxui_ui_cancel_carry(ui: *mut PxuiUi) -> i32;
    fn pxui_ui_drop_target(ui: *mut PxuiUi, box_: *const PxuiBox, tag: *mut u32, kind: *mut Str, value: *mut Str) -> i32;
    fn pxui_ui_context_at(ui: *mut PxuiUi, box_: *const PxuiBox, present: *mut u8, out: *mut Point) -> i32;
    fn pxui_ui_hovered(ui: *mut PxuiUi, box_: *const PxuiBox, within: u8, out: *mut u8) -> i32;
    fn pxui_ui_press_keys(ui: *mut PxuiUi, box_: *const PxuiBox, ptr: *mut *const u32, len: *mut usize) -> i32;
    fn pxui_ui_root_at(ui: *mut PxuiUi, x: f32, y: f32, roots: Slice<i64>, present: *mut u8, out: *mut i64) -> i32;
    fn pxui_ui_view_flat(ui: *mut PxuiUi, label: Str, revision: u64, width: PxuiSizeDesc, height: PxuiSizeDesc, build: Option<extern "C" fn(*mut c_void, *mut PxuiUi)>, user: *mut c_void, out: *mut PxuiBox) -> i32;
    fn pxui_ui_view_offset(ui: *mut PxuiUi, box_: *const PxuiBox, dx: f32, dy: f32) -> i32;
    fn pxui_ui_view_transform_about(ui: *mut PxuiUi, box_: *const PxuiBox, scale: f32, transform: *const f32, px: f32, py: f32) -> i32;
    fn pxui_ui_view_backdrop_blur(ui: *mut PxuiUi, box_: *const PxuiBox, sigma: f32) -> i32;
});

/// A label written from `format_args!`: up to 128 bytes on the stack, longer ones on the heap.
pub(crate) struct Label { stack: [u8; 128], len: usize, heap: String }

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
extern "C" fn hit_release(user: *mut c_void) { drop(unsafe { Box::from_raw(user.cast::<HitFn>()) }) }

impl Ui {
    #[track_caller]
    pub fn box_(&mut self, label: &str, opts: BoxOpts) -> PxuiBox {
        let (hit, hit_user, hit_release_fn) = match &opts.hit {
            Some(hit) => (Some(hit_call as extern "C" fn(*mut c_void, Rect) -> Rect), Box::into_raw(Box::new(hit.clone())).cast::<c_void>(), Some(hit_release as extern "C" fn(*mut c_void))),
            None => (None, core::ptr::null_mut(), None),
        };
        let desc = PxuiBoxDesc {
            size: core::mem::size_of::<PxuiBoxDesc>() as u32, axis: opts.axis as u32, flags: opts.flags, w: size(opts.w), h: size(opts.h),
            max_h: opts.max_h, padding: opts.padding, scroll_step: opts.scroll_step, has_at: opts.at.is_some() as u8,
            has_hit_rect: opts.hit_rect.is_some() as u8, reserved: [0; 2], at: opts.at.map_or([0.0; 2], |(x, y)| [x, y]),
            hit_rect: opts.hit_rect.unwrap_or_default(), hit, hit_user, hit_release: hit_release_fn,
        };
        let mut out = PxuiBox::default();
        check(self, unsafe { lib::pxui_ui_box(self.0.as_ptr(), as_str(label), &desc, &mut out) });
        out
    }
    #[track_caller]
    pub fn box_fmt(&mut self, label: Arguments, opts: BoxOpts) -> PxuiBox { self.box_(Label::new(label).as_str(), opts) }
    #[inline] #[track_caller] pub fn set_at(&mut self, b: &PxuiBox, at: (f32, f32)) { check(self, unsafe { lib::pxui_ui_set_at(self.0.as_ptr(), b, at.0, at.1) }) }
    #[track_caller]
    pub fn patch_look(&mut self, b: &PxuiBox, value: &Look) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_patch_look(self.0.as_ptr(), b, &look(*value), &mut out) });
        out != 0
    }
    #[inline]
    #[track_caller]
    pub fn patch_rect(&mut self, b: &PxuiBox, value: Rect) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_patch_rect(self.0.as_ptr(), b, value, &mut out) });
        out != 0
    }
    #[track_caller]
    pub fn patch_mark(&mut self, b: &PxuiBox, value: Mark) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_patch_mark(self.0.as_ptr(), b, &mark(value), &mut out) });
        out != 0
    }
    #[inline]
    #[track_caller]
    pub fn local_rect(&self, b: &PxuiBox) -> Rect {
        let mut out = Rect::default();
        check(self, unsafe { lib::pxui_ui_local_rect(self.0.as_ptr(), b, &mut out) });
        out
    }
    #[inline]
    #[track_caller]
    pub fn scroll_offset(&self, b: &PxuiBox) -> f32 {
        let mut out = 0.0;
        check(self, unsafe { lib::pxui_ui_scroll_offset(self.0.as_ptr(), b, 0, &mut out) });
        out
    }
    /// The painted scroll position, including elastic edge movement.
    #[inline]
    #[track_caller]
    pub fn scroll_position(&self, b: &PxuiBox) -> f32 {
        let mut out = 0.0;
        check(self, unsafe { lib::pxui_ui_scroll_offset(self.0.as_ptr(), b, 1, &mut out) });
        out
    }
    #[inline] #[track_caller] pub fn set_scroll_offset(&mut self, b: &PxuiBox, value: f32) { check(self, unsafe { lib::pxui_ui_set_scroll_offset(self.0.as_ptr(), b, value) }) }
    #[track_caller]
    pub fn text_state(&self, b: &PxuiBox) -> Option<String> {
        let (mut present, mut out) = (0, Str::default());
        check(self, unsafe { lib::pxui_ui_text_state(self.0.as_ptr(), b, &mut present, &mut out) });
        (present != 0).then(|| String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(out.ptr, out.len) }).into_owned())
    }
    #[track_caller]
    pub fn set_text_state(&mut self, b: &PxuiBox, value: Option<String>) {
        let text = value.as_deref().unwrap_or("");
        check(self, unsafe { lib::pxui_ui_set_text_state(self.0.as_ptr(), b, value.is_some() as u8, as_str(text)) })
    }
    /// The text kept in the box's slot under `index`, set to `text` when it differs: borrowed from the library until the
    /// next call on this box or the next frame.
    #[track_caller]
    pub fn cached_label(&mut self, b: &PxuiBox, index: usize, text: &str) -> &str {
        let mut out = Str::default();
        check(self, unsafe { lib::pxui_ui_cached_label(self.0.as_ptr(), b, index as u64, as_str(text), &mut out) });
        unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(out.ptr, out.len)) }
    }
    #[inline] #[track_caller] pub fn to_front(&mut self, order: i64, b: &PxuiBox) { check(self, unsafe { lib::pxui_ui_to_front(self.0.as_ptr(), order, b) }) }

    #[track_caller]
    pub fn carrying(&self) -> Option<Payload> {
        let (mut present, mut kind, mut value) = (0, Str::default(), Str::default());
        check(self, unsafe { lib::pxui_ui_carrying(self.0.as_ptr(), &mut present, &mut kind, &mut value) });
        (present != 0).then(|| Payload { kind: owned(kind), value: owned(value) })
    }
    #[track_caller]
    pub fn carry(&mut self, from: Option<&PxuiBox>, kind: &str, value: &str) {
        let raw = from.map_or(core::ptr::null(), |b| b as *const PxuiBox);
        check(self, unsafe { lib::pxui_ui_carry(self.0.as_ptr(), from.is_some() as u8, raw, as_str(kind), as_str(value)) })
    }
    #[inline] #[track_caller] pub fn cancel_carry(&mut self) { check(self, unsafe { lib::pxui_ui_cancel_carry(self.0.as_ptr()) }) }
    #[track_caller]
    pub fn drop_target(&mut self, b: &PxuiBox) -> Option<Drop> {
        let (mut tag, mut kind, mut value) = (0, Str::default(), Str::default());
        check(self, unsafe { lib::pxui_ui_drop_target(self.0.as_ptr(), b, &mut tag, &mut kind, &mut value) });
        let payload = || Payload { kind: owned(kind), value: owned(value) };
        match tag { 1 => Some(Drop::Hover(payload())), 2 => Some(Drop::Dropped(payload())), _ => None }
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
    /// The key codes held at the box's captured press, retained through release.
    #[track_caller]
    pub fn press_keys(&self, b: &PxuiBox) -> Vec<u32> {
        let (mut ptr, mut len) = (core::ptr::null(), 0);
        check(self, unsafe { lib::pxui_ui_press_keys(self.0.as_ptr(), b, &mut ptr, &mut len) });
        if len == 0 { Vec::new() } else { unsafe { core::slice::from_raw_parts(ptr, len) }.to_vec() }
    }
    #[track_caller]
    pub fn root_at(&self, point: (f32, f32), roots: &[i64]) -> Option<i64> {
        let (mut present, mut out) = (0, 0);
        check(self, unsafe { lib::pxui_ui_root_at(self.0.as_ptr(), point.0, point.1, Slice { ptr: roots.as_ptr(), len: roots.len() }, &mut present, &mut out) });
        (present != 0).then_some(out)
    }
    /// `view` that takes no entry of its own in the painter's view table: its instances ride its parent's.
    #[track_caller]
    pub fn view_flat<F: FnMut(&mut Ui)>(&mut self, label: &str, revision: u64, w: Size, h: Size, mut build: F) -> PxuiBox {
        let mut out = PxuiBox::default();
        check(self, unsafe { lib::pxui_ui_view_flat(self.0.as_ptr(), as_str(label), revision, size(w), size(h), Some(call::<F>), &mut build as *mut F as *mut c_void, &mut out) });
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
    #[inline]
    #[track_caller]
    pub fn view_transform_about(&mut self, b: &PxuiBox, scale: f32, transform: [f32; 4], pivot: (f32, f32)) {
        check(self, unsafe { lib::pxui_ui_view_transform_about(self.0.as_ptr(), b, scale, transform.as_ptr(), pivot.0, pivot.1) })
    }
    #[inline] #[track_caller] pub fn view_backdrop_blur(&mut self, b: &PxuiBox, sigma: f32) { check(self, unsafe { lib::pxui_ui_view_backdrop_blur(self.0.as_ptr(), b, sigma) }) }
}

/// A string the library lent: copied, since it only lives until the next frame.
fn owned(value: Str) -> String { if value.len == 0 { String::new() } else { String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(value.ptr, value.len) }).into_owned() } }
