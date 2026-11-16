//! PXUI's own plain f32 geometry, colour and key hash: the engine depends on nothing for them.
//! Points and sizes are logical points; colour channels are bytes, as in the Rays colour.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Edges {
    pub l: f32,
    pub t: f32,
    pub r: f32,
    pub b: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Edges {
    pub const ZERO: Edges = Edges { l: 0.0, t: 0.0, r: 0.0, b: 0.0 };
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

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect { Rect { x, y, w, h } }
    /// The rectangle as Rays f64 data at the boundary.
    pub fn f64(self) -> (f64, f64, f64, f64) { (self.x as f64, self.y as f64, self.w as f64, self.h as f64) }
    pub fn contains(self, p: (f32, f32)) -> bool { p.0 >= self.x && p.1 >= self.y && p.0 < self.x + self.w && p.1 < self.y + self.h }
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
    pub fn rgba(r: i64, g: i64, b: i64, a: i64) -> Rgba { Rgba { r: clamp_byte(r), g: clamp_byte(g), b: clamp_byte(b), a: clamp_byte(a) } }
    pub fn rgb(r: i64, g: i64, b: i64) -> Rgba { rgba(r, g, b, 255) }
    pub fn gray(v: i64) -> Rgba { rgb(v, v, v) }
    pub fn black() -> Rgba { rgb(0, 0, 0) }
    pub fn white() -> Rgba { rgb(255, 255, 255) }
    pub fn transparent() -> Rgba { rgba(0, 0, 0, 0) }
    pub fn with_alpha(c: Rgba, alpha: i64) -> Rgba { Rgba { a: clamp_byte(alpha), ..c } }
    pub fn of_unit_floats(alpha: Option<f64>, r: f64, g: f64, b: f64) -> Rgba {
        let byte = |v: f64| (v * 255.0).round() as i64;
        rgba(byte(r), byte(g), byte(b), byte(alpha.unwrap_or(1.0)))
    }
    pub fn to_floats(c: Rgba) -> (f64, f64, f64, f64) { (c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0, c.a as f64 / 255.0) }

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
    pub fn hex_exn(value: &str) -> Result<Rgba, String> { hex(value) }

    pub fn to_hex_string(c: Rgba) -> String {
        if c.a == 255 { format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b) } else { format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a) }
    }

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
    pub fn mix(hash: i64, value: i64) -> i64 { word(hash as u64, value as u64) as i64 }
}

/// A Rays f64 point at the Paint boundary.
pub fn p32(p: (f64, f64)) -> (f32, f32) { (p.0 as f32, p.1 as f32) }

/// A pointer position back to Rays f64 data.
pub fn p64(p: (f32, f32)) -> (f64, f64) { (p.0 as f64, p.1 as f64) }
