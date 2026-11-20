//! Plain UI data and pure functions shared by the engine and the wrapper: box flags, keys, small results and text helpers. One
//! definition each; `pxui_core` and `pxui` re-export them. The option records that hold closures (`Rc<dyn Fn>`) are not here: they
//! are not ABI types, and the engine and the wrapper each keep theirs (`opts` in both).
use crate::geom::{hash, Rgba};
use crate::theme::{Look, States};
use std::borrow::Cow;
use crate::PxuiBox as Handle;

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
    /// Rows move by dragging their `Part::HANDLE`, by Alt with Up or Down on the focused row and by the rows' accessibility move
    /// actions; a payload carried between rows shows the engine's insertion line in the drop look (`ListResponse::moved`,
    /// `ListResponse::drop_at`).
    pub reorder: bool,
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
        SectionList { count, row_height: 24.0, leading_height: 0.0, header_height: 0.0, trailing_height: 0.0, overlay_scrollbar: false, measured: false, reorder: false }
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

    /// Sets the height in points of the trailer after the last row (built as `SectionPart::Trailing`; blank when the callback builds
    /// nothing there).
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

    /// Makes the rows reorderable (`true`) or not: a drag that starts on a row's `Part::HANDLE` lifts the row, the rows between make
    /// room with the theme's move animation, and the release reports `ListResponse::moved` (the app moves its item); Alt with Up or
    /// Down moves the focused row and each row offers the accessibility actions Move up and Move down, reported the same way. A payload
    /// carried over the list (`Ui::carry`) shows an insertion line between the rows it would land between, which the engine draws in
    /// the list style's drop look (`SelectionList::drop_look`; a `section_list` uses the theme's accent), and its drop reports
    /// `ListResponse::drop_at`.
    /// Allocates: never (dragging allocates nothing either).
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::SectionList::new(3).reorder(true).reorder);
    /// ```
    pub const fn reorder(mut self, on: bool) -> SectionList { self.reorder = on; self }
}

/// What a press and the keys do to a `Ui::selection_list`'s selection (`SelectionList::mode`; C `PxuiSelectionListDesc.mode`, the
/// discriminant). Every mode activates a row on Enter (the keyboard's row, while selected; in `None` whatever its state), on a double
/// click (not in `Toggle`) and, with `open_on_click`, on a plain click (not in `Toggle`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SelectMode {
    /// A click or a key selects one row alone.
    Single = 0,
    /// Command (Ctrl) toggles a row and Shift selects a run; a plain click selects a row alone; Command-A selects every row and
    /// Escape clears the selection (Space toggles the keyboard's row off Apple).
    Multi = 1,
    /// As `Multi`, but a plain click toggles its row (a checklist's select mode, as a phone's edit mode).
    Toggle = 2,
    /// Rows activate and nothing is selected: presses and keys move the keyboard's row only (a flag the app writes still shows);
    /// Command-A, Escape and Space stay the app's.
    None = 3,
}

/// How `Ui::selection_list` varies the kit's selectable list, as a `Copy` value built with `const fn` builders from
/// `SelectionList::DEFAULT`: how rows are selected (`SelectMode`), the `SectionList` geometry (row height, lead-in, sticky heading, trailer,
/// measured rows, overlay scrollbar), reordering and the `selection_list` body variant. The row count is the call's runtime parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionList {
    /// What presses and keys do to the selection.
    pub mode: SelectMode,
    /// Height of each row in points (its estimate until built when `measured`).
    pub row_height: f32,
    /// Height in points of the lead-in that scrolls above the heading (`SectionPart::Leading`).
    pub leading_height: f32,
    /// Height in points of the sticky heading (`SectionPart::Header`).
    pub header_height: f32,
    /// Height in points of the space after the last row.
    pub trailing_height: f32,
    /// The scrollbar floats over the rows instead of taking width from them.
    pub overlay_scrollbar: bool,
    /// Rows take the heights they lay out to (`row_height` is their estimate until built).
    pub measured: bool,
    /// Rows move as `SectionList::reorder` describes, their flags in `ListState` moving with them; a fitted list (`fit`) moves rows by
    /// the keyboard and accessibility only (its rows are not dragged).
    pub reorder: bool,
    /// The `selection_list` body variant the rows are built with (0: the theme's).
    pub variant: u16,
    /// The list is as tall as its rows and does not scroll: every row is built (a short group in a sidebar column).
    pub fit: bool,
    /// A single pointer or touch click (without Command or Shift) also activates the row (`open_on_click`).
    pub open_on_click: bool,
    /// The look a row shows in place of its state look while a carried payload is over it (in a reorderable list, the insertion
    /// line's); `None`: the row keeps its state look.
    pub drop_look: Option<Look>,
    /// The list draws no frame: no frame look and no 1-point inset, so its rows sit at its own edges.
    pub bare: bool,
}

impl SelectionList {
    /// One row selected at a time (`SelectMode::Single`), 24-point fixed rows, no lead-in, heading or trailer, not reorderable, the theme's row body.
    pub const DEFAULT: SelectionList = SelectionList { mode: SelectMode::Single, row_height: 24.0, leading_height: 0.0, header_height: 0.0, trailing_height: 0.0, overlay_scrollbar: false, measured: false, reorder: false, variant: 0, fit: false, open_on_click: false, drop_look: None, bare: false };

    /// Sets what presses and keys do to the selection (`SelectMode`; `Single` by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::{SelectMode, SelectionList};
    /// const FILES: SelectionList = SelectionList::DEFAULT.mode(SelectMode::Multi);
    /// assert_eq!(FILES.mode, SelectMode::Multi);
    /// ```
    pub const fn mode(mut self, mode: SelectMode) -> SelectionList { self.mode = mode; self }

    /// Sets each row's height in points (positive and finite; the estimate until built when `measured`).
    /// Allocates: never.
    /// Panics: never (a height that is not positive and finite is refused by `Ui::selection_list`).
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::SelectionList::DEFAULT.row_h(38.0).row_height, 38.0);
    /// ```
    pub const fn row_h(mut self, h: f32) -> SelectionList { self.row_height = h; self }

    /// Sets the height in points of the lead-in that scrolls above the heading (built as `SectionPart::Leading`).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::SelectionList::DEFAULT.lead(120.0).leading_height, 120.0);
    /// ```
    pub const fn lead(mut self, h: f32) -> SelectionList { self.leading_height = h; self }

    /// Sets the height in points of the sticky heading (built as `SectionPart::Header`; a `Ui::table_head` fits there).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::SelectionList::DEFAULT.header(28.0).header_height, 28.0);
    /// ```
    pub const fn header(mut self, h: f32) -> SelectionList { self.header_height = h; self }

    /// Sets the height in points of the trailer after the last row (built as `SectionPart::Trailing`, never a row: not selectable,
    /// not counted, skipped by the keyboard).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::SelectionList::DEFAULT.trail(40.0).trailing_height, 40.0);
    /// ```
    pub const fn trail(mut self, h: f32) -> SelectionList { self.trailing_height = h; self }

    /// Floats the scrollbar over the rows instead of taking width from them.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::SelectionList::DEFAULT.overlay_scrollbar().overlay_scrollbar);
    /// ```
    pub const fn overlay_scrollbar(mut self) -> SelectionList { self.overlay_scrollbar = true; self }

    /// Lets each row take the height it lays out to, as `SectionList::measured` does (the same height cache and bounds).
    /// Allocates: never here; built, as `SectionList::measured` says.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::SelectionList::DEFAULT.measured().measured);
    /// ```
    pub const fn measured(mut self) -> SelectionList { self.measured = true; self }

    /// Makes the rows reorderable (`true`) or not, as `SectionList::reorder` describes; a row moved keeps its selection flags (the
    /// widget moves them in the list state with the row).
    /// Allocates: never (dragging allocates nothing either).
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::SelectionList::DEFAULT.reorder(true).reorder);
    /// ```
    pub const fn reorder(mut self, on: bool) -> SelectionList { self.reorder = on; self }

    /// Builds the rows with variant `variant` of the `selection_list` body (0: the theme's), so one registered body can draw several
    /// row styles (a sidebar row, a song row).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::SelectionList::DEFAULT.variant(2).variant, 2);
    /// ```
    pub const fn variant(mut self, variant: u16) -> SelectionList { self.variant = variant; self }

    /// Makes the list as tall as its rows, without scrolling or virtualizing (every row is built each frame): for a short group
    /// stacked with others in one column (a sidebar's routes, sources and lists). Keep such lists short; a long one is the default
    /// virtual list.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::SelectionList::DEFAULT.fit().fit);
    /// ```
    pub const fn fit(mut self) -> SelectionList { self.fit = true; self }

    /// With `true`, a single pointer or touch click (without Command or Shift) selects the row and also reports it in
    /// `ListResponse::activated` (a folder list that opens on one click); Enter and a double click always activate, and the arrow,
    /// Page, Home and End keys only move the selection, never activate.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::SelectionList::DEFAULT.open_on_click(true).open_on_click);
    /// ```
    pub const fn open_on_click(mut self, on: bool) -> SelectionList { self.open_on_click = on; self }

    /// Shows `look` on a row in place of its state look while a carried payload is over the row (a sidebar list a song is dragged
    /// onto; the app takes the drop with `Ui::drop_target(&cell.handle)`). The look is the list style's, so no per-row style call and
    /// no per-frame look swap; it wins over the look a row body gave the row, and the body is told through its config (`drop` 3). A row
    /// flagged `ListState::NO_DROP` takes no payload and keeps its look (U2b.4.21). In a reorderable list the payload lands
    /// between rows instead, and this look draws the insertion line.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ui::SelectionList, Look};
    /// const LISTS: SelectionList = SelectionList::DEFAULT.drop_look(Look::NONE.radius(6.0));
    /// assert!(LISTS.drop_look.is_some());
    /// ```
    pub const fn drop_look(mut self, look: Look) -> SelectionList { self.drop_look = Some(look); self }

    /// Draws the list without its frame: no frame look (the theme's `scroll_view`) and no 1-point inset, so the rows sit at the list's
    /// own edges (a sidebar group laid out with the column around it). Keyboard focus and scrolling are unchanged.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::SelectionList::DEFAULT.bare().bare);
    /// ```
    pub const fn bare(mut self) -> SelectionList { self.bare = true; self }
}

