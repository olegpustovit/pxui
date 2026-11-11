use crate::{Flags, Size};
use crate::a11y::Role;
use crate::geom::{Edges, Rgba as Color};
use crate::layout::{Align, Anchor, Cell, Grid, Justify};
use crate::theme::{Curve, Look, State};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Row,
    Column,
    /// children overlap at the padding origin, in `z` order (equal `z`: build order); `justify`
    /// places them across, `align` down
    Stack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Down,
    Up,
    Left,
    Right,
}

/// A drawn mark centred in a node's rectangle, in its look's text colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mark {
    None,
    Chevron(Dir),
    /// a line `width` thick in the look's text colour across the node's rectangle, between its
    /// corners inset by half the width: top left to bottom right (`rising`: bottom left to top
    /// right); `patch_rect` moves its ends, `patch_mark` turns it over, `patch_look` colours it
    Line {
        width: f32,
        rising: bool,
    },
}

/// A scroll node's thumb: `width` points wide, `gap` from the right edge, between `margin` points of
/// the top and bottom; `whole` keeps it on whole points (a panel's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub color: Color,
    pub radius: f32,
    pub width: f32,
    pub gap: f32,
    pub margin: f32,
    pub whole: bool,
}

/// A node's layout inputs and look. Sizes: `w`, `h` clamped to `min_*`/`max_*` (min wins); `aspect`
/// (w / h, 0 none) derives a `Fit` side from the other; a `Grow` size takes `grow` shares of the room
/// its line leaves, and when the line overflows a child gives up room by `shrink` times its size.
/// Children: `axis` (row, column or stack), or a `grid`; `padding` inside, `margin` outside (`l` is
/// the start edge), `gap` between children (`row_gap` between wrapped lines and grid rows), `justify`
/// along the axis, `align` across it (`align_self` overrides it for this node), `wrap` breaks a
/// flow into lines; `cell` is its place in a parent grid, `z` its order in a stack. `at` places it
/// outside the flow at an offset from the parent's origin, `anchor` by a window rectangle (a popup).
/// `inset` is where its text sits in it; `mark` is drawn
/// over its text; `bar` gives a scroll node its thumb; `state` is what the look was picked for (a
/// change of state starts the theme's transition).
#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub flags: Flags,
    pub w: Size,
    pub h: Size,
    pub min_w: f32,
    pub min_h: f32,
    pub max_w: f32,
    pub max_h: f32,
    pub grow: f32,
    pub shrink: f32,
    pub aspect: f32,
    pub axis: Axis,
    pub padding: Edges,
    pub margin: Edges,
    pub gap: f32,
    pub row_gap: f32,
    pub justify: Justify,
    pub align: Align,
    pub align_self: Option<Align>,
    pub wrap: bool,
    pub grid: Option<Grid>,
    pub cell: Cell,
    pub z: i16,
    pub anchor: Option<Anchor>,
    /// what the node is to a screen reader (`Unknown`: not in the accessibility tree) and the states
    /// its widget declares (`a11y::state` bits: CHECKABLE, CHECKED, EXPANDABLE, EXPANDED, EDITABLE,
    /// MULTILINE, SELECTED, MODAL); focus, press, enabled and visibility come from the engine
    pub role: Role,
    pub a11y: u32,
    pub at: Option<(f32, f32)>,
    pub scroll_step: f32,
    pub look: Look,
    pub inset: Edges,
    pub mark: Mark,
    pub bar: Option<Bar>,
    pub state: State,
    /// the transition `set_look` picked for a change of state, started when the node is painted
    pub transition: Option<Curve>,
}

impl Node {
    pub const DEFAULT: Node = Node {
        flags: 0,
        w: Size::Grow,
        h: Size::Fit,
        min_w: 0.0,
        min_h: 0.0,
        max_w: f32::INFINITY,
        max_h: f32::INFINITY,
        grow: 1.0,
        shrink: 0.0,
        aspect: 0.0,
        axis: Axis::Column,
        padding: Edges::ZERO,
        margin: Edges::ZERO,
        gap: 0.0,
        row_gap: 0.0,
        justify: Justify::Start,
        align: Align::Start,
        align_self: None,
        wrap: false,
        grid: None,
        cell: Cell::DEFAULT,
        z: 0,
        anchor: None,
        role: Role::Unknown,
        a11y: 0,
        at: None,
        scroll_step: 24.0,
        look: Look::NONE,
        inset: Edges {
            l: 0.0,
            t: 0.0,
            r: 0.0,
            b: 0.0,
        },
        mark: Mark::None,
        bar: None,
        state: State {
            hovered: false,
            pressed: false,
            focused: false,
            disabled: false,
            checked: false,
        },
        transition: None,
    };
}
