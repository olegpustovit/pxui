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
    /// Seconds for a press change (`Tokens::transition` uses it when only `pressed` differs).
    pub fast: f32,
    /// Seconds for every other state change.
    pub normal: f32,
    /// Seconds for slow movement (the engine's longer transitions; `Tokens::transition` does not read it).
    pub slow: f32,
    /// CSS cubic-bezier control points `[x1, y1, x2, y2]`; x1 and x2 must lie in 0 to 1.
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
    /// The ground colour of docked panels.
    pub panel: Color,
    /// Text and line ink.
    pub foreground: Color,
    /// The fill of buttons and other controls.
    pub control: Color,
    /// The fill of fields and floating sheets.
    pub input: Color,
    /// The fill of slider and progress tracks.
    pub track: Color,
    /// The highlight colour (focus, selection, marks).
    pub accent: Color,
    /// The error colour; always `theme::invalid()`, never read from the C snapshot.
    pub invalid: Color,
    /// The ground text sits on when it is not `panel`; `None` means `panel`.
    pub ink_ground: crate::node::OptColor,
    /// The spacing scale in points, smallest to largest.
    pub space: [f32; 4],
    /// The corner radii in points: small, medium, large.
    pub radius: [f32; 3],
    /// Point size of display text.
    pub display_size: i64,
    /// Point size of titles.
    pub title_size: i64,
    /// Point size of body text.
    pub font_size: i64,
    /// Point size of captions.
    pub label_size: i64,
    /// Durations and easing for state transitions.
    pub motion: Motion,
    /// When set, transitions are instant.
    pub reduced_motion: bool,
    /// Device pixels per point.
    pub density: f32,
}

impl Tokens {
    /// Converts the ABI-safe snapshot into its Rust form (`invalid` is filled with `theme::invalid()`; the snapshot has no such colour).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::ThemeTokens, theme::Tokens};
    /// let tokens = Tokens::default();
    /// assert_eq!(Tokens::from_ffi(ThemeTokens::from(tokens)), tokens);
    /// ```
    pub fn from_ffi(value: crate::ffi::ThemeTokens) -> Self { value.into() }

    /// The curve a widget follows when its interaction state goes `from` to `to`: instant for reduced motion or no change,
    /// `motion.fast` when only `pressed` differs, else `motion.normal`, with the `motion.ease` bezier.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::theme::{Curve, State, Tokens};
    /// let idle = State::default();
    /// let down = State { pressed: true, ..idle };
    /// let mut tokens = Tokens::default();
    /// tokens.motion.fast = 0.1;
    /// let Curve::Bezier { duration, .. } = tokens.transition(idle, down) else { unreachable!() };
    /// assert_eq!(duration, 0.1);
    /// assert!(tokens.transition(idle, idle).instant());
    /// ```
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

