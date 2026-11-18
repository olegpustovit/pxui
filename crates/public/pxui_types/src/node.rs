use crate::{Flags, Size};
use crate::a11y::Role;
use crate::geom::{Edges, Rgba};
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
/// The transition a node carries from `style` to its paint: a Bezier or a spring. A sampled table is 128 bytes, and kept
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
opt!(/** `Option<Rgba>` */ OptColor(Rgba) Rgba, |v| v, |v| Self::Some(v));

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
    /// The thumb's colour.
    pub color: Rgba,
    /// Corner radius in points.
    pub radius: f32,
    /// Width in points.
    pub width: f32,
    /// Points from the node's right edge.
    pub gap: f32,
    /// Points kept clear at the top and bottom.
    pub margin: f32,
    /// Keep the thumb on whole points.
    pub whole: bool,
}

impl Bar {
    /// A square thumb of `color`, `width` points wide, against the right edge, running the full height.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Rgba, Bar};
    /// const THUMB: Bar = Bar::new(Rgba::gray(128), 4.0);
    /// assert_eq!((THUMB.width, THUMB.gap, THUMB.whole), (4.0, 0.0, false));
    /// ```
    #[inline]
    pub const fn new(color: Rgba, width: f32) -> Bar { Bar { color, radius: 0.0, width, gap: 0.0, margin: 0.0, whole: false } }
    /// Sets the corner radius in points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Rgba, Bar};
    /// const THUMB: Bar = Bar::new(Rgba::BLACK, 4.0).radius(2.0);
    /// assert!(THUMB.radius == 2.0);
    /// ```
    #[inline]
    pub const fn radius(mut self, points: f32) -> Bar {
        self.radius = points;
        self
    }
    /// Sets the points between the thumb and the node's right edge.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Rgba, Bar};
    /// const THUMB: Bar = Bar::new(Rgba::BLACK, 4.0).gap(2.0);
    /// assert!(THUMB.gap == 2.0);
    /// ```
    #[inline]
    pub const fn gap(mut self, points: f32) -> Bar {
        self.gap = points;
        self
    }
    /// Sets the points kept clear at the top and bottom.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Rgba, Bar};
    /// const THUMB: Bar = Bar::new(Rgba::BLACK, 4.0).margin(6.0);
    /// assert!(THUMB.margin == 6.0);
    /// ```
    #[inline]
    pub const fn margin(mut self, points: f32) -> Bar {
        self.margin = points;
        self
    }
    /// Keeps the thumb on whole points (a panel's).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Rgba, Bar};
    /// const THUMB: Bar = Bar::new(Rgba::BLACK, 4.0).whole();
    /// assert!(THUMB.whole);
    /// ```
    #[inline]
    pub const fn whole(mut self) -> Bar {
        self.whole = true;
        self
    }
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
///
/// Build one with the const builders from `Node::DEFAULT`, `Node::row()`, `Node::column()` or `Node::stack()`
/// (`const ROW: Node = Node::row().h_px(44.0).gap(8.0).center();`); the fields are public only because the record is C's.
/// `state` and `transition` are written by the engine (`style`), and `size` by `DEFAULT`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Node {
    /// `size_of::<Node>()` (what `DEFAULT` holds): the library refuses a shorter record, so the struct can grow; never set it.
    #[doc(hidden)]
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
    /// the transition `style` picked for a change of state, started when the node is painted
    pub transition: OptTransition,
}

impl Node {
    /// Sets the declared interaction state.
    ///
    /// The engine uses it when choosing interaction transitions.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Node, theme::State};
    /// let _ = Node::DEFAULT.state(State::default().checked(true));
    /// ```
    pub const fn state(self, state: State) -> Node { Node { state, ..self } }

