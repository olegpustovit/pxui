//! Text shaping outside the widget tree. A `Text` is a shaper of its own for text laid out off the `Ui` (on any thread): it measures
//! and rasterizes to RGBA. `Ui::text_service` is the `Ui`'s own: the one font system and glyph atlas it draws with, whose glyph quads
//! sample the atlases `Renderer::atlas` hands out.
use crate::sys::{PxuiPreparedText, PxuiShaper, PxuiTextWorker, PxuiUi};
use crate::{as_str, Error, Ui};
use core::ptr::NonNull;
use pxui_types::{Label, Str, TextGlyph, TextMetrics, TextSpan, TextSpans, TextStyle};

library!(lib {
    fn pxui_text_fork(ui:*mut PxuiUi)->*mut PxuiShaper;
    fn pxui_shaper_prepare(shaper:*mut PxuiShaper,value:Str,style:*const TextStyle,max_width:f32,spans:*const TextSpans,out:*mut *mut PxuiPreparedText,out_metrics:*mut TextMetrics)->i32;
    fn pxui_text_install(ui:*mut PxuiUi,record:*mut PxuiPreparedText)->i32;
    fn pxui_text_prewarm(ui:*mut PxuiUi,value:Str,style:*const TextStyle,max_width:f32)->i32;
    fn pxui_text_worker_create()->*mut PxuiTextWorker;
    fn pxui_text_worker_destroy(worker:*mut PxuiTextWorker);
    fn pxui_ui_set_text_worker(ui:*mut PxuiUi,worker:*const PxuiTextWorker)->i32;
    fn pxui_prepared_destroy(record:*mut PxuiPreparedText);
    fn pxui_shaper_system_fonts(shaper:*mut PxuiShaper)->i32;
    fn pxui_text_system_fonts(ui:*mut PxuiUi)->i32;
    fn pxui_shaper_width_spans(shaper:*mut PxuiShaper,key:u64,value:Str,style:*const TextStyle,max_width:f32,spans:*const TextSpans,out:*mut TextMetrics)->i32;
    fn pxui_text_width_spans(ui:*mut PxuiUi,key:u64,value:Str,style:*const TextStyle,max_width:f32,spans:*const TextSpans,out:*mut TextMetrics)->i32;
    fn pxui_ui_text_line_box(ui:*mut PxuiUi,box_:*const crate::PxuiBox,value:Str,style:*const TextStyle,spans:*const TextSpans)->i32;
    fn pxui_ui_text_spans(ui:*mut PxuiUi,box_:*const crate::PxuiBox,value:Str,style:*const TextStyle,spans:*const TextSpans)->i32;
    fn pxui_ui_label_node(ui:*mut PxuiUi,text:Str,style:*const pxui_types::ffi::PxuiLabelStyle,label_box:*const pxui_types::ffi::PxuiLabelBox,out:*mut crate::PxuiBox)->i32;
    fn pxui_ui_resolve_label(ui:*mut PxuiUi,style:*const pxui_types::ffi::PxuiLabelStyle,out:*mut TextStyle)->i32;
    fn pxui_default_font() -> Str;
    fn pxui_shaper_create(fonts: *const Str, count: usize) -> *mut PxuiShaper;
    fn pxui_shaper_destroy(shaper: *mut PxuiShaper);
    fn pxui_shaper_add_font(shaper: *mut PxuiShaper, font: Str) -> u32;
    fn pxui_shaper_width(shaper: *mut PxuiShaper, key: u64, value: Str, style: *const TextStyle, max_width: f32, out: *mut TextMetrics) -> i32;
    fn pxui_shaper_rasterize_rgba(shaper: *mut PxuiShaper, key: u64, value: Str, style: *const TextStyle, max_width: f32, scale: f32, color_r: u8, color_g: u8, color_b: u8, color_a: u8, width: *mut u32, height: *mut u32, out: *mut u8, capacity: usize) -> usize;
    fn pxui_text_add_font(ui: *mut PxuiUi, font: Str) -> u32;
    fn pxui_text_width(ui: *mut PxuiUi, key: u64, value: Str, style: *const TextStyle, max_width: f32, out: *mut TextMetrics) -> i32;
    fn pxui_text_quads(ui: *mut PxuiUi, key: u64, value: Str, style: *const TextStyle, max_width: f32, x: f32, y: f32, scale: f32, out: *mut TextGlyph, capacity: usize) -> usize;
    fn pxui_text_rasterize_rgba(ui: *mut PxuiUi, key: u64, value: Str, style: *const TextStyle, max_width: f32, scale: f32, color_r: u8, color_g: u8, color_b: u8, color_a: u8, width: *mut u32, height: *mut u32, out: *mut u8, capacity: usize) -> usize;
});

