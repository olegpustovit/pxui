//! Plain UI data and pure functions shared by the engine and the wrapper: box flags, keys, small results and text helpers. One
//! definition each; `pxui_core` and `pxui` re-export them. The option records that hold closures (`Rc<dyn Fn>`) are not here: they
//! are not ABI types, and the engine and the wrapper each keep theirs (`opts` in both).
use crate::geom::{hash, Rect, Rgba};
use crate::theme::{Look, States};
use std::borrow::Cow;

/// Virtual rows in one scroll region, with a scrolling lead-in and a sticky column heading, as a `Copy` value built with `const fn`
/// builders from `SectionList::new(count)` (`Ui::section_list`). The fields are what the engine reads; write them through the builders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionList {
    /// Number of rows.
    pub count: usize,
    /// Height of each row in points (its estimate until built when `measured`).
    pub row_height: f32,
    /// Height in points of the lead-in that scrolls above the header.
    pub leading_height: f32,
    /// Height in points of the sticky column heading.
    pub header_height: f32,
    /// Height in points of the space after the last row.
    pub trailing_height: f32,
    /// The scrollbar floats over the rows instead of taking width from them.
    pub overlay_scrollbar: bool,
    /// Rows take the heights they lay out to (`row_height` is their estimate until built).
    pub measured: bool,
}

impl SectionList {
    /// `count` rows of the kit's 24 points, no lead-in, heading or trailer, a scrollbar beside the rows, fixed heights.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SONGS: pxui_types::ui::SectionList = pxui_types::ui::SectionList::new(0).row_h(44.0);
    /// assert_eq!(SONGS.row_height, 44.0);
    /// ```
    pub const fn new(count: usize) -> SectionList {
        SectionList { count, row_height: 24.0, leading_height: 0.0, header_height: 0.0, trailing_height: 0.0, overlay_scrollbar: false, measured: false }
    }

    /// Sets each row's height in points (positive and finite; the estimate until built when `measured`).
    /// Allocates: never.
    /// Panics: never (a height that is not positive and finite is refused by `Ui::section_list`).
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::SectionList::new(3).row_h(20.0).row_height, 20.0);
    /// ```
    pub const fn row_h(mut self, h: f32) -> SectionList { self.row_height = h; self }

    /// Sets the height in points of the lead-in that scrolls above the heading.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::SectionList::new(3).lead(100.0).leading_height, 100.0);
    /// ```
    pub const fn lead(mut self, h: f32) -> SectionList { self.leading_height = h; self }

    /// Sets the height in points of the sticky column heading.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::SectionList::new(3).header(30.0).header_height, 30.0);
    /// ```
    pub const fn header(mut self, h: f32) -> SectionList { self.header_height = h; self }

    /// Sets the height in points of the space after the last row.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::SectionList::new(3).trail(40.0).trailing_height, 40.0);
    /// ```
    pub const fn trail(mut self, h: f32) -> SectionList { self.trailing_height = h; self }

    /// Floats the scrollbar over the rows instead of taking width from them.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::SectionList::new(3).overlay_scrollbar().overlay_scrollbar);
    /// ```
    pub const fn overlay_scrollbar(mut self) -> SectionList { self.overlay_scrollbar = true; self }

    /// Lets each row take the height it lays out to (wrapped text): `row_height` is its estimate until built, and the heights are kept.
    /// Allocates: never here. Built, the list keeps a height cache of two `f32` a row (at most the 1,000,000-row limit, 8 MB), resized
    /// when the count changes and dropped 600 frames after the list's node is gone.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::SectionList::new(3).measured().measured);
    /// ```
    pub const fn measured(mut self) -> SectionList { self.measured = true; self }
}

/// How `Ui::selection_list` varies the kit's selectable list, as a `Copy` value built with `const fn` builders from
/// `SelectionList::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionList {
    /// Command (Ctrl) toggles a row and Shift selects a run; without, a click selects a row alone.
    pub multi: bool,
}

impl SelectionList {
    /// One row selected at a time.
    pub const DEFAULT: SelectionList = SelectionList { multi: false };

    /// Lets Command (Ctrl) toggle a row and Shift select a run (Space toggles the keyboard's row).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const FILES: pxui_types::ui::SelectionList = pxui_types::ui::SelectionList::DEFAULT.multi();
    /// assert!(FILES.multi);
    /// ```
    pub const fn multi(mut self) -> SelectionList { self.multi = true; self }
}

/// How `Ui::table_view` varies the kit's virtual table, as a `Copy` value built with `const fn` builders from `TableView::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableView {
    /// The sorted column and whether it ascends (its header shows a chevron); `None`: unsorted.
    pub sort: Option<(usize, bool)>,
    /// A fixed text table at this rectangle (in its parent) instead of the virtual table filling its parent: a 24-point header,
    /// hairlines and right-aligned text cells, a click reporting the row.
    pub text: Option<Rect>,
}

impl TableView {
    /// The virtual table, unsorted.
    pub const DEFAULT: TableView = TableView { sort: None, text: None };

    /// The fixed text table at `rect` (in its parent): every cell is the text its callback gives `Ui::item_text`; widths, selection and
    /// sort do not apply, and a click reports its row.
    /// Allocates: never.
    /// Panics: never (a rect that is not finite or is negative is refused by `Ui::table_view`).
    ///
    /// ```
    /// use pxui_types::{geom::Rect, ui::TableView};
    /// const POINTS: TableView = TableView::text(Rect { x: 0.0, y: 0.0, w: 296.0, h: 200.0 });
    /// assert!(POINTS.text.is_some());
    /// ```
    pub const fn text(rect: Rect) -> TableView { TableView { sort: None, text: Some(rect) } }

    /// Marks column `column` sorted, ascending or not (a chevron up or down in its header); the app sorts its rows.
    /// Allocates: never.
    /// Panics: never (a column past the titles is refused by `Ui::table_view`).
    ///
    /// ```
    /// let by_name = pxui_types::ui::TableView::DEFAULT.sort(0, true);
    /// assert_eq!(by_name.sort, Some((0, true)));
    /// ```
    pub const fn sort(mut self, column: usize, ascending: bool) -> TableView { self.sort = Some((column, ascending)); self }
}

/// What `Ui::selection_list` reports this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListResponse {
    /// The row a double click or Enter activated.
    pub activated: Option<usize>,
}

/// What `Ui::tree` reports this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TreeResponse {
    /// The node a double click or Enter activated.
    pub activated: Option<usize>,
    /// How many rows are shown (the start of `visible` the tree wrote).
    pub shown: usize,
}

/// What `Ui::table_view` reports this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TableResponse {
    /// The row clicked (a text table, `TableView::text`).
    pub row: Option<usize>,
    /// The column whose header was clicked: the sort intent (the virtual table).
    pub sort: Option<usize>,
}

/// One toast of `Ui::toasts`: its text and the item id that keys it (never its text), a `Copy` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Toast<'a> {
    /// The app's id of the toast: its identity (two toasts of one text are two toasts).
    pub id: i64,
    /// What it says.
    pub text: &'a str,
}

impl<'a> Toast<'a> {
    /// The toast `id` saying `text`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SAVED: pxui_types::ui::Toast = pxui_types::ui::Toast::new(1, "Saved");
    /// assert_eq!(SAVED.id, 1);
    /// ```
    pub const fn new(id: i64, text: &'a str) -> Toast<'a> { Toast { id, text } }
}

/// How `Ui::checkbox` varies the kit's checkbox, as a `Copy` value built with `const fn` builders from `Checkbox::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checkbox {
    /// The variant a registered checkbox body reads from its config (0: none).
    pub variant: u16,
}

impl Checkbox {
    /// The plain kit checkbox: a row with the 12-point box and the text.
    pub const DEFAULT: Checkbox = Checkbox { variant: 0 };

    /// Sets the variant a registered checkbox body reads from its config (`variant`, 0 by default): one body builds different
    /// structures by it. The built-in body ignores it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SETTING: pxui_types::ui::Checkbox = pxui_types::ui::Checkbox::DEFAULT.variant(1);
    /// assert_eq!(SETTING.variant, 1);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Checkbox { self.variant = variant; self }
}

