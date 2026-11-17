//! The option record of the box call that takes a closure (`Rc<dyn Fn>`): Rust-side only, never an ABI type. The engine keeps its own (`pxui_core::opts`).
use pxui_types::{geom::Rect, node::Axis, Size};
use std::rc::Rc;
use pxui_types::Flags;

pub type HitFn = Rc<dyn Fn(Rect) -> Rect>;

#[derive(Clone)]
pub struct BoxOpts {
    pub flags: Flags,
    pub w: Size,
    pub h: Size,
    pub max_h: f32,
    pub axis: Axis,
    pub padding: f32,
    pub at: Option<(f32, f32)>,
    pub scroll_step: f32,
    pub hit: Option<HitFn>,
    pub hit_rect: Option<Rect>,
}

impl Default for BoxOpts {
    fn default() -> BoxOpts {
        BoxOpts {
            flags: Flags::NONE,
            w: Size::Grow,
            h: Size::Fit,
            max_h: f32::INFINITY,
            axis: Axis::Column,
            padding: 0.0,
            at: None,
            scroll_step: 24.0,
            hit: None,
            hit_rect: None,
        }
    }
}

impl BoxOpts {
    /// The flags the box takes (`CLICKABLE`, `SCROLL` and the like), replacing any set before.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ui::{BoxOpts, Flags};
    /// assert_eq!(BoxOpts::default().flags(Flags::CLICKABLE).flags, Flags::CLICKABLE);
    /// ```
    pub fn flags(mut self, flags: Flags) -> BoxOpts {
        self.flags = flags;
        self
    }
    /// The box's width: `Size::Grow` by default.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ui::{BoxOpts, Size};
    /// assert!(matches!(BoxOpts::default().w(Size::Px(80.0)).w, Size::Px(80.0)));
    /// ```
    pub fn w(mut self, w: Size) -> BoxOpts {
        self.w = w;
        self
    }
    /// The box's height: `Size::Fit` by default.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ui::{BoxOpts, Size};
    /// assert!(matches!(BoxOpts::default().h(Size::Px(40.0)).h, Size::Px(40.0)));
    /// ```
    pub fn h(mut self, h: Size) -> BoxOpts {
        self.h = h;
        self
    }
    /// The most a `Fit` height may grow to, in points (unbounded by default); a box that would be taller scrolls when it has `SCROLL`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui::ui::BoxOpts::default().max_h(120.0).max_h, 120.0);
    /// ```
    pub fn max_h(mut self, max_h: f32) -> BoxOpts {
        self.max_h = max_h;
        self
    }
    /// The direction the box lays its children out in: `Axis::Column` by default.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ui::{Axis, BoxOpts};
    /// assert_eq!(BoxOpts::default().axis(Axis::Row).axis, Axis::Row);
    /// ```
    pub fn axis(mut self, axis: Axis) -> BoxOpts {
        self.axis = axis;
        self
    }
    /// The inset between the box's edge and its children, in points, on every side.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui::ui::BoxOpts::default().padding(8.0).padding, 8.0);
    /// ```
    pub fn padding(mut self, padding: f32) -> BoxOpts {
        self.padding = padding;
        self
    }
    /// Places the box at (`x`, `y`) points in its parent's space instead of in the flow; `set_at` moves it later.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui::ui::BoxOpts::default().at((4.0, 6.0)).at, Some((4.0, 6.0)));
    /// ```
    pub fn at(mut self, at: (f32, f32)) -> BoxOpts {
        self.at = Some(at);
        self
    }
    /// Points a wheel notch or arrow key scrolls the box by (24 by default); only a box with `SCROLL` uses it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui::ui::BoxOpts::default().scroll_step(48.0).scroll_step, 48.0);
    /// ```
    pub fn scroll_step(mut self, step: f32) -> BoxOpts {
        self.scroll_step = step;
        self
    }
    /// Sets a function that maps the box's laid-out rectangle to the rectangle that takes the pointer (a larger tap target, say).
    ///
    /// The closure runs on every hit test and must not allocate or call back into the `Ui`; a panic in it surfaces at the frame call.
    /// Allocates: the closure's `Rc` clone is boxed from a pool; never on a warm frame.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ui::{BoxOpts, Rect};
    /// let hit: pxui::ui::HitFn = std::rc::Rc::new(|r: Rect| Rect::new(r.x - 4.0, r.y - 4.0, r.w + 8.0, r.h + 8.0));
    /// assert!(BoxOpts::default().hit(hit).hit.is_some());
    /// ```
    pub fn hit(mut self, hit: HitFn) -> BoxOpts {
        self.hit = Some(hit);
        self
    }
}