/// What `Ui::list`, `Ui::section_list` and `Ui::selection_list` report this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct ListResponse {
    /// The list's scroll box (for `Ui::reveal`, `Ui::scroll`, `Ui::transfer_carry` and `Ui::drop_target`).
    pub handle: Handle,
    /// The row a double click or Enter activated (`Ui::selection_list`; a plain list's rows report their own presses).
    pub activated: Option<usize>,
    /// Set whenever the widget changed the list state's selection this frame (`Ui::selection_list`, `Ui::table`), naming the row the
    /// change acted on: the row clicked, Command-toggled (on or off), Shift-extended to, moved to by a key or toggled by Space (not on Apple); for
    /// Command-A the keyboard's row (0 without one). A navigation list (a sidebar, a folder browser) acts on it as "this row was
    /// chosen"; a multi-selection list reads it as "the selection changed" (to invalidate what it caches). Either way the app never
    /// scans `ListState::rows` per frame. App writes to `ListState` never set it.
    pub selected: Option<usize>,
    /// The row a secondary press (right click, Control-click or long press) began on (`Ui::selection_list`).
    pub secondary: Option<usize>,
    /// The box of the row `activated` or `secondary` names, to anchor a menu or popover on it; valid for this frame.
    pub row: Option<Handle>,
    /// A reorder this frame (`SectionList::reorder`, `SelectionList::reorder`): `(from, to)`, the row at index `from` before the move
    /// now sits at index `to` after it (`to` is the final index: removing the item at `from` and inserting it at `to` gives the new
    /// order; `from != to`). The app moves its item that way; a `selection_list` has already moved the row's flags in its state.
    pub moved: Option<(usize, usize)>,
    /// The insertion index (0 to the row count) a carried payload was dropped at between the rows of a reorderable list, this
    /// frame only; read the payload with `Ui::drop_target(&response.handle)` on the same frame. A move of the list's own row is
    /// `moved`, never `drop_at`.
    pub drop_at: Option<usize>,
}

/// What `Ui::tree` reports this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TreeResponse {
    /// The node a double click or Enter activated.
    pub activated: Option<usize>,
    /// How many rows are shown (the start of `visible` the tree wrote).
    pub shown: usize,
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
///
/// The default is the plain checkbox: a row with the 12-point box in the theme's `checkbox` look and the text, taking presses and focus.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Checkbox<'a> {
    /// What a registered checkbox body reads from its config (`variant`); 0 for none.
    pub variant: u16,
    /// The box's side in points (`None`: 12); its inner mark is half of it.
    pub size: Option<f32>,
    /// The box's look per state (the `checked` look is on) in place of the theme's `checkbox`, borrowed.
    pub look: Option<&'a States>,
    /// The checkbox takes no input, shows the `disabled` look and reads as disabled.
    pub disabled: bool,
    /// A passive mark: no press, focus or hover of its own (the row it sits in owns them) and, with empty text, the box alone.
    pub passive: bool,
}

impl<'a> Checkbox<'a> {
    /// The plain kit checkbox.
    pub const DEFAULT: Checkbox<'a> = Checkbox { variant: 0, size: None, look: None, disabled: false, passive: false };

    /// Sets the variant a registered body reads from its config (`variant`, 0 by default): one body builds different structures by it, so
    /// the app picks per call without comparing text. The built-in body ignores it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const V: pxui_types::ui::Checkbox = pxui_types::ui::Checkbox::DEFAULT.variant(1);
    /// assert_eq!(V.variant, 1);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Checkbox<'a> { self.variant = variant; self }

    /// Sets the box's side in points (12 by default, and for 0); the inner mark stays half of it.
    /// Allocates: never.
    /// Panics: never here; `Ui::checkbox` panics on a negative or non-finite side (a caller bug).
    ///
    /// ```
    /// const BIG: pxui_types::ui::Checkbox = pxui_types::ui::Checkbox::DEFAULT.size(18.0);
    /// assert_eq!(BIG.size, Some(18.0));
    /// ```
    pub const fn size(mut self, points: f32) -> Checkbox<'a> { self.size = Some(points); self }