/// How `Ui::radio` and `Ui::tabs` vary the kit's radio group and tab bar, as a `Copy` value built with `const fn` builders from
/// `Choice::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice<'a> {
    /// The group's (the tab bar's) accessible name; empty: none.
    pub name: &'a str,
    /// The variant the options' bodies (`radio` rows, `tab`s) read from their config (0: none).
    pub variant: u16,
}

impl<'a> Choice<'a> {
    /// The plain kit radio group or tab bar, unnamed (each option is named by its text).
    pub const DEFAULT: Choice<'a> = Choice { name: "", variant: 0 };

    /// Sets the group's (the tab bar's) accessible name; the key is identity only, never the name.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const QUALITY: pxui_types::ui::Choice = pxui_types::ui::Choice::DEFAULT.name("Quality");
    /// assert_eq!(QUALITY.name, "Quality");
    /// ```
    pub const fn name(mut self, name: &'a str) -> Choice<'a> { self.name = name; self }

    /// Sets the variant the registered bodies of the options read from their config (0 by default). The built-in bodies ignore it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const PILLS: pxui_types::ui::Choice = pxui_types::ui::Choice::DEFAULT.variant(2);
    /// assert_eq!(PILLS.variant, 2);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Choice<'a> { self.variant = variant; self }
}

/// How `Ui::dropdown` varies the kit's dropdown, as a `Copy` value built with `const fn` builders from `Dropdown::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dropdown {
    /// The variant a registered dropdown body reads from its config (0: none).
    pub variant: u16,
}

impl Dropdown {
    /// The plain kit dropdown: the text, then a button showing the choice and a chevron.
    pub const DEFAULT: Dropdown = Dropdown { variant: 0 };

    /// Sets the variant a registered dropdown body reads from its config (0 by default). The built-in body ignores it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const COMPACT: pxui_types::ui::Dropdown = pxui_types::ui::Dropdown::DEFAULT.variant(1);
    /// assert_eq!(COMPACT.variant, 1);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Dropdown { self.variant = variant; self }
}

/// How `Ui::slider` varies the kit's slider, as a `Copy` value built with `const fn` builders from `Slider::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slider {
    /// The variant a registered slider body reads from its config (0: none).
    pub variant: u16,
}

impl Slider {
    /// The plain kit slider: the text, then a rail with the filled part and an 8 x 14 thumb.
    pub const DEFAULT: Slider = Slider { variant: 0 };

    /// Sets the variant a registered slider body reads from its config (0 by default). The built-in body ignores it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SEEK: pxui_types::ui::Slider = pxui_types::ui::Slider::DEFAULT.variant(1);
    /// assert_eq!(SEEK.variant, 1);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Slider { self.variant = variant; self }
}

/// The rows a `Ui::picker` fill writes: `(label, detail)` text pairs pushed in order, kept between frames so a settled picker's row
/// strings are reused, not allocated again.
#[derive(Debug, Default)]
pub struct PickRows {
    rows: Vec<(String, String)>,
    len: usize,
}

impl PickRows {
    /// No rows, nothing allocated.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::PickRows::new().is_empty());
    /// ```
    pub const fn new() -> PickRows { PickRows { rows: Vec::new(), len: 0 } }

    /// Appends the row `label` with `detail` beside it.
    /// Allocates: only past the longest rows and the most rows kept so far (their strings are reused).
    /// Identity: none; rows are keyed by their index in the picker.
    /// Panics: never.
    ///
    /// ```
    /// let mut rows = pxui_types::ui::PickRows::new();
    /// rows.push("Song", "Artist");
    /// assert_eq!(rows.get(0), Some(("Song", "Artist")));
    /// ```
    pub fn push(&mut self, label: &str, detail: &str) {
        if self.len == self.rows.len() { self.rows.push((String::new(), String::new())); }
        let row = &mut self.rows[self.len];
        row.0.clear();
        row.0.push_str(label);
        row.1.clear();
        row.1.push_str(detail);
        self.len += 1;
    }

    /// Forgets the rows, keeping their storage.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let mut rows = pxui_types::ui::PickRows::new();
    /// rows.push("a", "");
    /// rows.clear();
    /// assert_eq!(rows.len(), 0);
    /// ```
    pub fn clear(&mut self) { self.len = 0; }

    /// How many rows there are.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::PickRows::new().is_empty());
    /// ```
    pub fn len(&self) -> usize { self.len }

    /// Whether there are no rows.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::PickRows::new().is_empty());
    /// ```
    pub fn is_empty(&self) -> bool { self.len == 0 }

    /// Row `i`'s label and detail, or none past the rows.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::PickRows::new().get(0), None);
    /// ```
    pub fn get(&self, i: usize) -> Option<(&str, &str)> { (i < self.len).then(|| (self.rows[i].0.as_str(), self.rows[i].1.as_str())) }
}

/// The part of a `SectionList` a build callback is asked to fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionPart {
    /// The scrolling lead-in above the heading.
    Leading,
    /// The sticky column heading.
    Header,
    /// Row `n`, counted from 0.
    Row(usize),
}

/// How `Ui::panel`, `Ui::popup`, `Ui::modal` and `Ui::anchored` place and dress their sheet, as one `Copy` value built with `const fn`
/// builders from `Overlay::DEFAULT`. Each call reads the options it has and ignores the rest (listed per builder).
///
/// The default: at (12, 12), the call's own width (a panel 280, a modal 320, a popup 180, an anchored sheet its content's), the
/// content's height, no limit, the theme's look, no padding, a sheet (not a window), unnamed, under its anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Overlay<'a> {
    /// The sheet's top-left in window points (panel, popup).
    pub at: (f32, f32),
    /// The width in points (`None`: the call's own).
    pub w: Option<f32>,
    /// A fixed height in points (panel, anchored), or a popup's height before it is first laid out (its first frame's hit area).
    pub h: Option<f32>,
    /// The tallest the sheet grows in points; past it the content scrolls (panel, popup).
    pub max_h: Option<f32>,
    /// A floating sheet's hairline: the theme's popup look in this colour (panel, popup).
    pub stroke: Option<Rgba>,
    /// A look in place of the theme's (panel).
    pub look: Option<Look>,
    /// The inner padding in whole points (panel).
    pub padding: f32,
    /// Lays the sheet out as a window with inspector rows (panel).
    pub window: bool,
    /// A press outside dismisses the popup on its first frame too (popup).
    pub dismiss_initial: bool,
    /// The popup belongs to the popup built before it (a submenu): it shares its events and is never dismissed by itself (popup).
    pub attached: bool,
    /// The sheet's accessible name (`None`: none; the key is identity only).
    pub name: Option<&'a str>,
    /// Where an anchored sheet sits by its anchor (anchored).
    pub place: crate::layout::Placement,
}

impl<'a> Overlay<'a> {
    /// The plain sheet of each call.
    pub const DEFAULT: Overlay<'a> = Overlay {
        at: (12.0, 12.0), w: None, h: None, max_h: None, stroke: None, look: None, padding: 0.0, window: false, dismiss_initial: false, attached: false,
        name: None, place: crate::layout::Placement::BELOW,
    };

    /// Places the sheet's top-left at (`x`, `y`) window points (panel, popup).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const MENU: pxui_types::ui::Overlay = pxui_types::ui::Overlay::DEFAULT.at(40.0, 60.0);
    /// assert_eq!(MENU.at, (40.0, 60.0));
    /// ```
    pub const fn at(mut self, x: f32, y: f32) -> Overlay<'a> { self.at = (x, y); self }

    /// Sets the width in points (a panel is at least 180).
    /// Allocates: never.
    /// Panics: never (a width that is not finite or is negative is refused by the call).
    ///
    /// ```
    /// const WIDE: pxui_types::ui::Overlay = pxui_types::ui::Overlay::DEFAULT.w(320.0);
    /// assert_eq!(WIDE.w, Some(320.0));
    /// ```
    pub const fn w(mut self, w: f32) -> Overlay<'a> { self.w = Some(w); self }

    /// Sets a fixed height in points (panel, anchored), or a popup's height before it is first laid out.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const PANE: pxui_types::ui::Overlay = pxui_types::ui::Overlay::DEFAULT.h(200.0);
    /// assert_eq!(PANE.h, Some(200.0));
    /// ```
    pub const fn h(mut self, h: f32) -> Overlay<'a> { self.h = Some(h); self }

    /// Sets the tallest the sheet grows in points; past it the content scrolls (panel, popup).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const LIST: pxui_types::ui::Overlay = pxui_types::ui::Overlay::DEFAULT.max_h(400.0);
    /// assert_eq!(LIST.max_h, Some(400.0));
    /// ```
    pub const fn max_h(mut self, h: f32) -> Overlay<'a> { self.max_h = Some(h); self }

    /// Draws the sheet as a floating one with a hairline in `color` (panel, popup).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Rgba, ui::Overlay};
    /// const EDGED: Overlay = Overlay::DEFAULT.stroke(Rgba { r: 0, g: 0, b: 0, a: 40 });
    /// assert!(EDGED.stroke.is_some());
    /// ```
    pub const fn stroke(mut self, color: Rgba) -> Overlay<'a> { self.stroke = Some(color); self }

    /// Draws the sheet in `look` in place of the theme's (panel).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{theme::Look, ui::Overlay};
    /// const BARE: Overlay = Overlay::DEFAULT.look(Look::NONE);
    /// assert!(BARE.look.is_some());
    /// ```
    pub const fn look(mut self, look: Look) -> Overlay<'a> { self.look = Some(look); self }

    /// Sets the inner padding in whole points (panel; a fraction is dropped).
    /// Allocates: never.
    /// Panics: never (a negative padding is refused by the call).
    ///
    /// ```
    /// const ROOMY: pxui_types::ui::Overlay = pxui_types::ui::Overlay::DEFAULT.padding(8.0);
    /// assert_eq!(ROOMY.padding, 8.0);
    /// ```
    pub const fn padding(mut self, padding: f32) -> Overlay<'a> { self.padding = padding; self }

    /// Lays the sheet out as a floating window with inspector rows (panel).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const INSPECTOR: pxui_types::ui::Overlay = pxui_types::ui::Overlay::DEFAULT.window();
    /// assert!(INSPECTOR.window);
    /// ```
    pub const fn window(mut self) -> Overlay<'a> { self.window = true; self }

    /// Lets a press outside dismiss the popup on its first frame too (popup).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const MENU: pxui_types::ui::Overlay = pxui_types::ui::Overlay::DEFAULT.dismiss_initial();
    /// assert!(MENU.dismiss_initial);
    /// ```
    pub const fn dismiss_initial(mut self) -> Overlay<'a> { self.dismiss_initial = true; self }

    /// Attaches the popup to the popup built before it (a submenu): it shares that popup's events and is never dismissed by itself
    /// (popup).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SUBMENU: pxui_types::ui::Overlay = pxui_types::ui::Overlay::DEFAULT.attached();
    /// assert!(SUBMENU.attached);
    /// ```
    pub const fn attached(mut self) -> Overlay<'a> { self.attached = true; self }

    /// Sets the sheet's accessible name (shown up to any `##` in it); without it the sheet has none.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SETTINGS: pxui_types::ui::Overlay = pxui_types::ui::Overlay::DEFAULT.name("Settings");
    /// assert_eq!(SETTINGS.name, Some("Settings"));
    /// ```
    pub const fn name(mut self, name: &'a str) -> Overlay<'a> { self.name = Some(name); self }

    /// Sets where an anchored sheet sits by its anchor (anchored; `Placement::BELOW` by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{layout::{Placement, Side}, ui::Overlay};
    /// const ABOVE: Overlay = Overlay::DEFAULT.place(Placement { side: Side::Above, ..Placement::BELOW });
    /// assert_eq!(ABOVE.place.side, Side::Above);
    /// ```
    pub const fn place(mut self, place: crate::layout::Placement) -> Overlay<'a> { self.place = place; self }
}

/// What `Ui::picker` reports: the query as it stands after this frame's edits, borrowed from the `Ui` until its next call, and what
/// happened to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickerResponse<'a> {
    /// The query after this frame's edits.
    pub query: &'a str,
    /// What happened: a row picked or deleted, submit, back, cancel, or nothing.
    pub pick: Pick,
}

/// Points a pressed pointer travels before a press is a drag: a carry starts, a click ends.
pub const DRAG_THRESHOLD: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    /// The pointer shape for dragging an edge left or right.
    HorizontalResize,
    /// The pointer shape for dragging an edge up or down.
    VerticalResize,
    /// The text insertion (I-beam) pointer.
    Text,
}

/// Which history step a keystroke asked for (`Ui::passed_undo`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndoKind {
    /// Step back.
    Undo,
    /// Step forward again.
    Redo,
}


/// What `Ui::animate` moves a node to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// Move the node's look to this one.
    Look(Look),
    /// A retained view's contents move through its table entry; an ordinary node moves its own paint.
    Offset(f32, f32),
    /// A retained view's contents fade together; an ordinary node fades its own paint.
    Opacity(f32),
    /// A view's contents, after its manual transform, about a fractional point of its box.
    /// The anchor is fixed for the segment; (0, 0) is top-left, (0.5, 0.5) centre.
    Scale { x: f32, y: f32, anchor: (f32, f32) },
}

/// One stop of `Ui::animate_keys`: the node reaches a target `time` seconds after the start, the segment before it shaped by a curve.
///
/// Built by the constructor of its target (`opacity`, `offset`, `look`, `scale`) and, when the segment needs a shape other than the default, `curve`.
///
/// ```
/// use pxui_types::{theme::Curve, ui::{Keyframe, Target}};
/// let stop = Keyframe::opacity(0.4, 0.5).curve(Curve::Spring { omega: 10.0 });
/// assert_eq!((stop.time(), stop.target()), (0.4, Target::Opacity(0.5)));
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe {
    time: f32,
    target: Target,
    curve: crate::theme::Curve,
}

impl Keyframe {
    /// The curve of a stop that names none: CSS `ease`, over the segment's own duration.
    const EASE: crate::theme::Curve = crate::theme::Curve::Bezier { duration: 0.0, points: [0.25, 0.1, 0.25, 1.0] };
    const fn new(time: f32, target: Target) -> Keyframe { Keyframe { time, target, curve: Keyframe::EASE } }
    /// A stop `time` seconds after the start that fades the node to `value` (0 to 1).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::Keyframe;
    /// let _ = Keyframe::opacity(0.2, 0.5);
    /// ```
    pub const fn opacity(time: f32, value: f32) -> Keyframe { Keyframe::new(time, Target::Opacity(value)) }
    /// A stop `time` seconds after the start that moves the node's paint offset to `(x, y)` points.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::Keyframe;
    /// let _ = Keyframe::offset(0.2, 0.0, 8.0);
    /// ```
    pub const fn offset(time: f32, x: f32, y: f32) -> Keyframe { Keyframe::new(time, Target::Offset(x, y)) }
    /// A stop `time` seconds after the start that moves the node's look to `look`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{theme::Look, ui::Keyframe};
    /// let _ = Keyframe::look(0.2, &Look::NONE);
    /// ```
    pub const fn look(time: f32, look: &Look) -> Keyframe { Keyframe::new(time, Target::Look(*look)) }
    /// A stop `time` seconds after the start that scales the view by `x` and `y` about the centre of its box (`anchor` moves the point).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::Keyframe;
    /// let _ = Keyframe::scale(0.2, 1.2, 1.2).anchor(0.5, 1.0);
    /// ```
    pub const fn scale(time: f32, x: f32, y: f32) -> Keyframe { Keyframe::new(time, Target::Scale { x, y, anchor: (0.5, 0.5) }) }
    /// Scales about the fractional point `(x, y)` of the box ((0, 0) is top-left); a stop that does not scale is unchanged.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::Keyframe;
    /// let _ = Keyframe::scale(0.2, 1.2, 1.2).anchor(0.5, 1.0);
    /// ```
    pub const fn anchor(self, x: f32, y: f32) -> Keyframe {
        match self.target {
            Target::Scale { x: sx, y: sy, .. } => Keyframe { target: Target::Scale { x: sx, y: sy, anchor: (x, y) }, ..self },
            _ => self,
        }
    }
    /// Shapes the segment that ends at this stop by `curve` (Bezier, table and steps use the segment's duration).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{theme::Curve, ui::Keyframe};
    /// let _ = Keyframe::opacity(0.2, 1.0).curve(Curve::Spring { omega: 12.0 });
    /// ```
    pub const fn curve(self, curve: crate::theme::Curve) -> Keyframe { Keyframe { curve, ..self } }
    /// Seconds after the start.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let key = pxui_types::ui::Keyframe::opacity(0.2, 0.5);
    /// assert_eq!(key.time(), 0.2);
    /// ```
    pub const fn time(&self) -> f32 { self.time }
    /// Where the stop moves its channel; the other channels keep the stop before's.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::{Keyframe, Target};
    /// assert_eq!(Keyframe::opacity(0.2, 0.5).target(), Target::Opacity(0.5));
    /// ```
    pub const fn target(&self) -> Target { self.target }
    /// The shape of the segment that ends at this stop.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{theme::Curve, ui::Keyframe};
    /// let spring = Curve::Spring { omega: 9.0 };
    /// assert_eq!(Keyframe::opacity(0.2, 0.5).curve(spring).segment(), spring);
    /// ```
    pub const fn segment(&self) -> crate::theme::Curve { self.curve }
}

