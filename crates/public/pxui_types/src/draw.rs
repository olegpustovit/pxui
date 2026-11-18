//! The draw list: plain 64-byte instances, one vector per UI layer. `pxui_render` draws a layer in
//! one call; the instance layout and its conventions are documented there.

/// Instance kinds (byte 50).
pub mod kind {
    /// A filled, optionally rounded and bordered rectangle (`Instance::rect`).
    pub const RECT: u8 = 0;
    /// A glyph sampled from the R8 mask atlas, tinted by `color` (`Instance::mask`).
    pub const MASK: u8 = 1;
    /// A colour glyph or image from the RGBA atlas or the layer's break texture (`Instance::image`).
    pub const IMAGE: u8 = 2;
    /// An analytic gaussian shadow (`Instance::shadow`).
    pub const SHADOW: u8 = 3;
    /// A straight stroke between two points (`Instance::line`).
    pub const LINE: u8 = 4;
    /// A circular arc (`Instance::arc`).
    pub const ARC: u8 = 5;
    /// A quadratic bezier stroke (`Instance::curve`).
    pub const CURVE: u8 = 6;
    /// A repeating dot pattern (`Instance::grid`): one instance for a whole grid.
    pub const GRID: u8 = 7;
    /// A CSS border with a width and colour per side (`borders`): one instance, corners mitred.
    pub const BORDER: u8 = 8;
    /// A filled triangle; rect contains A/B and extra contains C as f32 bits.
    pub const TRIANGLE:u8=9;
}
/// Instance flags (byte 51), one module per instance kind: a kind reads only its own bits, so the same bit means
/// different things to different kinds (bit 0 is a linear gradient on a rect and linear filtering on an image). Take a
/// flag from the module of the instance's `kind`; the `flags_*` test pins every shared bit.
pub mod flags {
    /// Flags of a `kind::RECT` (and `kind::BORDER`) instance.
    pub mod rect {
        /// Gradient kind 1 in the low two bits: a linear gradient (`Instance::linear`).
        pub const LINEAR: u8 = 1;
        /// Gradient kind 2 in the low two bits: a radial gradient (`Instance::radial`).
        pub const RADIAL: u8 = 2;
        /// Gradient kind 3 in the low two bits: a conic gradient (`Instance::conic`).
        pub const CONIC: u8 = 3;
        /// Pixel-area coverage for a plain rect, preserving the weight of subpixel fills (bit 2).
        pub const AREA: u8 = 4;
        /// Border on the top side (bits 4 to 7 are the border sides; none set, or all four, borders every side).
        pub const TOP: u8 = 16;
        /// Border on the right side.
        pub const RIGHT: u8 = 32;
        /// Border on the bottom side.
        pub const BOTTOM: u8 = 64;
        /// Border on the left side.
        pub const LEFT: u8 = 128;
    }
    /// Flags of a `kind::MASK` (R8 glyph) instance.
    pub mod mask {
        /// The glyph is on the second fixed mask atlas page (bit 1).
        pub const GLYPH_PAGE_2: u8 = 2;
        /// A glyph of the Apple raster (`Instance::light`): the shader adds CoreText's light-on-dark stem growth (bit 4).
        pub const LIGHT: u8 = 16;
    }
    /// Flags of a `kind::IMAGE` (colour glyph or picture) instance.
    pub mod image {
        /// Sampled with the painter's linear filter: a picture drawn at any size (`Paint::image`); without it images
        /// sample nearest, as glyphs do (bit 0).
        pub const SMOOTH: u8 = 1;
        /// The colour glyph is on the second fixed colour atlas page, independent of image breaks (bit 1).
        pub const GLYPH_PAGE_2: u8 = 2;
    }
    /// Flags of a `kind::SHADOW` instance.
    pub mod shadow {
        /// The shadow is drawn inside its rect (`Instance::shadow` with `inner`) (bit 2).
        pub const INNER: u8 = 4;
    }
    /// Flags of a stroke: `kind::LINE` and `kind::ARC` (a `kind::CURVE` is always round).
    pub mod stroke {
        /// Round caps (`Instance::round_caps`) instead of butt ends (bit 3).
        pub const ROUND: u8 = 8;
    }
}
/// The clip that cuts nothing: the whole `i16` range.
pub const NO_CLIP: [i16; 4] = [i16::MIN, i16::MIN, i16::MAX, i16::MAX];