    /// Whether state changes animate at all: reduced motion is off and `motion.fast` or `motion.normal` is above zero (`slow` is not read).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::theme::Tokens;
    /// let mut tokens = Tokens::default();
    /// assert!(!tokens.moves());
    /// tokens.motion.normal = 0.2;
    /// assert!(tokens.moves());
    /// ```
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
/// that kind (`Ui::register_toggle_body` and its siblings), numbered from 1 in the order the process first saw its name (the same id in every `Ui`).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BodyId(pub u16);

impl BodyId {
    /// The built-in body.
    pub const DEFAULT: BodyId = BodyId(0);
}

/// An app-defined colour token: an index into the light/dark colour table a `Ui` installs (`Ui::set_colors`,
/// `pxui_ui_set_colors`). Reads (`Ui::color`, `Paint::color`) return the light or dark entry for the theme in effect
/// (dark when `dark(&tokens)` holds); a token at or past `ColorToken::LIMIT`, or past the installed count, reads transparent.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ColorToken(pub u16);

impl ColorToken {
    /// The table's fixed capacity: tokens 0..64.
    pub const LIMIT: usize = 64;
}

/// The widget kinds that have a replaceable body: `kind register_fn BodyType AbiConfig` (the config as it crosses the C ABI, `pxui_types::ffi`), one line each. `Bodies` here, the registry and its
/// `Ui::register_*_body` functions and the theme file's `KIND.body = NAME` are all generated from it (`pxui_core::bodies`), so a
/// kind is added by one line here plus its config, default body and dispatch. Append only: a kind's place is its field's offset.
#[macro_export]
macro_rules! body_kinds {
    ($m:ident) => {
        $m! {
            toggle register_toggle_body ToggleBody PxuiToggleConfig,
            slider register_slider_body SliderBody PxuiSliderConfig,
            progress register_progress_body ProgressBody PxuiProgressConfig,
            button register_button_body ButtonBody PxuiButtonConfig,
            icon_button register_icon_button_body IconButtonBody PxuiIconButtonConfig,
            checkbox register_checkbox_body CheckBody PxuiCheckConfig,
            radio register_radio_body CheckBody PxuiCheckConfig,
            spinner register_spinner_body SpinnerBody PxuiSpinnerConfig,
            badge register_badge_body BadgeBody PxuiBadgeConfig,
            label register_label_body LabelBody PxuiLabelConfig,
            footer register_footer_body FooterBody PxuiFooterConfig,
            grip register_grip_body GripBody PxuiGripConfig,
            panel register_panel_body PanelBody PxuiPanelConfig,
            window register_window_body PanelBody PxuiPanelConfig,
            popup register_popup_body PanelBody PxuiPanelConfig,
            modal register_modal_body PanelBody PxuiPanelConfig,
            dialog register_dialog_body DialogBody PxuiDialogConfig,
            inspector_section register_inspector_section_body SectionBody PxuiSectionConfig,
            inspector_row register_inspector_row_body InspectorRowBody PxuiInspectorRowConfig,
            split register_split_body SplitBody PxuiSplitConfig,
            scroll_view register_scroll_view_body ScrollBody PxuiScrollConfig,
            toast register_toast_body ToastBody PxuiToastConfig,
            tip register_tip_body TipBody PxuiTipConfig,
            tab register_tab_body TabBody PxuiTabConfig,
            segment register_segment_body SegmentBody PxuiSegmentConfig,
            dropdown register_dropdown_body DropdownBody PxuiDropdownConfig,
            menu_bar register_menu_bar_body MenuItemBody PxuiMenuItemConfig,
            menu_item register_menu_item_body MenuItemBody PxuiMenuItemConfig,
            context_menu register_context_menu_body MenuRowBody PxuiMenuRowConfig,
            list_view register_list_view_body RowBody PxuiRowConfig,
            tree register_tree_body TreeRowBody PxuiTreeRowConfig,
            table_view register_table_view_body RowBody PxuiRowConfig,
            table_head register_table_head_body TableHeadBody PxuiTableHeadConfig,
            table register_table_body TableCellBody PxuiTableCellConfig,
            picker register_picker_body PickerRowBody PxuiPickerRowConfig,
            search_field register_search_field_body SearchFieldBody PxuiSearchFieldConfig,
            text_field register_text_field_body TextFieldBody PxuiTextFieldConfig,
            line_field register_line_field_body LineFieldBody PxuiLineFieldConfig,
            value_field register_value_field_body ValueFieldBody PxuiValueFieldConfig,
            picker_field register_picker_field_body PickerFieldBody PxuiPickerFieldConfig,
        }
    };
}

macro_rules! bodies {
    ($($kind:ident $register:ident $body:ident $config:ident),* $(,)?) => {
        /// The body each widget kind builds with, in a theme: ids, never function pointers, so a `Theme` stays `Copy` and comparable
        /// and a theme file can pick one by name (`toggle.body = checkbox`). One field per `body_kinds!` line, in its order; grows at
        /// the end only.
        #[repr(C)]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub struct Bodies {
            $(#[doc = concat!("Body id for the `", stringify!($kind), "` widget kind.")] pub $kind: BodyId,)*
        }

        impl Bodies {
            /// The kind names, in field order: what a theme file's `KIND.body` may name.
            pub const KINDS: &'static [&'static str] = &[$(stringify!($kind)),*];

            /// Sets the body of the widget kind named `kind` (one of `Bodies::KINDS`) to `id`; false, changing nothing, for a name that is no kind.
            ///
            /// Allocates: never.
            /// Panics: never.
            ///
            /// ```
            /// use pxui_types::theme::{Bodies, BodyId};
            /// let mut bodies = Bodies::default();
            /// assert!(bodies.set("toggle", BodyId(1)));
            /// assert_eq!(bodies.toggle, BodyId(1));
            /// assert!(!bodies.set("no_such_kind", BodyId(1)));
            /// ```
            pub fn set(&mut self, kind: &str, id: BodyId) -> bool {
                match kind {
                    $(stringify!($kind) => self.$kind = id,)*
                    _ => return false,
                }
                true
            }
        }
    };
}