/// The font the library embeds (Pragmasevka), as bytes for `Text::new` or `UiText::add_font`; valid for the life of the process.
/// Allocates: never.
/// Panics: never.
///
/// ```
/// assert!(!pxui::text::pragmasevka().is_empty());
/// ```
pub fn pragmasevka() -> &'static [u8] {
    let font = unsafe { lib::pxui_default_font() };
    unsafe { core::slice::from_raw_parts(font.ptr, font.len) }
}

/// What a shaping call talks to: a shaper of its own or a `Ui`'s text service.
#[derive(Clone, Copy)]
enum Backing {
    Shaper(*mut PxuiShaper),
    Ui(*mut PxuiUi),
}

impl Backing {
    fn add_font(self, bytes: &[u8]) -> Option<u32> {
        let font = Str { ptr: bytes.as_ptr(), len: bytes.len() };
        let index = unsafe { match self { Backing::Shaper(s) => lib::pxui_shaper_add_font(s, font), Backing::Ui(u) => lib::pxui_text_add_font(u, font) } };
        (index != u32::MAX).then_some(index)
    }

    fn measure(self, key: u64, value: &str, style: &TextStyle, max_width: f32) -> Result<TextMetrics, Error> {
        let mut out = TextMetrics::default();
        let rc = unsafe { match self {
            Backing::Shaper(s) => lib::pxui_shaper_width(s, key, as_str(value), style, max_width, &mut out),
            Backing::Ui(u) => lib::pxui_text_width(u, key, as_str(value), style, max_width, &mut out),
        } };
        if rc == 0 { Ok(out) } else { Err(Error::from_code(rc)) }
    }

    fn measure_spans(self,key:u64,value:&str,style:&TextStyle,max_width:f32,spans:&[TextSpan])->Result<TextMetrics,Error> {
        let spans=TextSpans::new(spans).ok_or(Error::InvalidInput)?;
        let mut out=TextMetrics::default();
        let rc=unsafe { match self {
            Backing::Shaper(s)=>lib::pxui_shaper_width_spans(s,key,as_str(value),style,max_width,&spans,&mut out),
            Backing::Ui(u)=>lib::pxui_text_width_spans(u,key,as_str(value),style,max_width,&spans,&mut out),
        } };
        if rc==0 { Ok(out) } else { Err(Error::from_code(rc)) }
    }

    fn rasterize_rgba(self, key: u64, value: &str, style: &TextStyle, max_width: f32, scale: f32, color: [u8; 4], pixels: &mut Vec<u8>) -> Result<(u32, u32), Error> {
        let (mut width, mut height) = (0, 0);
        let mut call = |out, capacity| unsafe { match self {
            Backing::Shaper(s) => lib::pxui_shaper_rasterize_rgba(s, key, as_str(value), style, max_width, scale, color[0], color[1], color[2], color[3], &mut width, &mut height, out, capacity),
            Backing::Ui(u) => lib::pxui_text_rasterize_rgba(u, key, as_str(value), style, max_width, scale, color[0], color[1], color[2], color[3], &mut width, &mut height, out, capacity),
        } };
        pixels.clear();
        let required = call(core::ptr::null_mut(), 0);
        if required == usize::MAX { return Err(Error::InvalidInput); }
        pixels.try_reserve_exact(required).map_err(|_| Error::Allocation)?;
        pixels.resize(required, 0);
        if call(pixels.as_mut_ptr(), pixels.len()) != required { pixels.clear(); return Err(Error::Engine); }
        Ok((width, height))
    }
}

/// A text shaper of its own, owned by the library; layouts are cached by content.
///
/// A shaper may move to another thread (a worker owns its own), but it is not `Sync`: one thread at a time uses one shaper, and its
/// calls never overlap (the library keeps no thread-local state for it). A fork shares immutable font sources with its `Ui`.
pub struct Text(NonNull<PxuiShaper>);

// SAFETY: a shaper is a plain object of the library (its own font system and caches, no thread-local state, no pointer into the
// caller), so moving it between threads is fine as long as one thread uses it at a time, which `&mut self` and no `Sync` guarantee.
unsafe impl Send for Text {}

/// A worker-owned layout. Move it to its originating UI for installation between frames.
pub struct PreparedText {
    raw: NonNull<PxuiPreparedText>,
    metrics: TextMetrics,
}

/// Replaced UI storage. Return it to the worker for destruction; it cannot be installed again.
pub struct RetiredText(NonNull<PxuiPreparedText>);

// SAFETY: both records uniquely own library storage and immutable font-generation tokens, with no UI pointer or thread-local state.
unsafe impl Send for PreparedText {}
unsafe impl Send for RetiredText {}

impl PreparedText { pub fn metrics(&self)->TextMetrics { self.metrics } }
impl Drop for PreparedText { fn drop(&mut self) { unsafe { lib::pxui_prepared_destroy(self.raw.as_ptr()) } } }
impl Drop for RetiredText { fn drop(&mut self) { unsafe { lib::pxui_prepared_destroy(self.0.as_ptr()) } } }

impl Text {
    /// Shapes `value` in `style` within `max_width` points (0: unbounded) with up to eight `spans` on this shaper, ready to install into the `Ui` it was forked from; do the work on a worker, install between frames.
    /// Installation needs a shaper forked from that `Ui` (`UiText::fork`); adding fonts or discovering fallback fonts afterwards invalidates the delivery. At most 1 MiB of UTF-8 and 4 MiB of glyph-list storage per record; rasterization stays cold.
    /// Allocates: the record and its glyph-list storage, once per call.
    /// Panics: never.
    /// Errors: `Error::InvalidInput` for more than eight or overlapping spans or text over the limits; `Error::Engine` when shaping fails.
    ///
    /// ```
    /// let mut text = pxui::Text::new(&[pxui::text::pragmasevka()]).unwrap();
    /// let prepared = text.prepare("Song title", &Default::default(), 0.0, &[]).unwrap();
    /// assert!(prepared.metrics().width > 0.0);
    /// ```
    pub fn prepare(&mut self,value:&str,style:&TextStyle,max_width:f32,spans:&[TextSpan])->Result<PreparedText,Error> {
        let spans=TextSpans::new(spans).ok_or(Error::InvalidInput)?;
        let mut raw=core::ptr::null_mut();
        let mut metrics=TextMetrics::default();
        let rc=unsafe { lib::pxui_shaper_prepare(self.0.as_ptr(),as_str(value),style,max_width,&spans,&mut raw,&mut metrics) };
        if rc!=0 { return Err(Error::from_code(rc)); }
        Ok(PreparedText { raw:NonNull::new(raw).expect("pxui: preparation succeeded without storage"),metrics })
    }
    /// Enables the platform's font fallback for this shaper (Apple); the first missing character can discover and map a font file.
    /// Call it before the first use. A shaper with explicit fonts only stays deterministic without it.
    /// Allocates: nothing until a fallback font is first needed.
    /// Panics: never.
    /// Errors: `Error::State` once the shaper has been used or when the platform has no system fonts.
    ///
    /// ```
    /// let mut text = pxui::Text::new(&[pxui::text::pragmasevka()]).unwrap();
    /// let _ = text.enable_system_fonts(); // `Err(State)` where the platform has none
    /// ```
    pub fn enable_system_fonts(&mut self)->Result<(),Error> {
        let rc=unsafe { lib::pxui_shaper_system_fonts(self.0.as_ptr()) };
        if rc==0 { Ok(()) } else { Err(Error::from_code(rc)) }
    }
    /// Creates a shaper of its own holding `fonts` (each the bytes of a font file, copied), the first being the default; `None` when a font cannot be read.
    /// A `Text` may move to another thread, but one thread at a time uses it.
    /// Allocates: the font system and caches, once; the font bytes are copied.
    /// Panics: never.
    ///
    /// ```
    /// let text = pxui::Text::new(&[pxui::text::pragmasevka()]);
    /// assert!(text.is_some());
    /// ```
    pub fn new(fonts: &[&[u8]]) -> Option<Self> {
        let fonts: Vec<Str> = fonts.iter().map(|font| Str { ptr: font.as_ptr(), len: font.len() }).collect();
        NonNull::new(unsafe { lib::pxui_shaper_create(fonts.as_ptr(), fonts.len()) }).map(Self)
    }
    /// Adds the font `bytes` (copied) to this shaper and returns its index; `None` when the bytes are refused: empty, over 32 MiB, not a parseable font, or past 64 fonts or 256 MiB of fonts on this shaper (system fonts are separate: memory-mapped, demand-paged, 128 files, 256 MiB each, 1 GiB mapped).
    /// Allocates: a copy of the font bytes and its tables.
    /// Panics: never.
    ///
    /// ```
    /// let mut text = pxui::Text::new(&[pxui::text::pragmasevka()]).unwrap();
    /// assert!(text.add_font(pxui::text::pragmasevka()).is_some());
    /// assert!(text.add_font(b"not a font").is_none()); // not a parseable font
    /// ```
    pub fn add_font(&mut self, bytes: &[u8]) -> Option<u32> { Backing::Shaper(self.0.as_ptr()).add_font(bytes) }
    /// Lays `value` out in `style` within `max_width` points (0: unbounded) and returns its size and line count, without drawing.
    /// Layouts are cached by content, so measuring the same text and style again shapes nothing.
    /// Allocates: the first measurement of a text and style shapes and caches it; never for a cached one.
    /// Panics: never.
    /// Errors: `Error::InvalidInput` for a non-finite `max_width` or oversized text.
    ///
    /// ```
    /// let mut text = pxui::Text::new(&[pxui::text::pragmasevka()]).unwrap();
    /// let m = text.measure("Song title", &Default::default(), 0.0).unwrap();
    /// assert!(m.width > 0.0 && m.height > 0.0);
    /// ```
    pub fn measure(&mut self, value: &str, style: &TextStyle, max_width: f32) -> Result<TextMetrics, Error> { Backing::Shaper(self.0.as_ptr()).measure(0, value, style, max_width) }
    /// `measure` for one layout with up to eight ordered, disjoint UTF-8 byte ranges `spans` (a bold word shapes wider than a plain one).
    /// Allocates: as `measure`.
    /// Panics: never.
    /// Errors: `Error::InvalidInput` for bad spans (more than eight, overlapping, or off a character boundary) or oversized text.
    ///
    /// ```
    /// let mut text = pxui::Text::new(&[pxui::text::pragmasevka()]).unwrap();
    /// let bold = pxui::TextSpan { start: 0, end: 4, weight: 700, ..Default::default() };
    /// assert!(text.measure_spans("Song title", &Default::default(), 0.0, &[bold]).unwrap().width > 0.0);
    /// ```
    pub fn measure_spans(&mut self,value:&str,style:&TextStyle,max_width:f32,spans:&[TextSpan])->Result<TextMetrics,Error> { Backing::Shaper(self.0.as_ptr()).measure_spans(0,value,style,max_width,spans) }
    /// `measure` returning only the width in points.
    /// Allocates: as `measure`.
    /// Panics: never.
    /// Errors: as `measure`.
    ///
    /// ```
    /// let mut text = pxui::Text::new(&[pxui::text::pragmasevka()]).unwrap();
    /// assert!(text.width("Song title", &Default::default(), 0.0).unwrap() > 0.0);
    /// ```
    pub fn width(&mut self, value: &str, style: &TextStyle, max_width: f32) -> Result<f32, Error> { self.measure(value, style, max_width).map(|m| m.width) }
    /// The text as straight-alpha RGBA8 written into `out` (cleared first): width and height; `out` holds `width * height * 4` bytes.
    /// Reuses `out`'s capacity.
    ///
    /// Allocates: only when `out`'s capacity is too small.
    /// Panics: never.
    ///
    /// ```
    /// let mut text = pxui::Text::new(&[pxui::text::pragmasevka()]).unwrap();
    /// let mut pixels = Vec::new();
    /// let (w, h) = text.rasterize_rgba("abc", &Default::default(), 0.0, 1.0, [255; 4], &mut pixels).unwrap();
    /// assert_eq!(pixels.len(), (w * h * 4) as usize);
    /// ```
    ///
    /// Errors: InvalidInput for empty or oversized text or a non-finite size; Allocation when the pixels cannot be reserved; Engine when rendering fails.
    pub fn rasterize_rgba(&mut self, value: &str, style: &TextStyle, max_width: f32, scale: f32, color: [u8; 4], out: &mut Vec<u8>) -> Result<(u32, u32), Error> {
        Backing::Shaper(self.0.as_ptr()).rasterize_rgba(0, value, style, max_width, scale, color, out)
    }
}

