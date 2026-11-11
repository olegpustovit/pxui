//! The painter of a canvas callback. Shapes are built by `pxui_types::shape::Shapes` and stored straight into a span of the
//! layer's instance storage (one 64-byte store each, no library call); the library is called when the span is full, and for
//! what needs the engine: text, images and measurement. Every library call commits the span first, so instances land in call
//! order.
use crate::composite::{defer_callback_panic, deferred};
use crate::sys::PxuiPaint as RawPaint;
use core::{ffi::c_void, marker::PhantomData, ptr::NonNull};
use std::alloc::Layout;
use pxui_types::{node::Dir, draw::Instance, ffi::PxuiPaintInfo, geom::{Rect, Rgba as Color, Rgba}, shape::{clip_of, Shapes}, theme::Tokens, ui::text_top_for, PxuiBox, Span, Str, TextMetrics, TextStyle};

library!(lib {
    fn pxui_ui_draw(ui: *mut PxuiUi, box_: *const PxuiBox, over: u8, build: Option<extern "C" fn(*mut c_void, *mut RawPaint, Rect)>, user: *mut c_void, release: Option<extern "C" fn(*mut c_void)>) -> i32;
    fn pxui_paint_info(p: *mut RawPaint, out: *mut PxuiPaintInfo) -> i32;
    fn pxui_paint_span(p: *mut RawPaint, at_least: usize, span: *mut Span) -> i32;
    fn pxui_paint_icon(p: *mut RawPaint, x: f32, y: f32, font: u16, size: f32, codepoint: u32, c: Rgba) -> f32;
    fn pxui_paint_text(p: *mut RawPaint, x: f32, y: f32, s: Str);
    fn pxui_paint_text_color(p: *mut RawPaint, x: f32, y: f32, c: Rgba, s: Str);
    fn pxui_paint_text_size(p: *mut RawPaint, x: f32, y: f32, size: i64, c: Rgba, s: Str);
    fn pxui_paint_text_with(p: *mut RawPaint, x: f32, y: f32, has_size: u8, size: i64, tracking: f32, has_color: u8, color: Rgba, s: Str);
    fn pxui_paint_text_styled(p: *mut RawPaint, x: f32, y: f32, style: *const TextStyle, width: f32, has_color: u8, color: Rgba, s: Str) -> TextMetrics;
    fn pxui_paint_text_width(p: *mut RawPaint, has_size: u8, size: i64, s: Str) -> f32;
    fn pxui_paint_text_metrics(p: *mut RawPaint, style: *const TextStyle, width: f32, s: Str) -> TextMetrics;
    fn pxui_paint_caret_x(p: *mut RawPaint, has_size: u8, size: i64, s: Str, index: usize) -> f32;
    fn pxui_paint_selection(p: *mut RawPaint, has_size: u8, size: i64, s: Str, a: usize, b: usize, build: Option<extern "C" fn(*mut c_void, f32, f32)>, user: *mut c_void) -> i32;
    fn pxui_paint_cap(p: *mut RawPaint, x: f32, y: f32, has_color: u8, color: Rgba, s: Str);
    fn pxui_paint_cap_width(p: *mut RawPaint, s: Str) -> f32;
    fn pxui_paint_ratio(p: *mut RawPaint, x: f32, y: f32, a: i64, b: i64);
    fn pxui_paint_input_region(p: *mut RawPaint, cursor: f32, x: f32, y: f32, w: f32, h: f32);
});

pub struct Paint<'a> {
    raw: NonNull<RawPaint>,
    _borrow: PhantomData<&'a mut RawPaint>,
    span: Span,
    open: bool,
    clip: Rect,
    pub theme: Tokens,
    pub font_size: i64,
    pub row_height: i64,
    pub density: i64,
    pub rtl: bool,
}

#[track_caller]
fn refused(what: &str, rc: i32) { if rc != 0 { panic!("pxui: {what} refused ({rc})"); } }

#[inline]
fn some(color: Option<Rgba>) -> (u8, Rgba) { (color.is_some() as u8, color.unwrap_or_default()) }

