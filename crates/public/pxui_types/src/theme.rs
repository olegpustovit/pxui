use crate::geom::Rgba as Color;
use crate::geom::color;

const CLEAR: Color = Color {
    r: 0,
    g: 0,
    b: 0,
    a: 0,
};

/// Motion durations in seconds and the CSS cubic-bezier easing curve `[x1, y1, x2, y2]`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub fast: f32,
    pub normal: f32,
    pub slow: f32,
    pub ease: [f32; 4],
}

impl From<crate::ffi::PxuiMotion> for Motion {
    fn from(value: crate::ffi::PxuiMotion) -> Self {
        Self { fast: value.fast, normal: value.normal, slow: value.slow, ease: value.ease }
    }
}

/// Ergonomic Rust view of the stable paint-token snapshot. `invalid` is a fixed kit color because
/// the C snapshot intentionally carries only the six theme colors and optional ink ground.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    pub panel: Color,
    pub foreground: Color,
    pub control: Color,
    pub input: Color,
    pub track: Color,
    pub accent: Color,
    pub invalid: Color,
    pub ink_ground: crate::node::OptColor,
    pub space: [f32; 4],
    pub radius: [f32; 3],
    pub display_size: i64,
    pub title_size: i64,
    pub font_size: i64,
    pub label_size: i64,
    pub motion: Motion,
    pub reduced_motion: bool,
    pub density: f32,
}

impl Tokens {
    /// Convert the ABI-safe snapshot into its Rust-facing form.
    pub fn from_ffi(value: crate::ffi::ThemeTokens) -> Self { value.into() }

    /// State transition timing in this theme; reduced motion makes it instantaneous.
    pub fn transition(&self, from: State, to: State) -> Curve {
        let duration = if self.reduced_motion || from == to {
            0.0
        } else if from.pressed != to.pressed {
            self.motion.fast
        } else {
            self.motion.normal
        };
        Curve::Bezier { duration, points: self.motion.ease }
    }

    pub fn moves(&self) -> bool {
        !self.reduced_motion && (self.motion.fast > 0.0 || self.motion.normal > 0.0)
    }
}

impl From<crate::ffi::ThemeTokens> for Tokens {
    fn from(value: crate::ffi::ThemeTokens) -> Self {
        Self {
            panel: value.panel,
            foreground: value.foreground,
            control: value.control,
            input: value.input,
            track: value.track,
            accent: value.accent,
            invalid: invalid(),
            ink_ground: (value.has_ink_ground != 0).then_some(value.ink_ground).into(),
            space: value.space,
            radius: value.radius,
            display_size: value.display_size,
            title_size: value.title_size,
            font_size: value.font_size,
            label_size: value.label_size,
            motion: value.motion.into(),
            reduced_motion: value.reduced_motion != 0,
            density: value.density,
        }
    }
}

impl From<Tokens> for crate::ffi::ThemeTokens {
    fn from(t: Tokens) -> Self {
        Self {
            size: core::mem::size_of::<crate::ffi::ThemeTokens>() as u32, panel: t.panel, foreground: t.foreground, control: t.control, input: t.input, track: t.track, accent: t.accent,
            ink_ground: t.ink_ground.get().unwrap_or_default(), has_ink_ground: t.ink_ground.is_some() as u8, reserved: [0; 3],
            space: t.space, radius: t.radius, display_size: t.display_size, title_size: t.title_size,
            font_size: t.font_size, label_size: t.label_size,
            motion: crate::ffi::PxuiMotion { fast: t.motion.fast, normal: t.motion.normal, slow: t.motion.slow, ease: t.motion.ease },
            reduced_motion: t.reduced_motion as u8, reserved_tail: [0; 3], density: t.density,
        }
    }
}

