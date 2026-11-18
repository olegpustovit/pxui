//! The style values, callbacks and results of the text inputs (`text_field`, `line_field`, `search_field`, `value_field`, `text_area`, `scrub`, `picker`).
//!
//! A style is a `Copy` value built from `DEFAULT` with `const fn` builders. Callbacks are call-scoped: borrowed `&dyn Fn` values the library
//! calls on the calling thread during the widget's call and never keeps. Byte offsets are `usize`; a value over `i64::MAX` is a programmer
//! error the widget calls refuse with a panic. A result names what happened to the field's row (`response`) and the new text (`changed`),
//! borrowed from the library until the next call on the `Ui`.
use crate::{DropRef, Response};
use pxui_types::{geom::Rgba, ui::{CodeContext, Completion, LineStep, ScrubPhase}, Capitalize, ReturnKey, TextTraits};

/// How `Ui::text_field` builds the one-line text field: the keyboard traits, the body variant and the accessible name, as a `Copy` value built with `const fn` builders from `TextField::DEFAULT` or `TextField::SEARCH`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextField<'a> {
    pub(crate) traits: TextTraits,
    pub(crate) variant: u16,
    pub(crate) name: &'a str,
}

impl<'a> TextField<'a> {
    /// The kit's text field: autocorrect on, sentence capitalization, Return, no variant, unnamed.
    pub const DEFAULT: TextField<'a> = TextField { traits: TextTraits::DEFAULT, variant: 0, name: "" };
    /// A search box: no autocorrect, no capitalization, the Search key, unnamed.
    pub const SEARCH: TextField<'a> = TextField { traits: TextTraits::SEARCH, variant: 0, name: "" };
    /// Sets whether the platform keyboard corrects words while typing (on by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::TextField = pxui::TextField::DEFAULT.autocorrect(false);
    /// let _ = STYLE;
    /// ```
    pub const fn autocorrect(mut self, on: bool) -> TextField<'a> { self.traits = self.traits.autocorrect(on); self }
    /// Sets how the platform keyboard capitalizes (sentences by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::Capitalize;
    /// const STYLE: pxui::TextField = pxui::TextField::DEFAULT.capitalization(Capitalize::Words);
    /// let _ = STYLE;
    /// ```
    pub const fn capitalization(mut self, capitalization: Capitalize) -> TextField<'a> { self.traits = self.traits.capitalization(capitalization); self }
    /// Sets the return key's action (`ReturnKey::Return` by default; `Done` ends editing, `Search` leaves submission to the app).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ReturnKey;
    /// const STYLE: pxui::TextField = pxui::TextField::DEFAULT.action(ReturnKey::Done);
    /// let _ = STYLE;
    /// ```
    pub const fn action(mut self, action: ReturnKey) -> TextField<'a> { self.traits = self.traits.action(action); self }
    /// Sets the variant a registered text field body reads from its config (0 by default): one body builds different structures by it. The built-in body ignores it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::TextField = pxui::TextField::DEFAULT.variant(1);
    /// let _ = STYLE;
    /// ```
    pub const fn variant(mut self, variant: u16) -> TextField<'a> { self.variant = variant; self }
    /// Sets what a screen reader calls the field. A field without a name is unnamed: its key is never used as one.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::TextField = pxui::TextField::DEFAULT.name("Find songs");
    /// let _ = STYLE;
    /// ```
    pub const fn name(mut self, name: &'a str) -> TextField<'a> { self.name = name; self }
    /// The keyboard traits the platform reads.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui::TextField::SEARCH.traits(), pxui_types::TextTraits::SEARCH);
    /// ```
    pub const fn traits(&self) -> TextTraits { self.traits }
    /// The variant a registered body reads; 0 for none.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui::TextField::DEFAULT.variant(2).body_variant(), 2);
    /// ```
    pub const fn body_variant(&self) -> u16 { self.variant }
    /// What a screen reader calls the field; empty for none.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui::TextField::DEFAULT.name("Find").accessible_name(), "Find");
    /// ```
    pub const fn accessible_name(&self) -> &'a str { self.name }
}

/// How `Ui::line_field` builds the one-line field with a hairline, as a `Copy` value built with `const fn` builders from `LineField::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineField<'a> {
    pub(crate) placeholder: &'a str,
    pub(crate) grab: bool,
    pub(crate) name: &'a str,
}