    /// Sets the box's look per state (idle, hover, pressed, focus, disabled; `checked` while on) in place of the theme's `checkbox`.
    /// The looks are borrowed, not copied.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{theme::{Look, States}, ui::Checkbox};
    /// const BOX: States = States::of(Look::NONE);
    /// const CHIP: Checkbox = Checkbox::DEFAULT.look(&BOX);
    /// assert!(CHIP.look.is_some());
    /// ```
    pub const fn look(mut self, looks: &'a States) -> Checkbox<'a> { self.look = Some(looks); self }

    /// Sets whether the checkbox is disabled: it takes no press or focus, shows the `disabled` look and reads as disabled to
    /// accessibility, and still blocks what is under it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::Checkbox::DEFAULT.disabled(true).disabled);
    /// ```
    pub const fn disabled(mut self, disabled: bool) -> Checkbox<'a> { self.disabled = disabled; self }

    /// Sets whether the checkbox is a passive mark for a row that owns the press: it has no hit area, focus or hover, takes no input and
    /// reports no click; it still shows `on` and tells assistive technology its checked state (read-only). With empty text it is the box
    /// alone, as wide as the box.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::Checkbox::DEFAULT.passive(true).passive);
    /// ```
    pub const fn passive(mut self, passive: bool) -> Checkbox<'a> { self.passive = passive; self }
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

/// The artwork or icon slot of a picker row (`PickRow::art`): what the row's body draws at the row's start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickArt {
    /// A character of the icon font, which the built-in row body draws.
    Glyph(char),
    /// An image by its `Images` id (the id `Images::store` returns and `Images::get` takes): the built-in row body leaves the slot empty
    /// and a registered picker body draws the image.
    Image(usize),
    /// The app's icon `word` (as `Icon::Draw`'s word, the app's icon id): the built-in row body leaves the slot empty and a registered
    /// picker body draws the icon by its word (`PickerConfig::art`).
    Draw(u32),
}

/// One row of a `Ui::picker` fill, a `Copy` value built with `const fn` builders from `PickRow::new` and given to `PickRows::push`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickRow<'a> {
    /// The row's text.
    pub label: &'a str,
    /// The row's secondary text ("": none).
    pub detail: &'a str,
    /// The artwork or icon at the row's start (`None`: none).
    pub art: Option<PickArt>,
    /// The heading of the group this row starts ("": the row falls under the heading before it, or none).
    pub heading: &'a str,
}

