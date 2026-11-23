//! The painter of a canvas callback. Shapes are built by `pxui_types::shape::Shapes` and stored straight into a span of the
//! layer's instance storage (one 64-byte store each, no library call); the library is called when the span is full, and for
//! what needs the engine: text, images and measurement. Every library call commits the span first, so instances land in call
//! order.
use crate::Handle;
use crate::composite::{defer_callback_panic, deferred};
use crate::sys::PxuiPaint as RawPaint;
use core::{cell::Cell, ffi::c_void, marker::PhantomData, ptr::NonNull};
use std::alloc::Layout;
use pxui_types::{node::Dir, draw::Instance, ffi::{PxuiLabelStyle, PxuiPaintInfo}, Label, geom::{Rect, Rgba}, shape::{clip_of, Shapes}, theme::Tokens, PxuiBox, Span, Str, TextMetrics};

library!(lib {
    fn pxui_paint_label_spans(p:*mut RawPaint,x:f32,y:f32,style:*const PxuiLabelStyle,width:f32,text:Str,spans:*const pxui_types::TextSpans,out:*mut TextMetrics)->i32;
    fn pxui_ui_draw(ui: *mut PxuiUi, box_: *const PxuiBox, over: u8, build: Option<extern "C" fn(*mut c_void, *mut RawPaint, Rect, *const PxuiPaintInfo)>, user: *mut c_void, release: Option<extern "C" fn(*mut c_void)>) -> i32;
    fn pxui_paint_span(p: *mut RawPaint, at_least: usize, span: *mut Span) -> i32;
    fn pxui_paint_icon(p: *mut RawPaint, x: f32, y: f32, font: u16, size: f32, codepoint: u32, c: Rgba) -> f32;
    fn pxui_paint_label(p: *mut RawPaint, x: f32, y: f32, style: *const PxuiLabelStyle, width: f32, s: Str) -> TextMetrics;
    fn pxui_paint_label_chars(p: *mut RawPaint, x: f32, y: f32, style: *const PxuiLabelStyle, s: Str) -> f32;
    fn pxui_paint_scroll_y(p: *mut RawPaint, slot: u64) -> f32;
    fn pxui_paint_color(p: *mut RawPaint, token: u16) -> Rgba;
    fn pxui_paint_icon_word(p: *mut RawPaint) -> u32;
});

/// The painter a canvas callback draws with (`Ui::canvas`, `canvas_over`, an `Icon::Draw`): shapes, text and images in points, in call
/// order, cut to the node's clip.
///
/// Geometry is a `Rect` for areas and `(x, y)` points for lines and marks; colours of shapes are `Fill` and `Stroke` values; text takes a
/// `Label` (or a resolved `TextStyle` for spans). It lives only for the callback.
pub struct Paint<'a> {
    w: Writer,
    _borrow: PhantomData<&'a mut RawPaint>,
    clip_radius: f32,
    rtl: bool,
    ink: Cell<(u16, Rgba)>,
    capture_palette: bool,
}

/// The span writer behind a `Paint`: the one implementor of `Shapes` in the wrapper, kept private so the painter's public surface is its
/// own `Rect` + `Fill`/`Stroke` calls.
struct Writer {
    raw: NonNull<RawPaint>,
    span: Span,
    open: bool,
    clip: Rect,
    theme: Tokens,
    density: i64,
}

impl Writer {
    /// Commits the span, so the library's own instances (text, images) follow the shapes drawn before them.
    #[inline]
    fn flush(&mut self) {
        if self.open {
            self.open = false;
            refused("span commit", unsafe { lib::pxui_paint_span(self.raw.as_ptr(), 0, &mut self.span) });
        }
    }

    /// Commits the span and takes the next window, restamped with this painter's clip.
    #[cold]
    fn refill(&mut self) {
        self.open = true;
        refused("span", unsafe { lib::pxui_paint_span(self.raw.as_ptr(), 1, &mut self.span) });
        self.span.clip = clip_of(self.clip);
    }
}

#[track_caller]
fn refused(what: &str, rc: i32) { if rc != 0 { panic!("pxui: {what} refused ({rc})"); } }

impl<'a> Paint<'a> {
    unsafe fn from_raw(raw: *mut RawPaint, info: &PxuiPaintInfo) -> Option<Self> {
        let raw = NonNull::new(raw)?;
        let w = Writer { raw, span: Span::EMPTY, open: false, clip: info.clip, theme: info.theme.into(), density: info.density };
        Some(Paint { w, _borrow: PhantomData, clip_radius: info.clip_radius, rtl: info.rtl != 0,
            ink: Cell::new((u16::MAX, Rgba::CLEAR)), capture_palette: info.reserved[0] != 0 })
    }

    /// Commits the span, so the library's own instances (text, images) follow the shapes drawn before them.
    #[inline]
    fn flush(&mut self) { self.w.flush() }

    /// The library's painter for a call that draws or measures itself.
    #[inline]
    pub(crate) fn raw(&mut self) -> *mut RawPaint {
        self.w.flush();
        self.w.raw.as_ptr()
    }

