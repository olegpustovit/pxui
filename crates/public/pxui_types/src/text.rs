//! Stable text inputs and outputs shared with libpxui.
use crate::{a11y::Role, theme::ColorToken, Size};

/// How a style's lines break inside its width (`ffi::PxuiTextStyle::wrap` carries its number).
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrap {
    /// One line; text past the width is cut (or ends in an ellipsis).
    None = 0,
    /// Lines break between words.
    Word = 1,
    /// Lines break between any two characters.
    Glyph = 2,
    /// Lines break between words, the fitting width keeping the separators a visible soft wrap omits (ABI 1.18).
    WordSpaces = 3,
}

/// How cut text ends (`ffi::PxuiTextStyle::ellipsis` carries its number); a single line when no line limit is set.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ellipsis {
    /// Cut text ends at the cut.
    None = 0,
    /// An ellipsis replaces the end.
    End = 1,
    /// An ellipsis replaces the middle.
    Middle = 2,
    /// An ellipsis replaces the start.
    Head = 3,
}

/// Where a style's lines sit inside its width (`ffi::PxuiTextStyle::align` carries its number).
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlign {
    /// At the start (the left in left-to-right text).
    Start = 0,
    /// Centred.
    Center = 1,
    /// At the end.
    End = 2,
}

/// The authoring text style (`Ui::label`, `label_fmt`, `label_digits`, `Ui::text_line_box`/`text_spans`, `Paint::text`/
/// `text_chars`): a theme-relative text style, colour and box. `Ui::resolve` turns it into the `TextStyle` the shaper sees. App styles
/// are consts built from a preset with the const builders: `const TITLE: Label = Label::BODY.size(15.0).bold();`.
///
/// One meaning throughout: a `None` field is the theme's label style, resolved from the theme's label look (`Theme::label`) and its
/// rows when the label body builds (or the painter paints): its font, size, weight, tracking, capitals and colour, a full-width
/// box one row high per line, 12 points in from the start with 4 above when it is its parent's first child. `Label::DEFAULT` sets
/// nothing, so it is the theme's label; a builder sets only its own field. `BODY`, `CAPTION`, `TITLE` and `MONO` set every field
/// but the colour (theirs is the theme label colour unless `color` sets one) and the box height (theirs is as tall as the lines).
/// Every label is `Role::Label` to accessibility unless `role` says otherwise; a fitted width is fractional unless `whole_pixels`.
///
/// The fields are private: a label is built only from a preset and the builders.
#[derive(Clone, Copy, Debug)]
pub struct Label(LabelFields);

/// The fields of a `Label`, one meaning each as `Label` documents it: what the library reads (`Label::fields`) and decodes a C
/// label record into (starting from `LabelFields::UNSET`). Not for authoring: nothing turns it into a `Label`, which is built only
/// from a preset and the builders, and it cannot be written as a literal outside this crate.
#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct LabelFields {
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
    /// How lines break inside the width.
    pub wrap: Wrap,
    /// The most lines shown, 0 for all.
    pub max_lines: u16,
    /// How cut text ends.
    pub ellipsis: Ellipsis,
    /// Where lines sit inside the width.
    pub align: TextAlign,
    /// OpenType feature bits (`ffi::PxuiTextStyle::FEATURE_TABULAR_NUMBERS`).
    pub features: u32,
    /// What a registered label body reads from its config (`PxuiLabelConfig::variant`); 0 for none.
    pub variant: u16,
    /// The node's height in points, the lines centred in it; `None`: as tall as the lines it shows (the theme's row for its label).
    pub height: Option<f32>,
    /// Whether a `Fit` width is rounded up to whole pixels of the display (SwiftUI's text frames).
    pub whole_pixels: bool,
    /// The accessible role: `Role::Label`, `Role::Paragraph` for running text, `Role::Unknown` for text assistive technology skips.
    pub role: Role,
}

impl LabelFields {
    /// Every field unset: `Label::DEFAULT`'s fields, where a decoder starts.
    pub const UNSET: LabelFields = Label::DEFAULT.0;
}

