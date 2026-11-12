//! The design kit as data (kit rev 3, specification/pxui.md). `Tokens` are the colour roles, scales and
//! motion; a `Look` is what one node looks like (plain `Copy` data the engine paints with no closure);
//! `States` picks a look per interaction state; a `Theme` is the tokens plus one `States` per kit part.
//! The Rays look (Pragmasevka, 24-point rows, 20-point controls, six colours) is `Theme::default()`:
//! separation comes from space first, a hairline second, a rectangle last; nothing is filled with ink.
//! A theme is also a flat `key = value` file (`Theme::parse`, `Theme::write`, `assets/themes/`).
use crate::draw::flags::{BOTTOM, LEFT, RIGHT, TOP};
use crate::geom::{color, Rgba as Color};
pub use pxui_types::theme::{
    border, dark, default, edge, Bodies, BodyId, Part, faint_border, ground, hover_fill, ink_2, ink_3, invalid,
    muted, pressed_fill, tint, Curve, Font, Look, Motion, Ring, Shadow, State, States, Tokens,
    DISPLAY_SIZE, FONT_SIZE, LABEL_SIZE, TITLE_SIZE,
};

// ------------------------------------------------------------------ looks


macro_rules! widgets {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {
        /// The tokens and one `States` per kit part. `row_h` is the kit row and `control_h` a control's
        /// height; `ring` is the focus outline of a focusable node whose look draws none; `scrollbar_w`
        /// and `scrollbar_gap` are the thumb's width and its distance from the right edge.
        // C's layout, the same as `pxui_types::theme::ThemeSnapshot` (one field list): the library lends it to the app as it is
        #[repr(C)]
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Theme {
            pub tokens: Tokens,
            pub row_h: f32,
            pub control_h: f32,
            pub ring: Ring,
            pub scrollbar_w: f32,
            pub scrollbar_gap: f32,
            $($(#[$doc])* pub $name: States,)*
            /// The body each widget kind builds with (`Ui::register_toggle_body` and siblings); after the parts, as in the snapshot.
            pub body: Bodies,
        }
        const WIDGETS: &[&str] = &[$(stringify!($name)),*];
        fn widget_mut<'a>(t: &'a mut Theme, name: &str) -> Option<&'a mut States> {
            match name { $(stringify!($name) => Some(&mut t.$name),)* _ => None }
        }
        fn widget<'a>(t: &'a Theme, name: &str) -> &'a States {
            match name { $(stringify!($name) => &t.$name,)* _ => unreachable!() }
        }
    };
}

pxui_types::theme_widgets!(widgets);

impl Default for Theme {
    fn default() -> Theme {
        Theme::from_tokens(default())
    }
}

/// The Rays kit's looks from `t`.
fn rays(t: &Tokens) -> Theme {
    let n = Look::NONE;
    let label_font = Font { size: t.label_size - t.font_size, tracking: 0.08, caps: true, ..Font::BODY };
    let small = Font { size: t.label_size - t.font_size, ..Font::BODY };
    let ink = |c: Color| Look { text: c, ..n };
    let sheet = |edge: Color| Look { fill: t.input, border: edge, border_w: 1.0, radius: [t.radius[1]; 4], ..n };
    let line = |c: Color, sides: u8| Look { border: c, border_w: 1.0, sides, ..n };
    let accent_line = Ring { color: t.accent, width: 1.0, sides: BOTTOM };
    let button = |text: Color| {
        let idle = Look { text, ring: accent_line, radius: [t.radius[0]; 4], ..n };
        States {
            idle,
            hover: Look { fill: hover_fill(t), ..idle },
            pressed: Look { fill: pressed_fill(t), ..idle },
            focus: idle,
            disabled: Look { text: ink_3(t), ..idle },
            checked: Look { fill: t.control, ..idle },
        }
    };
    let row = Look { text: ink_2(t), ..n };
    let field = Look { text: t.foreground, selection: tint(t), caret: t.accent, radius: [t.radius[0]; 4], ..line(edge(t), BOTTOM) };
    let dot = Look { radius: [3.0; 4], ..n };
    let check = |r: f32| {
        let idle = Look { fill: t.input, border: border(t), border_w: 1.0, radius: [r; 4], ..n };
        States {
            idle,
            hover: Look { border: t.foreground, ..idle },
            pressed: Look { fill: pressed_fill(t), border: t.foreground, ..idle },
            focus: idle,
            disabled: Look { border: edge(t), fill: t.track, ..idle },
            checked: Look { border: t.accent, ..idle },
        }
    };
    let thumb = Look { fill: t.input, border: border(t), border_w: 1.0, radius: [t.radius[0]; 4], ..n };
    let segment = Look { text: ink_2(t), border: edge(t), border_w: 1.0, ..n };
    let rows = States {
        hover: Look { fill: faint_border(t), text: t.foreground, ..n },
        checked: Look { fill: t.control, text: t.foreground, ..n },
        focus: Look { text: t.foreground, ..line(t.accent, LEFT) },
        disabled: ink(ink_3(t)),
        ..States::all(ink(t.foreground))
    };
    Theme {
        tokens: *t,
        row_h: 24.0,
        control_h: 20.0,
        ring: Ring { color: t.accent, width: 1.0, sides: 0 },
        scrollbar_w: 4.0,
        scrollbar_gap: 2.0,
        panel: States::all(Look { fill: t.panel, radius: [t.radius[2]; 4], ..n }),
        popup: States::all(sheet(edge(t))),
        modal: States::all(sheet(border(t))),
        scrollbar: States::all(Look { fill: border(t), radius: [t.radius[0]; 4], ..n }),
        label: States::all(Look { text: ink_2(t), font: label_font, ..n }),
        section: States::all(Look { text: ink_3(t), font: label_font, ..n }),
        chevron: States::all(ink(t.foreground)),
        row: States { hover: Look { fill: faint_border(t), ..row }, checked: Look { fill: t.control, ..row }, ..States::all(row) },
        list: States {
            hover: Look { fill: faint_border(t), text: t.foreground, ..n },
            checked: Look { fill: t.control, text: t.foreground, ..n },
            disabled: ink(ink_3(t)),
            ..States::all(ink(t.foreground))
        },
        row_doomed: States::all(Look { fill: pressed_fill(t), text: invalid(), ..n }),
        pin: States { checked: Look { fill: t.foreground, ..dot }, ..States::all(Look { border: ink_3(t), border_w: 1.0, ..dot }) },
        mark: States::all(Look { fill: t.accent, ..n }),
        notice: States::all(Look { fill: t.accent, ..dot }),
        badge: States::all(Look { text: t.accent, font: label_font, ..n }),
        button: button(t.foreground),
        button_bare: button(ink_2(t)),
        toggle: States { hover: Look { fill: faint_border(t), text: t.foreground, ..n }, ..States::all(ink(t.foreground)) },
        switch: States {
            checked: Look { fill: t.input, border: border(t), border_w: 1.0, radius: [t.radius[0]; 4], ..n },
            ..States::all(Look { fill: t.track, border: border(t), border_w: 1.0, radius: [t.radius[0]; 4], ..n })
        },
        knob: States { checked: Look { fill: t.accent, radius: [t.radius[0]; 4], ..n }, ..States::all(Look { fill: ink_3(t), radius: [t.radius[0]; 4], ..n }) },
        field: States { focus: Look { border: t.accent, ..field }, disabled: Look { text: ink_3(t), ..field }, ..States::all(field) },
        field_invalid: States::all(Look { border: t.invalid, ..field }),
        fraction: States::all(Look { fill: ink_2(t), ..n }),
        placeholder: States::all(ink(ink_3(t))),
        value: States::all(ink(t.foreground)),
        key: States::all(Look { text: ink_3(t), font: small, ..n }),
        hint: States::all(ink(ink_2(t))),
        matched: States::all(ink(t.accent)),
        swatch: States::all(line(edge(t), 0)),
        menu: States {
            hover: Look { fill: t.control, text: t.foreground, ..n },
            checked: Look { fill: t.control, text: t.foreground, ..n },
            disabled: ink(ink_3(t)),
            ..States::all(ink(t.foreground))
        },
        menu_danger: States {
            hover: Look { fill: t.control, text: t.invalid, ..n },
            checked: Look { fill: t.control, text: t.invalid, ..n },
            disabled: ink(ink_3(t)),
            ..States::all(ink(t.invalid))
        },
        menu_key: States::all(Look { text: ink_2(t), font: small, ..n }),
        separator: States::all(Look { fill: edge(t), ..n }),
        footer: States::all(line(edge(t), TOP)),
        header: States::all(line(edge(t), BOTTOM)),
        title: States::all(Look { text: t.foreground, font: Font { tracking: -0.01, ..Font::BODY }, ..n }),
        index: States::all(Look { text: ink_3(t), font: Font { tracking: 0.04, ..small }, ..n }),
        table_head: States::all(Look { text: ink_2(t), ..line(border(t), BOTTOM) }),
        table_row: States::all(Look { text: t.foreground, ..line(faint_border(t), BOTTOM) }),
        tooltip: States::all(Look { text: t.foreground, font: small, ..sheet(edge(t)) }),
        ghost: States::all(Look { fill: t.input, text: t.foreground, radius: [t.radius[1]; 4], ..n }),
        text_area: States::all(Look { fill: t.input, border: faint_border(t), text: t.foreground, selection: tint(t), caret: t.accent, ..n }),
        checkbox: check(0.0),
        radio: check(6.0),
        slider: States {
            hover: Look { border: t.foreground, ..thumb },
            pressed: Look { fill: t.control, border: t.foreground, ..thumb },
            disabled: Look { border: edge(t), ..thumb },
            ..States::all(thumb)
        },
        progress: States::all(Look { fill: t.track, radius: [t.radius[0]; 4], ..n }),
        spinner: States { checked: Look { fill: t.accent, radius: [1.5; 4], ..n }, ..States::all(Look { fill: border(t), radius: [1.5; 4], ..n }) },
        tab: States {
            hover: Look { text: t.foreground, ..ink(ink_2(t)) },
            checked: Look { text: t.foreground, border: t.accent, border_w: 2.0, sides: BOTTOM, ..n },
            disabled: ink(ink_3(t)),
            ..States::all(ink(ink_2(t)))
        },
        segment: States {
            hover: Look { fill: hover_fill(t), ..segment },
            pressed: Look { fill: pressed_fill(t), ..segment },
            checked: Look { fill: t.control, text: t.foreground, ..segment },
            disabled: Look { text: ink_3(t), ..segment },
            ..States::all(segment)
        },
        dropdown: States {
            hover: Look { fill: hover_fill(t), ..field },
            pressed: Look { fill: pressed_fill(t), ..field },
            focus: Look { border: t.accent, ..field },
            disabled: Look { text: ink_3(t), ..field },
            ..States::all(field)
        },
        menubar: States::all(Look { fill: t.panel, text: t.foreground, ..line(edge(t), BOTTOM) }),
        scroll_view: States::all(line(edge(t), 0)),
        listbox: rows,
        tree: rows,
        column: States {
            hover: Look { fill: hover_fill(t), text: t.foreground, ..n },
            checked: ink(t.foreground),
            ..States::all(ink(ink_2(t)))
        },
        dialog: States::all(Look { text: t.foreground, ..line(edge(t), BOTTOM) }),
        toast: States { hover: Look { border: border(t), ..sheet(edge(t)) }, ..States::all(Look { text: t.foreground, ..sheet(edge(t)) }) },
        icon_button: button(t.foreground),
        search: States { focus: Look { border: t.accent, ..field }, ..States::all(field) },
        image: States::all(line(edge(t), 0)),
        body: Bodies::default(),
    }
}

impl Theme {
    /// Every look derived from `tokens` the way the Rays kit derives them.
    pub fn from_tokens(tokens: Tokens) -> Theme {
        rays(&tokens)
    }
}

// ------------------------------------------------------------- theme files

/// A theme file's line that could not be applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeError {
    pub line: usize,
    pub message: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Value {
    C(Color),
    O(Option<Color>),
    F(f32),
    I(i64),
    B(bool),
    Q([f32; 4]),
    S(u8),
}

/// How a field's value is written and which values it takes.
#[derive(Clone, Copy)]
enum Kind {
    Color,
    OptColor,
    Num(f32, f32),
    Int(i64, i64),
    Bool,
    /// one number for all four, or four
    Quad(f32, f32),
    Sides,
}

macro_rules! value_conv {
    ($($t:ty => $v:ident),*) => {$(
        impl From<$t> for Value { fn from(x: $t) -> Value { Value::$v(x) } }
        impl From<Value> for $t { fn from(v: Value) -> $t { match v { Value::$v(x) => x, _ => unreachable!("theme field kind") } } }
    )*};
}
value_conv!(Color => C, Option<Color> => O, f32 => F, i64 => I, bool => B, [f32; 4] => Q, u8 => S);

impl From<pxui_types::node::OptColor> for Value {
    fn from(x: pxui_types::node::OptColor) -> Value {
        Value::O(x.get())
    }
}

impl From<Value> for pxui_types::node::OptColor {
    fn from(v: Value) -> pxui_types::node::OptColor {
        Option::<Color>::from(v).into()
    }
}

impl From<u16> for Value {
    fn from(x: u16) -> Value {
        Value::I(x as i64)
    }
}
impl From<Value> for u16 {
    fn from(v: Value) -> u16 {
        i64::from(v) as u16
    }
}

struct Field<T> {
    key: &'static str,
    kind: Kind,
    get: fn(&T) -> Value,
    set: fn(&mut T, Value),
}

/// One table row: `key`, kind, and the field's path; get and set are generated from the path.
macro_rules! field {
    ($t:ty, $key:literal, $kind:expr, $($p:tt)+) => {
        Field::<$t> { key: $key, kind: $kind, get: |x: &$t| Value::from(x.$($p)+), set: |x: &mut $t, v: Value| x.$($p)+ = v.into() }
    };
}

const COLOR: Kind = Kind::Color;
const LEN: Kind = Kind::Num(0.0, 4096.0);
const OFFSET: Kind = Kind::Num(-4096.0, 4096.0);
const UNIT: Kind = Kind::Num(0.0, 1.0);
const SECONDS: Kind = Kind::Num(0.0, 10.0);
const SIZE: Kind = Kind::Int(1, 512);
const REL_SIZE: Kind = Kind::Int(-64, 512);
const TRACKING: Kind = Kind::Num(-1.0, 1.0);

fn token_fields() -> Vec<Field<Tokens>> {
    vec![
        field!(Tokens, "panel", COLOR, panel),
        field!(Tokens, "foreground", COLOR, foreground),
        field!(Tokens, "control", COLOR, control),
        field!(Tokens, "input", COLOR, input),
        field!(Tokens, "track", COLOR, track),
        field!(Tokens, "accent", COLOR, accent),
        field!(Tokens, "invalid", COLOR, invalid),
        field!(Tokens, "ink_ground", Kind::OptColor, ink_ground),
        field!(Tokens, "space", Kind::Quad(0.0, 256.0), space),
        field!(Tokens, "radius.small", LEN, radius[0]),
        field!(Tokens, "radius.medium", LEN, radius[1]),
        field!(Tokens, "radius.large", LEN, radius[2]),
        field!(Tokens, "type.display", SIZE, display_size),
        field!(Tokens, "type.title", SIZE, title_size),
        field!(Tokens, "type.body", SIZE, font_size),
        field!(Tokens, "type.label", SIZE, label_size),
        field!(Tokens, "motion.fast", SECONDS, motion.fast),
        field!(Tokens, "motion.normal", SECONDS, motion.normal),
        field!(Tokens, "motion.slow", SECONDS, motion.slow),
        field!(Tokens, "motion.ease", Kind::Quad(-4.0, 4.0), motion.ease),
        field!(Tokens, "reduced_motion", Kind::Bool, reduced_motion),
        field!(Tokens, "density", Kind::Num(0.5, 4.0), density),
    ]
}

fn theme_fields() -> Vec<Field<Theme>> {
    vec![
        field!(Theme, "row_h", Kind::Num(8.0, 256.0), row_h),
        field!(Theme, "control_h", Kind::Num(8.0, 256.0), control_h),
        field!(Theme, "ring.color", COLOR, ring.color),
        field!(Theme, "ring.width", LEN, ring.width),
        field!(Theme, "ring.sides", Kind::Sides, ring.sides),
        field!(Theme, "scrollbar_w", Kind::Num(0.0, 64.0), scrollbar_w),
        field!(Theme, "scrollbar_gap", Kind::Num(0.0, 64.0), scrollbar_gap),
    ]
}

fn look_fields() -> Vec<Field<Look>> {
    vec![
        field!(Look, "fill", COLOR, fill),
        field!(Look, "gradient", Kind::OptColor, gradient),
        field!(Look, "angle", Kind::Num(-360.0, 360.0), angle),
        field!(Look, "border", COLOR, border),
        field!(Look, "border_w", Kind::Num(0.0, 64.0), border_w),
        field!(Look, "sides", Kind::Sides, sides),
        field!(Look, "radius", Kind::Quad(0.0, 4096.0), radius),
        field!(Look, "shadow.color", COLOR, shadows[0].color),
        field!(Look, "shadow.x", OFFSET, shadows[0].offset[0]),
        field!(Look, "shadow.y", OFFSET, shadows[0].offset[1]),
        field!(Look, "shadow.blur", LEN, shadows[0].blur),
        field!(Look, "shadow.spread", OFFSET, shadows[0].spread),
        field!(Look, "shadow.inner", Kind::Bool, shadows[0].inner),
        field!(Look, "shadow2.color", COLOR, shadows[1].color),
        field!(Look, "shadow2.x", OFFSET, shadows[1].offset[0]),
        field!(Look, "shadow2.y", OFFSET, shadows[1].offset[1]),
        field!(Look, "shadow2.blur", LEN, shadows[1].blur),
        field!(Look, "shadow2.spread", OFFSET, shadows[1].spread),
        field!(Look, "shadow2.inner", Kind::Bool, shadows[1].inner),
        field!(Look, "text", COLOR, text),
        field!(Look, "font.family", Kind::Int(0, 255), font.family),
        field!(Look, "font.size", REL_SIZE, font.size),
        field!(Look, "font.weight", Kind::Int(100, 1000), font.weight),
        field!(Look, "font.tracking", TRACKING, font.tracking),
        field!(Look, "font.caps", Kind::Bool, font.caps),
        field!(Look, "opacity", UNIT, opacity),
        field!(Look, "ring.color", COLOR, ring.color),
        field!(Look, "ring.width", LEN, ring.width),
        field!(Look, "ring.sides", Kind::Sides, ring.sides),
        field!(Look, "selection", COLOR, selection),
        field!(Look, "caret", COLOR, caret),
    ]
}

/// The widget kinds a theme file can pick a body for (`KIND.body = NAME`).
const BODY_KINDS: [&str; 3] = ["toggle", "slider", "progress"];

const STATES: [&str; 6] = ["idle", "hover", "pressed", "focus", "disabled", "checked"];

fn state_mut<'a>(s: &'a mut States, name: &str) -> Option<&'a mut Look> {
    Some(match name {
        "idle" => &mut s.idle,
        "hover" => &mut s.hover,
        "pressed" => &mut s.pressed,
        "focus" => &mut s.focus,
        "disabled" => &mut s.disabled,
        "checked" => &mut s.checked,
        _ => return None,
    })
}

const SIDE_NAMES: [(&str, u8); 4] = [("top", TOP), ("right", RIGHT), ("bottom", BOTTOM), ("left", LEFT)];

fn parse_value(kind: Kind, text: &str) -> Result<Value, String> {
    let num = |s: &str, lo: f32, hi: f32| -> Result<f32, String> {
        let v: f32 = s.parse().map_err(|_| format!("`{s}` is not a number"))?;
        if v.is_finite() && v >= lo && v <= hi { Ok(v) } else { Err(format!("{s} is outside {lo}..={hi}")) }
    };
    let col = |s: &str| color::hex(s).map_err(|e| format!("`{s}`: {e}"));
    Ok(match kind {
        Kind::Color => Value::C(col(text)?),
        Kind::OptColor => Value::O(if text == "none" { None } else { Some(col(text)?) }),
        Kind::Num(lo, hi) => Value::F(num(text, lo, hi)?),
        Kind::Int(lo, hi) => {
            let v: i64 = text.parse().map_err(|_| format!("`{text}` is not a whole number"))?;
            if v < lo || v > hi {
                return Err(format!("{v} is outside {lo}..={hi}"));
            }
            Value::I(v)
        }
        Kind::Bool => Value::B(match text {
            "true" => true,
            "false" => false,
            _ => return Err(format!("`{text}` is not true or false")),
        }),
        Kind::Quad(lo, hi) => {
            let parts: Vec<&str> = text.split_whitespace().collect();
            match parts.len() {
                1 => Value::Q([num(parts[0], lo, hi)?; 4]),
                4 => Value::Q([num(parts[0], lo, hi)?, num(parts[1], lo, hi)?, num(parts[2], lo, hi)?, num(parts[3], lo, hi)?]),
                _ => return Err("expected one number or four".to_owned()),
            }
        }
        Kind::Sides => {
            let mut mask = 0u8;
            for word in text.split_whitespace() {
                match (word, SIDE_NAMES.iter().find(|(n, _)| *n == word)) {
                    ("all", _) => mask = 0,
                    (_, Some((_, bit))) => mask |= bit,
                    _ => return Err(format!("`{word}` is not a side (top, right, bottom, left, all)")),
                }
            }
            Value::S(mask)
        }
    })
}

fn write_value(v: Value) -> String {
    match v {
        Value::C(c) => color::to_hex_string(c),
        Value::O(None) => "none".to_owned(),
        Value::O(Some(c)) => color::to_hex_string(c),
        Value::F(x) => format!("{x}"),
        Value::I(x) => format!("{x}"),
        Value::B(x) => format!("{x}"),
        Value::Q(q) if q.iter().all(|x| *x == q[0]) => format!("{}", q[0]),
        Value::Q(q) => format!("{} {} {} {}", q[0], q[1], q[2], q[3]),
        Value::S(0) => "all".to_owned(),
        Value::S(m) => SIDE_NAMES.iter().filter(|(_, b)| m & b != 0).map(|(n, _)| *n).collect::<Vec<_>>().join(" "),
    }
}

/// A line without its comment: a `#` that starts the line or is followed by a space or the end (a
/// colour's `#` is followed by a digit).
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'#' && (line[..i].trim().is_empty() || bytes.get(i + 1).is_none_or(|n| n.is_ascii_whitespace())) {
            return &line[..i];
        }
    }
    line
}

fn apply<T>(fields: &[Field<T>], x: &mut T, key: &str, text: &str) -> Option<Result<(), String>> {
    let f = fields.iter().find(|f| f.key == key)?;
    Some(parse_value(f.kind, text).map(|v| (f.set)(x, v)))
}

impl Theme {
    /// A theme from a flat `key = value` file over `base`. `#` starts a comment. Keys: `tokens.NAME`
    /// (any token key re-derives every look from the new tokens before the other keys apply),
    /// `row_h`, `control_h`, `ring.*`, `scrollbar_w`, `scrollbar_gap`, and `PART.STATE.FIELD` (for
    /// example `button.hover.fill = #223344`, `button.idle.radius = 6`, `popup.idle.shadow.blur = 8`)
    /// or `PART.FIELD` for every state of a part. Values: colours `#rgb[a]`/`#rrggbb[aa]`, numbers,
    /// `true`/`false`, `none`, side names. Every unknown key and malformed or out-of-range value is
    /// reported with its line; nothing is applied then.
    /// A host reloads a theme file by calling `parse` again when `Watch::poll` reports a change.
    pub fn parse(text: &str, base: &Theme) -> Result<Theme, Vec<ThemeError>> {
        Theme::parse_bodies(text, base, &|_, name| (name == "default").then_some(BodyId::DEFAULT))
    }

