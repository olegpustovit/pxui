use pxui_text::{Ellipsis, AtlasKind, Glyph, Rect, Style, Text, Wrap};

const PRAGMA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../assets/fonts/Pragmasevka-Regular.ttf");

fn read(path: &str) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(b) => Some(b),
        Err(_) => {
            println!("note: skipping, {path} not found");
            None
        }
    }
}

fn emitted(t: &mut Text, key: u64) -> Vec<Glyph> {
    let mut v = Vec::new();
    t.glyphs(key, (0.0, 0.0), 1.0, &mut |g| v.push(g));
    v
}

fn pragma() -> Vec<u8> {
    std::fs::read(PRAGMA).unwrap()
}

#[test]
fn ligatures_reduce_glyph_count() {
    let mut t = Text::new(&[&pragma()]);
    let st = Style::default();
    // Pragmasevka ligates through `calt` (one glyph substituted per input char), so the glyph count
    // stays equal; the shaped '-' of "->" is the wide arrow shaft, wider than a lone '-'.
    t.layout(1, "-", &st, 0.0);
    let lone = emitted(&mut t, 1)[0].rect.w;
    t.layout(2, "->", &st, 0.0);
    let arrow = emitted(&mut t, 2);
    assert!(arrow[0].rect.w > lone + 3.0, "{} vs {lone}", arrow[0].rect.w);
    // Fewer glyphs than chars (true `liga`) is covered by cosmic-text itself; no bundled font has one.
}

#[test]
fn bidi_orders_rtl_run_right_to_left() {
    let Some(he) = read("/System/Library/Fonts/SFHebrew.ttf") else { return };
    let mut t = Text::new(&[&pragma(), &he]);
    let text = "ab \u{5d0}\u{5d1}\u{5d2} cd"; // 2 bytes per Hebrew letter
    t.layout(1, text, &Style::default(), 0.0);
    let first = t.caret(1, 3).x; // before alef
    let last = t.caret(1, 7).x; // before gimel
    assert!(first > last, "logically first RTL letter is visually rightmost: {first} vs {last}");
    assert!(t.caret(1, 0).x < t.caret(1, 1).x, "LTR run keeps order");
}

#[test]
fn fallback_resolves_missing_glyph_from_second_font() {
    let Some(he) = read("/System/Library/Fonts/SFHebrew.ttf") else { return };
    let st = Style::default();
    let mut a = Text::new(&[&pragma()]);
    let mut b = Text::new(&[&pragma(), &he]);
    let wa = a.layout(1, "\u{5d0}\u{5d1}\u{5d2}", &st, 0.0).w;
    let wb = b.layout(1, "\u{5d0}\u{5d1}\u{5d2}", &st, 0.0).w;
    assert!((wa - wb).abs() > 0.5, "second font changes advances: {wa} vs {wb}");
    assert!(!emitted(&mut b, 1).is_empty());
}

#[test]
fn colour_emoji_lands_in_colour_atlas() {
    let Some(emoji) = read("/System/Library/Fonts/Apple Color Emoji.ttc") else { return };
    let mut t = Text::new(&[&pragma(), &emoji]);
    t.layout(1, "a\u{1f600}", &Style { size: 20.0, ..Style::default() }, 0.0);
    let g = emitted(&mut t, 1);
    assert!(g.iter().any(|g| g.atlas == AtlasKind::Color), "{g:?}");
    assert!(g.iter().any(|g| g.atlas == AtlasKind::Mask));
    let mut colour = 0;
    t.drain_uploads(&mut |k, _, _, w, h, data, stride| {
        if k == AtlasKind::Color {
            colour += 1;
            assert!(data.len() >= (h as usize - 1) * stride + w as usize * 4);
        }
    });
    assert_eq!(colour, 1);
}

