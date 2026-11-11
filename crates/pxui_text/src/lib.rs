//! Text for PXUI: cosmic-text shaping/bidi/wrapping/fallback behind a small concrete API, a layout
//! cache keyed by a caller-supplied node key, and two glyph atlases (R8 mask, RGBA8 colour) that
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
//! `max_lines` (0 = unlimited) clips; with `ellipsis` the last visible line ends in an ellipsis
//! (one line when `max_lines` is 0). Unchanged layouts and glyph iteration of already
//! rasterised glyphs perform zero heap allocations (tests/text.rs). Changed text allocates inside
//! cosmic-text.
//!
//! Build cost (cosmic-text 0.19.0, M1, `nice -n 10` while the machine was in use): first cold debug
//! build of the dependency closure 24.5 s wall (116 s CPU); 40 external crates + pxui_text.

use cosmic_text::{
    Align as CAlign, Attrs, Buffer, CacheKey, Cursor, Ellipsize, EllipsizeHeightLimit, Family, FontSystem, Metrics as CMetrics, Shaping,
    Style as CStyle, SubpixelBin, SwashCache, SwashContent, Weight, Wrap as CWrap, fontdb,
};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Arc;

/// A multiply-rotate hasher for the engine's internal keys (glyph cache keys, node and layout keys):
/// std's SipHash was a fifth of a full rebuild. Not DoS-resistant; no key here comes from outside.
#[derive(Default, Clone, Copy)]
pub struct FastHasher(u64);
impl Hasher for FastHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            self.write_u64(u64::from_le_bytes(c.try_into().unwrap()));
        }
        let mut tail = [0u8; 8];
        tail[..chunks.remainder().len()].copy_from_slice(chunks.remainder());
        self.write_u64(u64::from_le_bytes(tail) ^ (bytes.len() as u64) << 56);
    }
    fn write_u8(&mut self, n: u8) {
        self.write_u64(n as u64);
    }
    fn write_u16(&mut self, n: u16) {
        self.write_u64(n as u64);
    }
    fn write_u32(&mut self, n: u32) {
        self.write_u64(n as u64);
    }
    fn write_u64(&mut self, n: u64) {
        // the final multiply spreads the low bits into the high bits hashbrown's control bytes read
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_usize(&mut self, n: usize) {
        self.write_u64(n as u64);
    }
}
/// `HashMap`'s hasher parameter for `FastHasher`.
pub type Fast = BuildHasherDefault<FastHasher>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wrap {
    None,
    Word,
    Glyph,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AtlasKind {
    Mask,
    Color,
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
    pub ellipsis: bool,
    /// Points added after every glyph (CSS `letter-spacing`).
    pub tracking: f32,
}
impl Default for Style {
    fn default() -> Style {
        Style { font: 0, size: 14.0, weight: 400, italic: false, line_height: 0.0, wrap: Wrap::None, align: Align::Left, max_lines: 0, ellipsis: false, tracking: 0.0 }
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
    pub atlas: AtlasKind,
}

/// Frames a layout outlives its last use (`Text::end_frame`), and how often the unused are swept: a
/// layout goes between `KEEP_FRAMES` and `KEEP_FRAMES + SWEEP_FRAMES` frames after its last use.
pub const KEEP_FRAMES: u32 = 600;
pub const SWEEP_FRAMES: u32 = 64;

/// The bundled UI face (the PXUI default theme's font).
pub const PRAGMASEVKA: &[u8] = include_bytes!("../../../assets/fonts/Pragmasevka-Regular.ttf");

const MASK_SIZE: (u32, u32) = (1024, 2048);
const COLOR_SIZE: (u32, u32) = (1024, 1024);

/// Frames whose GPU work may still sample the atlas after the CPU writes it: the painter keeps
/// `pxui_render`'s `FRAMES` (3) ring regions, so texels last drawn at frame `f` are free at `f + 3`.
pub const FRAMES_IN_FLIGHT: u32 = 3;

/// A set of atlas shelves, one bit per shelf of either atlas (`shelf_bit`; shelves past 256 share
/// bits, which only keeps more shelves).
pub type Shelves = [u64; 4];

fn shelf_bit(kind: AtlasKind, shelf: u16) -> (usize, u64) {
    let bit = (shelf as usize * 2 + idx(kind)) % 256;
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
        // a shelf is at least 2 texels tall: room for every shelf there can be, so no frame allocates
        Atlas { w, h, bpp, px: vec![0; (w * h * bpp) as usize], shelves: Vec::with_capacity(h as usize / 2 + 1), next_y: 0 }
    }

    /// Room for a `w`x`h` glyph (one pixel of padding on the right and bottom), shelf-LRU. A shelf
    /// is rewritten only when nothing can still sample it: not drawn in the last `FRAMES_IN_FLIGHT`
    /// frames and not in `pinned` (kept output shows glyphs on it), or retired at least
    /// `FRAMES_IN_FLIGHT` frames ago. With no such shelf the least recently used one no frame
    /// in flight drew is retired (`Alloc::Retired`): its glyphs go, its texels stay until the frames
    /// showing them are done, and the caller rebuilds what showed them.
    fn alloc(&mut self, kind: AtlasKind, w: u32, h: u32, frame: u32, pinned: &Shelves) -> Alloc {
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

/// A glyph of a layout as `glyphs` replays it: its cache key (with the subpixel bin it had), its
/// position before `physical` (`x + x_offset`, `y - y_offset`, its run's `line_y`) and its slot.
#[derive(Clone, Copy)]
struct CachedGlyph {
    key: CacheKey,
    x: f32,
    y: f32,
    line_y: f32,
    slot: Slot,
    uv: Rect,
}

/// Layouts stop keeping glyph lists past this many bytes in all (a list goes with its layout).
pub const CACHE_BYTES: usize = 4 << 20;

/// The parts of `Text` a glyph emission touches (fonts, rasteriser, atlases and their slots), a field
/// of their own so `glyphs` borrows them beside a layout entry.
struct Glyphs {
    fs: FontSystem,
    swash: SwashCache,
    atlases: [Atlas; 2],
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
        let kind = match img.content {
            SwashContent::Mask => AtlasKind::Mask,
            SwashContent::Color => AtlasKind::Color,
            SwashContent::SubpixelMask => return Some(empty),
        };
        let a = &mut self.atlases[idx(kind)];
        let bpp = a.bpp;
        let mut s = Slot { kind, shelf: 0, generation: 0, x: 0, y: 0, w, h, left: img.placement.left, top: img.placement.top };
        if w > 0 && h > 0 {
            if img.data.len() != (w * h * bpp) as usize {
                return Some(empty);
            }
            let (x, y, shelf, evicted) = match a.alloc(kind, w, h, self.frame, &self.pinned) {
                Alloc::At(x, y, shelf, evicted) => (x, y, shelf, evicted),
                Alloc::Retired => {
                    (self.evictions, self.retirements, self.misses) = (self.evictions + 1, self.retirements + 1, self.misses + 1);
                    let shelves = &a.shelves;
                    self.slots.retain(|_, o| o.kind != kind || o.w == 0 || o.h == 0 || shelves[o.shelf as usize].generation == o.generation);
                    return None;
                }
                Alloc::Full => {
                    self.misses += 1;
                    return None;
                }
            };
            for row in 0..h {
                let src = (row * w * bpp) as usize;
                let dst = (((y + row) * a.w + x) * bpp) as usize;
                a.px[dst..dst + (w * bpp) as usize].copy_from_slice(&img.data[src..src + (w * bpp) as usize]);
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
    fn emit(&mut self, slot: Slot, x: i32, y: i32, f: &mut dyn FnMut(Glyph)) {
        if slot.w == 0 || slot.h == 0 {
            return;
        }
        self.atlases[idx(slot.kind)].shelves[slot.shelf as usize].used = self.frame;
        let (word, bit) = shelf_bit(slot.kind, slot.shelf);
        self.marks[word] |= bit;
        f(Glyph {
            rect: Rect { x: (x + slot.left) as f32, y: (y - slot.top) as f32, w: slot.w as f32, h: slot.h as f32 },
            uv: self.uv(&slot),
            atlas: slot.kind,
        });
    }

    /// The slot's texel rectangle normalised by its atlas's size.
    fn uv(&self, slot: &Slot) -> Rect {
        let a = &self.atlases[idx(slot.kind)];
        let (aw, ah) = (a.w as f32, a.h as f32);
        Rect { x: slot.x as f32 / aw, y: slot.y as f32 / ah, w: slot.w as f32 / aw, h: slot.h as f32 / ah }
    }
}

/// `cosmic_text::SubpixelBin::new`, inlined: the replay must bin exactly as `physical` does.
#[inline]
fn subpixel_bin(pos: f32) -> (i32, SubpixelBin) {
    let trunc = pos as i32;
    let fract = pos - trunc as f32;
    if pos.is_sign_negative() {
        match fract {
            f if f > -0.125 => (trunc, SubpixelBin::Zero),
            f if f > -0.375 => (trunc - 1, SubpixelBin::Three),
            f if f > -0.625 => (trunc - 1, SubpixelBin::Two),
            f if f > -0.875 => (trunc - 1, SubpixelBin::One),
            _ => (trunc - 1, SubpixelBin::Zero),
        }
    } else {
        match fract {
            f if f < 0.125 => (trunc, SubpixelBin::Zero),
            f if f < 0.375 => (trunc, SubpixelBin::One),
            f if f < 0.625 => (trunc, SubpixelBin::Two),
            f if f < 0.875 => (trunc, SubpixelBin::Three),
            _ => (trunc + 1, SubpixelBin::Zero),
        }
    }
}

struct Entry {
    buf: Buffer,
    text: String,
    style: Style,
    width: u32,
    frame: u32,
    m: Metrics,
    /// The glyph list `glyphs` replays: valid while `cached`, for `cached_scale` (f32 bits), until
    /// the atlas evicts past `cached_evictions`.
    glyphs: Vec<CachedGlyph>,
    cached: bool,
    cached_scale: u32,
    cached_evictions: u64,
    /// the shelves the list's glyphs sit on
    shelves: Shelves,
}

pub struct Text {
    g: Glyphs,
    names: Vec<String>,
    ascents: Vec<f32>,
    entries: HashMap<u64, Entry, Fast>,
    frame: u32,
    walks: u64,
    cache_on: bool,
    /// Capacity of every layout's glyph list, in bytes.
    cache_bytes: usize,
}

fn idx(kind: AtlasKind) -> usize {
    kind as usize
}

impl Text {
    pub fn new(fonts: &[&[u8]]) -> Text {
        Text::new_sized(fonts, MASK_SIZE, COLOR_SIZE)
    }

    /// `new` with a `mask` (R8) and `color` (RGBA8) atlas of these sizes in texels.
    pub fn new_sized(fonts: &[&[u8]], mask: (u32, u32), color: (u32, u32)) -> Text {
        let mut t = Text {
            // not new_with_fonts: that calls load_system_fonts. ponytail: fixed locale; pass one in when per-locale fallback matters.
            g: Glyphs {
                fs: FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new()),
                swash: SwashCache::new(),
                atlases: [Atlas::new(mask, 1), Atlas::new(color, 4)],
                slots: HashMap::default(),
                dirty: Vec::new(),
                evictions: 0,
                retirements: 0,
                misses: 0,
                frame: 1,
                marks: [0; 4],
                pinned: [0; 4],
            },
            names: Vec::new(),
            ascents: Vec::new(),
            entries: HashMap::default(),
            frame: 1,
            walks: 0,
            cache_on: true,
            cache_bytes: 0,
        };
        for f in fonts {
            t.add_font(f);
        }
        t
    }

    /// Loads a font (copies the bytes); returns its index for `Style::font`. Existing layouts keep
    /// their old shaping until their text, width or style changes.
    pub fn add_font(&mut self, bytes: &[u8]) -> usize {
        let ids = self.g.fs.db_mut().load_font_source(fontdb::Source::Binary(Arc::new(bytes.to_vec())));
        let name = ids.first().and_then(|id| self.g.fs.db().face(*id)).and_then(|f| f.families.first()).map(|f| f.0.clone()).unwrap_or_default();
        let ascent = ids.first().and_then(|id| self.g.fs.get_font(*id, Weight::NORMAL)).map_or(0.0, |f| {
            let m = f.as_swash().metrics(&[]);
            m.ascent / m.units_per_em as f32
        });
        self.names.push(name);
        self.ascents.push(ascent);
        self.names.len() - 1
    }

    /// The font's ascent as a fraction of its size (0 for an unknown index).
    pub fn ascent(&self, font: u16) -> f32 {
        self.ascents.get(font as usize).copied().unwrap_or(0.0)
    }

    pub fn layout(&mut self, key: u64, text: &str, style: &Style, max_width: f32) -> Metrics {
        let width = if max_width.is_finite() && max_width > 0.0 { max_width } else { f32::INFINITY };
        let frame = self.frame;
        if let Some(e) = self.entries.get_mut(&key) {
            e.frame = frame;
            if e.width == width.to_bits() && e.style == *style && e.text == text {
                return e.m;
            }
        }
        let lh = if style.line_height > 0.0 { style.line_height } else { style.size * 1.2 };
        let e = self.entries.entry(key).or_insert_with(|| Entry {
            buf: Buffer::new_empty(CMetrics::new(style.size, lh)),
            text: String::new(),
            style: *style,
            width: 0,
            frame,
            m: Metrics::default(),
            glyphs: Vec::new(),
            cached: false,
            cached_scale: 0,
            cached_evictions: 0,
            shelves: [0; 4],
        });
        e.frame = frame;
        e.cached = false;
        e.style = *style;
        e.width = width.to_bits();
        e.text.clear();
        e.text.push_str(text);
        let family = match self.names.get(style.font as usize) {
            Some(n) => Family::Name(n),
            None => Family::SansSerif,
        };
        let attrs = Attrs::new().family(family).weight(Weight(style.weight)).style(if style.italic { CStyle::Italic } else { CStyle::Normal });
        let attrs = if style.tracking != 0.0 { attrs.letter_spacing(style.tracking / style.size) } else { attrs };
        let b = &mut e.buf;
        b.set_metrics(CMetrics::new(style.size, lh));
        b.set_wrap(match style.wrap {
            Wrap::None => CWrap::None,
            Wrap::Word => CWrap::WordOrGlyph,
            Wrap::Glyph => CWrap::Glyph,
        });
        let lines = style.max_lines as usize;
        b.set_ellipsize(if style.ellipsis { Ellipsize::End(EllipsizeHeightLimit::Lines(lines.max(1))) } else { Ellipsize::None });
        let height = if lines > 0 && !style.ellipsis { Some(lines as f32 * lh) } else { None };
        b.set_size(width.is_finite().then_some(width), height);
        let align = match style.align {
            Align::Left => CAlign::Left,
            Align::Center => CAlign::Center,
            Align::Right => CAlign::Right,
        };
        b.set_text(text, &attrs, Shaping::Advanced, Some(align));
        b.shape_until_scroll(&mut self.g.fs, true);
        let mut m = Metrics::default();
        for run in b.layout_runs() {
            if m.lines == 0 {
                m.baseline = run.line_y - run.line_top;
            }
            m.lines += 1;
            m.w = m.w.max(run.line_w);
            m.h = run.line_top + run.line_height;
        }
        e.m = m;
        m
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
            for (x, w) in run.highlight(ca, cb) {
                f(Rect { x, y: run.line_top, w, h: run.line_height });
            }
        }
    }

    /// Emits the layout's glyph quads at `origin` (points) for `scale`. The first walk through
    /// cosmic-text's runs keeps each glyph's line-relative position, subpixel bin and atlas slot in the
    /// layout's entry; later calls at any origin replay that list with the same float operations as
    /// `LayoutGlyph::physical`, so the quads equal a walk's. A glyph whose bin differs at the new
    /// origin (a fractional origin) looks its slot up as a walk would. The list is rebuilt when the
    /// layout is re-shaped, the scale changes or the atlas evicted a shelf.
    pub fn glyphs(&mut self, key: u64, origin: (f32, f32), scale: f32, f: &mut dyn FnMut(Glyph)) {
        let Text { g, entries, walks, cache_on, cache_bytes, .. } = self;
        let Some(e) = entries.get_mut(&key) else { return };
        if e.cached && e.cached_scale == scale.to_bits() && e.cached_evictions == g.evictions {
            for k in 0..4 {
                g.marks[k] |= e.shelves[k];
            }
            let (ox, start) = (origin.0 * scale, g.evictions);
            for c in &e.glyphs {
                let (x, bin) = subpixel_bin(c.x.mul_add(scale, ox));
                // y is whole after `trunc`: its bin is zero and its pixel the truncation
                let y = c.y.mul_add(scale, (origin.1 + c.line_y) * scale).trunc() as i32;
                if bin == c.key.x_bin && g.evictions == start {
                    if c.slot.w > 0 && c.slot.h > 0 {
                        g.atlases[idx(c.slot.kind)].shelves[c.slot.shelf as usize].used = g.frame;
                        let s = &c.slot;
                        f(Glyph { rect: Rect { x: (x + s.left) as f32, y: (y - s.top) as f32, w: s.w as f32, h: s.h as f32 }, uv: c.uv, atlas: s.kind });
                    }
                } else if let Some(slot) = g.slot(CacheKey { x_bin: bin, ..c.key }) {
                    // a bin the list does not hold, or a slot an eviction during this call may have moved
                    g.emit(slot, x, y, f);
                }
            }
            return;
        }
        *walks += 1;
        let start = g.evictions;
        let keep = *cache_on && *cache_bytes < CACHE_BYTES;
        *cache_bytes -= e.glyphs.capacity() * size_of::<CachedGlyph>();
        e.glyphs.clear();
        e.shelves = [0; 4];
        e.cached = keep;
        for run in e.buf.layout_runs() {
            for lg in run.glyphs {
                let p = lg.physical((origin.0 * scale, (origin.1 + run.line_y) * scale), scale);
                let Some(slot) = g.slot(p.cache_key) else {
                    e.cached = false; // the atlas has no room this frame: walk again next time
                    continue;
                };
                if keep {
                    let (x, y) = (lg.x + lg.font_size * lg.x_offset, lg.y - lg.font_size * lg.y_offset);
                    e.glyphs.push(CachedGlyph { key: p.cache_key, x, y, line_y: run.line_y, slot, uv: g.uv(&slot) });
                    if slot.w > 0 && slot.h > 0 {
                        let (word, bit) = shelf_bit(slot.kind, slot.shelf);
                        e.shelves[word] |= bit;
                    }
                }
                g.emit(slot, p.x, p.y, f);
            }
        }
        *cache_bytes += e.glyphs.capacity() * size_of::<CachedGlyph>();
        (e.cached_scale, e.cached_evictions) = (scale.to_bits(), start);
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
    pub fn atlas_pixels(&self, kind: AtlasKind) -> &[u8] {
        &self.g.atlases[idx(kind)].px
    }

    pub fn atlas_size(&self, kind: AtlasKind) -> (u32, u32) {
        let a = &self.g.atlases[idx(kind)];
        (a.w, a.h)
    }

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

    /// Glyphs that found no atlas room so far (each is missing from its walk: repaint next frame).
    pub fn misses(&self) -> u64 {
        self.g.misses
    }

    /// The frame `end_frame` closes (glyphs `glyphs` emits now are marked with it).
    pub fn frame(&self) -> u32 {
        self.frame
    }

    /// The shelves glyphs were emitted from since the last `set_marks` (a caller that keeps output
    /// without emitting it again records these with it).
    pub fn marks(&self) -> Shelves {
        self.g.marks
    }

    pub fn set_marks(&mut self, marks: Shelves) {
        self.g.marks = marks;
    }

    /// The shelves output shown without being emitted again samples (kept views): the atlas does not
    /// rewrite them (it retires one instead when nothing else has room). Empty by default: every
    /// frame emits all it shows.
    pub fn pin(&mut self, shelves: Shelves) {
        self.g.pinned = shelves;
    }

    /// Call once per frame: drops layouts not laid out in the last `KEEP_FRAMES` frames. A cached
    /// view's text is not laid out while the view is kept; its layouts outlive a short keep, so the
    /// view's next rebuild is a cache hit, not a re-shape.
    // ponytail: age-based, not size-based; a view kept past KEEP_FRAMES frames re-shapes its text (and
    // allocates) on its next rebuild. An LRU with a byte budget if that shows.
    // ponytail: one frame in SWEEP_FRAMES walks every layout (about 5 ns each); an intrusive LRU list
    // if that spike shows.
    pub fn end_frame(&mut self) {
        let frame = self.frame;
        self.g.frame = frame + 1;
        if frame % SWEEP_FRAMES == 0 {
            let bytes = &mut self.cache_bytes;
            self.entries.retain(|_, e| {
                let keep = frame.wrapping_sub(e.frame) < KEEP_FRAMES;
                if !keep {
                    *bytes -= e.glyphs.capacity() * size_of::<CachedGlyph>();
                }
                keep
            });
        }
        self.frame += 1;
    }
}