    /// `parse` with `bodies` resolving the names of `KIND.body = NAME` (`toggle`, `slider`, `progress`): `bodies(kind, name)` is
    /// the registered body's id, `None` for a name that is not registered (a `ThemeError`). `default` is always the built-in one.
    /// `Ui::parse_theme` passes its own registry.
    pub fn parse_bodies(text: &str, base: &Theme, bodies: &dyn Fn(&str, &str) -> Option<BodyId>) -> Result<Theme, Vec<ThemeError>> {
        let (tf, thf, lf) = (token_fields(), theme_fields(), look_fields());
        let mut errors = Vec::new();
        let mut lines = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            match line.split_once('=') {
                Some((k, v)) if !k.trim().is_empty() && !v.trim().is_empty() => lines.push((index + 1, k.trim(), v.trim())),
                _ => errors.push(ThemeError { line: index + 1, message: format!("expected `key = value`, found `{line}`") }),
            }
        }
        let mut tokens = base.tokens;
        let mut retoken = false;
        for &(line, key, value) in &lines {
            if let Some(name) = key.strip_prefix("tokens.") {
                retoken = true;
                match apply(&tf, &mut tokens, name, value) {
                    None => errors.push(ThemeError { line, message: format!("unknown token `{name}`") }),
                    Some(Err(e)) => errors.push(ThemeError { line, message: format!("{key}: {e}") }),
                    Some(Ok(())) => {}
                }
            }
        }
        let mut theme = if retoken { Theme { tokens, body: base.body, ..Theme::from_tokens(tokens) } } else { *base };
        for &(line, key, value) in &lines {
            if key.starts_with("tokens.") {
                continue;
            }
            if let Some(kind) = key.strip_suffix(".body").filter(|k| BODY_KINDS.contains(k)) {
                let id = if value == "default" { Some(BodyId::DEFAULT) } else { bodies(kind, value) };
                match (id, kind) {
                    (None, _) => errors.push(ThemeError { line, message: format!("{key}: no {kind} body is registered as `{value}`") }),
                    (Some(id), "toggle") => theme.body.toggle = id,
                    (Some(id), "slider") => theme.body.slider = id,
                    (Some(id), _) => theme.body.progress = id,
                }
                continue;
            }
            let result = apply(&thf, &mut theme, key, value).or_else(|| {
                let (part, rest) = key.split_once('.')?;
                let states = widget_mut(&mut theme, part)?;
                if let Some((state, field)) = rest.split_once('.') {
                    if let Some(look) = state_mut(states, state) {
                        return apply(&lf, look, field, value);
                    }
                }
                // `part.field`: every state
                let mut all = Some(Ok(()));
                for s in STATES {
                    all = apply(&lf, state_mut(states, s).unwrap(), rest, value);
                }
                all
            });
            match result {
                None => errors.push(ThemeError { line, message: format!("unknown key `{key}`") }),
                Some(Err(e)) => errors.push(ThemeError { line, message: format!("{key}: {e}") }),
                Some(Ok(())) => {}
            }
        }
        errors.sort_by_key(|e| e.line);
        if errors.is_empty() { Ok(theme) } else { Err(errors) }
    }

    /// Every field as a theme file: `Theme::parse(&t.write(), &any)` gives `t` back. Bodies are not written: a body's name lives in
    /// the `Ui` that registered it, so a theme with an overridden body round-trips through `Ui::parse_theme` only by its text.
    // ponytail: no `KIND.body` lines; write them when a `Ui` can name its bodies for a saved theme.
    pub fn write(&self) -> String {
        use std::fmt::Write;
        let mut out = String::from("# pxui theme: `key = value`, see Theme::parse\n");
        for f in token_fields() {
            let _ = writeln!(out, "tokens.{} = {}", f.key, write_value((f.get)(&self.tokens)));
        }
        for f in theme_fields() {
            let _ = writeln!(out, "{} = {}", f.key, write_value((f.get)(self)));
        }
        let lf = look_fields();
        // a field every state shares is one `part.field` line
        for part in WIDGETS {
            let mut states = *widget(self, part);
            for f in &lf {
                let values: Vec<Value> = STATES.iter().map(|s| (f.get)(state_mut(&mut states, s).unwrap())).collect();
                if values.iter().all(|v| *v == values[0]) {
                    let _ = writeln!(out, "{part}.{} = {}", f.key, write_value(values[0]));
                } else {
                    for (s, v) in STATES.iter().zip(values) {
                        let _ = writeln!(out, "{part}.{s}.{} = {}", f.key, write_value(v));
                    }
                }
            }
        }
        out
    }
}

/// A theme file watched the designed way: its modification time polled at most once per `period`
/// seconds, driven by the host's wait (sleep until `next_poll`, as for `Ui::repaint_at`). No thread,
/// no OS notification API, nothing between polls.
pub struct Watch {
    path: std::path::PathBuf,
    stamp: Option<std::time::SystemTime>,
    next: f64,
    period: f64,
}

impl Watch {
    /// Watches `path` (its current modification time is the baseline), polling every `period` s.
    pub fn new(path: &std::path::Path, period: f64) -> Watch {
        let mut w = Watch { path: path.to_path_buf(), stamp: None, next: 0.0, period };
        w.stamp = w.modified();
        w
    }

    fn modified(&self) -> Option<std::time::SystemTime> {
        std::fs::metadata(&self.path).and_then(|m| m.modified()).ok()
    }

    /// At time `now` (seconds, the host's clock): whether the file changed (or appeared or went)
    /// since the last poll that saw a change. Reads the file system only when a poll is due.
    pub fn poll(&mut self, now: f64) -> bool {
        if now < self.next {
            return false;
        }
        self.next = now + self.period;
        let stamp = self.modified();
        let changed = stamp != self.stamp;
        self.stamp = stamp;
        changed
    }

    /// When the next poll is due: a host waiting for input wakes by then.
    pub fn next_poll(&self) -> f64 {
        self.next
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}
