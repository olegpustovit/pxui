//! PXUI's own plain f32 geometry, colour (`Rgba`) and key hash: the engine depends on nothing for them.
//! Points and sizes are logical points; colour channels are bytes, as in the Rays colour.

/// A position in logical points (window space unless a call says otherwise): `x` grows right, `y` grows down.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Point {
    /// Horizontal position in logical points.
    pub x: f32,
    /// Vertical position in logical points, growing downward.
    pub y: f32,
}

impl Point {
    /// Creates a position in logical points.
    ///
    /// The caller specifies the coordinate space.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::geom::Point::new(4.0, 8.0).x, 4.0);
    /// ```
    pub const fn new(x: f32, y: f32) -> Point { Point { x, y } }
}

/// An axis-aligned rectangle in logical points: top-left `x`, `y` and size `w` by `h` (a negative size is kept, and contains nothing).
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

/// Four insets in logical points (padding, margin, safe area): left, top, right, bottom; under right-to-left flow `l` is the start edge.
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
    /// Sets all four insets in logical points.
    ///
    /// The order is left, top, right, bottom.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::geom::Edges::new(1.0, 2.0, 3.0, 4.0).b, 4.0);
    /// ```
    pub const fn new(l: f32, t: f32, r: f32, b: f32) -> Edges { Edges { l, t, r, b } }

    /// Sets the left inset in logical points.
    ///
    /// Other edges keep their values.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::geom::Edges::all(0.0).l(4.0).l, 4.0);
    /// ```
    pub const fn l(self, points: f32) -> Edges { Edges { l: points, ..self } }

    /// Sets the top inset in logical points.
    ///
    /// Other edges keep their values.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::geom::Edges::all(0.0).t(4.0).t, 4.0);
    /// ```
    pub const fn t(self, points: f32) -> Edges { Edges { t: points, ..self } }

    /// Sets the right inset in logical points.
    ///
    /// Other edges keep their values.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::geom::Edges::all(0.0).r(4.0).r, 4.0);
    /// ```
    pub const fn r(self, points: f32) -> Edges { Edges { r: points, ..self } }

    /// Sets the bottom inset in logical points.
    ///
    /// Other edges keep their values.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::geom::Edges::all(0.0).b(4.0).b, 4.0);
    /// ```
    pub const fn b(self, points: f32) -> Edges { Edges { b: points, ..self } }

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

impl Rgba {
    /// Red, green and blue under the token encoding: a clear colour with these two channels carries a token in `b`.
    const TOKEN_RG: (u8, u8) = (0x54, 0x4b);

    /// The opaque colour of channels `r`, `g`, `b`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::Rgba;
    /// const INK: Rgba = Rgba::rgb(1, 2, 3);
    /// assert_eq!(INK, Rgba { r: 1, g: 2, b: 3, a: 255 });
    /// ```
    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba { Rgba { r, g, b, a: 255 } }

    /// Opaque black.
    pub const BLACK: Rgba = Rgba::rgb(0, 0, 0);
    /// Opaque white.
    pub const WHITE: Rgba = Rgba::rgb(255, 255, 255);
    /// Fully transparent black (all four channels 0): paints nothing.
    pub const CLEAR: Rgba = Rgba::rgba(0, 0, 0, 0);

    /// The colour of channels `r`, `g`, `b` and opacity `a` (0 clear, 255 opaque; not premultiplied).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::Rgba;
    /// const SCRIM: Rgba = Rgba::rgba(0, 0, 0, 128);
    /// assert_eq!(SCRIM, Rgba { r: 0, g: 0, b: 0, a: 128 });
    /// ```
    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba { Rgba { r, g, b, a } }

    /// The opaque grey with all three channels `v`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::Rgba;
    /// assert_eq!(Rgba::gray(128), Rgba::rgb(128, 128, 128));
    /// ```
    #[inline]
    pub const fn gray(v: u8) -> Rgba { Rgba::rgb(v, v, v) }

