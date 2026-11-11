use pxui_text::{Align, AtlasKind, Style, Text, Wrap};
use crate::PXUI_ERR_ARGUMENT;
use pxui_types::{Str, TextGlyph, TextMetrics, TextStyle};

pub struct PxuiText { text: Text }

const MAX_FONTS: usize = 128;

unsafe fn bytes<'a>(value: Str) -> Option<&'a [u8]> {
    if value.len == 0 { return Some(&[]); }
    if value.ptr.is_null() { return None; }
    Some(unsafe { std::slice::from_raw_parts(value.ptr, value.len) })
}

unsafe fn utf8<'a>(value: Str) -> Option<&'a str> {
    std::str::from_utf8(unsafe { bytes(value) }?).ok()
}

pub(crate) fn style(value: TextStyle) -> Option<Style> {
    if value.size < core::mem::size_of::<TextStyle>() as u32 || value.reserved != 0
        || value.italic > 1 || value.ellipsis > 1
        || !value.size_points.is_finite() || value.size_points <= 0.0
        || !value.line_height.is_finite() || !value.tracking.is_finite()
    { return None; }
    Some(Style {
        font: value.font,
        size: value.size_points,
        weight: value.weight,
        italic: value.italic != 0,
        line_height: value.line_height,
        wrap: match value.wrap { TextStyle::WRAP_NONE => Wrap::None, TextStyle::WRAP_WORD => Wrap::Word, TextStyle::WRAP_GLYPH => Wrap::Glyph, _ => return None },
        align: match value.align { TextStyle::ALIGN_LEFT => Align::Left, TextStyle::ALIGN_CENTER => Align::Center, TextStyle::ALIGN_RIGHT => Align::Right, _ => return None },
        max_lines: value.max_lines,
        ellipsis: value.ellipsis != 0,
        tracking: value.tracking,
    })
}

pub(crate) fn metrics(value: pxui_text::Metrics) -> TextMetrics {
    TextMetrics { width: value.w, height: value.h, baseline: value.baseline, lines: value.lines }
}

export!(bare pub unsafe fn pxui_text_create(fonts: *const Str, count: usize) -> *mut PxuiText {
    if count > MAX_FONTS || (count != 0 && fonts.is_null()) { return core::ptr::null_mut(); }
    let mut font_bytes = Vec::with_capacity(count);
    if count != 0 {
        for font in unsafe { std::slice::from_raw_parts(fonts, count) } {
            let Some(font) = (unsafe { bytes(*font) }) else { return core::ptr::null_mut(); };
            font_bytes.push(font);
        }
    }
    Box::into_raw(Box::new(PxuiText { text: Text::new(&font_bytes) }))
});

export!(bare pub unsafe fn pxui_text_destroy(text: *mut PxuiText) {
    if !text.is_null() { drop(unsafe { Box::from_raw(text) }); }
});

export!(bare pub unsafe fn pxui_text_add_font(text: *mut PxuiText, font: Str) -> u32 {
    if text.is_null() { return u32::MAX; }
    let Some(font) = (unsafe { bytes(font) }) else { return u32::MAX; };
    u32::try_from(unsafe { &mut *text }.text.add_font(font)).unwrap_or(u32::MAX)
});