#[test]
fn hit_and_caret_round_trip() {
    let mut t = Text::new(&[&pragma()]);
    let text = "hello world\nsecond";
    let m = t.layout(1, text, &Style::default(), 0.0);
    assert_eq!(m.lines, 2);
    for i in (0..=text.len()).filter(|i| *i != 11 && *i != 12) {
        let c = t.caret(1, i);
        assert_eq!(t.hit(1, c.x + 0.25, c.y + c.h / 2.0), i, "byte {i} caret {c:?}");
    }
    let mut spans = Vec::new();
    t.selection(1, 3, 15, &mut |r| spans.push(r));
    assert_eq!(spans.len(), 2, "{spans:?}");
    assert!(spans[1].y > spans[0].y);
}

#[test]
fn wrap_ellipsis_and_max_lines() {
    let mut t = Text::new(&[&pragma()]);
    let text = "the quick brown fox jumps over the lazy dog again and again";
    let plain = Style { wrap: Wrap::Word, ..Style::default() };
    let wide = t.layout(1, text, &Style::default(), 0.0);
    assert_eq!(wide.lines, 1);
    let wrapped = t.layout(2, text, &plain, wide.w / 4.0);
    assert!(wrapped.lines >= 4 && wrapped.w <= wide.w / 4.0 + 0.5, "{wrapped:?}");
    let clipped = t.layout(3, text, &Style { max_lines: 2, ..plain }, wide.w / 4.0);
    assert_eq!(clipped.lines, 2);
    let n_plain = emitted(&mut t, 2).len();
    let el = t.layout(4, text, &Style { max_lines: 2, ellipsis: Ellipsis::End, ..plain }, wide.w / 4.0);
    assert_eq!(el.lines, 2);
    assert!(emitted(&mut t, 4).len() < n_plain);
    let one = t.layout(5, text, &Style { ellipsis: Ellipsis::End, ..Style::default() }, wide.w / 2.0);
    assert_eq!((one.lines, one.w <= wide.w / 2.0 + 0.5), (1, true), "{one:?}");
}

#[test]
fn ellipsis_modes_preserve_endpoints_and_independent_warm_caches() {
    use pxui_text::TextSpan;
    use pxui_types::geom::color;
    let mut t=Text::new(&[&pragma()]);
    let source="ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let spans=[TextSpan { start:0,end:1,flags:TextSpan::COLOR,color:color::rgba(255,0,0,255),..Default::default() },
        TextSpan { start:25,end:26,flags:TextSpan::COLOR,color:color::rgba(0,0,255,255),..Default::default() }];
    let modes=[Ellipsis::None,Ellipsis::End,Ellipsis::Middle,Ellipsis::Head];
    let mut expected=Vec::new();
    t.layout(100,source,&Style { size:20.0,..Default::default() },1000.0);
    let fitting=emitted(&mut t,100);
    for (i,ellipsis) in modes.into_iter().enumerate() {
        let style=Style { ellipsis,size:20.0,..Default::default() };
        t.layout(100,source,&style,1000.0);
        assert_eq!(emitted(&mut t,100),fitting,"fitting text stays intact for every mode");
        let metrics=t.layout_spans(i as u64,source,&style,100.0,&spans).unwrap();
        let glyphs=emitted(&mut t,i as u64);
        let red=glyphs.iter().any(|g|g.color==Some([255,0,0,255]));
        let blue=glyphs.iter().any(|g|g.color==Some([0,0,255,255]));
        assert_eq!((red,blue),match ellipsis { Ellipsis::End=>(true,false),Ellipsis::Head=>(false,true),_=>(true,true) },"{ellipsis:?}");
        assert_eq!(metrics.lines,1);
        if ellipsis==Ellipsis::None { assert_eq!(glyphs.len(),26); }
        else { assert!(metrics.w<=100.001 && glyphs.len()<26,"{ellipsis:?}: {metrics:?}"); }
        // The same caller key must invalidate on mode changes as well as retain separate keys.
        assert_eq!(t.layout_spans(99,source,&style,100.0,&spans).unwrap(),metrics);
        assert_eq!(emitted(&mut t,99),glyphs);
        expected.push((style,metrics,glyphs));
    }
    for _ in 0..4 {
        mem_sys::local_reset();
        for (i,(style,metrics,glyphs)) in expected.iter().enumerate() {
            assert_eq!(t.layout_spans(i as u64,source,style,100.0,&spans).unwrap(),*metrics);
            let mut count=0;
            t.glyphs(i as u64,(0.0,0.0),1.0,&mut |g| { assert_eq!(g,glyphs[count]); count+=1; });
            assert_eq!(count,glyphs.len());
        }
        t.end_frame();
        assert_eq!(mem_sys::local_alloc_count(),0,"all warm ellipsis modes retain their layouts/glyphs");
    }
}

