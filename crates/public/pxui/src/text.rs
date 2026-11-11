use core::ptr::NonNull;
use pxui_types::{Str, TextGlyph, TextMetrics, TextStyle};

pub const PRAGMASEVKA: &[u8] = include_bytes!("../../../../assets/fonts/Pragmasevka-Regular.ttf");

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{pxui_text_add_font, pxui_text_create, pxui_text_destroy, pxui_text_end_frame, pxui_text_quads, pxui_text_rasterize_rgba, pxui_text_width, PxuiText};
}

#[cfg(not(feature = "static"))]
mod sys {
    use pxui_types::{Str, TextGlyph, TextMetrics, TextStyle};
    #[repr(C)] pub struct PxuiText { _private: [u8; 0] }
    #[link(name = "pxui")]
    unsafe extern "C" {
        pub fn pxui_text_create(fonts: *const Str, count: usize) -> *mut PxuiText;
        pub fn pxui_text_destroy(text: *mut PxuiText);
        pub fn pxui_text_add_font(text: *mut PxuiText, font: Str) -> u32;
        pub fn pxui_text_width(text: *mut PxuiText, key: u64, value: Str, style: *const TextStyle, max_width: f32, out: *mut TextMetrics) -> i32;
        pub fn pxui_text_quads(text: *mut PxuiText, key: u64, value: Str, style: *const TextStyle, max_width: f32, x: f32, y: f32, scale: f32, out: *mut TextGlyph, capacity: usize) -> usize;
        pub fn pxui_text_end_frame(text: *mut PxuiText) -> i32;
        pub fn pxui_text_rasterize_rgba(text: *mut PxuiText, key: u64, value: Str, style: *const TextStyle, max_width: f32, scale: f32, color_r: u8, color_g: u8, color_b: u8, color_a: u8, width: *mut u32, height: *mut u32, out: *mut u8, capacity: usize) -> usize;
    }
}

pub use pxui_types::{TextGlyph as Glyph, TextMetrics as Metrics, TextStyle as Style};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextError { InvalidInput, Allocation, Engine }

/// A text shaper with the font bytes and glyph atlas owned inside libpxui.
pub struct Text(NonNull<sys::PxuiText>);

impl Text {
    pub fn new(fonts: &[&[u8]]) -> Option<Self> {
        let fonts: Vec<Str> = fonts.iter().map(|font| Str { ptr: font.as_ptr(), len: font.len() }).collect();
        NonNull::new(unsafe { sys::pxui_text_create(fonts.as_ptr(), fonts.len()) }).map(Self)
    }

    pub fn add_font(&mut self, bytes: &[u8]) -> Option<u32> {
        let font = Str { ptr: bytes.as_ptr(), len: bytes.len() };
        let index = unsafe { sys::pxui_text_add_font(self.0.as_ptr(), font) };
        (index != u32::MAX).then_some(index)
    }

    pub fn measure(&mut self, key: u64, value: &str, style: &TextStyle, max_width: f32) -> Result<TextMetrics, i32> {
        let mut out = TextMetrics::default();
        let value = as_str(value);
        let rc = unsafe { sys::pxui_text_width(self.0.as_ptr(), key, value, style, max_width, &mut out) };
        if rc == 0 { Ok(out) } else { Err(rc) }
    }

    pub fn width(&mut self, key: u64, value: &str, style: &TextStyle, max_width: f32) -> Result<f32, i32> {
        self.measure(key, value, style, max_width).map(|metrics| metrics.width)
    }

    pub fn quads(&mut self, key: u64, value: &str, style: &TextStyle, max_width: f32, origin: (f32, f32), scale: f32) -> Result<Vec<TextGlyph>, i32> {
        let value = as_str(value);
        let call = |out, capacity| unsafe {
            sys::pxui_text_quads(self.0.as_ptr(), key, value, style, max_width, origin.0, origin.1, scale, out, capacity)
        };
        let count = call(core::ptr::null_mut(), 0);
        if count == usize::MAX { return Err(-1); }
        let mut glyphs = Vec::new();
        glyphs.try_reserve_exact(count).map_err(|_| -1)?;
        glyphs.resize(count, TextGlyph::default());
        let written = call(glyphs.as_mut_ptr(), glyphs.len());
        if written == usize::MAX { return Err(-1); }
        let written = written.min(glyphs.len());
        glyphs.truncate(written);
        Ok(glyphs)
    }

    pub fn end_frame(&mut self) -> Result<(), i32> {
        let rc = unsafe { sys::pxui_text_end_frame(self.0.as_ptr()) };
        if rc == 0 { Ok(()) } else { Err(rc) }
    }

    pub fn rasterize_rgba(&mut self, key: u64, value: &str, style: &TextStyle, max_width: f32, scale: f32, color: [u8; 4]) -> Result<(u32, u32, Vec<u8>), TextError> {
        let mut width = 0;
        let mut height = 0;
        let value = as_str(value);
        let mut call = |out, capacity| unsafe {
            sys::pxui_text_rasterize_rgba(self.0.as_ptr(), key, value, style, max_width, scale,
                color[0], color[1], color[2], color[3], &mut width, &mut height, out, capacity)
        };
        let required = call(core::ptr::null_mut(), 0);
        if required == usize::MAX { return Err(TextError::InvalidInput); }
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(required).map_err(|_| TextError::Allocation)?;
        pixels.resize(required, 0);
        if call(pixels.as_mut_ptr(), pixels.len()) != required { return Err(TextError::Engine); }
        Ok((width, height, pixels))
    }
}

impl Drop for Text {
    fn drop(&mut self) { unsafe { sys::pxui_text_destroy(self.0.as_ptr()) } }
}

fn as_str(value: &str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }
