//! PXUI's own plain f32 geometry, colour and key hash: the engine depends on nothing for them.
//! Points and sizes are logical points; colour channels are bytes, as in the Rays colour.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Point {
    /// Horizontal position in logical points.
    pub x: f32,
    /// Vertical position in logical points, growing downward.
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Rect {
    /// Left edge in logical points.
    pub x: f32,
    /// Top edge in logical points.
    pub y: f32,
    /// Width in logical points.
    pub w: f32,
    /// Height in logical points.
    pub h: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Edges {
    /// Left (start) inset in logical points.
    pub l: f32,
    /// Top inset in logical points.
    pub t: f32,
    /// Right (end) inset in logical points.
    pub r: f32,
    /// Bottom inset in logical points.
    pub b: f32,
}

/// A colour: four channel bytes, not premultiplied, or a colour token (`Rgba::token`) the engine resolves where a look is applied.
///
/// The token encoding is in-band and final (ABI 1.61): a colour with alpha 0, red 0x54 and green 0x4b names token `b`. Such a colour
/// therefore never paints as the clear colour its bytes spell where a look is applied; every other colour means its channels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Rgba {
    /// Red, 0 to 255.
    pub r: u8,
    /// Green, 0 to 255.
    pub g: u8,
    /// Blue, 0 to 255.
    pub b: u8,
    /// Opacity, 0 (clear) to 255 (opaque); the colour is not premultiplied.
    pub a: u8,
}

impl Edges {
    /// No inset on any side.
    pub const ZERO: Edges = Edges { l: 0.0, t: 0.0, r: 0.0, b: 0.0 };
    /// Edges with the same inset `v` points on all four sides.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::Edges;
    /// assert_eq!(Edges::all(4.0), Edges { l: 4.0, t: 4.0, r: 4.0, b: 4.0 });
    /// ```
    pub const fn all(v: f32) -> Edges { Edges { l: v, t: v, r: v, b: v } }
    /// Edges with `x` on the left and right and `y` on the top and bottom.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::Edges;
    /// const E: Edges = Edges::xy(2.0, 3.0);
    /// assert_eq!(E, Edges { l: 2.0, t: 3.0, r: 2.0, b: 3.0 });
    /// ```
    #[inline]
    pub const fn xy(x: f32, y: f32) -> Edges {
        Edges { l: x, t: y, r: x, b: y }
    }
}

/// The colour a look's fields take (`Look::fill`, `Look::text`, ...): `Color::rgb(..)` or `Color::token(TOKEN)`, the same type as `Rgba`.
pub type Color = Rgba;

impl Rgba {
    /// Red, green and blue under the token encoding: a clear colour with these two channels carries a token in `b`.
    const TOKEN_RG: (u8, u8) = (0x54, 0x4b);

    /// The opaque colour of channels `r`, `g`, `b`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::Color;
    /// const INK: Color = Color::rgb(1, 2, 3);
    /// assert_eq!(INK, Color { r: 1, g: 2, b: 3, a: 255 });
    /// ```
    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba { Rgba { r, g, b, a: 255 } }

    /// The colour of `token` in the app's light/dark table (`Ui::set_colors`), resolved by the engine for the theme in effect when a
    /// look holding it is applied to a node (a node's look, `set_look`, `pressable`, a button's, icon button's, toggle's or segment's
    /// looks, `patch_look`, an animated look), so a theme or appearance switch needs no rebuild by the app.
    ///
    /// It is encoded in the colour's bytes (final): alpha 0, red 0x54, green 0x4b, blue the token. A token at or past `ColorToken::LIMIT`
    /// (64), or past the installed table, resolves transparent (a token past 255 is stored as 255). Bodies receive their looks, swatches
    /// and ink already resolved, and so does a theme whose looks name tokens. Resolution happens once, where a look is applied, never on
    /// an interpolated look (`Look::lerp`, a running transition). A `Paint` call given a raw token paints nothing (the clear colour its
    /// bytes spell): resolve it first with `ui.color(token)` or `paint.color(token)`; text span colours are not resolved either.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Color, theme::ColorToken};
    /// const ACCENT: Color = Color::token(ColorToken(3));
    /// assert_eq!(ACCENT.as_token(), Some(ColorToken(3)));
    /// assert_eq!(Color::rgb(0, 0, 0).as_token(), None);
    /// ```
    #[inline]
    pub const fn token(token: crate::theme::ColorToken) -> Rgba {
        Rgba { r: Self::TOKEN_RG.0, g: Self::TOKEN_RG.1, b: if token.0 > 255 { 255 } else { token.0 as u8 }, a: 0 }
    }

    /// The token this colour names (`Rgba::token`), or `None` for a plain colour.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Color, theme::ColorToken};
    /// assert_eq!(Color::token(ColorToken(1)).as_token(), Some(ColorToken(1)));
    /// assert_eq!(Color::default().as_token(), None);
    /// ```
    #[inline]
    pub const fn as_token(self) -> Option<crate::theme::ColorToken> {
        if self.a == 0 && self.r == Self::TOKEN_RG.0 && self.g == Self::TOKEN_RG.1 { Some(crate::theme::ColorToken(self.b as u16)) } else { None }
    }
}

impl Rect {
    /// The rectangle with top-left `x`, `y` and size `w` by `h`, all in logical points; nothing is checked, so a negative size is kept.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::Rect;
    /// assert_eq!(Rect::new(1.0, 2.0, 3.0, 4.0), Rect { x: 1.0, y: 2.0, w: 3.0, h: 4.0 });
    /// ```
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect { Rect { x, y, w, h } }
    /// The rectangle as an `(x, y, w, h)` tuple of `f64`, each widened exactly from its `f32`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::Rect;
    /// assert_eq!(Rect::new(1.0, 2.0, 3.0, 4.0).f64(), (1.0, 2.0, 3.0, 4.0));
    /// ```
    pub fn f64(self) -> (f64, f64, f64, f64) { (self.x as f64, self.y as f64, self.w as f64, self.h as f64) }
    /// Whether the point `p` (x, y in the rectangle's coordinate space) is inside: left and top edges count, right and bottom do not.
    ///
    /// A rectangle of zero or negative size contains nothing.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::Rect;
    /// let r = Rect::new(0.0, 0.0, 10.0, 10.0);
    /// assert!(r.contains((0.0, 0.0)) && !r.contains((10.0, 5.0)));
    /// ```
    pub fn contains(self, p: (f32, f32)) -> bool { p.0 >= self.x && p.1 >= self.y && p.0 < self.x + self.w && p.1 < self.y + self.h }
    /// The overlap of this rectangle and `o`; when they do not overlap the width or height (or both) is 0.
    ///
    /// An empty result keeps the position the overlap arithmetic gives (the larger left and top), not the origin.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::Rect;
    /// let a = Rect::new(0.0, 0.0, 10.0, 10.0);
    /// assert_eq!(a.intersect(Rect::new(5.0, 5.0, 10.0, 10.0)), Rect::new(5.0, 5.0, 5.0, 5.0));
    /// assert_eq!(a.intersect(Rect::new(20.0, 0.0, 5.0, 5.0)).w, 0.0);
    /// ```
    pub fn intersect(self, o: Rect) -> Rect {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let right = (self.x + self.w).min(o.x + o.w);
        let bottom = (self.y + self.h).min(o.y + o.h);
        Rect { x, y, w: (right - x).max(0.0), h: (bottom - y).max(0.0) }
    }
}

/// Colour constructors and blends, with the Rays colour's rounding.
pub mod color {
    pub use super::Rgba as T;
    use super::Rgba;

    fn clamp_byte(value: i64) -> u8 { value.clamp(0, 255) as u8 }
    /// The colour of channels `r`, `g`, `b` and opacity `a`, each clamped to 0 to 255.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::{color, Rgba};
    /// assert_eq!(color::rgba(300, -5, 10, 128), Rgba { r: 255, g: 0, b: 10, a: 128 });
    /// ```
    pub fn rgba(r: i64, g: i64, b: i64, a: i64) -> Rgba { Rgba { r: clamp_byte(r), g: clamp_byte(g), b: clamp_byte(b), a: clamp_byte(a) } }
    /// The opaque colour of channels `r`, `g`, `b`, each clamped to 0 to 255.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::{color, Rgba};
    /// assert_eq!(color::rgb(1, 2, 3), Rgba { r: 1, g: 2, b: 3, a: 255 });
    /// ```
    pub fn rgb(r: i64, g: i64, b: i64) -> Rgba { rgba(r, g, b, 255) }
    /// The opaque grey with all three channels `v`, clamped to 0 to 255.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::geom::color::gray(128).g, 128);
    /// ```
    pub fn gray(v: i64) -> Rgba { rgb(v, v, v) }
    /// Opaque black.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::geom::color::black(), pxui_types::geom::color::rgb(0, 0, 0));
    /// ```
    pub fn black() -> Rgba { rgb(0, 0, 0) }
    /// Opaque white.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::geom::color::white().r, 255);
    /// ```
    pub fn white() -> Rgba { rgb(255, 255, 255) }
    /// Fully transparent black (all four channels 0).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::geom::color::transparent().a, 0);
    /// ```
    pub fn transparent() -> Rgba { rgba(0, 0, 0, 0) }
    /// The colour `c` with its opacity replaced by `alpha`, clamped to 0 to 255 (not multiplied into the old one).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::color;
    /// assert_eq!(color::with_alpha(color::rgb(1, 2, 3), 10), color::rgba(1, 2, 3, 10));
    /// ```
    pub fn with_alpha(c: Rgba, alpha: i64) -> Rgba { Rgba { a: clamp_byte(alpha), ..c } }
    /// The colour from channels in 0.0 to 1.0 (`alpha` `None`: opaque); each is scaled by 255, rounded and clamped, so out-of-range values saturate.
    ///
    /// The argument order is alpha first, then red, green, blue.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::color;
    /// assert_eq!(color::of_unit_floats(None, 1.0, 0.5, 0.0), color::rgb(255, 128, 0));
    /// ```
    pub fn of_unit_floats(alpha: Option<f64>, r: f64, g: f64, b: f64) -> Rgba {
        let byte = |v: f64| (v * 255.0).round() as i64;
        rgba(byte(r), byte(g), byte(b), byte(alpha.unwrap_or(1.0)))
    }
    /// The colour's channels as `(r, g, b, a)`, each divided by 255 into 0.0 to 1.0.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::color;
    /// assert_eq!(color::to_floats(color::rgba(255, 0, 255, 0)), (1.0, 0.0, 1.0, 0.0));
    /// ```
    pub fn to_floats(c: Rgba) -> (f64, f64, f64, f64) { (c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0, c.a as f64 / 255.0) }

    /// Parses `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa` (the `#` is optional, digits of either case); a missing alpha is opaque.
    ///
    /// The short forms repeat each digit (`#f80` is `#ff8800`). Any other length or a non-hex digit is an `Err` with a message.
    /// Allocates: the short forms allocate a scratch buffer; the long forms do not (an `Err` allocates its message).
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::color;
    /// assert_eq!(color::hex("#f80"), Ok(color::rgb(255, 136, 0)));
    /// assert_eq!(color::hex("11223380"), Ok(color::rgba(17, 34, 51, 128)));
    /// assert!(color::hex("#12").is_err());
    /// ```
    pub fn hex(value: &str) -> Result<Rgba, String> {
        let value = value.strip_prefix('#').unwrap_or(value).as_bytes();
        let expanded = if value.len() == 3 || value.len() == 4 { Some(value.iter().flat_map(|b| [*b, *b]).collect::<Vec<_>>()) } else { None };
        let bytes = expanded.as_deref().unwrap_or(value);
        if bytes.len() != 6 && bytes.len() != 8 { return Err("Color.hex: expected #rgb, #rgba, #rrggbb, or #rrggbbaa".to_owned()); }
        let nibble = |v: u8| (v as char).to_digit(16).map(|d| d as i64);
        let channel = |i: usize| Some(nibble(bytes[i])? * 16 + nibble(bytes[i + 1])?);
        match (channel(0), channel(2), channel(4), if bytes.len() == 8 { channel(6) } else { Some(255) }) {
            (Some(r), Some(g), Some(b), Some(a)) => Ok(rgba(r, g, b, a)),
            _ => Err("Color.hex: invalid hexadecimal digit".to_owned()),
        }
    }

    /// The colour as lower-case `#rrggbb`, or `#rrggbbaa` when it is not fully opaque; `hex` reads it back.
    ///
    /// Allocates: one `String`.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::color;
    /// assert_eq!(color::to_hex_string(color::rgb(255, 136, 0)), "#ff8800");
    /// assert_eq!(color::to_hex_string(color::rgba(0, 0, 0, 1)), "#00000001");
    /// ```
    pub fn to_hex_string(c: Rgba) -> String {
        if c.a == 255 { format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b) } else { format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a) }
    }

    /// The colour `pct` of the way from `c1` to `c2`, every channel (alpha too) interpolated linearly and rounded to nearest.
    ///
    /// `pct` is clamped to 0.0 to 1.0, so 0 gives `c1` and 1 gives `c2`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::color;
    /// assert_eq!(color::blend(color::black(), color::white(), 0.5), color::gray(128));
    /// assert_eq!(color::blend(color::black(), color::white(), 9.0), color::white());
    /// ```
    pub fn blend(c1: Rgba, c2: Rgba, pct: f64) -> Rgba {
        let pct = pct.clamp(0.0, 1.0);
        let inv = 1.0 - pct;
        let mix = |a: u8, b: u8| ((a as f64 * inv) + (b as f64 * pct) + 0.5) as u8;
        Rgba { r: mix(c1.r, c2.r), g: mix(c1.g, c2.g), b: mix(c1.b, c2.b), a: mix(c1.a, c2.a) }
    }
}