body_kinds!(bodies);

macro_rules! body_kind_numbers {
    ($($kind:ident $register:ident $body:ident $config:ident),* $(,)?) => {
        /// The number of each widget kind in `pxui_body_register`: `Bodies`' field order, numbered once and never renumbered.
        #[repr(u32)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[allow(non_camel_case_types)]
        pub enum BodyKind {
            $(#[doc = concat!("The `", stringify!($kind), "` widget kind.")] $kind),
            *,
        }
    };
}
body_kinds!(body_kind_numbers);

/// A node a widget's behaviour finds by role (`Ui::part`): the toggle's hit shape, the slider's rail and thumb, the progress
/// track and its busy bar, a spinner's dots. Numbered once, never renumbered; every part has its own number, and a part's key is derived
/// from the widget's key and that number alone (`pxui_core::ui_kit::part_key`).
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
    /// Dialog button `i` (the dialog's body builds one clickable node for each of its buttons; behaviour reads which was pressed
    /// and sets its role and name).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::theme::Part;
    /// assert!(Part::valid(Part::button(3).0));
    /// ```
    pub const fn button(i: u8) -> Part {
        Part(1024 + i as u32)
    }
    /// A dialog's title bar: behaviour gives it the heading role and the dialog's title as name.
    pub const TITLE: Part = Part(6);
    /// A tree row's disclosure mark: behaviour reads its presses (a click expands or collapses the row); the built-in body builds one on
    /// every row, a spacer when the row has no children.
    pub const DISCLOSURE: Part = Part(7);
    /// A search field's clear button: behaviour reads its presses (a click clears the query), and gives it the button role and the name
    /// Clear; the built-in body builds one while the query is not empty.
    pub const CLEAR: Part = Part(8);
    /// A text field's editing surface: where its value shows and its caret and selection are placed and painted, and what takes its
    /// presses (the field's own box when a body builds none).
    pub const EDITOR: Part = Part(9);
    /// Dot `i` of a spinner, which `busy_step` turns.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::theme::Part;
    /// assert_eq!(Part::dot(0), Part(16));
    /// ```
    pub const fn dot(i: u8) -> Part {
        Part(16 + i as u32)
    }
    /// Whether the number names a part (what `pxui_part` accepts): the named ones (1 to 9), the dots (16 to 271) and the dialog buttons (1024 to 1279).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::theme::Part;
    /// assert!(Part::valid(Part::HIT.0));
    /// assert!(!Part::valid(0) && !Part::valid(10));
    /// ```
    pub const fn valid(n: u32) -> bool {
        matches!(n, 1..=9 | 16..=271 | 1024..=1279)
    }
}