    /// The scroll offset of the retained slot `slot` (a handle's `box_slot`), as this frame paints it; 0 for a slot out of range.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("list", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     let slot = node.box_slot;
    ///     ui.canvas(&node, move |paint, _| assert_eq!(paint.scroll_y(slot), 0.0));
    /// });
    /// ```
    #[inline] pub fn scroll_y(&mut self, slot: u64) -> f32 { unsafe { lib::pxui_paint_scroll_y(self.raw(), slot) } }
    /// Reads a color token from the painter's root theme.
    ///
    /// The painter takes the palette when painting starts. Unset tokens read transparent.
    /// Changing the root palette repaints canvases that use it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{theme::ColorToken, Node, Rgba, Size, ThemeHandle};
    /// use std::{cell::Cell, rc::Rc};
    /// let ink = Rgba::rgb(20, 20, 20);
    /// let seen = Rc::new(Cell::new(Rgba::default()));
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// let mut theme = ThemeHandle::default();
    /// theme.set_color(ColorToken(0), ink);
    /// ui.set_theme(&theme).unwrap();
    /// ui.frame(&pxui::FrameIn::new(100.0, 100.0), &mut |ui| {
    ///     let b = ui.node("canvas", Node::DEFAULT.w(Size::Px(10.0)).h(Size::Px(10.0)));
    ///     let seen = seen.clone();
    ///     ui.canvas(&b, move |p, _| seen.set(p.color(ColorToken(0))));
    /// }).unwrap();
    /// assert_eq!(seen.get(), ink);
    /// ```
    #[inline]
    pub fn color(&self, token: pxui_types::theme::ColorToken) -> Rgba {
        let (last, ink) = self.ink.get();
        if token.0 == last {
            return ink;
        }
        let ink = unsafe { lib::pxui_paint_color(self.w.raw.as_ptr(), token.0) };
        self.ink.set((token.0, ink));
        ink
    }
    /// The theme tokens this painter draws with: the `Ui`'s root theme when painting started.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("canvas", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, _| { let _accent = paint.tokens().accent; });
    /// });
    /// ```
    #[inline] pub fn tokens(&self) -> Tokens { self.w.theme }
    /// Device pixels per point of the surface being painted (1 or more).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("canvas", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, _| assert!(paint.density() >= 1.0));
    /// });
    /// ```
    #[inline] pub fn density(&self) -> f32 { self.w.density as f32 }
    /// Whether the node paints right to left (its layout direction), so a canvas can mirror what it draws.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("canvas", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, _| assert!(!paint.rtl()));
    /// });
    /// ```
    #[inline] pub fn rtl(&self) -> bool { self.rtl }
    /// The corner radius of the clip shapes are cut to, in points: a rounded `CLIP` ancestor's, 0 for a square clip.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("canvas", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, _| assert_eq!(paint.clip_radius(), 0.0));
    /// });
    /// ```
    #[inline] pub fn clip_radius(&self) -> f32 { self.clip_radius }