/// Deterministic key hash (SplitMix64 words, length-framed bytes), same values as before.
pub mod hash {
    /// A multiply-rotate hasher for the engine's internal keys (glyph cache keys, node and layout keys):
    /// Reuse for internal UI revisions and identities; this is not a DoS-resistant map hasher.
    #[derive(Default, Clone, Copy)]
    pub struct FastHasher(u64);
    impl std::hash::Hasher for FastHasher {
        #[inline]
        fn finish(&self) -> u64 {
            self.0
        }
        #[inline]
        fn write(&mut self, bytes: &[u8]) {
            let mut chunks = bytes.chunks_exact(8);
            for c in &mut chunks {
                self.write_u64(u64::from_le_bytes(c.try_into().unwrap()));
            }
            let mut tail = [0u8; 8];
            tail[..chunks.remainder().len()].copy_from_slice(chunks.remainder());
            self.write_u64(u64::from_le_bytes(tail) ^ (bytes.len() as u64) << 56);
        }
        #[inline]
        fn write_u8(&mut self, n: u8) {
            self.write_u64(n as u64);
        }
        #[inline]
        fn write_u16(&mut self, n: u16) {
            self.write_u64(n as u64);
        }
        #[inline]
        fn write_u32(&mut self, n: u32) {
            self.write_u64(n as u64);
        }
        #[inline]
        fn write_u64(&mut self, n: u64) {
            // the final multiply spreads the low bits into the high bits hashbrown's control bytes read
            self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
        #[inline]
        fn write_usize(&mut self, n: usize) {
            self.write_u64(n as u64);
        }
    }
    const SEED_GAMMA: u64 = 0x9e3779b97f4a7c15;
    fn word(hash: u64, value: u64) -> u64 {
        let mut v = (hash ^ value).wrapping_add(SEED_GAMMA);
        v = (v ^ (v >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        v = (v ^ (v >> 27)).wrapping_mul(0x94d049bb133111eb);
        v ^ (v >> 31)
    }
    /// A 64-bit hash of `bytes` continuing from `seed`, framed by the length so a prefix never hashes like the whole.
    ///
    /// The same seed and bytes always give the same value, across runs and machines; it is not a cryptographic or DoS-safe hash.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::hash;
    /// assert_eq!(hash::bytes(1, b"abc"), hash::bytes(1, b"abc"));
    /// assert_ne!(hash::bytes(1, b"abc"), hash::bytes(1, b"abc\0"));
    /// ```
    pub fn bytes(seed: u64, bytes: &[u8]) -> u64 {
        let mut hash = word(seed, bytes.len() as u64);
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks { hash = word(hash, u64::from_le_bytes(chunk.try_into().unwrap())); }
        if !chunks.remainder().is_empty() {
            let mut tail = [0; 8];
            tail[..chunks.remainder().len()].copy_from_slice(chunks.remainder());
            hash = word(hash, u64::from_le_bytes(tail));
        }
        hash
    }
    /// Folds `value` into `hash`, a new 64-bit hash (as `i64`) that depends on both and on their order.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::hash;
    /// assert_ne!(hash::mix(hash::mix(0, 1), 2), hash::mix(hash::mix(0, 2), 1));
    /// ```
    pub fn mix(hash: i64, value: i64) -> i64 { word(hash as u64, value as u64) as i64 }
}

/// The point `p` narrowed from `f64` to `f32` (nearest; large values lose precision).
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// assert_eq!(pxui_types::geom::p32((1.5, -2.0)), (1.5f32, -2.0f32));
/// ```
pub fn p32(p: (f64, f64)) -> (f32, f32) { (p.0 as f32, p.1 as f32) }

/// The point `p` widened from `f32` to `f64`, exactly.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// assert_eq!(pxui_types::geom::p64((1.5, -2.0)), (1.5f64, -2.0f64));
/// ```
pub fn p64(p: (f32, f32)) -> (f64, f64) { (p.0 as f64, p.1 as f64) }