impl Drop for Text { fn drop(&mut self) { unsafe { lib::pxui_shaper_destroy(self.0.as_ptr()) } } }

/// The prepare queue's worker: one thread, owned by the host, that shapes text ahead of need for every `Ui` attached to it
/// (`Ui::set_text_worker`). There is no hidden or per-`Ui` thread: a host with many windows attaches them all to one worker.
/// Cost: the thread, and per attached `Ui` a fork of its font database (font bytes shared) and fixed buffers (128 requests,
/// 16 KiB of text, 32 installs a frame); at most 64 `Ui`s at once. Dropping it waits for at most one shaping call; `Ui`s still
/// attached shape inline.
///
/// Threads: a `TextWorker` may be created, shared (`&TextWorker` from `Ui`s on several threads) and dropped on any thread; each
/// `Ui` attaches on its own thread. The borrow in `Ui::set_text_worker` keeps a drop from racing an attach.
pub struct TextWorker(NonNull<PxuiTextWorker>);

// SAFETY: the library's worker object is internally synchronized (its mailbox is behind a mutex; attaching clones a reference
// under it), and it holds no pointer into the caller or any `Ui`; dropping it needs `&mut`/ownership, which no attach can hold.
unsafe impl Send for TextWorker {}
unsafe impl Sync for TextWorker {}

impl TextWorker {
    /// Starts the worker's thread; `Error::Engine` when the platform cannot start one.
    /// Allocates: the thread and its mailbox, once.
    /// Panics: never.
    ///
    /// ```
    /// let worker = pxui::TextWorker::new();
    /// assert!(worker.is_ok());
    /// ```
    pub fn new()->Result<TextWorker,Error> {
        NonNull::new(unsafe { lib::pxui_text_worker_create() }).map(TextWorker).ok_or(Error::Engine)
    }
}

impl Drop for TextWorker { fn drop(&mut self) { unsafe { lib::pxui_text_worker_destroy(self.0.as_ptr()) } } }

/// The text service of a `Ui`: its one font system and glyph atlas.
pub struct UiText<'ui>(&'ui mut Ui);