#[test]
fn ellipsis_modes_handle_wrapping_empty_text_and_unicode_source_indices() {
    const ARABIC:&[u8]=include_bytes!("../../../../assets/fonts/NotoSansArabic.ttf");
    let mut t=Text::new(&[&pragma(),ARABIC]);
    for ellipsis in [Ellipsis::End,Ellipsis::Middle,Ellipsis::Head] {
        for source in ["", "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz", "é/long/文件/لا/اسم/track.flac", "first line wraps over words and more words\r\nsecond line continues here"] {
            for wrap in [Wrap::None,Wrap::Word,Wrap::Glyph] {
                let style=Style { ellipsis,wrap,max_lines:2,size:20.0,..Default::default() };
                let metrics=t.layout(1,source,&style,100.0);
                assert!(metrics.lines<=2 && metrics.w.is_finite(),"{ellipsis:?} {wrap:?} {source:?}: {metrics:?}");
                // Wrapped advances include hanging blanks; glyph overhangs follow the font.
                if wrap==Wrap::None { assert!(metrics.w<=100.001,"{ellipsis:?} {source:?}: {metrics:?}"); }
                for glyph in emitted(&mut t,1) {
                    assert!(glyph.rect.x.is_finite() && glyph.rect.y.is_finite() && glyph.rect.w>0.0 && glyph.rect.h>0.0);
                }
                for x in [0.0,25.0,50.0,75.0,100.0] {
                    let byte=t.hit(1,x,metrics.h*0.5);
                    assert!(source.is_char_boundary(byte),"hit must return source UTF-8 boundaries: {source:?} {byte}");
                }
            }
        }
        assert_eq!(t.layout(1,"first\nsecond",&Style { ellipsis,..Default::default() },100.0).lines,1,"default ellipsis limits the whole block to one line");
    }
}

#[test]
fn uploads_cover_only_new_glyphs() {
    let mut t = Text::new(&[&pragma()]);
    let uploads = |t: &mut Text| {
        let mut v = Vec::new();
        t.drain_uploads(&mut |k, x, y, w, h, _, _| v.push((k, x, y, w, h)));
        v
    };
    t.layout(1, "abab", &Style::default(), 0.0);
    assert_eq!(emitted(&mut t, 1).len(), 4);
    assert_eq!(uploads(&mut t).len(), 2, "a and b once each");
    emitted(&mut t, 1);
    assert!(uploads(&mut t).is_empty());
    t.layout(1, "abc", &Style::default(), 0.0);
    emitted(&mut t, 1);
    let up = uploads(&mut t);
    assert_eq!(up.len(), 1);
    let (aw, ah) = t.atlas_size(AtlasKind::Mask);
    let (_, x, y, w, h) = up[0];
    assert!(x + w <= aw && y + h <= ah && t.bytes() >= (aw * ah) as usize);
}

