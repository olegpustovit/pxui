use crate::{Flags, Size};
use crate::a11y::Role;
use crate::geom::{Edges, Rgba as Color};
use crate::layout::{Align, Anchor, Cell, Grid, Justify};
use crate::theme::{Curve, Look, State};

/// An optional field of a node or a look with a C layout (`Option` has none): a byte tag (0 none, 1 some) then the value. `get`
/// reads it as an `Option`, `Option::into` writes one.
macro_rules! opt {
    ($(#[$doc:meta])* $name:ident($($t:ty),+) $get:ty, |$($v:ident),+| $some:expr, |$o:ident| $from:expr) => {
        $(#[$doc])*
        #[repr(C, u8)]
        #[derive(Clone, Copy, Debug, Default, PartialEq)]
        pub enum $name {
            #[default] None,
            Some($($t),+),
        }
        impl $name {
            #[inline] pub const fn get(self) -> Option<$get> { match self { Self::Some($($v),+) => Some($some), Self::None => None } }
            #[inline] pub const fn is_some(&self) -> bool { matches!(self, Self::Some(..)) }
            #[inline] pub const fn is_none(&self) -> bool { matches!(self, Self::None) }
        }
        impl From<Option<$get>> for $name {
            #[inline] fn from(value: Option<$get>) -> Self { match value { Some($o) => $from, None => Self::None } }
        }
    };
}
opt!(/** `Option<Align>` */ OptAlign(Align) Align, |v| v, |v| Self::Some(v));
opt!(/** `Option<Grid>` */ OptGrid(Grid) Grid, |v| v, |v| Self::Some(v));
opt!(/** `Option<Anchor>` */ OptAnchor(Anchor) Anchor, |v| v, |v| Self::Some(v));
opt!(/** `Option<(f32, f32)>`: a point */ OptPoint(f32, f32) (f32, f32), |x, y| (x, y), |v| Self::Some(v.0, v.1));
opt!(/** `Option<Bar>` */ OptBar(Bar) Bar, |v| v, |v| Self::Some(v));
/// The transition a node carries from `set_look` to its paint: a Bezier or a spring. A sampled table is 128 bytes, and kept
/// inline it made every `Node` copy 112 bytes longer, so `Ui::animate` and keyframes take tables and a node does not.
#[repr(C, u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transition {
    Bezier { duration: f32, points: [f32; 4] },
    Spring { omega: f32 },
}
impl Transition {
    /// The node-carriable `Transition` of `curve`: `Some` for a Bezier or Spring, `None` for a table or steps (a node does not carry those).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{theme::Curve, node::Transition};
    /// assert_eq!(Transition::of(Curve::Spring { omega: 12.0 }), Some(Transition::Spring { omega: 12.0 }));
    /// assert_eq!(Transition::of(Curve::Steps { duration: 1.0, count: 4 }), None);
    /// ```
    pub fn of(curve: Curve) -> Option<Self> {
        match curve { Curve::Bezier { duration, points } => Some(Self::Bezier { duration, points }), Curve::Spring { omega } => Some(Self::Spring { omega }), _ => None }
    }
    /// The `Curve` this transition runs, with the same parameters.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{theme::Curve, node::Transition};
    /// assert_eq!(Transition::Spring { omega: 12.0 }.curve(), Curve::Spring { omega: 12.0 });
    /// ```
    pub fn curve(self) -> Curve {
        match self { Self::Bezier { duration, points } => Curve::Bezier { duration, points }, Self::Spring { omega } => Curve::Spring { omega } }
    }
    /// Whether the parameters are usable (finite, a Bezier's x values in 0 to 1, a spring `omega` above 0), as `Curve::valid`; the ABI boundary refuses an invalid one.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::node::Transition;
    /// assert!(Transition::Spring { omega: 12.0 }.valid());
    /// assert!(!Transition::Spring { omega: 0.0 }.valid());
    /// ```
    #[inline]
    pub fn valid(&self) -> bool { match self { Self::Bezier { duration, points } => Curve::bezier_valid(*duration, points), Self::Spring { omega } => Curve::spring_valid(*omega) } }
}
opt!(/** `Option<Transition>` */ OptTransition(Transition) Transition, |v| v, |v| Self::Some(v));
opt!(/** `Option<Color>` */ OptColor(Color) Color, |v| v, |v| Self::Some(v));

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Row,
    Column,
    /// children overlap at the padding origin, in `z` order (equal `z`: build order); `justify`
    /// places them across, `align` down
    Stack,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Down,
    Up,
    Left,
    Right,
}