/// How many times an `animate_keys` run plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    /// This many cycles, at least 1; with `autoreverse` the count includes the reverse legs.
    Count(u16),
    /// Until another run or `Ui::animate` replaces it.
    Forever,
}

/// The playback of an `animate_keys` run: how often it repeats, whether each cycle plays back, and where it starts.
///
/// A `Copy` value built from `Keyframes::DEFAULT` (once, forward, from the start) with const builders.
///
/// ```
/// use pxui_types::ui::{Keyframes, Repeat};
/// const PULSE: Keyframes = Keyframes::DEFAULT.repeat(Repeat::Forever).autoreverse().offset(0.2);
/// assert_ne!(PULSE, Keyframes::DEFAULT);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframes {
    repeat: Repeat,
    autoreverse: bool,
    offset: f32,
}

impl Keyframes {
    /// One forward cycle from the start.
    pub const DEFAULT: Keyframes = Keyframes { repeat: Repeat::Count(1), autoreverse: false, offset: 0.0 };
    /// Plays `repeat` cycles.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::{Keyframes, Repeat};
    /// let _ = Keyframes::DEFAULT.repeat(Repeat::Count(2));
    /// ```
    pub const fn repeat(self, repeat: Repeat) -> Keyframes { Keyframes { repeat, ..self } }
    /// Plays each cycle back after it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui_types::ui::Keyframes::DEFAULT.autoreverse();
    /// ```
    pub const fn autoreverse(self) -> Keyframes { Keyframes { autoreverse: true, ..self } }
    /// Starts `seconds` (finite, not negative) into the first cycle.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui_types::ui::Keyframes::DEFAULT.offset(0.25);
    /// ```
    pub const fn offset(self, seconds: f32) -> Keyframes { Keyframes { offset: seconds, ..self } }
    /// How many cycles run.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::{Keyframes, Repeat};
    /// assert_eq!(Keyframes::DEFAULT.cycles(), Repeat::Count(1));
    /// ```
    pub const fn cycles(&self) -> Repeat { self.repeat }
    /// Whether each cycle plays back after it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::Keyframes::DEFAULT.autoreverse().reverses());
    /// ```
    pub const fn reverses(&self) -> bool { self.autoreverse }
    /// Seconds into the first cycle the run starts at.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::Keyframes::DEFAULT.offset(0.5).start(), 0.5);
    /// ```
    pub const fn start(&self) -> f32 { self.offset }
}

/// The result of a `picker` for a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// Nothing happened.
    None,
    /// Row `n` was chosen.
    Pick(i64),
    /// Row `n` was asked to be deleted.
    Delete(i64),
    /// The query was submitted.
    Submit,
    /// Backspace or Left on an empty query: go up a level.
    Back,
    /// The picker was dismissed.
    Cancel,
}

/// One row of a `Ui::menu`, a `Copy` value built with `const fn` builders from `MenuItem::new`.
///
/// Empty text is a separator line (never chosen, drawn at the menu's separator height).
#[derive(Clone, Copy, Debug)]
pub struct MenuItem<'a> {
    /// The row's label.
    pub text: &'a str,
    /// Whether the row can be chosen; a disabled row shows the disabled look and takes no press.
    pub enabled: bool,
    /// Whether the row reads in the error ink (a delete).
    pub danger: bool,
    /// `Some` keeps a slot before the label for the current-choice mark; `Some(true)` draws the accent square in it.
    pub checked: Option<bool>,
    /// The shortcut text at the row's end (empty for none).
    pub shortcut: &'a str,
    /// A small colour square before the label.
    pub swatch: Option<Rgba>,
    /// The rows of the second menu this row opens beside the menu while the pointer is on it (empty for none).
    pub submenu: &'a [MenuItem<'a>],
    /// Whether the row is the current choice of a settings menu: it reads CHECKED and SELECTED and shows the theme's `checked` menu look
    /// (rows of the menu itself, not of a submenu).
    pub selected: bool,
    /// The row's identity (`None`: its position among the menu's rows), never its text; `Chosen::chosen_id` reports it.
    pub id: Option<i64>,
}

impl<'a> MenuItem<'a> {
    /// An enabled row with `text` and nothing else.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::MenuItem;
    /// const PLAY: MenuItem = MenuItem::new("Play next");
    /// assert!(PLAY.enabled && !PLAY.danger);
    /// ```
    pub const fn new(text: &'a str) -> MenuItem<'a> { MenuItem { text, enabled: true, danger: false, checked: None, shortcut: "", swatch: None, submenu: &[], selected: false, id: None } }

    /// Sets whether the row can be chosen.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let off = pxui_types::ui::MenuItem::new("Paste").enabled(false);
    /// assert!(!off.enabled);
    /// ```
    pub const fn enabled(mut self, enabled: bool) -> MenuItem<'a> { self.enabled = enabled; self }

    /// Inks the row in the theme's error colour.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const DELETE: pxui_types::ui::MenuItem = pxui_types::ui::MenuItem::new("Delete").danger();
    /// assert!(DELETE.danger);
    /// ```
    pub const fn danger(mut self) -> MenuItem<'a> { self.danger = true; self }

    /// Gives the row the mark slot, marked when `checked`; the menu keeps the slot on every row that has one.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let row = pxui_types::ui::MenuItem::new("Small").checked(true);
    /// assert_eq!(row.checked, Some(true));
    /// ```
    pub const fn checked(mut self, checked: bool) -> MenuItem<'a> { self.checked = Some(checked); self }

    /// Shows `shortcut` at the row's end in the secondary ink (display only; the menu binds no key).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let row = pxui_types::ui::MenuItem::new("Copy").shortcut("Ctrl C");
    /// assert_eq!(row.shortcut, "Ctrl C");
    /// ```
    pub const fn shortcut(mut self, shortcut: &'a str) -> MenuItem<'a> { self.shortcut = shortcut; self }

    /// Draws a small colour square before the label.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Rgba, ui::MenuItem};
    /// let row = MenuItem::new("Red").swatch(Rgba { r: 255, g: 0, b: 0, a: 255 });
    /// assert!(row.swatch.is_some());
    /// ```
    pub const fn swatch(mut self, color: Rgba) -> MenuItem<'a> { self.swatch = Some(color); self }

    /// Gives the row a chevron and a second menu of `rows` (rows of its own, no deeper submenus) while the pointer is on it.
    /// The rows of the submenus are numbered after the rows of the menu, in the order of the rows that open them.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::MenuItem;
    /// const SIZES: [MenuItem; 2] = [MenuItem::new("Small"), MenuItem::new("Large")];
    /// let row = MenuItem::new("Size").submenu(&SIZES);
    /// assert_eq!(row.submenu.len(), 2);
    /// ```
    pub const fn submenu(mut self, rows: &'a [MenuItem<'a>]) -> MenuItem<'a> { self.submenu = rows; self }

    /// Sets whether the row is the current choice of a settings menu: it reads CHECKED and SELECTED to accessibility, the built-in row shows the
    /// theme's `checked` menu look (a registered row body gets `state.checked`). It keeps no mark slot; that is `checked`'s accent square.
    /// A row of a submenu ignores it (mark a submenu's choice with `checked`).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SIZES: [pxui_types::ui::MenuItem; 2] = [pxui_types::ui::MenuItem::new("4 GB").selected(true), pxui_types::ui::MenuItem::new("8 GB")];
    /// assert!(SIZES[0].selected && !SIZES[1].selected);
    /// ```
    pub const fn selected(mut self, selected: bool) -> MenuItem<'a> { self.selected = selected; self }

    /// Keys the row by `id` instead of its position, so its focus and hover follow it when rows move, and reports `id` as
    /// `Chosen::chosen_id` when it is chosen. Rows of one menu with the same text are distinct rows either way.
    /// Allocates: never.
    /// Identity: the row is keyed by `id` under the menu (as `Key::Id`); two rows of one menu with one id collide. A submenu's rows are
    /// keyed by their position; their ids are only reported.
    /// Panics: never.
    ///
    /// ```
    /// const MIX: pxui_types::ui::MenuItem = pxui_types::ui::MenuItem::new("Mix").id(42);
    /// assert_eq!(MIX.id, Some(42));
    /// ```
    pub const fn id(mut self, id: i64) -> MenuItem<'a> { self.id = Some(id); self }
}

/// How `Ui::menu` varies the kit's menu, as a `Copy` value built with `const fn` builders from `Menu::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Menu<'a> {
    /// A row's height in points (`None`: the theme's row).
    pub row_h: Option<f32>,
    /// The least width in points (`None`: 150); the menu is as wide as its longest row.
    pub w: Option<f32>,
    /// The most rows shown at once (`None`: all); a menu with more rows scrolls its sheet.
    pub max_rows: Option<usize>,
    /// What the registered bodies of the menu's rows read from their config (`PxuiMenuRowConfig::variant`); 0 for none.
    pub variant: u16,
    /// The space in points above the first row and below the last (6 by default).
    pub padding: f32,
    /// The sheet's accessible name (`None`: none; the key is only the menu's identity).
    pub name: Option<&'a str>,
    /// A press focuses the row it lands on (as `Tab` does), instead of the rows being Tab stops only.
    pub focus_on_click: bool,
    /// The most height of the sheet in points, its edge and padding included (`None`: no bound); a menu taller scrolls its sheet.
    pub max_h: Option<f32>,
}

impl<'a> Menu<'a> {
    /// The plain kit menu: the theme's row height, at least 150 points wide, every row shown, 6 points above and below the rows, no name.
    pub const DEFAULT: Menu<'a> = Menu { row_h: None, w: None, max_rows: None, variant: 0, padding: 6.0, name: None, focus_on_click: false, max_h: None };

    /// Sets a row's height in points; the label, mark, swatch and shortcut centre in it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const TALL: pxui_types::ui::Menu = pxui_types::ui::Menu::DEFAULT.row_h(36.0);
    /// assert_eq!(TALL.row_h, Some(36.0));
    /// ```
    pub const fn row_h(mut self, row_h: f32) -> Menu<'a> { self.row_h = Some(row_h); self }

    /// Sets the least width in points.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const WIDE: pxui_types::ui::Menu = pxui_types::ui::Menu::DEFAULT.w_px(200.0);
    /// assert_eq!(WIDE.w, Some(200.0));
    /// ```
    pub const fn w_px(mut self, w: f32) -> Menu<'a> { self.w = Some(w); self }

    /// Shows at most `rows` rows at once: a menu with more rows is that tall and its sheet scrolls (wheel, drag or touch), the space
    /// above the first row and below the last with the rows.
    /// The submenu of a row is not limited, and is placed level with its row unscrolled.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SHORT: pxui_types::ui::Menu = pxui_types::ui::Menu::DEFAULT.max_rows(8);
    /// assert_eq!(SHORT.max_rows, Some(8));
    /// ```
    pub const fn max_rows(mut self, rows: usize) -> Menu<'a> { self.max_rows = Some(rows); self }

    /// Bounds the whole sheet's height in points, its 1-point edges and the padding above and below the rows included: a menu taller
    /// than `h` is `h` tall and its sheet scrolls, as past `max_rows`. With `max_rows` too the smaller height wins. A sheet is never
    /// shorter than one of the theme's rows; 0 is no bound (stored as `None`). The submenu of a row is not bounded. `Ui::menu` refuses
    /// a negative or non-finite height.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SHEET: pxui_types::ui::Menu = pxui_types::ui::Menu::DEFAULT.max_rows(8).max_h(240.0);
    /// assert_eq!(SHEET.max_h, Some(240.0));
    /// assert_eq!(pxui_types::ui::Menu::DEFAULT.max_h(0.0).max_h, None);
    /// ```
    pub const fn max_h(mut self, h: f32) -> Menu<'a> { self.max_h = if h == 0.0 { None } else { Some(h) }; self }

    /// Sets the variant the registered body of each row reads from its config (`variant`, 0 by default): one body builds different
    /// structures by it, so the app picks per call without comparing text. The built-in body ignores it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const COMPACT: pxui_types::ui::Menu = pxui_types::ui::Menu::DEFAULT.variant(1);
    /// assert_eq!(COMPACT.variant, 1);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Menu<'a> { self.variant = variant; self }

    /// Sets the space in points above the first row and below the last (and so of the submenus); finite and not negative.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const ROOMY: pxui_types::ui::Menu = pxui_types::ui::Menu::DEFAULT.padding(10.0);
    /// assert_eq!(ROOMY.padding, 10.0);
    /// ```
    pub const fn padding(mut self, padding: f32) -> Menu<'a> { self.padding = padding; self }

    /// Sets the sheet's accessible name; without it the sheet has none. The key stays the menu's identity only, never display text.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SIZES: pxui_types::ui::Menu = pxui_types::ui::Menu::DEFAULT.name("Cache size");
    /// assert_eq!(SIZES.name, Some("Cache size"));
    /// ```
    pub const fn name(mut self, name: &'a str) -> Menu<'a> { self.name = Some(name); self }

    /// Sets whether a press on a row focuses it (false by default: the rows take the focus from Tab alone, as buttons do).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const TOUCH: pxui_types::ui::Menu = pxui_types::ui::Menu::DEFAULT.focus_on_click(true);
    /// assert!(TOUCH.focus_on_click);
    /// ```
    pub const fn focus_on_click(mut self, focus: bool) -> Menu<'a> { self.focus_on_click = focus; self }
}

/// One option of a segmented control (`Ui::segmented_with`), a `Copy` value built with `const fn` builders from `Segment::new`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment<'a> {
    /// The text the segment shows; also its accessible name unless `name` is set.
    pub text: &'a str,
    /// The accessible name when it differs from the text (`None`: the text).
    pub name: Option<&'a str>,
}