/// Adds `[a, b)` to `ranges`, a sorted list of disjoint instance ranges; overlapping and touching
/// ranges merge. Past `cap` ranges the two with the smallest gap between them merge, so scattered
/// edits stay a few small ranges, never one span over everything between them. O(ranges); no
/// allocation once the list has held `cap + 1` ranges.
///
/// `ranges` must already be sorted and disjoint (as this function leaves it); an empty range (`a >= b`) changes nothing, and `cap` of
/// 0 counts as 1.
/// Allocates: only when `ranges` has to grow past its capacity (at most `cap + 1` entries).
/// Panics: never.
///
/// ```
/// use pxui_types::draw::add_range;
/// let mut ranges = Vec::new();
/// add_range(&mut ranges, 0, 4, 8);
/// add_range(&mut ranges, 4, 6, 8);
/// add_range(&mut ranges, 10, 12, 8);
/// assert_eq!(ranges, [(0, 6), (10, 12)]);
/// add_range(&mut ranges, 20, 22, 2); // past `cap` ranges, the two closest merge
/// assert_eq!(ranges, [(0, 12), (20, 22)]);
/// ```
#[inline]
pub fn add_range(ranges: &mut Vec<(u32, u32)>, a: u32, b: u32, cap: usize) {
    if a >= b {
        return;
    }
    if let Some(last) = ranges.last_mut() {
        if a >= last.0 && a <= last.1 {
            last.1 = last.1.max(b);
            return;
        }
    }
    insert_range(ranges, a, b, cap);
}

fn insert_range(ranges: &mut Vec<(u32, u32)>, a: u32, b: u32, cap: usize) {
    let at = ranges.partition_point(|r| r.1 < a);
    let mut end = at;
    let (mut lo, mut hi) = (a, b);
    while end < ranges.len() && ranges[end].0 <= hi {
        lo = lo.min(ranges[end].0);
        hi = hi.max(ranges[end].1);
        end += 1;
    }
    if end > at {
        ranges[at] = (lo, hi);
        ranges.drain(at + 1..end);
    } else {
        ranges.insert(at, (lo, hi));
    }
    if ranges.len() > cap.max(1) {
        let k = (0..ranges.len() - 1).min_by_key(|&k| ranges[k + 1].0 - ranges[k].1).unwrap_or(0);
        ranges[k].1 = ranges[k + 1].1;
        ranges.remove(k + 1);
    }
}

/// One quad. Plain data: build with the constructors below or write the fields per the layout table.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    /// Corners `[x0, y0, x1, y1]` in points; line, curve, arc, triangle and grid instances put their own geometry here (see their constructors).
    pub rect: [f32; 4],
    /// Clip `[left, top, right, bottom]` in whole points.
    pub clip: [i16; 4],
    /// Fill, tint or stroke colour, `[r, g, b, a]`.
    pub color: [u8; 4],
    /// Second colour: the border colour of a rect, the second side of a `borders` instance.
    pub color2: [u8; 4],
    /// Kind-specific words: atlas uv, dash, gradient end colour, border colours, grid spacing.
    pub aux: [u32; 2],
    /// Corner radii as `f16` bits: top left, top right, bottom right, bottom left.
    pub radii: [u16; 4],
    /// Stroke or border width as `f16` bits, in points.
    pub width: u16,
    /// One of the `kind` constants.
    pub kind: u8,
    /// A mix of the `flags` constants (gradient kind in the low bits, border sides in the high bits).
    pub flags: u8,
    /// Index into the layer's view table (its transform and opacity).
    pub view: u16,
    /// Clip corner radius as `f16` bits, in points.
    pub clip_radius: u16,
    /// Kind-specific words: shadow offset and sigma, curve control point, gradient geometry, glyph scale.
    pub extra: [u32; 2],
}
const _: () = assert!(size_of::<Instance>() == 64);

/// `Break::image` of a break back to the colour atlas (colour glyphs after an image).
pub const ATLAS: u32 = u32::MAX;