    /// Draws one glyph of icon font `font` at `size` points, its top-left at `at`, in `color`; returns its advance in points.
    ///
    /// `font` is the index the font was added at (`UiText::add_font`); a character the font does not hold, or a size that is not
    /// positive and finite, draws nothing and returns 0.
    /// Allocates: never when warm; a glyph it has not drawn before is rasterised once.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("icon", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| { paint.icon((rect.x, rect.y), 0, 12.0, 'x', pxui::geom::Rgba::rgb(0, 0, 0)); });
    /// });
    /// ```
    #[inline] pub fn icon(&mut self, at: (f32, f32), font: u16, size: f32, ch: char, color: Rgba) -> f32 { unsafe { lib::pxui_paint_icon(self.raw(), at.0, at.1, font, size, ch as u32, color) } }
    /// Draws `s` in `style` at `at` (the top-left of its layout, in points), laid out within `width` points (0: unbounded); returns its metrics.
    ///
    /// The first baseline lies `TextMetrics::baseline` below `at`, on a device pixel, whatever the line height: to put it on `y`, draw at
    /// `y - baseline` (measure with `text_metrics`). A `Label::line_height` text drawn at the top of a line box lands where the line box's glyphs do.
    /// The style's unset fields resolve as a label body resolves them, against this painter's theme (the root theme's label look,
    /// its rows and the app's colour tokens); the box fields (`width`, `pad`) are the caller's here. A style that does not resolve
    /// (a size that is not positive, an unknown wrap or align) draws nothing and returns zero metrics.
    /// Allocates: never when warm; text it has not shaped before is shaped once.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::Label;
    /// use std::{cell::Cell, rc::Rc};
    /// let width = Rc::new(Cell::new(0.0));
    /// pxui::doc::frames(2, &mut |ui| {
    ///     let node = ui.node("title", pxui::Node::DEFAULT.w(pxui::ui::Size::Px(100.0)).h(pxui::ui::Size::Px(20.0)));
    ///     let width = width.clone();
    ///     ui.canvas(&node, move |paint, rect| width.set(paint.text(pxui::Point::new(rect.x, rect.y), Label::TITLE, 0.0, "Songs").width));
    /// });
    /// assert!(width.get() > 0.0);
    /// ```
    #[inline]
    pub fn text(&mut self, at: crate::Point, style: Label, width: f32, s: &str) -> TextMetrics {
        let style = PxuiLabelStyle::from(&style);
        unsafe { lib::pxui_paint_label(self.raw(), at.x, at.y, &style, width, crate::as_str(s)) }
    }
    /// Draws `s` in `style` one character at a time from one-character layouts, digits on the advance of `0` (tabular): a number
    /// that changes every frame shapes nothing after the first use of its characters in that style.
    ///
    /// Each distinct character is shaped once per style and then hit in the text cache; there is no kerning between characters and
    /// no wrapping. Returns the width drawn. `0`-`9`, `:` `.` `,` `-` `/` `+` `%` and space are the intended set; any other character
    /// is drawn with its own advance. `style` resolves as in `text`, and `at` is the layout's top as there.
    /// Allocates: never when warm; the first use of a character in a style shapes it.
    /// Panics: never.
    ///
    /// ```
    /// use std::{cell::Cell, rc::Rc};
    /// let width = Rc::new(Cell::new(0.0));
    /// pxui::doc::frames(2, &mut |ui| {
    ///     let node = ui.node("clock", pxui::Node::DEFAULT.w(pxui::ui::Size::Px(100.0)).h(pxui::ui::Size::Px(20.0)));
    ///     let width = width.clone();
    ///     ui.canvas(&node, move |paint, rect| width.set(paint.text_chars(pxui::Point::new(rect.x, rect.y), pxui::Label::MONO, "1:23.4")));
    /// });
    /// assert!(width.get() > 0.0);
    /// ```
    pub fn text_chars(&mut self, at: crate::Point, style: Label, s: &str) -> f32 {
        let style = PxuiLabelStyle::from(&style);
        unsafe { lib::pxui_paint_label_chars(self.raw(), at.x, at.y, &style, crate::as_str(s)) }
    }
    /// Draws `text` at `at` (the top-left of its layout, in points; the first baseline is `TextMetrics::baseline` below it) in one shaped layout, wrapped within `width` points (0: unbounded), with up to 256 styled byte ranges `spans`; returns its metrics.
    ///
    /// `style` is a `Label`, resolved in this call under the painter's theme as `Paint::text` resolves it; the text is in the label's colour
    /// token, or the theme's ink when the label sets none. Spans may colour their own ranges. Spans are ordered, non-overlapping and nonempty, as UTF-8 byte ranges of `text` on character boundaries.
    /// Allocates: never when warm; text it has not shaped before is shaped once.
    /// Panics: never; a bad argument is the `Err` below.
    /// Errors: `Error::InvalidInput` for more than 256 or overlapping spans, a bad range or a non-finite size; `Error::State` when a span is open.
    ///
    /// ```
    /// use pxui::{Label, TextSpan};
    /// use std::{cell::Cell, rc::Rc};
    /// let width = Rc::new(Cell::new(0.0));
    /// let out = width.clone();
    /// pxui::doc::frames(2, &mut |ui| {
    ///     let node = ui.node("rich", pxui::Node::DEFAULT.w(pxui::ui::Size::Px(200.0)).h(pxui::ui::Size::Px(30.0)));
    ///     let out = out.clone();
    ///     ui.canvas(&node, move |paint, rect| {
    ///         let bold = TextSpan::new(0, 5).weight(700);
    ///         out.set(paint.text_spans(pxui::Point::new(rect.x, rect.y), Label::BODY, 0.0, "Hello world", &[bold]).unwrap().width);
    ///     });
    /// });
    /// assert!(width.get() > 0.0);
    /// ```
    pub fn text_spans(&mut self,at:crate::Point,style:Label,width:f32,text:&str,spans:&[pxui_types::TextSpan])->Result<TextMetrics,crate::Error> {
        let spans=pxui_types::TextSpans::new(spans);
        let mut out=TextMetrics::default();
        let rc=unsafe { lib::pxui_paint_label_spans(self.raw(),at.x,at.y,&PxuiLabelStyle::from(&style),width,crate::as_str(text),&spans,&mut out) };
        if rc==0 { Ok(out) } else { Err(crate::Error::from_code(rc)) }
    }
    /// Borrows the active painter's text service.
    ///
    /// The service uses the same fonts and label defaults as this painting scope. The borrow ends before painting continues.
    /// Allocates: never to borrow the service.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("canvas", pxui::Node::DEFAULT.w_px(60.0).h_px(20.0));
    ///     ui.canvas(&node, |paint, _| { let _ = paint.text_service(); });
    /// });
    /// ```
    pub fn text_service(&mut self) -> crate::UiText<'_> { crate::UiText::from_paint(self) }
}

/// How a shape's area is filled: one colour, its edges snapped to whole device pixels unless `fractional`.
///
/// A `Copy` value made with const builders, so an app names its fills once: `const RULE: Fill = Fill::new(Rgba::token(LINE)).fractional();`.
/// The colour may be an app colour token (`Rgba::token`): the painter resolves it for its theme each time it paints, so a const fill
/// follows light and dark.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fill {
    pub(crate) color: Rgba,
    pub(crate) fractional: bool,
}