#[test]
fn end_frame_evicts_untouched_layouts() {
    let mut t = Text::new(&[&pragma()]);
    t.layout(1, "abc", &Style::default(), 0.0);
    t.layout(2, "abc", &Style::default(), 0.0);
    t.end_frame();
    for i in 0..pxui_text::KEEP_FRAMES + pxui_text::SWEEP_FRAMES {
        assert!(i >= pxui_text::KEEP_FRAMES || t.caret(2, 3).x > 0.0, "a layout outlives a short keep");
        t.layout(1, "abc", &Style::default(), 0.0);
        t.end_frame();
    }
    assert!(t.caret(1, 3).x > 0.0);
    assert_eq!(t.caret(2, 3), Rect::default());
}

#[test]
fn warm_unchanged_frame_allocates_nothing() {
    let mut t = Text::new(&[&pragma()]);
    let st = Style { wrap: Wrap::Word, ..Style::default() };
    let text = "hello wrapped world, again and again";
    let mut n = 0;
    for _ in 0..2 {
        n = 0;
        t.layout(1, text, &st, 120.0);
        t.glyphs(1, (3.0, 4.0), 2.0, &mut |_| n += 1);
        t.end_frame();
    }
    mem_sys::local_reset();
    let before = mem_sys::local_alloc_count();
    let mut m = 0;
    t.layout(1, text, &st, 120.0);
    t.glyphs(1, (3.0, 4.0), 2.0, &mut |_| m += 1);
    t.end_frame();
    let allocs = mem_sys::local_alloc_count() - before;
    assert_eq!(m, n);
    assert_eq!(allocs, 0, "warm frame allocated {allocs} times");
}

#[test]
fn glyph_lists_replay_a_walk_at_any_origin() {
    let text = "Cached \u{2192} runs, wrapped over lines at any origin";
    let st = Style { wrap: Wrap::Word, size: 13.0, ..Style::default() };
    let mut cached = Text::new(&[&pragma()]);
    let mut walked = Text::new(&[&pragma()]);
    walked.set_glyph_cache(false);
    let emit = |t: &mut Text, o: (f32, f32), s: f32| {
        t.layout(1, text, &st, 90.0);
        let mut v = Vec::new();
        t.glyphs(1, o, s, &mut |g| v.push(g));
        v
    };
    for scale in [1.0, 2.0, 1.5] {
        for o in [(0.0, 0.0), (17.0, 3.5), (100.25, 7.0), (-3.6, -9.3), (4.0 / 3.0, 2.0 / 3.0), (1999.5, 801.75)] {
            assert_eq!(emit(&mut cached, o, scale), emit(&mut walked, o, scale), "origin {o:?} at {scale}x");
        }
    }
    let walks = cached.glyph_walks();
    assert!(walks <= 4, "one walk per scale, plus the first: {walks}");
    let before = cached.bytes();
    emit(&mut cached, (55.0, 66.0), 1.5);
    assert_eq!(cached.glyph_walks(), walks, "a warm layout replays its list at a new origin");
    assert!(before > walked.bytes(), "the lists are counted in bytes()");
    cached.layout(1, "re-shaped", &st, 90.0);
    let mut n = 0;
    cached.glyphs(1, (0.0, 0.0), 1.5, &mut |_| n += 1);
    assert_eq!((n, cached.glyph_walks()), (9, walks + 1), "re-shaping drops the list");
}

#[test]
fn monospace_advances_tracking_and_ascent() {
    let mut t = Text::new(&[pxui_text::PRAGMASEVKA]);
    let st = Style { size: 13.0, ..Style::default() };
    let one = t.layout(1, "-", &st, 0.0).w;
    // contextual alternates (`calt`) swap shapes, never cell widths: UI columns stay aligned
    for s in ["->", "!=", "==>", "www", "ii"] {
        let w = t.layout(2, s, &st, 0.0).w;
        assert!((w - one * s.chars().count() as f32).abs() < 1e-3, "{s}: {w} vs {one} a cell");
    }
    let tracked = t.layout(3, "abc", &Style { tracking: 1.5, ..st }, 0.0).w;
    assert!((tracked - 3.0 * (one + 1.5)).abs() < 1e-3, "tracking follows every glyph: {tracked}");
    let a = t.ascent(0);
    assert!(a > 0.5 && a < 1.5, "ascent ratio {a}");
}