impl<'a> Segment<'a> {
    /// An option showing `text`, named by it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const FOUR: pxui_types::ui::Segment = pxui_types::ui::Segment::new("4 GB");
    /// assert_eq!((FOUR.text, FOUR.name), ("4 GB", None));
    /// ```
    pub const fn new(text: &'a str) -> Segment<'a> { Segment { text, name: None } }

    /// Sets the accessible name, read in place of the text (what the option does, when the text alone is terse).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const FOUR: pxui_types::ui::Segment = pxui_types::ui::Segment::new("4 GB").name("Use 4 GB");
    /// assert_eq!(FOUR.name, Some("Use 4 GB"));
    /// ```
    pub const fn name(mut self, name: &'a str) -> Segment<'a> { self.name = Some(name); self }
}

/// How `Ui::segmented_with` varies the kit's segmented control, as a `Copy` value built with `const fn` builders from `Segmented::DEFAULT`.
///
/// The default is the kit's plain strip: the theme's control height, each segment as wide as its text plus 16, the text 8 in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segmented<'a> {
    /// The strip's height in points (`None`: the theme's control height).
    pub h: Option<f32>,
    /// The strip fills its line and its segments share its width.
    pub grow: bool,
    /// Every segment's look per state (`None`: the theme's `segment`).
    pub look: Option<&'a States>,
    /// The strip takes input.
    pub enabled: bool,
    /// The strip's accessible name (`None`: none).
    pub name: Option<&'a str>,
    /// What a registered segment body reads from its config (`variant`); 0 for none.
    pub variant: u16,
    /// The strip's width in points, its segments sharing it (`None`: as `grow` says).
    pub w: Option<f32>,
    /// A segment's text inset, and half the room around its text, in points (8 by default).
    pub padding: f32,
    /// The least width of a segment in points (0: none).
    pub min_segment: f32,
    /// A press on an option focuses it (false: a press chooses without focusing, as the kit's buttons leave the focus).
    pub focus_on_click: bool,
}

impl<'a> Segmented<'a> {
    /// The plain kit segmented control.
    pub const DEFAULT: Segmented<'a> = Segmented { h: None, grow: false, look: None, enabled: true, name: None, variant: 0, w: None, padding: 8.0, min_segment: 0.0, focus_on_click: false };

    /// Sets the strip's height in points.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const CHIPS: pxui_types::ui::Segmented = pxui_types::ui::Segmented::DEFAULT.h_px(28.0);
    /// let _ = CHIPS;
    /// ```
    pub const fn h_px(mut self, h: f32) -> Segmented<'a> { self.h = Some(h); self }

    /// Makes the strip fill its line, its segments sharing the width equally, instead of each being as wide as its text.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const WIDE: pxui_types::ui::Segmented = pxui_types::ui::Segmented::DEFAULT.grow();
    /// let _ = WIDE;
    /// ```
    pub const fn grow(mut self) -> Segmented<'a> { self.grow = true; self }

    /// Sets every segment's look per state (idle, hover, pressed, focus, disabled, checked for the choice) in place of the theme's `segment`.
    /// The looks are borrowed, not copied.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{theme::{Look, States}, ui::Segmented};
    /// const CHIP: States = States::all(Look::NONE);
    /// const S: Segmented = Segmented::DEFAULT.look(&CHIP);
    /// let _ = S;
    /// ```
    pub const fn look(mut self, look: &'a States) -> Segmented<'a> { self.look = Some(look); self }

    /// Sets whether the strip takes input: disabled takes no press, key or focus, shows the `disabled` look and reads as disabled to
    /// accessibility.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let s = pxui_types::ui::Segmented::DEFAULT.enabled(false);
    /// let _ = s;
    /// ```
    pub const fn enabled(mut self, enabled: bool) -> Segmented<'a> { self.enabled = enabled; self }

    /// Sets the variant a registered body reads from its config (`variant`, 0 by default): one body builds different structures by it, so the app
    /// picks per call without comparing text. The built-in body ignores it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const V: pxui_types::ui::Segmented = pxui_types::ui::Segmented::DEFAULT.variant(1);
    /// let _ = V;
    /// ```
    pub const fn variant(mut self, variant: u16) -> Segmented<'a> { self.variant = variant; self }

    /// Sets the strip's accessible name; without it the strip has none (each option is named by its text).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const S: pxui_types::ui::Segmented = pxui_types::ui::Segmented::DEFAULT.name("Appearance");
    /// let _ = S;
    /// ```
    pub const fn name(mut self, name: &'a str) -> Segmented<'a> { self.name = Some(name); self }

    /// Sets the strip's width in points, its segments sharing it equally (as `grow` shares its line's).
    /// Allocates: never.
    /// Panics: never (a negative or non-finite width panics at the `segmented_with` call).
    ///
    /// ```
    /// const STRIP: pxui_types::ui::Segmented = pxui_types::ui::Segmented::DEFAULT.w_px(240.0);
    /// let _ = STRIP;
    /// ```
    pub const fn w_px(mut self, w: f32) -> Segmented<'a> { self.w = Some(w); self }

    /// Sets a segment's text inset in points, and half the room around its text when segments are as wide as their text (8 by default).
    /// Allocates: never.
    /// Panics: never (a negative or non-finite padding panics at the `segmented_with` call).
    ///
    /// ```
    /// const ROOMY: pxui_types::ui::Segmented = pxui_types::ui::Segmented::DEFAULT.padding(12.0);
    /// let _ = ROOMY;
    /// ```
    pub const fn padding(mut self, padding: f32) -> Segmented<'a> { self.padding = padding; self }

    /// Sets the least width of a segment in points (none by default), whether it is as wide as its text or shares the strip.
    /// Allocates: never.
    /// Panics: never (a negative or non-finite width panics at the `segmented_with` call).
    ///
    /// ```
    /// const EVEN: pxui_types::ui::Segmented = pxui_types::ui::Segmented::DEFAULT.min_segment(56.0);
    /// let _ = EVEN;
    /// ```
    pub const fn min_segment(mut self, w: f32) -> Segmented<'a> { self.min_segment = w; self }

    /// Sets whether a press on an option focuses it (false by default: a press chooses without focusing, as `Button`, `Menu` and `Pressable`
    /// do). A pressed option is the choice, so it becomes the strip's one Tab stop; a strip with `enabled(false)` takes no press.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const CHIPS: pxui_types::ui::Segmented = pxui_types::ui::Segmented::DEFAULT.focus_on_click(true);
    /// assert!(CHIPS.focus_on_click);
    /// ```
    pub const fn focus_on_click(mut self, focus: bool) -> Segmented<'a> { self.focus_on_click = focus; self }
}

/// What a choosing overlay reports for the frame: `Ui::menu` (a row) and `Ui::dialog` (a button) both answer with it.
///
/// It is the result struct of the calls whose job is a one-off choice; widgets that edit a value they keep on screen (a toggle, a
/// segmented control) report `Response` instead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Chosen {
    /// The index of the row or button chosen this frame (a click, Return or Enter on the focused one; a dialog's Escape picks its cancel
    /// button); the overlay has closed itself.
    pub chosen: Option<usize>,
    /// The `MenuItem::id` of the chosen menu row (`None` when nothing was chosen, the row has none, or the overlay is a dialog).
    pub chosen_id: Option<i64>,
    /// The overlay closed (Escape, window focus loss or a press outside): stop calling it. A dialog's `chosen` is then its cancel button,
    /// when it has an enabled one; a menu's is `None`.
    pub dismissed: bool,
}

/// How `Ui::dialog` varies the modal dialog, as a `Copy` value built with `const fn` builders from `Dialog::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dialog {
    /// The sheet's width in points.
    pub w: f32,
    /// What the registered dialog body reads from its config (`PxuiDialogConfig::variant`); 0 for none. Only the dialog's body is given
    /// it: the buttons and the widgets of the dialog's content see 0.
    pub variant: u16,
}

impl Dialog {
    /// The plain dialog: a 320-point sheet.
    pub const DEFAULT: Dialog = Dialog { w: 320.0, variant: 0 };

    /// Sets the sheet's width in points; it must be finite and not negative.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const WIDE: pxui_types::ui::Dialog = pxui_types::ui::Dialog::DEFAULT.w_px(420.0);
    /// assert_eq!(WIDE.w, 420.0);
    /// ```
    pub const fn w_px(mut self, w: f32) -> Dialog { self.w = w; self }