impl<'a> Paint<'a> {
    unsafe fn from_raw(raw: *mut RawPaint) -> Option<Self> {
        let raw = NonNull::new(raw)?;
        let mut info = PxuiPaintInfo { size: core::mem::size_of::<PxuiPaintInfo>() as u32, rtl: 0, reserved: [0; 3], font_size: 0, row_height: 0, density: 0, clip: Rect::default(), clip_radius: 0.0, theme: pxui_types::ffi::ThemeTokens::from(pxui_types::theme::default()) };
        refused("paint info", unsafe { lib::pxui_paint_info(raw.as_ptr(), &mut info) });
        Some(Paint { raw, _borrow: PhantomData, span: Span::EMPTY, open: false, clip: info.clip, theme: info.theme.into(), font_size: info.font_size, row_height: info.row_height, density: info.density, rtl: info.rtl != 0 })
    }

    /// Commits the span, so the library's own instances (text, images) follow the shapes drawn before them.
    #[inline]
    fn flush(&mut self) {
        if self.open {
            self.open = false;
            refused("span commit", unsafe { lib::pxui_paint_span(self.raw.as_ptr(), 0, &mut self.span) });
        }
    }

    /// The library's painter for a call that draws or measures itself.
    #[inline]
    pub(crate) fn raw(&mut self) -> *mut RawPaint {
        self.flush();
        self.raw.as_ptr()
    }

    /// Commits the span and takes the next window, restamped with this painter's clip.
    #[cold]
    fn refill(&mut self) {
        self.open = true;
        refused("span", unsafe { lib::pxui_paint_span(self.raw.as_ptr(), 1, &mut self.span) });
        self.span.clip = clip_of(self.clip);
    }

    #[inline] pub fn label_size(&self) -> i64 { 8.max(self.font_size - 2) }
    #[inline] pub fn cap_tracking(&self) -> f32 { 0.08 * self.label_size() as f32 }
    #[inline] pub fn text_top(&self, size: Option<i64>, y: f32, h: f32) -> f32 { text_top_for(self.font_size, size, y, h) }

    /// One glyph of icon font `font` at `size` points, its top-left at `at`: its advance.
    #[inline] pub fn icon(&mut self, at: (f32, f32), font: u16, size: f32, ch: char, c: Rgba) -> f32 { unsafe { lib::pxui_paint_icon(self.raw(), at.0, at.1, font, size, ch as u32, c) } }
    #[inline] pub fn text(&mut self, at: (f32, f32), s: &str) { unsafe { lib::pxui_paint_text(self.raw(), at.0, at.1, crate::as_str(s)) } }
    #[inline] pub fn text_c(&mut self, at: (f32, f32), c: Rgba, s: &str) { unsafe { lib::pxui_paint_text_color(self.raw(), at.0, at.1, c, crate::as_str(s)) } }
    #[inline] pub fn text_s(&mut self, at: (f32, f32), size: i64, c: Rgba, s: &str) { unsafe { lib::pxui_paint_text_size(self.raw(), at.0, at.1, size, c, crate::as_str(s)) } }
    #[inline]
    pub fn text_with(&mut self, at: (f32, f32), size: Option<i64>, tracking: f32, color: Option<Rgba>, s: &str) {
        let (has_color, color) = some(color);
        unsafe { lib::pxui_paint_text_with(self.raw(), at.0, at.1, size.is_some() as u8, size.unwrap_or_default(), tracking, has_color, color, crate::as_str(s)) }
    }
    #[inline]
    pub fn text_styled(&mut self, at: (f32, f32), style: &TextStyle, width: f32, color: Option<Rgba>, s: &str) -> TextMetrics {
        let (has_color, color) = some(color);
        unsafe { lib::pxui_paint_text_styled(self.raw(), at.0, at.1, style, width, has_color, color, crate::as_str(s)) }
    }
    #[inline] pub fn text_width(&mut self, size: Option<i64>, s: &str) -> f32 { unsafe { lib::pxui_paint_text_width(self.raw(), size.is_some() as u8, size.unwrap_or_default(), crate::as_str(s)) } }
    #[inline] pub fn text_metrics(&mut self, style: &TextStyle, width: f32, s: &str) -> TextMetrics { unsafe { lib::pxui_paint_text_metrics(self.raw(), style, width, crate::as_str(s)) } }
    #[inline] pub fn caret_x(&mut self, size: Option<i64>, s: &str, index: usize) -> f32 { unsafe { lib::pxui_paint_caret_x(self.raw(), size.is_some() as u8, size.unwrap_or_default(), crate::as_str(s), index) } }
    /// Calls `f(x, width)` for each run of the selection `a..b` of `s`.
    #[track_caller]
    pub fn selection<F: FnMut(f32, f32)>(&mut self, size: Option<i64>, s: &str, a: usize, b: usize, mut f: F) {
        extern "C" fn call<F: FnMut(f32, f32)>(user: *mut c_void, x: f32, w: f32) {
            if deferred() { return; }
            if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (unsafe { &mut *user.cast::<F>() })(x, w))) { defer_callback_panic(panic); }
        }
        refused("selection", unsafe { lib::pxui_paint_selection(self.raw(), size.is_some() as u8, size.unwrap_or_default(), crate::as_str(s), a, b, Some(call::<F>), &mut f as *mut F as *mut c_void) })
    }
    /// A label: upper case at the label size, ink-2 by default.
    #[inline]
    pub fn cap(&mut self, at: (f32, f32), color: Option<Rgba>, s: &str) {
        let (has_color, color) = some(color);
        unsafe { lib::pxui_paint_cap(self.raw(), at.0, at.1, has_color, color, crate::as_str(s)) }
    }
    #[inline] pub fn cap_width(&mut self, s: &str) -> f32 { unsafe { lib::pxui_paint_cap_width(self.raw(), crate::as_str(s)) } }
    #[inline] pub fn ratio(&mut self, at: (f32, f32), a: i64, b: i64) { unsafe { lib::pxui_paint_ratio(self.raw(), at.0, at.1, a, b) } }
    #[inline] pub fn input_region(&mut self, cursor: f32, x: f32, y: f32, w: f32, h: f32) { unsafe { lib::pxui_paint_input_region(self.raw(), cursor, x, y, w, h) } }
}