/// A drawn mark centred in a node's rectangle, in its look's text colour.
#[repr(C, u8)]
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
#[repr(C)]
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
/// change of state starts the theme's transition). The layout is C's (`pxui.h`'s `PxuiNode`): a node crosses the
/// library boundary as these bytes, checked where it lands, never converted.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Node {
    /// `size_of::<Node>()` (what `DEFAULT` holds): the library refuses a shorter record, so the struct can grow
    pub size: u32,
    // the byte-sized fields and `z`, back to back after `size`: the library tests the first 16 bytes of a node as two words
    pub axis: Axis,
    pub justify: Justify,
    pub align: Align,
    pub wrap: bool,
    /// what the node is to a screen reader (`Unknown`: not in the accessibility tree); `a11y` holds the states
    /// its widget declares (`a11y::state` bits: CHECKABLE, CHECKED, EXPANDABLE, EXPANDED, EDITABLE,
    /// MULTILINE, SELECTED, MODAL); focus, press, enabled and visibility come from the engine
    pub role: Role,
    pub state: State,
    pub z: i16,
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
    pub padding: Edges,
    pub margin: Edges,
    pub gap: f32,
    pub row_gap: f32,
    pub grid: OptGrid,
    pub cell: Cell,
    pub align_self: OptAlign,
    pub anchor: OptAnchor,
    pub a11y: u32,
    pub at: OptPoint,
    pub scroll_step: f32,
    pub look: Look,
    pub inset: Edges,
    pub mark: Mark,
    pub bar: OptBar,
    /// the transition `set_look` picked for a change of state, started when the node is painted
    pub transition: OptTransition,
}

impl Node {
    pub const DEFAULT: Node = Node {
        size: core::mem::size_of::<Node>() as u32,
        flags: Flags::NONE,
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
        align_self: OptAlign::None,
        wrap: false,
        grid: OptGrid::None,
        cell: Cell::DEFAULT,
        z: 0,
        anchor: OptAnchor::None,
        role: Role::Unknown,
        a11y: 0,
        at: OptPoint::None,
        scroll_step: 24.0,
        look: Look::NONE,
        inset: Edges {
            l: 0.0,
            t: 0.0,
            r: 0.0,
            b: 0.0,
        },
        mark: Mark::None,
        bar: OptBar::None,
        state: State {
            hovered: false,
            pressed: false,
            focused: false,
            disabled: false,
            checked: false,
        },
        transition: OptTransition::None,
    };