impl<'a> PickRow<'a> {
    /// A row showing `label`, with no detail, artwork or heading.
    /// Allocates: never.
    /// Identity: none; a picker keys its rows by their index, never the label.
    /// Panics: never.
    ///
    /// ```
    /// const SONG: pxui_types::ui::PickRow = pxui_types::ui::PickRow::new("Song");
    /// assert_eq!((SONG.label, SONG.detail), ("Song", ""));
    /// ```
    pub const fn new(label: &'a str) -> PickRow<'a> { PickRow { label, detail: "", art: None, heading: "" } }

    /// Sets the row's secondary text, shown beside the label.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let row = pxui_types::ui::PickRow::new("Song").detail("Artist");
    /// assert_eq!(row.detail, "Artist");
    /// ```
    pub const fn detail(mut self, detail: &'a str) -> PickRow<'a> { self.detail = detail; self }

    /// Sets the artwork or icon drawn at the row's start; when any row of a fill has one, every row keeps the slot's room so labels align.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::{PickArt, PickRow};
    /// const ROW: PickRow = PickRow::new("Song").art(PickArt::Image(7));
    /// assert_eq!(ROW.art, Some(PickArt::Image(7)));
    /// ```
    pub const fn art(mut self, art: PickArt) -> PickRow<'a> { self.art = Some(art); self }

    /// Makes this row start a group headed `heading`: the picker draws the heading above it, and this row and the rows after it up to the
    /// next heading fall under it. A heading is not a row: the keyboard skips it and `Pick` indices count only rows.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let row = pxui_types::ui::PickRow::new("Song").heading("Songs");
    /// assert_eq!(row.heading, "Songs");
    /// ```
    pub const fn heading(mut self, heading: &'a str) -> PickRow<'a> { self.heading = heading; self }
}

#[derive(Debug, Default)]
struct RowText {
    label: String,
    detail: String,
    heading: String,
    art: Option<PickArt>,
}

/// The rows a `Ui::picker` fill writes: `PickRow`s pushed in order, kept between frames so a settled picker's row strings are reused,
/// not allocated again.
#[derive(Debug, Default)]
pub struct PickRows {
    rows: Vec<RowText>,
    len: usize,
}

impl PickRows {
    /// No rows.
    /// Allocates: never (the first rows allocate their strings).
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::PickRows::new().is_empty());
    /// ```
    pub const fn new() -> PickRows { PickRows { rows: Vec::new(), len: 0 } }

    /// Appends `row`: its label, detail, artwork and the heading of the group it starts.
    /// Allocates: only past the most rows or the longest texts so far (the row strings are reused).
    /// Identity: none; rows are keyed by their index in the picker (headings are not rows).
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::{PickRow, PickRows};
    /// let mut rows = PickRows::new();
    /// rows.push(PickRow::new("Song").detail("Artist").heading("Songs"));
    /// assert_eq!(rows.get(0), Some(PickRow::new("Song").detail("Artist").heading("Songs")));
    /// ```
    pub fn push(&mut self, row: PickRow<'_>) {
        if self.len == self.rows.len() { self.rows.push(RowText::default()); }
        let at = &mut self.rows[self.len];
        for (to, from) in [(&mut at.label, row.label), (&mut at.detail, row.detail), (&mut at.heading, row.heading)] {
            to.clear();
            to.push_str(from);
        }
        at.art = row.art;
        self.len += 1;
    }

    /// Forgets the rows, keeping their strings for the next fill.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let mut rows = pxui_types::ui::PickRows::new();
    /// rows.push(pxui_types::ui::PickRow::new("a"));
    /// rows.clear();
    /// assert!(rows.is_empty());
    /// ```
    pub fn clear(&mut self) { self.len = 0; }

    /// The number of rows.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::PickRows::new().len(), 0);
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

    /// Row `i` as it was pushed, borrowed from the rows; `None` past them.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::PickRows::new().get(0), None);
    /// ```
    pub fn get(&self, i: usize) -> Option<PickRow<'_>> {
        let r = self.rows.get(i).filter(|_| i < self.len)?;
        Some(PickRow { label: &r.label, detail: &r.detail, art: r.art, heading: &r.heading })
    }
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
    /// The trailer after the last row (`trail`), scrolling with the rows: an action row such as "+ New list" that ends a list
    /// without being one of its rows (never selected, keyed or counted). Built only when the trailer has a height; a callback that
    /// builds nothing in it leaves blank space.
    Trailing,
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
    pub look: Option<&'a Look>,
    /// The inner padding in points (panel), fractions kept.
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
    /// How a modal is presented: centred, as a bottom sheet or full screen (modal).
    pub present: Presentation,
    /// The drag-to-dismiss thresholds of a bottom or full-screen modal (modal).
    pub dismiss: DismissOpts,
}

/// How `Ui::modal` presents its sheet, one of the `Overlay::present` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presentation {
    /// Centred in the content area, `Overlay::w` wide (the default).
    Centered,
    /// A sheet on the content area's bottom edge, `Overlay::w` wide (the area's whole width by default) and as tall as its content (to
    /// `Overlay::max_h`, else the area less 32 points). A downward drag dismisses it with the thresholds of `Overlay::dismiss`.
    Bottom,
    /// A sheet filling the content area. A downward drag dismisses it with the thresholds of `Overlay::dismiss`.
    Full,
    /// Centred across the content area, `Overlay::w` wide (320 points by default), its top edge at `Overlay::at`'s y below the content
    /// area's top (a command palette placed at a share of the window height computes that y itself); as tall as its content, to
    /// `Overlay::max_h` and never past 16 points above the content area's bottom.
    Top,
}

/// Drag-to-dismiss thresholds; a threshold left unset takes the UIKit sheet's (a quarter of the node height, 600 points/s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DismissOpts {
    /// The fraction of the node height a release past commits (`None`: a quarter).
    pub fraction: Option<f32>,
    /// The downward release speed in points per second that commits (`None`: 600).
    pub velocity: Option<f32>,
}

impl DismissOpts {
    /// The UIKit sheet's thresholds.
    pub const DEFAULT: DismissOpts = DismissOpts { fraction: None, velocity: None };
    /// The fraction of the node height a release past commits.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui_types::ui::DismissOpts::DEFAULT.fraction(0.25);
    /// ```
    pub const fn fraction(self, fraction: f32) -> DismissOpts { DismissOpts { fraction: Some(fraction), ..self } }
    /// The downward release speed in points per second that commits.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui_types::ui::DismissOpts::DEFAULT.velocity(600.0);
    /// ```
    pub const fn velocity(self, velocity: f32) -> DismissOpts { DismissOpts { velocity: Some(velocity), ..self } }
}
impl Default for DismissOpts {
    fn default() -> DismissOpts { DismissOpts::DEFAULT }
}