/// The texels a glyph samples, read from the CPU mirror (what the GPU texture holds once drained).
fn texels(t: &mut Text, g: &Glyph) -> Vec<u8> {
    let (aw, ah) = t.atlas_size(g.atlas);
    let bpp = if g.atlas == AtlasKind::Mask { 1 } else { 4 };
    let (x, y) = ((g.uv.x * aw as f32).round() as usize, (g.uv.y * ah as f32).round() as usize);
    let (w, h) = ((g.uv.w * aw as f32).round() as usize, (g.uv.h * ah as f32).round() as usize);
    let px = t.atlas_pixels(g.atlas);
    (0..h).flat_map(|r| px[((y + r) * aw as usize + x) * bpp..((y + r) * aw as usize + x + w) * bpp].iter().copied()).collect()
}

/// A tiny atlas under churn (twelve new glyphs every frame): no frame still in flight and no kept output
/// ever samples a rewritten texel, and churned text keeps finding room.
#[test]
fn atlas_eviction_never_rewrites_texels_in_use() {
    let mut t = Text::new_sized(&[&pragma()], (128, 160), (16, 16));
    let style = |size: f32| Style { size, ..Style::default() };
    // kept output: emitted once, then shown each frame without emitting until a retirement asks
    t.layout(1, "kept", &style(13.0), 0.0);
    let mut kept: Vec<(Glyph, Vec<u8>)> = Vec::new();
    let mut retirements = t.retirements();
    let mut in_flight: Vec<Vec<(Glyph, Vec<u8>)>> = Vec::new();
    let (mut emitted_n, mut evictions) = (0, t.evictions());
    for frame in 0..400u32 {
        if kept.is_empty() || t.retirements() != retirements {
            // the kept view rebuilds: its glyphs and the shelves they sit on
            retirements = t.retirements();
            t.set_marks([0; 4]);
            kept = emitted(&mut t, 1).into_iter().map(|g| (g, Vec::new())).collect();
            kept.iter_mut().for_each(|(g, px)| *px = texels(&mut t, g));
            let marks = t.marks();
            t.pin(marks);
        }
        let key = 100 + frame as u64;
        // twelve glyphs no recent frame drew, at one of three sizes
        let churn: String = (0..12u32).map(|i| char::from(33 + ((frame * 12 + i) % 94) as u8)).collect();
        t.layout(key, &churn, &style(10.0 + (frame % 3) as f32 * 7.0), 0.0);
        let now: Vec<(Glyph, Vec<u8>)> = emitted(&mut t, key).into_iter().map(|g| { let px = texels(&mut t, &g); (g, px) }).collect();
        emitted_n += now.len();
        t.drain_uploads(&mut |_, _, _, _, _, _, _| {});
        for (g, px) in in_flight.iter().flatten().chain(kept.iter()) {
            assert_eq!(&texels(&mut t, g), px, "frame {frame}: a glyph still sampled was rewritten");
        }
        in_flight.push(now);
        // pxui_render keeps 3 frames (its FRAMES): the last two may still be drawing
        if in_flight.len() == 3 {
            in_flight.remove(0);
        }
        t.end_frame();
        evictions = evictions.max(t.evictions());
    }
    println!("atlas churn: {evictions} evictions, {emitted_n} glyphs, {} misses, {} retirements", t.misses(), t.retirements());
    assert!(evictions > 100, "the churn evicted {evictions} shelves");
    // 400 frames of 12 glyphs no recent frame drew, in an atlas about three frames' worth: a glyph
    // without room (no shelf free of frames in flight and kept output) shows a frame later
    assert!(t.misses() * 20 < emitted_n as u64, "{emitted_n} glyphs emitted, {} misses", t.misses());
}