impl Fill {
    /// Opaque black, pixel-snapped: the preset to build from when only the edges matter.
    pub const DEFAULT: Fill = Fill::new(Rgba::rgb(0, 0, 0));
    /// A fill of `color` (a colour or a colour token) with pixel-snapped edges: the pixels a rectangle of the same corners covers, crisp at any position.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{geom::Rgba, Fill};
    /// use pxui::theme::ColorToken;
    /// const INK: Fill = Fill::new(Rgba::rgb(20, 20, 20));
    /// const RULE: Fill = Fill::new(Rgba::token(ColorToken(3)));
    /// assert_ne!(INK, RULE);
    /// ```
    #[inline] pub const fn new(color: Rgba) -> Fill { Fill { color, fractional: false } }
    /// The same fill with unsnapped edges and analytic (pixel-area) coverage: a rectangle at fractional points keeps its exact area,
    /// its edge pixels partly covered.
    ///
    /// A rectangle that is empty or not finite draws nothing.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{geom::Rgba, Fill};
    /// const HAIRLINE: Fill = Fill::new(Rgba::rgb(20, 20, 20)).fractional();
    /// assert_ne!(HAIRLINE, Fill::new(Rgba::rgb(20, 20, 20)));
    /// ```
    #[inline] pub const fn fractional(self) -> Fill { Fill { fractional: true, ..self } }
}

/// How a line or an outline is drawn: its colour and its width in points (1 by default).
///
/// A `Copy` value made with const builders: `const RULE: Stroke = Stroke::new(Rgba::token(LINE)).width(1.5);`. Line ends are butt caps.
/// The colour may be an app colour token, resolved by the painter as `Fill`'s is. One width rule for every shape that takes a stroke: a
/// width that is not finite, not positive or past `Stroke::MAX_WIDTH` draws nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub(crate) color: Rgba,
    pub(crate) width: f32,
}

impl Stroke {
    /// One point of opaque black.
    pub const DEFAULT: Stroke = Stroke::new(Rgba::rgb(0, 0, 0));
    /// The widest stroke, in points, a shape draws (the largest finite half-precision value instances store).
    pub const MAX_WIDTH: f32 = 65504.0;
    /// A one-point stroke of `color` (a colour or a colour token).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{geom::Rgba, Stroke};
    /// const EDGE: Stroke = Stroke::new(Rgba::rgb(20, 20, 20));
    /// assert_eq!(EDGE, Stroke::new(Rgba::rgb(20, 20, 20)).width(1.0));
    /// ```
    #[inline] pub const fn new(color: Rgba) -> Stroke { Stroke { color, width: 1.0 } }
    /// The same stroke `width` points wide; a shape given a width that is not finite, not positive or past `MAX_WIDTH` draws nothing.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{geom::Rgba, Stroke};
    /// const BOLD: Stroke = Stroke::new(Rgba::rgb(20, 20, 20)).width(2.0);
    /// assert_ne!(BOLD, Stroke::new(Rgba::rgb(20, 20, 20)));
    /// ```
    #[inline] pub const fn width(self, width: f32) -> Stroke { Stroke { width, ..self } }
    /// Whether a shape draws with this stroke: its width is finite, positive and at most `MAX_WIDTH`.
    #[inline] fn drawn(&self) -> bool { self.width.is_finite() && self.width > 0.0 && self.width <= Self::MAX_WIDTH }
}

impl Paint<'_> {
    /// `color`, or the colour its token (`Rgba::token`) names in this painter's theme.
    #[inline]
    fn ink(&self, color: Rgba) -> Rgba {
        if self.capture_palette { return color; }
        match color.as_token() {
            // the token's opacity (`Rgba::alpha`, final): its green byte
            Some(token) => { let e = self.color(token); Rgba { a: ((u32::from(e.a) * u32::from(color.g) + 127) / 255) as u8, ..e } }
            None => color,
        }
    }
    /// The width and resolved colour of `stroke`, or `None` when the stroke draws nothing (the width rule on `Stroke`).
    #[inline]
    fn line_of(&self, stroke: Stroke) -> Option<(f32, Rgba)> { stroke.drawn().then(|| (stroke.width, self.ink(stroke.color))) }
}