/// The kit parts of a theme, in the order the C snapshot lists them. `pxui_core`'s `Theme` and `ThemeSnapshot` are
/// generated from this one list, so a part exists in both or in neither.
#[macro_export]
macro_rules! theme_widgets {
    ($m:ident) => {
        $m! {
    /// a docked panel's ground
    panel,
    /// a floating sheet (popup, menu, completion list): the input white with a hairline
    popup,
    /// a modal's sheet
    modal,
    /// the scroll thumb (fill)
    scrollbar,
    /// a caption: upper case at the label size
    label,
    /// a section header's caption and its chevron
    section,
    chevron,
    /// an inspector row (its label in ink-2)
    row,
    /// a list row (picker, completion); `checked` is the keyboard's current row
    list,
    /// a row armed for deletion
    row_doomed,
    /// a parameter row's pin dot, `checked` when the row is on its card
    pin,
    /// the current choice's square, a message's dot
    mark,
    notice,
    /// an accent caption beside a name
    badge,
    /// a text button; `checked` while it is on
    button,
    /// a quieter button (`bare`, a trailing reset)
    button_bare,
    /// a switch row, its track and knob (`checked` when on)
    toggle,
    switch,
    knob,
    /// a field's hairline and value; `focus` while it is edited
    field,
    field_invalid,
    /// a value's position in its soft range
    fraction,
    /// empty text's stand-in
    placeholder,
    /// a value read out
    value,
    /// keys beside what they do (`hint`)
    key,
    hint,
    /// a query's matched letters
    matched,
    /// a colour swatch's frame
    swatch,
    /// a menu row and a destructive one, a separator line
    menu,
    menu_danger,
    menu_key,
    separator,
    /// a window's foot and an inspector's bar (the hairline)
    footer,
    /// an inspector head: its hairline, the name and its index
    header,
    title,
    index,
    /// a read-only table's header row and body rows
    table_head,
    table_row,
    /// a tooltip, the carried payload and its tip
    tooltip,
    ghost,
    /// the text area: ground, gutter rule (`border`), selection and caret
    text_area,
    /// a checkbox's box and a radio's ring (`checked` when on; the inner square is `mark`, the dot `notice`)
    checkbox,
    radio,
    /// a slider's thumb (its rail is `separator`, the filled part `mark`)
    slider,
    /// a progress track (the bar is `mark`) and a spinner's dots (`checked` is the lead dot)
    progress,
    spinner,
    /// a tab (`checked`: the one in use) and a segment of a segmented control
    tab,
    segment,
    /// a dropdown's button (its menu is `popup` with `menu` rows)
    dropdown,
    /// a menu bar (its items are `menu` rows)
    menubar,
    /// a scroll view's frame
    scroll_view,
    /// a list row and a tree row (`checked`: selected; `focus`: the keyboard's row)
    listbox,
    tree,
    /// a table's column header (`checked`: the sort column)
    column,
    /// a dialog's title bar
    dialog,
    /// a toast's sheet
    toast,
    /// a square button with a glyph
    icon_button,
    /// a search field's well
    search,
    /// an image's frame
    image,
        }
    };
}

/// Which registered body builds a widget kind's nodes: 0 is the built-in one, any other is the nth body the app registered for
/// that kind (`Ui::register_toggle_body` and its siblings), numbered from 1 in registration order.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BodyId(pub u16);

impl BodyId {
    pub const DEFAULT: BodyId = BodyId(0);
}

/// The body each widget kind builds with, in a theme: ids, never function pointers, so a `Theme` stays `Copy` and comparable
/// and a theme file can pick one by name (`toggle.body = checkbox`). Grows at the end only.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bodies {
    pub toggle: BodyId,
    pub slider: BodyId,
    pub progress: BodyId,
}

/// A node a widget's behaviour finds by role (`Ui::part`): the toggle's hit shape, the slider's rail and thumb, the progress
/// track and its busy bar, a spinner's dots. Numbered once, never renumbered. The built-in bodies' parts keep the keys those nodes
/// had before bodies existed (`Part::offset`), which is why `HIT` and `TRACK` share one.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part(pub u32);

impl Part {
    /// The area that takes a toggle's presses; the whole row when a body builds none.
    pub const HIT: Part = Part(1);
    /// A slider's rail: the pointer maps to the value across its width.
    pub const RAIL: Part = Part(2);
    /// A slider's thumb: half its width is kept at each end of the rail.
    pub const THUMB: Part = Part(3);
    /// A progress row's track: the accessible node, and the extent `busy_step` sweeps its bar across.
    pub const TRACK: Part = Part(4);
    /// The bar `busy_step` sweeps along an indeterminate progress track (inside the track).
    pub const BAR: Part = Part(5);
    /// Dot `i` of a spinner, which `busy_step` turns.
    pub const fn dot(i: u8) -> Part {
        Part(16 + i as u32)
    }

    /// The number a part's key is derived from.
    pub const fn offset(self) -> i64 {
        match self.0 {
            1 | 4 => 0,
            2 => 2,
            3 => 6,
            n => n as i64 - 16,
        }
    }
}