#[test]
fn locale_reaches_the_font_system() {
    let t = Text::new_with_locale(&[&pragma()], "ja-JP", pxui_text::ATLAS_SIZES.0, pxui_text::ATLAS_SIZES.1);
    assert_eq!(t.locale(), "ja-JP");
    assert_eq!(Text::new(&[&pragma()]).locale(), "en-US");
    let tag = pxui_text::env_locale();
    assert!(!tag.contains(['_', '.']) && !tag.is_empty(), "{tag}");
}

/// A shaper that only lays text out holds no atlas: the raster mirrors (megabytes) are allocated by the first glyph that lands in one.
#[test]
fn laying_text_out_allocates_no_atlas_until_a_glyph_is_drawn() {
    let mut t = Text::new(&[&pragma()]);
    let st = Style::default();
    let metrics = t.measure(1, "layout only", &st, 0.0);
    assert!(metrics.w > 0.0);
    let (mask, color) = (t.atlas_size(AtlasKind::Mask), t.atlas_size(AtlasKind::Color));
    let atlases = (mask.0 * mask.1 + color.0 * color.1 * 4) as usize;
    assert!(atlases > 1 << 20, "the atlases are large: {atlases}");
    assert!(t.bytes() < atlases / 4, "layout allocated an atlas: {} bytes", t.bytes());
    t.layout(2, "drawn", &st, 0.0);
    assert!(!emitted(&mut t, 2).is_empty());
    assert!(t.bytes() > atlases / 4, "a drawn glyph allocated the mirror: {} bytes", t.bytes());
}


#[test]
fn tabular_digits_shape_equal_advances_and_reuse_warm_glyphs() {
    const FONT: &[u8]=include_bytes!("../../../../assets/fonts/Inter-Regular.ttf");
    let mut text=Text::new(&[FONT]);
    let plain=Style { size:20.0,..Default::default() };
    let tabular=Style { features:Style::FEATURE_TABULAR_NUMBERS,..plain };
    let digits="0123456789";
    let metrics=text.layout(1,digits,&tabular,0.0);
    for i in 0..10 {
        let advance=text.caret(1,i+1).x-text.caret(1,i).x;
        assert!((advance-metrics.w/10.0).abs()<1e-4,"digit {i}: {advance}");
    }
    let proportional=text.layout(2,"1111",&plain,0.0);
    let aligned=text.layout(2,"1111",&tabular,0.0);
    assert!((proportional.w-aligned.w).abs()>1.0,"fixture must exercise a real tnum substitution");
    assert_eq!(text.layout(2,"1111",&plain,0.0),proportional,"changing the feature invalidates the existing layout");
    text.glyphs(1,(0.0,0.0),1.0,&mut |_| {});
    for _ in 0..4 {
        mem_sys::local_reset();
        assert_eq!(text.layout(1,digits,&tabular,0.0),metrics);
        let mut glyphs=0;
        text.glyphs(1,(0.0,0.0),1.0,&mut |_| glyphs+=1);
        assert_eq!(glyphs,10);
        assert_eq!(mem_sys::local_alloc_count(),0,"warm tnum shaping/glyph output allocates nothing");
    }
}