    /// Sets the variant the registered dialog body reads from its config (`variant`, 0 by default): one body builds different
    /// structures by it, so the app picks per call without comparing text. The built-in body ignores it.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const ALERT: pxui_types::ui::Dialog = pxui_types::ui::Dialog::DEFAULT.variant(1);
    /// assert_eq!(ALERT.variant, 1);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Dialog { self.variant = variant; self }
}

/// One button of `Ui::dialog`, a `Copy` value built with `const fn` builders from `DialogButton::new`; the buttons sit at the end of
/// the dialog's last row in the order given. Return presses the `primary` one, Escape the `cancel` one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DialogButton<'a> {
    /// The label, which is also the accessible name.
    pub text: &'a str,
    /// Return presses this button (the first one marked when several are).
    pub primary: bool,
    /// Escape, and a press outside the dialog, choose this button (the first one marked when several are).
    pub cancel: bool,
    /// The label shows in the theme's invalid colour.
    pub danger: bool,
    /// A disabled button takes no press, shows its disabled look and is skipped by Return and Escape.
    pub enabled: bool,
    /// A secondary line under the label (empty: none), in the theme's label style.
    pub detail: &'a str,
}

impl<'a> DialogButton<'a> {
    /// A plain, enabled button labelled `text`, which it borrows (a literal or a runtime string).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const KEEP: pxui_types::ui::DialogButton = pxui_types::ui::DialogButton::new("Keep");
    /// assert!(KEEP.enabled && !KEEP.primary);
    /// ```
    pub const fn new(text: &'a str) -> DialogButton<'a> { DialogButton { text, primary: false, cancel: false, danger: false, enabled: true, detail: "" } }

    /// Marks the button Return presses: the primary action (not `Default::default`; a button has no default value of its own).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const OK: pxui_types::ui::DialogButton = pxui_types::ui::DialogButton::new("OK").primary();
    /// assert!(OK.primary);
    /// ```
    pub const fn primary(mut self) -> DialogButton<'a> { self.primary = true; self }

    /// Marks the button Escape (and a press outside the dialog) chooses.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const NO: pxui_types::ui::DialogButton = pxui_types::ui::DialogButton::new("Cancel").cancel();
    /// assert!(NO.cancel);
    /// ```
    pub const fn cancel(mut self) -> DialogButton<'a> { self.cancel = true; self }

    /// Inks the label in the theme's invalid colour (a destructive choice).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const DELETE: pxui_types::ui::DialogButton = pxui_types::ui::DialogButton::new("Delete").danger();
    /// assert!(DELETE.danger);
    /// ```
    pub const fn danger(mut self) -> DialogButton<'a> { self.danger = true; self }

    /// Sets whether the button takes input.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let b = pxui_types::ui::DialogButton::new("Save").enabled(false);
    /// assert!(!b.enabled);
    /// ```
    pub const fn enabled(mut self, enabled: bool) -> DialogButton<'a> { self.enabled = enabled; self }

    /// Sets a secondary line under the label, which it borrows (a literal or a runtime string): a choice that needs a sentence of
    /// explanation. The built-in dialog body draws it in the theme's label style under the label, a text node inside the button that
    /// accessibility reads as a label; a registered body reads it with `config.text_at(&config.details, i)` (`bodies::DialogConfig`). Empty is none.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const FOLDER: pxui_types::ui::DialogButton = pxui_types::ui::DialogButton::new("Add folder").detail("Keeps watching it for new music");
    /// assert_eq!(FOLDER.detail, "Keeps watching it for new music");
    /// ```
    pub const fn detail(mut self, detail: &'a str) -> DialogButton<'a> { self.detail = detail; self }
}

/// One ranked suggestion of `Language::complete`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    /// the byte span `insert` replaces
    pub replace: (usize, usize),
    /// the text that goes in
    pub insert: String,
    /// the row
    pub label: String,
    /// the row's right column: a type, a category
    pub detail: String,
    /// one line under the rows while the row is selected
    pub doc: String,
}

/// What a character typed at the caret lands in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeContext {
    /// Plain code.
    Code,
    /// Inside a string literal.
    String,
    /// Inside a comment.
    Comment,
}




/// Options of a floating panel; build from `PanelOpts::default()` with the setters.
#[derive(Clone)]
pub struct PanelOpts {
    /// a floating sheet's hairline (the theme's popup look in this colour); `look` replaces both
    pub stroke: Option<Rgba>,
    /// A look that replaces the theme's for the sheet.
    pub look: Option<Look>,
    /// A window (title-bar look) rather than a sheet.
    pub window: bool,
    /// Left edge in points.
    pub x: f32,
    /// Top edge in points.
    pub y: f32,
    /// Width in points.
    pub width: f32,
    /// Fixed height in points; `None` sizes to the content.
    pub height: Option<f32>,
    /// Largest height in points; `None` for no limit.
    pub max_height: Option<f32>,
    /// Inner padding in points.
    pub padding: i64,
}

impl Default for PanelOpts {
    fn default() -> PanelOpts {
        PanelOpts { stroke: None, look: None, window: false, x: 12.0, y: 12.0, width: 280.0, height: None, max_height: None, padding: 0 }
    }
}

impl PanelOpts {
    /// The options with the panel's top-left at `x`, `y` points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::PanelOpts;
    /// let opts = PanelOpts::default().at(20.0, 30.0);
    /// assert_eq!((opts.x, opts.y), (20.0, 30.0));
    /// ```
    pub fn at(mut self, x: f32, y: f32) -> PanelOpts {
        self.x = x;
        self.y = y;
        self
    }
    /// The options with the panel `width` points wide (default 280).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::PanelOpts::default().width(320.0).width, 320.0);
    /// ```
    pub fn width(mut self, width: f32) -> PanelOpts {
        self.width = width;
        self
    }
    /// The options with a fixed height of `height` points instead of the content's.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::PanelOpts::default().height(200.0).height, Some(200.0));
    /// ```
    pub fn height(mut self, height: f32) -> PanelOpts {
        self.height = Some(height);
        self
    }
    /// The options with the panel no taller than `height` points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::PanelOpts::default().max_height(400.0).max_height, Some(400.0));
    /// ```
    pub fn max_height(mut self, height: f32) -> PanelOpts {
        self.max_height = Some(height);
        self
    }
    /// The options with `padding` points of inner padding.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::PanelOpts::default().padding(8).padding, 8);
    /// ```
    pub fn padding(mut self, padding: i64) -> PanelOpts {
        self.padding = padding;
        self
    }
    /// The options with the panel drawn as a window (`true`) or a sheet (`false`, the default).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::PanelOpts::default().window(true).window);
    /// ```
    pub fn window(mut self, window: bool) -> PanelOpts {
        self.window = window;
        self
    }
}

/// What a `line_field`'s keys asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineStep {
    /// No navigation key was pressed.
    None,
    /// Move on to the next field.
    Next,
    /// Move back to the previous field.
    Previous,
    /// Escape: close the field.
    Close,
}


/// A text button of an inspector head: its label, its key in ink-3, whether it is on (the control
/// fill) and whether it can be pressed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadAction {
    /// The button's label.
    pub caption: String,
    /// The key shown beside it in ink-3.
    pub keycap: String,
    /// Whether it is on (drawn with the control fill).
    pub active: bool,
    /// Whether it can be pressed.
    pub usable: bool,
}

/// What a head reports for a frame: the name as edited, the index of the pressed action, and
/// whether the trailing `reset` button was pressed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Head {
    /// The name as edited.
    pub renamed: String,
    /// Index of the action pressed this frame, if any.
    pub chosen: Option<i64>,
    /// Whether the trailing reset button was pressed.
    pub reset_pressed: bool,
}


/// The text being edited with its caret and selection anchor (byte offsets).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEdit {
    /// The text.
    pub text: String,
    /// Caret byte offset (a char boundary).
    pub caret: usize,
    /// The other end of the selection, as a byte offset; equal to `caret` when nothing is selected.
    pub anchor: usize,
}