    /// Sets or clears the node's transition.
    ///
    /// The curve is checked when the node is built.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Node, node::Transition};
    /// let _ = Node::DEFAULT.transition(Some(Transition::Spring { omega: 12.0 }));
    /// ```
    pub const fn transition(self, transition: Option<Transition>) -> Node {
        let transition = match transition { Some(curve) => OptTransition::Some(curve), None => OptTransition::None };
        Node { transition, ..self }
    }

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
    /// use pxui_types::Node;
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
    /// use pxui_types::Node;
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
    /// use pxui_types::Node;
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
    /// use pxui_types::Node;
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
    /// Sets how children flow: `Axis::Row`, `Column` or `Stack` (`Node::row()` and its siblings start from one).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Node;
    /// const V: Node = Node::DEFAULT.axis(pxui_types::Axis::Row);
    /// assert!(matches!(V.axis, pxui_types::Axis::Row));
    /// ```
    #[inline]
    pub const fn axis(mut self, axis: Axis) -> Node {
        self.axis = axis;
        self
    }
    /// Sets where children sit along the main axis (CSS `justify-content`).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{layout::Justify, Node};
    /// const V: Node = Node::row().justify(Justify::SpaceBetween);
    /// assert!(V.justify == Justify::SpaceBetween);
    /// ```
    #[inline]
    pub const fn justify(mut self, justify: Justify) -> Node {
        self.justify = justify;
        self
    }
    /// Breaks the flow into lines when the children overflow the main axis (`row_gap` between lines).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Node;
    /// const V: Node = Node::row().wrap();
    /// assert!(V.wrap);
    /// ```
    #[inline]
    pub const fn wrap(mut self) -> Node {
        self.wrap = true;
        self
    }
    /// Sets the order among a stack's children: higher draws over lower and takes the pointer first.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Node;
    /// const V: Node = Node::DEFAULT.z(2);
    /// assert!(V.z == 2);
    /// ```
    #[inline]
    pub const fn z(mut self, order: i16) -> Node {
        self.z = order;
        self
    }
    /// Sets the smallest width in points (`min_*` wins over `max_*`).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Node;
    /// const V: Node = Node::DEFAULT.min_w(40.0);
    /// assert!(V.min_w == 40.0);
    /// ```
    #[inline]
    pub const fn min_w(mut self, points: f32) -> Node {
        self.min_w = points;
        self
    }
    /// Sets the smallest height in points (`min_*` wins over `max_*`).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Node;
    /// const V: Node = Node::DEFAULT.min_h(20.0);
    /// assert!(V.min_h == 20.0);
    /// ```
    #[inline]
    pub const fn min_h(mut self, points: f32) -> Node {
        self.min_h = points;
        self
    }
    /// Sets the largest width in points (`f32::INFINITY`: no limit).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Node;
    /// const V: Node = Node::DEFAULT.max_w(300.0);
    /// assert!(V.max_w == 300.0);
    /// ```
    #[inline]
    pub const fn max_w(mut self, points: f32) -> Node {
        self.max_w = points;
        self
    }
    /// Sets the largest height in points (`f32::INFINITY`: no limit).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Node;
    /// const V: Node = Node::DEFAULT.max_h(200.0);
    /// assert!(V.max_h == 200.0);
    /// ```
    #[inline]
    pub const fn max_h(mut self, points: f32) -> Node {
        self.max_h = points;
        self
    }
    /// Sets how much room this node gives up, times its size, when its line overflows (0: none).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Node;
    /// const V: Node = Node::DEFAULT.shrink(1.0);
    /// assert!(V.shrink == 1.0);
    /// ```
    #[inline]
    pub const fn shrink(mut self, factor: f32) -> Node {
        self.shrink = factor;
        self
    }
    /// Sets the width-to-height ratio a `Fit` side is derived from (0: none).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Node;
    /// const V: Node = Node::DEFAULT.aspect(1.5);
    /// assert!(V.aspect == 1.5);
    /// ```
    #[inline]
    pub const fn aspect(mut self, ratio: f32) -> Node {
        self.aspect = ratio;
        self
    }
    /// Sets the inset of the children on each side in points (`pad`, `pad_x`, `pad_y` set it evenly).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Edges, Node};
    /// const V: Node = Node::DEFAULT.padding(Edges::xy(4.0, 2.0));
    /// assert!(V.padding == Edges::xy(4.0, 2.0));
    /// ```
    #[inline]
    pub const fn padding(mut self, edges: Edges) -> Node {
        self.padding = edges;
        self
    }
    /// Sets the room kept around this node on each side in points (`l` is the start edge).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Edges, Node};
    /// const V: Node = Node::DEFAULT.margin(Edges::all(8.0));
    /// assert!(V.margin == Edges::all(8.0));
    /// ```
    #[inline]
    pub const fn margin(mut self, edges: Edges) -> Node {
        self.margin = edges;
        self
    }
    /// Sets the points between wrapped lines and between grid rows (`gap` is between children and grid columns).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Node;
    /// const V: Node = Node::row().wrap().row_gap(6.0);
    /// assert!(V.row_gap == 6.0);
    /// ```
    #[inline]
    pub const fn row_gap(mut self, points: f32) -> Node {
        self.row_gap = points;
        self
    }
    /// Lays the children out on `grid`'s tracks instead of the axis.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{layout::{Grid, Track}, Node};
    /// const V: Node = Node::DEFAULT.grid(Grid::new(&[Track::Fr(1.0), Track::Fr(1.0)], &[]));
    /// assert!(V.grid.get().is_some_and(|g| g.cols().len() == 2));
    /// ```
    #[inline]
    pub const fn grid(mut self, grid: Grid) -> Node {
        self.grid = OptGrid::Some(grid);
        self
    }
    /// Sets this node's place in its parent's grid (`Cell::DEFAULT`: the next free cell).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{layout::Cell, Node};
    /// const V: Node = Node::DEFAULT.cell(Cell::at(1, 0).span(2, 1));
    /// assert!(V.cell.col_span == 2);
    /// ```
    #[inline]
    pub const fn cell(mut self, cell: Cell) -> Node {
        self.cell = cell;
        self
    }
    /// Overrides the parent's `align` for this node alone.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{layout::Align, Node};
    /// const V: Node = Node::DEFAULT.align_self(Align::End);
    /// assert!(V.align_self.get() == Some(Align::End));
    /// ```
    #[inline]
    pub const fn align_self(mut self, align: Align) -> Node {
        self.align_self = OptAlign::Some(align);
        self
    }
    /// Places this node by a window rectangle (a popup) instead of its parent's flow (`place_anchored` is the rule).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{layout::{Anchor, Placement}, geom::Rect, Node};
    /// const V: Node = Node::DEFAULT.anchor(Anchor::new(Rect::new(0.0, 0.0, 10.0, 10.0), Placement::BELOW));
    /// assert!(V.anchor.is_some());
    /// ```
    #[inline]
    pub const fn anchor(mut self, anchor: Anchor) -> Node {
        self.anchor = OptAnchor::Some(anchor);
        self
    }
    /// Sets the accessibility states the node's widget declares (`a11y::state` bits; focus, press, enabled and visibility come from the engine).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{a11y::state, Node};
    /// const V: Node = Node::DEFAULT.a11y(state::CHECKABLE | state::CHECKED);
    /// assert!(V.a11y == state::CHECKABLE | state::CHECKED);
    /// ```
    #[inline]
    pub const fn a11y(mut self, states: u32) -> Node {
        self.a11y = states;
        self
    }
    /// Places this node outside the flow, `x`, `y` points from its parent's top-left.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Node;
    /// const V: Node = Node::DEFAULT.at(4.0, 8.0);
    /// assert!(V.at.get() == Some((4.0, 8.0)));
    /// ```
    #[inline]
    pub const fn at(mut self, x: f32, y: f32) -> Node {
        self.at = OptPoint::Some(x, y);
        self
    }
    /// Sets the points one wheel line or arrow key scrolls a scrolling node.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Node;
    /// const V: Node = Node::DEFAULT.scroll_step(48.0);
    /// assert!(V.scroll_step == 48.0);
    /// ```
    #[inline]
    pub const fn scroll_step(mut self, points: f32) -> Node {
        self.scroll_step = points;
        self
    }
    /// Sets how the node is painted (`style` replaces it by state while the app builds).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Look, Node};
    /// const V: Node = Node::DEFAULT.look(Look::NONE.radius(4.0));
    /// assert!(V.look.radius == [4.0; 4]);
    /// ```
    #[inline]
    pub const fn look(mut self, look: Look) -> Node {
        self.look = look;
        self
    }
    /// Sets where the node's text sits inside it, in points from each side.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Edges, Node};
    /// const V: Node = Node::DEFAULT.inset(Edges::xy(6.0, 0.0));
    /// assert!(V.inset.l == 6.0);
    /// ```
    #[inline]
    pub const fn inset(mut self, edges: Edges) -> Node {
        self.inset = edges;
        self
    }
    /// Sets the mark drawn over the node's text (a chevron or a line).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{node::Dir, Mark, Node};
    /// const V: Node = Node::DEFAULT.mark(Mark::Chevron(Dir::Down));
    /// assert!(V.mark == Mark::Chevron(Dir::Down));
    /// ```
    #[inline]
    pub const fn mark(mut self, mark: Mark) -> Node {
        self.mark = mark;
        self
    }
    /// Gives a scrolling node a thumb painted as `bar`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Rgba, Bar, Node};
    /// const V: Node = Node::DEFAULT.bar(Bar::new(Rgba::BLACK, 4.0).radius(2.0).gap(2.0).margin(2.0));
    /// assert!(V.bar.is_some());
    /// ```
    #[inline]
    pub const fn bar(mut self, bar: Bar) -> Node {
        self.bar = OptBar::Some(bar);
        self
    }
}