impl Ui {
    /// Enables the platform's font fallback for this `Ui` (Apple) before its first frame; a `Ui` with explicit fonts stays deterministic without it.
    /// A first missing character can then discover and map a font file.
    /// Allocates: nothing until a fallback font is first needed.
    /// Panics: never.
    /// Errors: `Error::State` once a frame has run or when the platform has no system fonts.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new().unwrap();
    /// let _ = ui.enable_system_fonts(); // `Err(State)` where the platform has none
    /// ```
    pub fn enable_system_fonts(&mut self)->Result<(),Error> {
        let rc=unsafe { lib::pxui_text_system_fonts(self.0.as_ptr()) };
        if rc==0 { Ok(()) } else { Err(Error::from_code(rc)) }
    }
    /// The text style `style` shapes with: its unset fields the theme label's, its tracking in points. What `ui.label` (line-box
    /// text) would shape, so text measured or prepared with the result (`UiText`, a worker's `Text`) matches what the `Ui` draws.
    ///
    /// Label is the authoring style (theme-relative); `TextStyle` is the resolved style the shaper sees. `caps`, `color`, `width` and
    /// `pad` are the label's box and ink, not shaping, so they are not in the result.
    /// Allocates: never.
    /// Panics: when the style does not resolve (a size that is not positive, an unknown wrap, align or ellipsis), or when the `Ui` has
    /// failed.
    ///
    /// ```
    /// let ui = pxui::Ui::new().unwrap();
    /// const TITLE: pxui::Label = pxui::Label::BODY.size(20.0).tracking(-0.05);
    /// let style = ui.resolve(&TITLE);
    /// assert_eq!((style.size_points, style.weight, style.tracking), (20.0, 400, -0.05 * 20.0));
    /// ```
    #[track_caller]
    pub fn resolve(&self,style:&Label)->TextStyle {
        let mut out=TextStyle::default();
        crate::check(self,unsafe { lib::pxui_ui_resolve_label(self.0.as_ptr(),&pxui_types::ffi::PxuiLabelStyle::from(style),&mut out) });
        out
    }
    /// Sets a frame's node text in `style` with up to eight inline spans, preserving the complete accessibility label; the style
    /// resolves as `resolve` resolves it. Plain `text`/`patch_text` clears the spans.
    ///
    /// Allocates: never when warm; text it has not shaped before is shaped once.
    /// Panics: never (refusals are errors).
    ///
    /// ```
    /// const NAME: pxui::Label = pxui::Label::BODY.line_height(20.0);
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("name".into(), pxui::Node::DEFAULT);
    ///     let bold = pxui::TextSpan { start: 0, end: 4, weight: 700, ..Default::default() };
    ///     ui.text_spans(&node, "Song title", &NAME, &[bold]).unwrap();
    /// });
    /// ```
    ///
    /// Errors: InvalidInput for more than eight or overlapping spans or a style that does not resolve; Stale for a gone box.
    pub fn text_spans(&mut self,box_:&crate::PxuiBox,text:&str,style:&Label,spans:&[TextSpan])->Result<(),Error> {
        let spans=TextSpans::new(spans).ok_or(Error::InvalidInput)?;
        let style=self.resolved(style)?;
        let rc=unsafe { lib::pxui_ui_text_spans(self.0.as_ptr(),box_,as_str(text),&style,&spans) };
        if rc==0 { Ok(()) } else { Err(Error::from_code(rc)) }
    }
    /// CSS line boxes with primary-font baseline rounding for a frame's node text in `style` (resolved as `resolve`); requires a
    /// positive line height.
    ///
    /// Allocates: never when warm; text it has not shaped before is shaped once.
    /// Panics: never (refusals are errors).
    ///
    /// ```
    /// const NAME: pxui::Label = pxui::Label::BODY.size(15.0).line_height(21.0);
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("name".into(), pxui::Node::DEFAULT);
    ///     ui.text_line_box(&node, "Song title", &NAME, &[]).unwrap();
    /// });
    /// ```
    ///
    /// Errors: InvalidInput for a non-positive line height, bad spans or a style that does not resolve; Stale for a gone box.
    pub fn text_line_box(&mut self,box_:&crate::PxuiBox,text:&str,style:&Label,spans:&[TextSpan])->Result<(),Error> {
        let spans=TextSpans::new(spans).ok_or(Error::InvalidInput)?;
        let style=self.resolved(style)?;
        let rc=unsafe {lib::pxui_ui_text_line_box(self.0.as_ptr(),box_,as_str(text),&style,&spans)};
        if rc==0 {Ok(())} else {Err(Error::from_code(rc))}
    }
    /// Attaches this `Ui` to `worker`, the prepare queue's thread, which any number of `Ui`s share (`None` detaches). Attached,
    /// the `Ui`'s virtual lists (`list`, sectioned lists, grids; measured lists below the viewport only) request the unseen text
    /// of the rows they build past the viewport, and `prewarm_text` requests text: it is shaped on the worker and installed
    /// before a later frame builds, so scrolling does not shape on the UI thread. Without a worker the `Ui` shapes inline. The
    /// `Ui` keeps no reference to `worker`: dropping the worker first leaves the `Ui` shaping inline. A worker serves at most 64
    /// `Ui`s at once. Call it between frames, on this `Ui`'s thread (the worker may be shared with `Ui`s on other threads).
    /// Allocates: a fork of this `Ui`'s font database (font bytes shared) and its lane's fixed buffers (128 requests, 16 KiB of
    /// text), once per attach; detaching frees them on the worker without waiting.
    /// Panics: never.
    /// Errors: `Error::State` inside a frame, or when the worker serves 64 `Ui`s already (this `Ui` is left detached).
    ///
    /// ```
    /// let worker = pxui::TextWorker::new().unwrap();
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("x", pxui::Label::DEFAULT); });
    /// ui.set_text_worker(Some(&worker)).unwrap();
    /// ui.set_text_worker(None).unwrap();
    /// ```
    pub fn set_text_worker(&mut self,worker:Option<&TextWorker>)->Result<(),Error> {
        let worker=worker.map_or(core::ptr::null(),|worker|worker.0.as_ptr().cast_const());
        let rc=unsafe { lib::pxui_ui_set_text_worker(self.0.as_ptr(),worker) };
        if rc==0 { Ok(()) } else { Err(Error::from_code(rc)) }
    }
    /// Asks the attached worker (`set_text_worker`) for `text` in `style` (resolved as `resolve`) within `max_width` points
    /// (0: unbounded, as `UiText::width`), the layout a label draws when it shows that text, style and width: it is shaped on
    /// the worker and installed before a later frame builds, so that frame does not shape it. Call it for text a coming frame
    /// will show (rows ahead of a scroll). Without a worker, or for cached or already requested text, it does nothing and
    /// returns `Ok`; a full lane (128 requests in flight) drops the request. Text a fast fling shows before its delivery still
    /// shapes on the frame: the contract's one named exception.
    ///
    /// Natural width: a `max_width` of 0 prepares the unbounded layout, which a label drawn at any width of at least its natural
    /// one (the widest line the unbounded layout measures) shows, with no layout on the UI thread: no line is wider than the
    /// width, so no wrap or ellipsis can change it. Physically left-aligned text only (equal to the bounded layout up to float rounding); centred or
    /// right-aligned text (right-to-left text too) places its glyphs by the width, so the unbounded layout serves exactly the natural width (a drawn width less than the natural one, or any
    /// finite `max_width`, is its own layout, found only at that same width). Prewarm a cell drawn at its measured width with 0.
    /// Allocates: never (the worker's fork of the fonts is renewed once after a font change, at the end of that frame).
    /// Panics: never.
    /// Errors: `Error::InvalidInput` for a negative or non-finite `max_width`, text over 16 KiB, or a style that does not
    /// resolve.
    ///
    /// ```
    /// const TITLE: pxui::Label = pxui::Label::BODY.line_height(20.0).lines(1).ellipsis();
    /// let worker = pxui::TextWorker::new().unwrap();
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("Song title", TITLE); });
    /// ui.set_text_worker(Some(&worker)).unwrap();
    /// ui.prewarm_text("Next song title", &TITLE, 240.0).unwrap();
    /// assert!(ui.prewarm_text("x", &TITLE, -1.0).is_err());
    /// ```
    pub fn prewarm_text(&mut self,text:&str,style:&Label,max_width:f32)->Result<(),Error> {
        let style=self.resolved(style)?;
        let rc=unsafe { lib::pxui_text_prewarm(self.0.as_ptr(),as_str(text),&style,max_width) };
        if rc==0 { Ok(()) } else { Err(Error::from_code(rc)) }
    }
    /// `resolve` as a result: a style that does not resolve is InvalidInput.
    fn resolved(&self,style:&Label)->Result<TextStyle,Error> {
        let mut out=TextStyle::default();
        let rc=unsafe { lib::pxui_ui_resolve_label(self.0.as_ptr(),&pxui_types::ffi::PxuiLabelStyle::from(style),&mut out) };
        if rc==0 { Ok(out) } else { Err(Error::from_code(rc)) }
    }
    /// The `Ui`'s own text service: its font system and glyph atlas, for measuring, preparing and rasterizing outside a frame's build.
    /// Measurements through it share the caches the frames draw with.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("x", pxui::Label::DEFAULT); });
    /// assert!(ui.text_service().width("abc", &Default::default(), 0.0).unwrap() > 0.0);
    /// ```
    pub fn text_service(&mut self) -> UiText<'_> { UiText(self) }

    /// The label call of `label`, `label_fmt` and `label_digits`, returning the label's node: linked statically, the library's trusted
    /// entry with the style as it is; else `pxui_ui_label_node` with no record for `Label::DEFAULT` (every field unset: the C label), or
    /// the style as its record and, when it sets one, its box as a `PxuiLabelBox`.
    #[inline(always)]
    #[track_caller]
    pub(crate) fn label_call(&mut self, text: &str, style: &pxui_types::Label, digits: bool) -> crate::PxuiBox {
        let mut out = crate::PxuiBox::default();
        if style.variant != 0 { self.set_variant(pxui_types::theme::BodyKind::label, style.variant); }
        #[cfg(feature = "static")]
        let rc = unsafe { pxui_abi_link::trusted::pxui_ui_label(self.0.as_ptr(), as_str(text), style, digits, &mut out) };
        #[cfg(not(feature = "static"))]
        let rc = {
            let l = style;
            let boxed = l.height.is_some() || l.whole_pixels || l.role != pxui_types::a11y::Role::Label;
            let plain = !digits && !boxed && l.font.is_none() && l.size.is_none() && l.weight.is_none() && l.tracking.is_none() && l.caps.is_none()
                && l.color.is_none() && l.width.is_none() && l.line_height.is_none() && l.pad.is_none() && l.wrap == pxui_types::TextStyle::WRAP_NONE
                && l.max_lines == 0 && l.ellipsis == pxui_types::TextStyle::ELLIPSIS_NONE && l.align == pxui_types::TextStyle::ALIGN_LEFT && l.features == 0;
            if plain { unsafe { lib::pxui_ui_label_node(self.0.as_ptr(), as_str(text), core::ptr::null(), core::ptr::null(), &mut out) } }
            else {
                let label_box = pxui_types::ffi::PxuiLabelBox::from(style);
                let label_box = if boxed { &label_box as *const pxui_types::ffi::PxuiLabelBox } else { core::ptr::null() };
                unsafe { lib::pxui_ui_label_node(self.0.as_ptr(), as_str(text), &pxui_types::ffi::PxuiLabelStyle { digits: digits as u8, ..pxui_types::ffi::PxuiLabelStyle::from(style) }, label_box, &mut out) }
            }
        };
        if style.variant != 0 { self.set_variant(pxui_types::theme::BodyKind::label, 0); }
        crate::check(self, rc);
        out
    }
}