/// From `at_instance` on, the layer's [`kind::IMAGE`] instances sample `images[image]` (or the
/// colour atlas for [`ATLAS`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Break {
    /// Index of the first instance the break applies to.
    pub at_instance: u32,
    /// Index into the layer's images, or `ATLAS`.
    pub image: u32,
}

/// CSS `border` with a width and colour per side `[top, right, bottom, left]` (draw the fill
/// separately): one `kind::BORDER` instance, `count` 1 (0 with no width). Each pixel of the band takes
/// the colour of the side it is nearest to relative to that side's width, so corners split on the
/// mitre from the outer to the inner corner as in CSS, with no seam; the inner corners' radii are the
/// outer ones less the wider adjacent side.
///
/// All four arrays are `[top, right, bottom, left]`; `rect` is `[x0, y0, x1, y1]` in points. Returns an array of four instances and how
/// many of them to draw: 0 when no width is above 0, else 1 (the first).
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::draw::borders;
/// let (_, none) = borders([0.0, 0.0, 10.0, 10.0], [0.0; 4], [0.0; 4], [[0; 4]; 4]);
/// let (_, one) = borders([0.0, 0.0, 10.0, 10.0], [0.0; 4], [1.0; 4], [[255; 4]; 4]);
/// assert_eq!((none, one), (0, 1));
/// ```
pub fn borders(rect: [f32; 4], radii: [f32; 4], widths: [f32; 4], colors: [[u8; 4]; 4]) -> ([Instance; 4], usize) {
    let mut out = [Instance::DEFAULT; 4];
    if widths.iter().all(|w| *w <= 0.0) {
        return (out, 0);
    }
    out[0] = Instance {
        rect, color: colors[0], color2: colors[1], kind: kind::BORDER,
        aux: [u32::from_le_bytes(colors[2]), u32::from_le_bytes(colors[3])],
        extra: [halves(widths[0], widths[1]), halves(widths[2], widths[3])], ..Instance::DEFAULT
    }.radii(radii);
    (out, 1)
}

/// How an image fills a box, like CSS `object-fit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Stretch to the box.
    Fill,
    /// Scale uniformly to fit inside the box, centred; the rect shrinks.
    Contain,
    /// Scale uniformly to cover the box, centred; the uv crop.
    Cover,
    /// Natural size, centred: smaller images shrink the rect, larger ones crop the uv.
    None,
}

/// The rect and uv to draw an `image` (width, height) in `bounds` (`[x0, y0, x1, y1]`) under `mode`.
/// `uv` is the image's normalised rect in its texture (a whole texture is `[0, 0, 1, 1]`, an atlas
/// entry its slot); the returned uv is the visible part of it. Pass both to [`Instance::image`].
///
/// `image` and `bounds` share one unit (`Fit::None` draws one image unit per bounds unit). An image with a side of 0 or less, or
/// `Fit::Fill`, returns `bounds` and `uv` unchanged. The result is cut to `bounds`.
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::draw::{fit, Fit};
/// let (rect, uv) = fit(Fit::Contain, [100.0, 50.0], [0.0, 0.0, 50.0, 50.0], [0.0, 0.0, 1.0, 1.0]);
/// assert_eq!(rect, [0.0, 12.5, 50.0, 37.5]);
/// assert_eq!(uv, [0.0, 0.0, 1.0, 1.0]);
/// ```
pub fn fit(mode: Fit, image: [f32; 2], bounds: [f32; 4], uv: [f32; 4]) -> ([f32; 4], [f32; 4]) {
    let (bw, bh) = (bounds[2] - bounds[0], bounds[3] - bounds[1]);
    if mode == Fit::Fill || !(image[0] > 0.0 && image[1] > 0.0) { return (bounds, uv); }
    let k = match mode {
        Fit::Contain => (bw / image[0]).min(bh / image[1]),
        Fit::Cover => (bw / image[0]).max(bh / image[1]),
        _ => 1.0,
    };
    let (w, h) = (image[0] * k, image[1] * k);
    let (x0, y0) = (bounds[0] + (bw - w) * 0.5, bounds[1] + (bh - h) * 0.5);
    let rect = [x0.max(bounds[0]), y0.max(bounds[1]), (x0 + w).min(bounds[2]), (y0 + h).min(bounds[3])];
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let visible = [(rect[0] - x0) / w, (rect[1] - y0) / h, (rect[2] - x0) / w, (rect[3] - y0) / h];
    (rect, [lerp(uv[0], uv[2], visible[0]), lerp(uv[1], uv[3], visible[1]), lerp(uv[0], uv[2], visible[2]), lerp(uv[1], uv[3], visible[3])])
}

