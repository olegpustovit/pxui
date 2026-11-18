//! Layout data a node carries (plain `Copy` values, no allocation) and the pure placement rule of
//! anchored popups. The engine reads them during intrinsic sizing and placement.
use crate::geom::Rect;

/// Where children sit along the main axis (CSS `justify-content`).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Justify {
    #[default]
    /// Children pack at the start of the main axis.
    Start,
    /// Children pack in the middle.
    Center,
    /// Children pack at the end.
    End,
    /// Free room goes between children; the first and last touch the edges.
    SpaceBetween,
    /// Free room goes around each child: half a share at the ends, a full share between.
    SpaceAround,
    /// Free room is split into equal shares between children and at both ends.
    SpaceEvenly,
}

/// Where a child sits across its line (CSS `align-items`, `align-self`).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    /// The child sits at the start of the cross axis.
    Start,
    /// The child is centred across the line.
    Center,
    /// The child sits at the end of the cross axis.
    End,
    /// a `Fit` cross size fills the line (a `Grow` one always does)
    Stretch,
    /// the first text baselines line up (a node with no text: its bottom edge)
    Baseline,
}

/// Where a thing sits when stretching and baselines do not apply: a popup along its anchor's edge (`Placement::align`), a
/// button's contents across its box (`Button::content_align`). Its numbers are those of `Align::Start`, `Center` and `End`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Along {
    #[default]
    /// At the start (left, or top).
    Start,
    /// Centred.
    Center,
    /// At the end (right, or bottom).
    End,
}

/// Row flow direction: `Rtl` mirrors flow children inside their parent (`Edges::l` is the start edge,
/// `Justify::Start` the right). `at` offsets stay physical.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    /// Left to right: flow children start at the left.
    Ltr,
    /// Right to left: flow children start at the right.
    Rtl,
}

/// A grid track: fixed points, a share of the room left (`fr`), or its content: its largest
/// single-span child, plus an equal share of what a spanning child needs past its tracks.
#[repr(C, u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Track {
    /// A fixed size in logical points.
    Px(f32),
    /// A share of the room left after fixed and content tracks, in proportion to the number.
    Fr(f32),
    /// The size of its content.
    Fit,
}

/// Grid tracks. `rows` empty: rows are added as children need them, each sized to its content.
/// Gaps are the node's `gap` (between columns) and `row_gap`.
// Columns: at most MAX_TRACKS (stack arrays in the passes); rows: as many as the children reach (a
// scratch the Ui keeps, no allocation once warm). Spans are spread in build order, not smallest span
// first as CSS.
// A pointer and a length each, not slices, so that a node is the same bytes in C; only `Grid::new` makes one, from 'static
// slices, so the accessors are sound (the library replaces a C caller's tracks by its own copies before the engine sees them).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Grid {
    cols: *const Track,
    cols_len: usize,
    rows: *const Track,
    rows_len: usize,
}

impl Grid {
    /// Grid tracks from `'static` column and row lists (`rows` may be empty: rows are then added as children need them).
    ///
    /// Only `'static` slices are accepted: the engine keeps the node, and these addresses, past the call (layout runs after the
    /// build, and an unchanged node is compared with the next frame's), so tracks are `const`s or statics (`const COLS: [Track; 2]`).
    /// A C caller's tracks, which can be temporary, are copied by the library instead. At most `MAX_TRACKS` columns are used.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::layout::{Grid, Track};
    /// const G: Grid = Grid::new(&[Track::Px(40.0), Track::Fr(1.0)], &[]);
    /// assert_eq!(G.cols().len(), 2);
    /// assert!(G.rows().is_empty());
    /// ```
    pub const fn new(cols: &'static [Track], rows: &'static [Track]) -> Grid {
        Grid { cols: cols.as_ptr(), cols_len: cols.len(), rows: rows.as_ptr(), rows_len: rows.len() }
    }
    /// The column tracks, left to right.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::layout::{Grid, Track};
    /// assert_eq!(Grid::new(&[Track::Fit], &[]).cols(), &[Track::Fit]);
    /// ```
    #[inline]
    pub fn cols(&self) -> &'static [Track] { unsafe { core::slice::from_raw_parts(self.cols, self.cols_len) } }
    /// The explicit row tracks, top to bottom; empty when rows are sized by their content as children reach them.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::layout::{Grid, Track};
    /// assert_eq!(Grid::new(&[], &[Track::Px(8.0)]).rows(), &[Track::Px(8.0)]);
    /// ```
    #[inline]
    pub fn rows(&self) -> &'static [Track] { unsafe { core::slice::from_raw_parts(self.rows, self.rows_len) } }
    /// The tracks as the C caller wrote them, not yet trusted: (columns pointer, length, rows pointer, length).
    ///
    /// The pointers are only valid for the lengths given, and only as long as the caller of the C API keeps them; the library copies them first.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::layout::{Grid, Track};
    /// let (_, cols, _, rows) = Grid::new(&[Track::Fit, Track::Fit], &[]).raw();
    /// assert_eq!((cols, rows), (2, 0));
    /// ```
    pub fn raw(&self) -> (*const Track, usize, *const Track, usize) { (self.cols, self.cols_len, self.rows, self.rows_len) }
}

impl PartialEq for Grid {
    fn eq(&self, other: &Grid) -> bool { self.cols() == other.cols() && self.rows() == other.rows() }
}

/// The most columns a grid uses.
pub const MAX_TRACKS: usize = 64;

/// A grid child's cell. `col == AUTO` places it at the next free cell in row order.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Zero-based column of the cell's first track, or `Cell::AUTO`.
    pub col: u16,
    /// Zero-based row of the cell's first track.
    pub row: u16,
    /// Columns covered (1 or more).
    pub col_span: u16,
    /// Rows covered (1 or more).
    pub row_span: u16,
}

