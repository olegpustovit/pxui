//! The `Ui`'s text service: the one font system and glyph atlas the `Ui` draws with, for canvas text an app lays out itself.
//! Glyph quads reference the atlases `Renderer::atlas` hands out.
use crate::{as_str, Ui};
use pxui_types::{Str, TextGlyph, TextMetrics, TextStyle};

library!(lib {
    fn pxui_text_add_font(ui: *mut PxuiUi, font: Str) -> u32;
    fn pxui_text_width(ui: *mut PxuiUi, key: u64, value: Str, style: *const TextStyle, max_width: f32, out: *mut TextMetrics) -> i32;
    fn pxui_text_quads(ui: *mut PxuiUi, key: u64, value: Str, style: *const TextStyle, max_width: f32, x: f32, y: f32, scale: f32, out: *mut TextGlyph, capacity: usize) -> usize;
    fn pxui_text_rasterize_rgba(ui: *mut PxuiUi, key: u64, value: Str, style: *const TextStyle, max_width: f32, scale: f32, color_r: u8, color_g: u8, color_b: u8, color_a: u8, width: *mut u32, height: *mut u32, out: *mut u8, capacity: usize) -> usize;
});

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextError { InvalidInput, Allocation, Engine }

/// The text service of a `Ui`. `key` names a layout in the service's cache: the same key with the same text and style is
/// shaped once.
pub struct Text<'ui>(&'ui mut Ui);

impl Ui {
    pub fn text_service(&mut self) -> Text<'_> { Text(self) }
}

impl Text<'_> {
    /// Adds a font (the bytes are copied); its index.
    #[inline]
    pub fn add_font(&mut self, bytes: &[u8]) -> Option<u32> {
        let index = unsafe { lib::pxui_text_add_font(self.0.0.as_ptr(), Str { ptr: bytes.as_ptr(), len: bytes.len() }) };
        (index != u32::MAX).then_some(index)
    }

    #[inline]
    pub fn measure(&mut self, key: u64, value: &str, style: &TextStyle, max_width: f32) -> Result<TextMetrics, i32> {
        let mut out = TextMetrics::default();
        let rc = unsafe { lib::pxui_text_width(self.0.0.as_ptr(), key, as_str(value), style, max_width, &mut out) };
        if rc == 0 { Ok(out) } else { Err(rc) }
    }

    #[inline]
    pub fn width(&mut self, key: u64, value: &str, style: &TextStyle, max_width: f32) -> Result<f32, i32> {
        self.measure(key, value, style, max_width).map(|metrics| metrics.width)
    }

    /// The glyph quads of `value`, positioned at `origin` and scaled by `scale`.
    #[inline]
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

    /// The text as straight-alpha RGBA8: width, height and pixels.
    #[inline]
    pub fn rasterize_rgba(&mut self, key: u64, value: &str, style: &TextStyle, max_width: f32, scale: f32, color: [u8; 4]) -> Result<(u32, u32, Vec<u8>), TextError> {
        let ui = self.0.0.as_ptr();
        let (mut width, mut height) = (0, 0);
        let mut call = |out, capacity| unsafe {
            lib::pxui_text_rasterize_rgba(ui, key, as_str(value), style, max_width, scale, color[0], color[1], color[2], color[3], &mut width, &mut height, out, capacity)
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