impl Label {
    /// The theme's label: every field unset.
    pub const DEFAULT: Label = Label(LabelFields {
        font: None, size: None, weight: None, tracking: None, caps: None, color: None, width: None, line_height: None, pad: None,
        wrap: Wrap::None, max_lines: 0, ellipsis: Ellipsis::None, align: TextAlign::Start, features: 0, variant: 0,
        height: None, whole_pixels: false, role: Role::Label,
    });
    /// Running text: the UI face at 14 points, regular, as wide as the text, lines 1.2 times the size, no inset.
    pub const BODY: Label = Label(LabelFields {
        font: Some(0), size: Some(14.0), weight: Some(400), tracking: Some(0.0), caps: Some(false), width: Some(Size::Fit), line_height: Some(0.0),
        pad: Some(0.0), ..Label::DEFAULT.0
    });
    /// Secondary text: `BODY` at 11 points.
    pub const CAPTION: Label = Label::BODY.size(11.0);
    /// Headings: `BODY` at 20 points, bold.
    pub const TITLE: Label = Label::BODY.size(20.0).bold();
    /// Figures and counters: `BODY` with tabular numbers, so digits keep one width.
    pub const MONO: Label = Label::BODY.tabular();

    /// This style in font `family`: an index into the `Ui`'s font order (0 is the UI face, then the fonts in the order `Ui::new`
    /// and `UiText::add_font` registered them), so one value names the same face in the library and on the worker shapers forked from it.
    ///
    /// Allocates: never.
    /// Panics: never (a font the `Ui` has not loaded is refused when the label is shown).
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, Label};
    /// const CLOCK: Label = Label::BODY.font(1).size(12.0);
    /// assert_eq!(PxuiLabelStyle::from(&CLOCK).font, 1);
    /// ```
    #[inline]
    pub const fn font(mut self, family: u16) -> Label { self.0.font = Some(family); self }
    /// This style at `points` points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, Label};
    /// const L: Label = Label::BODY.size(15.0);
    /// assert_eq!(PxuiLabelStyle::from(&L).size_points, 15.0);
    /// ```
    #[inline]
    pub const fn size(mut self, points: f32) -> Label { self.0.size = Some(points); self }
    /// This style in bold (weight 700).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, Label};
    /// let bold = PxuiLabelStyle::from(&Label::DEFAULT.bold());
    /// assert_eq!(bold.weight, 700);
    /// assert_eq!(bold.set & PxuiLabelStyle::SET_SIZE, 0, "the theme still sizes it");
    /// ```
    #[inline]
    pub const fn bold(mut self) -> Label { self.0.weight = Some(700); self }
    /// This style at weight `weight` (400 regular, 500 medium, 700 bold).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::BODY.weight(500)).weight, 500);
    /// ```
    #[inline]
    pub const fn weight(mut self, weight: u16) -> Label { self.0.weight = Some(weight); self }
    /// This style with `ems` of extra space after every glyph (a fraction of the size; negative tightens). The resolved style
    /// (`Ui::resolve`) carries it in points, `ems * size`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::BODY.tracking(-0.02)).tracking, -0.02);
    /// ```
    #[inline]
    pub const fn tracking(mut self, ems: f32) -> Label { self.0.tracking = Some(ems); self }
    /// This style in capitals (`true`) or as written (`false`); the theme's label shows capitals.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, Label};
    /// const SECTION: Label = Label::BODY.size(11.0).caps(true);
    /// assert_eq!(PxuiLabelStyle::from(&SECTION).caps, 1);
    /// assert_eq!(PxuiLabelStyle::from(&Label::DEFAULT.caps(false)).set, PxuiLabelStyle::SET_CAPS);
    /// ```
    #[inline]
    pub const fn caps(mut self, on: bool) -> Label { self.0.caps = Some(on); self }
    /// This style with lines `points` high (0: 1.2 times the size).
    ///
    /// It also grows the node's height, which is its lines' unless `h_px` sets a box taller than them.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::BODY.line_height(18.2)).line_height, 18.2);
    /// ```
    #[inline]
    pub const fn line_height(mut self, points: f32) -> Label { self.0.line_height = Some(points); self }
    /// This style inset `points` before its text (`None`, the theme's: 12, with 4 points above a parent's first child).
    ///
    /// Allocates: never.
    /// Panics: never (a padding that is not finite is refused when the label is shown).
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::DEFAULT.pad(0.0)).set, PxuiLabelStyle::SET_PAD);
    /// assert_eq!(PxuiLabelStyle::from(&Label::DEFAULT.pad(6.0)).pad, 6.0);
    /// ```
    #[inline]
    pub const fn pad(mut self, points: f32) -> Label { self.0.pad = Some(points); self }
    /// Uses the active theme's colour `token` when building or painting the label.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, theme::ColorToken, Label};
    /// const DIM: Label = Label::BODY.color(ColorToken(3));
    /// assert_eq!(PxuiLabelStyle::from(&DIM).color, 3);
    /// ```
    #[inline]
    pub const fn color(mut self, token: ColorToken) -> Label { self.0.color = Some(token); self }
    /// This style filling its parent's width (and wrapping there when it wraps): `w(Size::Grow)`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::{PxuiLabelStyle, PxuiSizeDesc}, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::TITLE.grow()).width.kind, PxuiSizeDesc::GROW);
    /// ```
    #[inline]
    pub const fn grow(self) -> Label { self.w(Size::Grow) }
    /// This style `width` wide: `Size::Grow`, `Size::Fit` (the text's own width), `Size::Px`, `Size::Pct` or `Size::Rel`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::{PxuiLabelStyle, PxuiSizeDesc}, Label, Size};
    /// assert_eq!(PxuiLabelStyle::from(&Label::BODY.w(Size::Pct(50.0))).width.kind, PxuiSizeDesc::PCT);
    /// ```
    #[inline]
    pub const fn w(mut self, width: Size) -> Label { self.0.width = Some(width); self }
    /// This style `points` wide: `w(Size::Px(points))`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::BODY.w_px(50.0)).width.value, 50.0);
    /// ```
    #[inline]
    pub const fn w_px(self, points: f32) -> Label { self.w(Size::Px(points)) }
    /// This style wrapping at word boundaries inside its width (`Wrap::Word`); the label is as tall as the lines it shows.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, text::Wrap, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::BODY.wrap()).wrap, Wrap::Word as u32);
    /// ```
    #[inline]
    pub const fn wrap(mut self) -> Label { self.0.wrap = Wrap::Word; self }
    /// This style wrapping at words, the fitting width keeping the separators a visible soft wrap omits (`Wrap::WordSpaces`).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, text::Wrap, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::BODY.wrap_spaces()).wrap, Wrap::WordSpaces as u32);
    /// ```
    #[inline]
    pub const fn wrap_spaces(mut self) -> Label { self.0.wrap = Wrap::WordSpaces; self }
    /// This style wrapping between any two characters (`Wrap::Glyph`): text without spaces, such as a path or a hash.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, text::Wrap, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::MONO.wrap_glyph()).wrap, Wrap::Glyph as u32);
    /// ```
    #[inline]
    pub const fn wrap_glyph(mut self) -> Label { self.0.wrap = Wrap::Glyph; self }
    /// This style showing at most `lines` lines (0: every line).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::BODY.wrap().lines(2)).max_lines, 2);
    /// ```
    #[inline]
    pub const fn lines(mut self, lines: u16) -> Label { self.0.max_lines = lines; self }
    /// This style ending text cut by `lines`, or a single line too long for its width, in an ellipsis (`Ellipsis::End`).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, text::Ellipsis, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::BODY.ellipsis()).ellipsis, Ellipsis::End as u8);
    /// ```
    #[inline]
    pub const fn ellipsis(mut self) -> Label { self.0.ellipsis = Ellipsis::End; self }
    /// This style cutting text in its middle with an ellipsis (`Ellipsis::Middle`), keeping both ends (a file name and its extension).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, text::Ellipsis, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::BODY.ellipsis_middle()).ellipsis, Ellipsis::Middle as u8);
    /// ```
    #[inline]
    pub const fn ellipsis_middle(mut self) -> Label { self.0.ellipsis = Ellipsis::Middle; self }
    /// This style cutting text at its start with an ellipsis (`Ellipsis::Head`), keeping the end (the last folders of a path).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, text::Ellipsis, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::BODY.ellipsis_head()).ellipsis, Ellipsis::Head as u8);
    /// ```
    #[inline]
    pub const fn ellipsis_head(mut self) -> Label { self.0.ellipsis = Ellipsis::Head; self }
    /// This style placing its lines at `align` inside its width: `TextAlign::Start` (the default), `Center` or `End`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelStyle, text::TextAlign, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::BODY.align(TextAlign::Center)).align, TextAlign::Center as u32);
    /// ```
    #[inline]
    pub const fn align(mut self, align: TextAlign) -> Label { self.0.align = align; self }
    /// This style with tabular numbers: every digit one width, so figures in a column or a counter keep their place (`MONO`).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::{PxuiLabelStyle, PxuiTextStyle}, Label};
    /// assert_eq!(PxuiLabelStyle::from(&Label::BODY.size(13.0).tabular()).features, PxuiTextStyle::FEATURE_TABULAR_NUMBERS);
    /// ```
    #[inline]
    pub const fn tabular(mut self) -> Label { self.0.features |= crate::ffi::PxuiTextStyle::FEATURE_TABULAR_NUMBERS; self }
    /// This style with the variant a registered label body reads from its config (`variant`, 0 by default): one body builds different
    /// structures by it, so the app picks per call without comparing text. The built-in body ignores it.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Label;
    /// assert_eq!(Label::BODY.variant(2).body_variant(), 2);
    /// ```
    #[inline]
    pub const fn variant(mut self, variant: u16) -> Label { self.0.variant = variant; self }
    /// The variant `variant` set (0 for none): what a registered label body reads from its config.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Label;
    /// assert_eq!(Label::BODY.body_variant(), 0);
    /// ```
    #[inline]
    pub const fn body_variant(&self) -> u16 { self.0.variant }

    /// This style in a node `points` high with its lines centred in it (a 12-point heading in a 30-point row), whatever its line height.
    ///
    /// The line height still spaces the lines; the box only places them. The painter (`Paint::text`) has no box and ignores it.
    /// Allocates: never.
    /// Panics: never (a height that is not finite and at least zero is refused when the label is shown).
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelBox, Label};
    /// const HEADING: Label = Label::BODY.size(12.0).line_height(16.8).h_px(30.0);
    /// assert_eq!(PxuiLabelBox::from(&HEADING).height, 30.0);
    /// ```
    #[inline]
    pub const fn h_px(mut self, points: f32) -> Label { self.0.height = Some(points); self }

    /// This style with a `Fit` width rounded up to whole pixels of the display, as SwiftUI frames text; other widths are unchanged.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ffi::PxuiLabelBox, Label};
    /// assert_eq!(PxuiLabelBox::from(&Label::BODY.whole_pixels()).whole_pixels, 1);
    /// ```
    #[inline]
    pub const fn whole_pixels(mut self) -> Label { self.0.whole_pixels = true; self }

    /// This style announced as `role`: `Role::Paragraph` for running text, `Role::Unknown` for a caption assistive technology skips.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{a11y::Role, ffi::PxuiLabelBox, Label};
    /// assert_eq!(PxuiLabelBox::from(&Label::BODY.role(Role::Paragraph)).role, Role::Paragraph as u8);
    /// assert_eq!(PxuiLabelBox::from(&Label::BODY).set, 0, "Role::Label by default");
    /// ```
    #[inline]
    pub const fn role(mut self, role: Role) -> Label { self.0.role = role; self }
    /// The label's fields as the library reads them (the label body, `Ui::resolve`); one copy, borrowed, no conversion.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Label;
    /// assert_eq!(Label::BODY.size(15.0).fields().size, Some(15.0));
    /// ```
    #[doc(hidden)]
    #[inline]
    pub const fn fields(&self) -> &LabelFields { &self.0 }
}

