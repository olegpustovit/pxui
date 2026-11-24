//! Text features and ellipsis modes participate in both caller-keyed style comparisons and content
//! layout keys. Feature setup allocates only for new layouts. Preserve original accessibility
//! labels, shaping, bitmap coverage, subpixel bins and cache replay when applying inline colours.
//! Decorations stream from caret geometry without cosmic-text LayoutRun::highlight allocation;
//! rules sit at line-box bottom with independent colour. Glyphs/decorations share opacity, clip and
//! one instanced draw per layer. Preserve ligature, BiDi, CRLF, wrapping and ellipsis behavior
//! without treating hanging blanks/font overhangs as clipping requirements.
//!
//! Center rich nodes using fractional font points and the whole wrapped block's line count/height.
//! Measure baselines at current layout width. System fallback queries only missing clusters, checks
//! outline/bitmap raster compatibility as well as coverage, reuses fontdb mappings and never copies
//! large system files or scans directories. Keep explicit-font constructors deterministic, mapped
//! paths bounded to 128 and negatives to 512x256 bytes; clearing negatives permits cold retries.
//!
//! Glyph atlases have two fixed pages per mask/colour kind. Second mirrors/textures allocate on
//! first use only; retain old handles/UVs and in-flight/retained shelf protection. Newly active
//! full mirrors upload on the caller's command buffer before partial uploads/draws; failed upload
//! resets initialization. Second pages use flags/codes without adding draw breaks.
//!
//! Text for PXUI: cosmic-text shaping/bidi/wrapping/fallback behind a small concrete API, a layout
//! cache keyed by a caller-supplied node key, and two pages each of R8 mask and RGBA8 colour atlases that
//! are exposed as dirty rectangles. The only workspace crate with an external dependency.
//!
//! ```text
//! let mut t = Text::new(&[pragmasevka_bytes, hebrew_bytes]);   // explicit fonts, no system scan
//! let st = Style { size: 14.0, ..Style::default() };
//! let m = t.layout(node, "hello", &st, 200.0);                 // -> Metrics { w, h, lines, baseline }
//! t.glyphs(node, (x, y), scale, &mut |g| quads.push(g));       // device-pixel quads + normalised uv
//! t.drain_uploads(&mut |kind, x, y, w, h, data, stride| ..);   // upload only new glyph rects
//! t.end_frame();                                               // evict layouts unused for KEEP_FRAMES frames (swept every SWEEP_FRAMES)
//! ```
//!
//! API (all geometry f32; inputs in logical points, glyph quads in device pixels = points * scale):
//! - `Text::new(&[&[u8]])`, `add_font(&[u8]) -> usize` (font index = load order; glyphs missing
//!   from `Style::font` fall back across every loaded font).
//! - `layout(key, text, &Style, max_width) -> Metrics` (`max_width` not finite or <= 0: no wrap
//!   width). Re-shapes only when text, width or style changed; `measure` is the same call.
//! - `hit(key, x, y) -> usize`, `caret(key, byte) -> Rect` (zero width),
//!   `selection(key, a, b, &mut dyn FnMut(Rect))`. Byte indices are into the whole text.
//! - `glyphs(key, origin, scale, &mut dyn FnMut(Glyph))`: `Glyph.rect` is device pixels, `Glyph.uv`
//!   is normalised against `atlas_size(kind)` (fixed). Mask glyphs are tinted by the caller;
//!   `AtlasKind::Color` glyphs are RGBA as rasterised. Missing glyphs are rasterised on demand.
//! - `drain_uploads(&mut dyn FnMut(AtlasKind, x, y, w, h, &[u8], stride))`: texel rectangles
//!   written since the last drain; rows are `stride` bytes apart, `w * bytes_per_pixel` used.
//! - `atlas_size(kind) -> (w, h)`, `bytes() -> usize` (atlas mirrors), `end_frame()`,
//!   `ascent(font) -> f32` (fraction of the size). `PRAGMASEVKA` is the bundled UI face.
//!
//! `max_lines` (0 = unlimited) clips; `ellipsis` selects end, middle or head truncation of the
//! last visible line (one line when `max_lines` is 0). Unchanged layouts and glyph iteration of already
//! rasterised glyphs perform zero heap allocations (tests/text.rs). Changed text allocates inside
//! cosmic-text.
//!
//! Build cost (cosmic-text 0.19.0, M1, `nice -n 10` while the machine was in use): first cold debug
//! build of the dependency closure 24.5 s wall (116 s CPU); 40 external crates + pxui_text.

use pxui_types::draw::{self, Break, Instance};
use cosmic_text::{
    Align as CAlign, Attrs, AttrsList, Buffer, CacheKey, CacheKeyFlags, Cursor, Ellipsize, EllipsizeHeightLimit, Family, FeatureTag, FontFeatures, FontSystem, Metrics as CMetrics, Shaping,
    Style as CStyle, SubpixelBin, SwashCache, SwashContent, Weight, Wrap as CWrap, fontdb,
};
pub use pxui_types::{TextSpan, TextSpans};
use std::collections::HashMap;
use pxui_types::geom::hash;
use std::hash::BuildHasherDefault;
use std::sync::Arc;
mod system;
mod prepared;
mod queue;
pub use prepared::Prepared;
pub use queue::{Worker, INSTALLS_PER_FRAME, MAX_LANES, QUEUE_BYTES, QUEUE_ROWS};