// The shapes as inherent methods, as the engine's `Paint` has them: an app needs no trait import. Each is the one definition in
// `pxui_types::shape`.
#[allow(clippy::too_many_arguments)]
impl Paint<'_> {
    #[inline] pub fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) { Shapes::fill(self, x, y, w, h, color) }
    #[inline] pub fn stroke_w(&mut self, x: f32, y: f32, w: f32, h: f32, width: f32, color: Color) { Shapes::stroke_w(self, x, y, w, h, width, color) }
    #[inline] pub fn stroke(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) { Shapes::stroke(self, x, y, w, h, color) }
    #[inline] pub fn frame_w(&mut self, x: f32, y: f32, w: f32, h: f32, width: f32, color: Color) { Shapes::frame_w(self, x, y, w, h, width, color) }
    #[inline] pub fn frame(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) { Shapes::frame(self, x, y, w, h, color) }
    #[inline] pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, fill: Option<Color>, stroke: Option<Color>) { Shapes::rect(self, x, y, w, h, fill, stroke) }
    #[inline] pub fn brackets(&mut self, x: f32, y: f32, w: f32, h: f32, offset: f32, length: f32, width: f32, color: Color) { Shapes::brackets(self, x, y, w, h, offset, length, width, color) }
    #[inline] pub fn dashed_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) { Shapes::dashed_rect(self, x, y, w, h, color) }
    #[inline] pub fn line_w(&mut self, from: (f32, f32), to: (f32, f32), width: f32, color: Color) { Shapes::line_w(self, from, to, width, color) }
    #[inline] pub fn line(&mut self, from: (f32, f32), to: (f32, f32), color: Color) { Shapes::line(self, from, to, color) }
    #[inline] pub fn circle(&mut self, at: (f32, f32), radius: f32, fill: Option<Color>, stroke: Option<Color>) { Shapes::circle(self, at, radius, fill, stroke) }
    #[inline] pub fn grid(&mut self, x: f32, y: f32, w: f32, h: f32, origin: (f32, f32), spacing: f32, dot: f32, color: Color) { Shapes::grid(self, x, y, w, h, origin, spacing, dot, color) }
    #[inline] pub fn chevron(&mut self, at: (f32, f32), direction: Dir, color: Color) { Shapes::chevron(self, at, direction, color) }
    #[inline] pub fn dashed_w(&mut self, from: (f32, f32), to: (f32, f32), width: f32, color: Color) { Shapes::dashed_w(self, from, to, width, color) }
    #[inline] pub fn dashed(&mut self, from: (f32, f32), to: (f32, f32), color: Color) { Shapes::dashed(self, from, to, color) }
    #[inline] pub fn flag(&mut self, at: (f32, f32), round: bool, on: bool) { Shapes::flag(self, at, round, on) }

}