/// Size-prefixed (`size_of::<TextMetrics>()`, 20): the library writes it (`Default` sets it); a caller's out record whose `size` is short is refused.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMetrics {
    pub size: u32,
    pub width: f32,
    pub height: f32,
    /// Distance, in points, from the top of the layout to its first line's baseline: `Paint::text`, `text_spans` and `text_chars` put
    /// the first baseline this far below `at`.
    pub baseline: f32,
    pub lines: u32,
}
impl Default for TextMetrics {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, width: 0.0, height: 0.0, baseline: 0.0, lines: 0 } }
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
    /// Creates an unstyled span over a UTF-8 byte range.
    ///
    /// The text setter validates ordered ranges and character boundaries.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui_types::TextSpan::new(0, 4);
    /// ```
    pub const fn new(start: u32, end: u32) -> TextSpan {
        TextSpan { start, end, weight: 0, flags: 0, color: crate::geom::Rgba::CLEAR,
            background: crate::geom::Rgba::CLEAR, underline: crate::geom::Rgba::CLEAR }
    }

    /// Sets the font weight for this span.
    ///
    /// Zero uses the surrounding text's weight.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::TextSpan::new(0, 4).weight(700).weight, 700);
    /// ```
    pub const fn weight(self, weight: u16) -> TextSpan { TextSpan { weight, ..self } }

    /// Sets the span's color colour.
    ///
    /// The change applies to this span's byte range.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui_types::TextSpan::new(0, 4).color(pxui_types::geom::Rgba::BLACK);
    /// ```
    pub const fn color(self, color: crate::geom::Rgba) -> TextSpan { TextSpan { color, flags: self.flags | TextSpan::COLOR, ..self } }

    /// Sets the span's background colour.
    ///
    /// The change applies to this span's byte range.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui_types::TextSpan::new(0, 4).background(pxui_types::geom::Rgba::BLACK);
    /// ```
    pub const fn background(self, background: crate::geom::Rgba) -> TextSpan { TextSpan { background, flags: self.flags | TextSpan::BACKGROUND, ..self } }

    /// Sets the span's underline colour.
    ///
    /// The change applies to this span's byte range.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui_types::TextSpan::new(0, 4).underline(pxui_types::geom::Rgba::BLACK);
    /// ```
    pub const fn underline(self, underline: crate::geom::Rgba) -> TextSpan { TextSpan { underline, flags: self.flags | TextSpan::UNDERLINE, ..self } }

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
    /// Packs `spans` into the fixed record the library takes; `None` for more than eight (`TextSpans::MAX`).
    /// This checks the count only; the library checks that ranges are ordered, nonempty, disjoint and on character boundaries.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{TextSpan, TextSpans};
    /// let bold = TextSpan { start: 0, end: 4, weight: 700, ..Default::default() };
    /// assert!(TextSpans::new(&[bold]).is_some());
    /// assert!(TextSpans::new(&[bold; 9]).is_none());
    /// ```
    pub fn new(spans: &[TextSpan]) -> Option<Self> {
        if spans.len()>Self::MAX { return None; }
        let mut value=Self::default();
        value.count=spans.len() as u32;
        value.spans[..spans.len()].copy_from_slice(spans);
        Some(value)
    }
    /// The spans the record holds, in order; `None` when its count is over eight (a record built by hand, not by `new`).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{TextSpan, TextSpans};
    /// let bold = TextSpan { start: 0, end: 4, weight: 700, ..Default::default() };
    /// let spans = TextSpans::new(&[bold, TextSpan { start: 5, end: 8, ..bold }]).unwrap();
    /// assert_eq!(spans.as_slice().map(|s| s.len()), Some(2));
    /// ```
    pub fn as_slice(&self) -> Option<&[TextSpan]> { self.spans.get(..self.count as usize) }
}