pub use pxui_types::geom::hash::FastHasher;
/// `HashMap`'s hasher parameter for `FastHasher`.
pub type Fast = BuildHasherDefault<FastHasher>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wrap {
    None,
    Word,
    Glyph,
    WordSpaces,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ellipsis {
    None,
    End,
    Middle,
    Head,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AtlasKind {
    Mask,
    Color,
    Mask2,
    Color2,
}
impl AtlasKind {
    pub const ALL:[Self;4]=[Self::Mask,Self::Color,Self::Mask2,Self::Color2];
    pub fn is_mask(self)->bool { matches!(self,Self::Mask|Self::Mask2) }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// Index of the preferred font (`add_font` order).
    pub font: u16,
    pub size: f32,
    pub weight: u16,
    pub italic: bool,
    /// Points; 0 = 1.2 * size.
    pub line_height: f32,
    pub wrap: Wrap,
    pub align: Align,
    /// 0 = unlimited.
    pub max_lines: u16,
    pub ellipsis: Ellipsis,
    /// Points added after every glyph (CSS `letter-spacing`).
    pub tracking: f32,
    /// OpenType feature bits; 0 preserves the font defaults.
    pub features: u32,
}
impl Style { pub const FEATURE_TABULAR_NUMBERS: u32 = 1; }
impl Default for Style {
    fn default() -> Style {
        Style { font: 0, size: 14.0, weight: 400, italic: false, line_height: 0.0, wrap: Wrap::None, align: Align::Left, max_lines: 0, ellipsis: Ellipsis::None, tracking: 0.0, features: 0 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Metrics {
    pub w: f32,
    pub h: f32,
    pub lines: u32,
    /// Distance from the top to the first line's baseline.
    pub baseline: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub rect: Rect,
    pub uv: Rect,
    /// Atlas coordinates packed exactly as a draw instance samples them.
    pub atlas_uv: [u32; 2],
    pub atlas: AtlasKind,
    /// An inline foreground override; None inherits the caller's colour.
    pub color: Option<[u8;4]>,
}

/// The shared draw attributes of one text layout's glyphs.
#[derive(Clone, Copy)]
pub struct GlyphPaint {
    pub clip: [i16; 4],
    pub view: u16,
    pub color: [u8; 4],
    pub opacity: f32,
    pub mask_flags: u8,
    pub light: [u32; 2],
}

impl GlyphPaint {
    #[inline(always)]
    fn push(&self, glyph: Glyph, scale: f32, inverse: Option<f32>, out: &mut Vec<Instance>, breaks: &mut Vec<Break>) {
        let color = glyph.color.map_or(self.color, |mut color| {
            if self.opacity < 1.0 { color[3] = (color[3] as f32 * self.opacity).round() as u8; }
            color
        });
        let r = glyph.rect;
        let pixels = [r.x, r.y, r.x + r.w, r.y + r.h];
        let rect = if let Some(inv) = inverse { pixels.map(|p| p * inv) } else { pixels.map(|p| p / scale) };
        let (kind, color, flags, extra) = if glyph.atlas.is_mask() {
            (draw::kind::MASK, color, self.mask_flags, self.light)
        } else {
            if breaks.last().is_some_and(|b| b.image != draw::ATLAS) {
                breaks.push(Break { at_instance: out.len() as u32, image: draw::ATLAS });
            }
            (draw::kind::IMAGE, [255, 255, 255, color[3]], 0, [0; 2])
        };
        let page = if matches!(glyph.atlas, AtlasKind::Mask2 | AtlasKind::Color2) { draw::flags::mask::GLYPH_PAGE_2 } else { 0 };
        out.push(Instance { rect, color, aux: glyph.atlas_uv, clip: self.clip, view: self.view,
            kind, flags: flags | page, extra, ..Instance::DEFAULT });
    }
}

#[inline(always)]
fn emit_glyph(
    glyph: Glyph, scale: f32,
    output: &mut Option<(GlyphPaint, Option<f32>, &mut Vec<Instance>, &mut Vec<Break>)>,
    callback: &mut dyn FnMut(Glyph),
) {
    if let Some((paint, inverse, out, breaks)) = output {
        paint.push(glyph, scale, *inverse, out, breaks);
    } else {
        callback(glyph);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decoration { pub rect: Rect, pub color:[u8;4] }

/// Inline spans are byte ranges in source order, with one style per byte.
pub fn valid_spans(text: &str, spans: &[TextSpan]) -> bool {
    if spans.len()>TextSpans::MAX { return false; }
    let mut end=0;
    for span in spans {
        let (start,next)=(span.start as usize,span.end as usize);
        if span.flags & !(TextSpan::COLOR|TextSpan::BACKGROUND|TextSpan::UNDERLINE)!=0 || start<end || start>=next
            || !text.is_char_boundary(start) || !text.is_char_boundary(next) { return false; }
        end=next;
    }
    true
}

fn rgba(color:pxui_types::geom::Rgba)->[u8;4] { [color.r,color.g,color.b,color.a] }

fn glyph_range(run:&cosmic_text::LayoutRun,glyph:&cosmic_text::LayoutGlyph,start:usize,end:usize)->[f32;2] {
    let position=|at| {
        if at==glyph.start { if glyph.level.is_rtl() { glyph.x+glyph.w } else { glyph.x } }
        else if at==glyph.end { if glyph.level.is_rtl() { glyph.x } else { glyph.x+glyph.w } }
        else { run.cursor_position(&Cursor::new(run.line_i,at)).unwrap_or(glyph.x) }
    };
    let (a,b)=(position(start),position(end));
    [a.min(b),a.max(b)]
}

// Stream selected visual ranges using the same caret geometry as inline decorations.
fn run_ranges(run:&cosmic_text::LayoutRun,start:usize,end:usize,f:&mut dyn FnMut([f32;2])) {
    let mut active:Option<[f32;2]>=None;
    for glyph in run.glyphs {
        let (lo,hi)=(start.max(glyph.start),end.min(glyph.end));
        let range=(lo<hi).then(||glyph_range(run,glyph,lo,hi)).filter(|r|r[1]>r[0]);
        match (active,range) {
            (Some(old),Some(next)) if next[0]<=old[1]+1e-4 && next[1]>=old[0]-1e-4 => active=Some([old[0].min(next[0]),old[1].max(next[1])]),
            (Some(old),next)=>{ f(old); active=next; }
            (None,next)=>active=next,
        }
    }
    if let Some(range)=active { f(range); }
}

// Caret cuts preserve the original shaping when a colour boundary crosses a ligature.
fn glyph_parts(run:&cosmic_text::LayoutRun,glyph:&cosmic_text::LayoutGlyph,base:usize,spans:&[TextSpan],f:&mut dyn FnMut([f32;2],Option<[u8;4]>)) {
    let (start,end)=(base+glyph.start,base+glyph.end);
    if !spans.iter().any(|s|s.flags & TextSpan::COLOR!=0 && (s.start as usize)<end && (s.end as usize)>start) {
        f([f32::NEG_INFINITY,f32::INFINITY],None);
        return;
    }
    if start==end {
        let at=start.saturating_sub(1);
        let color=spans.iter().find(|s|s.flags & TextSpan::COLOR!=0 && s.start as usize<=at && s.end as usize>at).map(|s|rgba(s.color));
        f([f32::NEG_INFINITY,f32::INFINITY],color);
        return;
    }
    let mut cursor=start;
    let mut part=|lo:usize,hi:usize,color| {
        if lo>=hi { return; }
        let mut clip=glyph_range(run,glyph,lo-base,hi-base);
        if (!glyph.level.is_rtl() && lo==start) || (glyph.level.is_rtl() && hi==end) { clip[0]=f32::NEG_INFINITY; }
        if (!glyph.level.is_rtl() && hi==end) || (glyph.level.is_rtl() && lo==start) { clip[1]=f32::INFINITY; }
        f(clip,color);
    };
    for span in spans.iter().filter(|s|s.flags & TextSpan::COLOR!=0) {
        let (lo,hi)=((span.start as usize).max(start),(span.end as usize).min(end));
        if lo>=hi { continue; }
        part(cursor,lo,None);
        part(lo,hi,Some(rgba(span.color)));
        cursor=hi;
    }
    part(cursor,end,None);
}

const UNCLIPPED:[f32;2]=[f32::NEG_INFINITY,f32::INFINITY];

fn emit_clipped(mut glyph:Glyph,clip:[f32;2],f:&mut dyn FnMut(Glyph)) {
    if clip==UNCLIPPED { f(glyph); return; }
    let (left,right)=(glyph.rect.x.max(clip[0]),(glyph.rect.x+glyph.rect.w).min(clip[1]));
    if left>=right { return; }
    let advance=glyph.uv.w/glyph.rect.w;
    glyph.uv.x+=(left-glyph.rect.x)*advance;
    glyph.uv.w=(right-left)*advance;
    glyph.rect.x=left;
    glyph.rect.w=right-left;
    glyph.atlas_uv = packed_uv(glyph.uv);
    f(glyph);
}

fn packed_uv(uv: Rect) -> [u32; 2] {
    pxui_types::draw::Instance::mask([0.0; 4], [uv.x, uv.y, uv.x + uv.w, uv.y + uv.h], [0; 4]).aux
}

/// Content identity shared by UI drawing and keyless standalone/UI measurement.
pub fn text_hash(text: &str) -> u64 {
    hash::bytes(0, text.as_bytes())
}

pub fn content_key(style: &Style, width: f32, text: &str, spans: &[TextSpan]) -> u64 {
    layout_key(style, width, text_hash(text), spans)
}

/// A retained node can reuse its text fingerprint across widths/styles without hashing bytes.
pub fn layout_key(style: &Style, width: f32, text_hash: u64, spans: &[TextSpan]) -> u64 {
    let mut seed = hash::mix(style.font as i64, style.size.to_bits() as i64);
    for v in [style.weight as i64, style.italic as i64, style.line_height.to_bits() as i64, style.wrap as i64, style.align as i64, style.max_lines as i64, style.ellipsis as i64, style.tracking.to_bits() as i64, style.features as i64, width.to_bits() as i64] {
        seed = hash::mix(seed, v);
    }
    let mut key = hash::mix(seed, text_hash as i64) as u64;
    for span in spans {
        for value in [span.start as i64, span.end as i64, span.weight as i64, span.flags as i64,
            u32::from_le_bytes(rgba(span.color)) as i64, u32::from_le_bytes(rgba(span.background)) as i64,
            u32::from_le_bytes(rgba(span.underline)) as i64] {
            key = hash::mix(key as i64, value) as u64;
        }
    }
    key
}



/// Age before incremental expiry and the fraction of remaining layouts inspected per frame.
/// Removing entries shrinks each following sweep budget; MAX_LAYOUTS also bounds storage.
pub const KEEP_FRAMES: u32 = 600;
pub const SWEEP_FRAMES: u32 = 64;
/// Largest font `Text::add_font` accepts, in bytes.
pub const MAX_FONT_BYTES: usize = 32 << 20;
/// Most fonts one text service holds (`Style::font` is a `u16`; fonts are never removed).
pub const MAX_FONTS: usize = 64;
/// Total bytes of the fonts one text service holds.
pub const MAX_FONT_TOTAL_BYTES: usize = 256 << 20;
/// Byte budget over the estimated size of retained layouts (`layout_cost`: shaped lines and glyph runs plus the text, about
/// `SHAPED_BYTES_PER_TEXT_BYTE` per text byte and `LAYOUT_BASE_BYTES` per entry; cosmic-text does not report its buffer sizes). Past it the
/// least valuable layouts go (`victim` order), but never one laid out in the current frame: a frame showing many large texts keeps them all and
/// the excess goes the next frame. A layout above the whole budget stays only while it is used. A text service that never ends a frame
/// has no "current frame to protect": past twice the budget it evicts regardless, sparing only the layout just made.
pub const LAYOUT_BYTES: usize = 64 << 20;
pub const SHAPED_BYTES_PER_TEXT_BYTE: usize = 160;
pub const LAYOUT_BASE_BYTES: usize = 1024;
fn layout_cost(text_len: usize) -> usize { text_len.saturating_mul(SHAPED_BYTES_PER_TEXT_BYTE).saturating_add(LAYOUT_BASE_BYTES) }
/// Bound retained layouts even for standalone shapers without a frame clock.
const MAX_LAYOUTS: usize = 4096;
/// Evicted layouts kept for the next fresh one (their buffer, text and glyph list storage), so text that
/// changes every frame shapes into reused storage. The sweep evicts in bursts about `SWEEP_FRAMES` frames apart, so the pool holds two bursts' worth. Bounded: at most this many, each with text up to
/// `SPARE_TEXT_BYTES`; a larger layout is dropped.
const SPARE_LAYOUTS: usize = 2 * SWEEP_FRAMES as usize;
const SPARE_TEXT_BYTES: usize = 1024;
/// Slots of the recent-width index (`Text::recent_widths`): direct-mapped by the plain text's natural key, each holding that text's last two
/// laid-out widths. A fixed table of `RECENT_SLOTS * 24` bytes made at construction: never grows, a colliding text overwrites, and a stale
/// width only names a layout `reusable_layout` then fails to find or validate.
const RECENT_SLOTS: usize = 512;
#[derive(Clone, Copy)]
struct Recent { key: u64, widths: [f32; 2] }
/// Text storage a new layout starts with, so ordinary labels never regrow a pooled layout.
const LABEL_BYTES: usize = 64;
/// Glyph slots a new layout starts with, for the same reason.
const LABEL_GLYPHS: usize = 32;
/// Rows past a virtual list's built window whose layouts are never age-evicted; farther ones go first.
pub const LIST_KEEP_ROWS: u32 = 64;
/// Lists whose windows are tracked; the least recently built one is replaced by another.
const LIST_WINDOWS: usize = 8;
// A jump should leave one return destination warm without retaining an entire continuous scroll.
const JUMP_KEEP_FRAMES: u32 = 4 * SWEEP_FRAMES;

#[derive(Clone, Copy, Default)]
struct ListWindow {
    list: u64,
    first: u32,
    last: u32,
    departed: (u32, u32),
}

impl ListWindow {
    fn distance(&self, row: u32) -> u32 {
        if row < self.first { self.first - row }
        else if row >= self.last { (row - self.last).saturating_add(1) }
        else { 0 }
    }
}

/// The bundled UI face (the PXUI default theme's font).
pub const PRAGMASEVKA: &[u8] = include_bytes!("../../../../assets/fonts/Pragmasevka-Regular.ttf");

const MASK_SIZE: (u32, u32) = (1024, 2048);
const COLOR_SIZE: (u32, u32) = (1024, 1024);
/// `Text::new`'s mask and colour atlas sizes.
pub const ATLAS_SIZES: ((u32, u32), (u32, u32)) = (MASK_SIZE, COLOR_SIZE);

/// The user's locale from the POSIX environment (`LC_ALL`, then `LC_MESSAGES`, then `LANG`;
/// `de_DE.UTF-8` gives `de-DE`), `en-US` when none names one.
pub fn env_locale() -> String {
    let name = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().filter_map(|k| std::env::var(k).ok()).find(|v| !v.is_empty() && v != "C" && v != "POSIX");
    let name = name.as_deref().unwrap_or("en_US");
    let tag = name.split(['.', '@']).next().unwrap_or("en_US").replace('_', "-");
    if tag.is_empty() { "en-US".into() } else { tag }
}

/// Frames whose GPU work may still sample the atlas after the CPU writes it: the painter keeps
/// `pxui_render`'s `FRAMES` (3) ring regions, so texels last drawn at frame `f` are free at `f + 3`.
pub const FRAMES_IN_FLIGHT: u32 = 3;

/// A set of atlas shelves, one bit per shelf of all four pages (`shelf_bit`; shelves past 128 per page share
/// bits, which only keeps more shelves).
pub type Shelves = [u64; 8];

// The color pages are indices 1 and 3 in each group of four shelf bits.
const COLOR_SHELVES: u64 = 0xaaaa_aaaa_aaaa_aaaa;

fn shelf_bit(kind: AtlasKind, shelf: u16) -> (usize, u64) {
    let bit = (shelf as usize * 4 + idx(kind)) % 512;
    (bit / 64, 1 << (bit % 64))
}

struct Shelf {
    y: u32,
    h: u32,
    x: u32,
    used: u32,
    /// the frame it was retired at (its glyphs dropped, its texels left for the frames still
    /// showing them); 0: open
    retired: u32,
    /// bumped whenever its glyphs are dropped: a slot of another generation is gone
    generation: u32,
}
struct Atlas {
    w: u32,
    h: u32,
    bpp: u32,
    px: Vec<u8>,
    shelves: Vec<Shelf>,
    next_y: u32,
}
#[derive(Clone, Copy)]
struct Slot {
    kind: AtlasKind,
    shelf: u16,
    generation: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    left: i32,
    top: i32,
}

impl Atlas {
    fn new((w, h): (u32, u32), bpp: u32) -> Atlas {
        // the texels are allocated by `pixels`, when the first glyph lands or the mirror is read: a shaper that only lays text out holds
        // none. A shelf is at least 2 texels tall: room for every shelf there can be, so no frame allocates
        Atlas { w, h, bpp, px: Vec::new(), shelves: Vec::with_capacity(h as usize / 2 + 1), next_y: 0 }
    }

    /// The texels, `w * h * bpp` of them, allocated (zeroed) on first use.
    fn pixels(&mut self) -> &mut [u8] {
        if self.px.is_empty() { self.px = vec![0; (self.w * self.h * self.bpp) as usize]; }
        &mut self.px
    }

    /// Room for a `w`x`h` glyph (one pixel of padding on the right and bottom), shelf-LRU. A shelf
    /// is rewritten only when nothing can still sample it: not drawn in the last `FRAMES_IN_FLIGHT`
    /// frames and not in `pinned` (kept output shows glyphs on it), or retired at least
    /// `FRAMES_IN_FLIGHT` frames ago. With no such shelf the least recently used one no frame
    /// in flight drew is retired (`Alloc::Retired`): its glyphs go, its texels stay until the frames
    /// showing them are done, and the caller rebuilds what showed them.
    fn alloc(&mut self, kind: AtlasKind, w: u32, h: u32, frame: u32, pinned: &Shelves, reuse:Reuse) -> Alloc {
        let (pw, ph) = (w + 1, h + 1);
        if pw > self.w || ph > self.h {
            return Alloc::Full;
        }
        let open = |s: &Shelf| s.retired == 0;
        // a shelf holds glyphs of about its height (a quarter taller at most), so one size's shelves
        // fall idle together and can be reused when that size is gone
        let fits = |s: &Shelf| s.h >= ph && s.h <= ph + ph / 4 + 1;
        let best = self.shelves.iter().enumerate().filter(|(_, s)| open(s) && fits(s) && s.x + pw <= self.w).min_by_key(|(_, s)| s.h).map(|(i, _)| i);
        let (i, evicted) = if let Some(i) = best {
            (i, false)
        } else if self.next_y + ph <= self.h {
            self.shelves.push(Shelf { y: self.next_y, h: ph, x: 0, used: frame, retired: 0, generation: 0 });
            self.next_y += ph;
            (self.shelves.len() - 1, false)
        } else {
            if reuse==Reuse::Fresh { return Alloc::Full; }
            let unpinned = |i: usize| {
                let (word, bit) = shelf_bit(kind, i as u16);
                pinned[word] & bit == 0
            };
            let free = |i: usize, s: &Shelf| s.h > 0 && if s.retired != 0 { s.retired + FRAMES_IN_FLIGHT <= frame } else { s.used + FRAMES_IN_FLIGHT <= frame && unpinned(i) };
            let lru = |shelves: &[Shelf], want: &dyn Fn(usize, &Shelf) -> bool| shelves.iter().enumerate().filter(|(i, s)| s.h >= ph && want(*i, s)).min_by_key(|(_, s)| (s.h, s.used)).map(|(i, _)| i);
            match lru(&self.shelves, &free).or_else(|| self.merge(ph, &free)) {
                Some(i) => {
                    self.split(i, ph);
                    let s = &mut self.shelves[i];
                    (s.x, s.retired, s.generation) = (0, 0, s.generation.wrapping_add(1));
                    (i, true)
                }
                None => {
                    if reuse==Reuse::Free { return Alloc::Full; }
                    let Some(i) = lru(&self.shelves, &|_, s: &Shelf| s.h > 0 && open(s) && s.used + FRAMES_IN_FLIGHT <= frame) else { return Alloc::Full };
                    let s = &mut self.shelves[i];
                    (s.retired, s.generation) = (frame, s.generation.wrapping_add(1));
                    return Alloc::Retired;
                }
            }
        };
        let s = &mut self.shelves[i];
        s.used = frame;
        let x = s.x;
        s.x += pw;
        Alloc::At(x, s.y, i as u16, evicted)
    }

    /// The shelf below `i` (y-adjacent, live), if any.
    fn below(&self, i: usize) -> Option<usize> {
        let end = self.shelves[i].y + self.shelves[i].h;
        self.shelves.iter().position(|s| s.h > 0 && s.y == end)
    }

    /// One shelf at least `ph` tall from a run of y-adjacent `free` shelves (the first is kept, the
    /// rest become dead entries, height 0, for `split` to reuse); `None` when no run is tall enough.
    /// O(shelves^2), only when the atlas has no room.
    fn merge(&mut self, ph: u32, free: &dyn Fn(usize, &Shelf) -> bool) -> Option<usize> {
        let start = (0..self.shelves.len()).find(|&i| {
            let (mut at, mut total) = (i, 0);
            while free(at, &self.shelves[at]) {
                total += self.shelves[at].h;
                if total >= ph {
                    return true;
                }
                let Some(next) = self.below(at) else { return false };
                at = next;
            }
            false
        })?;
        // `below(start)` is the next shelf of the run as `start` grows over it
        while self.shelves[start].h < ph {
            let Some(next) = self.below(start) else { break };
            self.shelves[start].h += self.shelves[next].h;
            let n = &mut self.shelves[next];
            (n.h, n.generation) = (0, n.generation.wrapping_add(1));
        }
        Some(start)
    }

    /// Cut a reused shelf `i` down to about `ph`; the rest becomes a free shelf (a dead entry, or a
    /// new one within the capacity reserved at creation).
    fn split(&mut self, i: usize, ph: u32) {
        let s = &self.shelves[i];
        if s.h <= ph + ph / 4 + 1 {
            return;
        }
        let rest = Shelf { y: s.y + ph, h: s.h - ph, x: 0, used: 0, retired: 0, generation: 0 };
        self.shelves[i].h = ph;
        match self.shelves.iter().position(|s| s.h == 0) {
            Some(d) => self.shelves[d] = Shelf { generation: self.shelves[d].generation.wrapping_add(1), ..rest },
            None => self.shelves.push(rest),
        }
    }
}

enum Alloc {
    /// x, y, shelf, and whether a shelf's glyphs were dropped for it
    At(u32, u32, u16, bool),
    /// no room this frame; a shelf was retired for a later one
    Retired,
    Full,
}

#[derive(Clone,Copy,PartialEq)]
enum Reuse { Fresh,Free,Retire }

/// A glyph of a layout as `glyphs` replays it: its cache key (with the subpixel bin it had), its
/// position before `physical` (`x + x_offset`, `y - y_offset`, its run's `line_y`) and its slot.
#[derive(Clone, Copy)]
struct CachedGlyph {
    key: CacheKey,
    x: f32,
    y: f32,
    line_y: f32,
    physical: (i32, i32),
    slot: Slot,
    uv: Rect,
    atlas_uv: [u32; 2],
    color: Option<[u8;4]>,
    clip: [f32;2],
}

/// Layouts stop keeping glyph lists past this many bytes in all (a list goes with its layout).
pub const CACHE_BYTES: usize = 4 << 20;

/// The parts of `Text` a glyph emission touches (fonts, rasteriser, atlases and their slots), a field
/// of their own so `glyphs` borrows them beside a layout entry.
struct Glyphs {
    fs: FontSystem,
    swash: SwashCache,
    /// the dilated mask of the glyph being rasterised (reused, so only a larger glyph allocates)
    scratch: Vec<u8>,
    atlases: [Atlas; 4],
    slots: HashMap<CacheKey, Slot, Fast>,
    dirty: Vec<(AtlasKind, u32, u32, u32, u32)>,
    evictions: u64,
    /// shelves retired while kept output may show them, and glyphs that found no room
    retirements: u64,
    misses: u64,
    /// `Text::frame`, for the shelves' use marks
    frame: u32,
    /// shelves emitted from since `Text::set_marks`, and the ones kept output shows (`Text::pin`)
    marks: Shelves,
    pinned: Shelves,
}

impl Glyphs {
    /// The atlas slot of a glyph, rasterised on a miss. A zero-sized slot draws nothing (also the
    /// answer for a glyph without an image); `None` when the atlas has no room this frame.
    fn slot(&mut self, key: CacheKey) -> Option<Slot> {
        if let Some(s) = self.slots.get(&key) {
            return Some(*s);
        }
        let empty = Slot { kind: AtlasKind::Mask, shelf: 0, generation: 0, x: 0, y: 0, w: 0, h: 0, left: 0, top: 0 };
        let Some(img) = self.swash.get_image_uncached(&mut self.fs, key) else { return Some(empty) };
        let (w, h) = (img.placement.width, img.placement.height);
        let (left, top) = (img.placement.left, img.placement.top);
        let (kind, bpp) = match img.content {
            SwashContent::Mask => (AtlasKind::Mask, 1),
            SwashContent::Color => (AtlasKind::Color, 4),
            SwashContent::SubpixelMask => return Some(empty),
        };
        if img.data.len() != (w * h * bpp) as usize { return Some(empty); }
        // cosmic-text's (swash) coverage on every platform, with macOS-style font smoothing (`dilate`) on outline masks
        let (mut s, data, stride) = if kind == AtlasKind::Mask && w > 0 && h > 0 {
            let (x0, y0, nw, nh) = dilate(&img.data, w, h, 0.006 * f32::from_bits(key.font_size_bits), &mut self.scratch);
            (Slot { w: nw, h: nh, left: left - 1 + x0 as i32, top: top + 1 - y0 as i32, ..empty }, self.scratch.as_slice(), nw as usize)
        } else {
            (Slot { kind, w, h, left, top, ..empty }, img.data.as_slice(), (w * bpp) as usize)
        };
        let (primary, w, h) = (s.kind, s.w, s.h);
        let bpp = self.atlases[idx(primary)].bpp;
        if w > 0 && h > 0 {
            let second=if primary==AtlasKind::Mask { AtlasKind::Mask2 } else { AtlasKind::Color2 };
            let mut found=None;
            'search: for reuse in [Reuse::Fresh,Reuse::Free,Reuse::Retire] {
                for kind in [primary,second] {
                    let a=&mut self.atlases[idx(kind)];
                    match a.alloc(kind,w,h,self.frame,&self.pinned,reuse) {
                        Alloc::At(x,y,shelf,evicted)=>{ found=Some((kind,x,y,shelf,evicted)); break 'search; }
                        Alloc::Retired=>{
                            (self.evictions,self.retirements,self.misses)=(self.evictions+1,self.retirements+1,self.misses+1);
                            let shelves=&a.shelves;
                            self.slots.retain(|_,o|o.kind!=kind || o.w==0 || o.h==0 || shelves[o.shelf as usize].generation==o.generation);
                            return None;
                        }
                        Alloc::Full=>{}
                    }
                }
            }
            let Some((kind,x,y,shelf,evicted))=found else { self.misses+=1; return None };
            let a=&mut self.atlases[idx(kind)];
            s.kind=kind;
            for row in 0..h {
                let src = row as usize * stride;
                let dst = (((y + row) * a.w + x) * bpp) as usize;
                a.pixels()[dst..dst + (w * bpp) as usize].copy_from_slice(&data[src..src + (w * bpp) as usize]);
            }
            if evicted {
                self.evictions += 1;
                let shelves = &a.shelves;
                self.slots.retain(|_, o| o.kind != kind || o.w == 0 || o.h == 0 || shelves[o.shelf as usize].generation == o.generation);
            }
            self.dirty.push((kind, x, y, w, h));
            (s.shelf, s.generation, s.x, s.y) = (shelf, a.shelves[shelf as usize].generation, x, y);
        }
        self.slots.insert(key, s);
        Some(s)
    }

    /// One quad for `slot` with its pen at device pixel (`x`, `y`); marks the slot's shelf used.
    fn emit(&mut self, slot: Slot, x: i32, y: i32, color:Option<[u8;4]>, f: &mut dyn FnMut(Glyph)) {
        if slot.w == 0 || slot.h == 0 {
            return;
        }
        self.atlases[idx(slot.kind)].shelves[slot.shelf as usize].used = self.frame;
        let (word, bit) = shelf_bit(slot.kind, slot.shelf);
        self.marks[word] |= bit;
        let uv = self.uv(&slot);
        f(Glyph {
            rect: Rect { x: (x + slot.left) as f32, y: (y - slot.top) as f32, w: slot.w as f32, h: slot.h as f32 },
            uv,
            atlas_uv: packed_uv(uv),
            atlas: slot.kind,
            color,
        });
    }

    /// The slot's texel rectangle normalised by its atlas's size.
    fn uv(&self, slot: &Slot) -> Rect {
        let a = &self.atlases[idx(slot.kind)];
        let (aw, ah) = (a.w as f32, a.h as f32);
        Rect { x: slot.x as f32 / aw, y: slot.y as f32 / ah, w: slot.w as f32 / aw, h: slot.h as f32 / ah }
    }
}


/// macOS-style font smoothing on every platform: CoreText's stem growth applied to cosmic-text's coverage, a round dilation by `d` pixels shifted up by
/// `d` is `c + d (|grad c| + dc/dy)` at an edge (empty pixels fill only where the edge crosses in). `out` gets the mask grown by one pixel all round,
/// then trimmed of empty rows and columns; returns the trim's offset into the grown mask and its size.
fn dilate(src: &[u8], w: u32, h: u32, d: f32, out: &mut Vec<u8>) -> (u32, u32, u32, u32) {
    let (w, h) = (w as i32, h as i32);
    let at = |x: i32, y: i32| if x < 0 || y < 0 || x >= w || y >= h { 0.0 } else { src[(y * w + x) as usize] as f32 };
    out.clear();
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, -1, -1);
    for y in -1..=h {
        for x in -1..=w {
            let gx = (at(x + 1, y) - at(x - 1, y)) * 0.5;
            let gy = (at(x, y + 1) - at(x, y - 1)) * 0.5;
            let c = at(x, y);
            // an empty pixel only fills where the grown edge crosses into it (the top edge grows 2d);
            // only next to ink, or at large sizes (2d > 1) every empty pixel of the box would fill
            let grown = if c > 0.0 { c + d * (gx.hypot(gy) + gy) } else {
                let edge = |n: f32, g: f32| if n > 0.0 { n / 255.0 + g - 1.0 } else { 0.0 };
                255.0 * edge(at(x - 1, y).max(at(x + 1, y)), d).max(edge(at(x, y + 1), 2.0 * d)).max(0.0)
            };
            let v = grown.clamp(0.0, 255.0).round() as u8;
            if v > 0 {
                (x0, y0, x1, y1) = (x0.min(x + 1), y0.min(y + 1), x1.max(x + 1), y1.max(y + 1));
            }
            out.push(v);
        }
    }
    if x1 < 0 {
        out.clear();
        return (0, 0, 0, 0);
    }
    let (pw, nw, nh) = (w + 2, x1 - x0 + 1, y1 - y0 + 1);
    for r in 0..nh {
        let from = ((y0 + r) * pw + x0) as usize;
        out.copy_within(from..from + nw as usize, (r * nw) as usize);
    }
    out.truncate((nw * nh) as usize);
    (x0 as u32, y0 as u32, nw as u32, nh as u32)
}

/// CoreText uses fewer horizontal phases as the em grows.
#[inline]
fn apple_x_bins(ppem: f32) -> i32 {
    if ppem < 100.0 / 9.0 { 4 }
    else if ppem < 100.0 / 6.0 { 3 }
    else if ppem < 100.0 / 3.0 { 2 }
    else { 1 }
}

/// Where CoreText puts a glyph origin: x floored to 1/4, 1/3, 1/2 or 1 pixel by ppem (a third is
/// the nearest cosmic-text bin, a quarter or three quarters; their error is under 1/12 pixel).
fn apple_x(ppem: f32, pos: f32) -> (i32, SubpixelBin) {
    let n = apple_x_bins(ppem);
    let m = (pos * n as f32).floor() as i32;
    // one division: the remainder from the quotient
    let q = m.div_euclid(n);
    let bin = match (n, m - q * n) {
        (_, 0) => SubpixelBin::Zero,
        (4 | 3, 1) => SubpixelBin::One,
        (4, 2) | (2, 1) => SubpixelBin::Two,
        _ => SubpixelBin::Three,
    };
    (q, bin)
}

/// Whether an integral device-pixel origin can translate the cached CoreText x bin.
fn apple_x_translates(ppem: f32, x: f32) -> bool {
    // Within ±8192 pixels the f32 rounding of position costs at most 1/512 of a quarter-pixel bin.
    // A bin boundary itself is exact with a binary denominator and an integral origin.
    let n = apple_x_bins(ppem);
    if !(-4096.0..=4096.0).contains(&x) || n == 3 {
        return false;
    }
    let bin = x * n as f32;
    let gap = bin.ceil() - bin;
    gap == 0.0 || gap >= 1.0 / 256.0
}


struct Entry {
    buf: Buffer,
    text: String,
    style: Style,
    /// the spans this layout was made with (kept while the entry is recycled, so a layout with spans reuses their storage), and whether it has had any
    rich: Vec<TextSpan>,
    is_rich: bool,
    width: u32,
    frame: u32,
    /// the virtual list (0: none) and row that last asked for this layout
    list: u64,
    row: u32,
    /// the view (0: none) whose paint last drew this layout: it owns it while its last paint is `frame`
    view: u32,
    m: Metrics,
    /// the first baseline below the top as the shaped glyphs sit (`m.baseline` is the CSS one when the style has a line height)
    raw_baseline: f32,
    /// The glyph list `glyphs` replays: valid while `cached`, for `cached_scale` (f32 bits), until
    /// the atlas evicts past `cached_evictions`.
    glyphs: Vec<CachedGlyph>,
    cached: bool,
    cacheable: bool,
    cached_scale: u32,
    cached_origin: (u32, u32),
    cached_translates_x: bool,
    cached_evictions: u64,
    /// the shelves the list's glyphs sit on
    shelves: Shelves,
}

impl Entry {
    /// An evicted entry made ready to be laid out again; its storage is kept.
    fn recycle(&mut self, frame: u32) {
        self.rich.clear();
        self.is_rich = false;
        self.width = 0;
        self.frame = frame;
        (self.list, self.row, self.view) = (0, 0, 0);
        self.m = Metrics::default();
        self.glyphs.clear();
        self.cached = false;
        self.cacheable = true;
        (self.cached_scale, self.cached_evictions) = (0, 0);
        self.shelves = [0; 8];
    }

    fn empty(style: &Style, frame: u32) -> Self {
        let lh = if style.line_height > 0.0 { style.line_height } else { style.size * 1.2 };
        Self {
            buf: Buffer::new_empty(CMetrics::new(style.size, lh)),
            text: String::new(), style: *style, rich: Vec::new(), is_rich: false, width: 0, frame, list: 0, row: 0, view: 0,
            m: Metrics::default(), raw_baseline: 0.0, glyphs: Vec::new(), cached: false, cacheable: true,
            cached_scale: 0, cached_origin: (0, 0), cached_translates_x: false, cached_evictions: 0, shelves: [0; 8],
        }
    }
}

pub struct Text {
    g: Glyphs,
    system:Option<Box<system::SystemFonts>>,
    names: Vec<String>,
    /// bytes of the fonts `add_font` accepted, against `MAX_FONT_TOTAL_BYTES`
    font_bytes: usize,
    /// estimated bytes of the retained layouts (`layout_cost` of each text), against `LAYOUT_BYTES`
    layout_bytes: usize,
    font_metrics: Vec<(f32,f32)>,
    wrap_rows: Vec<(usize,usize,usize,f32)>,
    entries: HashMap<u64, Entry, Fast>,
    /// evicted layouts awaiting reuse (at most `SPARE_LAYOUTS`)
    spare: Vec<Entry>,
    /// every entry's key, and where the incremental sweep goes on
    keys: Vec<u64>,
    sweep: usize,
    /// per plain text (by `layout_key` at width 0) the widths it was last shaped at, for a node that never laid it out (`recent_widths`)
    recent: Box<[Recent]>,
    frame: u32,
    /// built row windows `(list, first, last)` of up to `LIST_WINDOWS` lists, in build order
    windows: [ListWindow; LIST_WINDOWS],
    /// the list and row being built (list 0: none)
    scope: (u64, u32),
    /// the view being painted (0: none)
    owner: u32,
    /// each live view's last painted frame (0: not live), by view number
    views: Vec<u32>,
    walks: u64,
    layouts: u64,
    cache_on: bool,
    /// Capacity of every layout's glyph list, in bytes.
    cache_bytes: usize,
    font_generation: Arc<()>,
    /// The prepare queue's lane (`attach`): text shaped ahead of need on a worker.
    queue: Option<Box<queue::Queue>>,
    /// Debug builds: (text hash, key, style, width) of the last installed prepared layouts, to name a draw that asks for another style.
    #[cfg(debug_assertions)]
    installed: Vec<(u64, u64, Style, f32)>,
    /// Debug builds: where a style mismatch is reported (the host sets it; none = silent).
    #[cfg(debug_assertions)]
    pub reporter: Option<fn(std::fmt::Arguments)>,
}

fn idx(kind: AtlasKind) -> usize {
    kind as usize
}

impl Text {
    /// Text for locale `en-US` (deterministic: goldens and tests); hosts pass the user's with
    /// `new_with_locale` (`env_locale`).
    pub fn new(fonts: &[&[u8]]) -> Text {
        Text::new_with_locale(fonts, "en-US", MASK_SIZE, COLOR_SIZE)
    }

    /// `new` with a `mask` (R8) and `color` (RGBA8) atlas of these sizes in texels.
    pub fn new_sized(fonts: &[&[u8]], mask: (u32, u32), color: (u32, u32)) -> Text {
        Text::new_with_locale(fonts, "en-US", mask, color)
    }

    /// Text whose font fallback follows `locale` (a BCP 47 tag such as `ja-JP`), with atlases of
    /// these sizes (`ATLAS_SIZES` are `new`'s).
    pub fn new_with_locale(fonts: &[&[u8]], locale: &str, mask: (u32, u32), color: (u32, u32)) -> Text {
        let mut t = Text {
            // not new_with_fonts: that calls load_system_fonts
            g: Glyphs {
                fs: FontSystem::new_with_locale_and_db(locale.into(), fontdb::Database::new()),
                swash: SwashCache::new(),
                scratch: Vec::with_capacity(32 << 10),
                atlases: [Atlas::new(mask, 1), Atlas::new(color, 4),Atlas::new(mask,1),Atlas::new(color,4)],
                slots: HashMap::default(),
                dirty: Vec::new(),
                evictions: 0,
                retirements: 0,
                misses: 0,
                frame: 1,
                marks: [0; 8],
                pinned: [0; 8],
            },
            names: Vec::new(),
            font_bytes: 0,
            layout_bytes: 0,
            system:None,
            font_metrics: Vec::new(),
            wrap_rows: Vec::new(),
            entries: HashMap::default(),
            spare: Vec::with_capacity(SPARE_LAYOUTS),
            keys: Vec::new(),
            sweep: 0,
            recent: vec![Recent { key: 0, widths: [0.0; 2] }; RECENT_SLOTS].into_boxed_slice(),
            frame: 1,
            windows: [ListWindow::default(); LIST_WINDOWS],
            scope: (0, 0),
            owner: 0,
            views: Vec::new(),
            walks: 0,
            layouts: 0,
            cache_on: true,
            cache_bytes: 0,
            font_generation: Arc::new(()),
            queue: None,
            #[cfg(debug_assertions)]
            installed: Vec::new(),
            #[cfg(debug_assertions)]
            reporter: None,
        };
        for f in fonts {
            let _ = t.add_font(f);
        }
        t
    }

    /// Enable OS discovery on missing glyphs. Explicit-font constructors stay deterministic.
    /// Returns false when this platform has no native provider. Cold discovery may allocate/map files. System fonts are memory-mapped,
    /// demand-paged and outside the `add_font` budget; they are bounded by file count, file size and total mapped bytes (see `system.rs`:
    /// 128 files, 256 MiB each, 1 GiB mapped).
    pub fn enable_system_fonts(&mut self)->bool {
        if !font_sys::SUPPORTED { return false; }
        if self.system.is_none() {
            let mut system=system::SystemFonts::default();
            if self.g.fs.db().faces().next().is_none() && !system.resolve("A",&mut self.g.fs) { return false; }
            self.system=Some(Box::new(system));
            self.entries.clear();
            self.keys.clear();
            self.sweep=0;
            self.cache_bytes=0;
            self.layout_bytes=0;
            self.font_generation=Arc::new(());
        }
        true
    }

    /// Loads a font (copies the bytes); returns its index for `Style::font`, or `None` when it is refused: empty, larger than
    /// `MAX_FONT_BYTES`, not a parseable font, or past `MAX_FONTS` fonts or `MAX_FONT_TOTAL_BYTES` in all (nothing is kept then).
    /// Existing layouts keep their old shaping until their text, width or style changes.
    pub fn add_font(&mut self, bytes: &[u8]) -> Option<usize> {
        if bytes.is_empty() || bytes.len() > MAX_FONT_BYTES || self.names.len() >= MAX_FONTS || self.font_bytes + bytes.len() > MAX_FONT_TOTAL_BYTES { return None; }
        let ids = self.g.fs.db_mut().load_font_source(fontdb::Source::Binary(Arc::new(bytes.to_vec())));
        let metrics = ids.first().and_then(|id| self.g.fs.get_font(*id, Weight::NORMAL)).map(|f| {
            let m = f.as_swash().metrics(&[]);
            (m.ascent / m.units_per_em as f32,m.descent.abs() / m.units_per_em as f32)
        });
        let Some(metrics) = metrics else {
            for id in ids { self.g.fs.db_mut().remove_face(id); }
            return None;
        };
        let name = ids.first().and_then(|id| self.g.fs.db().face(*id)).and_then(|f| f.families.first()).map(|f| f.0.clone()).unwrap_or_default();
        self.names.push(name);
        self.font_metrics.push(metrics);
        self.font_bytes += bytes.len();
        self.font_generation=Arc::new(());
        Some(self.names.len() - 1)
    }

    /// How many fonts `add_font` has accepted.
    pub fn font_count(&self) -> usize { self.names.len() }

    /// The font's ascent as a fraction of its size (0 for an unknown index).
    pub fn ascent(&self, font: u16) -> f32 {
        self.font_metrics.get(font as usize).map_or(0.0,|metrics|metrics.0)
    }

    /// The first baseline of a line of `style` with an explicit line height, below the line's top, as CSS places it: the rounded
    /// primary-face ascent plus the floored half-leading (ULP-snapped at whole points); `None` without a line height or font metrics.
    fn css_baseline(&self,style:&Style)->Option<f32> {
        if !(style.line_height>0.0) { return None; }
        let (a,d)=self.font_metrics(style.font)?;
        let (a,d)=((a*style.size).round(),(d*style.size).round());
        let line=style.line_height;
        let leading=(line-a-d)/2.0;
        let rounded=leading.round();
        let tolerance=4.0*f32::EPSILON*line.abs().max(a).max(d);
        let leading=if (leading-rounded).abs()<=tolerance {rounded} else {leading};
        Some(a+leading.floor())
    }

    /// Primary-face distances above and below the baseline, normalized to the em size.
    pub fn font_metrics(&self,font:u16)->Option<(f32,f32)> {
        self.font_metrics.get(font as usize).copied().filter(|(a,d)|a.is_finite() && d.is_finite() && *a>0.0 && *d>=0.0)
    }

    /// Debug builds: a layout is shaped for text that was installed prepared under another style or width.
    #[cfg(debug_assertions)]
    fn report_style_mismatch(&self, key: u64, text: &str, style: &Style, width: f32) {
        let Some(report) = self.reporter else { return };
        if self.installed.is_empty() { return; }
        let hash = text_hash(text);
        if let Some((_, _, prepared, prepared_width)) = self.installed.iter().find(|item| item.0 == hash && item.1 != key) {
            report(format_args!("prepared text {text:?} was requested with a different style: prepared {prepared:?} width {prepared_width}, requested {style:?} width {width}"));
        }
    }

    pub fn has_text(&self, key: u64, text: &str) -> bool {
        self.entries.get(&key).is_some_and(|entry| entry.text == text)
    }

    /// The cached layout `key` of plain `text`, when it is also the layout at `max_width`: prepared unbounded, or at a width W >= `max_width`
    /// with every line already no wider than `max_width` (`Metrics::w`). Greedy wrapping breaks only where the next word does not fit, and a
    /// word that did not fit W does not fit a narrower width, while every line that fit W and is no wider than `max_width` still fits: the
    /// breaks, and so the glyphs of left-aligned text, are identical. Other alignments place glyphs by the width: only an unbounded layout at
    /// exactly its natural width, or the width it was laid out at. An ellipsizing style reuses a bounded layout only when it has fewer lines
    /// than its limit, so nothing was cut. `None` for any other width, style, text, or an uncached key.
    /// Allocates: never.
    pub fn reusable_layout(&mut self, key: u64, text: &str, style: &Style, max_width: f32) -> Option<Metrics> {
        if !(max_width.is_finite() && max_width > 0.0) { return None; }
        let frame = self.frame;
        let e = self.entries.get_mut(&key)?;
        if e.style != *style || e.text != text || e.is_rich || e.m.w > max_width { return None; }
        let built = f32::from_bits(e.width);
        let fits = if built.is_infinite() { style.align == Align::Left || e.m.w == max_width }
            else {
                max_width <= built && (max_width == built || (style.align == Align::Left
                    && (style.ellipsis == Ellipsis::None || (e.m.lines as u32) < style.max_lines.max(1) as u32)))
            };
        if !fits { return None; }
        e.frame = frame;
        if self.scope.0 != 0 { (e.list, e.row) = self.scope; }
        Some(e.m)
    }

    /// The last two widths (newest first, 0.0: none) plain text of natural key `natural` (`layout_key` at width 0, no spans) was shaped at. A node
    /// that never laid the text out tries each through `reusable_layout`: a measure at W serves a draw at its measured width. Bounded by
    /// `RECENT_SLOTS` (direct-mapped, overwritten by a colliding text; entries are never swept because a stale one is validated on use).
    /// Allocates: never.
    pub fn recent_widths(&self, natural: u64) -> [f32; 2] {
        let slot = &self.recent[natural as usize % RECENT_SLOTS];
        if slot.key == natural { slot.widths } else { [0.0; 2] }
    }

    fn note_width(&mut self, natural: u64, width: f32) {
        let slot = &mut self.recent[natural as usize % RECENT_SLOTS];
        if slot.key != natural { *slot = Recent { key: natural, widths: [width, 0.0] }; }
        else if slot.widths[0] != width { slot.widths = [width, slot.widths[0]]; }
    }

    pub fn layout(&mut self, key: u64, text: &str, style: &Style, max_width: f32) -> Metrics {
        self.layout_inner(key,text,style,max_width,&[])
    }

    pub fn layout_spans(&mut self,key:u64,text:&str,style:&Style,max_width:f32,spans:&[TextSpan])->Option<Metrics> {
        if !valid_spans(text,spans) { return None; }
        Some(self.layout_inner(key,text,style,max_width,spans))
    }

    fn layout_inner(&mut self, key: u64, text: &str, style: &Style, max_width: f32, spans:&[TextSpan]) -> Metrics {
        let width = if max_width.is_finite() && max_width > 0.0 { max_width } else { f32::INFINITY };
        let frame = self.frame;
        if let Some(e) = self.entries.get_mut(&key) {
            e.frame = frame;
            if self.scope.0 != 0 { (e.list, e.row) = self.scope; }
            if e.width == width.to_bits() && e.style == *style && e.text == text
                && e.rich==spans {
                return e.m;
            }
        } else if self.keys.len() >= MAX_LAYOUTS {
            let at = self.victim();
            let old = self.keys.swap_remove(at);
            self.sweep = (at + 1) % MAX_LAYOUTS;
            if let Some(entry) = self.entries.remove(&old) {
                self.retire(entry);
            }
        }
        #[cfg(debug_assertions)]
        self.report_style_mismatch(key, text, style, width);
        self.layouts = self.layouts.saturating_add(1);
        // shaping dwarfs one hash of the text
        if spans.is_empty() && width.is_finite() { self.note_width(layout_key(style, 0.0, text_hash(text), &[]), width); }
        let lh = if style.line_height > 0.0 { style.line_height } else { style.size * 1.2 };
        let css = self.css_baseline(style);
        let before = self.entries.get(&key).map_or(0, |e| layout_cost(e.text.len()));
        let keys = &mut self.keys;
        let (spare, cache_bytes) = (&mut self.spare, &mut self.cache_bytes);
        let e = self.entries.entry(key).or_insert_with(|| {
            keys.push(key);
            // one whose text storage already fits, so a changing label grows storage only until the pool has seen its longest
            let fit = spare.iter().rposition(|e| e.text.capacity() >= text.len()).or(spare.len().checked_sub(1));
            match fit.map(|at| spare.swap_remove(at)) {
                Some(mut e) => {
                    *cache_bytes += e.glyphs.capacity() * size_of::<CachedGlyph>();
                    e.recycle(frame);
                    e
                }
                None => {
                    let mut e = Entry::empty(style, frame);
                    e.text.reserve(LABEL_BYTES);
                    e.glyphs.reserve_exact(LABEL_GLYPHS);
                    *cache_bytes += e.glyphs.capacity() * size_of::<CachedGlyph>();
                    e
                }
            }
        });
        e.frame = frame;
        if self.scope.0 != 0 { (e.list, e.row) = self.scope; }
        e.cached = false;
        e.cacheable = true;
        e.style = *style;
        e.width = width.to_bits();
        e.text.clear();
        e.text.push_str(text);
        let family = match self.names.get(style.font as usize) {
            Some(n) => Family::Name(n),
            None => Family::SansSerif,
        };
        let attrs = Attrs::new().family(family).weight(Weight(style.weight)).style(if style.italic { CStyle::Italic } else { CStyle::Normal });
        let attrs = attrs.cache_key_flags(CacheKeyFlags::DISABLE_HINTING);
        let attrs = if style.tracking != 0.0 { attrs.letter_spacing(style.tracking / style.size) } else { attrs };
        let mut features=FontFeatures::new();
        if style.features & Style::FEATURE_TABULAR_NUMBERS != 0 { features.enable(FeatureTag::new(b"tnum")); }
        let attrs=attrs.font_features(features);
        let b = &mut e.buf;
        b.set_metrics(CMetrics::new(style.size, lh));
        b.set_wrap(match style.wrap {
            Wrap::None => CWrap::None,
            Wrap::Word | Wrap::WordSpaces => CWrap::WordOrGlyph,
            Wrap::Glyph => CWrap::Glyph,
        });
        let lines = style.max_lines as usize;
        let limit=EllipsizeHeightLimit::Lines(lines.max(1));
        b.set_ellipsize(match style.ellipsis {
            Ellipsis::None=>Ellipsize::None,
            Ellipsis::End=>Ellipsize::End(limit),
            Ellipsis::Middle=>Ellipsize::Middle(limit),
            Ellipsis::Head=>Ellipsize::Start(limit),
        });
        let visible=if style.ellipsis==Ellipsis::None { lines } else { lines.max(1) };
        let height = (visible>0).then_some(visible as f32*lh);
        b.set_size(width.is_finite().then_some(width), height);
        let align = match style.align {
            Align::Left => CAlign::Left,
            Align::Center => CAlign::Center,
            Align::Right => CAlign::Right,
        };
        b.set_text(text, &attrs, Shaping::Advanced, Some(align));
        if spans.iter().any(|s|s.weight!=0 && s.weight!=style.weight) {
            let mut offset=0;
            for line in &mut b.lines {
                let end=offset+line.text().len();
                let mut list=AttrsList::new(&attrs);
                for span in spans.iter().filter(|s|s.weight!=0) {
                    let (start,stop)=((span.start as usize).max(offset),(span.end as usize).min(end));
                    if start>=stop { continue; }
                    let custom=attrs.clone().weight(Weight(span.weight));
                    list.add_span(start-offset..stop-offset,&custom);
                }
                offset=end+line.ending().as_str().len();
                line.set_attrs_list(list);
            }
        }
        b.shape_until_scroll(&mut self.g.fs, true);
        if let Some(system)=&mut self.system {
            if system::shape(b,&mut self.g.fs,system) { self.font_generation=Arc::new(()); }
        }
        let mut m = Metrics::default();
        let mut raw = 0.0;
        let logical_spaces=style.wrap==Wrap::WordSpaces;
        let rows=&mut self.wrap_rows;
        if logical_spaces {rows.clear();}
        for run in b.layout_runs() {
            if logical_spaces {
                let start=run.glyphs.iter().map(|g|g.start).min().unwrap_or(0);
                let end=run.glyphs.iter().map(|g|g.end).max().unwrap_or(0);
                rows.push((run.line_i,start,end,run.line_w));
            }
            if m.lines == 0 {
                raw = run.line_y - run.line_top;
                m.baseline = css.unwrap_or(raw);
            }
            m.lines += 1;
            m.w = m.w.max(run.line_w);
            m.h = run.line_top + run.line_height;
        }
        if logical_spaces {
            rows.sort_unstable_by_key(|row|(row.0,row.2));
            // Soft wraps omit their separating blank glyphs; retain their logical advance for fitting.
            for (line,buffer) in b.lines.iter().enumerate() {
                if let Some(shape)=buffer.shape_opt() {
                    for glyph in shape.spans.iter().flat_map(|s|&s.words).filter(|w|w.blank).flat_map(|w|&w.glyphs) {
                        let next=rows.partition_point(|row|(row.0,row.2)<=(line,glyph.start));
                        if next>0 && next<rows.len() && rows[next-1].0==line && rows[next].0==line
                            && glyph.start>=rows[next-1].2 && glyph.end<=rows[next].1 {
                            rows[next-1].3+=glyph.width(style.size);
                        }
                    }
                }
            }
            m.w=rows.iter().fold(m.w,|width,row|width.max(row.3));
        }
        if !spans.is_empty() || e.is_rich {
            e.is_rich = true;
            e.rich.clear();
            e.rich.extend_from_slice(spans);
        }
        e.m = m;
        e.raw_baseline = raw;
        self.layout_bytes = self.layout_bytes - before + layout_cost(text.len());
        self.fit_budget(key);
        m
    }

    /// Evicts layouts until `layout_bytes` fits `LAYOUT_BYTES`: the best victim by `rank` that was not laid out this frame (any but `keep`
    /// once past twice the budget). Scans once per eviction, only while over budget.
    fn fit_budget(&mut self, keep: u64) {
        while self.layout_bytes > LAYOUT_BYTES {
            let hard = self.layout_bytes > 2 * LAYOUT_BYTES;
            let mut best: Option<(usize, u64)> = None;
            for (at, key) in self.keys.iter().enumerate() {
                let Some(e) = self.entries.get(key) else { best = Some((at, u64::MAX)); break };
                if *key == keep || (!hard && e.frame == self.frame) { continue; }
                let rank = self.rank(e);
                if best.is_none_or(|(_, r)| rank >= r) { best = Some((at, rank)); }
            }
            let Some((at, _)) = best else { return };
            let old = self.keys.swap_remove(at);
            if let Some(entry) = self.entries.remove(&old) { self.retire(entry); }
        }
    }

    /// Where the shaped glyphs of layout `key` put their first baseline below the layout's top: `Metrics::baseline` unless the style has
    /// a line height, whose baseline is the CSS one; the offset `glyphs`' origin is relative to. 0 for an unknown key.
    pub fn glyph_baseline(&self,key:u64)->f32 { self.entries.get(&key).map_or(0.0,|e|e.raw_baseline) }

    /// Line boxes from the retained shaped runs, with no temporary allocation.
    pub fn decorations(&self,key:u64,underline:bool,f:&mut dyn FnMut(Decoration)) {
        let Some(entry)=self.entries.get(&key) else { return };
        if !entry.is_rich { return; }
        let flag=if underline { TextSpan::UNDERLINE } else { TextSpan::BACKGROUND };
        for span in entry.rich.iter().filter(|s|s.flags & flag!=0) {
            let (a,b)=(Self::line_of(&entry.buf,span.start as usize),Self::line_of(&entry.buf,span.end as usize));
            for run in entry.buf.layout_runs().filter(|r|r.line_i>=a.line && r.line_i<=b.line) {
                let start=if run.line_i==a.line { a.index } else { 0 };
                let end=if run.line_i==b.line { b.index } else { run.text.len() };
                let emit=|range:[f32;2],f:&mut dyn FnMut(Decoration)| {
                    let h=if underline { run.line_height.min(1.5) } else { run.line_height };
                    let y=if underline { run.line_top+run.line_height-h } else { run.line_top };
                    f(Decoration { rect:Rect { x:range[0],y,w:range[1]-range[0],h },color:rgba(if underline { span.underline } else { span.background }) });
                };
                run_ranges(&run,start,end,&mut |range| emit(range,f));
            }
        }
    }

    /// Same as `layout`: the shaped buffer is cached either way.
    pub fn measure(&mut self, key: u64, text: &str, style: &Style, max_width: f32) -> Metrics {
        self.layout(key, text, style, max_width)
    }

    /// Whole-text byte index -> (line, byte in line).
    fn line_of(buf: &Buffer, byte: usize) -> Cursor {
        let mut start = 0;
        for (i, l) in buf.lines.iter().enumerate() {
            let len = l.text().len();
            if byte <= start + len || i + 1 == buf.lines.len() {
                return Cursor::new(i, byte.saturating_sub(start).min(len));
            }
            start += len + l.ending().as_str().len();
        }
        Cursor::new(0, 0)
    }
    fn line_start(buf: &Buffer, line: usize) -> usize {
        buf.lines.iter().take(line).map(|l| l.text().len() + l.ending().as_str().len()).sum()
    }

    pub fn hit(&self, key: u64, x: f32, y: f32) -> usize {
        let Some(e) = self.entries.get(&key) else { return 0 };
        match e.buf.hit(x, y) {
            Some(c) => Self::line_start(&e.buf, c.line) + c.index,
            None => 0,
        }
    }

    /// Zero-width rect (x, line top, 0, line height) of the caret before `byte`.
    pub fn caret(&self, key: u64, byte: usize) -> Rect {
        let Some(e) = self.entries.get(&key) else { return Rect::default() };
        let c = Self::line_of(&e.buf, byte);
        let lh = e.buf.metrics().line_height;
        match e.buf.cursor_position(&c) {
            Some((x, y)) => Rect { x, y, w: 0.0, h: lh },
            None => Rect { x: 0.0, y: 0.0, w: 0.0, h: lh },
        }
    }

    /// One rect per contiguous highlighted span per visual line.
    pub fn selection(&self, key: u64, a: usize, b: usize, f: &mut dyn FnMut(Rect)) {
        let Some(e) = self.entries.get(&key) else { return };
        let (a, b) = (a.min(b), a.max(b));
        let (ca, cb) = (Self::line_of(&e.buf, a), Self::line_of(&e.buf, b));
        for run in e.buf.layout_runs() {
            if run.line_i < ca.line || run.line_i > cb.line {
                continue;
            }
            let start=if run.line_i==ca.line { ca.index } else { 0 };
            let end=if run.line_i==cb.line { cb.index } else { run.text.len() };
            run_ranges(&run,start,end,&mut |range| {
                f(Rect { x:range[0],y:run.line_top,w:range[1]-range[0],h:run.line_height });
            });
        }
    }

    /// Emits the layout's glyph quads at `origin` (points) for `scale`. The first walk through
    /// cosmic-text's runs keeps each glyph's line-relative position, subpixel bin and atlas slot in the
    /// layout's entry; later calls at any origin replay that list with the same float operations as
    /// `LayoutGlyph::physical`, so the quads equal a walk's. A glyph whose bin differs at the new
    /// origin (a fractional origin) looks its slot up as a walk would. The list is rebuilt when the
    /// layout is re-shaped, the scale changes or the atlas evicted a shelf. Inside `paint_view` the
    /// layout belongs to that view.
    pub fn glyphs(&mut self, key: u64, origin: (f32, f32), scale: f32, f: &mut dyn FnMut(Glyph)) {
        self.emit_glyphs(key, origin, scale, false, None, f);
    }

    /// Emit at an absolute first baseline, reusing the layout lookup to find its original baseline.
    pub fn glyphs_at_baseline(&mut self, key: u64, at: (f32, f32), scale: f32, f: &mut dyn FnMut(Glyph)) {
        self.emit_glyphs(key, at, scale, true, None, f);
    }

    /// Populate an integral-origin glyph cache entry without emitting draw instances.
    pub fn prepare_glyphs(&mut self, key: u64, scale: f32) {
        if self.entries.get(&key).is_some_and(|entry| {
            !entry.cached || entry.cached_scale != scale.to_bits() || entry.cached_evictions != self.g.evictions
        }) {
            self.glyphs_at_baseline(key, (0.0, 0.0), scale, &mut |_| {});
        }
    }

    /// Draw the layout at a snapped baseline without calling back for every unadorned cached glyph.
    pub fn draw_at_baseline(&mut self, key: u64, at: (f32, f32), density: i64, paint: GlyphPaint, out: &mut Vec<Instance>, breaks: &mut Vec<Break>) {
        let scale = density as f32;
        let inverse = (density as u64).is_power_of_two().then(|| scale.recip());
        self.emit_glyphs(key, at, scale, true, Some((paint, inverse, out, breaks)), &mut |_| {});
    }

    fn emit_glyphs(
        &mut self, key: u64, mut origin: (f32, f32), scale: f32, baseline: bool,
        mut draw: Option<(GlyphPaint, Option<f32>, &mut Vec<Instance>, &mut Vec<Break>)>,
        f: &mut dyn FnMut(Glyph),
    ) {
        let Text { g, entries, walks, cache_on, cache_bytes, owner, .. } = self;
        let Some(e) = entries.get_mut(&key) else { return };
        if baseline { origin.1 -= e.raw_baseline; }
        if *owner != 0 { e.view = *owner; }
        if e.cached && e.cached_scale == scale.to_bits() && e.cached_evictions == g.evictions {
            for k in 0..8 {
                g.marks[k] |= e.shelves[k];
            }
            let (ox, start) = (origin.0 * scale, g.evictions);
            let old_x = f32::from_bits(e.cached_origin.0) * scale;
            let pixel_origin = |x: f32| x.abs() <= 4096.0 && x == x.trunc();
            let shift_x = if origin.0.to_bits() == e.cached_origin.0 {
                Some(0)
            } else if e.cached_translates_x && pixel_origin(old_x) && pixel_origin(ox) {
                Some(ox as i32 - old_x as i32)
            } else {
                None
            };
            let same_y = origin.1.to_bits() == e.cached_origin.1;
            if let (Some(dx), Some((paint, Some(inverse), out, _))) = (shift_x, draw.as_mut())
                && !e.is_rich && e.shelves.iter().all(|word| word & COLOR_SHELVES == 0)
            {
                // An integral x shift keeps the bins and never has to look a slot up again.
                let template = Instance { color: paint.color, clip: paint.clip, view: paint.view,
                    kind: draw::kind::MASK, flags: paint.mask_flags, extra: paint.light, ..Instance::DEFAULT };
                let mut last_shelf = None;
                for c in &e.glyphs {
                    let s = &c.slot;
                    if s.w == 0 || s.h == 0 { continue; }
                    let shelf = (s.kind, s.shelf);
                    if last_shelf != Some(shelf) {
                        g.atlases[idx(s.kind)].shelves[s.shelf as usize].used = g.frame;
                        last_shelf = Some(shelf);
                    }
                    let x = (c.physical.0 + dx + s.left) as f32;
                    let y = if same_y { c.physical.1 } else {
                        let py = c.y.mul_add(scale, (origin.1 + c.line_y) * scale);
                        py.round() as i32
                    };
                    let y = (y - s.top) as f32;
                    let rect = [x, y, x + s.w as f32, y + s.h as f32].map(|p| p * *inverse);
                    let page = if s.kind == AtlasKind::Mask2 { draw::flags::mask::GLYPH_PAGE_2 } else { 0 };
                    out.push(Instance { rect, aux: c.atlas_uv, flags: template.flags | page, ..template });
                }
                return;
            }
            let mut last_shelf = None;
            for c in &e.glyphs {
                let (x, bin) = if let Some(dx) = shift_x { (c.physical.0 + dx, c.key.x_bin) } else {
                    let px = c.x.mul_add(scale, ox);
                    apple_x(f32::from_bits(c.key.font_size_bits), px)
                };
                let y = if same_y { c.physical.1 } else {
                    // y has no subpixel bin.
                    let py = c.y.mul_add(scale, (origin.1 + c.line_y) * scale);
                    py.round() as i32
                };
                if bin == c.key.x_bin && g.evictions == start {
                    if c.slot.w > 0 && c.slot.h > 0 {
                        let s = &c.slot;
                        let shelf = (s.kind, s.shelf);
                        if last_shelf != Some(shelf) {
                            g.atlases[idx(s.kind)].shelves[s.shelf as usize].used = g.frame;
                            last_shelf = Some(shelf);
                        }
                        let glyph=Glyph { rect: Rect { x: (x + s.left) as f32, y: (y - s.top) as f32, w: s.w as f32, h: s.h as f32 }, uv: c.uv, atlas_uv: c.atlas_uv, atlas: s.kind, color:c.color };
                        if c.clip == UNCLIPPED {
                            emit_glyph(glyph, scale, &mut draw, f);
                        } else {
                            emit_clipped(glyph, c.clip.map(|v| v.mul_add(scale, ox)), &mut |glyph| {
                                emit_glyph(glyph, scale, &mut draw, f);
                            });
                        }
                    }
                } else if let Some(slot) = g.slot(CacheKey { x_bin: bin, ..c.key }) {
                    // a bin the list does not hold, or a slot an eviction during this call may have moved
                    g.emit(slot, x, y, c.color, &mut |glyph|emit_clipped(glyph,c.clip.map(|v|v.mul_add(scale,ox)), &mut |glyph| {
                        emit_glyph(glyph, scale, &mut draw, f);
                    }));
                }
            }
            return;
        }
        *walks += 1;
        let start = g.evictions;
        let keep = *cache_on && e.cacheable && *cache_bytes < CACHE_BYTES;
        *cache_bytes -= e.glyphs.capacity() * size_of::<CachedGlyph>();
        e.glyphs.clear();
        e.shelves = [0; 8];
        e.cached = keep;
        let mut translates_x = (1.0..=16.0).contains(&scale) && scale.to_bits() & 0x007f_ffff == 0;
        let spans=&e.rich[..];
        let (mut line,mut base)=(0,0);
        for run in e.buf.layout_runs() {
            while line<run.line_i { base+=e.buf.lines[line].text().len()+e.buf.lines[line].ending().as_str().len(); line+=1; }
            for lg in run.glyphs {
                let p={
                    let ppem=lg.font_size*scale;
                    let (x,x_bin)=apple_x(ppem,(lg.x+lg.font_size*lg.x_offset).mul_add(scale,origin.0*scale));
                    let y=(lg.y-lg.font_size*lg.y_offset).mul_add(scale,(origin.1+run.line_y)*scale).round() as i32;
                    let cache_key=CacheKey { font_id:lg.font_id, glyph_id:lg.glyph_id, font_size_bits:ppem.to_bits(), x_bin, y_bin:SubpixelBin::Zero, font_weight:lg.font_weight, flags:lg.cache_key_flags };
                    cosmic_text::PhysicalGlyph { cache_key, x, y }
                };
                let Some(slot)=g.slot(p.cache_key) else { e.cached=false; continue; };
                glyph_parts(&run,lg,base,spans,&mut |clip,color| {
                    if keep {
                        let (x,y)=(lg.x+lg.font_size*lg.x_offset,lg.y-lg.font_size*lg.y_offset);
                        translates_x = translates_x && apple_x_translates(f32::from_bits(p.cache_key.font_size_bits), x * scale);
                        let uv = g.uv(&slot);
                        e.glyphs.push(CachedGlyph { key:p.cache_key,x,y,line_y:run.line_y,physical:(p.x,p.y),slot,uv,atlas_uv:packed_uv(uv),color,clip });
                        if slot.w>0 && slot.h>0 { let (word,bit)=shelf_bit(slot.kind,slot.shelf); e.shelves[word]|=bit; }
                    }
                    g.emit(slot,p.x,p.y,color,&mut |glyph|emit_clipped(glyph,clip.map(|v|v.mul_add(scale,origin.0*scale)), &mut |glyph| {
                        emit_glyph(glyph, scale, &mut draw, f);
                    }));
                });
            }
        }
        *cache_bytes += e.glyphs.capacity() * size_of::<CachedGlyph>();
        (e.cached_scale, e.cached_evictions) = (scale.to_bits(), start);
        e.cached_origin = (origin.0.to_bits(), origin.1.to_bits());
        e.cached_translates_x = translates_x;
    }

    /// Walks through cosmic-text's runs so far (a warm frame of unchanged text adds none).
    pub fn glyph_walks(&self) -> u64 {
        self.walks
    }

    /// Turns the per-layout glyph lists off (`glyphs` walks every time) or back on; for tests that
    /// compare the two paths.
    pub fn set_glyph_cache(&mut self, on: bool) {
        self.cache_on = on;
        for e in self.entries.values_mut() {
            e.cached = false;
        }
    }

    /// Hands out every texel rectangle written since the last call; `data` starts at the rectangle's
    /// first texel in the CPU mirror, `stride` bytes per row.
    pub fn drain_uploads(&mut self, f: &mut dyn FnMut(AtlasKind, u32, u32, u32, u32, &[u8], usize)) {
        for (kind, x, y, w, h) in self.g.dirty.drain(..) {
            let a = &self.g.atlases[idx(kind)];
            let stride = (a.w * a.bpp) as usize;
            let start = (y as usize) * stride + (x * a.bpp) as usize;
            let end = start + (h as usize - 1) * stride + (w * a.bpp) as usize;
            f(kind, x, y, w, h, &a.px[start..end], stride);
        }
    }

    /// The CPU mirror of an atlas, `atlas_size` texels, rows tightly packed (for CPU compositing).
    pub fn atlas_pixels(&mut self, kind: AtlasKind) -> &[u8] {
        self.g.atlases[idx(kind)].pixels()
    }

    pub fn atlas_size(&self, kind: AtlasKind) -> (u32, u32) {
        let a = &self.g.atlases[idx(kind)];
        (a.w, a.h)
    }

    /// The first pages exist at startup; second pages become active with their first CPU texels.
    pub fn atlas_active(&self,kind:AtlasKind)->bool { idx(kind)<2 || !self.g.atlases[idx(kind)].px.is_empty() }

    /// Heap held by the atlas mirrors and the layouts' glyph lists (`CACHE_BYTES` plus at most one
    /// list); cosmic-text's layout buffers are not counted.
    pub fn bytes(&self) -> usize {
        self.g.atlases.iter().map(|a| a.px.len()).sum::<usize>() + self.cache_bytes
    }

    /// The glyph lists' part of `bytes`.
    pub fn glyph_list_bytes(&self) -> usize {
        self.cache_bytes
    }

    /// Shelves evicted so far: glyph lists and slots made before an eviction are dropped.
    pub fn evictions(&self) -> u64 {
        self.g.evictions
    }

    /// Shelves retired so far: each dropped glyphs that output emitted at or after the pin may still
    /// show. Rebuild that output (the texels stay `FRAMES_IN_FLIGHT` frames for it).
    pub fn retirements(&self) -> u64 {
        self.g.retirements
    }

    /// Cumulative cache-miss layouts, including changed content/style/width on an existing key.
    pub fn layout_count(&self) -> u64 { self.layouts }

    /// Glyphs that found no atlas room so far (each is missing from its walk: repaint next frame).
    pub fn misses(&self) -> u64 {
        self.g.misses
    }

    /// The locale font fallback follows.
    pub fn locale(&self) -> &str {
        self.g.fs.locale()
    }

    /// The frame `end_frame` closes (glyphs `glyphs` emits now are marked with it).
    pub fn frame(&self) -> u32 {
        self.frame
    }

    /// The shelves glyphs were emitted from since the last `set_marks` (a caller that keeps output
    /// without emitting it again records these with it).
    #[inline]
    pub fn marks(&self) -> Shelves {
        self.g.marks
    }

    #[inline]
    pub fn set_marks(&mut self, marks: Shelves) {
        self.g.marks = marks;
    }

    /// Adds `shelves` to the marks (output kept from earlier frames that shows them).
    #[inline]
    pub fn add_marks(&mut self, shelves: &Shelves) {
        for k in 0..8 {
            self.g.marks[k] |= shelves[k];
        }
    }

    /// The shelves output shown without being emitted again samples (kept views): the atlas does not
    /// rewrite them (it retires one instead when nothing else has room). Empty by default: every
    /// frame emits all it shows.
    pub fn pin(&mut self, shelves: Shelves) {
        self.g.pinned = shelves;
    }

    /// A virtual list builds rows `first..last` (call each time it builds). Layouts asked for inside
    /// `list_row`..`list_end` are list-owned: while within `LIST_KEEP_ROWS` rows of the window they are
    /// never age-evicted, so frames that do not build keep them; farther ones are dropped first by a
    /// full cache, and by the sweep once a sweep pass (`SWEEP_FRAMES`) has gone by unused.
    /// The previous window of a jump gets a short return interval before the sweep drops its rows.
    /// Windows persist until the list builds again.
    /// Allocates: never.
    pub fn list_window(&mut self, list: u64, first: usize, last: usize) {
        let list = list.max(1);
        let first = first.min(u32::MAX as usize) as u32;
        let last = last.min(u32::MAX as usize) as u32;
        match self.windows.iter_mut().find(|window| window.list == list) {
            Some(window) => {
                if first.saturating_sub(window.last) > LIST_KEEP_ROWS || window.first.saturating_sub(last) > LIST_KEEP_ROWS {
                    window.departed = (window.first, window.last);
                }
                (window.first, window.last) = (first, last);
            }
            None => {
                // the first free slot, else drop the oldest (slot 0) and append, keeping build order
                let at = self.windows.iter().position(|window| window.list == 0).unwrap_or_else(|| { self.windows.rotate_left(1); LIST_WINDOWS - 1 });
                self.windows[at] = ListWindow { list, first, last, ..ListWindow::default() };
            }
        }
    }

    /// The layouts requested until `list_end` belong to row `row` of `list` (see `list_window`).
    pub fn list_row(&mut self, list: u64, row: usize) {
        self.scope = (list.max(1), row.min(u32::MAX as usize) as u32);
    }

    /// Ends `list_row`.
    pub fn list_end(&mut self) {
        self.scope = (0, 0);
    }

    /// A rebuilt view's paint starts: layouts drawn (`glyphs`) from now on belong to `view`, and are never
    /// age-evicted while it stays live (`view_freed`) and unpainted since; a view kept through a long
    /// idle keeps its text, so its next rebuild does not re-shape. Returns the enclosing view (0: none)
    /// for `paint_view_end`. Allocates: only when `view` is a new, higher number.
    // kept out of line: inlined into `Ui::visit` it cost every abi_bench page ~20 retired instructions a frame
    #[inline(never)]
    pub fn paint_view(&mut self, view: u32) -> u32 {
        match self.views.get_mut(view as usize) {
            Some(painted) => *painted = self.frame,
            None => self.grow_views(view),
        }
        std::mem::replace(&mut self.owner, view)
    }

    #[cold]
    fn grow_views(&mut self, view: u32) {
        self.views.resize(view as usize + 1, 0);
        self.views[view as usize] = self.frame;
    }

    /// The view `paint_view` started is painted: layouts belong to `enclosing` (its return) again. Allocates: never.
    #[inline]
    pub fn paint_view_end(&mut self, enclosing: u32) {
        self.owner = enclosing;
    }

    /// `view` is gone: its layouts fall back to age. Allocates: never.
    pub fn view_freed(&mut self, view: u32) {
        if let Some(painted) = self.views.get_mut(view as usize) {
            *painted = 0;
        }
    }

    /// The entry's view is live and not painted since it asked for the entry.
    #[inline]
    /// Takes an evicted layout out of the glyph-cache accounting and keeps its storage for reuse.
    fn retire(&mut self, mut e: Entry) {
        self.layout_bytes -= layout_cost(e.text.len());
        self.cache_bytes -= e.glyphs.capacity() * size_of::<CachedGlyph>();
        if self.spare.len() < SPARE_LAYOUTS && e.text.capacity() <= SPARE_TEXT_BYTES {
            e.glyphs.clear();
            self.spare.push(e);
        }
    }

    fn view_owned(&self, e: &Entry) -> bool {
        e.view != 0 && self.views.get(e.view as usize) == Some(&e.frame)
    }

    /// Rows between an entry's row and its list's window (0 inside it); `None` when not list-owned or its list is untracked.
    fn list_distance(&self, e: &Entry) -> Option<u32> {
        let w = self.list_tracking(e)?;
        Some(w.distance(e.row))
    }

    fn list_tracking(&self, e: &Entry) -> Option<&ListWindow> {
        if e.list == 0 { return None; }
        self.windows.iter().find(|w| w.list == e.list)
    }

    /// The index in `keys` to evict from a full cache: of the next 64 entries from the sweep, the
    /// farthest list row beyond its keep window, else the oldest one not owned by a list window or a live view, else the sweep's own.
    /// How readily layout `e` goes: higher is evicted first (far list rows, then the longest unused; kept rows and live views never rank above 0).
    fn rank(&self, e: &Entry) -> u64 {
        match self.list_distance(e) {
            Some(d) if d > LIST_KEEP_ROWS => 1 << 40 | d as u64,
            Some(_) => 0,
            None if self.view_owned(e) => 0,
            None => 1 << 20 | self.frame.wrapping_sub(e.frame) as u64,
        }
    }

    fn victim(&mut self) -> usize {
        let n = self.keys.len();
        self.sweep %= n;
        let (mut best, mut best_rank) = (self.sweep, 0u64);
        for step in 0..n.min(64) {
            let at = (self.sweep + step) % n;
            let Some(e) = self.entries.get(&self.keys[at]) else { return at };
            let rank = self.rank(e);
            if rank > best_rank { (best, best_rank) = (at, rank); }
        }
        best
    }

    /// Call once per frame: drops layouts not laid out in the last `KEEP_FRAMES` frames, unless a
    /// list window (`list_window`) or a live, unrepainted view (`paint_view`) owns them. A kept view's
    /// text is not laid out while the view is kept; its owned layouts outlive any keep, so the
    /// view's next rebuild is a cache hit, not a re-shape. The sweep is incremental: each frame looks
    /// at 1/`SWEEP_FRAMES` of the remaining layouts plus one, so no frame walks them all.
    /// Removing entries reduces the following frame's budget. With the prepare queue running it first flushes the frame's
    /// requests to the worker (`queue.rs`).
    pub fn end_frame(&mut self) {
        self.flush_queue();
        let frame = self.frame;
        self.g.frame = frame + 1;
        let mut budget = self.keys.len() / SWEEP_FRAMES as usize + 1;
        while budget > 0 && !self.keys.is_empty() {
            budget -= 1;
            if self.sweep >= self.keys.len() {
                self.sweep = 0;
            }
            let key = self.keys[self.sweep];
            let stale = self.entries.get(&key).is_none_or(|e| {
                let age = frame.wrapping_sub(e.frame);
                if let Some(w) = self.list_tracking(e) {
                    let distance = w.distance(e.row);
                    let (first, last) = w.departed;
                    let jumped_from = first < last && (first.saturating_sub(LIST_KEEP_ROWS)..last.saturating_add(LIST_KEEP_ROWS)).contains(&e.row);
                    distance > LIST_KEEP_ROWS && age >= if jumped_from { JUMP_KEEP_FRAMES } else { SWEEP_FRAMES }
                } else {
                    age >= KEEP_FRAMES && !self.view_owned(e)
                }
            });
            if !stale {
                self.sweep += 1;
                continue;
            }
            if let Some(e) = self.entries.remove(&key) {
                self.retire(e);
            }
            self.keys.swap_remove(self.sweep);
        }
        self.frame += 1;
    }
}

#[cfg(test)]
mod layout_cache_tests {
    use super::*;

    #[test]
    fn fonts_are_capped_and_unparseable_ones_refused() {
        let mut text = Text::new(&[PRAGMASEVKA]);
        assert_eq!(text.add_font(&[]), None);
        assert_eq!(text.add_font(b"not a font"), None);
        assert_eq!(text.add_font(&vec![0; MAX_FONT_BYTES + 1]), None);
        assert_eq!(text.font_count(), 1);
        for _ in 1..MAX_FONTS { assert!(text.add_font(PRAGMASEVKA).is_some()); }
        assert_eq!(text.add_font(PRAGMASEVKA), None, "the {MAX_FONTS}th font is the last");
        assert_eq!(text.font_count(), MAX_FONTS);
        let mut text = Text::new(&[]);
        text.font_bytes = MAX_FONT_TOTAL_BYTES - 1;
        assert_eq!(text.add_font(PRAGMASEVKA), None, "past the total budget");
    }

    #[test]
    fn many_large_layouts_stay_cached_across_warm_frames_within_the_budget() {
        let mut text = Text::new(&[PRAGMASEVKA]);
        let big = "a".repeat(20 << 10);
        let style = Style::default();
        let count = 6u64;
        assert!(count as usize * layout_cost(big.len()) < LAYOUT_BYTES);
        for key in 1..=count { text.layout(key, &big, &style, 0.0); }
        text.end_frame();
        let shaped = text.layout_count();
        mem_sys::local_reset();
        for _ in 0..5 {
            for key in 1..=count { text.layout(key, &big, &style, 0.0); }
            text.end_frame();
        }
        assert_eq!(mem_sys::local_alloc_count(), 0);
        assert_eq!(text.layout_count(), shaped, "no layout reshaped");
        assert_eq!(text.entries.len(), count as usize);
        assert!(text.layout_bytes <= LAYOUT_BYTES);
    }

    #[test]
    fn the_layout_budget_evicts_unused_layouts_but_never_one_used_this_frame() {
        let mut text = Text::new(&[PRAGMASEVKA]);
        let big = "a".repeat(LAYOUT_BYTES / SHAPED_BYTES_PER_TEXT_BYTE / 3);
        let style = Style::default();
        for key in 1..=2 { text.layout(key, &big, &style, 0.0); }
        text.end_frame();
        for key in 3..=4 { text.layout(key, &big, &style, 0.0); }
        // 4 x a third of the budget: the two used this frame stay, an older one goes
        assert!(text.entries.contains_key(&3) && text.entries.contains_key(&4));
        assert!(text.layout_bytes <= LAYOUT_BYTES + layout_cost(0) * 4);
        assert_eq!(text.entries.len(), text.keys.len());
        let sum: usize = text.entries.values().map(|e| layout_cost(e.text.len())).sum();
        assert_eq!(sum, text.layout_bytes);
    }

    #[test]
    fn standalone_layout_cache_stays_bounded_without_end_frame() {
        let mut text = Text::new(&[PRAGMASEVKA]);
        text.layout(1, "a", &Style::default(), 0.0);
        text.glyphs(1, (0.0, 0.0), 1.0, &mut |_| {});
        assert!(text.cache_bytes > 0);
        let held = |text: &Text| text.entries.values().map(|e| e.glyphs.capacity() * size_of::<CachedGlyph>()).sum::<usize>();
        for key in 2..=MAX_LAYOUTS as u64 + 8 {
            text.layout(key, "", &Style::default(), 0.0);
        }
        // evicted layouts' glyph storage waits in the pool and is counted only once a layout holds it again
        assert_eq!(text.cache_bytes, held(&text));
        assert_eq!(text.entries.len(), MAX_LAYOUTS);
        assert_eq!(text.keys.len(), MAX_LAYOUTS);
        for key in MAX_LAYOUTS as u64..=MAX_LAYOUTS as u64 + 8 {
            assert!(text.entries.contains_key(&key));
        }
        for _ in 0..KEEP_FRAMES + MAX_LAYOUTS as u32 {
            text.end_frame();
        }
        assert!(text.entries.is_empty());
        assert!(text.keys.is_empty());
        assert_eq!(text.cache_bytes, 0);
    }

    /// A label that is new every frame (a fresh key and text) lays out into evicted layouts' storage:
    /// once the sweep has returned some, frames allocate nothing, and the pool stays bounded.
    #[test]
    fn fresh_layouts_reuse_evicted_storage_without_warm_allocations() {
        let labels: Vec<String> = (0..4096).map(|i| format!("Track {i:04} - {}", i * 7919 % 10_000)).collect();
        let style = Style::default();
        let mut text = Text::new(&[PRAGMASEVKA]);
        let frame = |text: &mut Text, i: usize| {
            text.layout(i as u64 + 1, &labels[i], &style, 0.0);
            text.glyphs(i as u64 + 1, (0.0, 0.0), 1.0, &mut |_| {});
            text.end_frame();
        };
        for i in 0..1024 {
            frame(&mut text, i);
        }
        assert!(!text.spare.is_empty() || text.entries.len() < 64);
        let scope = mem_sys::warm_scope();
        for i in 1024..4096 {
            frame(&mut text, i);
        }
        drop(scope);
        // shaping a string the engine has never seen allocates inside cosmic-text (the named fling
        // exception); everything the engine itself allocates for a fresh layout must be reused storage
        let reports = mem_sys::take_reports();
        let ours: Vec<_> = reports.iter().filter(|r| !["cosmic_text", "unicode_bidi", "harfrust", "swash"].iter().any(|lib| r.contains(lib))).collect();
        assert!(!reports.is_empty(), "the shaping exception disappeared: tighten this test");
        assert!(ours.is_empty(), "{} of {} warm allocations are not the shaping library's, first:\n{}", ours.len(), reports.len(), ours[0]);
        assert!(text.spare.len() <= SPARE_LAYOUTS && text.spare.capacity() <= 2 * SPARE_LAYOUTS);
        assert!(text.entries.len() < 2 * KEEP_FRAMES as usize + 8);
    }

    fn row(text: &mut Text, row: usize) {
        text.list_row(7, row);
        text.layout(1000 + row as u64, "row", &Style::default(), 0.0);
        text.list_end();
    }

    #[test]
    fn list_rows_outlive_idle_age_and_far_rows_go_first() {
        let mut text = Text::new(&[PRAGMASEVKA]);
        text.layout(1, "plain", &Style::default(), 0.0);
        text.list_window(7, 0, 20);
        (0..20).for_each(|r| row(&mut text, r));
        // frames that build nothing: the plain layout ages out, the visible rows do not
        for _ in 0..3 * KEEP_FRAMES {
            text.end_frame();
        }
        assert!(!text.entries.contains_key(&1));
        assert!((0..20).all(|r| text.entries.contains_key(&(1000 + r))));
        // scrolled far away: the old rows are beyond the keep window, dropped regardless of age
        text.list_window(7, 5000, 5020);
        for _ in 0..4 * SWEEP_FRAMES {
            text.end_frame();
        }
        assert!((0..20).all(|r| !text.entries.contains_key(&(1000 + r))));
    }

    #[test]
    fn apple_x_floors_to_its_ppems_fraction_on_both_sides_of_zero() {
        for (ppem, n) in [(11.0, 4), (11.25, 3), (13.0, 3), (16.5, 3), (16.75, 2), (20.0, 2), (30.0, 2), (33.0, 2), (33.5, 1), (40.0, 1)] {
            for k in -40..40 {
                let pos = k as f32 * 0.1;
                let m = (pos * n as f32).floor() as i32;
                let bin = match (n, m.rem_euclid(n)) {
                    (_, 0) => SubpixelBin::Zero,
                    (4 | 3, 1) => SubpixelBin::One,
                    (4, 2) | (2, 1) => SubpixelBin::Two,
                    _ => SubpixelBin::Three,
                };
                assert_eq!(apple_x(ppem, pos), (m.div_euclid(n), bin), "ppem {ppem} at {pos}");
            }
        }
    }

    #[test]
    fn a_far_rows_shared_text_waits_a_sweep_pass_for_the_next_row_that_shows_it() {
        let mut text = Text::new(&[PRAGMASEVKA]);
        text.list_window(7, 200, 220);
        row(&mut text, 217);
        // the window moves past row 217; row 317 shows the same layout (one content key) 40 frames on
        text.list_window(7, 300, 320);
        for _ in 0..40 {
            text.end_frame();
        }
        assert!(text.entries.contains_key(&1217));
        // nothing asks for it again: the sweep drops the far row
        for _ in 0..4 * SWEEP_FRAMES {
            text.end_frame();
        }
        assert!(!text.entries.contains_key(&1217));
    }

    #[test]
    fn revisiting_the_previous_jump_window_does_not_shape_again() {
        let mut text = Text::new(&[PRAGMASEVKA]);
        for first in [0, 500] {
            text.list_window(7, first, first + 20);
            row(&mut text, first);
        }
        let layouts = text.layouts;
        for first in [0, 500, 0] {
            for _ in 0..120 { text.end_frame(); }
            text.list_window(7, first, first + 20);
            mem_sys::local_reset();
            row(&mut text, first);
            assert_eq!(text.layouts, layouts);
            assert_eq!(mem_sys::local_alloc_count(), 0);
        }
    }

    #[test]
    fn a_kept_views_layouts_outlive_idle_age_until_it_rebuilds_or_goes() {
        let mut text = Text::new(&[PRAGMASEVKA]);
        for (key, view) in [(1, 3), (2, 4)] {
            let outer = text.paint_view(view);
            text.layout(key, "Songs", &Style::default(), 0.0);
            text.glyphs(key, (0.0, 0.0), 1.0, &mut |_| {});
            text.paint_view_end(outer);
        }
        text.layout(5, "plain", &Style::default(), 0.0);
        for _ in 0..3 * KEEP_FRAMES {
            text.end_frame();
        }
        assert!(text.entries.contains_key(&1) && text.entries.contains_key(&2), "kept views keep their text");
        assert!(!text.entries.contains_key(&5));
        // view 3 rebuilt without this text, view 4 freed: both fall back to age
        let outer = text.paint_view(3);
        text.paint_view_end(outer);
        text.view_freed(4);
        for _ in 0..2 * KEEP_FRAMES {
            text.end_frame();
        }
        assert!(text.entries.is_empty());
    }

    #[test]
    fn a_long_scroll_stays_bounded_and_keeps_the_window() {
        let mut text = Text::new(&[PRAGMASEVKA]);
        for top in (0..20_000).step_by(3) {
            text.list_window(7, top, top + 20);
            for r in top..top + 20 {
                row(&mut text, r);
            }
            text.end_frame();
            assert!(text.entries.len() <= MAX_LAYOUTS);
        }
        // the last window's rows are all still cached
        assert!((19_998..20_018).all(|r| text.entries.contains_key(&(1000 + r as u64))));
        assert!(text.entries.len() < 4 * LIST_KEEP_ROWS as usize + 200, "{} layouts after a scroll", text.entries.len());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn dilate_never_fills_empty_pixels_away_from_ink() {
        // one inked pixel in a 5x5 box at a large size (d = 0.6, 2d > 1)
        let mut src = [0u8; 25];
        src[12] = 255;
        let mut out = Vec::new();
        let (_, _, w, h) = super::dilate(&src, 5, 5, 0.6, &mut out);
        assert!(w <= 3 && h <= 3, "grew to {w}x{h}");
    }
}