/// CSS `border-image` nine-slice: nine image instances stretching the centre and edges of an `image`
/// (width, height in texels) between corners of `inset` (`[left, top, right, bottom]` in image units)
/// drawn 1:1 in `rect`, row-major from the top-left. Corners shrink together if the rect is smaller
/// than the insets. `uv` as for [`fit`].
///
/// `tint` multiplies every texel as in `Instance::image`. An `image` side of 0 gives non-finite uv (it is not checked).
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::draw::nine_slice;
/// let cells = nine_slice([0.0, 0.0, 30.0, 30.0], [0.0, 0.0, 1.0, 1.0], [30.0, 30.0], [10.0, 10.0, 10.0, 10.0], [255; 4]);
/// assert_eq!(cells[4].rect, [10.0, 10.0, 20.0, 20.0]);
/// ```
pub fn nine_slice(rect: [f32; 4], uv: [f32; 4], image: [f32; 2], inset: [f32; 4], tint: [u8; 4]) -> [Instance; 9] {
    let squeeze = |a: f32, b: f32, room: f32| if a + b > room && a + b > 0.0 { room.max(0.0) / (a + b) } else { 1.0 };
    let (kx, ky) = (squeeze(inset[0], inset[2], rect[2] - rect[0]), squeeze(inset[1], inset[3], rect[3] - rect[1]));
    let xs = [rect[0], rect[0] + inset[0] * kx, rect[2] - inset[2] * kx, rect[2]];
    let ys = [rect[1], rect[1] + inset[1] * ky, rect[3] - inset[3] * ky, rect[3]];
    let (du, dv) = (uv[2] - uv[0], uv[3] - uv[1]);
    let us = [uv[0], uv[0] + du * inset[0] / image[0], uv[2] - du * inset[2] / image[0], uv[2]];
    let vs = [uv[1], uv[1] + dv * inset[1] / image[1], uv[3] - dv * inset[3] / image[1], uv[3]];
    std::array::from_fn(|i| {
        let (x, y) = (i % 3, i / 3);
        Instance::image([xs[x], ys[y], xs[x + 1], ys[y + 1]], [us[x], vs[y], us[x + 1], vs[y + 1]], tint)
    })
}

/// IEEE half-precision bits of `x`, rounded to nearest even; values too large become infinity, NaN a quiet NaN.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::draw::f16;
/// assert_eq!(f16(1.0), 0x3c00);
/// assert_eq!(f16(-2.0), 0xc000);
/// assert_eq!(f16(1.0e9), 0x7c00);
/// ```
pub fn f16(x: f32) -> u16 {
    // most values a drawing passes (radii, widths, zero) are exact halves: their bits move over with no rounding
    let b = x.to_bits();
    let (sign, exponent, mantissa) = ((b >> 16) & 0x8000, (b >> 23) & 0xff, b & 0x7f_ffff);
    if (113..=142).contains(&exponent) && mantissa & 0x1fff == 0 { return (sign | (exponent - 112) << 10 | mantissa >> 13) as u16; }
    if b << 1 == 0 { return sign as u16; }
    f16_rounded(x)
}

/// `f16` of any value: the exact halves' shortcut aside, every case rounds here.
fn f16_rounded(x: f32) -> u16 {
    let bits = (x as f64).to_bits();
    let sign = if bits >> 63 != 0 { 0x8000 } else { 0 };
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mantissa = bits & 0x000f_ffff_ffff_ffff;
    let round = |value: u64, shift: u32| -> u64 {
        let q = value >> shift;
        let rest = value & ((1u64 << shift) - 1);
        let half = 1u64 << (shift - 1);
        q + if rest > half || (rest == half && q & 1 == 1) { 1 } else { 0 }
    };
    (if exponent == 0x7ff {
        sign | if mantissa == 0 { 0x7c00 } else { 0x7e00 }
    } else if exponent - 1008 >= 31 {
        sign | 0x7c00
    } else if exponent - 1008 > 0 {
        sign | ((((exponent - 1008) as u64) << 10) + round(mantissa, 42))
    } else if 1051 - exponent >= 54 {
        sign
    } else {
        sign | round(mantissa | (1u64 << 52), (1051 - exponent) as u32)
    }) as u16
}
fn halves(a: f32, b: f32) -> u32 { f16(a) as u32 | (f16(b) as u32) << 16 }
fn unorms(a: f32, b: f32) -> u32 {
    let u = |x: f32| (x.clamp(0.0, 1.0) * 65535.0).round() as u32;
    u(a) | u(b) << 16
}