impl Cell {
    /// The `col` value that means "next free cell in row order".
    pub const AUTO: u16 = u16::MAX;
    /// Automatic placement, one column and one row.
    pub const DEFAULT: Cell = Cell {
        col: Cell::AUTO,
        row: 0,
        col_span: 1,
        row_span: 1,
    };
    /// A one-by-one cell at column `col`, row `row` (zero-based).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::layout::Cell;
    /// assert_eq!(Cell::at(2, 1), Cell { col: 2, row: 1, col_span: 1, row_span: 1 });
    /// ```
    pub const fn at(col: u16, row: u16) -> Cell {
        Cell {
            col,
            row,
            col_span: 1,
            row_span: 1,
        }
    }
    /// This cell covering `col_span` columns and `row_span` rows from where it starts (neither is checked for zero or for the grid's size).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::layout::Cell;
    /// assert_eq!(Cell::at(0, 0).span(2, 3).col_span, 2);
    /// ```
    pub const fn span(self, col_span: u16, row_span: u16) -> Cell {
        Cell {
            col_span,
            row_span,
            ..self
        }
    }
}

/// Which side of its anchor a popup opens on.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Under the anchor.
    Below,
    /// Over the anchor.
    Above,
    /// To the right of the anchor.
    Right,
    /// To the left of the anchor.
    Left,
}

/// How a popup sits by its anchor: on `side`, `gap` points off it, `align`ed along it, kept `margin` points inside the bounds. With `flip` it opens on the opposite side when its
/// own does not fit and that one does; without, it is moved inside the bounds.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// The side of the anchor the popup opens on.
    pub side: Side,
    /// Where the popup sits along the anchor's edge.
    pub align: Along,
    /// Points between the anchor and the popup.
    pub gap: f32,
    /// Points the popup keeps inside the bounds.
    pub margin: f32,
    /// Open on the opposite side when the chosen one does not fit.
    pub flip: bool,
}

impl Placement {
    /// Opens under the anchor, left edges aligned, no gap or margin, flipping above when it does not fit.
    pub const BELOW: Placement = Placement {
        side: Side::Below,
        align: Along::Start,
        gap: 0.0,
        margin: 0.0,
        flip: true,
    };
}

/// A node placed by an anchor rectangle (window points) rather than its parent's flow.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    /// The anchor rectangle in window points.
    pub rect: Rect,
    /// How the node sits by it.
    pub placement: Placement,
}

impl Anchor {
    /// A node placed by `rect` (window points) as `placement` says.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Rect, layout::{Anchor, Placement}};
    /// const BY: Anchor = Anchor::new(Rect::new(10.0, 10.0, 40.0, 20.0), Placement::BELOW);
    /// assert_eq!(BY.placement, Placement::BELOW);
    /// ```
    #[inline]
    pub const fn new(rect: Rect, placement: Placement) -> Anchor { Anchor { rect, placement } }
}

/// The rectangle a popup of `size` (width, height in points) takes by `anchor` inside `bounds` under placement `p`.
///
/// All rectangles share one coordinate space (window points). With `p.flip` the popup moves to the opposite side whenever its own side
/// overflows `bounds`, without checking that the opposite side fits; the result is then pulled inside `bounds` less `p.margin`
/// on both axes, and a popup larger than the room ends up at the bounds' top-left plus the margin.
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::layout::{place_anchored, Placement};
/// use pxui_types::geom::Rect;
/// let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
/// let below = place_anchored(Rect::new(10.0, 10.0, 20.0, 10.0), (30.0, 20.0), Placement::BELOW, bounds);
/// assert_eq!((below.x, below.y), (10.0, 20.0));
/// let flipped = place_anchored(Rect::new(10.0, 80.0, 20.0, 10.0), (30.0, 20.0), Placement::BELOW, bounds);
/// assert_eq!(flipped.y, 60.0);
/// ```
pub fn place_anchored(anchor: Rect, size: (f32, f32), p: Placement, bounds: Rect) -> Rect {
    let (w, h) = size;
    let m = p.margin;
    let vertical = matches!(p.side, Side::Below | Side::Above);
    // along the side: the side itself, else the opposite one when it fits and this does not
    let at = |side: Side| match side {
        Side::Below => anchor.y + anchor.h + p.gap,
        Side::Above => anchor.y - p.gap - h,
        Side::Right => anchor.x + anchor.w + p.gap,
        Side::Left => anchor.x - p.gap - w,
    };
    let fits = |side: Side, v: f32| match side {
        Side::Below => v + h <= bounds.y + bounds.h - m,
        Side::Above => v >= bounds.y + m,
        Side::Right => v + w <= bounds.x + bounds.w - m,
        Side::Left => v >= bounds.x + m,
    };
    let opposite = match p.side {
        Side::Below => Side::Above,
        Side::Above => Side::Below,
        Side::Right => Side::Left,
        Side::Left => Side::Right,
    };
    let mut main = at(p.side);
    if p.flip && !fits(p.side, main) {
        main = at(opposite);
    }
    let (lo, len, extent) = if vertical {
        (anchor.x, anchor.w, w)
    } else {
        (anchor.y, anchor.h, h)
    };
    let cross = match p.align {
        Along::Start => lo,
        Along::Center => lo + (len - extent) / 2.0,
        Along::End => lo + len - extent,
    };
    let clamp = |v: f32, start: f32, room: f32, extent: f32| {
        v.min(start + room - m - extent).max(start + m)
    };
    let (x, y) = if vertical {
        (cross, main)
    } else {
        (main, cross)
    };
    Rect::new(
        clamp(x, bounds.x, bounds.w, w),
        clamp(y, bounds.y, bounds.h, h),
        w,
        h,
    )
}