impl<'a> Overlay<'a> {
    /// The plain sheet of each call.
    pub const DEFAULT: Overlay<'a> = Overlay {
        at: (12.0, 12.0), w: None, h: None, max_h: None, stroke: None, look: None, padding: 0.0, window: false, dismiss_initial: false, attached: false,
        name: None, place: crate::layout::Placement::BELOW, present: Presentation::Centered, dismiss: DismissOpts::DEFAULT,
    };

    /// Places the sheet's top-left at (`x`, `y`) window points (panel, popup).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const MENU: pxui_types::ui::Overlay = pxui_types::ui::Overlay::DEFAULT.at(pxui_types::geom::Point::new(40.0, 60.0));
    /// assert_eq!(MENU.at, (40.0, 60.0));
    /// ```
    pub const fn at(mut self, at: crate::geom::Point) -> Overlay<'a> { self.at = (at.x, at.y); self }

    /// Sets the width in points (a panel is at least 180).
    /// Allocates: never.
    /// Panics: never (a width that is not finite or is negative is refused by the call).
    ///
    /// ```
    /// const WIDE: pxui_types::ui::Overlay = pxui_types::ui::Overlay::DEFAULT.w_px(320.0);
    /// assert_eq!(WIDE.w, Some(320.0));
    /// ```
    pub const fn w_px(mut self, w: f32) -> Overlay<'a> { self.w = Some(w); self }

    /// Sets a fixed height in points (panel, anchored), or a popup's height before it is first laid out.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const PANE: pxui_types::ui::Overlay = pxui_types::ui::Overlay::DEFAULT.h_px(200.0);
    /// assert_eq!(PANE.h, Some(200.0));
    /// ```
    pub const fn h_px(mut self, h: f32) -> Overlay<'a> { self.h = Some(h); self }

    /// Sets the tallest the sheet grows in points; past it the content scrolls (panel, popup).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const LIST: pxui_types::ui::Overlay = pxui_types::ui::Overlay::DEFAULT.max_h_px(400.0);
    /// assert_eq!(LIST.max_h, Some(400.0));
    /// ```
    pub const fn max_h_px(mut self, h: f32) -> Overlay<'a> { self.max_h = Some(h); self }

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
    /// const BARE: Overlay = Overlay::DEFAULT.look(&Look::NONE);
    /// assert!(BARE.look.is_some());
    /// ```
    pub const fn look(mut self, look: &'a Look) -> Overlay<'a> { self.look = Some(look); self }

    /// Sets the panel's inner padding in points, fractions kept.
    ///
    /// The panel call sends it in the size-prefixed panel record with an explicit presence flag, checked finite and nonnegative before
    /// the body starts; the 1.x `PxuiPanel` integer `padding` keeps its meaning for C callers.
    /// Allocates: never.
    /// Panics: never (a negative or non-finite padding is refused by the call).
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

    /// Sets how a modal is presented: centred (the default), as a bottom sheet or full screen (modal). A bottom or full-screen sheet is
    /// dismissed by a downward drag that the sheet takes through the engine's drag-to-dismiss, with the thresholds of `Overlay::dismiss`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::{Overlay, Presentation};
    /// const QUEUE: Overlay = Overlay::DEFAULT.present(Presentation::Bottom);
    /// assert_eq!(QUEUE.present, Presentation::Bottom);
    /// ```
    pub const fn present(mut self, presentation: Presentation) -> Overlay<'a> { self.present = presentation; self }

    /// Sets the drag-to-dismiss thresholds of a bottom or full-screen modal (a quarter of the sheet's height or 600 points/s by default).
    /// Allocates: never.
    /// Panics: never (a threshold that is not finite is refused by the call).
    ///
    /// ```
    /// use pxui_types::ui::{DismissOpts, Overlay, Presentation};
    /// const PLAYER: Overlay = Overlay::DEFAULT.present(Presentation::Full).dismiss(DismissOpts::DEFAULT.fraction(0.4));
    /// assert_eq!(PLAYER.dismiss.fraction, Some(0.4));
    /// ```
    pub const fn dismiss(mut self, dismiss: DismissOpts) -> Overlay<'a> { self.dismiss = dismiss; self }
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
    /// let off = pxui_types::ui::MenuItem::new("Paste").disabled(true);
    /// assert!(!off.enabled);
    /// ```
    pub const fn disabled(mut self, disabled: bool) -> MenuItem<'a> { self.enabled = !disabled; self }

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

/// One option of a pick-one control (`Ui::segmented`, `Ui::radio`, `Ui::dropdown`), a `Copy` value built with `const fn` builders from `Segment::new`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment<'a> {
    /// The text the segment shows; also its accessible name unless `name` is set.
    pub text: &'a str,
    /// Optional item id used for identity and returned as chosen_id.
    pub id: Option<i64>,
    /// Additional text for the option's body.
    pub detail: &'a str,
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
    pub const fn new(text: &'a str) -> Segment<'a> { Segment { text, id: None, detail: "", name: None } }

    /// Sets the accessible name, read in place of the text (what the option does, when the text alone is terse).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const FOUR: pxui_types::ui::Segment = pxui_types::ui::Segment::new("4 GB").name("Use 4 GB");
    /// assert_eq!(FOUR.name, Some("Use 4 GB"));
    /// ```
    pub const fn name(mut self, name: &'a str) -> Segment<'a> { self.name = Some(name); self }

    /// Assigns an explicit id to this item.
    ///
    /// The chosen result carries it unchanged.
    /// Allocates: never.
    /// Identity: Use ids unique among sibling items. The id keys the item's node, focus and secondary-press routing, so they follow it
    /// through a reorder. When one is absent, the item's position supplies identity in a namespace that never collides with an id.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui_types::ui::Segment::new("Dark").id(7);
    /// ```
    pub const fn id(self, id: i64) -> Segment<'a> { Segment { id: Some(id), ..self } }

    /// Sets additional text for the item's body.
    ///
    /// The string is borrowed for the build call.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui_types::ui::Segment::new("Dark").detail("Theme");
    /// ```
    pub const fn detail(self, detail: &'a str) -> Segment<'a> { Segment { detail, ..self } }
}

/// How `Ui::segmented` varies the kit's segmented control, as a `Copy` value built with `const fn` builders from `Segmented::DEFAULT`.
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
    /// const CHIP: States = States::of(Look::NONE);
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
    /// let s = pxui_types::ui::Segmented::DEFAULT.disabled(true);
    /// let _ = s;
    /// ```
    pub const fn disabled(mut self, disabled: bool) -> Segmented<'a> { self.enabled = !disabled; self }

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
    /// Panics: never (a negative or non-finite width panics at the `segmented` call).
    ///
    /// ```
    /// const STRIP: pxui_types::ui::Segmented = pxui_types::ui::Segmented::DEFAULT.w_px(240.0);
    /// let _ = STRIP;
    /// ```
    pub const fn w_px(mut self, w: f32) -> Segmented<'a> { self.w = Some(w); self }

    /// Sets a segment's text inset in points, and half the room around its text when segments are as wide as their text (8 by default).
    /// Allocates: never.
    /// Panics: never (a negative or non-finite padding panics at the `segmented` call).
    ///
    /// ```
    /// const ROOMY: pxui_types::ui::Segmented = pxui_types::ui::Segmented::DEFAULT.padding(12.0);
    /// let _ = ROOMY;
    /// ```
    pub const fn padding(mut self, padding: f32) -> Segmented<'a> { self.padding = padding; self }

    /// Sets the least width of a segment in points (none by default), whether it is as wide as its text or shares the strip.
    /// Allocates: never.
    /// Panics: never (a negative or non-finite width panics at the `segmented` call).
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
    /// let b = pxui_types::ui::DialogButton::new("Save").disabled(true);
    /// assert!(!b.enabled);
    /// ```
    pub const fn disabled(mut self, disabled: bool) -> DialogButton<'a> { self.enabled = !disabled; self }

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
    pub padding: f32,
}

impl Default for PanelOpts {
    fn default() -> PanelOpts {
        PanelOpts { stroke: None, look: None, window: false, x: 12.0, y: 12.0, width: 280.0, height: None, max_height: None, padding: 0.0 }
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
    /// assert_eq!(pxui_types::ui::PanelOpts::default().padding(8.0).padding, 8.0);
    /// ```
    pub const fn padding(mut self, padding: f32) -> PanelOpts {
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
/// Pass a fixed label or item id directly: `ui.node("toolbar", node)` or `ui.node(song.id, node)`.
/// Use [Key::Resolved] when a query receives an already resolved box key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A fixed label, hashed as the label of `key_of` under the parent's seed (a `###` suffix replaces it).
    Label(&'static str),
    /// An item id, hashed as the label of a NUL byte and its 16 lowercase hex digits under the parent's seed.
    Id(i64),
    /// An already resolved key such as Handle::box_key: used as it is, never hashed again, in lookups, hit queries and creation calls.
    /// 0 and 1 are never a box key: a key lookup of them finds nothing, and a call that takes the key as a label refuses them (as it
    /// refuses any invalid key), so they never alias an item id.
    Resolved(i64),
}
impl From<&'static str> for Key { #[inline] fn from(label: &'static str) -> Key { Key::Label(label) } }
impl From<i64> for Key { #[inline] fn from(id: i64) -> Key { Key::Id(id) } }
impl From<u64> for Key { #[inline] fn from(id: u64) -> Key { Key::Id(id as i64) } }

impl Key {
    /// The label form this key crosses the C ABI in: a `Label` itself; an `Id` written into `buf` as a NUL byte and its 16 lowercase
    /// hex digits (so an id never collides with a printable label); a `Resolved` key written as a 0x01 byte and its 16 lowercase hex
    /// digits, which the engine takes as the already hashed key itself (no seed, no rehash), so a resolved key is a valid identity in
    /// creation calls and lookups alike; the reserved `Resolved(0)` and `Resolved(1)` cross in the same form, which a call refuses as
    /// an invalid key. Authored labels that begin with a byte below 0x20 are reserved for these forms and the engine's
    /// own positional keys; a call refuses them.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::Key;
    /// let mut buf = [0u8; 17];
    /// assert_eq!(Key::Label("row").label(&mut buf), "row");
    /// assert_eq!(Key::Id(255).label(&mut buf), "\u{0}00000000000000ff");
    /// assert_eq!(Key::Resolved(255).label(&mut buf), "\u{1}00000000000000ff");
    /// assert_eq!(Key::Resolved(1).label(&mut buf), "\u{1}0000000000000001");
    /// ```
    #[inline]
    pub fn label<'b>(&self, buf: &'b mut [u8; 17]) -> &'b str {
        match *self {
            Key::Label(label) => label,
            Key::Id(id) => id_text(0, id, buf),
            Key::Resolved(key) => id_text(1, key, buf),
        }
    }
}

/// `id` as a label: the `tag` byte (0 an item id, 1 a resolved key) and its 16 lowercase hex digits, written into `out`. Out of line:
/// label keys inline without it. The digits are made eight at a time in a word (a list keys every row this way).
#[inline(never)]
fn id_text(tag: u8, id: i64, out: &mut [u8; 17]) -> &str {
    out[0] = tag;
    out[1..9].copy_from_slice(&hex8((id as u64 >> 32) as u32));
    out[9..].copy_from_slice(&hex8(id as u32));
    // a control byte and ASCII hex digits: always UTF-8
    core::str::from_utf8(out).unwrap_or_default()
}

/// `x`'s 8 lowercase hex digits, the most significant first.
#[inline]
fn hex8(x: u32) -> [u8; 8] {
    // one nibble a byte, the most significant nibble in the lowest byte
    let mut v = (x as u64).swap_bytes() >> 32;
    v = (v | v << 16) & 0x0000_ffff_0000_ffff;
    v = (v | v << 8) & 0x00ff_00ff_00ff_00ff;
    v = (v >> 4 & 0x000f_000f_000f_000f) | (v & 0x000f_000f_000f_000f) << 8;
    // '0' + n, and 'a' - '0' - 10 more where n > 9
    let letters = ((v + 0x0606_0606_0606_0606) >> 4) & 0x0101_0101_0101_0101;
    (v + 0x3030_3030_3030_3030 + letters * 0x27).to_le_bytes()
}

#[test]
fn id_text_writes_the_id_in_hex() {
    let mut x = 0x0123_4567_89ab_cdefu64;
    for id in [0i64, 1, -1, i64::MIN, i64::MAX, 0x0123_4567_89ab_cdef, 0xa, 0x9f].into_iter().chain((0..2000).map(|_| { x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); x as i64 })) {
        let mut out = [0u8; 17];
        let text = id_text(1, id, &mut out).to_owned();
        let mut expected = String::from("\u{1}");
        expected.push_str(&format!("{:016x}", id as u64));
        assert_eq!(text, expected, "{id:#x}");
    }
}

/// The key a `Key::Resolved` label carries (a 0x01 byte and 16 lowercase hex digits), or `None` for any other label and for the
/// reserved values 0 and 1.
#[inline]
fn resolved(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    if b.len() != 17 || b[0] != 1 { return None; }
    // 0 and 1 are the tables' empty and tombstone marks, never a box key: such a label is hashed as text, as a malformed one is
    b[1..].iter().try_fold(0u64, |key, &d| Some(key << 4 | match d { b'0'..=b'9' => d - b'0', b'a'..=b'f' => d - b'a' + 10, _ => return None } as u64)).filter(|k| *k > 1).map(|k| k as i64)
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
    // a byte scan: `str::contains(char)` goes through the pattern searcher, which costs a short label more than the scan
    if text.as_bytes().contains(&b'#') { text.find(marker) } else { None }
}

/// The key of the label `text` under the parent seed `seed`: the id after a `###` replaces the label's hash, otherwise all of `text`
/// is hashed (a `##` suffix is part of the key; `shown` is what strips it from the display).
///
/// Never 0 or 1 (a `Key::Resolved` label is the key it carries, taken as it is; one carrying 0 or 1 is hashed as text). Two labels
/// that differ only before `###` share a key.
/// Allocates: never.
/// Identity: `text` is the node label; the same text under the same seed collides.
/// Panics: never.
///
/// ```
/// use pxui_types::ui::key_of;
/// assert_eq!(key_of(1, "Play###a"), key_of(1, "Stop###a"));
/// assert_ne!(key_of(1, "Play##a"), key_of(1, "Play##b"));
/// // a resolved key's label is the key itself, under any seed
/// let mut buf = [0u8; 17];
/// assert_eq!(key_of(1, pxui_types::ui::Key::Resolved(-5).label(&mut buf)), -5);
/// // the reserved values are never returned, whatever the label
/// assert!(key_of(1, "\u{1}0000000000000000") > 1 && key_of(1, "\u{1}0000000000000001") > 1);
/// ```
pub fn key_of(seed: i64, text: &str) -> i64 {
    if let Some(key) = resolved(text) { return key; }
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