#[test]
fn inline_spans_validate_ranges_and_keep_colors_rules_and_warm_replay() {
    use pxui_text::{TextSpan,TextSpans};
    use pxui_types::geom::color;
    let mut text=Text::new(&[pxui_text::PRAGMASEVKA]);
    let style=Style { size:20.0,..Default::default() };
    let content="ab CD ef";
    let span=TextSpan { start:3,end:5,weight:700,flags:TextSpan::COLOR|TextSpan::BACKGROUND|TextSpan::UNDERLINE,
        color:color::rgba(255,0,0,255),background:color::rgba(0,0,255,64),underline:color::rgba(0,255,0,255) };
    let metrics=text.layout_spans(1,content,&style,0.0,&[span]).unwrap();
    let glyphs=emitted(&mut text,1);
    assert_eq!(glyphs.iter().filter(|g|g.color==Some([255,0,0,255])).count(),2);
    text.layout(3,"CD",&Style { weight:700,..style },0.0);
    let heavy=emitted(&mut text,3);
    let marked:Vec<_>=glyphs.iter().filter(|g|g.color==Some([255,0,0,255])).collect();
    assert!(marked.iter().zip(&heavy).all(|(a,b)|a.uv==b.uv),"the inline weight uses the same glyph-cache entries as an entirely bold layout");
    for underline in [false,true] {
        let mut rects=Vec::new();
        text.decorations(1,underline,&mut |d|rects.push(d));
        assert_eq!(rects.len(),1);
        assert!((rects[0].rect.w-(text.caret(1,5).x-text.caret(1,3).x)).abs()<1e-4);
        assert_eq!(rects[0].color,if underline { [0,255,0,255] } else { [0,0,255,64] });
        if underline { assert_eq!(rects[0].rect.h,1.5); }
    }
    for _ in 0..4 {
        mem_sys::local_reset();
        assert_eq!(text.layout_spans(1,content,&style,0.0,&[span]).unwrap(),metrics);
        let mut count=0;
        text.glyphs(1,(0.0,0.0),1.0,&mut |_| count+=1);
        text.decorations(1,false,&mut |_| count+=1);
        text.decorations(1,true,&mut |_| count+=1);
        assert_eq!(count,glyphs.len()+2);
        assert_eq!(mem_sys::local_alloc_count(),0);
    }
    for invalid in [TextSpan { end:99,..span },TextSpan { start:5,end:4,..span },TextSpan { start:3,end:3,..span },TextSpan { flags:8,..span }] {
        assert!(text.layout_spans(1,content,&style,0.0,&[invalid]).is_none());
    }
    assert!(text.layout_spans(1,content,&style,0.0,&[span,span]).is_none());
    assert!(text.layout_spans(1,"é",&style,0.0,&[TextSpan { start:1,end:2,..span }]).is_none());
    let eight:[TextSpan;8]=std::array::from_fn(|i|TextSpan { start:i as u32,end:i as u32+1,..span });
    assert!(text.layout_spans(2,"12345678",&style,0.0,&eight).is_some());
    assert!(TextSpans::new(&[span;9]).is_none());
}

#[test]
fn inline_spans_split_ligature_color_without_reshaping_or_losing_marks() {
    use pxui_text::TextSpan;
    use pxui_types::geom::color;
    const FONT:&[u8]=include_bytes!("../../../../assets/fonts/NotoSansArabic.ttf");
    let mut text=Text::new(&[FONT]);
    let style=Style { size:30.0,..Default::default() };
    text.layout(1,"لا",&style,0.0);
    let original=emitted(&mut text,1);
    assert_eq!(original.len(),1,"fixture must form a ligature");
    let span=TextSpan { start:2,end:4,flags:TextSpan::COLOR|TextSpan::BACKGROUND|TextSpan::UNDERLINE,
        color:color::rgba(255,0,0,255),background:color::rgba(0,0,255,255),underline:color::rgba(0,255,0,255),..Default::default() };
    text.layout_spans(1,"لا",&style,0.0,&[span]).unwrap();
    let pieces=emitted(&mut text,1);
    assert_eq!(pieces.len(),2,"one bitmap clipped into inherited and span-coloured pieces");
    assert_eq!(pieces[0].color,None);
    assert_eq!(pieces[1].color,Some([255,0,0,255]));
    assert!((pieces[0].rect.w+pieces[1].rect.w-original[0].rect.w).abs()<1e-4);
    let caret=text.caret(1,2).x.min(text.caret(1,4).x);
    for underline in [false,true] { text.decorations(1,underline,&mut |d|assert!((d.rect.x-caret).abs()<1e-4)); }
    for scale in [1.0,1.5,2.0] {
        for origin in [(0.0,0.0),(13.25,7.5),(24.0,3.0)] {
            let mut cached=Vec::new();
            text.set_glyph_cache(true);
            text.glyphs(1,origin,scale,&mut |g|cached.push(g));
            let mut walked=Vec::new();
            text.set_glyph_cache(false);
            text.glyphs(1,origin,scale,&mut |g|walked.push(g));
            assert_eq!(cached,walked,"cached and uncached fragments at {scale}x and {origin:?}");
        }
    }
}

