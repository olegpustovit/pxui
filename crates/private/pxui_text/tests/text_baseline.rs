//! U2b.8: the first baseline a layout reports (`Metrics::baseline`, what `Paint::text_spans` puts below its `at`, the layout top) is
//! where CoreText puts it for the same face, size and line height (the ascent plus half the leading), snapped to whole points the CSS
//! way: never below CoreText's and less than a point above it. macOS only; 12 samples.
#![cfg(target_os = "macos")]
use core::ffi::c_void;
use pxui_text::{Style, Text};

#[link(name = "CoreFoundation", kind = "framework")]
#[link(name = "CoreGraphics", kind = "framework")]
#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CGDataProviderCreateWithData(info: *mut c_void, data: *const c_void, size: usize, release: *const c_void) -> *mut c_void;
    fn CGFontCreateWithDataProvider(provider: *mut c_void) -> *mut c_void;
    fn CTFontCreateWithGraphicsFont(font: *mut c_void, size: f64, matrix: *const c_void, attributes: *const c_void) -> *mut c_void;
    fn CTFontGetAscent(font: *const c_void) -> f64;
    fn CTFontGetDescent(font: *const c_void) -> f64;
    fn CFRelease(object: *const c_void);
}

#[test]
fn text_spans_baseline_matches_coretext() {
    let bytes = pxui_text::PRAGMASEVKA;
    let mut text = Text::new(&[bytes]);
    let (provider, graphics) = unsafe {
        let provider = CGDataProviderCreateWithData(core::ptr::null_mut(), bytes.as_ptr().cast(), bytes.len(), core::ptr::null());
        (provider, CGFontCreateWithDataProvider(provider))
    };
    assert!(!graphics.is_null(), "CoreText reads the face");
    let mut worst = 0.0f32;
    for size in [11.0f32, 12.0, 13.0, 14.0, 15.0, 17.0] {
        for factor in [1.2f32, 1.5] {
            let line_height = (size * factor).round();
            let (ascent, descent) = unsafe {
                let font = CTFontCreateWithGraphicsFont(graphics, f64::from(size), core::ptr::null(), core::ptr::null());
                let m = (CTFontGetAscent(font) as f32, CTFontGetDescent(font) as f32);
                CFRelease(font);
                m
            };
            let coretext = ascent + (line_height - ascent - descent) / 2.0;
            let ours = text.layout(1, "Hgx", &Style { size, line_height, ..Style::default() }, f32::INFINITY).baseline;
            let delta = (ours - coretext).abs();
            worst = worst.max(delta);
            // the CSS line box (Blink) rounds the ascent and descent and floors the half leading: the baseline is whole points, at or
            // above CoreText's exact one and less than a point from it (measured: 0.23 to 0.99 points over these 12 samples)
            assert!(ours <= coretext + 1e-3 && coretext - ours < 1.0 && ours.fract() == 0.0, "size {size}, line {line_height}: baseline {ours} against CoreText's {coretext}");
        }
    }
    unsafe { CFRelease(graphics); CFRelease(provider); }
    println!("worst baseline difference against CoreText: {worst} pt");
}
