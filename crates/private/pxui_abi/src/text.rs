use pxui_text::{Align, AtlasKind, Ellipsis, Style, Text, Wrap};
use crate::state::{PxuiUi, engine};
use crate::PXUI_ERR_ARGUMENT;
use pxui_types::{Str, TextGlyph, TextMetrics, TextSpans, TextStyle};

unsafe fn bytes<'a>(value: Str) -> Option<&'a [u8]> {
    if value.len == 0 { return Some(&[]); }
    if value.ptr.is_null() { return None; }
    Some(unsafe { std::slice::from_raw_parts(value.ptr, value.len) })
}

unsafe fn utf8<'a>(value: Str) -> Option<&'a str> {
    std::str::from_utf8(unsafe { bytes(value) }?).ok()
}

pub(crate) fn style(value: TextStyle) -> Option<Style> {
    if value.size < core::mem::size_of::<TextStyle>() as u32 || value.features & !Style::FEATURE_TABULAR_NUMBERS != 0
        || value.italic > 1
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
        ellipsis: match value.ellipsis { TextStyle::ELLIPSIS_NONE=>Ellipsis::None, TextStyle::ELLIPSIS_END=>Ellipsis::End, TextStyle::ELLIPSIS_MIDDLE=>Ellipsis::Middle, TextStyle::ELLIPSIS_HEAD=>Ellipsis::Head, _=>return None },
        tracking: value.tracking,
        features: value.features,
    })
}

pub(crate) unsafe fn read_style(value:*const TextStyle)->Option<Style> {
    if value.is_null() { return None; }
    style(read_desc!(value,TextStyle)?)
}

pub(crate) unsafe fn span_packet(value:*const TextSpans)->Option<TextSpans> {
    if value.is_null() { return None; }
    let size=unsafe { value.cast::<u32>().read_unaligned() };
    if size<(core::mem::size_of::<TextSpans>() as u32) { return None; }
    let count=unsafe { core::ptr::addr_of!((*value).count).read_unaligned() };
    if count>TextSpans::MAX as u32 { return None; }
    let mut packet=TextSpans { count,..Default::default() };
    for i in 0..count as usize {
        let raw=unsafe { core::ptr::addr_of!((*value).spans).cast::<pxui_types::TextSpan>().add(i) };
        let flags=unsafe { core::ptr::addr_of!((*raw).flags).read_unaligned() };
        if flags & !(pxui_types::TextSpan::COLOR|pxui_types::TextSpan::BACKGROUND|pxui_types::TextSpan::UNDERLINE)!=0 { return None; }
        let span=&mut packet.spans[i];
        span.flags=flags;
        span.start=unsafe { core::ptr::addr_of!((*raw).start).read_unaligned() };
        span.end=unsafe { core::ptr::addr_of!((*raw).end).read_unaligned() };
        span.weight=unsafe { core::ptr::addr_of!((*raw).weight).read_unaligned() };
        if flags & pxui_types::TextSpan::COLOR!=0 { span.color=unsafe { core::ptr::addr_of!((*raw).color).read_unaligned() }; }
        if flags & pxui_types::TextSpan::BACKGROUND!=0 { span.background=unsafe { core::ptr::addr_of!((*raw).background).read_unaligned() }; }
        if flags & pxui_types::TextSpan::UNDERLINE!=0 { span.underline=unsafe { core::ptr::addr_of!((*raw).underline).read_unaligned() }; }
    }
    Some(packet)
}

unsafe fn measure_spans(text:&mut Text,key:u64,value:Str,style_:*const TextStyle,max_width:f32,spans:*const TextSpans,out:*mut TextMetrics)->i32 {
    if style_.is_null() || out.is_null() || !max_width.is_finite() { return PXUI_ERR_ARGUMENT; }
    let (Some(value),Some(style),Some(spans))=(unsafe { utf8(value) },unsafe { read_style(style_) },unsafe { span_packet(spans) }) else { return PXUI_ERR_ARGUMENT };
    let Some(value)=text.layout_spans(key,value,&style,max_width,spans.as_slice().unwrap()) else { return PXUI_ERR_ARGUMENT };
    unsafe { out.write(metrics(value)); }
    0
}

export!(bare pub unsafe fn pxui_shaper_width_spans(shaper:*mut PxuiShaper,key:u64,value:Str,style_:*const TextStyle,max_width:f32,spans:*const TextSpans,out:*mut TextMetrics)->i32 {
    let Some(shaper)=(unsafe { shaper.as_mut() }) else { return PXUI_ERR_ARGUMENT };
    unsafe { measure_spans(&mut shaper.text,key,value,style_,max_width,spans,out) }
});

