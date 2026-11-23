//! An Apple grayscale glyph rasterizer. CoreText owns the font and the mutable bitmap is exclusive.
use std::ffi::c_void;
use std::ptr::{self, NonNull};
use std::sync::Arc;

type Source = Arc<dyn AsRef<[u8]> + Send + Sync>;

#[repr(C)]
#[derive(Default)]
struct NativeMask {
    pixels: *const u8,
    stride: usize,
    width: u32,
    height: u32,
    left: i32,
    top: i32,
}

#[link(name = "CoreGraphics", kind = "framework")]
#[link(name = "CoreText", kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: *const c_void);
    fn pxui_font_mask_face(bytes: *const u8, len: isize, index: u32, name: *const u8, name_len: isize, owner: *mut c_void,
                           release: unsafe extern "C" fn(*const c_void)) -> *mut c_void;
    fn pxui_font_mask_font(face: *const c_void, ppem: f64, weight: f64) -> *mut c_void;
    fn pxui_font_mask_draw(font: *const c_void, ctx: *mut *mut c_void, glyph: u16,
                           phase: f64, out: *mut NativeMask) -> i32;
}

struct Owned(NonNull<c_void>);

// Fonts and descriptors are immutable. A Text worker may move its native state to another thread;
// each bitmap is exclusively accessed through a mutable borrow.
unsafe impl Send for Owned {}
impl Drop for Owned {
    fn drop(&mut self) { unsafe { CFRelease(self.0.as_ptr()) }; }
}

/// A face indexed from the same bytes used for shaping, without copying or registering the font.
pub struct MaskFace(Owned);
impl MaskFace {
    /// Read the indexed font face. CoreText retains the source while any native font uses it.
    pub fn new(source: Source, index: u32, postscript_name: &str) -> Option<Self> {
        unsafe extern "C" fn release(owner: *const c_void) {
            // The native allocator owns exactly one boxed source.
            drop(unsafe { Box::from_raw(owner.cast_mut().cast::<Source>()) });
        }
        let bytes = source.as_ref().as_ref();
        let (ptr, len) = (bytes.as_ptr(), isize::try_from(bytes.len()).ok()?);
        let name_len = isize::try_from(postscript_name.len()).ok()?;
        if len == 0 { return None; }
        let owner = Box::into_raw(Box::new(source));
        NonNull::new(unsafe { pxui_font_mask_face(ptr, len, index, postscript_name.as_ptr(), name_len, owner.cast(), release) })
            .map(|value| Self(Owned(value)))
    }

    /// Create an immutable font at a physical ppem and an optional OpenType weight-axis value.
    pub fn at_size(&self, ppem: f32, weight: Option<f32>) -> Option<MaskFont> {
        if !ppem.is_finite() || ppem <= 0.0 { return None; }
        NonNull::new(unsafe { pxui_font_mask_font(self.0.0.as_ptr(), ppem.into(), weight.unwrap_or(0.0).into()) })
            .map(|value| MaskFont(Owned(value)))
    }
}

/// An immutable CoreText font at an exact physical size and weight.
pub struct MaskFont(Owned);

/// Single-channel mask layout. Coordinates are relative to the integral baseline origin.
pub struct MaskLayout {
    pub stride: usize,
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
}

/// A reusable alpha-only context, bounded to 2048 × 2048 pixels and allocated on demand.
pub struct MaskRaster(*mut c_void);

// The bitmap is exclusively accessed through a mutable borrow.
unsafe impl Send for MaskRaster {}
impl Default for MaskRaster {
    fn default() -> Self { Self(ptr::null_mut()) }
}
impl Drop for MaskRaster {
    fn drop(&mut self) {
        if !self.0.is_null() { unsafe { CFRelease(self.0) }; }
    }
}

