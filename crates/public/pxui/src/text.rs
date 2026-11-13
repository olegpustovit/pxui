//! Text shaping outside the widget tree. A `Text` is a shaper of its own for text laid out off the `Ui` (on any thread): it measures
//! and rasterizes to RGBA. `Ui::text_service` is the `Ui`'s own: the one font system and glyph atlas it draws with, whose glyph quads
//! sample the atlases `Renderer::atlas` hands out.
use crate::sys::{PxuiShaper, PxuiUi};
use crate::{as_str, Ui};
use core::ptr::NonNull;
use pxui_types::{Str, TextGlyph, TextMetrics, TextSpan, TextSpans, TextStyle};

library!(lib {
    fn pxui_shaper_system_fonts(shaper:*mut PxuiShaper)->i32;
    fn pxui_text_system_fonts(ui:*mut PxuiUi)->i32;
    fn pxui_shaper_width_spans(shaper:*mut PxuiShaper,key:u64,value:Str,style:*const TextStyle,max_width:f32,spans:*const TextSpans,out:*mut TextMetrics)->i32;
    fn pxui_text_width_spans(ui:*mut PxuiUi,key:u64,value:Str,style:*const TextStyle,max_width:f32,spans:*const TextSpans,out:*mut TextMetrics)->i32;
    fn pxui_ui_text_spans(ui:*mut PxuiUi,box_:*const crate::PxuiBox,value:Str,style:*const TextStyle,spans:*const TextSpans)->i32;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextError { InvalidInput, Allocation, Engine }

/// The default font (Pragmasevka) the library embeds.
pub fn pragmasevka() -> &'static [u8] {
    let font = unsafe { lib::pxui_default_font() };
    unsafe { core::slice::from_raw_parts(font.ptr, font.len) }
}

/// What a shaping call talks to: a shaper of its own or a `Ui`'s text service.
#[derive(Clone, Copy)]
enum Backing { Shaper(*mut PxuiShaper), Ui(*mut PxuiUi) }

impl Backing {
    fn add_font(self, bytes: &[u8]) -> Option<u32> {
        let font = Str { ptr: bytes.as_ptr(), len: bytes.len() };
        let index = unsafe { match self { Backing::Shaper(s) => lib::pxui_shaper_add_font(s, font), Backing::Ui(u) => lib::pxui_text_add_font(u, font) } };
        (index != u32::MAX).then_some(index)
    }

    fn measure(self, key: u64, value: &str, style: &TextStyle, max_width: f32) -> Result<TextMetrics, i32> {
        let mut out = TextMetrics::default();
        let rc = unsafe { match self {
            Backing::Shaper(s) => lib::pxui_shaper_width(s, key, as_str(value), style, max_width, &mut out),
            Backing::Ui(u) => lib::pxui_text_width(u, key, as_str(value), style, max_width, &mut out),
        } };
        if rc == 0 { Ok(out) } else { Err(rc) }
    }

    fn measure_spans(self,key:u64,value:&str,style:&TextStyle,max_width:f32,spans:&[TextSpan])->Result<TextMetrics,i32> {
        let spans=TextSpans::new(spans).ok_or(pxui_types::PXUI_ERR_ARGUMENT)?;
        let mut out=TextMetrics::default();
        let rc=unsafe { match self {
            Backing::Shaper(s)=>lib::pxui_shaper_width_spans(s,key,as_str(value),style,max_width,&spans,&mut out),
            Backing::Ui(u)=>lib::pxui_text_width_spans(u,key,as_str(value),style,max_width,&spans,&mut out),
        } };
        if rc==0 { Ok(out) } else { Err(rc) }
    }

    fn rasterize_rgba(self, key: u64, value: &str, style: &TextStyle, max_width: f32, scale: f32, color: [u8; 4]) -> Result<(u32, u32, Vec<u8>), TextError> {
        let (mut width, mut height) = (0, 0);
        let mut call = |out, capacity| unsafe { match self {
            Backing::Shaper(s) => lib::pxui_shaper_rasterize_rgba(s, key, as_str(value), style, max_width, scale, color[0], color[1], color[2], color[3], &mut width, &mut height, out, capacity),
            Backing::Ui(u) => lib::pxui_text_rasterize_rgba(u, key, as_str(value), style, max_width, scale, color[0], color[1], color[2], color[3], &mut width, &mut height, out, capacity),
        } };
        let required = call(core::ptr::null_mut(), 0);
        if required == usize::MAX { return Err(TextError::InvalidInput); }
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(required).map_err(|_| TextError::Allocation)?;
        pixels.resize(required, 0);
        if call(pixels.as_mut_ptr(), pixels.len()) != required { return Err(TextError::Engine); }
        Ok((width, height, pixels))
    }
}

/// A text shaper of its own, owned by the library; `key` names a layout in its cache.
///
/// A shaper may move to another thread (a worker owns its own), but it is not `Sync`: one thread at a time uses one shaper, and its
/// calls never overlap (the library keeps no thread-local state for it). It shares nothing with the `Ui`'s text service.
pub struct Text(NonNull<PxuiShaper>);

// SAFETY: a shaper is a plain object of the library (its own font system and caches, no thread-local state, no pointer into the
// caller), so moving it between threads is fine as long as one thread uses it at a time, which `&mut self` and no `Sync` guarantee.
unsafe impl Send for Text {}

impl Text {
    /// Enable native font fallback (Apple). A first missing cluster can discover/map a font file.
    pub fn enable_system_fonts(&mut self)->Result<(),i32> {
        let rc=unsafe { lib::pxui_shaper_system_fonts(self.0.as_ptr()) };
        if rc==0 { Ok(()) } else { Err(rc) }
    }
    pub fn new(fonts: &[&[u8]]) -> Option<Self> {
        let fonts: Vec<Str> = fonts.iter().map(|font| Str { ptr: font.as_ptr(), len: font.len() }).collect();
        NonNull::new(unsafe { lib::pxui_shaper_create(fonts.as_ptr(), fonts.len()) }).map(Self)
    }
    /// Adds a font (the bytes are copied); its index.
    pub fn add_font(&mut self, bytes: &[u8]) -> Option<u32> { Backing::Shaper(self.0.as_ptr()).add_font(bytes) }
    pub fn measure(&mut self, key: u64, value: &str, style: &TextStyle, max_width: f32) -> Result<TextMetrics, i32> { Backing::Shaper(self.0.as_ptr()).measure(key, value, style, max_width) }
    /// Measures one layout with up to eight ordered, disjoint UTF-8 byte ranges.
    pub fn measure_spans(&mut self,key:u64,value:&str,style:&TextStyle,max_width:f32,spans:&[TextSpan])->Result<TextMetrics,i32> { Backing::Shaper(self.0.as_ptr()).measure_spans(key,value,style,max_width,spans) }
    pub fn width(&mut self, key: u64, value: &str, style: &TextStyle, max_width: f32) -> Result<f32, i32> { self.measure(key, value, style, max_width).map(|m| m.width) }
    /// The text as straight-alpha RGBA8: width, height and pixels.
    pub fn rasterize_rgba(&mut self, key: u64, value: &str, style: &TextStyle, max_width: f32, scale: f32, color: [u8; 4]) -> Result<(u32, u32, Vec<u8>), TextError> {
        Backing::Shaper(self.0.as_ptr()).rasterize_rgba(key, value, style, max_width, scale, color)
    }
}

impl Drop for Text { fn drop(&mut self) { unsafe { lib::pxui_shaper_destroy(self.0.as_ptr()) } } }

/// The text service of a `Ui`: its one font system and glyph atlas.
pub struct UiText<'ui>(&'ui mut Ui);