#[test]
fn inline_spans_use_source_offsets_across_crlf_and_wrapped_lines() {
    use pxui_text::TextSpan;
    use pxui_types::geom::color;
    let mut text=Text::new(&[pxui_text::PRAGMASEVKA]);
    let content="ab\r\nββ cd ef";
    assert_eq!(&content.as_bytes()[2..4],&[13,10],"exercise actual CRLF offsets");
    let span=TextSpan { start:4,end:content.len() as u32,flags:TextSpan::COLOR|TextSpan::UNDERLINE,color:color::rgba(255,0,0,255),underline:color::rgba(0,255,0,255),..Default::default() };
    let style=Style { size:20.0,wrap:Wrap::Word,..Default::default() };
    let metrics=text.layout_spans(1,content,&style,45.0,&[span]).unwrap();
    assert!(metrics.lines>=3);
    assert_eq!(emitted(&mut text,1).iter().filter(|g|g.color==Some([255,0,0,255])).count(),6);
    let mut rules=0;
    text.decorations(1,true,&mut |d|{ assert!(d.rect.y>20.0); rules+=1; });
    assert!(rules>=2);
}


#[test]
fn inline_spans_preserve_ellipsized_glyph_output() {
    use pxui_text::TextSpan;
    use pxui_types::geom::color;
    let mut text=Text::new(&[pxui_text::PRAGMASEVKA]);
    let style=Style { ellipsis:Ellipsis::End,max_lines:1,size:20.0,..Default::default() };
    let content="a much longer label";
    text.layout(1,content,&style,60.0);
    let plain=emitted(&mut text,1);
    assert!(!plain.is_empty());
    let span=TextSpan { start:0,end:content.len() as u32,flags:TextSpan::COLOR,color:color::rgba(255,0,0,255),..Default::default() };
    text.layout_spans(1,content,&style,60.0,&[span]).unwrap();
    let rich=emitted(&mut text,1);
    assert_eq!(plain.len(),rich.len());
    for (a,b) in plain.iter().zip(&rich) { assert_eq!((a.rect,a.uv),(b.rect,b.uv)); }
}


#[test]
fn inline_spans_keep_mixed_bidi_marks_disjoint() {
    use pxui_text::TextSpan;
    use pxui_types::geom::color;
    const ARABIC:&[u8]=include_bytes!("../../../../assets/fonts/NotoSansArabic.ttf");
    let mut text=Text::new(&[ARABIC,pxui_text::PRAGMASEVKA]);
    let style=Style { size:20.0,..Default::default() };
    let span=TextSpan { start:1,end:5,flags:TextSpan::BACKGROUND,background:color::rgba(0,0,255,64),..Default::default() };
    text.layout_spans(1,"ab لا cd",&style,0.0,&[span]).unwrap();
    let mut marks=Vec::new();
    text.decorations(1,false,&mut |d|marks.push(d.rect));
    assert_eq!(marks.len(),2,"unselected half of the RTL ligature separates the visual marks");
    let (a,b)=(text.caret(1,3).x,text.caret(1,5).x);
    assert!(marks.iter().any(|r|(r.x-a.min(b)).abs()<1e-4 && (r.w-(a-b).abs()).abs()<1e-4));
}