impl MaskRaster {
    /// Draw the shaped glyph id at a calibrated x phase. The data is valid until the next draw.
    pub fn draw(&mut self, font: &MaskFont, glyph: u16, phase: f32) -> Option<(MaskLayout, &[u8])> {
        if !(0.0..1.0).contains(&phase) { return None; }
        let mut output = NativeMask::default();
        let ok = unsafe { pxui_font_mask_draw(font.0.0.as_ptr(), &mut self.0, glyph, phase.into(), &mut output) };
        if ok == 0 { return None; }
        let NativeMask { pixels, stride, width, height, left, top } = output;
        let coverage = if height == 0 { &[] } else {
            // The native window is within the context's allocation, retained by self.
            unsafe { std::slice::from_raw_parts(pixels, (height as usize - 1) * stride + width as usize) }
        };
        Some((MaskLayout { stride, width, height, left, top }, coverage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Ref = *const c_void;

    #[repr(C)]
    struct Point { x: f64, y: f64 }
    #[repr(C)]
    struct Rect { origin: Point, w: f64, h: f64 }

    unsafe extern "C" {
        static kCGColorSpaceSRGB: Ref;
        fn CGColorSpaceCreateWithName(name: Ref) -> *mut c_void;
        fn CGBitmapContextCreate(data: *mut u8, width: usize, height: usize, bits: usize,
                                 stride: usize, color_space: Ref, alpha: u32) -> *mut c_void;
        fn CGContextSetGrayFillColor(ctx: Ref, gray: f64, alpha: f64);
        fn CGContextFillRect(ctx: Ref, rect: Rect);
        fn CGContextSetAllowsFontSmoothing(ctx: Ref, on: bool);
        fn CGContextSetShouldSmoothFonts(ctx: Ref, on: bool);
        fn CGContextSetAllowsFontSubpixelPositioning(ctx: Ref, on: bool);
        fn CGContextSetShouldSubpixelPositionFonts(ctx: Ref, on: bool);
        fn CGContextSetAllowsFontSubpixelQuantization(ctx: Ref, on: bool);
        fn CGContextSetShouldSubpixelQuantizeFonts(ctx: Ref, on: bool);
        fn CTFontGetGlyphsForCharacters(font: Ref, chars: *const u16, glyphs: *mut u16, count: isize) -> bool;
        fn CTFontDrawGlyphs(font: Ref, glyphs: *const u16, positions: *const Point, count: usize, ctx: Ref);
        fn CGContextFlush(ctx: Ref);
    }

    /// Compare the reusable crop to a full opaque CoreText canvas at the same absolute baseline, font-smoothed on macOS.
    #[test]
    fn grayscale_crop_matches_native_pixels_and_bearings_after_reuse() {
        const SIZE: usize = 128;
        const BASE: i32 = 64;
        let source = include_bytes!("../../../../assets/fonts/Pragmasevka-Regular.ttf").to_vec();
        let face = MaskFace::new(Arc::new(source), 0, "Pragmasevka-Regular").unwrap();
        let small = face.at_size(13.0, None).unwrap();
        let large = face.at_size(95.0, None).unwrap();
        drop(face);
        let mut raster = MaskRaster::default();

        for (font, ch, phase) in [(&small, 'j', 0.375), (&small, 'j', 0.75), (&large, 'Q', 0.0), (&small, ' ', 0.0), (&small, 'H', 0.375)] {
            let ct = font.0.0.as_ptr();
            let mut glyph = 0;
            assert!(unsafe { CTFontGetGlyphsForCharacters(ct, &(ch as u16), &mut glyph, 1) });
            let mut reference = vec![0; SIZE * SIZE * 4];
            {
                let space = Owned(NonNull::new(unsafe { CGColorSpaceCreateWithName(kCGColorSpaceSRGB) }).unwrap());
                let ctx = Owned(NonNull::new(unsafe { CGBitmapContextCreate(reference.as_mut_ptr(), SIZE, SIZE, 8, SIZE * 4, space.0.as_ptr(), 1) }).unwrap());
                let ctx = ctx.0.as_ptr();
                // 95 ppem can extend above a 64-point baseline. Give its reference more room.
                let y = if ch == 'Q' { 96 } else { BASE };
                unsafe {
                    CGContextSetGrayFillColor(ctx, 1.0, 1.0);
                    CGContextFillRect(ctx, Rect { origin: Point { x: 0.0, y: 0.0 }, w: SIZE as f64, h: SIZE as f64 });
                    CGContextSetGrayFillColor(ctx, 0.0, 1.0);
                    // macOS text is font-smoothed (the native mask draws it so); iOS has no smoothing
                    CGContextSetAllowsFontSmoothing(ctx, cfg!(target_os = "macos"));
                    CGContextSetShouldSmoothFonts(ctx, cfg!(target_os = "macos"));
                    CGContextSetAllowsFontSubpixelPositioning(ctx, true);
                    CGContextSetShouldSubpixelPositionFonts(ctx, true);
                    CGContextSetAllowsFontSubpixelQuantization(ctx, true);
                    CGContextSetShouldSubpixelQuantizeFonts(ctx, true);
                    CTFontDrawGlyphs(ct, &glyph, &Point { x: 24.0 + f64::from(phase), y: SIZE as f64 - f64::from(y) }, 1, ctx);
                    CGContextFlush(ctx);
                }
            }
            let (mask, pixels) = raster.draw(font, glyph, phase).unwrap();
            assert_eq!(pixels.is_empty(), ch == ' ');
            let baseline = if ch == 'Q' { 96 } else { BASE };
            let (left, top) = (24 + mask.left, baseline - mask.top);
            for y in 0..SIZE {
                for x in 0..SIZE {
                    let (mx, my) = (x as i32 - left, y as i32 - top);
                    let coverage = if (0..mask.width as i32).contains(&mx) && (0..mask.height as i32).contains(&my) {
                        pixels[my as usize * mask.stride + mx as usize]
                    } else { 0 };
                    let at = (y * SIZE + x) * 4;
                    assert_eq!(&reference[at..at + 4], &[255 - coverage, 255 - coverage, 255 - coverage, 255], "{ch} phase {phase}: {x},{y}");
                }
            }
        }
        assert!(raster.draw(&small, u16::MAX, 0.0).is_none());
    }
}
