//! Read leading TextStyle size before copying, including legacy measure/quads/raster/canvas calls.
//! Reject unknown features/ellipsis, invalid UTF-8 span boundaries, unordered/overlapping/empty
//! ranges and invalid flags/lengths before mutation. Validate packet count before records; read
//! only active records and flagged colours. Preserve short-prefix descriptor support. UI system-
//! font discovery must precede the first frame; standalone enabling invalidates old layouts once
//! and repeated enabling is idempotent.
//!
use pxui_text::{Align, AtlasKind, Ellipsis, Style, Text, Wrap};
use crate::state::{PxuiUi, engine};
use crate::PXUI_ERR_ARGUMENT;
use pxui_types::{ffi::PxuiTextStyle as TextStyle, text as T, Str, TextGlyph, TextMetrics, TextSpans};

unsafe fn bytes<'a>(value: Str) -> Option<&'a [u8]> {
    if value.len == 0 { return Some(&[]); }
    if value.ptr.is_null() { return None; }
    Some(unsafe { std::slice::from_raw_parts(value.ptr, value.len) })
}

/// A font's bytes: refused when longer than `MAX_FONT_BYTES`, before any read of the caller's memory.
unsafe fn font_slice<'a>(value: Str) -> Option<&'a [u8]> {
    if value.len > pxui_text::MAX_FONT_BYTES { return None; }
    unsafe { bytes(value) }
}

/// Text to measure or shape: every shaper, text-service and spans entry takes its text here, editor text (`state::checked_text`): over
/// `MAX_TEXT_BYTES` is refused (`PXUI_ERR_ARGUMENT`) before any read.
unsafe fn utf8<'a>(value: Str) -> Option<&'a str> { unsafe { crate::state::checked_text(value) } }

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
        wrap: match crate::ui::wrap_of(value.wrap)? { T::Wrap::None => Wrap::None, T::Wrap::Word => Wrap::Word, T::Wrap::Glyph => Wrap::Glyph, T::Wrap::WordSpaces => Wrap::WordSpaces },
        align: match crate::ui::align_of(value.align)? { T::TextAlign::Start => Align::Left, T::TextAlign::Center => Align::Center, T::TextAlign::End => Align::Right },
        max_lines: value.max_lines,
        ellipsis: match crate::ui::ellipsis_of(value.ellipsis)? { T::Ellipsis::None=>Ellipsis::None, T::Ellipsis::End=>Ellipsis::End, T::Ellipsis::Middle=>Ellipsis::Middle, T::Ellipsis::Head=>Ellipsis::Head },
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
    let key=if key==0 {pxui_text::content_key(&style,max_width,value,spans.as_slice().unwrap())} else {key};
    let Some(value)=text.layout_spans(key,value,&style,max_width,spans.as_slice().unwrap()) else { return PXUI_ERR_ARGUMENT };
    unsafe { out.write(metrics(value)); }
    0
}