impl Instance {
    /// A rect instance of nothing: zero geometry and colour, no clip.
    pub const DEFAULT: Instance = Instance {
        rect: [0.0; 4], clip: NO_CLIP, color: [0; 4], color2: [0; 4], aux: [0; 2], radii: [0; 4],
        width: 0, kind: kind::RECT, flags: 0, view: 0, clip_radius: 0, extra: [0; 2],
    };
    /// A filled rect `[x0, y0, x1, y1]` in points, in `color`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{kind, Instance};
    /// let i = Instance::rect([0.0, 0.0, 4.0, 4.0], [1, 2, 3, 4]);
    /// assert_eq!((i.kind, i.color), (kind::RECT, [1, 2, 3, 4]));
    /// ```
    pub fn rect(rect: [f32; 4], color: [u8; 4]) -> Self { Instance { rect, color, ..Self::DEFAULT } }
    /// A glyph from the R8 mask atlas drawn in `rect` (points), `uv` is `[u0, v0, u1, v1]` normalised (clamped to 0 to 1, 16-bit precision).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{kind, Instance};
    /// let i = Instance::mask([0.0, 0.0, 8.0, 8.0], [0.0, 0.0, 0.5, 0.5], [0, 0, 0, 255]);
    /// assert_eq!(i.kind, kind::MASK);
    /// ```
    pub fn mask(rect: [f32; 4], uv: [f32; 4], color: [u8; 4]) -> Self {
        Instance { rect, color, kind: kind::MASK, aux: [unorms(uv[0], uv[1]), unorms(uv[2], uv[3])], ..Self::DEFAULT }
    }
    /// The `extra` words of a light glyph at `ppem` device pixels and device scale `scale` (both as halves):
    /// pack once per text, not per glyph.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::Instance;
    /// assert_eq!(Instance::light_extra(26.0, 2.0)[1], 0);
    /// ```
    pub fn light_extra(ppem: f32, scale: f32) -> [u32; 2] { [halves(ppem, scale), 0] }
    /// A mask glyph rasterised for CoreText parity; the shader grows its ink by the fill colour's
    /// luminance level (`flags::mask::LIGHT`, `extra` from [`Instance::light_extra`]).
    ///
    /// Call it on a `mask` instance; it sets `flags::mask::LIGHT` and replaces `extra`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{flags, Instance};
    /// let glyph = Instance::mask([0.0; 4], [0.0; 4], [0; 4]).light(Instance::light_extra(26.0, 2.0));
    /// assert_ne!(glyph.flags & flags::mask::LIGHT, 0);
    /// ```
    pub fn light(self, extra: [u32; 2]) -> Self { Instance { flags: self.flags | flags::mask::LIGHT, extra, ..self } }
    /// A colour glyph or image from the RGBA atlas (or the texture of the enclosing break). `tint`
    /// multiplies the texel per channel, alpha included: white keeps the image, a grey scales brightness,
    /// a translucent white fades it, a colour tints a white-on-transparent icon. Corner radii (`radii`)
    /// round the image, see [`fit`] and [`nine_slice`] for placing it.
    ///
    /// `rect` is in points, `uv` as for `mask`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{kind, Instance};
    /// let i = Instance::image([0.0, 0.0, 8.0, 8.0], [0.0, 0.0, 1.0, 1.0], [255; 4]);
    /// assert_eq!(i.kind, kind::IMAGE);
    /// ```
    pub fn image(rect: [f32; 4], uv: [f32; 4], tint: [u8; 4]) -> Self { Instance { kind: kind::IMAGE, ..Self::mask(rect, uv, tint) } }
    /// An analytic gaussian shadow of the rounded rect `rect`/`radii` (points), shifted by `offset`, blurred with gaussian `sigma`,
    /// grown by `spread`; `inner` draws it inside the rect.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{kind, Instance};
    /// let i = Instance::shadow([0.0, 0.0, 20.0, 20.0], [4.0; 4], [0, 0, 0, 90], [0.0, 2.0], 3.0, 0.0, false);
    /// assert_eq!(i.kind, kind::SHADOW);
    /// ```
    pub fn shadow(rect: [f32; 4], radii: [f32; 4], color: [u8; 4], offset: [f32; 2], sigma: f32, spread: f32, inner: bool) -> Self {
        Instance {
            rect, color, kind: kind::SHADOW, flags: if inner { flags::shadow::INNER } else { 0 },
            extra: [halves(offset[0], offset[1]), halves(sigma, spread)], ..Self::DEFAULT
        }.radii(radii)
    }
    /// A butt-capped stroke from `a` to `b` (points) `width` points thick (`round_caps` rounds the ends, `dash` dashes it).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{kind, Instance};
    /// assert_eq!(Instance::line([0.0, 0.0], [5.0, 5.0], 1.0, [0, 0, 0, 255]).kind, kind::LINE);
    /// ```
    pub fn line(a: [f32; 2], b: [f32; 2], width: f32, color: [u8; 4]) -> Self {
        Instance { rect: [a[0], a[1], b[0], b[1]], color, width: f16(width), kind: kind::LINE, ..Self::DEFAULT }
    }
    /// A circular arc of `center` and `radius` (points) from angle `start` sweeping `sweep` radians (positive is clockwise on screen), `width` points thick.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{kind, Instance};
    /// let i = Instance::arc([10.0, 10.0], 8.0, 0.0, 3.14, 2.0, [0, 0, 0, 255]);
    /// assert_eq!(i.kind, kind::ARC);
    /// ```
    pub fn arc(center: [f32; 2], radius: f32, start: f32, sweep: f32, width: f32, color: [u8; 4]) -> Self {
        Instance {
            rect: [center[0], center[1], radius, 0.0], color, width: f16(width), kind: kind::ARC,
            extra: [start.to_bits(), sweep.to_bits()], ..Self::DEFAULT
        }
    }
    /// A filled triangle of corners `a`, `b`, `c` in points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{kind, Instance};
    /// let i = Instance::triangle([0.0, 0.0], [4.0, 0.0], [0.0, 4.0], [255; 4]);
    /// assert_eq!(i.kind, kind::TRIANGLE);
    /// ```
    pub fn triangle(a:[f32;2],b:[f32;2],c:[f32;2],color:[u8;4])->Self {
        Self {rect:[a[0],a[1],b[0],b[1]],extra:[c[0].to_bits(),c[1].to_bits()],color,kind:kind::TRIANGLE,..Self::DEFAULT}
    }
    /// A quadratic bezier from `a` through control point `control` to `b` (points), `width` points thick, round caps.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{kind, Instance};
    /// let i = Instance::curve([0.0, 0.0], [5.0, 8.0], [10.0, 0.0], 1.0, [0, 0, 0, 255]);
    /// assert_eq!(i.kind, kind::CURVE);
    /// ```
    pub fn curve(a: [f32; 2], control: [f32; 2], b: [f32; 2], width: f32, color: [u8; 4]) -> Self {
        Instance {
            rect: [a[0], a[1], b[0], b[1]], color, width: f16(width), kind: kind::CURVE,
            extra: [control[0].to_bits(), control[1].to_bits()], ..Self::DEFAULT
        }
    }
    /// Squares `dot` points wide every `spacing` points from `origin` (in both axes), over `rect`:
    /// the fragment stage draws the dot nearest each pixel exactly as a `rect` instance per dot would.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{kind, Instance};
    /// let i = Instance::grid([0.0, 0.0, 100.0, 100.0], [0.0, 0.0], 10.0, 2.0, [0, 0, 0, 255]);
    /// assert_eq!(i.kind, kind::GRID);
    /// ```
    pub fn grid(rect: [f32; 4], origin: [f32; 2], spacing: f32, dot: f32, color: [u8; 4]) -> Self {
        Instance { rect, color, kind: kind::GRID, aux: [spacing.to_bits(), dot.to_bits()], extra: [origin[0].to_bits(), origin[1].to_bits()], ..Self::DEFAULT }
    }
    /// The instance with corner radii (points) top-left, top-right, bottom-right, bottom-left, stored as half floats.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{f16, Instance};
    /// let i = Instance::rect([0.0; 4], [0; 4]).radii([4.0; 4]);
    /// assert_eq!(i.radii, [f16(4.0); 4]);
    /// ```
    pub fn radii(self, radii: [f32; 4]) -> Self { Instance { radii: radii.map(f16), ..self } }
    /// The instance with a border `width` points wide in `color`, drawn inside its rect (all sides unless `sides` narrows it).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::Instance;
    /// let i = Instance::rect([0.0, 0.0, 9.0, 9.0], [0; 4]).border(1.0, [255; 4]);
    /// assert_eq!(i.color2, [255; 4]);
    /// ```
    pub fn border(self, width: f32, color: [u8; 4]) -> Self { Instance { width: f16(width), color2: color, ..self } }
    /// Draw the border (see [`Instance::border`]) only on the sides in `sides` (`flags::rect::TOP | …`).
    ///
    /// Only bits 4 to 7 of `sides` are read; none set draws every side. The instance's low flag bits are kept.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{flags, Instance};
    /// let i = Instance::rect([0.0; 4], [0; 4]).border(1.0, [255; 4]).sides(flags::rect::TOP | flags::rect::LEFT);
    /// assert_eq!(i.flags, flags::rect::TOP | flags::rect::LEFT);
    /// ```
    pub fn sides(self, sides: u8) -> Self { Instance { flags: self.flags & 0x0f | sides & 0xf0, ..self } }
    fn gradient(self, kind: u8, geometry: [f32; 4], to: [u8; 4]) -> Self {
        Instance { flags: self.flags & !3 | kind, aux: [u32::from_le_bytes(to), 0], extra: [halves(geometry[0], geometry[1]), halves(geometry[2], geometry[3])], ..self }
    }
    /// Colour the gradient from row `row` of the colour atlas (see `pxui_render::ramp`) instead of the
    /// two inline stops; call after `linear`/`radial`/`conic`.
    ///
    /// The stored value is `row + 1` (0 means no ramp).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::Instance;
    /// let i = Instance::rect([0.0; 4], [0; 4]).linear([0.0, 0.0], [0.0, 1.0], [255; 4]).ramp(3);
    /// assert_eq!(i.aux[1], 4);
    /// ```
    pub fn ramp(self, row: u32) -> Self { Instance { aux: [self.aux[0], row + 1], ..self } }
    /// A linear gradient from the instance's colour at point `from` to `color` at point `to` (points).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{flags, Instance};
    /// let i = Instance::rect([0.0; 4], [0; 4]).linear([0.0, 0.0], [0.0, 10.0], [255; 4]);
    /// assert_eq!(i.flags & 3, flags::rect::LINEAR);
    /// ```
    pub fn linear(self, from: [f32; 2], to: [f32; 2], color: [u8; 4]) -> Self { self.gradient(flags::rect::LINEAR, [from[0], from[1], to[0], to[1]], color) }
    /// A radial gradient from the instance's colour at `center` out to `color` at `radius` (x and y radii, points).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{flags, Instance};
    /// let i = Instance::rect([0.0; 4], [0; 4]).radial([5.0, 5.0], [5.0, 5.0], [255; 4]);
    /// assert_eq!(i.flags & 3, flags::rect::RADIAL);
    /// ```
    pub fn radial(self, center: [f32; 2], radius: [f32; 2], color: [u8; 4]) -> Self { self.gradient(flags::rect::RADIAL, [center[0], center[1], radius[0], radius[1]], color) }
    /// A conic gradient round `center` (points) from the instance's colour to `color`, starting at `start_turns` (a fraction of a turn).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{flags, Instance};
    /// let i = Instance::rect([0.0; 4], [0; 4]).conic([5.0, 5.0], 0.25, [255; 4]);
    /// assert_eq!(i.flags & 3, flags::rect::CONIC);
    /// ```
    pub fn conic(self, center: [f32; 2], start_turns: f32, color: [u8; 4]) -> Self { self.gradient(flags::rect::CONIC, [center[0], center[1], start_turns, 0.0], color) }
    // curves have no dashes: specification/pxui.md, designed limits
    /// Dashes for lines and arcs: `on` points drawn, `off` points skipped, starting `phase` points into the pattern (curves do not dash).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::Instance;
    /// let i = Instance::line([0.0, 0.0], [20.0, 0.0], 1.0, [0, 0, 0, 255]).dash(4.0, 3.0, 0.0);
    /// assert_ne!(i.aux[0], 0);
    /// ```
    pub fn dash(self, on: f32, off: f32, phase: f32) -> Self { Instance { aux: [halves(on, off), halves(phase, 0.0)], ..self } }
    /// The instance with round ends (`flags::stroke::ROUND`) instead of butt ends.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::{flags, Instance};
    /// let i = Instance::line([0.0, 0.0], [5.0, 0.0], 2.0, [0; 4]).round_caps();
    /// assert_ne!(i.flags & flags::stroke::ROUND, 0);
    /// ```
    pub fn round_caps(self) -> Self { Instance { flags: self.flags | flags::stroke::ROUND, ..self } }
    /// The instance clipped to `rect` (`[left, top, right, bottom]` whole points) with corners rounded by `radius` points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::Instance;
    /// assert_eq!(Instance::rect([0.0; 4], [0; 4]).clip([0, 0, 10, 10], 0.0).clip, [0, 0, 10, 10]);
    /// ```
    pub fn clip(self, rect: [i16; 4], radius: f32) -> Self { Instance { clip: rect, clip_radius: f16(radius), ..self } }
    /// The instance drawn through view `view` of the layer's view table (its transform and opacity).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::draw::Instance;
    /// assert_eq!(Instance::rect([0.0; 4], [0; 4]).view(2).view, 2);
    /// ```
    pub fn view(self, view: u16) -> Self { Instance { view, ..self } }
}