impl Ui {
    /// Enable native font fallback before the first frame. Explicit-font UIs stay deterministic.
    pub fn enable_system_fonts(&mut self)->Result<(),i32> {
        let rc=unsafe { lib::pxui_text_system_fonts(self.0.as_ptr()) };
        if rc==0 { Ok(()) } else { Err(rc) }
    }
    /// Sets a frame's node text and explicit shaping style, preserving the complete accessibility label.
    /// Spans are at most eight ordered, disjoint UTF-8 byte ranges; plain `text`/`patch_text` clears them.
    pub fn text_spans(&mut self,box_:&crate::PxuiBox,text:&str,style:&TextStyle,spans:&[TextSpan])->Result<(),i32> {
        let spans=TextSpans::new(spans).ok_or(pxui_types::PXUI_ERR_ARGUMENT)?;
        let rc=unsafe { lib::pxui_ui_text_spans(self.0.as_ptr(),box_,as_str(text),style,&spans) };
        if rc==0 { Ok(()) } else { Err(rc) }
    }
    pub fn text_service(&mut self) -> UiText<'_> { UiText(self) }
}

impl UiText<'_> {
    fn backing(&self) -> Backing { Backing::Ui(self.0.0.as_ptr()) }
    pub fn add_font(&mut self, bytes: &[u8]) -> Option<u32> { self.backing().add_font(bytes) }
    pub fn measure(&mut self, key: u64, value: &str, style: &TextStyle, max_width: f32) -> Result<TextMetrics, i32> { self.backing().measure(key, value, style, max_width) }
    /// Measures one layout with up to eight ordered, disjoint UTF-8 byte ranges.
    pub fn measure_spans(&mut self,key:u64,value:&str,style:&TextStyle,max_width:f32,spans:&[TextSpan])->Result<TextMetrics,i32> { self.backing().measure_spans(key,value,style,max_width,spans) }
    pub fn width(&mut self, key: u64, value: &str, style: &TextStyle, max_width: f32) -> Result<f32, i32> { self.measure(key, value, style, max_width).map(|m| m.width) }
    pub fn rasterize_rgba(&mut self, key: u64, value: &str, style: &TextStyle, max_width: f32, scale: f32, color: [u8; 4]) -> Result<(u32, u32, Vec<u8>), TextError> {
        self.backing().rasterize_rgba(key, value, style, max_width, scale, color)
    }

    /// The glyph quads of `value`, positioned at `origin` and scaled by `scale`.
    pub fn quads(&mut self, key: u64, value: &str, style: &TextStyle, max_width: f32, origin: (f32, f32), scale: f32) -> Result<Vec<TextGlyph>, i32> {
        let ui = self.0.0.as_ptr();
        let call = |out, capacity| unsafe { lib::pxui_text_quads(ui, key, as_str(value), style, max_width, origin.0, origin.1, scale, out, capacity) };
        let count = call(core::ptr::null_mut(), 0);
        if count == usize::MAX { return Err(-1); }
        let mut glyphs = Vec::new();
        glyphs.try_reserve_exact(count).map_err(|_| -1)?;
        glyphs.resize(count, TextGlyph::default());
        let written = call(glyphs.as_mut_ptr(), glyphs.len());
        if written == usize::MAX { return Err(-1); }
        glyphs.truncate(written.min(glyphs.len()));
        Ok(glyphs)
    }
}