macro_rules! snapshot {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {
        /// The tokens and one `States` per kit part: the engine's `Theme` (the same C layout), borrowed from the library.
        #[repr(C)]
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct ThemeSnapshot {
            /// The colour, size and motion tokens.
            pub tokens: Tokens,
            /// Height of a list or inspector row in points.
            pub row_h: f32,
            /// Height of a button or field in points.
            pub control_h: f32,
            /// The focus ring shared by the kit.
            pub ring: Ring,
            /// Width of the scroll thumb in points.
            pub scrollbar_w: f32,
            /// Gap between the scroll thumb and the view's edge in points.
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

/// The engine's theme under its short name: the same type as `ThemeSnapshot`.
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

/// Default body text size in points.
pub const FONT_SIZE: i64 = 13;
/// Default caption size in points.
pub const LABEL_SIZE: i64 = 11;
/// Default title size in points.
pub const TITLE_SIZE: i64 = 20;
/// Default display text size in points.
pub const DISPLAY_SIZE: i64 = 40;

/// The default tokens: a light theme with orange accent, 13-point body text, 0 radii and all motion durations 0 (nothing animates until `motion` is set).
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::theme;
/// let tokens = theme::default();
/// assert_eq!(tokens.font_size, theme::FONT_SIZE);
/// assert!(!theme::dark(&tokens));
/// ```
pub fn default() -> Tokens {
    let hex = |s| color::hex(s).expect("valid theme color constant");
    Tokens {
        panel: hex("#f4f5f0"), foreground: hex("#222b2b"), control: hex("#dce3de"),
        input: hex("#ffffff"), track: hex("#e3e8e4"), accent: hex("#f0481f"),
        invalid: invalid(), ink_ground: crate::node::OptColor::None, space: [4.0, 8.0, 12.0, 16.0], radius: [0.0; 3],
        display_size: DISPLAY_SIZE, title_size: TITLE_SIZE, font_size: FONT_SIZE, label_size: LABEL_SIZE,
        motion: Motion { fast: 0.0, normal: 0.0, slow: 0.0, ease: [0.25, 0.1, 0.25, 1.0] },
        reduced_motion: false, density: 1.0,
    }
}

/// The colour text sits on: `ink_ground` when set, else `panel`.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::theme::{self, Tokens};
/// let tokens = Tokens::default();
/// assert_eq!(theme::ground(&tokens), tokens.panel);
/// ```
pub fn ground(theme: &Tokens) -> Color { theme.ink_ground.get().unwrap_or(theme.panel) }
/// Secondary text ink: the foreground 32% of the way toward the ground.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::theme::{self, Tokens};
/// let tokens = Tokens::default();
/// assert_ne!(theme::ink_2(&tokens), tokens.foreground);
/// ```
pub fn ink_2(theme: &Tokens) -> Color { color::blend(theme.foreground, ground(theme), 0.32) }
/// Tertiary text ink: the foreground 52.1% of the way toward the ground, fainter than `ink_2`.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::theme::{self, Tokens};
/// let tokens = Tokens::default();
/// assert_ne!(theme::ink_3(&tokens), theme::ink_2(&tokens));
/// ```
pub fn ink_3(theme: &Tokens) -> Color { color::blend(theme.foreground, ground(theme), 0.521) }
/// Muted text ink: the same colour as `ink_2`.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::theme::{self, Tokens};
/// let tokens = Tokens::default();
/// assert_eq!(theme::muted(&tokens), theme::ink_2(&tokens));
/// ```
pub fn muted(theme: &Tokens) -> Color { ink_2(theme) }
/// The standard border colour: the foreground at alpha 77 of 255.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::theme::{self, Tokens};
/// assert_eq!(theme::border(&Tokens::default()).a, 77);
/// ```
pub fn border(theme: &Tokens) -> Color { color::with_alpha(theme.foreground, 77) }
/// A lighter edge colour: the foreground at alpha 38 of 255.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::theme::{self, Tokens};
/// assert_eq!(theme::edge(&Tokens::default()).a, 38);
/// ```
pub fn edge(theme: &Tokens) -> Color { color::with_alpha(theme.foreground, 38) }
/// The faintest line colour: the foreground at alpha 20 of 255.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::theme::{self, Tokens};
/// assert_eq!(theme::faint_border(&Tokens::default()).a, 20);
/// ```
pub fn faint_border(theme: &Tokens) -> Color { color::with_alpha(theme.foreground, 20) }
/// The fill of a hovered field: `input` 11% of the way toward `accent`.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::theme::{self, Tokens};
/// let tokens = Tokens::default();
/// assert_ne!(theme::hover_fill(&tokens), tokens.input);
/// ```
pub fn hover_fill(theme: &Tokens) -> Color { color::blend(theme.input, theme.accent, 0.11) }
/// The fill of a pressed control: `control` 18% of the way toward `accent`.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::theme::{self, Tokens};
/// let tokens = Tokens::default();
/// assert_ne!(theme::pressed_fill(&tokens), tokens.control);
/// ```
pub fn pressed_fill(theme: &Tokens) -> Color { color::blend(theme.control, theme.accent, 0.18) }
/// A translucent wash of the accent: `accent` at alpha 41 of 255.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::theme::{self, Tokens};
/// assert_eq!(theme::tint(&Tokens::default()).a, 41);
/// ```
pub fn tint(theme: &Tokens) -> Color { color::with_alpha(theme.accent, 41) }
/// The fixed error colour `#c2255c`, the same in every theme (a theme cannot change it).
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// assert_eq!(pxui_types::theme::invalid(), pxui_types::geom::color::rgb(0xc2, 0x25, 0x5c));
/// ```
pub fn invalid() -> Color { color::hex("#c2255c").expect("valid invalid color constant") }
/// Whether the theme is dark: the `panel` colour's red, green and blue sum to under 384 (an average below 128); `ink_ground` is not read.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::theme::{self, Tokens};
/// assert!(!theme::dark(&Tokens::default()));
/// ```
pub fn dark(theme: &Tokens) -> bool {
    (theme.panel.r as u32 + theme.panel.g as u32 + theme.panel.b as u32) < 384
}

/// A box shadow: outer, or `inner` (CSS `inset`); none while `color` is transparent.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    /// The shadow colour; transparent draws nothing.
    pub color: Color,
    /// Horizontal and vertical offset in points.
    pub offset: [f32; 2],
    /// Blur radius in points.
    pub blur: f32,
    /// Points the shadow grows (negative: shrinks) before blurring.
    pub spread: f32,
    /// An inset shadow, drawn inside the box.
    pub inner: bool,
}

impl Shadow {
    /// No shadow (transparent).
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
    /// The `pxui_text` font index.
    pub family: u16,
    /// Points: 0 is the UI font size, negative is that many points below it (at least 8).
    pub size: i64,
    /// Weight, 400 regular, 700 bold.
    pub weight: u16,
    /// Extra spacing after every glyph, in ems.
    pub tracking: f32,
    /// Upper-case the text.
    pub caps: bool,
}

impl Font {
    /// Font 0 at the UI body size, regular weight, no tracking, no caps.
    pub const BODY: Font = Font {
        family: 0,
        size: 0,
        weight: 400,
        tracking: 0.0,
        caps: false,
    };
    /// Sets the size in points (0 the UI body size, negative relative to it).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Font;
    /// const F: Font = Font::BODY.size(14);
    /// assert_eq!(F.size, 14);
    /// ```
    #[inline]
    pub const fn size(mut self, points: i64) -> Font {
        self.size = points;
        self
    }
    /// Sets the weight (400 regular, 700 bold).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Font;
    /// const F: Font = Font::BODY.weight(700);
    /// assert_eq!(F.weight, 700);
    /// ```
    #[inline]
    pub const fn weight(mut self, weight: u16) -> Font {
        self.weight = weight;
        self
    }
    /// The size in points this font draws at when the UI body size is `body`: its own `size` if positive, `body` for 0, else `body` less
    /// its magnitude, never below 8.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Font;
    /// assert_eq!(Font::BODY.points(13), 13);
    /// assert_eq!(Font::BODY.size(20).points(13), 20);
    /// assert_eq!(Font::BODY.size(-2).points(13), 11);
    /// assert_eq!(Font::BODY.size(-9).points(13), 8);
    /// ```
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
    /// The ring colour; transparent draws nothing.
    pub color: Color,
    /// Width in points, inside the box.
    pub width: f32,
    /// `draw::flags::TOP | ...` sides to draw; 0 is all four.
    pub sides: u8,
}

impl Ring {
    /// No ring (transparent).
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
    /// The background colour (the gradient's start).
    pub fill: Color,
    /// The gradient's end colour; `None` for a solid fill.
    pub gradient: crate::node::OptColor,
    /// Gradient direction in degrees, CSS-style (180 runs top to bottom).
    pub angle: f32,
    /// The border colour.
    pub border: Color,
    /// Border width in points, inside the box.
    pub border_w: f32,
    /// Border sides as `draw::flags` bits; 0 is all four.
    pub sides: u8,
    /// Corner radii in points: top left, top right, bottom right, bottom left.
    pub radius: [f32; 4],
    /// Two shadows, each outer or inner by its own `inner` flag.
    pub shadows: [Shadow; 2],
    /// The text colour.
    pub text: Color,
    /// The text font.
    pub font: Font,
    /// Multiplies every alpha, 0 to 1.
    pub opacity: f32,
    /// The outline drawn while the node holds focus.
    pub ring: Ring,
    /// The selection colour of a text widget.
    pub selection: Color,
    /// The caret colour of a text widget.
    pub caret: Color,
    /// A dashed square border (4 points on, 3 off).
    pub dashed: bool,
}

impl Look {
    /// Paints nothing: transparent colours, 180 degree angle, full opacity, the body font.
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

    /// Sets the fill colour.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Rgba, Look};
    /// const L: Look = Look::NONE.fill(Rgba { r: 1, g: 2, b: 3, a: 255 });
    /// assert_eq!(L.fill.g, 2);
    /// ```
    #[inline]
    pub const fn fill(mut self, color: Color) -> Look {
        self.fill = color;
        self
    }
    /// Sets all four corner radii, in points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Look;
    /// const L: Look = Look::NONE.radius(6.0);
    /// assert_eq!(L.radius, [6.0; 4]);
    /// ```
    #[inline]
    pub const fn radius(mut self, points: f32) -> Look {
        self.radius = [points; 4];
        self
    }
    /// Sets the border colour and its width in points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Rgba, Look};
    /// const L: Look = Look::NONE.border(Rgba { r: 9, g: 9, b: 9, a: 255 }, 1.0);
    /// assert_eq!(L.border_w, 1.0);
    /// ```
    #[inline]
    pub const fn border(mut self, color: Color, width: f32) -> Look {
        self.border = color;
        self.border_w = width;
        self
    }
    /// Sets the text colour.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Rgba, Look};
    /// const L: Look = Look::NONE.text(Rgba { r: 1, g: 2, b: 3, a: 255 });
    /// assert_eq!(L.text.b, 3);
    /// ```
    #[inline]
    pub const fn text(mut self, color: Color) -> Look {
        self.text = color;
        self
    }
    /// Sets the text font.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Font, Look};
    /// const L: Look = Look::NONE.font(Font::BODY.size(14));
    /// assert_eq!(L.font.size, 14);
    /// ```
    #[inline]
    pub const fn font(mut self, font: Font) -> Look {
        self.font = font;
        self
    }

    /// The look `t` of the way from `self` to `to`: colours, widths, radii, angle, opacity and shadow geometry interpolate; the rest
    /// (font, border and ring sides, `dashed`) is `to`'s from the start.
    ///
    /// `t` is not clamped: an overshooting curve extrapolates, and colour channels saturate at 0 and 255. A gradient present on either
    /// look is interpolated, the other side standing in its `fill`. A shadow's `inner` flag is `to`'s once `to`'s shadow is visible.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::color, Look};
    /// let a = Look::NONE.fill(color::rgb(0, 0, 0));
    /// let b = Look::NONE.fill(color::rgb(255, 255, 255));
    /// assert_eq!(a.lerp(&b, 0.0).fill, color::rgb(0, 0, 0));
    /// assert_eq!(a.lerp(&b, 1.0).fill, color::rgb(255, 255, 255));
    /// assert_eq!(a.lerp(&b, 0.5).fill, color::rgb(128, 128, 128));
    /// ```
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

/// Progress over seconds: a cubic Bezier, a critically damped spring, a sampled piecewise linear
/// curve, or CSS end steps. Tables hold 2..=16 samples at strictly increasing normalized positions
/// from 0 to 1; values start at 0 and end at 1, and may overshoot in between.
#[repr(C, u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    /// A CSS cubic-bezier over `duration` seconds with control points `[x1, y1, x2, y2]`.
    Bezier { duration: f32, points: [f32; 4] },
    /// A critically damped spring of angular frequency `omega` per second; it has no fixed duration.
    Spring { omega: f32 },
    /// Piecewise linear: `count` (2 to 16) samples of `values` at `positions` (0 to 1) over `duration` seconds.
    Table { duration: f32, count: u32, positions: [f32; 16], values: [f32; 16] },
    /// `count` equal end-steps over `duration` seconds, as CSS `steps(count, jump-end)`.
    Steps { duration: f32, count: u32 },
}

impl Curve {
    /// Whether every parameter is usable: finite, durations not negative, bezier x values in 0 to 1, a spring `omega` above 0, steps at
    /// least 1, and a table of 2 to 16 samples that start at position 0 and value 0, end at 1 and 1, with strictly increasing positions.
    ///
    /// The ABI boundary rejects a curve that is not valid.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::theme::Curve;
    /// assert!(Curve::Spring { omega: 12.0 }.valid());
    /// assert!(!Curve::Steps { duration: 1.0, count: 0 }.valid());
    /// ```
    pub fn valid(&self) -> bool {
        match self {
            Self::Bezier { duration, points } => Self::bezier_valid(*duration, points),
            Self::Spring { omega } => Self::spring_valid(*omega),
            Self::Table { duration, count, positions, values } => {
                if !duration.is_finite() || *duration < 0.0 || !(2..=16).contains(count) { return false; }
                let n = *count as usize;
                positions[0] == 0.0 && positions[n - 1] == 1.0 && values[0] == 0.0 && values[n - 1] == 1.0
                    && positions[..n].iter().all(|v| v.is_finite()) && values[..n].iter().all(|v| v.is_finite())
                    && positions[..n].windows(2).all(|w| w[0] < w[1])
            }
            Self::Steps { duration, count } => duration.is_finite() && *duration >= 0.0 && *count > 0,
        }
    }

    /// Whether bezier parameters are usable: `duration` finite and not negative, all four points finite, and x1 and x2 within 0 to 1
    /// (what `valid` checks for a `Bezier`, without building a `Curve`).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::theme::Curve;
    /// assert!(Curve::bezier_valid(0.2, &[0.25, 0.1, 0.25, 1.0]));
    /// assert!(!Curve::bezier_valid(0.2, &[1.5, 0.0, 0.0, 1.0]));
    /// ```
    #[inline]
    pub fn bezier_valid(duration: f32, points: &[f32; 4]) -> bool {
        duration.is_finite() && duration >= 0.0 && points.iter().all(|v| v.is_finite()) && (0.0..=1.0).contains(&points[0]) && (0.0..=1.0).contains(&points[2])
    }
    /// Whether a spring frequency is usable: finite and above 0 (what `valid` checks for a `Spring`).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::theme::Curve;
    /// assert!(Curve::spring_valid(8.0) && !Curve::spring_valid(0.0));
    /// ```
    #[inline]
    pub fn spring_valid(omega: f32) -> bool { omega.is_finite() && omega > 0.0 }

    /// Whether the run is over before it starts: a timed curve whose duration is not above 0 (a NaN counts); a spring never is.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::theme::Curve;
    /// assert!(Curve::Steps { duration: 0.0, count: 4 }.instant());
    /// assert!(!Curve::Spring { omega: 10.0 }.instant());
    /// ```
    pub fn instant(&self) -> bool {
        match self {
            Self::Bezier { duration, .. } | Self::Table { duration, .. } | Self::Steps { duration, .. } => !(*duration > 0.0),
            Self::Spring { .. } => false,
        }
    }

    /// Whether the run has ended `t` seconds in: a timed curve when `t` reaches its duration (whatever `at` returns by then); a spring
    /// when its progress has settled to 1.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::theme::Curve;
    /// let curve = Curve::Bezier { duration: 0.5, points: [0.25, 0.1, 0.25, 1.0] };
    /// assert!(!curve.finished(0.4) && curve.finished(0.5));
    /// ```
    pub fn finished(&self, t: f32) -> bool {
        match self {
            Self::Bezier { duration, .. } | Self::Table { duration, .. } | Self::Steps { duration, .. } => !(t < *duration),
            Self::Spring { .. } => self.at(t) >= 1.0,
        }
    }

    /// The curve with its duration set to `seconds` (any value, not validated); a `Spring` is returned unchanged.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::theme::Curve;
    /// let curve = Curve::Steps { duration: 1.0, count: 4 }.with_duration(2.0);
    /// assert_eq!(curve, Curve::Steps { duration: 2.0, count: 4 });
    /// ```
    pub fn with_duration(mut self, seconds: f32) -> Self {
        match &mut self {
            Self::Bezier { duration, .. } | Self::Table { duration, .. } | Self::Steps { duration, .. } => *duration = seconds,
            Self::Spring { .. } => {},
        }
        self
    }

    /// The progress `t` seconds into the run: 0 at the start, exactly 1 once it is over or when a duration is 0; a bezier, table or spring
    /// curve may pass 1 in between (an overshoot) and `Steps` jumps in whole steps.
    ///
    /// A negative `t` reads as 0 seconds. A spring is treated as over once it is within 0.1% of 1.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::theme::Curve;
    /// let curve = Curve::Steps { duration: 1.0, count: 4 };
    /// assert_eq!(curve.at(0.3), 0.25);
    /// assert_eq!(curve.at(1.0), 1.0);
    /// ```
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
            Curve::Table { duration, count, ref positions, ref values } => {
                if !(t < duration) || !(duration > 0.0) { return 1.0; }
                let p = (t / duration).max(0.0);
                let n = (count as usize).clamp(2, 16);
                for i in 1..n {
                    if p <= positions[i] {
                        let fraction = (p - positions[i - 1]) / (positions[i] - positions[i - 1]);
                        return values[i - 1] + (values[i] - values[i - 1]) * fraction;
                    }
                }
                1.0
            }
            Curve::Steps { duration, count } => {
                if !(t < duration) || !(duration > 0.0) || count == 0 { return 1.0; }
                // Allow a small float-boundary epsilon when sampling scheduled steps.
                (((t as f64 + 1.0e-6) / duration as f64 * count as f64).floor() / count as f64).clamp(0.0, 1.0) as f32
            }
        }
    }
}