// The shapes as inherent methods: an app needs no trait import. Each draws through the one definition in `pxui_types::shape`, so the
// wrapper's instances are the engine's bytes.
impl Paint<'_> {
    /// Fills `rect` with `fill`: snapped to whole device pixels, or with exact fractional coverage (`Fill::fractional`).
    ///
    /// An empty rectangle draws nothing.
    /// Allocates: never when warm.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{geom::{Rect, Rgba}, Fill};
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("box", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| {
    ///         paint.fill(rect, Fill::new(Rgba::rgb(240, 240, 240)));
    ///         paint.fill(Rect::new(rect.x, rect.y + rect.h - 0.5, rect.w, 0.5), Fill::new(Rgba::rgb(0, 0, 0)).fractional());
    ///     });
    /// });
    /// ```
    #[inline]
    pub fn fill(&mut self, rect: Rect, fill: Fill) {
        let (Rect { x, y, w, h }, color) = (rect, self.ink(fill.color));
        if fill.fractional { Shapes::fill_fractional(&mut self.w, x, y, w, h, color) } else { Shapes::fill(&mut self.w, x, y, w, h, color) }
    }
    /// Strokes the edges of `rect` with a band `stroke.width` wide centred on them (half outside the rectangle), snapped to device pixels.
    ///
    /// An empty rectangle draws nothing. For a border drawn inside the box use `frame`.
    /// Allocates: never when warm.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{geom::Rgba, Stroke};
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("box", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| paint.stroke(rect, Stroke::new(Rgba::rgb(0, 0, 0)).width(2.0)));
    /// });
    /// ```
    #[inline] pub fn stroke(&mut self, rect: Rect, stroke: Stroke) { if let Some((width, color)) = self.line_of(stroke) { Shapes::stroke_w(&mut self.w, rect.x, rect.y, rect.w, rect.h, width, color) } }
    /// Draws a border `stroke.width` wide inside `rect`, as CSS draws one: a box of whole points gets a border of whole points.
    ///
    /// A rectangle narrower than twice the width draws as a filled band.
    /// Allocates: never when warm.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{geom::Rgba, Stroke};
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("box", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| paint.frame(rect, Stroke::new(Rgba::rgb(0, 0, 0))));
    /// });
    /// ```
    #[inline] pub fn frame(&mut self, rect: Rect, stroke: Stroke) { if let Some((width, color)) = self.line_of(stroke) { Shapes::frame_w(&mut self.w, rect.x, rect.y, rect.w, rect.h, width, color) } }
    /// Draws four corner brackets `offset` points outside `rect`, each arm `length` points long, in `stroke` (a selection mark).
    ///
    /// Allocates: never when warm.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{geom::Rgba, Stroke};
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("box", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| paint.brackets(rect, 3.0, 8.0, Stroke::new(Rgba::rgb(0, 0, 0))));
    /// });
    /// ```
    #[inline] pub fn brackets(&mut self, rect: Rect, offset: f32, length: f32, stroke: Stroke) { if let Some((width, color)) = self.line_of(stroke) { Shapes::brackets(&mut self.w, rect.x, rect.y, rect.w, rect.h, offset, length, width, color) } }
    /// Outlines `rect` with dashes 4 points on and 3 off, drawn in `stroke` (a zone, a drop target).
    ///
    /// Allocates: never when warm.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("zone", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| paint.dashed_rect(rect, pxui::Stroke::new(pxui::geom::Rgba::rgb(0, 0, 0))));
    /// });
    /// ```
    #[inline] pub fn dashed_rect(&mut self, rect: Rect, stroke: Stroke) { if let Some((width, color)) = self.line_of(stroke) { Shapes::dashed_rect(&mut self.w, rect.x, rect.y, rect.w, rect.h, width, color) } }
    /// Draws a straight line from `from` to `to` in `stroke`, butt-capped; a horizontal or vertical line is an exact pixel-snapped band.
    ///
    /// Allocates: never when warm.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{geom::Rgba, Stroke};
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("rule", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| paint.line((rect.x, rect.y), (rect.x + rect.w, rect.y), Stroke::new(Rgba::rgb(0, 0, 0))));
    /// });
    /// ```
    #[inline] pub fn line(&mut self, from: (f32, f32), to: (f32, f32), stroke: Stroke) { if let Some((width, color)) = self.line_of(stroke) { Shapes::line_w(&mut self.w, from, to, width, color) } }
    /// Draws a line of 4-point dashes every 7 points from `from` to `to` in `stroke`; a zero-length line draws nothing.
    ///
    /// Allocates: never when warm.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{geom::Rgba, Stroke};
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("rule", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| paint.dashed((rect.x, rect.y), (rect.x + 40.0, rect.y), Stroke::new(Rgba::rgb(0, 0, 0))));
    /// });
    /// ```
    #[inline] pub fn dashed(&mut self, from: (f32, f32), to: (f32, f32), stroke: Stroke) { if let Some((width, color)) = self.line_of(stroke) { Shapes::dashed_w(&mut self.w, from, to, width, color) } }
    /// Fills the triangle `a`, `b`, `c` with `fill`'s colour as one unsnapped, antialiased instance; a degenerate or non-finite
    /// triangle draws nothing.
    ///
    /// A triangle's edges are always exact (as `Fill::fractional`), whichever the fill says.
    /// Allocates: never when warm.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("play", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| paint.triangle((rect.x, rect.y), (rect.x, rect.y + 10.0), (rect.x + 9.0, rect.y + 5.0), pxui::Fill::new(pxui::geom::Rgba::rgb(0, 0, 0))));
    /// });
    /// ```
    #[inline] pub fn triangle(&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), fill: Fill) { let color = self.ink(fill.color); Shapes::triangle(&mut self.w, a, b, c, color) }
    /// Draws the quadratic Bezier from `from` through `control` to `to` in `stroke`, with round caps, unsnapped and antialiased.
    ///
    /// A stroke width outside 0 (exclusive) to 65504, or a non-finite point, draws nothing.
    /// Allocates: never when warm.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("arc", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| paint.curve((rect.x, rect.y), (rect.x + 5.0, rect.y + 10.0), (rect.x + 10.0, rect.y), pxui::Stroke::new(pxui::geom::Rgba::rgb(0, 0, 0)).width(1.5)));
    /// });
    /// ```
    #[inline] pub fn curve(&mut self, from: (f32, f32), control: (f32, f32), to: (f32, f32), stroke: Stroke) { if let Some((width, color)) = self.line_of(stroke) { Shapes::curve(&mut self.w, from, control, to, width, color) } }
    /// Draws the cubic Bezier through `points` (start, two controls, end) in `stroke`, as four midpoint-matched quadratics, without a
    /// tessellation buffer.
    ///
    /// Suited to icon-sized paths; the width and point rules are `curve`'s.
    /// Allocates: never when warm.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("wave", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, r| paint.cubic([(r.x, r.y), (r.x + 4.0, r.y + 8.0), (r.x + 8.0, r.y - 8.0), (r.x + 12.0, r.y)], pxui::Stroke::new(pxui::geom::Rgba::rgb(0, 0, 0)).width(1.5)));
    /// });
    /// ```
    #[inline] pub fn cubic(&mut self, points: [(f32, f32); 4], stroke: Stroke) { if let Some((width, color)) = self.line_of(stroke) { Shapes::cubic(&mut self.w, points, width, color) } }
    /// Draws a disc of `radius` centred on `at`, filled with `fill` and/or edged by `stroke` centred on its edge, as one antialiased
    /// instance.
    ///
    /// With neither it draws nothing, as does a radius that is not positive or a stroke the width rule refuses (`Stroke`); the edge is
    /// always exact (as `Fill::fractional`).
    /// Allocates: never when warm.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("dot", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| paint.circle((rect.x + 6.0, rect.y + 6.0), 4.0, Some(pxui::Fill::new(pxui::geom::Rgba::rgb(0, 0, 0))), None));
    /// });
    /// ```
    #[inline] pub fn circle(&mut self, at: (f32, f32), radius: f32, fill: Option<Fill>, stroke: Option<Stroke>) {
        let edge = match stroke { Some(stroke) => match self.line_of(stroke) { Some(edge) => Some(edge), None => return }, None => None };
        let fill = fill.map(|fill| self.ink(fill.color));
        if fill.is_some() || edge.is_some() { Shapes::circle(&mut self.w, at, radius, fill, edge) }
    }
    /// Draws `dot`-point squares in `fill`'s colour every `spacing` points from `origin` inside `rect`, as one instance whatever the
    /// number of dots.
    ///
    /// An empty rectangle or a spacing that is not positive draws nothing.
    /// Allocates: never when warm.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("grid", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| paint.grid(rect, (rect.x, rect.y), 8.0, 1.0, pxui::Fill::new(pxui::geom::Rgba::rgb(0, 0, 0))));
    /// });
    /// ```
    #[inline] pub fn grid(&mut self, rect: Rect, origin: (f32, f32), spacing: f32, dot: f32, fill: Fill) { let color = self.ink(fill.color); Shapes::grid(&mut self.w, rect.x, rect.y, rect.w, rect.h, origin, spacing, dot, color) }
    /// Draws a chevron pointing `direction` in `stroke`, centred on `at`: with a one-point stroke, 9 by 5.5 points of ink.
    ///
    /// Allocates: never when warm.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("more", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| paint.chevron((rect.x + 6.0, rect.y + 6.0), pxui::ui::Dir::Down, pxui::Stroke::new(pxui::geom::Rgba::rgb(0, 0, 0))));
    /// });
    /// ```
    #[inline] pub fn chevron(&mut self, at: (f32, f32), direction: Dir, stroke: Stroke) { if let Some((width, color)) = self.line_of(stroke) { Shapes::chevron(&mut self.w, at, direction, width, color) } }
    /// Draws a 12-point check flag centred on `at` in the theme's input colours: square, or `round` (a radio), marked when `on`.
    ///
    /// Allocates: never when warm.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("flag", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| paint.flag((rect.x + 6.0, rect.y + 6.0), false, true));
    /// });
    /// ```
    #[inline] pub fn flag(&mut self, at: (f32, f32), round: bool, on: bool) { Shapes::flag(&mut self.w, at, round, on) }
}

