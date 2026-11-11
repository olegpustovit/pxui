//! The shapes a painter draws without text: rectangles, strokes, lines, circles, dot grids, chevrons, brackets, dashes and
//! flags, built into 64-byte instances. One definition: the engine's `Paint` and the wrapper's span writer both implement
//! `Shapes` (a push, the display density, the theme tokens and the clip) and get every shape from it, so the instances
//! they produce are the same bytes.
use crate::draw::Instance;
use crate::geom::{color, Rect, Rgba as Color};
use crate::node::Dir;
use crate::theme::{self, Tokens};

/// The instance clip of `rect`: whole points covering it.
pub fn clip_of(Rect { x, y, w, h }: Rect) -> [i16; 4] {
    let whole = |v: f32| v.clamp(i16::MIN as f32, i16::MAX as f32) as i16;
    [whole(x.floor()), whole(y.floor()), whole((x + w).ceil()), whole((y + h).ceil())]
}

fn rgba(c: Color) -> [u8; 4] {
    [c.r, c.g, c.b, c.a]
}

/// A window into a layer's instance storage that shapes are written into directly, one 64-byte store each, stamped with
/// the clip, clip radius and view table index the library gave. The writer advances `at`; when `at == end` it asks
/// the library for the next window (`pxui_paint_span`), which also commits what was written.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub size: u32,
    pub view: u16,
    pub clip_radius_f16: u16,
    pub clip: [i16; 4],
    pub at: *mut Instance,
    pub end: *mut Instance,
}

impl Span {
    pub const EMPTY: Span = Span { size: core::mem::size_of::<Span>() as u32, view: 0, clip_radius_f16: 0, clip: [0; 4], at: core::ptr::null_mut(), end: core::ptr::null_mut() };

    /// Room for this many more instances.
    #[inline]
    pub fn room(&self) -> usize {
        (self.end as usize - self.at as usize) / core::mem::size_of::<Instance>()
    }

    /// Stamps `instance` with the span's clip and view and stores it at `at`; false when the span is full.
    #[inline]
    pub fn write(&mut self, instance: Instance) -> bool {
        if self.at == self.end {
            return false;
        }
        // SAFETY: the library handed out `at..end` as writable instance storage and `at < end` here.
        unsafe {
            self.at.write(Instance { clip: self.clip, clip_radius: self.clip_radius_f16, view: self.view, ..instance });
            self.at = self.at.add(1);
        }
        true
    }
}

pub trait Shapes {
    /// Appends `instance` under the current clip and view.
    fn push(&mut self, instance: Instance);
    /// Device pixels per point.
    fn density(&self) -> i64;
    fn theme(&self) -> Tokens;
    /// The clip rectangle shapes are cut to.
    fn clip_rect(&self) -> Rect;
    fn set_clip_rect(&mut self, clip: Rect);

    /// An edge at `v` points as the rasteriser covers pixel centres (`a <= centre < b`): snapped
    /// to the pixel grid, so hard-edged fills keep the pixels a triangle rectangle covers.
    fn snap(&self, v: f32) -> f32 {
        let d = self.density() as f32;
        (v * d - 0.5).ceil() / d
    }

    fn hard(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: [u8; 4]) {
        let r = [self.snap(x0), self.snap(y0), self.snap(x1), self.snap(y1)];
        if r[2] > r[0] && r[3] > r[1] {
            self.push(Instance::rect(r, color));
        }
    }

    /// Bands `width` wide inside `r` on `sides` (0: all four, the stroke `frame_w` draws).
    fn bands(&mut self, Rect { x, y, w, h }: Rect, width: f32, sides: u8, color: Color) {
        use crate::draw::flags::{BOTTOM, LEFT, RIGHT, TOP};
        if sides & 0xf0 == 0 || sides & 0xf0 == 0xf0 {
            self.frame_w(x, y, w, h, width, color);
            return;
        }
        if sides & TOP != 0 {
            self.fill(x, y, w, width, color);
        }
        if sides & RIGHT != 0 {
            self.fill(x + w - width, y, width, h, color);
        }
        if sides & BOTTOM != 0 {
            self.fill(x, y + h - width, w, width, color);
        }
        if sides & LEFT != 0 {
            self.fill(x, y, width, h, color);
        }
    }

