//! Stable text inputs and outputs shared with libpxui.
use crate::{theme::ColorToken, Size};

/// The resolved style the shaper sees: every field concrete, tracking in points. Shaping and worker calls take it (`Text`,
/// `UiText`, `PreparedText`, `Paint::text_metrics`/`text_spans`); authoring calls take a `Label`, and `Ui::resolve(&label)` gives
/// the `TextStyle` that label shapes with, so worker-prepared text matches what the `Ui` draws.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TextStyle {
    pub size: u32,
    pub font: u16,
    pub weight: u16,
    pub size_points: f32,
    pub line_height: f32,
    pub wrap: u32,
    pub align: u32,
    pub max_lines: u16,
    pub italic: u8,
    /// ELLIPSIS_NONE, END, MIDDLE or HEAD. max_lines 0 uses one visible line when enabled.
    pub ellipsis: u8,
    /// OpenType feature bits; 0 preserves the font defaults.
    pub features: u32,
    pub tracking: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            size: core::mem::size_of::<Self>() as u32,
            font: 0,
            weight: 400,
            size_points: 14.0,
            line_height: 0.0,
            wrap: 0,
            align: 0,
            max_lines: 0,
            italic: 0,
            ellipsis: 0,
            features: 0,
            tracking: 0.0,
        }
    }
}

impl TextStyle {
    pub const ELLIPSIS_NONE: u8 = 0;
    pub const ELLIPSIS_END: u8 = 1;
    pub const ELLIPSIS_MIDDLE: u8 = 2;
    pub const ELLIPSIS_HEAD: u8 = 3;
    pub const FEATURE_TABULAR_NUMBERS: u32 = 1;
    pub const WRAP_NONE: u32 = 0;
    pub const WRAP_WORD: u32 = 1;
    pub const WRAP_GLYPH: u32 = 2;
    /// Word wrapping whose fitting width includes separators omitted at visible soft wraps (ABI 1.18).
    pub const WRAP_WORD_SPACES: u32 = 3;
    pub const ALIGN_LEFT: u32 = 0;
    pub const ALIGN_CENTER: u32 = 1;
    pub const ALIGN_RIGHT: u32 = 2;
}

/// The authoring text style (`Ui::label`, `label_fmt`, `label_digits`, `Ui::text_line_box`/`text_spans`, `Paint::text_styled`/
/// `text_chars`): a theme-relative text style, colour and box. `Ui::resolve` turns it into the `TextStyle` the shaper sees. App styles
/// are consts built from a preset with the const builders: `const TITLE: Label = Label::BODY.size(15.0).bold();`.
///
/// One meaning throughout: a `None` field is the theme's label style, resolved from the theme's label look (`Theme::label`) and its
/// rows when the label body builds (or the painter paints): its font, size, weight, tracking, capitals and colour, a full-width
/// box one row high per line, 12 points in from the start with 4 above when it is its parent's first child. `Label::DEFAULT` sets
/// nothing, so it is the theme's label; a builder sets only its own field. `BODY`, `CAPTION`, `TITLE` and `MONO` set every field
/// but the colour (theirs is the theme label colour unless `color` sets one).
#[derive(Clone, Copy, Debug)]
pub struct Label {
    /// The font family (the `Ui`'s font order); `None`: the theme label's.
    pub font: Option<u16>,
    /// The size in points; `None`: the theme label's.
    pub size: Option<f32>,
    /// The weight (400 regular, 700 bold); `None`: the theme label's.
    pub weight: Option<u16>,
    /// Extra space after every glyph, in ems; `None`: the theme label's.
    pub tracking: Option<f32>,
    /// Whether the text shows in capitals; `None`: the theme label's.
    pub caps: Option<bool>,
    /// The text colour, an app colour token; `None`: the theme label's.
    pub color: Option<ColorToken>,
    /// The node's width; `None`: the full width of the parent.
    pub width: Option<Size>,
    /// The height of one line in points, 0 for 1.2 times the size; `None`: the theme's row height.
    pub line_height: Option<f32>,
    /// The inset before the text in points; `None`: the theme's 12, with 4 points above a parent's first child.
    pub pad: Option<f32>,
    /// `TextStyle::WRAP_*`: how lines break inside the width.
    pub wrap: u32,
    /// The most lines shown, 0 for all.
    pub max_lines: u16,
    /// `TextStyle::ELLIPSIS_*`: how cut text ends.
    pub ellipsis: u8,
    /// `TextStyle::ALIGN_*`: where lines sit inside the width.
    pub align: u32,
    /// OpenType feature bits (`TextStyle::FEATURE_TABULAR_NUMBERS`).
    pub features: u32,
    /// What a registered label body reads from its config (`PxuiLabelConfig::variant`); 0 for none.
    pub variant: u16,
}