macro_rules! snapshot {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {
        /// The tokens and one `States` per kit part: the engine's `Theme` (the same C layout), borrowed from the library.
        #[repr(C)]
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct ThemeSnapshot {
            pub tokens: Tokens,
            pub row_h: f32,
            pub control_h: f32,
            pub ring: Ring,
            pub scrollbar_w: f32,
            pub scrollbar_gap: f32,
            $($(#[$doc])* pub $name: States,)*
            /// The body each widget kind builds with; after the parts, so their array keeps its offsets.
            pub body: Bodies,
        }
        /// How many kit parts a snapshot lists.
        pub const WIDGET_COUNT: usize = [$(stringify!($name)),*].len();
    };
}
theme_widgets!(snapshot);

pub type Theme = ThemeSnapshot;

impl From<Look> for crate::ffi::PxuiLookDesc {
    fn from(v: Look) -> Self {
        let shadow = |s: Shadow| crate::ffi::PxuiShadowDesc { color: s.color, offset: s.offset, blur: s.blur, spread: s.spread, inner: s.inner as u8, reserved: [0; 3] };
        Self {
            fill: v.fill, gradient: v.gradient.get().unwrap_or_default(), has_gradient: v.gradient.is_some() as u8, reserved: [0; 3], angle: v.angle,
            border: v.border, border_w: v.border_w, sides: v.sides as u32, radius: v.radius, shadows: [shadow(v.shadows[0]), shadow(v.shadows[1])], text: v.text,
            font: crate::ffi::PxuiFontDesc { family: v.font.family, weight: v.font.weight, size: v.font.size, tracking: v.font.tracking, caps: v.font.caps as u8, reserved: [0; 3] },
            opacity: v.opacity, ring: crate::ffi::PxuiRingDesc { color: v.ring.color, width: v.ring.width, sides: v.ring.sides as u32 },
            selection: v.selection, caret: v.caret, dashed: v.dashed as u8, reserved_tail: [0; 3],
        }
    }
}

impl From<crate::ffi::PxuiLookDesc> for Look {
    fn from(v: crate::ffi::PxuiLookDesc) -> Self {
        let shadow = |s: crate::ffi::PxuiShadowDesc| Shadow {
            color:s.color, offset:s.offset, blur:s.blur, spread:s.spread, inner:s.inner != 0,
        };
        Self {
            fill:v.fill, gradient:(v.has_gradient != 0).then_some(v.gradient).into(), angle:v.angle,
            border:v.border, border_w:v.border_w, sides:v.sides as u8, radius:v.radius,
            shadows:[shadow(v.shadows[0]),shadow(v.shadows[1])], text:v.text,
            font:Font { family:v.font.family, size:v.font.size, weight:v.font.weight, tracking:v.font.tracking, caps:v.font.caps != 0 },
            opacity:v.opacity, ring:Ring { color:v.ring.color, width:v.ring.width, sides:v.ring.sides as u8 },
            selection:v.selection, caret:v.caret, dashed:v.dashed != 0,
        }
    }
}

impl Default for Tokens {
    fn default() -> Self { self::default() }
}

pub const FONT_SIZE: i64 = 13;
pub const LABEL_SIZE: i64 = 11;
pub const TITLE_SIZE: i64 = 20;
pub const DISPLAY_SIZE: i64 = 40;

/// Rays' default tokens.
pub fn default() -> Tokens {
    let hex = |s| color::hex_exn(s).expect("valid theme color constant");
    Tokens {
        panel: hex("#f4f5f0"), foreground: hex("#222b2b"), control: hex("#dce3de"),
        input: hex("#ffffff"), track: hex("#e3e8e4"), accent: hex("#f0481f"),
        invalid: invalid(), ink_ground: crate::node::OptColor::None, space: [4.0, 8.0, 12.0, 16.0], radius: [0.0; 3],
        display_size: DISPLAY_SIZE, title_size: TITLE_SIZE, font_size: FONT_SIZE, label_size: LABEL_SIZE,
        motion: Motion { fast: 0.0, normal: 0.0, slow: 0.0, ease: [0.25, 0.1, 0.25, 1.0] },
        reduced_motion: false, density: 1.0,
    }
}

pub fn ground(theme: &Tokens) -> Color { theme.ink_ground.get().unwrap_or(theme.panel) }
pub fn ink_2(theme: &Tokens) -> Color { color::blend(theme.foreground, ground(theme), 0.32) }
pub fn ink_3(theme: &Tokens) -> Color { color::blend(theme.foreground, ground(theme), 0.521) }
pub fn muted(theme: &Tokens) -> Color { ink_2(theme) }
pub fn border(theme: &Tokens) -> Color { color::with_alpha(theme.foreground, 77) }
pub fn edge(theme: &Tokens) -> Color { color::with_alpha(theme.foreground, 38) }
pub fn faint_border(theme: &Tokens) -> Color { color::with_alpha(theme.foreground, 20) }
pub fn hover_fill(theme: &Tokens) -> Color { color::blend(theme.input, theme.accent, 0.11) }
pub fn pressed_fill(theme: &Tokens) -> Color { color::blend(theme.control, theme.accent, 0.18) }
pub fn tint(theme: &Tokens) -> Color { color::with_alpha(theme.accent, 41) }
pub fn invalid() -> Color { color::hex_exn("#c2255c").expect("valid invalid color constant") }
pub fn dark(theme: &Tokens) -> bool {
    (theme.panel.r as u32 + theme.panel.g as u32 + theme.panel.b as u32) < 384
}

/// A box shadow: outer, or `inner` (CSS `inset`); none while `color` is transparent.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub color: Color,
    pub offset: [f32; 2],
    pub blur: f32,
    pub spread: f32,
    pub inner: bool,
}

impl Shadow {
    pub const NONE: Shadow = Shadow {
        color: CLEAR,
        offset: [0.0, 0.0],
        blur: 0.0,
        spread: 0.0,
        inner: false,
    };
}

/// Node text: font `family` (the `pxui_text` font index), `size` points (0: the UI's font size; below 0:
/// that many points from it, at least 8), `weight`, `tracking` in ems after every glyph, `caps`
/// upper-cases it.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Font {
    pub family: u16,
    pub size: i64,
    pub weight: u16,
    pub tracking: f32,
    pub caps: bool,
}

