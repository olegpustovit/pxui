//! U2b.8: the first baseline a layout reports (`Metrics::baseline`, what `Paint::text_spans` puts below its `at`, the layout top) is
//! where CoreText puts it for the same face, size and line height (the ascent plus half the leading), snapped to whole points the CSS
//! way: never below CoreText's and less than a point above it. macOS only; 12 samples.
#![cfg(target_os = "macos")]
use pxui_text::{Style, Text};

#[test]
fn text_spans_baseline_matches_coretext() {
    let bytes = pxui_text::PRAGMASEVKA;
    let mut text = Text::new(&[bytes]);
    let mut worst = 0.0f32;
    for size in [11.0f32, 12.0, 13.0, 14.0, 15.0, 17.0] {
        for factor in [1.2f32, 1.5] {
            let line_height = (size * factor).round();
            let (ascent, descent) = font_sys::ascent_descent(bytes, f64::from(size)).expect("CoreText reads the face");
            let (ascent, descent) = (ascent as f32, descent as f32);
            let coretext = ascent + (line_height - ascent - descent) / 2.0;
            let ours = text.layout(1, "Hgx", &Style { size, line_height, ..Style::default() }, f32::INFINITY).baseline;
            let delta = (ours - coretext).abs();
            worst = worst.max(delta);
            // the CSS line box (Blink) rounds the ascent and descent and floors the half leading: the baseline is whole points, at or
            // above CoreText's exact one and less than a point from it (measured: 0.23 to 0.99 points over these 12 samples)
            assert!(ours <= coretext + 1e-3 && coretext - ours < 1.0 && ours.fract() == 0.0, "size {size}, line {line_height}: baseline {ours} against CoreText's {coretext}");
        }
    }
    println!("worst baseline difference against CoreText: {worst} pt");
}