#[cfg(test)]
mod tests {
    use super::flags::{image, mask, rect, shadow, stroke};

    /// The exact halves' shortcut gives the rounded conversion's bits for every value it takes (a sweep of f32 bit patterns,
    /// both signs, the half range's ends and the values just past them).
    #[test]
    fn exact_halves_convert_as_the_rounded_path_does() {
        for bits in (0..=u32::MAX).step_by(4099).chain([0x3880_0000, 0x477f_e000, 0x3880_2000, 0x477f_f000, 0x3800_0000, 0x4780_0000, 0, 0x8000_0000]) {
            let x = f32::from_bits(bits);
            assert_eq!(super::f16(x), super::f16_rounded(x), "{bits:#x}");
        }
    }

    /// Each kind's flags are distinct bits of its own, and the bits two kinds share are the ones the shaders read per kind.
    #[test]
    fn flags_are_per_kind_and_their_shared_bits_are_pinned() {
        let disjoint = |bits: &[u8]| bits.iter().enumerate().all(|(i, a)| bits[i + 1..].iter().all(|b| a & b == 0));
        // a rect: the gradient kind is a 2-bit number, then AREA and the four sides
        assert!(disjoint(&[rect::CONIC, rect::AREA, rect::TOP, rect::RIGHT, rect::BOTTOM, rect::LEFT]));
        assert_eq!((rect::LINEAR, rect::RADIAL, rect::CONIC), (1, 2, 3));
        assert!(disjoint(&[mask::GLYPH_PAGE_2, mask::LIGHT]) && disjoint(&[image::SMOOTH, image::GLYPH_PAGE_2]));
        // shared across kinds (never meaningful together: an instance has one kind)
        assert_eq!(image::SMOOTH, rect::LINEAR);
        assert_eq!([mask::GLYPH_PAGE_2, image::GLYPH_PAGE_2], [rect::RADIAL; 2]);
        assert_eq!(shadow::INNER, rect::AREA);
        assert_eq!(mask::LIGHT, rect::TOP);
        assert!(disjoint(&[stroke::ROUND, rect::CONIC, rect::AREA, rect::TOP]));
    }
}