export!(pub unsafe fn pxui_text_width_spans(ui:*mut PxuiUi,key:u64,value:Str,style_:*const TextStyle,max_width:f32,spans:*const TextSpans,out:*mut TextMetrics)->i32 {
    let Some(engine)=(unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT };
    unsafe { measure_spans(engine.text_mut(),key,value,style_,max_width,spans,out) }
});

export!(pub unsafe fn pxui_ui_text_spans(ui:*mut PxuiUi,box_:*const crate::PxuiBox,value:Str,style_:*const TextStyle,spans:*const TextSpans)->i32 {
    let Some(state)=(unsafe { crate::state::get(ui) }) else { return PXUI_ERR_ARGUMENT };
    if !state.shared().in_frame || box_.is_null() || style_.is_null() { return unsafe { crate::fail(ui,"text spans need a frame, a node and a style") }; }
    let (Some(value),Some(style),Some(spans),Some(box_))=(unsafe { crate::state::checked(value) },unsafe { read_style(style_) },unsafe { span_packet(spans) },crate::state::core_box(unsafe { *box_ })) else { return unsafe { crate::fail(ui,"invalid text spans") } };
    if !unsafe { &mut *state.engine }.text_spans(&box_,value,&style,spans.as_slice().unwrap()) { return unsafe { crate::fail(ui,"invalid text span ranges or node") }; }
    0
});

pub(crate) fn metrics(value: pxui_text::Metrics) -> TextMetrics {
    TextMetrics { width: value.w, height: value.h, baseline: value.baseline, lines: value.lines }
}

/// A text shaper of its own, for text laid out off the `Ui` (on any thread): its own font system and no GPU atlas.
pub struct PxuiShaper { text: Text }

const MAX_FONTS: usize = 128;

export!(bare pub unsafe fn pxui_shaper_create(fonts: *const Str, count: usize) -> *mut PxuiShaper {
    if count > MAX_FONTS || (count != 0 && fonts.is_null()) { return core::ptr::null_mut(); }
    let mut font_bytes = Vec::with_capacity(count);
    if count != 0 {
        for font in unsafe { std::slice::from_raw_parts(fonts, count) } {
            let Some(font) = (unsafe { bytes(*font) }) else { return core::ptr::null_mut(); };
            font_bytes.push(font);
        }
    }
    Box::into_raw(Box::new(PxuiShaper { text: Text::new(&font_bytes) }))
});

export!(bare pub unsafe fn pxui_shaper_destroy(shaper: *mut PxuiShaper) { if !shaper.is_null() { drop(unsafe { Box::from_raw(shaper) }); } });

export!(bare pub unsafe fn pxui_shaper_add_font(shaper: *mut PxuiShaper, font: Str) -> u32 {
    let (Some(shaper), Some(font)) = (unsafe { shaper.as_mut() }, unsafe { bytes(font) }) else { return u32::MAX; };
    u32::try_from(shaper.text.add_font(font)).unwrap_or(u32::MAX)
});

export!(bare pub unsafe fn pxui_shaper_width(shaper: *mut PxuiShaper, key: u64, value: Str, style_: *const TextStyle, max_width: f32, out: *mut TextMetrics) -> i32 {
    let Some(shaper) = (unsafe { shaper.as_mut() }) else { return PXUI_ERR_ARGUMENT; };
    unsafe { measure(&mut shaper.text, key, value, style_, max_width, out) }
});

export!(bare pub unsafe fn pxui_shaper_rasterize_rgba(
    shaper: *mut PxuiShaper, key: u64, value: Str, style_: *const TextStyle, max_width: f32, scale: f32,
    color_r: u8, color_g: u8, color_b: u8, color_a: u8, width: *mut u32, height: *mut u32, out: *mut u8, capacity: usize,
) -> usize {
    let Some(shaper) = (unsafe { shaper.as_mut() }) else { return usize::MAX; };
    unsafe { rasterize(&mut shaper.text, true, key, value, style_, max_width, scale, color_r, color_g, color_b, color_a, width, height, out, capacity) }
});

// The default font (Pragmasevka) the library embeds: its bytes live as long as the library.
export!(bare pub unsafe fn pxui_default_font() -> Str { Str { ptr: pxui_text::PRAGMASEVKA.as_ptr(), len: pxui_text::PRAGMASEVKA.len() } });