    /// Square fills cover exactly the pixels of the equivalent triangle rectangle.
    fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        if w > 0.0 && h > 0.0 {
            self.hard(x, y, x + w, y + h, rgba(color));
        }
    }

    /// A band of `width` centred on the rectangle's edges.
    fn stroke_w(&mut self, x: f32, y: f32, w: f32, h: f32, width: f32, color: Color) {
        if !(w > 0.0 && h > 0.0) {
            return;
        }
        let half = width / 2.0;
        let c = rgba(color);
        let o = [self.snap(x - half), self.snap(y - half), self.snap(x + w + half), self.snap(y + h + half)];
        let i = [self.snap(x + half), self.snap(y + half), self.snap(x + w - half), self.snap(y + h - half)];
        if o[2] <= o[0] || o[3] <= o[1] {
            return;
        }
        let sides = [i[0] - o[0], i[1] - o[1], o[2] - i[2], o[3] - i[3]];
        if i[2] <= i[0] || i[3] <= i[1] {
            self.push(Instance::rect(o, c));
        } else if sides.iter().all(|side| *side == sides[0]) {
            self.push(Instance::rect(o, [0; 4]).border(sides[0], c));
        } else {
            self.hard(o[0], o[1], o[2], i[1], c);
            self.hard(o[0], i[3], o[2], o[3], c);
            self.hard(o[0], i[1], i[0], i[3], c);
            self.hard(i[2], i[1], o[2], i[3], c);
        }
    }

    fn stroke(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.stroke_w(x, y, w, h, 1.0, color);
    }

    /// A border drawn inside its box, as CSS draws one: `stroke` centres its line on the
    /// rectangle's edge, so a box of whole points gets the line half a width in; the border then
    /// covers whole points.
    fn frame_w(&mut self, x: f32, y: f32, w: f32, h: f32, width: f32, color: Color) {
        let half = width / 2.0;
        self.stroke_w(x + half, y + half, w - width, h - width, width, color);
    }

    fn frame(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.frame_w(x, y, w, h, 1.0, color);
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, fill: Option<Color>, stroke: Option<Color>) {
        let fill = match (fill, stroke) {
            (None, None) => Some(color::white()),
            _ => fill,
        };
        if let Some(c) = fill {
            self.fill(x, y, w, h, c);
        }
        if let Some(c) = stroke {
            self.stroke(x, y, w, h, c);
        }
    }

    /// Butt-capped; axis-aligned lines are exact rectangles.
    fn line_w(&mut self, from: (f32, f32), to: (f32, f32), width: f32, color: Color) {
        let (x0, y0) = from;
        let (x1, y1) = to;
        let half = width / 2.0;
        if x0 == x1 {
            self.fill(x0 - half, y0.min(y1), width, (y1 - y0).abs(), color);
        } else if y0 == y1 {
            self.fill(x0.min(x1), y0 - half, (x1 - x0).abs(), width, color);
        } else {
            self.push(Instance::line([x0, y0], [x1, y1], width, rgba(color)));
        }
    }

    fn line(&mut self, from: (f32, f32), to: (f32, f32), color: Color) {
        self.line_w(from, to, 1.0, color);
    }

    /// A disc with a one-point stroke centred on its edge.
    fn circle(&mut self, at: (f32, f32), radius: f32, fill: Option<Color>, stroke: Option<Color>) {
        if !(radius > 0.0) {
            return;
        }
        let fill = match (fill, stroke) {
            (None, None) => Some(color::white()),
            _ => fill,
        };
        let fill = fill.map(rgba).unwrap_or([0; 4]);
        let (cx, cy) = at;
        self.push(match stroke {
            None => Instance::rect([cx - radius, cy - radius, cx + radius, cy + radius], fill).radii([radius; 4]),
            Some(stroke) => {
                let r = radius + 0.5;
                Instance::rect([cx - r, cy - r, cx + r, cy + r], fill).radii([r; 4]).border(1.0, rgba(stroke))
            }
        });
    }

    /// `dot`-point squares every `spacing` points from `origin`, inside the rectangle: one instance
    /// (`kind::GRID`) whatever the number of dots.
    fn grid(&mut self, x: f32, y: f32, w: f32, h: f32, origin: (f32, f32), spacing: f32, dot: f32, color: Color) {
        if !(w > 0.0 && h > 0.0 && spacing > 0.0) {
            return;
        }
        let saved = self.clip_rect();
        self.set_clip_rect(saved.intersect(Rect::new(x, y, w, h)));
        self.push(Instance::grid([x, y, x + w, y + h], [origin.0, origin.1], spacing, dot, rgba(color)));
        self.set_clip_rect(saved);
    }

    /// A chevron centred on `at`: its strokes span 7 x 3.5, which with the 1-point line is the
    /// sheet's 9 x 5.5 of ink.
    fn chevron(&mut self, at: (f32, f32), direction: Dir, color: Color) {
        let (a, b) = match direction {
            Dir::Down => ((-3.5, -1.75), (3.5, -1.75)),
            Dir::Up => ((-3.5, 1.75), (3.5, 1.75)),
            Dir::Right => ((-1.75, -3.5), (-1.75, 3.5)),
            Dir::Left => ((1.75, -3.5), (1.75, 3.5)),
        };
        let tip = match direction {
            Dir::Down => (0.0, 1.75),
            Dir::Up => (0.0, -1.75),
            Dir::Right => (1.75, 0.0),
            Dir::Left => (-1.75, 0.0),
        };
        let p = |d: (f32, f32)| (at.0 + d.0, at.1 + d.1);
        self.line(p(a), p(tip), color);
        self.line(p(tip), p(b), color);
    }

    /// Selection: four corner brackets `offset` outside the rectangle.
    fn brackets(&mut self, x: f32, y: f32, w: f32, h: f32, offset: f32, length: f32, width: f32, color: Color) {
        let x = x - offset;
        let y = y - offset;
        let w = w + 2.0 * offset;
        let h = h + 2.0 * offset;
        for (cx, cy, sx, sy) in [(x, y, 1.0, 1.0), (x + w, y, -1.0, 1.0), (x, y + h, 1.0, -1.0), (x + w, y + h, -1.0, -1.0)] {
            self.fill(
                if sx > 0.0 { cx } else { cx - length },
                if sy > 0.0 { cy } else { cy - width },
                length,
                width,
                color,
            );
            self.fill(
                if sx > 0.0 { cx } else { cx - width },
                if sy > 0.0 { cy } else { cy - length },
                width,
                length,
                color,
            );
        }
    }

    fn dashed_w(&mut self, from: (f32, f32), to: (f32, f32), width: f32, color: Color) {
        let (x0, y0) = from;
        let (x1, y1) = to;
        let length = (x1 - x0).hypot(y1 - y0);
        let steps = (length / 7.0).ceil() as i64;
        for step in 0..steps {
            let a = step as f32 * 7.0 / length;
            let b = 1.0f32.min((step as f32 * 7.0 + 4.0) / length);
            self.line_w((x0 + (x1 - x0) * a, y0 + (y1 - y0) * a), (x0 + (x1 - x0) * b, y0 + (y1 - y0) * b), width, color);
        }
    }

    fn dashed(&mut self, from: (f32, f32), to: (f32, f32), color: Color) {
        self.dashed_w(from, to, 1.0, color);
    }

    /// 4 on, 3 off: a zone, a drop target.
    fn dashed_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.dashed((x, y), (x + w, y), color);
        self.dashed((x, y + h), (x + w, y + h), color);
        self.dashed((x, y), (x, y + h), color);
        self.dashed((x + w, y), (x + w, y + h), color);
    }

    /// An outline flag centred at `at`: 12 points, the input fill, a line-3 edge inside the box
    /// and, when on, a 6-point mark in ink-2; square, or `round` for the columns after the first.
    fn flag(&mut self, at: (f32, f32), round: bool, on: bool) {
        let theme = self.theme();
        let (cx, cy) = at;
        if round {
            self.circle((cx, cy), 5.5, Some(theme.input), Some(theme::border(&theme)));
            if on {
                self.circle((cx, cy), 3.0, Some(theme::ink_2(&theme)), None);
            }
        } else {
            self.fill(cx - 6.0, cy - 6.0, 12.0, 12.0, theme.input);
            self.frame(cx - 6.0, cy - 6.0, 12.0, 12.0, theme::border(&theme));
            if on {
                self.fill(cx - 3.0, cy - 3.0, 6.0, 6.0, theme::ink_2(&theme));
            }
        }
    }
}