impl UiText<'_> {
    /// Between frames, clones the `Ui`'s font database into a `Text` for a worker, with a bounded cache for delivering layouts back; `None` when it cannot.
    /// The fork keeps this `Ui`'s font identities, so what it prepares installs here. Move it to the worker thread.
    /// Allocates: the cloned font sources and the receiving cache, once.
    /// Panics: never.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("x", pxui::Label::DEFAULT); });
    /// let worker_text = ui.text_service().fork();
    /// assert!(worker_text.is_some());
    /// ```
    pub fn fork(&mut self)->Option<Text> { NonNull::new(unsafe { lib::pxui_text_fork(self.0.0.as_ptr()) }).map(Text) }
    /// Between frames, adopts a layout a forked shaper prepared, without shaping or allocating; ordinary text with the same content, style and width then uses it.
    /// Glyph rasterization and atlas growth may still allocate. Success returns storage to hand back to the worker to destroy; failure returns the record with the error.
    /// Allocates: never.
    /// Panics: never.
    /// Errors: `Error::State` when the record was prepared by a shaper not forked from this `Ui`, or the fonts changed since; the record comes back with it.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("x", pxui::Label::DEFAULT); });
    /// let mut worker = ui.text_service().fork().unwrap();
    /// let prepared = worker.prepare("Song title", &Default::default(), 0.0, &[]).unwrap();
    /// let retired = ui.text_service().install(prepared);
    /// assert!(retired.is_ok()); // send the `RetiredText` back to the worker to drop
    /// ```
    pub fn install(&mut self,prepared:PreparedText)->Result<RetiredText,(Error,PreparedText)> {
        let rc=unsafe { lib::pxui_text_install(self.0.0.as_ptr(),prepared.raw.as_ptr()) };
        if rc!=0 { return Err((Error::from_code(rc),prepared)); }
        let raw=prepared.raw;
        core::mem::forget(prepared);
        Ok(RetiredText(raw))
    }
    fn backing(&self) -> Backing { Backing::Ui(self.0.0.as_ptr()) }
    /// Adds the font `bytes` (copied) to the `Ui`'s font system and returns its index; `None` when the bytes are refused: empty, over 32 MiB, not a parseable font, or past 64 fonts or 256 MiB of fonts on this `Ui` (system fonts are separate: memory-mapped, demand-paged, 128 files, 256 MiB each, 1 GiB mapped).
    /// Fonts that layouts prepared on a fork were shaped with are no longer valid afterwards.
    /// Allocates: a copy of the font bytes and its tables.
    /// Panics: never.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("x", pxui::Label::DEFAULT); });
    /// assert!(ui.text_service().add_font(pxui::text::pragmasevka()).is_some());
    /// ```
    pub fn add_font(&mut self, bytes: &[u8]) -> Option<u32> { self.backing().add_font(bytes) }
    /// `Text::measure` on the `Ui`'s font system: the size of `value` in `style` within `max_width` points (0: unbounded), without drawing.
    /// Allocates: the first measurement of a text and style shapes and caches it; never for a cached one.
    /// Panics: never.
    /// Errors: `Error::InvalidInput` for a non-finite `max_width` or oversized text.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("x", pxui::Label::DEFAULT); });
    /// let m = ui.text_service().measure("Song title", &Default::default(), 0.0).unwrap();
    /// assert!(m.width > 0.0);
    /// ```
    pub fn measure(&mut self, value: &str, style: &TextStyle, max_width: f32) -> Result<TextMetrics, Error> { self.backing().measure(0, value, style, max_width) }
    /// `Text::measure_spans` on the `Ui`'s font system: one layout with up to eight ordered, disjoint byte ranges `spans`.
    /// Allocates: as `measure`.
    /// Panics: never.
    /// Errors: `Error::InvalidInput` for bad spans or oversized text.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("x", pxui::Label::DEFAULT); });
    /// let bold = pxui::TextSpan { start: 0, end: 4, weight: 700, ..Default::default() };
    /// assert!(ui.text_service().measure_spans("Song title", &Default::default(), 0.0, &[bold]).unwrap().width > 0.0);
    /// ```
    pub fn measure_spans(&mut self,value:&str,style:&TextStyle,max_width:f32,spans:&[TextSpan])->Result<TextMetrics,Error> { self.backing().measure_spans(0,value,style,max_width,spans) }
    /// `measure` returning only the width in points.
    /// Allocates: as `measure`.
    /// Panics: never.
    /// Errors: as `measure`.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("x", pxui::Label::DEFAULT); });
    /// assert!(ui.text_service().width("Song title", &Default::default(), 0.0).unwrap() > 0.0);
    /// ```
    pub fn width(&mut self, value: &str, style: &TextStyle, max_width: f32) -> Result<f32, Error> { self.measure(value, style, max_width).map(|m| m.width) }
    /// The text as straight-alpha RGBA8 written into `out` (cleared first): width and height; `out` holds `width * height * 4` bytes.
    /// Reuses `out`'s capacity.
    ///
    /// Allocates: only when `out`'s capacity is too small.
    /// Panics: never.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("x", pxui::Label::DEFAULT); });
    /// let mut text = ui.text_service();
    /// let mut pixels = Vec::new();
    /// let (w, h) = text.rasterize_rgba("abc", &Default::default(), 0.0, 1.0, [255; 4], &mut pixels).unwrap();
    /// assert_eq!(pixels.len(), (w * h * 4) as usize);
    /// ```
    ///
    /// Errors: InvalidInput for empty or oversized text or a non-finite size; Allocation when the pixels cannot be reserved; Engine when rendering fails.
    pub fn rasterize_rgba(&mut self, value: &str, style: &TextStyle, max_width: f32, scale: f32, color: [u8; 4], out: &mut Vec<u8>) -> Result<(u32, u32), Error> {
        self.backing().rasterize_rgba(0, value, style, max_width, scale, color, out)
    }

    /// The glyph quads of `value`, positioned at `origin` and scaled by `scale`, written into `out` (cleared first); the count.
    /// Reuses `out`'s capacity.
    ///
    /// Allocates: only when `out`'s capacity is too small.
    /// Panics: never.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("x", pxui::Label::DEFAULT); });
    /// let mut glyphs = Vec::new();
    /// let n = ui.text_service().quads("abc", &Default::default(), 0.0, (0.0, 0.0), 1.0, &mut glyphs).unwrap();
    /// assert_eq!((n, glyphs.len()), (3, 3));
    /// ```
    ///
    /// Errors: InvalidInput for empty or oversized text or a non-finite size; Allocation when `out` cannot grow; Engine when the quads cannot be written.
    pub fn quads(&mut self, value: &str, style: &TextStyle, max_width: f32, origin: (f32, f32), scale: f32, out: &mut Vec<TextGlyph>) -> Result<usize, Error> {
        let ui = self.0.0.as_ptr();
        let call = |out, capacity| unsafe { lib::pxui_text_quads(ui, 0, as_str(value), style, max_width, origin.0, origin.1, scale, out, capacity) };
        out.clear();
        let count = call(core::ptr::null_mut(), 0);
        if count == usize::MAX { return Err(Error::InvalidInput); }
        out.try_reserve_exact(count).map_err(|_| Error::Allocation)?;
        out.resize(count, TextGlyph::default());
        let written = call(out.as_mut_ptr(), out.len());
        if written == usize::MAX { out.clear(); return Err(Error::Engine); }
        out.truncate(written.min(out.len()));
        Ok(out.len())
    }
}