impl Shapes for Paint<'_> {
    #[inline]
    fn push(&mut self, instance: Instance) {
        if !self.span.write(instance) {
            self.refill();
            self.span.write(instance);
        }
    }
    #[inline] fn density(&self) -> i64 { self.density }
    #[inline] fn theme(&self) -> Tokens { self.theme }
    #[inline] fn clip_rect(&self) -> Rect { self.clip }
    #[inline]
    fn set_clip_rect(&mut self, clip: Rect) {
        self.clip = clip;
        self.span.clip = clip_of(clip);
    }
}

// The canvas closure is boxed behind the user pointer; the library releases it exactly once.
struct Draw<F>(F);
extern "C" fn draw<F: FnMut(&mut Paint<'_>, Rect)>(user: *mut c_void, raw: *mut RawPaint, rect: Rect) {
    if deferred() { return; }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(mut paint) = (unsafe { Paint::from_raw(raw) }) else { return };
        (unsafe { &mut *user.cast::<Draw<F>>() }.0)(&mut paint, rect);
        paint.flush();
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}

// A canvas closure is boxed once per rebuilt frame and released when the library lets go of it. Released boxes wait in a small pool and
// the next closure of the same layout (a canvas rebuilt each frame is always the same type) takes one: a settled frame allocates nothing.
// ponytail: at most POOLED boxes per thread, never returned to the allocator at thread exit; per-size pools if canvases of many types alternate.
const POOLED: usize = 64;
thread_local! { static POOL: std::cell::RefCell<Vec<(Layout, *mut u8)>> = const { std::cell::RefCell::new(Vec::new()) }; }

fn boxed<F>(f: F) -> *mut c_void {
    let layout = Layout::new::<Draw<F>>();
    let spare = if layout.size() == 0 { None } else { POOL.with(|pool| { let mut pool = pool.borrow_mut(); pool.iter().position(|(l, _)| *l == layout).map(|at| pool.swap_remove(at).1) }) };
    let at = match spare { Some(at) => at, None if layout.size() == 0 => return Box::into_raw(Box::new(Draw(f))).cast(), None => unsafe { std::alloc::alloc(layout) } };
    if at.is_null() { std::alloc::handle_alloc_error(layout); }
    unsafe { at.cast::<Draw<F>>().write(Draw(f)); }
    at.cast()
}

extern "C" fn release<F>(user: *mut c_void) {
    let layout = Layout::new::<Draw<F>>();
    unsafe { core::ptr::drop_in_place(user.cast::<Draw<F>>()); }
    if layout.size() == 0 { return unsafe { drop(Box::from_raw(user.cast::<Draw<F>>())) }; }
    let kept = POOL.with(|pool| { let mut pool = pool.borrow_mut(); (pool.len() < POOLED).then(|| pool.push((layout, user.cast()))).is_some() });
    if !kept { unsafe { std::alloc::dealloc(user.cast(), layout) } }
}

impl crate::Ui {
    fn canvas_with<F: FnMut(&mut Paint<'_>, Rect) + 'static>(&mut self, b: &PxuiBox, over: u8, f: F) {
        let user = boxed(f);
        crate::check(self, unsafe { lib::pxui_ui_draw(self.0.as_ptr(), b, over, Some(draw::<F>), user, Some(release::<F>)) });
    }
    /// Free-form drawing with the node's final laid-out rectangle, after its look and before its children.
    #[inline] #[track_caller] pub fn canvas<F: FnMut(&mut Paint<'_>, Rect) + 'static>(&mut self, b: &PxuiBox, f: F) { self.canvas_with(b, 0, f) }
    /// A canvas that draws after the node's children, outside its content clip.
    #[inline] #[track_caller] pub fn canvas_over<F: FnMut(&mut Paint<'_>, Rect) + 'static>(&mut self, b: &PxuiBox, f: F) { self.canvas_with(b, 1, f) }
    #[inline] #[track_caller] pub fn draw<F: FnMut(&mut Paint<'_>, Rect) + 'static>(&mut self, b: &PxuiBox, f: F) { self.canvas_with(b, 0, f) }
    #[inline] #[track_caller] pub fn draw_over<F: FnMut(&mut Paint<'_>, Rect) + 'static>(&mut self, b: &PxuiBox, f: F) { self.canvas_with(b, 1, f) }
}