// Adds a font (its bytes are copied) to the Ui's text service; its index, or `u32::MAX` when refused.
export!(failing(u32::MAX) pub unsafe fn pxui_text_add_font(ui: *mut PxuiUi, font: Str) -> u32 {
    let Some(engine) = (unsafe { engine(ui) }) else { return u32::MAX; };
    let Some(font) = (unsafe { bytes(font) }) else { return u32::MAX; };
    u32::try_from(engine.text_mut().add_font(font)).unwrap_or(u32::MAX)
});

unsafe fn measure(text: &mut Text, key: u64, value: Str, style_: *const TextStyle, max_width: f32, out: *mut TextMetrics) -> i32 {
    if style_.is_null() || out.is_null() || !max_width.is_finite() { return PXUI_ERR_ARGUMENT; }
    let Some(value) = (unsafe { utf8(value) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(style) = (unsafe { read_style(style_) }) else { return PXUI_ERR_ARGUMENT; };
    let measured = text.layout(key, value, &style, max_width);
    unsafe { out.write(metrics(measured)); }
    0
}

// A text's glyph quads (see `pxui_text_quads`).
unsafe fn quads(
    text: &mut Text,
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
    if style_.is_null() || (capacity != 0 && out.is_null())
        || !origin_x.is_finite() || !origin_y.is_finite() || !scale.is_finite() || scale <= 0.0
    { return usize::MAX; }
    let Some(value) = (unsafe { utf8(value) }) else { return usize::MAX; };
    let Some(style) = (unsafe { read_style(style_) }) else { return usize::MAX; };
    {
        text.layout(key, value, &style, max_width);
        let mut count = 0;
        text.glyphs(key, (origin_x, origin_y), scale, &mut |glyph| {
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
}

// Renders shaped text to straight-alpha RGBA8. A null/short output buffer is a size query; the
// dimensions and required byte count are still returned. The caller retries with that capacity.
unsafe fn rasterize(
    text: &mut Text,
    standalone: bool,
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
    if style_.is_null() || width.is_null() || height.is_null()
        || !max_width.is_finite() || !scale.is_finite() || scale <= 0.0
        || (capacity != 0 && out.is_null())
    { return usize::MAX; }
    let Some(value) = (unsafe { utf8(value) }) else { return usize::MAX; };
    let Some(style) = (unsafe { read_style(style_) }) else { return usize::MAX; };
    {
        let metrics = text.layout(key, value, &style, max_width);
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
        text.glyphs(key, (0.0, 0.0), scale, &mut |glyph| glyphs.push(glyph));
        for glyph in glyphs {
            let (atlas_w, atlas_h) = text.atlas_size(glyph.atlas);
            let atlas = text.atlas_pixels(glyph.atlas);
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
        // a shaper nothing draws from keeps no uploads pending; the Ui's text leaves them to its renderer
        if standalone { text.drain_uploads(&mut |_, _, _, _, _, _, _| {}); }
        required
    }
}

fn over(dst: &mut [u8], src: [u8; 4]) {
    let (sa, da) = (src[3] as u32, dst[3] as u32);
    let oa = sa * 255 + da * (255 - sa);
    if oa == 0 { return; }
    for i in 0..3 {
        dst[i] = ((src[i] as u32 * sa * 255 + dst[i] as u32 * da * (255 - sa)) / oa) as u8;
    }
    dst[3] = ((oa + 127) / 255) as u8;
}


// Measures `value` in the Ui's text service: the one font system and glyph atlas the Ui itself draws with. `key` names the
// layout in the service's cache.
export!(pub unsafe fn pxui_text_width(ui: *mut PxuiUi, key: u64, value: Str, style_: *const TextStyle, max_width: f32, out: *mut TextMetrics) -> i32 {
    let Some(engine) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    unsafe { measure(engine.text_mut(), key, value, style_, max_width, out) }
});

// The glyph quads of `value` in the Ui's text service, for an app pass that samples the atlases of `pxui_renderer_atlas`. Returns
// the total glyph count, or `usize::MAX` for invalid arguments; when `capacity` is smaller only the first `capacity` are written.
export!(failing(usize::MAX) pub unsafe fn pxui_text_quads(
    ui: *mut PxuiUi, key: u64, value: Str, style_: *const TextStyle, max_width: f32, origin_x: f32, origin_y: f32, scale: f32, out: *mut TextGlyph, capacity: usize,
) -> usize {
    let Some(engine) = (unsafe { engine(ui) }) else { return usize::MAX; };
    unsafe { quads(engine.text_mut(), key, value, style_, max_width, origin_x, origin_y, scale, out, capacity) }
});

export!(failing(usize::MAX) pub unsafe fn pxui_text_rasterize_rgba(
    ui: *mut PxuiUi, key: u64, value: Str, style_: *const TextStyle, max_width: f32, scale: f32,
    color_r: u8, color_g: u8, color_b: u8, color_a: u8, width: *mut u32, height: *mut u32, out: *mut u8, capacity: usize,
) -> usize {
    let Some(engine) = (unsafe { engine(ui) }) else { return usize::MAX; };
    unsafe { rasterize(engine.text_mut(), false, key, value, style_, max_width, scale, color_r, color_g, color_b, color_a, width, height, out, capacity) }
});

#[cfg(test)]
mod tests {
    use super::{over, style};
    use pxui_types::TextStyle;

    #[test]
    fn text_style_validates_codes_and_numbers() {
        assert!(style(TextStyle::default()).is_some());
        let enabled=style(TextStyle { features:TextStyle::FEATURE_TABULAR_NUMBERS,..Default::default() }).unwrap();
        assert_eq!(enabled.features,pxui_text::Style::FEATURE_TABULAR_NUMBERS);
        for features in [2,3,u32::MAX] { assert!(style(TextStyle { features,..Default::default() }).is_none()); }
        let mut invalid = TextStyle::default();
        invalid.wrap = u32::MAX;
        assert!(style(invalid).is_none());
        invalid = TextStyle::default();
        invalid.size_points = f32::NAN;
        assert!(style(invalid).is_none());
    }

    #[test]
    fn ellipsis_modes_preserve_existing_codes_and_refuse_unknown_values() {
        for (ellipsis,expected) in [(TextStyle::ELLIPSIS_NONE,pxui_text::Ellipsis::None),
            (TextStyle::ELLIPSIS_END,pxui_text::Ellipsis::End),
            (TextStyle::ELLIPSIS_MIDDLE,pxui_text::Ellipsis::Middle),
            (TextStyle::ELLIPSIS_HEAD,pxui_text::Ellipsis::Head)] {
            assert_eq!(style(TextStyle { ellipsis,..Default::default() }).unwrap().ellipsis,expected);
        }
        for ellipsis in 4..=u8::MAX {
            assert!(style(TextStyle { ellipsis,..Default::default() }).is_none());
        }
    }

    #[test]
    fn span_packets_refuse_null_short_records_and_excess_counts() {
        assert!(unsafe { super::span_packet(core::ptr::null()) }.is_none());
        let short=[8u32,0];
        assert!(unsafe { super::span_packet(short.as_ptr().cast()) }.is_none());
        let invalid_header=[core::mem::size_of::<pxui_types::TextSpans>() as u32,9];
        assert!(unsafe { super::span_packet(invalid_header.as_ptr().cast()) }.is_none());
        let empty_header=[core::mem::size_of::<pxui_types::TextSpans>() as u32,0];
        assert_eq!(unsafe { super::span_packet(empty_header.as_ptr().cast()) }.unwrap().count,0);
        let invalid=pxui_types::TextSpans { count:9,..Default::default() };
        assert!(unsafe { super::span_packet(&invalid) }.is_none());
        let full=pxui_types::TextSpans::default();
        assert!(unsafe { super::span_packet(&full) }.is_some());
        #[repr(C)]
        struct Prefix { size:u32,count:u32,start:u32,end:u32,weight:u16,flags:u16 }
        let prefix=Prefix { size:core::mem::size_of::<pxui_types::TextSpans>() as u32,count:1,start:0,end:2,weight:700,flags:0 };
        let decoded=unsafe { super::span_packet((&prefix as *const Prefix).cast()) }.unwrap();
        assert_eq!(decoded.spans[0].weight,700);
        assert_eq!(decoded.spans[0].color,Default::default(),"unflagged colour fields are not read");
    }

    #[test]
    fn text_style_reader_refuses_null_and_short_records() {
        assert!(unsafe { super::read_style(core::ptr::null()) }.is_none());
        let short=[8u32,0];
        assert!(unsafe { super::read_style(short.as_ptr().cast()) }.is_none());
        assert!(unsafe { super::read_style(&TextStyle::default()) }.is_some());
    }

    #[test]
    fn straight_alpha_compositing_keeps_source_color() {
        let mut pixel = [0, 0, 0, 0];
        over(&mut pixel, [220, 40, 10, 128]);
        assert_eq!(pixel, [220, 40, 10, 128]);
    }
}