/// `label` cut to fit in `limit` points, ending in an ellipsis (U+2026) when it was cut; `width` measures a string in points.
///
/// The whole label when it fits; otherwise its longest prefix that fits with the ellipsis after it, and an empty string when even
/// the ellipsis alone does not fit.
/// Allocates: only when it cuts (the shortened `String`); a label that fits is borrowed.
/// Identity: `label` is display text, not a key; nothing is keyed or cached.
/// Panics: never (unless `width` does).
///
/// ```
/// use pxui_types::ui::ellipsis;
/// let mut width = |s: &str| s.chars().count() as f32 * 10.0;
/// assert_eq!(ellipsis(&mut width, 55.0, "abcdefgh"), "abcd\u{2026}");
/// assert_eq!(ellipsis(&mut width, 100.0, "abc"), "abc");
/// assert_eq!(ellipsis(&mut width, 5.0, "abc"), "");
/// ```
pub fn ellipsis<'a>(width: &mut dyn FnMut(&str) -> f32, limit: f32, label: &'a str) -> Cow<'a, str> {
    let dots = width("\u{2026}");
    // `cut`: the longest prefix that fits with the ellipsis after it; -1 when the ellipsis alone does
    // not
    let mut cut: i64 = if dots <= limit { 0 } else { -1 };
    let mut used = 0.0;
    for (index, c) in label.char_indices() {
        let next = index + c.len_utf8();
        used += width(&label[index..next]);
        if used > limit {
            return if cut < 0 { Cow::Borrowed("") } else { Cow::Owned(format!("{}\u{2026}", &label[..cut as usize])) };
        }
        if used + dots <= limit {
            cut = next as i64;
        }
    }
    Cow::Borrowed(label)
}

// ---------------------------------------------------------------- keys

/// A node's key among its siblings: a fixed label or an item id, never display text.
///
/// Like React's `key`: unique among the children of one parent, so list rows pass their item id. A key is resolved under the
/// parent's seed, so the same key under two parents names two nodes. Two siblings with one key take order-stable `#dup` keys.
/// Write it with `.into()` at a call site: `ui.node("toolbar".into(), node)`, `ui.node(song.id.into(), node)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A fixed label, hashed as the label of `key_of` under the parent's seed (a `###` suffix replaces it).
    Label(&'static str),
    /// An item id, hashed as the label of a NUL byte and its 16 lowercase hex digits under the parent's seed.
    Id(i64),
}
impl From<&'static str> for Key { #[inline] fn from(label: &'static str) -> Key { Key::Label(label) } }
impl From<i64> for Key { #[inline] fn from(id: i64) -> Key { Key::Id(id) } }
impl From<u64> for Key { #[inline] fn from(id: u64) -> Key { Key::Id(id as i64) } }

impl Key {
    /// The label this key is hashed as under the parent's seed: a `Label` itself, an `Id` written into `buf` as a NUL byte and
    /// its 16 lowercase hex digits (so an id never collides with a printable label).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::Key;
    /// let mut buf = [0u8; 17];
    /// assert_eq!(Key::Label("row").label(&mut buf), "row");
    /// assert_eq!(Key::Id(255).label(&mut buf), "\u{0}00000000000000ff");
    /// ```
    #[inline]
    pub fn label<'b>(&self, buf: &'b mut [u8; 17]) -> &'b str {
        match *self { Key::Label(label) => label, Key::Id(id) => id_text(id, buf) }
    }
}

/// `id` as a label: a NUL byte and its 16 lowercase hex digits, written into `out`. Out of line: label keys inline without it.
#[cold]
#[inline(never)]
fn id_text(id: i64, out: &mut [u8; 17]) -> &str {
    out[0] = 0;
    for (at, digit) in out[1..].iter_mut().enumerate() { *digit = b"0123456789abcdef"[(id as u64 >> (60 - 4 * at)) as usize & 15]; }
    // NUL and ASCII hex digits: always UTF-8
    core::str::from_utf8(out).unwrap_or_default()
}

/// The seed of a root: the key every top-level label hashes under.
pub const ROOT_KEY: i64 = 0x2c1b3c6d;

/// The key of bytes `first..last` of `text` under `seed`; never 0 or 1, which the engine's tables keep as empty and tombstone marks.
///
/// `first` and `last` are byte offsets and are not checked for char boundaries.
/// Allocates: never.
/// Panics: when `first > last` or `last` is past the end of `text`.
///
/// ```
/// use pxui_types::ui::hash_range;
/// assert_eq!(hash_range(7, "abcdef", 2, 4), hash_range(7, "xxcdxx", 2, 4));
/// ```
pub fn hash_range(seed: i64, text: &str, first: usize, last: usize) -> i64 {
    let hash = hash::bytes(seed as u64, &text.as_bytes()[first..last]) as i64;
    if hash == 0 || hash == 1 { hash + 2 } else { hash }
}

/// The byte index of the first `marker` (`##` or `###`) in `text`, or `None`; text with no `#` at all is rejected without searching.
///
/// A `marker` that has no `#` in it is never found.
pub(crate) fn find_marker(text: &str, marker: &str) -> Option<usize> {
    if text.contains('#') { text.find(marker) } else { None }
}

/// The key of the label `text` under the parent seed `seed`: the id after a `###` replaces the label's hash, otherwise all of `text`
/// is hashed (a `##` suffix is part of the key; `shown` is what strips it from the display).
///
/// Never 0 or 1. Two labels that differ only before `###` share a key.
/// Allocates: never.
/// Identity: `text` is the node label; the same text under the same seed collides.
/// Panics: never.
///
/// ```
/// use pxui_types::ui::key_of;
/// assert_eq!(key_of(1, "Play###a"), key_of(1, "Stop###a"));
/// assert_ne!(key_of(1, "Play##a"), key_of(1, "Play##b"));
/// ```
pub fn key_of(seed: i64, text: &str) -> i64 {
    // the id after ### replaces the label's hash, under the same parent: two panels that build the
    // same widget keep separate state
    match find_marker(text, "###") {
        Some(index) => hash_range(seed, text, index + 3, text.len()),
        None => hash_range(seed, text, 0, text.len()),
    }
}

/// The label without its `##` suffix (everything from the first `##` on), borrowed from `text`.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// assert_eq!(pxui_types::ui::shown("Play##hotkey"), "Play");
/// assert_eq!(pxui_types::ui::shown("Play"), "Play");
/// ```
pub fn shown(text: &str) -> &str {
    match find_marker(text, "##") {
        Some(index) => &text[..index],
        None => text,
    }
}

/// The label without its `##` suffix, as an owned `String`.
///
/// Allocates: one `String`.
/// Panics: never.
///
/// ```
/// assert_eq!(pxui_types::ui::display("Play##hotkey"), "Play".to_owned());
/// ```
pub fn display(text: &str) -> String {
    shown(text).to_owned()
}

/// The key of child number `n` under `seed`, derived from the integers alone (no label string per frame); never 0 or 1.
///
/// Allocates: never.
/// Identity: `n` is the child's number; two equal `n` under one seed collide.
/// Panics: never.
///
/// ```
/// use pxui_types::ui::int_key;
/// assert_ne!(int_key(5, 1), int_key(5, 2));
/// assert_eq!(int_key(5, 1), int_key(5, 1));
/// ```
pub fn int_key(seed: i64, n: i64) -> i64 {
    let hash = hash::mix(seed, n);
    if hash == 0 || hash == 1 { hash + 2 } else { hash }
}

// ---------------------------------------------------------------- text helpers

/// Where the top of kit text of `size` points goes to sit centred in a row at `y` of height `h`: its
/// top is half of what the row has over the text size. Measured against the kit's reference
/// renders: 13-point text in a 24-point row has its ascenders at 7 and its descenders at 18.
///
/// `size` of `None` is `font_size`; the offset rounds down to a whole point. All values are points.
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::ui::text_top_for;
/// assert_eq!(text_top_for(13, None, 100.0, 24.0), 105.0);
/// ```
pub fn text_top_for(font_size: i64, size: Option<i64>, y: f32, h: f32) -> f32 {
    y + ((h - size.unwrap_or(font_size) as f32) / 2.0).floor()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// How far a number-literal scrub has got.
pub enum ScrubPhase {
    /// The pointer is still dragging; the value is provisional.
    Live,
    /// The drag ended; the value is final.
    Done,
}