impl<'a> LineField<'a> {
    /// The plain line field: no placeholder, no keyboard grab, unnamed.
    pub const DEFAULT: LineField<'a> = LineField { placeholder: "", grab: false, name: "" };
    /// Sets the text shown while the value is empty.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::LineField = pxui::LineField::DEFAULT.placeholder("find");
    /// let _ = STYLE;
    /// ```
    pub const fn placeholder(mut self, text: &'a str) -> LineField<'a> { self.placeholder = text; self }
    /// Sets whether the field takes the keyboard on the frame it is built (off by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::LineField = pxui::LineField::DEFAULT.grab(true);
    /// let _ = STYLE;
    /// ```
    pub const fn grab(mut self, on: bool) -> LineField<'a> { self.grab = on; self }
    /// Sets what a screen reader calls the field; a field without a name is unnamed.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::LineField = pxui::LineField::DEFAULT.name("Find");
    /// let _ = STYLE;
    /// ```
    pub const fn name(mut self, name: &'a str) -> LineField<'a> { self.name = name; self }
}

/// How `Ui::search_field` builds the search field, as a `Copy` value built with `const fn` builders from `SearchField::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchField<'a> {
    pub(crate) icon: &'a str,
    pub(crate) placeholder: &'a str,
    pub(crate) name: &'a str,
    pub(crate) variant: u16,
    pub(crate) traits: TextTraits,
}

impl<'a> SearchField<'a> {
    /// The plain search field: no icon, no placeholder, unnamed, the theme's `search_field` body, search keyboard traits
    /// (`TextTraits::SEARCH`: no autocorrection or capitalization, the Search return key).
    pub const DEFAULT: SearchField<'a> = SearchField { icon: "", placeholder: "", name: "", variant: 0, traits: TextTraits::SEARCH };
    /// Sets the return key the on-screen keyboard shows (`ReturnKey::Done` ends editing), as `TextField::action` does.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ReturnKey;
    /// const FIND: pxui::SearchField = pxui::SearchField::DEFAULT.action(ReturnKey::Done);
    /// let _ = FIND;
    /// ```
    pub const fn action(mut self, action: ReturnKey) -> SearchField<'a> { self.traits = self.traits.action(action); self }
    /// Builds the field with variant `variant` of the `search_field` body (0: the theme's), so one registered body draws a desktop and
    /// a compact search field.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const COMPACT: pxui::SearchField = pxui::SearchField::DEFAULT.variant(2);
    /// let _ = COMPACT;
    /// ```
    pub const fn variant(mut self, variant: u16) -> SearchField<'a> { self.variant = variant; self }
    /// Sets the glyph shown at the start of the field.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::SearchField = pxui::SearchField::DEFAULT.icon("?");
    /// let _ = STYLE;
    /// ```
    pub const fn icon(mut self, icon: &'a str) -> SearchField<'a> { self.icon = icon; self }
    /// Sets the text shown while the value is empty.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::SearchField = pxui::SearchField::DEFAULT.placeholder("Search");
    /// let _ = STYLE;
    /// ```
    pub const fn placeholder(mut self, text: &'a str) -> SearchField<'a> { self.placeholder = text; self }
    /// Sets what a screen reader calls the field; a field without a name is unnamed.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::SearchField = pxui::SearchField::DEFAULT.name("Find");
    /// let _ = STYLE;
    /// ```
    pub const fn name(mut self, name: &'a str) -> SearchField<'a> { self.name = name; self }
}

/// How `Ui::value_field` builds the compact value field, as a `Copy` value built with `const fn` builders from `ValueField::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueField<'a> {
    pub(crate) size: Option<usize>,
    pub(crate) display: Option<&'a str>,
    pub(crate) fraction: Option<f32>,
    pub(crate) left: bool,
    pub(crate) edit: bool,
    pub(crate) bare: bool,
    pub(crate) kept_drag: bool,
    pub(crate) lead: Option<(&'a str, Rgba)>,
    pub(crate) trail: Option<(&'a str, Rgba)>,
    pub(crate) line: Option<Rgba>,
    pub(crate) placeholder: Option<&'a str>,
    pub(crate) tracking: Option<f32>,
    pub(crate) name: &'a str,
}

impl<'a> ValueField<'a> {
    /// The plain value field: the theme's size, centred, unnamed.
    pub const DEFAULT: ValueField<'a> = ValueField { size: None, display: None, fraction: None, left: false, edit: false, bare: false, kept_drag: false, lead: None, trail: None, line: None, placeholder: None, tracking: None, name: "" };
    /// Sets the value's font size in points (the theme's by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::ValueField = pxui::ValueField::DEFAULT.size(14);
    /// let _ = STYLE;
    /// ```
    pub const fn size(mut self, points: usize) -> ValueField<'a> { self.size = Some(points); self }
    /// Sets the text shown in place of the value while the field is not edited (the value itself by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::ValueField = pxui::ValueField::DEFAULT.display("25%");
    /// let _ = STYLE;
    /// ```
    pub const fn display(mut self, text: &'a str) -> ValueField<'a> { self.display = Some(text); self }
    /// Sets the slider fraction (0 to 1) the field's track shows; without one the field has no track.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::ValueField = pxui::ValueField::DEFAULT.fraction(0.25);
    /// let _ = STYLE;
    /// ```
    pub const fn fraction(mut self, fraction: f32) -> ValueField<'a> { self.fraction = Some(fraction); self }
    /// Sets whether the value aligns to the start of the field instead of centring (off by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::ValueField = pxui::ValueField::DEFAULT.left(true);
    /// let _ = STYLE;
    /// ```
    pub const fn left(mut self, on: bool) -> ValueField<'a> { self.left = on; self }
    /// Sets whether the field opens in text editing (off by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::ValueField = pxui::ValueField::DEFAULT.edit(true);
    /// let _ = STYLE;
    /// ```
    pub const fn edit(mut self, on: bool) -> ValueField<'a> { self.edit = on; self }
    /// Sets whether the field is drawn without its frame (off by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::ValueField = pxui::ValueField::DEFAULT.bare(true);
    /// let _ = STYLE;
    /// ```
    pub const fn bare(mut self, on: bool) -> ValueField<'a> { self.bare = on; self }
    /// Sets whether the owner follows a drag of the slider between builds (`field_drag`, `patch_field`): the drag dirties no view (off by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::ValueField = pxui::ValueField::DEFAULT.kept_drag(true);
    /// let _ = STYLE;
    /// ```
    pub const fn kept_drag(mut self, on: bool) -> ValueField<'a> { self.kept_drag = on; self }
    /// Sets the text shown in `color` before the value.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::ValueField = pxui::ValueField::DEFAULT.lead("x", pxui::ui::Rgba { r: 200, g: 200, b: 200, a: 255 });
    /// let _ = STYLE;
    /// ```
    pub const fn lead(mut self, text: &'a str, color: Rgba) -> ValueField<'a> { self.lead = Some((text, color)); self }
    /// Sets the text shown in `color` after the value.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::ValueField = pxui::ValueField::DEFAULT.trail("px", pxui::ui::Rgba { r: 200, g: 200, b: 200, a: 255 });
    /// let _ = STYLE;
    /// ```
    pub const fn trail(mut self, text: &'a str, color: Rgba) -> ValueField<'a> { self.trail = Some((text, color)); self }
    /// Sets the colour of the soft-range line under the value.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::ValueField = pxui::ValueField::DEFAULT.line(pxui::ui::Rgba { r: 200, g: 200, b: 200, a: 255 });
    /// let _ = STYLE;
    /// ```
    pub const fn line(mut self, color: Rgba) -> ValueField<'a> { self.line = Some(color); self }
    /// Sets the text shown while the value is empty.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::ValueField = pxui::ValueField::DEFAULT.placeholder("0.0");
    /// let _ = STYLE;
    /// ```
    pub const fn placeholder(mut self, text: &'a str) -> ValueField<'a> { self.placeholder = Some(text); self }
    /// Sets the value's letter spacing in points.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::ValueField = pxui::ValueField::DEFAULT.tracking(0.5);
    /// let _ = STYLE;
    /// ```
    pub const fn tracking(mut self, tracking: f32) -> ValueField<'a> { self.tracking = Some(tracking); self }
    /// Sets what a screen reader calls the field; a field without a name is unnamed.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::ValueField = pxui::ValueField::DEFAULT.name("Scale");
    /// let _ = STYLE;
    /// ```
    pub const fn name(mut self, name: &'a str) -> ValueField<'a> { self.name = name; self }
}

/// Accepts the text a value field may commit: `valid(text)`.
pub type ValidFn<'a> = dyn Fn(&str) -> bool + 'a;
/// The text a value field's slider drag asks for: `slide(fraction)` returns the new text.
pub type SlideFn<'a> = dyn Fn(f32) -> String + 'a;
/// The text a value field's value scrub asks for: `scrub(origin, dx, coarse)` returns the new text.
pub type ScrubFn<'a> = dyn Fn(&str, f32, bool) -> String + 'a;

/// The call-scoped callbacks of `Ui::value_field`: borrowed closures the library calls on the calling thread during the call and never keeps; a panic in one surfaces at the call. A closure must not call back into the `Ui`.
#[derive(Clone, Copy)]
pub struct ValueCallbacks<'a> {
    pub(crate) valid: Option<&'a ValidFn<'a>>,
    pub(crate) slide: Option<&'a SlideFn<'a>>,
    pub(crate) scrub: Option<&'a ScrubFn<'a>>,
}

impl<'a> ValueCallbacks<'a> {
    /// No callbacks: every text commits, no slider, no scrub.
    pub const DEFAULT: ValueCallbacks<'a> = ValueCallbacks { valid: None, slide: None, scrub: None };
    /// Sets the check of the text a commit may take (every text commits by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let largest = 10.0_f32; // the closure borrows this local: callbacks are call-scoped
    /// let accept = |t: &str| t.parse::<f32>().is_ok_and(|v| v <= largest);
    /// let _ = pxui::ValueCallbacks::DEFAULT.valid(&accept);
    /// ```
    pub const fn valid(mut self, valid: &'a ValidFn<'a>) -> ValueCallbacks<'a> { self.valid = Some(valid); self }
    /// Makes the field a slider: dragging its track calls `slide(fraction)` for the new text.
    /// Allocates: never; the closure's text allocates on each call.
    /// Panics: never.
    ///
    /// ```
    /// let scale = 100.0_f32;
    /// let slide = |f: f32| format!("{:.2}", f * scale);
    /// let _ = pxui::ValueCallbacks::DEFAULT.slide(&slide);
    /// ```
    pub const fn slide(mut self, slide: &'a SlideFn<'a>) -> ValueCallbacks<'a> { self.slide = Some(slide); self }
    /// Makes the field scrub: dragging its value sideways calls `scrub(origin, dx, coarse)` for the new text.
    /// Allocates: never; the closure's text allocates on each call.
    /// Panics: never.
    ///
    /// ```
    /// let scrub = |origin: &str, dx: f32, _: bool| format!("{origin}{dx}");
    /// let _ = pxui::ValueCallbacks::DEFAULT.scrub(&scrub);
    /// ```
    pub const fn scrub(mut self, scrub: &'a ScrubFn<'a>) -> ValueCallbacks<'a> { self.scrub = Some(scrub); self }
}

/// A request to scroll byte `at` into view; a new `request` repeats the scroll for the same offset (equal requests are one request).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reveal {
    pub(crate) at: usize,
    pub(crate) request: u64,
}

impl Reveal {
    /// The request `request` to reveal byte `at`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui::Reveal::new(10, 1);
    /// ```
    pub const fn new(at: usize, request: u64) -> Reveal { Reveal { at, request } }
}

/// A note on a line of a text area: `text`, over a byte range of the line when it has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AreaNote<'a> {
    pub(crate) line: usize,
    pub(crate) range: Option<(usize, usize)>,
    pub(crate) text: &'a str,
}

impl<'a> AreaNote<'a> {
    /// A note `text` on `line`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui::AreaNote::new(1, "unused");
    /// ```
    pub const fn new(line: usize, text: &'a str) -> AreaNote<'a> { AreaNote { line, range: None, text } }
    /// This note over the bytes `start..end`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui::AreaNote::new(1, "unused").range(0, 3);
    /// ```
    pub const fn range(mut self, start: usize, end: usize) -> AreaNote<'a> { self.range = Some((start, end)); self }
}

/// An edit this frame's text area takes before anything else: the bytes `start..end` become `text`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Splice<'a> {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) text: &'a str,
}

impl<'a> Splice<'a> {
    /// The edit that replaces the bytes `start..end` with `text`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui::Splice::new(0, 5, "ALPHA");
    /// ```
    pub const fn new(start: usize, end: usize, text: &'a str) -> Splice<'a> { Splice { start, end, text } }
}

/// How `Ui::text_area` builds the multiline text area, as a `Copy` value built with `const fn` builders from `TextArea::DEFAULT`. Positions are byte offsets into the text; the slices are borrowed for the call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextArea<'a> {
    pub(crate) wrap: bool,
    pub(crate) reveal: Option<Reveal>,
    pub(crate) select: Option<(usize, usize)>,
    pub(crate) errors: &'a [usize],
    pub(crate) notes: &'a [AreaNote<'a>],
    pub(crate) spans: &'a [(usize, usize)],
    pub(crate) splices: &'a [Splice<'a>],
    pub(crate) chips: &'a [(usize, usize, Rgba)],
    pub(crate) name: &'a str,
}

impl<'a> TextArea<'a> {
    /// The plain text area: no wrap, nothing marked, unnamed.
    pub const DEFAULT: TextArea<'a> = TextArea { wrap: false, reveal: None, select: None, errors: &[], notes: &[], spans: &[], splices: &[], chips: &[], name: "" };
    /// Sets whether long lines wrap at the area's width instead of scrolling sideways (off by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::TextArea = pxui::TextArea::DEFAULT.wrap(true);
    /// let _ = STYLE;
    /// ```
    pub const fn wrap(mut self, on: bool) -> TextArea<'a> { self.wrap = on; self }
    /// Scrolls a byte into view (see `Reveal`).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::Reveal;
    /// const STYLE: pxui::TextArea = pxui::TextArea::DEFAULT.reveal(Reveal::new(10, 1));
    /// let _ = STYLE;
    /// ```
    pub const fn reveal(mut self, reveal: Reveal) -> TextArea<'a> { self.reveal = Some(reveal); self }
    /// Selects the bytes `start..end` and takes the keyboard for the area.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::TextArea = pxui::TextArea::DEFAULT.select(0, 5);
    /// let _ = STYLE;
    /// ```
    pub const fn select(mut self, start: usize, end: usize) -> TextArea<'a> { self.select = Some((start, end)); self }
    /// Replaces the lines marked as errors.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::TextArea = pxui::TextArea::DEFAULT.errors(&[2]);
    /// let _ = STYLE;
    /// ```
    pub const fn errors(mut self, lines: &'a [usize]) -> TextArea<'a> { self.errors = lines; self }
    /// Replaces the notes shown on lines.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::AreaNote;
    /// const NOTES: [AreaNote; 1] = [AreaNote::new(1, "unused")];
    /// const STYLE: pxui::TextArea = pxui::TextArea::DEFAULT.notes(&NOTES);
    /// let _ = STYLE;
    /// ```
    pub const fn notes(mut self, notes: &'a [AreaNote<'a>]) -> TextArea<'a> { self.notes = notes; self }
    /// Replaces the byte ranges highlighted.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::TextArea = pxui::TextArea::DEFAULT.spans(&[(0, 3)]);
    /// let _ = STYLE;
    /// ```
    pub const fn spans(mut self, spans: &'a [(usize, usize)]) -> TextArea<'a> { self.spans = spans; self }
    /// Replaces the edits taken before anything else this frame.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::Splice;
    /// const EDITS: [Splice; 1] = [Splice::new(0, 5, "ALPHA")];
    /// const STYLE: pxui::TextArea = pxui::TextArea::DEFAULT.splices(&EDITS);
    /// let _ = STYLE;
    /// ```
    pub const fn splices(mut self, splices: &'a [Splice<'a>]) -> TextArea<'a> { self.splices = splices; self }
    /// Replaces the coloured chips over byte ranges `(start, end, color)`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const CHIPS: [(usize, usize, pxui::ui::Rgba); 1] = [(0, 3, pxui::ui::Rgba { r: 200, g: 200, b: 200, a: 255 })];
    /// const STYLE: pxui::TextArea = pxui::TextArea::DEFAULT.chips(&CHIPS);
    /// let _ = STYLE;
    /// ```
    pub const fn chips(mut self, chips: &'a [(usize, usize, Rgba)]) -> TextArea<'a> { self.chips = chips; self }
    /// Sets what a screen reader calls the area; an area without a name is unnamed.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::TextArea = pxui::TextArea::DEFAULT.name("Notes");
    /// let _ = STYLE;
    /// ```
    pub const fn name(mut self, name: &'a str) -> TextArea<'a> { self.name = name; self }
}

/// What a language colours: sorted, non-overlapping byte spans and their colour; the rest is the foreground.
pub type ColorizeFn<'a> = dyn Fn(&str) -> Vec<(usize, usize, Rgba)> + 'a;
/// What a language matches: the bracket pairs as (open, close) byte positions; the pair at the caret is lit.
pub type BracketsFn<'a> = dyn Fn(&str) -> Vec<(usize, usize)> + 'a;
/// What a language indents: `indent(text, caret)` returns the indentation Enter puts after the line break at `caret`.
pub type IndentFn<'a> = dyn Fn(&str, usize) -> String + 'a;
/// What a language suggests: `complete(text, caret)` returns the ranked suggestions for the token ending at `caret`, best first.
pub type CompleteFn<'a> = dyn Fn(&str, usize) -> Vec<Completion> + 'a;
/// What a language describes: `describe(text, byte)` returns the token at `byte`, as a byte range, and its description.
pub type DescribeFn<'a> = dyn Fn(&str, usize) -> Option<(usize, usize, String)> + 'a;
/// What a language reads as a number: `number_at(text, byte)` returns the numeric literal at `byte`; dragging it sideways changes the value.
pub type NumberAtFn<'a> = dyn Fn(&str, usize) -> Option<(usize, usize)> + 'a;
/// What a character typed lands in: `context_at(text, caret)`.
pub type ContextAtFn<'a> = dyn Fn(&str, usize) -> CodeContext + 'a;
/// What a language rewrites after every frame that edited the text: `rewrite(text, caret)` returns the text as the language keeps it and where `caret` lands in it.
pub type RewriteFn<'a> = dyn Fn(&str, usize) -> (String, usize) + 'a;

/// What a code editor knows about its text, as call-scoped borrowed closures (see `AreaCallbacks`). A host supplies one; the text area itself is language-free. Every hook answers nothing by default.
#[derive(Clone, Copy)]
pub struct Language<'a> {
    pub(crate) colorize: &'a ColorizeFn<'a>,
    pub(crate) brackets: &'a BracketsFn<'a>,
    pub(crate) indent: &'a IndentFn<'a>,
    pub(crate) pairs: &'a [(char, char)],
    pub(crate) complete: &'a CompleteFn<'a>,
    pub(crate) describe: &'a DescribeFn<'a>,
    pub(crate) number_at: &'a NumberAtFn<'a>,
    pub(crate) context_at: &'a ContextAtFn<'a>,
    pub(crate) comment: &'a str,
    pub(crate) rewrite: Option<&'a RewriteFn<'a>>,
}

impl<'a> Language<'a> {
    /// A language that colours, matches, indents, suggests and describes nothing.
    pub const DEFAULT: Language<'a> = Language { colorize: &|_| Vec::new(), brackets: &|_| Vec::new(), indent: &|_, _| String::new(), pairs: &[], complete: &|_, _| Vec::new(), describe: &|_, _| None, number_at: &|_, _| None, context_at: &|_, _| CodeContext::Code, comment: "", rewrite: None };
    /// Sets what the language colours.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let ink = pxui::ui::Rgba { r: 200, g: 200, b: 200, a: 255 };
    /// let f = |text: &str| if text.is_empty() { Vec::new() } else { vec![(0, text.len(), ink)] };
    /// let _ = pxui::Language::DEFAULT.colorize(&f);
    /// ```
    pub const fn colorize(mut self, f: &'a ColorizeFn<'a>) -> Language<'a> { self.colorize = f; self }
    /// Sets what the language matches.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_: &str| Vec::new();
    /// let _ = pxui::Language::DEFAULT.brackets(&f);
    /// ```
    pub const fn brackets(mut self, f: &'a BracketsFn<'a>) -> Language<'a> { self.brackets = f; self }
    /// Sets what the language indents.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_: &str, _: usize| String::new();
    /// let _ = pxui::Language::DEFAULT.indent(&f);
    /// ```
    pub const fn indent(mut self, f: &'a IndentFn<'a>) -> Language<'a> { self.indent = f; self }
    /// Sets the brackets typed in pairs: the opener wraps the selection or inserts both, the closer typed before itself steps over it, Backspace between an empty pair takes both.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui::Language::DEFAULT.pairs(&[('(', ')')]);
    /// ```
    pub const fn pairs(mut self, pairs: &'a [(char, char)]) -> Language<'a> { self.pairs = pairs; self }
    /// Sets what the language suggests.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_: &str, _: usize| Vec::new();
    /// let _ = pxui::Language::DEFAULT.complete(&f);
    /// ```
    pub const fn complete(mut self, f: &'a CompleteFn<'a>) -> Language<'a> { self.complete = f; self }
    /// Sets what the language describes.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_: &str, _: usize| None;
    /// let _ = pxui::Language::DEFAULT.describe(&f);
    /// ```
    pub const fn describe(mut self, f: &'a DescribeFn<'a>) -> Language<'a> { self.describe = f; self }
    /// Sets what the language reads as a number.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_: &str, _: usize| None;
    /// let _ = pxui::Language::DEFAULT.number_at(&f);
    /// ```
    pub const fn number_at(mut self, f: &'a NumberAtFn<'a>) -> Language<'a> { self.number_at = f; self }
    /// Sets what a character typed lands in.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_: &str, _: usize| pxui::ui::CodeContext::Code;
    /// let _ = pxui::Language::DEFAULT.context_at(&f);
    /// ```
    pub const fn context_at(mut self, f: &'a ContextAtFn<'a>) -> Language<'a> { self.context_at = f; self }
    /// Sets what starts a line comment: Command-/ comments or uncomments the lines the selection touches.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui::Language::DEFAULT.comment("//");
    /// ```
    pub const fn comment(mut self, prefix: &'a str) -> Language<'a> { self.comment = prefix; self }
    /// Sets what the language rewrites after every frame that edited the text.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |text: &str, caret: usize| (text.to_owned(), caret);
    /// let _ = pxui::Language::DEFAULT.rewrite(&f);
    /// ```
    pub const fn rewrite(mut self, f: &'a RewriteFn<'a>) -> Language<'a> { self.rewrite = Some(f); self }
}

/// The call-scoped callbacks of `Ui::text_area`: borrowed closures the library calls on the calling thread during the call and never keeps; a panic in one surfaces at the call. A closure must not call back into the `Ui`.
#[derive(Clone, Copy)]
pub struct AreaCallbacks<'a> {
    pub(crate) on_context: Option<&'a dyn Fn((f32, f32))>,
    pub(crate) on_scrub: Option<&'a dyn Fn(ScrubPhase)>,
    pub(crate) on_scrub_edit: Option<&'a dyn Fn((usize, usize), &str)>,
    pub(crate) on_click: Option<&'a dyn Fn(usize, bool)>,
    pub(crate) on_caret: Option<&'a dyn Fn(usize)>,
    pub(crate) on_caret_move: Option<&'a dyn Fn(usize)>,
    pub(crate) on_select: Option<&'a dyn Fn((usize, usize))>,
    pub(crate) on_drop: Option<&'a dyn Fn(usize, DropRef<'_>)>,
    pub(crate) language: Option<Language<'a>>,
}

impl<'a> AreaCallbacks<'a> {
    /// No callbacks and no language.
    pub const DEFAULT: AreaCallbacks<'a> = AreaCallbacks { on_context: None, on_scrub: None, on_scrub_edit: None, on_click: None, on_caret: None, on_caret_move: None, on_select: None, on_drop: None, language: None };
    /// Calls `f` with the point of a context click.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_at: (f32, f32)| {};
    /// let _ = pxui::AreaCallbacks::DEFAULT.on_context(&f);
    /// ```
    pub const fn on_context(mut self, f: &'a dyn Fn((f32, f32))) -> AreaCallbacks<'a> { self.on_context = Some(f); self }
    /// Calls `f` with the phase of a number scrub.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_phase: pxui::ui::ScrubPhase| {};
    /// let _ = pxui::AreaCallbacks::DEFAULT.on_scrub(&f);
    /// ```
    pub const fn on_scrub(mut self, f: &'a dyn Fn(ScrubPhase)) -> AreaCallbacks<'a> { self.on_scrub = Some(f); self }
    /// Calls `f` with the byte range of a scrubbed number and its new text.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_range: (usize, usize), _text: &str| {};
    /// let _ = pxui::AreaCallbacks::DEFAULT.on_scrub_edit(&f);
    /// ```
    pub const fn on_scrub_edit(mut self, f: &'a dyn Fn((usize, usize), &str)) -> AreaCallbacks<'a> { self.on_scrub_edit = Some(f); self }
    /// Calls `f` with the byte a click landed on and whether it was a modified click.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_at: usize, _modified: bool| {};
    /// let _ = pxui::AreaCallbacks::DEFAULT.on_click(&f);
    /// ```
    pub const fn on_click(mut self, f: &'a dyn Fn(usize, bool)) -> AreaCallbacks<'a> { self.on_click = Some(f); self }
    /// Calls `f` with the caret's byte when it changed.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_at: usize| {};
    /// let _ = pxui::AreaCallbacks::DEFAULT.on_caret(&f);
    /// ```
    pub const fn on_caret(mut self, f: &'a dyn Fn(usize)) -> AreaCallbacks<'a> { self.on_caret = Some(f); self }
    /// Calls `f` with the caret's byte when a key moved it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_at: usize| {};
    /// let _ = pxui::AreaCallbacks::DEFAULT.on_caret_move(&f);
    /// ```
    pub const fn on_caret_move(mut self, f: &'a dyn Fn(usize)) -> AreaCallbacks<'a> { self.on_caret_move = Some(f); self }
    /// Calls `f` with the selection's byte range when it changed.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_range: (usize, usize)| {};
    /// let _ = pxui::AreaCallbacks::DEFAULT.on_select(&f);
    /// ```
    pub const fn on_select(mut self, f: &'a dyn Fn((usize, usize))) -> AreaCallbacks<'a> { self.on_select = Some(f); self }
    /// Calls `f` with the byte a drag hovers or drops at and the payload.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_at: usize, _drop: pxui::DropRef| {};
    /// let _ = pxui::AreaCallbacks::DEFAULT.on_drop(&f);
    /// ```
    pub const fn on_drop(mut self, f: &'a dyn Fn(usize, DropRef<'_>)) -> AreaCallbacks<'a> { self.on_drop = Some(f); self }
    /// Makes the area a code editor for `language`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui::AreaCallbacks::DEFAULT.language(pxui::Language::DEFAULT);
    /// ```
    pub const fn language(mut self, language: Language<'a>) -> AreaCallbacks<'a> { self.language = Some(language); self }
}

/// How `Ui::picker` shows its rows, as a `Copy` value built with `const fn` builders from `Picker::DEFAULT`; the optional `mark` and `off` are call-scoped borrowed closures.
#[derive(Clone, Copy)]
pub struct Picker<'a> {
    pub(crate) limit: usize,
    pub(crate) slash: bool,
    pub(crate) at_rest: bool,
    pub(crate) mark: Option<&'a dyn Fn(usize) -> Option<Rgba>>,
    pub(crate) off: Option<&'a dyn Fn(usize) -> bool>,
}

impl<'a> Picker<'a> {
    /// The plain picker: ten rows, the `/`, no marks.
    pub const DEFAULT: Picker<'a> = Picker { limit: 10, slash: true, at_rest: false, mark: None, off: None };
    /// Sets the most rows windowed around the cursor (10 by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Picker = pxui::Picker::DEFAULT.limit(4);
    /// let _ = STYLE;
    /// ```
    pub const fn limit(mut self, rows: usize) -> Picker<'a> { self.limit = rows; self }
    /// Sets whether the search field shows a `/` before the query (on by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Picker = pxui::Picker::DEFAULT.slash(false);
    /// let _ = STYLE;
    /// ```
    pub const fn slash(mut self, on: bool) -> Picker<'a> { self.slash = on; self }
    /// Sets whether the search field keeps the keyboard but looks at rest (a hairline, no caret) until something is typed in it (off by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Picker = pxui::Picker::DEFAULT.at_rest(true);
    /// let _ = STYLE;
    /// ```
    pub const fn at_rest(mut self, on: bool) -> Picker<'a> { self.at_rest = on; self }
    /// Sets `f(row)`: a colour marking row `row`, or none. Call-scoped: borrowed for the `picker` call, on the calling thread, never kept; it must not call back into the `Ui`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_row: usize| None;
    /// let _ = pxui::Picker::DEFAULT.mark(&f);
    /// ```
    pub const fn mark(mut self, f: &'a dyn Fn(usize) -> Option<Rgba>) -> Picker<'a> { self.mark = Some(f); self }
    /// Sets `f(row)`: whether row `row` shows off (it cannot be chosen). Call-scoped like `mark`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let f = |_row: usize| false;
    /// let _ = pxui::Picker::DEFAULT.off(&f);
    /// ```
    pub const fn off(mut self, f: &'a dyn Fn(usize) -> bool) -> Picker<'a> { self.off = Some(f); self }
}

/// How `Ui::scrub` builds the caller-painted scrubber's behaviour, as a `Copy` value built with `const fn` builders from `Scrubber::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scrubber<'a> {
    pub(crate) step: f32,
    pub(crate) name: &'a str,
}

impl<'a> Scrubber<'a> {
    /// The plain scrubber: a step of 1, unnamed.
    pub const DEFAULT: Scrubber<'a> = Scrubber { step: 1.0, name: "" };
    /// Sets what an arrow key moves the value by (1 by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Scrubber = pxui::Scrubber::DEFAULT.step(5.0);
    /// let _ = STYLE;
    /// ```
    pub const fn step(mut self, step: f32) -> Scrubber<'a> { self.step = step; self }
    /// Sets what a screen reader calls the scrubber; a scrubber without a name is unnamed.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Scrubber = pxui::Scrubber::DEFAULT.name("Playback position");
    /// let _ = STYLE;
    /// ```
    pub const fn name(mut self, name: &'a str) -> Scrubber<'a> { self.name = name; self }
}

/// What `Ui::text_field` reports.
#[derive(Clone, Copy, Debug)]
pub struct TextFieldResponse<'a> {
    /// What happened to the field's row this frame: hover, press, focus; `response.changed` is true exactly when `changed` holds text.
    pub response: Response,
    /// The text the user changed the value to, borrowed from the library and valid until the next call on `ui`; none when it did not change.
    pub changed: Option<&'a str>,
}

/// What `Ui::search_field` reports.
#[derive(Clone, Copy, Debug)]
pub struct SearchFieldResponse<'a> {
    /// What happened to the field's row this frame: hover, press, focus; `response.changed` is true exactly when `changed` holds text.
    pub response: Response,
    /// The text the user changed the value to, borrowed from the library and valid until the next call on `ui`; none when it did not change.
    pub changed: Option<&'a str>,
}

/// What `Ui::line_field` reports; `changed` is the new text when it differs from the value given.
#[derive(Clone, Copy, Debug)]
pub struct LineFieldResponse<'a> {
    /// What happened to the field's row this frame: hover, press, focus; `response.changed` is true exactly when `changed` holds text.
    pub response: Response,
    /// The text the user changed the value to, borrowed from the library and valid until the next call on `ui`; none when it did not change.
    pub changed: Option<&'a str>,
    /// What the field's navigation keys asked.
    pub step: LineStep,
}

/// What `Ui::text_area` reports.
#[derive(Clone, Copy, Debug)]
pub struct TextAreaResponse<'a> {
    /// What happened to the field's row this frame: hover, press, focus; `response.changed` is true exactly when `changed` holds text.
    pub response: Response,
    /// The text the user changed the value to, borrowed from the library and valid until the next call on `ui`; none when it did not change.
    pub changed: Option<&'a str>,
    /// Whether the area was submitted.
    pub submitted: bool,
}

/// What `Ui::value_field` reports.
#[derive(Clone, Copy, Debug)]
pub struct ValueFieldResponse<'a> {
    /// What happened to the field's row this frame: hover, press, focus; `response.changed` is true exactly when `changed` holds text.
    pub response: Response,
    /// The text the user changed the value to, borrowed from the library and valid until the next call on `ui`; none when it did not change.
    pub changed: Option<&'a str>,
    /// Whether a text editor is open on the field.
    pub editing: bool,
}