impl Shapes for Writer {
    #[inline]
    fn push(&mut self, instance: Instance) {
        // SAFETY: `span` is the window `pxui_paint_span` handed out (or empty, which writes nothing)
        if !unsafe { self.span.write(instance) } {
            self.refill();
            unsafe { self.span.write(instance); }
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

/// A canvas closure as the library keeps it: the user pointer is a pooled box that starts with this erased pointer to the closure
/// stored after it in the same box (`boxed`), so drawing and releasing it are not generic; the library releases it exactly once.
type Canvas = dyn FnMut(&mut Paint<'_>, Rect) + 'static;

extern "C" fn draw(user: *mut c_void, raw: *mut RawPaint, rect: Rect, info: *const PxuiPaintInfo) {
    if deferred() { return; }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(mut paint) = (unsafe { info.as_ref().and_then(|info| Paint::from_raw(raw, info)) }) else { return };
        (unsafe { &mut **user.cast::<*mut Canvas>() })(&mut paint, rect);
        paint.flush();
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}

/// The C callback of an `Icon::Draw`: `user` is the app's drawing function itself (a plain `fn` pointer, so nothing to release); the
/// icon's word is the painter's (`pxui_paint_icon_word`).
pub(crate) extern "C" fn icon_draw(user: *mut c_void, raw: *mut c_void, rect: Rect, ink: Rgba, info: *const PxuiPaintInfo) {
    if deferred() { return; }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let word = unsafe { lib::pxui_paint_icon_word(raw.cast()) };
        let Some(mut paint) = (unsafe { info.as_ref().and_then(|info| Paint::from_raw(raw.cast(), info)) }) else { return };
        let f = unsafe { core::mem::transmute::<*mut c_void, fn(&mut Paint<'_>, Rect, Rgba, u32)>(user) };
        f(&mut paint, rect, ink, word);
        paint.flush();
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}

// A boxed closure (a canvas, a hit shape) is released when the library lets go of it. Released storage waits in a small pool and
// the next closure of the same layout (a canvas rebuilt each frame is always the same type) takes it: a settled frame allocates
// nothing. A zero-sized closure has no storage at all.
// ponytail: at most POOLED boxes per layout and thread, never returned to the allocator at thread exit; layouts are the distinct closure types, so a bounded handful.
const POOLED: usize = 64;
thread_local! { static POOL: std::cell::RefCell<Vec<(Layout, Vec<*mut u8>)>> = const { std::cell::RefCell::new(Vec::new()) }; }

/// Storage for a value of `layout`: a pooled box, else a new allocation (a dangling, aligned pointer for a zero-sized value).
pub(crate) fn pool_take(layout: Layout) -> *mut u8 {
    if layout.size() == 0 { return layout.align() as *mut u8; }
    // `try_with`: after this thread's locals are torn down (a release at thread exit) the pool is simply absent
    let spare = POOL.try_with(|pool| pool.borrow_mut().iter_mut().find(|(l, _)| *l == layout).and_then(|(_, boxes)| boxes.pop())).ok().flatten();
    let at = spare.unwrap_or_else(|| unsafe { std::alloc::alloc(layout) });
    if at.is_null() { std::alloc::handle_alloc_error(layout); }
    at
}

/// Gives the storage of a value of `layout`, already dropped, back to the pool (or the allocator).
pub(crate) fn pool_give(layout: Layout, at: *mut u8) {
    if layout.size() == 0 { return; }
    let kept = POOL.try_with(|pool| {
        let mut pool = pool.borrow_mut();
        let boxes = match pool.iter().position(|(l, _)| *l == layout) { Some(i) => &mut pool[i].1, None => { pool.push((layout, Vec::new())); &mut pool.last_mut().expect("just pushed").1 } };
        (boxes.len() < POOLED).then(|| boxes.push(at)).is_some()
    }).unwrap_or(false);
    if !kept { unsafe { std::alloc::dealloc(at, layout) } }
}

/// What a closure's `Drop` panicked with is kept to resume at the next call that can: it must not unwind into the library.
pub(crate) fn dropped(result: std::thread::Result<()>) { if let Err(panic) = result { defer_callback_panic(panic); } }

/// The box of a closure of layout `value`: the erased pointer, then the closure at the returned offset.
fn canvas_box(value: Layout) -> (Layout, usize) {
    let (layout, at) = Layout::new::<*mut Canvas>().extend(value).expect("a closure's layout fits a box");
    (layout.pad_to_align(), at)
}

/// The one generic piece of `canvas`: `f` moved into a pooled box behind its own erased pointer (see `Canvas`), one copy per closure type.
fn boxed<F: FnMut(&mut Paint<'_>, Rect) + 'static>(f: F) -> *mut c_void {
    let (layout, at) = canvas_box(Layout::new::<F>());
    let base = pool_take(layout);
    unsafe {
        let value = base.add(at).cast::<F>();
        value.write(f);
        base.cast::<*mut Canvas>().write(value as *mut Canvas);
    }
    base.cast()
}

extern "C" fn release(user: *mut c_void) {
    let erased = unsafe { *user.cast::<*mut Canvas>() };
    let (layout, _) = canvas_box(unsafe { Layout::for_value(&*erased) });
    dropped(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { core::ptr::drop_in_place(erased) })));
    pool_give(layout, user.cast());
}

impl crate::Ui {
    /// The one body of `canvas` and `canvas_over`: hands the boxed closure (`boxed`) to the library, under (`over` 0) or over (1) the children.
    #[track_caller]
    fn canvas_at(&mut self, b: &Handle, over: u8, user: *mut c_void) {
        crate::check(self, unsafe { lib::pxui_ui_draw(self.0.as_ptr(), b, over, Some(draw), user, Some(release)) });
    }
    /// Draws `f` into node `b` each time it paints, given the node's final laid-out rectangle: after its look, before its children.
    ///
    /// `f` is kept (boxed in a pooled slot) until the library lets go of it; it must not call the `Ui`, and a panic in it surfaces at
    /// the frame call. Shapes and text land in call order.
    /// Allocates: never when warm (a closure of a type seen before reuses its pooled box).
    /// Panics: when the library refuses the call (a stale box, outside a frame).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("canvas", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, |paint, rect| paint.fill(rect, pxui::Fill::new(pxui::geom::Rgba::rgb(0, 0, 0))));
    /// });
    /// ```
    #[inline] #[track_caller] pub fn canvas<F: FnMut(&mut Paint<'_>, Rect) + 'static>(&mut self, b: &Handle, f: F) { self.canvas_at(b, 0, boxed(f)) }
    /// Like `canvas`, but `f` draws after the node's children, outside its content clip (an underline, a focus ring, an overlay).
    ///
    /// Allocates: as `canvas`.
    /// Panics: as `canvas`.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("row", pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas_over(&node, |paint, r| paint.fill(pxui::geom::Rect::new(r.x, r.y + r.h - 1.0, r.w, 1.0), pxui::Fill::new(pxui::geom::Rgba::rgb(0, 0, 0))));
    /// });
    /// ```
    #[inline] #[track_caller] pub fn canvas_over<F: FnMut(&mut Paint<'_>, Rect) + 'static>(&mut self, b: &Handle, f: F) { self.canvas_at(b, 1, boxed(f)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static DROPS: AtomicUsize = AtomicUsize::new(0);
    struct Counted;
    impl Drop for Counted { fn drop(&mut self) { DROPS.fetch_add(1, Ordering::SeqCst); } }
    struct Bomb;
    impl Drop for Bomb { fn drop(&mut self) { panic!("drop bomb"); } }

    #[test]
    fn a_zero_sized_closure_is_dropped_once_and_a_panicking_drop_stays_out_of_c() {
        let counted = Counted;
        let f = move |_: &mut Paint<'_>, _: Rect| { let _ = &counted; };
        assert_eq!(core::mem::size_of_val(&f), 0, "the closure owns a zero-sized value");
        let user = boxed(f);
        release(user);
        assert_eq!(DROPS.load(Ordering::SeqCst), 1, "dropped exactly once");

        let bomb = Bomb;
        let g = move |_: &mut Paint<'_>, _: Rect| { let _ = &bomb; };
        let user = boxed(g);
        release(user);
        let panic = crate::composite::take_callback_panic().expect("the panic of Drop is kept, not unwound into C");
        assert_eq!(panic.downcast_ref::<&str>(), Some(&"drop bomb"));
    }

    /// One layout filling its share of the pool (a graph of many cards) leaves the others theirs: a canvas of another type
    /// still finds its box again instead of allocating every frame.
    #[test]
    fn a_full_pool_of_one_layout_still_pools_another() {
        let (many, few) = (Layout::new::<[u64; 38]>(), Layout::new::<[u64; 3]>());
        let held: Vec<_> = (0..POOLED + 6).map(|_| pool_take(many)).collect();
        for at in held { pool_give(many, at); }
        let one = pool_take(few);
        pool_give(few, one);
        assert_eq!(pool_take(few), one, "the second layout's box came back from the pool");
    }

    use crate::FrameIn;
    use pxui_types::{Node, Size};

    const INK: Rgba = Rgba::rgb(1, 2, 3);
    const NARROW: Rect = Rect::new(10.0, 10.0, 30.0, 20.0);

    fn frame_with(ui: &mut crate::Ui, canvas: fn(&mut Paint<'_>, Rect)) -> Result<(), crate::Error> {
        let mut canvas = Some(canvas);
        ui.frame(&FrameIn::new(200.0, 100.0), &mut |ui: &mut crate::Ui| {
            let node = ui.node("canvas", Node::DEFAULT.w(Size::Px(80.0)).h(Size::Px(40.0)));
            if let Some(canvas) = canvas.take() { ui.canvas(&node, canvas); }
        }).map(|_| ())
    }

    /// A clip changed mid-drawing (as `grid` narrows it) with more shapes under it than one span holds: every refilled span is
    /// stamped with the new clip, and the old clip comes back after.
    #[test]
    fn a_clip_set_mid_drawing_restamps_every_refilled_span() {
        let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        frame_with(&mut ui, move |p, _| {
            let saved = Shapes::clip_rect(&p.w);
            Shapes::set_clip_rect(&mut p.w, NARROW);
            for i in 0..3000 { p.fill(Rect::new((i % 40) as f32, (i / 40) as f32 * 0.5, 1.0, 1.0), Fill::new(INK)); }
            Shapes::set_clip_rect(&mut p.w, saved);
            p.fill(Rect::new(0.0, 0.0, 4.0, 4.0), Fill::new(INK));
        }).expect("frame");
        let clips: Vec<_> = ui.layers().flat_map(|layer| layer.instances.iter().map(|i| i.clip)).collect();
        assert_eq!(clips.iter().filter(|c| **c == clip_of(NARROW)).count(), 3000, "every shape under the NARROW clip carries it");
        assert_ne!(*clips.last().unwrap(), clip_of(NARROW), "the saved clip is restored");
    }

    #[test]
    fn a_span_left_open_or_a_library_call_under_one_is_an_error_not_a_silent_drop() {
        let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        // a library call made while a span is open (the wrapper's own calls commit first; this one bypasses it)
        let rc = frame_with(&mut ui, move |p, _| {
            p.fill(Rect::new(0.0, 0.0, 4.0, 4.0), Fill::new(INK));
            unsafe { lib::pxui_paint_label_chars(p.w.raw.as_ptr(), 0.0, 0.0, &PxuiLabelStyle::from(&Label::BODY), crate::as_str("x")) };
        });
        assert_eq!(rc.err(), Some(crate::Error::State));
        assert!(ui.last_error().contains("span is open"), "{}", ui.last_error());
        // a callback that returns with its span uncommitted (its shapes would be dropped)
        let rc = frame_with(&mut ui, move |p, _| { p.fill(Rect::new(0.0, 0.0, 4.0, 4.0), Fill::new(INK)); p.w.open = false; });
        assert_eq!(rc.err(), Some(crate::Error::State));
        assert!(ui.last_error().contains("uncommitted"), "{}", ui.last_error());
        // a panic with a span open is carried to the frame, and the next frame draws cleanly
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { let _ = frame_with(&mut ui, move |p, _| { p.fill(Rect::new(0.0, 0.0, 4.0, 4.0), Fill::new(INK)); panic!("with a span open"); }); }));
        assert!(result.is_err());
        assert!(frame_with(&mut ui, move |p, _| p.fill(Rect::new(0.0, 0.0, 4.0, 4.0), Fill::new(INK))).is_ok(), "the Ui draws again");
    }
}