/// What the pointer and keyboard are doing to a widget; `States::pick` resolves it to a look.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// The pointer is over the widget.
    pub hovered: bool,
    /// A press is held on the widget.
    pub pressed: bool,
    /// The widget holds keyboard focus.
    pub focused: bool,
    /// The widget does not respond.
    pub disabled: bool,
    /// The widget is on, current or selected.
    pub checked: bool,
}

/// A look per interaction state, plus `checked` for a widget that is on, current or selected.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct States {
    /// The resting look.
    pub idle: Look,
    /// The look while hovered.
    pub hover: Look,
    /// The look while pressed.
    pub pressed: Look,
    /// The look while focused.
    pub focus: Look,
    /// The look while disabled.
    pub disabled: Look,
    /// The look while on, current or selected.
    pub checked: Look,
}

impl States {
    /// A `States` whose six looks are all `look`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::theme::{State, States};
    /// use pxui_types::Look;
    /// let states = States::all(Look::NONE.radius(4.0));
    /// assert_eq!(states.pick(State { hovered: true, ..Default::default() }).radius, [4.0; 4]);
    /// ```
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
    /// The look for state `s`: disabled, else pressed, hovered, checked, focused, idle, whichever holds first.
    ///
    /// So a focused and checked widget shows `checked`, and a hovered and pressed one shows `pressed`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{theme::{State, States}, Look};
    /// let mut states = States::all(Look::NONE);
    /// states.pressed = Look::NONE.radius(2.0);
    /// let both = State { hovered: true, pressed: true, ..Default::default() };
    /// assert_eq!(states.pick(both).radius, [2.0; 4]);
    /// ```
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