impl Label {
    /// The theme's label: every field unset.
    pub const DEFAULT: Label = Label {
        font: None, size: None, weight: None, tracking: None, caps: None, color: None, width: None, line_height: None, pad: None,
        wrap: TextStyle::WRAP_NONE, max_lines: 0, ellipsis: TextStyle::ELLIPSIS_NONE, align: TextStyle::ALIGN_LEFT, features: 0, variant: 0,
    };
    /// Running text: the UI face at 14 points, regular, as wide as the text, lines 1.2 times the size, no inset.
    pub const BODY: Label = Label {
        font: Some(0), size: Some(14.0), weight: Some(400), tracking: Some(0.0), caps: Some(false), width: Some(Size::Fit), line_height: Some(0.0),
        pad: Some(0.0), ..Label::DEFAULT
    };
    /// Secondary text: `BODY` at 11 points.
    pub const CAPTION: Label = Label::BODY.size(11.0);
    /// Headings: `BODY` at 20 points, bold.
    pub const TITLE: Label = Label::BODY.size(20.0).bold();
    /// Figures and counters: `BODY` with tabular numbers, so digits keep one width.
    pub const MONO: Label = Label { features: TextStyle::FEATURE_TABULAR_NUMBERS, ..Label::BODY };

    /// This style in font `family` (the `Ui`'s font order; 0 is the UI face).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Label, Size, TextStyle, theme::ColorToken};
    /// const CLOCK: Label = Label::BODY.font(1).size(12.0);
    /// assert_eq!(CLOCK.font, Some(1));
    /// ```
    #[inline]
    pub const fn font(mut self, family: u16) -> Label { self.font = Some(family); self }
    /// This style at `points` points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Label, Size, TextStyle, theme::ColorToken};
    /// const L: Label = Label::BODY.size(15.0);
    /// assert_eq!(L.size, Some(15.0));
    /// ```
    #[inline]
    pub const fn size(mut self, points: f32) -> Label { self.size = Some(points); self }
    /// This style in bold (weight 700).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Label, Size, TextStyle, theme::ColorToken};
    /// assert_eq!(Label::DEFAULT.bold().weight, Some(700));
    /// assert_eq!(Label::DEFAULT.bold().size, None, "the theme still sizes it");
    /// ```
    #[inline]
    pub const fn bold(mut self) -> Label { self.weight = Some(700); self }
    /// This style at weight `weight` (400 regular, 500 medium, 700 bold).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Label;
    /// assert_eq!(Label::BODY.weight(500).weight, Some(500));
    /// ```
    #[inline]
    pub const fn weight(mut self, weight: u16) -> Label { self.weight = Some(weight); self }
    /// This style with `ems` of extra space after every glyph (a fraction of the size; negative tightens). The resolved style
    /// (`Ui::resolve`) carries it in points, `ems * size`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Label;
    /// assert_eq!(Label::BODY.tracking(-0.02).tracking, Some(-0.02));
    /// ```
    #[inline]
    pub const fn tracking(mut self, ems: f32) -> Label { self.tracking = Some(ems); self }
    /// This style with lines `points` high (0: 1.2 times the size).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Label;
    /// assert_eq!(Label::BODY.line_height(18.2).line_height, Some(18.2));
    /// ```
    #[inline]
    pub const fn line_height(mut self, points: f32) -> Label { self.line_height = Some(points); self }
    /// This style wrapping at words, the fitting width keeping the separators a visible soft wrap omits (`WRAP_WORD_SPACES`).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Label, TextStyle};
    /// assert_eq!(Label::BODY.wrap_spaces().wrap, TextStyle::WRAP_WORD_SPACES);
    /// ```
    #[inline]
    pub const fn wrap_spaces(mut self) -> Label { self.wrap = TextStyle::WRAP_WORD_SPACES; self }
    /// This style in the app's colour `token` (`Ui::set_colors`), resolved when the label is built or painted.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Label, Size, TextStyle, theme::ColorToken};
    /// const DIM: Label = Label::BODY.color(ColorToken(3));
    /// assert_eq!(DIM.color, Some(ColorToken(3)));
    /// ```
    #[inline]
    pub const fn color(mut self, token: ColorToken) -> Label { self.color = Some(token); self }
    /// This style filling its parent's width (and wrapping there when it wraps).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Label, Size, TextStyle, theme::ColorToken};
    /// assert!(Label::TITLE.grow().width.unwrap().is_grow());
    /// ```
    #[inline]
    pub const fn grow(self) -> Label { self.w(Size::Grow) }
    /// This style `width` wide: `Size::Grow`, `Size::Fit` (the text's own width) or `Size::Px`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Label, Size, TextStyle, theme::ColorToken};
    /// assert!(matches!(Label::BODY.w(Size::Px(50.0)).width, Some(Size::Px(w)) if w == 50.0));
    /// ```
    #[inline]
    pub const fn w(mut self, width: Size) -> Label { self.width = Some(width); self }
    /// This style wrapping at word boundaries inside its width; the label is as tall as the lines it shows.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Label, Size, TextStyle, theme::ColorToken};
    /// assert_eq!(Label::BODY.wrap().wrap, TextStyle::WRAP_WORD);
    /// ```
    #[inline]
    pub const fn wrap(mut self) -> Label { self.wrap = TextStyle::WRAP_WORD; self }
    /// This style showing at most `lines` lines (0: every line).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Label, Size, TextStyle, theme::ColorToken};
    /// assert_eq!(Label::BODY.wrap().lines(2).max_lines, 2);
    /// ```
    #[inline]
    pub const fn lines(mut self, lines: u16) -> Label { self.max_lines = lines; self }
    /// This style ending text cut by `lines`, or a single line too long for its width, in an ellipsis.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Label, Size, TextStyle, theme::ColorToken};
    /// assert_eq!(Label::BODY.ellipsis().ellipsis, TextStyle::ELLIPSIS_END);
    /// ```
    #[inline]
    pub const fn ellipsis(mut self) -> Label { self.ellipsis = TextStyle::ELLIPSIS_END; self }
    /// This style centring its lines inside its width.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Label, Size, TextStyle, theme::ColorToken};
    /// assert_eq!(Label::BODY.center().align, TextStyle::ALIGN_CENTER);
    /// ```
    #[inline]
    pub const fn center(mut self) -> Label { self.align = TextStyle::ALIGN_CENTER; self }
    /// This style placing its lines at the end of its width.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Label, Size, TextStyle, theme::ColorToken};
    /// assert_eq!(Label::BODY.end().align, TextStyle::ALIGN_RIGHT);
    /// ```
    #[inline]
    pub const fn end(mut self) -> Label { self.align = TextStyle::ALIGN_RIGHT; self }
    /// This style with the variant a registered label body reads from its config (`variant`, 0 by default): one body builds different
    /// structures by it, so the app picks per call without comparing text. The built-in body ignores it.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Label;
    /// assert_eq!(Label::BODY.variant(2).variant, 2);
    /// ```
    #[inline]
    pub const fn variant(mut self, variant: u16) -> Label { self.variant = variant; self }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
    pub lines: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextGlyph {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    /// 0/1: first mask/colour pages; 2/3: second mask/colour pages (ABI 1.7).
    pub atlas: u32,
    pub reserved: u32,
}


/// A nonempty UTF-8 byte range. Ranges are ordered and do not overlap; weight 0 inherits the base style.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextSpan {
    pub start: u32,
    pub end: u32,
    pub weight: u16,
    pub flags: u16,
    pub color: crate::geom::Rgba,
    pub background: crate::geom::Rgba,
    /// A 1.5-point rule at the bottom of the line box, independent of glyph colour.
    pub underline: crate::geom::Rgba,
}
impl TextSpan {
    pub const COLOR: u16=1;
    pub const BACKGROUND: u16=2;
    pub const UNDERLINE: u16=4;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSpans {
    pub size: u32,
    pub count: u32,
    pub spans: [TextSpan;8],
}
impl Default for TextSpans {
    fn default() -> Self { Self { size:core::mem::size_of::<Self>() as u32,count:0,spans:[TextSpan::default();8] } }
}
impl TextSpans {
    pub const MAX: usize=8;
    pub fn new(spans: &[TextSpan]) -> Option<Self> {
        if spans.len()>Self::MAX { return None; }
        let mut value=Self::default();
        value.count=spans.len() as u32;
        value.spans[..spans.len()].copy_from_slice(spans);
        Some(value)
    }
    pub fn as_slice(&self) -> Option<&[TextSpan]> { self.spans.get(..self.count as usize) }
}