    /// A default node whose children flow as a row.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Axis, Node};
    /// const N: Node = Node::row();
    /// assert!(matches!(N.axis, Axis::Row));
    /// ```
    #[inline]
    pub const fn row() -> Node {
        Node { axis: Axis::Row, ..Node::DEFAULT }
    }
    /// A default node whose children flow as a column.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Axis, Node};
    /// const N: Node = Node::column();
    /// assert!(matches!(N.axis, Axis::Column));
    /// ```
    #[inline]
    pub const fn column() -> Node {
        Node { axis: Axis::Column, ..Node::DEFAULT }
    }
    /// A default node whose children flow as a stack.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Axis, Node};
    /// const N: Node = Node::stack();
    /// assert!(matches!(N.axis, Axis::Stack));
    /// ```
    #[inline]
    pub const fn stack() -> Node {
        Node { axis: Axis::Stack, ..Node::DEFAULT }
    }
    /// Sets the width rule.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Node, Size};
    /// const N: Node = Node::row().w(Size::Px(44.0));
    /// assert!(matches!(N.w, Size::Px(v) if v == 44.0));
    /// ```
    #[inline]
    pub const fn w(mut self, size: Size) -> Node {
        self.w = size;
        self
    }
    /// Sets the height rule.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Node, Size};
    /// const N: Node = Node::row().h(Size::Grow);
    /// assert!(N.h.is_grow());
    /// ```
    #[inline]
    pub const fn h(mut self, size: Size) -> Node {
        self.h = size;
        self
    }
    /// Sets the width to `points` fixed points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Node, Size};
    /// const N: Node = Node::row().w_px(44.0);
    /// assert!(matches!(N.w, Size::Px(v) if v == 44.0));
    /// ```
    #[inline]
    pub const fn w_px(mut self, points: f32) -> Node {
        self.w = Size::Px(points);
        self
    }
    /// Sets the height to `points` fixed points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Node, Size};
    /// const N: Node = Node::row().h_px(44.0);
    /// assert!(matches!(N.h, Size::Px(v) if v == 44.0));
    /// ```
    #[inline]
    pub const fn h_px(mut self, points: f32) -> Node {
        self.h = Size::Px(points);
        self
    }
    /// Sets the node's `grow` shares of the room its line leaves.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Node};
    /// const N: Node = Node::row().grow(2.0);
    /// assert_eq!(N.grow, 2.0);
    /// ```
    #[inline]
    pub const fn grow(mut self, share: f32) -> Node {
        self.grow = share;
        self
    }
    /// Sets the gap between children, in points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Node};
    /// const N: Node = Node::row().gap(8.0);
    /// assert_eq!(N.gap, 8.0);
    /// ```
    #[inline]
    pub const fn gap(mut self, points: f32) -> Node {
        self.gap = points;
        self
    }
    /// Sets the padding on all four edges, in points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Node;
    /// const N: Node = Node::row().pad(4.0);
    /// assert_eq!(N.padding.t, 4.0);
    /// ```
    #[inline]
    pub const fn pad(mut self, points: f32) -> Node {
        self.padding = Edges::all(points);
        self
    }
    /// Sets the left and right padding, in points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Node};
    /// const N: Node = Node::row().pad_x(4.0);
    /// assert_eq!((N.padding.l, N.padding.r, N.padding.t), (4.0, 4.0, 0.0));
    /// ```
    #[inline]
    pub const fn pad_x(mut self, points: f32) -> Node {
        self.padding.l = points;
        self.padding.r = points;
        self
    }
    /// Sets the top and bottom padding, in points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Node};
    /// const N: Node = Node::row().pad_y(4.0);
    /// assert_eq!((N.padding.t, N.padding.b, N.padding.l), (4.0, 4.0, 0.0));
    /// ```
    #[inline]
    pub const fn pad_y(mut self, points: f32) -> Node {
        self.padding.t = points;
        self.padding.b = points;
        self
    }
    /// Centres children along and across the axis.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::layout::{Align, Justify};
    /// use pxui_types::Node;
    /// const N: Node = Node::row().center();
    /// assert!(matches!((N.justify, N.align), (Justify::Center, Align::Center)));
    /// ```
    #[inline]
    pub const fn center(mut self) -> Node {
        self.justify = Justify::Center;
        self.align = Align::Center;
        self
    }
    /// Sets where children sit across the axis.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{layout::Align, Node};
    /// const N: Node = Node::row().align(Align::End);
    /// assert!(matches!(N.align, Align::End));
    /// ```
    #[inline]
    pub const fn align(mut self, align: Align) -> Node {
        self.align = align;
        self
    }
    /// Sets what the node is to a screen reader.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{a11y::Role, Node};
    /// const N: Node = Node::row().role(Role::Button);
    /// assert!(matches!(N.role, Role::Button));
    /// ```
    #[inline]
    pub const fn role(mut self, role: Role) -> Node {
        self.role = role;
        self
    }
    /// Sets the node's flag bits.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Flags, Node};
    /// const N: Node = Node::row().flags(Flags::CLICKABLE.or(Flags::CLIP));
    /// assert_eq!(N.flags.bits(), 9);
    /// ```
    #[inline]
    pub const fn flags(mut self, flags: Flags) -> Node {
        self.flags = flags;
        self
    }
}