    /// The opaque colour written `0xRRGGBB` (the bits above the low 24 are ignored).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::Rgba;
    /// const ACCENT: Rgba = Rgba::hex(0xf0481f);
    /// assert_eq!(ACCENT, Rgba::rgb(0xf0, 0x48, 0x1f));
    /// ```
    #[inline]
    pub const fn hex(rgb: u32) -> Rgba { Rgba::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8) }

    /// This colour with its opacity replaced by `alpha` (not multiplied into the old one).
    ///
    /// A token colour (`Rgba::token`) given an alpha above 0 becomes the plain colour its bytes spell.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::Rgba;
    /// const HAZE: Rgba = Rgba::WHITE.with_alpha(40);
    /// assert_eq!(HAZE, Rgba::rgba(255, 255, 255, 40));
    /// ```
    #[inline]
    pub const fn with_alpha(self, alpha: u8) -> Rgba { Rgba { a: alpha, ..self } }

    /// The colour `t` of the way from this one to `to`, every channel (alpha too) interpolated linearly and rounded to nearest.
    ///
    /// `t` is clamped to 0.0 to 1.0, so 0 gives this colour and 1 gives `to`. Token colours blend as their bytes; resolve them first.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::Rgba;
    /// assert_eq!(Rgba::BLACK.blend(Rgba::WHITE, 0.5), Rgba::gray(128));
    /// assert_eq!(Rgba::BLACK.blend(Rgba::WHITE, 9.0), Rgba::WHITE);
    /// ```
    pub fn blend(self, to: Rgba, t: f64) -> Rgba {
        let t = t.clamp(0.0, 1.0);
        let inv = 1.0 - t;
        let mix = |a: u8, b: u8| ((a as f64 * inv) + (b as f64 * t) + 0.5) as u8;
        Rgba { r: mix(self.r, to.r), g: mix(self.g, to.g), b: mix(self.b, to.b), a: mix(self.a, to.a) }
    }

    /// Parses `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa` (the `#` is optional, digits of either case); a missing alpha is opaque.
    ///
    /// The short forms repeat each digit (`#f80` is `#ff8800`). Any other length is `ColorParseError::Length`, a non-hex digit
    /// `ColorParseError::Digit`.
    /// Use it for text read at run time (theme files); a colour known when writing the code is `Rgba::hex(0xRRGGBB)`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::{ColorParseError, Rgba};
    /// assert_eq!(Rgba::parse("#f80"), Ok(Rgba::rgb(255, 136, 0)));
    /// assert_eq!(Rgba::parse("11223380"), Ok(Rgba::rgba(17, 34, 51, 128)));
    /// assert_eq!(Rgba::parse("#12"), Err(ColorParseError::Length));
    /// assert_eq!(Rgba::parse("#12345g"), Err(ColorParseError::Digit));
    /// ```
    pub fn parse(text: &str) -> Result<Rgba, ColorParseError> {
        let digits = text.strip_prefix('#').unwrap_or(text).as_bytes();
        let short = digits.len() == 3 || digits.len() == 4;
        if !short && digits.len() != 6 && digits.len() != 8 { return Err(ColorParseError::Length); }
        let nibble = |i: usize| (digits[i] as char).to_digit(16).map(|d| d as u8);
        let channel = |i: usize| if short { nibble(i).map(|d| d * 17) } else { Some(nibble(2 * i)? * 16 + nibble(2 * i + 1)?) };
        let alpha = if digits.len() == 4 || digits.len() == 8 { channel(3) } else { Some(255) };
        match (channel(0), channel(1), channel(2), alpha) {
            (Some(r), Some(g), Some(b), Some(a)) => Ok(Rgba { r, g, b, a }),
            _ => Err(ColorParseError::Digit),
        }
    }

    /// The colour as lower-case `#rrggbb`, or `#rrggbbaa` when it is not fully opaque; `Rgba::parse` reads it back.
    ///
    /// Allocates: one `String`.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::geom::Rgba;
    /// assert_eq!(Rgba::rgb(255, 136, 0).to_hex_string(), "#ff8800");
    /// assert_eq!(Rgba::rgba(0, 0, 0, 1).to_hex_string(), "#00000001");
    /// ```
    pub fn to_hex_string(self) -> String {
        if self.a == 255 { format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b) } else { format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a) }
    }

    /// The colour of `token` in the active theme's palette, resolved by the engine when a
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
    /// use pxui_types::{geom::Rgba, theme::ColorToken};
    /// const ACCENT: Rgba = Rgba::token(ColorToken(3));
    /// assert_eq!(ACCENT.as_token(), Some(ColorToken(3)));
    /// assert_eq!(Rgba::BLACK.as_token(), None);
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
    /// use pxui_types::{geom::Rgba, theme::ColorToken};
    /// assert_eq!(Rgba::token(ColorToken(1)).as_token(), Some(ColorToken(1)));
    /// assert_eq!(Rgba::default().as_token(), None);
    /// ```
    #[inline]
    pub const fn as_token(self) -> Option<crate::theme::ColorToken> {
        if self.a == 0 && self.r == Self::TOKEN_RG.0 && self.g == Self::TOKEN_RG.1 { Some(crate::theme::ColorToken(self.b as u16)) } else { None }
    }
}

/// Why `Rgba::parse` refused a text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorParseError {
    /// Not 3, 4, 6 or 8 hex digits after the optional `#`.
    Length,
    /// A character that is not a hexadecimal digit.
    Digit,
}

impl std::fmt::Display for ColorParseError {
    /// The refusal in words, for a theme file's error message.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self { ColorParseError::Length => "expected #rgb, #rgba, #rrggbb or #rrggbbaa", ColorParseError::Digit => "invalid hexadecimal digit" })
    }
}

impl std::error::Error for ColorParseError {}

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