impl Font {
    pub const BODY: Font = Font {
        family: 0,
        size: 0,
        weight: 400,
        tracking: 0.0,
        caps: false,
    };
    /// The size in points at UI font size `body`.
    pub fn points(&self, body: i64) -> i64 {
        if self.size > 0 {
            self.size
        } else if self.size < 0 {
            8.max(body + self.size)
        } else {
            body
        }
    }
}

/// An outline drawn while the node holds the focus: `width` points inside its box on `sides`
/// (`draw::flags::TOP | …`; 0 all four). None while `color` is transparent.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring {
    pub color: Color,
    pub width: f32,
    pub sides: u8,
}

impl Ring {
    pub const NONE: Ring = Ring {
        color: CLEAR,
        width: 0.0,
        sides: 0,
    };
}

/// What a node looks like, painted in this order: outer shadows, the fill (solid, or a linear
/// `gradient` from `fill` to that colour along `angle` degrees, CSS-style: 180 runs top to bottom),
/// inner shadows, a `border_w` border inside the box on `sides` (0: all four), `radius` per corner (top
/// left, top right, bottom right, bottom left), the node's text in `text` and `font`, its mark, then
/// `ring` while it holds the focus. `opacity` scales every alpha. `selection` and `caret` colour a text
/// widget's editing marks. Transparent colours paint nothing. A `dashed` square border is one point
/// of 4-on, 3-off dashes (a drop target, an expression card).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub fill: Color,
    pub gradient: crate::node::OptColor,
    pub angle: f32,
    pub border: Color,
    pub border_w: f32,
    pub sides: u8,
    pub radius: [f32; 4],
    pub shadows: [Shadow; 2],
    pub text: Color,
    pub font: Font,
    pub opacity: f32,
    pub ring: Ring,
    pub selection: Color,
    pub caret: Color,
    pub dashed: bool,
}

impl Look {
    pub const NONE: Look = Look {
        fill: CLEAR,
        gradient: crate::node::OptColor::None,
        angle: 180.0,
        border: CLEAR,
        border_w: 0.0,
        sides: 0,
        radius: [0.0; 4],
        shadows: [Shadow::NONE; 2],
        text: CLEAR,
        font: Font::BODY,
        opacity: 1.0,
        ring: Ring::NONE,
        selection: CLEAR,
        caret: CLEAR,
        dashed: false,
    };