export!(object pub unsafe fn pxui_shaper_width_spans(shaper:*mut PxuiShaper,key:u64,value:Str,style_:*const TextStyle,max_width:f32,spans:*const TextSpans,out:*mut TextMetrics)->i32 {
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

export!(pub unsafe fn pxui_ui_text_line_box(ui:*mut PxuiUi,box_:*const crate::PxuiBox,value:Str,style_:*const TextStyle,spans:*const TextSpans)->i32 {
    let Some(state)=(unsafe { crate::state::get(ui) }) else { return PXUI_ERR_ARGUMENT };
    if !state.shared().in_frame || box_.is_null() || style_.is_null() { return unsafe { crate::fail(ui,"line-box text needs a frame, a node and a style") }; }
    let (Some(value),Some(style),Some(spans),Some(box_))=(unsafe { crate::state::checked(value) },unsafe { read_style(style_) },unsafe { span_packet(spans) },crate::state::core_box(unsafe { *box_ })) else { return unsafe { crate::fail(ui,"invalid line-box text") } };
    if !unsafe { &mut *state.engine }.text_line_box(&box_,value,&style,spans.as_slice().unwrap()) { return unsafe { crate::fail(ui,"invalid line-box font, height, spans or node") }; }
    0
});

pub(crate) fn metrics(value: pxui_text::Metrics) -> TextMetrics {
    TextMetrics { width: value.w, height: value.h, baseline: value.baseline, lines: value.lines }
}

/// A text shaper of its own, for text laid out off the `Ui` (on any thread): its own font system and no GPU atlas.
pub struct PxuiShaper { text: Text, raster_glyphs: Vec<pxui_text::Glyph>, pub(crate) poisoned: bool }

/// Owned entirely by the library. Installation replaces its contents with retired UI storage.
pub struct PxuiPreparedText { prepared: pxui_text::Prepared }

export!(pub unsafe fn pxui_text_fork(ui:*mut PxuiUi)->*mut PxuiShaper {
    let Some(state)=(unsafe { crate::state::get(ui) }) else { return core::ptr::null_mut() };
    if state.shared().in_frame { return core::ptr::null_mut(); }
    let text=unsafe { &mut *state.engine }.text_mut().preparer();
    Box::into_raw(Box::new(PxuiShaper { text, raster_glyphs: Vec::new(), poisoned: false }))
});

export!(object pub unsafe fn pxui_shaper_prepare(shaper:*mut PxuiShaper,value:Str,style_:*const TextStyle,max_width:f32,spans:*const TextSpans,out:*mut *mut PxuiPreparedText,out_metrics:*mut TextMetrics)->i32 {
    if out.is_null() || out_metrics.is_null() { return PXUI_ERR_ARGUMENT; }
    let (Some(shaper),Some(value),Some(style),Some(spans))=(unsafe { shaper.as_mut() },unsafe { utf8(value) },unsafe { read_style(style_) },unsafe { span_packet(spans) }) else { return PXUI_ERR_ARGUMENT };
    let Some(prepared)=shaper.text.prepare(value,&style,max_width,spans.as_slice().unwrap()) else { return PXUI_ERR_ARGUMENT };
    let measured=metrics(prepared.metrics());
    let record=Box::into_raw(Box::new(PxuiPreparedText { prepared }));
    unsafe { out.write(record); out_metrics.write(measured); }
    0
});

export!(pub unsafe fn pxui_text_install(ui:*mut PxuiUi,record:*mut PxuiPreparedText)->i32 {
    let Some(state)=(unsafe { crate::state::get(ui) }) else { return PXUI_ERR_ARGUMENT };
    if state.shared().in_frame { return crate::PXUI_ERR_STATE; }
    let Some(record)=(unsafe { record.as_mut() }) else { return PXUI_ERR_ARGUMENT };
    if unsafe { &mut *state.engine }.text_mut().install(&mut record.prepared) { 0 } else { crate::PXUI_ERR_STATE }
});

export!(bare pub unsafe fn pxui_prepared_destroy(record:*mut PxuiPreparedText) {
    if !record.is_null() { drop(unsafe { Box::from_raw(record) }); }
});

#[cfg(test)]
mod prepared_tests {
    use super::*;

    fn value(text:&str)->Str { Str { ptr:text.as_ptr(),len:text.len() } }

    #[test]
    fn prepared_text_validation_preserves_outputs_and_reads_only_descriptor_prefixes() {
        unsafe {
            let shaper=pxui_shaper_create(core::ptr::null(),0);
            let style=TextStyle::default();
            let spans=TextSpans::default();
            let short=4u32;
            let oversized_count=[core::mem::size_of::<TextSpans>() as u32,9];
            let mid_utf8=TextSpans::new(&[pxui_types::TextSpan {start:1,end:2,..Default::default()}]).unwrap();
            let bad_utf8=[255u8];
            let mut out=core::ptr::dangling_mut();
            let sentinel=out;
            let mut measured=TextMetrics {width:-99.0,height:-99.0,baseline:-99.0,lines:777};
            for (text,style,width,spans) in [
                (value("valid"),(&short as *const u32).cast(),200.0,&spans as *const TextSpans),
                (value("valid"),&style,200.0,(&short as *const u32).cast()),
                (value("valid"),&style,200.0,oversized_count.as_ptr().cast()),
                (value("é"),&style,200.0,&mid_utf8),
                (value("valid"),&style,f32::NAN,&spans),
                (Str {ptr:bad_utf8.as_ptr(),len:1},&style,200.0,&spans),
            ] {
                assert_eq!(pxui_shaper_prepare(shaper,text,style,width,spans,&mut out,&mut measured),PXUI_ERR_ARGUMENT);
                assert_eq!(out,sentinel);
                assert_eq!((measured.width,measured.height,measured.baseline,measured.lines),(-99.0,-99.0,-99.0,777));
            }
            assert_eq!(pxui_shaper_prepare(shaper,value("valid"),&style,200.0,&spans,core::ptr::null_mut(),&mut measured),PXUI_ERR_ARGUMENT);
            assert_eq!(pxui_shaper_prepare(shaper,value("valid"),&style,200.0,&spans,&mut out,core::ptr::null_mut()),PXUI_ERR_ARGUMENT);
            pxui_shaper_destroy(shaper);
            pxui_prepared_destroy(core::ptr::null_mut());
        }
    }

    #[test]
    fn prepared_text_refuses_foreign_retired_and_callback_handles_without_losing_ownership() {
        struct Context {owner:*mut PxuiUi,record:*mut PxuiPreparedText}
        extern "C" fn build(context:*mut core::ffi::c_void,borrowed:*mut core::ffi::c_void) {
            let context=unsafe { &mut *context.cast::<Context>() };
            for ui in [context.owner,borrowed.cast()] {
                assert!(unsafe { pxui_text_fork(ui) }.is_null());
                assert_eq!(unsafe { pxui_text_install(ui,context.record) },crate::PXUI_ERR_STATE);
            }
        }
        unsafe {
            let ui=crate::pxui_ui_create(&crate::PxuiUiDesc::default());
            let other=crate::pxui_ui_create(&crate::PxuiUiDesc::default());
            let shaper=pxui_text_fork(ui);
            let other_shaper=pxui_text_fork(other);
            assert!(!shaper.is_null() && !other_shaper.is_null());
            let mut record=core::ptr::null_mut();
            let mut measured=TextMetrics::default();
            let style=TextStyle::default();
            let spans=TextSpans::default();
            assert_eq!(pxui_shaper_prepare(shaper,value("prepared"),&style,200.0,&spans,&mut record,&mut measured),0);
            pxui_shaper_destroy(shaper);
            pxui_shaper_destroy(other_shaper);
            assert_eq!(pxui_text_install(core::ptr::null_mut(),record),PXUI_ERR_ARGUMENT);
            assert_eq!(pxui_text_install(ui,core::ptr::null_mut()),PXUI_ERR_ARGUMENT);
            assert_eq!(pxui_text_install(other,record),crate::PXUI_ERR_STATE);
            let mut context=Context {owner:ui,record};
            let mut output=crate::FrameOut::default();
            assert_eq!(crate::pxui_frame(ui,&crate::FrameIn {width:320.0,height:240.0,..Default::default()},Some(build),(&mut context as *mut Context).cast(),&mut output),0);
            // A split renderer callback owns the engine even though no UI frame is active.
            (*(*ui).shared).current=core::ptr::null_mut();
            assert!(pxui_text_fork(ui).is_null());
            assert_eq!(pxui_text_install(ui,record),crate::PXUI_ERR_STATE);
            (*(*ui).shared).current=ui;
            let layouts=engine(ui).unwrap().text_mut().layout_count();
            assert_eq!(pxui_text_install(ui,record),0);
            let mut actual=TextMetrics::default();
            assert_eq!(pxui_text_width_spans(ui,0,value("prepared"),&style,200.0,&spans,&mut actual),0);
            assert_eq!((actual.width,actual.height,actual.baseline,actual.lines),(measured.width,measured.height,measured.baseline,measured.lines));
            assert_eq!(engine(ui).unwrap().text_mut().layout_count(),layouts);
            assert_eq!(pxui_text_install(ui,record),crate::PXUI_ERR_STATE);
            pxui_prepared_destroy(record);
            assert_eq!(crate::pxui_ui_destroy(ui)|crate::pxui_ui_destroy(other),0);
        }
    }
}


// A shaper over `fonts` (copied; the first is the default); null when there is none (shaping needs a font: one without any would fail
// at its first text), when one is refused, or past the font limits.
export!(bare pub unsafe fn pxui_shaper_create(fonts: *const Str, count: usize) -> *mut PxuiShaper {
    if count == 0 || count > pxui_text::MAX_FONTS || fonts.is_null() { return core::ptr::null_mut(); }
    let mut font_bytes = Vec::with_capacity(count);
    for font in unsafe { std::slice::from_raw_parts(fonts, count) } {
        let Some(font) = (unsafe { font_slice(*font) }) else { return core::ptr::null_mut(); };
        font_bytes.push(font);
    }
    // each font within the per-font and total caps and parseable: a refused font refuses the shaper
    if font_bytes.iter().map(|font| font.len()).sum::<usize>() > pxui_text::MAX_FONT_TOTAL_BYTES { return core::ptr::null_mut(); }
    let mut text = Text::new(&font_bytes);
    if text.font_count() != font_bytes.len() { return core::ptr::null_mut(); }
    // the same default as `Ui` (ui.rs `platform`), so a standalone shaper rasterises like the UI's
    text.set_apple_raster(cfg!(any(target_os = "macos", target_os = "ios")));
    Box::into_raw(Box::new(PxuiShaper { text, raster_glyphs: Vec::new(), poisoned: false }))
});

export!(bare pub unsafe fn pxui_shaper_destroy(shaper: *mut PxuiShaper) { if !shaper.is_null() { drop(unsafe { Box::from_raw(shaper) }); } });

export!(object pub unsafe fn pxui_shaper_system_fonts(shaper:*mut PxuiShaper)->i32 {
    let Some(shaper)=(unsafe { shaper.as_mut() }) else { return PXUI_ERR_ARGUMENT };
    if shaper.text.enable_system_fonts() { 0 } else { crate::PXUI_ERR_STATE }
});

export!(pub unsafe fn pxui_text_system_fonts(ui:*mut PxuiUi)->i32 {
    let Some(engine)=(unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT };
    if engine.frame_index()!=0 { return unsafe { crate::state::fail_code(ui,"enable system fonts before the first frame",crate::PXUI_ERR_STATE) }; }
    if !engine.text_mut().enable_system_fonts() { return unsafe { crate::state::fail_code(ui,"no system font provider for this platform",crate::PXUI_ERR_STATE) }; }
    0
});

export!(object failing(u32::MAX) pub unsafe fn pxui_shaper_add_font(shaper: *mut PxuiShaper, font: Str) -> u32 {
    let (Some(shaper), Some(font)) = (unsafe { shaper.as_mut() }, unsafe { font_slice(font) }) else { return u32::MAX; };
    shaper.text.add_font(font).and_then(|index| u32::try_from(index).ok()).unwrap_or(u32::MAX)
});

export!(object pub unsafe fn pxui_shaper_width(shaper: *mut PxuiShaper, key: u64, value: Str, style_: *const TextStyle, max_width: f32, out: *mut TextMetrics) -> i32 {
    let Some(shaper) = (unsafe { shaper.as_mut() }) else { return PXUI_ERR_ARGUMENT; };
    unsafe { measure(&mut shaper.text, key, value, style_, max_width, out) }
});

export!(object failing(usize::MAX) pub unsafe fn pxui_shaper_rasterize_rgba(
    shaper: *mut PxuiShaper, key: u64, value: Str, style_: *const TextStyle, max_width: f32, scale: f32,
    color_r: u8, color_g: u8, color_b: u8, color_a: u8, width: *mut u32, height: *mut u32, out: *mut u8, capacity: usize,
) -> usize {
    let Some(shaper) = (unsafe { shaper.as_mut() }) else { return usize::MAX; };
    unsafe { rasterize(&mut shaper.text, &mut shaper.raster_glyphs, true, key, value, style_, max_width, scale, color_r, color_g, color_b, color_a, width, height, out, capacity) }
});

// The default font (Pragmasevka) the library embeds: its bytes live as long as the library.
export!(bare pub unsafe fn pxui_default_font() -> Str { Str { ptr: pxui_text::PRAGMASEVKA.as_ptr(), len: pxui_text::PRAGMASEVKA.len() } });

// Adds a font (its bytes are copied) to the Ui's text service; its index, or `u32::MAX` when refused.
export!(failing(u32::MAX) pub unsafe fn pxui_text_add_font(ui: *mut PxuiUi, font: Str) -> u32 {
    let Some(engine) = (unsafe { engine(ui) }) else { return u32::MAX; };
    let Some(font) = (unsafe { font_slice(font) }) else { return u32::MAX; };
    engine.text_mut().add_font(font).and_then(|index| u32::try_from(index).ok()).unwrap_or(u32::MAX)
});

unsafe fn measure(text: &mut Text, key: u64, value: Str, style_: *const TextStyle, max_width: f32, out: *mut TextMetrics) -> i32 {
    if style_.is_null() || out.is_null() || !max_width.is_finite() { return PXUI_ERR_ARGUMENT; }
    let Some(value) = (unsafe { utf8(value) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(style) = (unsafe { read_style(style_) }) else { return PXUI_ERR_ARGUMENT; };
    let key = if key == 0 { pxui_text::content_key(&style, max_width, value, &[]) } else { key };
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
        let key = if key == 0 { pxui_text::content_key(&style, max_width, value, &[]) } else { key };
        text.layout(key, value, &style, max_width);
        let mut count = 0;
        text.glyphs(key, (origin_x, origin_y), scale, &mut |glyph| {
            if count < capacity {
                let atlas = glyph.atlas as u32;
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
    scratch: &mut Vec<pxui_text::Glyph>,
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
        let key = if key == 0 { pxui_text::content_key(&style, max_width, value, &[]) } else { key };
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
        let mut glyphs = core::mem::take(scratch);
        glyphs.clear();
        text.glyphs(key, (0.0, 0.0), scale, &mut |glyph| glyphs.push(glyph));
        for &glyph in &glyphs {
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
                        AtlasKind::Mask|AtlasKind::Mask2 => [ink[0], ink[1], ink[2], ((atlas[at as usize] as u32 * alpha + 127) / 255) as u8],
                        AtlasKind::Color|AtlasKind::Color2 => {
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
        *scratch = glyphs;
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

/// The prepare queue's worker: one thread the host owns, shaping for every `Ui` attached to it.
pub struct PxuiTextWorker { worker: pxui_text::Worker }

// Starts a prepare-queue worker (one thread); null when the platform cannot start one.
export!(bare pub unsafe fn pxui_text_worker_create() -> *mut PxuiTextWorker {
    pxui_text::Worker::start().map_or(core::ptr::null_mut(), |worker| Box::into_raw(Box::new(PxuiTextWorker { worker })))
});

// Stops a worker (waiting for at most one shaping call); the Uis still attached shape inline from then on. Null is allowed.
export!(bare pub unsafe fn pxui_text_worker_destroy(worker: *mut PxuiTextWorker) {
    if !worker.is_null() { drop(unsafe { Box::from_raw(worker) }); }
});

// Between frames, on the Ui's thread, attaches the Ui to `worker` (null: detaches); a Ui keeps no pointer to the worker object.
// PXUI_ERR_STATE inside a frame or when the worker serves MAX_LANES Uis (the Ui is left detached). The worker pointer is unchecked:
// one pxui_text_worker_create returned and not destroyed (destroy is serialized against this call by the host).
export!(pub unsafe fn pxui_ui_set_text_worker(ui: *mut PxuiUi, worker: *const PxuiTextWorker) -> i32 {
    let Some(state) = (unsafe { crate::state::get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if state.shared().in_frame { return crate::PXUI_ERR_STATE; }
    let worker = unsafe { worker.as_ref() }.map(|worker| &worker.worker);
    if unsafe { &mut *state.engine }.set_text_worker(worker) { 0 } else { crate::PXUI_ERR_STATE }
});

// Requests `value` in `style` within `max_width` points (0: unbounded) from the attached worker: shaped there and installed
// before a later frame builds. Nothing without a worker, or when cached or already requested.
export!(pub unsafe fn pxui_text_prewarm(ui: *mut PxuiUi, value: Str, style_: *const TextStyle, max_width: f32) -> i32 {
    if style_.is_null() { return PXUI_ERR_ARGUMENT; }
    let Some(engine) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let (Some(value), Some(style)) = (unsafe { utf8(value) }, unsafe { read_style(style_) }) else { return PXUI_ERR_ARGUMENT; };
    if engine.prewarm_text(value, &style, max_width) { 0 } else { PXUI_ERR_ARGUMENT }
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
    let Some(state) = (unsafe { crate::state::get(ui) }) else { return usize::MAX; };
    let (engine, shared) = (unsafe { &mut *state.engine }, state.shared());
    unsafe { rasterize(engine.text_mut(), &mut shared.raster_glyphs, false, key, value, style_, max_width, scale, color_r, color_g, color_b, color_a, width, height, out, capacity) }
});

#[cfg(test)]
mod tests {
    use super::{over, style};
    use pxui_types::ffi::PxuiTextStyle as TextStyle;

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
        use pxui_types::text::Ellipsis as E;
        for (ellipsis,expected) in [(E::None,pxui_text::Ellipsis::None),
            (E::End,pxui_text::Ellipsis::End),
            (E::Middle,pxui_text::Ellipsis::Middle),
            (E::Head,pxui_text::Ellipsis::Head)] {
            assert_eq!(style(TextStyle { ellipsis:ellipsis as u8,..Default::default() }).unwrap().ellipsis,expected);
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

// ABI 1.65: the text calls on a label style (`PxuiLabelStyle`; `digits` ignored), resolved against the Ui's theme as
// `pxui_ui_resolve_label` resolves it, in the same call. A null, invalid or unresolvable style is refused like an invalid style of
// the resolved-style twin (`pxui_ui_text_spans`, `pxui_ui_text_line_box`, `pxui_text_width`/`_spans`, `pxui_text_quads`,
// `pxui_text_rasterize_rgba`, `pxui_text_prewarm`); the layout key is the content's (key 0).

export!(pub unsafe fn pxui_ui_label_text_spans(ui:*mut PxuiUi,box_:*const crate::PxuiBox,value:Str,style:*const pxui_types::ffi::PxuiLabelStyle,spans:*const TextSpans)->i32 {
    if unsafe { crate::state::get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some((style,_))=crate::ui::label_style(style) else { return unsafe { crate::fail(ui,"invalid label style") } };
    unsafe { crate::trusted::label_text_spans(ui,box_,value,&style,spans) }
});

export!(pub unsafe fn pxui_ui_label_text_line_box(ui:*mut PxuiUi,box_:*const crate::PxuiBox,value:Str,style:*const pxui_types::ffi::PxuiLabelStyle,spans:*const TextSpans)->i32 {
    if unsafe { crate::state::get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some((style,_))=crate::ui::label_style(style) else { return unsafe { crate::fail(ui,"invalid label style") } };
    unsafe { crate::trusted::label_text_line_box(ui,box_,value,&style,spans) }
});

// `spans` may be null: the plain measurement.
export!(pub unsafe fn pxui_ui_label_measure(ui:*mut PxuiUi,value:Str,style:*const pxui_types::ffi::PxuiLabelStyle,max_width:f32,spans:*const TextSpans,out:*mut TextMetrics)->i32 {
    let Some((style,_))=crate::ui::label_style(style) else { return PXUI_ERR_ARGUMENT };
    unsafe { crate::trusted::label_measure(ui,value,&style,max_width,spans,out) }
});

export!(failing(usize::MAX) pub unsafe fn pxui_ui_label_quads(
    ui: *mut PxuiUi, value: Str, style: *const pxui_types::ffi::PxuiLabelStyle, max_width: f32, origin_x: f32, origin_y: f32, scale: f32, out: *mut TextGlyph, capacity: usize,
) -> usize {
    let Some((style, _)) = crate::ui::label_style(style) else { return usize::MAX };
    unsafe { crate::trusted::label_quads(ui, value, &style, max_width, origin_x, origin_y, scale, out, capacity) }
});

export!(failing(usize::MAX) pub unsafe fn pxui_ui_label_rasterize_rgba(
    ui: *mut PxuiUi, value: Str, style: *const pxui_types::ffi::PxuiLabelStyle, max_width: f32, scale: f32,
    color_r: u8, color_g: u8, color_b: u8, color_a: u8, width: *mut u32, height: *mut u32, out: *mut u8, capacity: usize,
) -> usize {
    let Some((style, _)) = crate::ui::label_style(style) else { return usize::MAX };
    unsafe { crate::trusted::label_rasterize_rgba(ui, value, &style, max_width, scale, color_r, color_g, color_b, color_a, width, height, out, capacity) }
});

export!(pub unsafe fn pxui_ui_label_prewarm(ui: *mut PxuiUi, value: Str, style: *const pxui_types::ffi::PxuiLabelStyle, max_width: f32) -> i32 {
    let Some((style, _)) = crate::ui::label_style(style) else { return PXUI_ERR_ARGUMENT };
    unsafe { crate::trusted::label_prewarm(ui, value, &style, max_width) }
});