export!(bare pub unsafe fn pxui_text_width(text: *mut PxuiText, key: u64, value: Str, style_: *const TextStyle, max_width: f32, out: *mut TextMetrics) -> i32 {
    if text.is_null() || style_.is_null() || out.is_null() || !max_width.is_finite() { return PXUI_ERR_ARGUMENT; }
    let Some(value) = (unsafe { utf8(value) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(style) = style(unsafe { *style_ }) else { return PXUI_ERR_ARGUMENT; };
    let measured = unsafe { &mut *text }.text.layout(key, value, &style, max_width);
    unsafe { out.write(metrics(measured)); }
    0
});

// Returns the total glyph count, or `usize::MAX` for invalid arguments or a panic. If `capacity`
// is smaller than the result, only the first `capacity` glyphs are written.
export!(bare pub unsafe fn pxui_text_quads(
    text: *mut PxuiText,
    key: u64,
    value: Str,
    style_: *const TextStyle,
    max_width: f32,
    origin_x: f32,
    origin_y: f32,
    scale: f32,
    out: *mut TextGlyph,
    capacity: usize,
) -> usize {
    if text.is_null() || style_.is_null() || (capacity != 0 && out.is_null())
        || !origin_x.is_finite() || !origin_y.is_finite() || !scale.is_finite() || scale <= 0.0
    { return usize::MAX; }
    let Some(value) = (unsafe { utf8(value) }) else { return usize::MAX; };
    let Some(style) = style(unsafe { *style_ }) else { return usize::MAX; };
    {
        let state = unsafe { &mut *text };
        state.text.layout(key, value, &style, max_width);
        let mut count = 0;
        state.text.glyphs(key, (origin_x, origin_y), scale, &mut |glyph| {
            if count < capacity {
                let atlas = match glyph.atlas { AtlasKind::Mask => 0, AtlasKind::Color => 1 };
                let value = TextGlyph {
                    rect: [glyph.rect.x, glyph.rect.y, glyph.rect.w, glyph.rect.h],
                    uv: [glyph.uv.x, glyph.uv.y, glyph.uv.w, glyph.uv.h],
                    atlas,
                    reserved: 0,
                };
                unsafe { out.add(count).write(value); }
            }
            count += 1;
        });
        count
    }
});

export!(bare pub unsafe fn pxui_text_end_frame(text: *mut PxuiText) -> i32 {
    if text.is_null() { return -1; }
    unsafe { &mut *text }.text.end_frame();
    0
});

// Renders shaped text to straight-alpha RGBA8. A null/short output buffer is a size query; the
// dimensions and required byte count are still returned. The caller retries with that capacity.
export!(bare pub unsafe fn pxui_text_rasterize_rgba(
    text: *mut PxuiText,
    key: u64,
    value: Str,
    style_: *const TextStyle,
    max_width: f32,
    scale: f32,
    color_r: u8,
    color_g: u8,
    color_b: u8,
    color_a: u8,
    width: *mut u32,
    height: *mut u32,
    out: *mut u8,
    capacity: usize,
) -> usize {
    if text.is_null() || style_.is_null() || width.is_null() || height.is_null()
        || !max_width.is_finite() || !scale.is_finite() || scale <= 0.0
        || (capacity != 0 && out.is_null())
    { return usize::MAX; }
    let Some(value) = (unsafe { utf8(value) }) else { return usize::MAX; };
    let Some(style) = style(unsafe { *style_ }) else { return usize::MAX; };
    {
        let state = unsafe { &mut *text };
        let metrics = state.text.layout(key, value, &style, max_width);
        let (w, h) = if value.is_empty() {
            (1u32, 1u32)
        } else {
            let w = if max_width > 0.0 { (max_width * scale).ceil() } else { (metrics.w * scale).ceil() };
            let h = (metrics.h * scale).ceil();
            if !w.is_finite() || !h.is_finite() || w < 1.0 || h < 1.0 || w > u32::MAX as f32 || h > u32::MAX as f32 {
                return usize::MAX;
            }
            (w as u32, h as u32)
        };
        let Some(required) = (w as usize).checked_mul(h as usize).and_then(|n| n.checked_mul(4)) else { return usize::MAX; };
        unsafe { width.write(w); height.write(h); }
        if capacity < required { return required; }
        unsafe { core::ptr::write_bytes(out, 0, required); }
        if value.is_empty() { return required; }

        let (ink, alpha) = ([color_r, color_g, color_b], color_a as u32);
        let mut glyphs = Vec::new();
        state.text.glyphs(key, (0.0, 0.0), scale, &mut |glyph| glyphs.push(glyph));
        for glyph in glyphs {
            let (atlas_w, atlas_h) = state.text.atlas_size(glyph.atlas);
            let atlas = state.text.atlas_pixels(glyph.atlas);
            let (u, v) = ((glyph.uv.x * atlas_w as f32).round() as i64, (glyph.uv.y * atlas_h as f32).round() as i64);
            let (gw, gh, gx, gy) = (glyph.rect.w as i64, glyph.rect.h as i64, glyph.rect.x as i64, glyph.rect.y as i64);
            for row in 0..gh {
                let y = gy + row;
                if y < 0 || y >= h as i64 { continue; }
                for col in 0..gw {
                    let x = gx + col;
                    if x < 0 || x >= w as i64 { continue; }
                    let at = (v + row) * atlas_w as i64 + u + col;
                    let src = match glyph.atlas {
                        AtlasKind::Mask => [ink[0], ink[1], ink[2], ((atlas[at as usize] as u32 * alpha + 127) / 255) as u8],
                        AtlasKind::Color => {
                            let i = at as usize * 4;
                            [atlas[i], atlas[i + 1], atlas[i + 2], ((atlas[i + 3] as u32 * alpha + 127) / 255) as u8]
                        }
                    };
                    let offset = ((y as usize * w as usize + x as usize) * 4) as usize;
                    let dst = unsafe { core::slice::from_raw_parts_mut(out.add(offset), 4) };
                    over(dst, src);
                }
            }
        }
        state.text.drain_uploads(&mut |_, _, _, _, _, _, _| {});
        required
    }
});

fn over(dst: &mut [u8], src: [u8; 4]) {
    let (sa, da) = (src[3] as u32, dst[3] as u32);
    let oa = sa * 255 + da * (255 - sa);
    if oa == 0 { return; }
    for i in 0..3 {
        dst[i] = ((src[i] as u32 * sa * 255 + dst[i] as u32 * da * (255 - sa)) / oa) as u8;
    }
    dst[3] = ((oa + 127) / 255) as u8;
}

#[cfg(test)]
mod tests {
    use super::{over, style};
    use pxui_types::TextStyle;

    #[test]
    fn text_style_validates_codes_and_numbers() {
        assert!(style(TextStyle::default()).is_some());
        let mut invalid = TextStyle::default();
        invalid.wrap = u32::MAX;
        assert!(style(invalid).is_none());
        invalid = TextStyle::default();
        invalid.size_points = f32::NAN;
        assert!(style(invalid).is_none());
    }

    #[test]
    fn straight_alpha_compositing_keeps_source_color() {
        let mut pixel = [0, 0, 0, 0];
        over(&mut pixel, [220, 40, 10, 128]);
        assert_eq!(pixel, [220, 40, 10, 128]);
    }
}