    /// The look `t` of the way from `self` to `to`: colours and sizes interpolate, the rest (font,
    /// sides) is `to`'s from the start.
    pub fn lerp(&self, to: &Look, t: f32) -> Look {
        let c = |a: Color, b: Color| {
            // an overshooting curve saturates at the channel's ends
            let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
            Color {
                r: m(a.r, b.r),
                g: m(a.g, b.g),
                b: m(a.b, b.b),
                a: m(a.a, b.a),
            }
        };
        let f = |a: f32, b: f32| a + (b - a) * t;
        let gradient = match (self.gradient.get(), to.gradient.get()) {
            (None, None) => None,
            (a, b) => Some(c(a.unwrap_or(self.fill), b.unwrap_or(to.fill))),
        }.into();
        let shadow = |a: &Shadow, b: &Shadow| Shadow {
            color: c(a.color, b.color),
            offset: [f(a.offset[0], b.offset[0]), f(a.offset[1], b.offset[1])],
            blur: f(a.blur, b.blur),
            spread: f(a.spread, b.spread),
            inner: if b.color.a > 0 { b.inner } else { a.inner },
        };
        Look {
            fill: c(self.fill, to.fill),
            gradient,
            angle: f(self.angle, to.angle),
            border: c(self.border, to.border),
            border_w: f(self.border_w, to.border_w),
            sides: to.sides,
            radius: std::array::from_fn(|i| f(self.radius[i], to.radius[i])),
            shadows: [
                shadow(&self.shadows[0], &to.shadows[0]),
                shadow(&self.shadows[1], &to.shadows[1]),
            ],
            text: c(self.text, to.text),
            font: to.font,
            opacity: f(self.opacity, to.opacity),
            ring: Ring {
                color: c(self.ring.color, to.ring.color),
                width: f(self.ring.width, to.ring.width),
                sides: to.ring.sides,
            },
            selection: c(self.selection, to.selection),
            caret: c(self.caret, to.caret),
            dashed: to.dashed,
        }
    }
}

/// How an animation runs from 0 to 1: a CSS cubic-bezier over `duration` seconds, or a critically
/// damped spring of natural frequency `omega` (radians a second) that settles by itself.
#[repr(C, u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    Bezier { duration: f32, points: [f32; 4] },
    Spring { omega: f32 },
}

impl Curve {
    /// A run that is over before it starts.
    pub fn instant(&self) -> bool {
        matches!(self, Curve::Bezier { duration, .. } if !(*duration > 0.0))
    }

    /// Progress `t` seconds in; exactly 1 once the run is over.
    pub fn at(&self, t: f32) -> f32 {
        match *self {
            Curve::Bezier {
                duration,
                points: [x1, y1, x2, y2],
            } => {
                if !(t < duration) {
                    return 1.0;
                }
                let s = (t / duration).max(0.0);
                let b = |a: f32, c: f32, u: f32| {
                    3.0 * (1.0 - u) * (1.0 - u) * u * a + 3.0 * (1.0 - u) * u * u * c + u * u * u
                };
                // x is monotonic in u for x1, x2 in 0..=1: bisect for the u where x(u) = s
                let (mut lo, mut hi) = (0.0f32, 1.0f32);
                for _ in 0..24 {
                    let mid = (lo + hi) / 2.0;
                    if b(x1, x2, mid) < s {
                        lo = mid
                    } else {
                        hi = mid
                    }
                }
                b(y1, y2, (lo + hi) / 2.0)
            }
            Curve::Spring { omega } => {
                let wt = (omega * t).max(0.0);
                let rest = (1.0 + wt) * (-wt).exp();
                if rest < 1.0e-3 || !(omega > 0.0) {
                    1.0
                } else {
                    1.0 - rest
                }
            }
        }
    }
}

/// What the pointer and keyboard are doing to a widget; `States::pick` resolves it to a look.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub disabled: bool,
    pub checked: bool,
}

/// A look per interaction state, plus `checked` for a widget that is on, current or selected.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct States {
    pub idle: Look,
    pub hover: Look,
    pub pressed: Look,
    pub focus: Look,
    pub disabled: Look,
    pub checked: Look,
}

impl States {
    /// Every state the same look.
    pub const fn all(look: Look) -> States {
        States {
            idle: look,
            hover: look,
            pressed: look,
            focus: look,
            disabled: look,
            checked: look,
        }
    }
    /// disabled, then pressed, hovered, checked, focused, idle: the first that holds.
    pub fn pick(&self, s: State) -> &Look {
        if s.disabled {
            &self.disabled
        } else if s.pressed {
            &self.pressed
        } else if s.hovered {
            &self.hover
        } else if s.checked {
            &self.checked
        } else if s.focused {
            &self.focus
        } else {
            &self.idle
        }
    }
}
