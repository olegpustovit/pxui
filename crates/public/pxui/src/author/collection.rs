//! The collections keyed apart from their text: `ui.section_list` (fixed or measured rows, one call), `ui.grid_list`,
//! `ui.selection_list`, `ui.tree`, `ui.table` and `ui.table_head`. Each reports a small result and is built through its bodies
//! (`selection_list`/`tree`/`table_row` rows and `table_head` headings), so a registered body replaces its structure.
//! Selection, sort and column widths are app-owned state (`ListState`, `TableState`) passed as one `&mut`; selection semantics
//! (click, Command-toggle, Shift-range, double-click activate, keyboard) live in the widget, intents stay with the app.
use super::{Key, Response};
use crate::{collections::TreeNode, Ui};
use crate::Handle;
use pxui_types::ui::{ListResponse, SectionList, SectionPart, SelectionList, TreeResponse};

/// A selectable list's app-owned state (`Ui::selection_list`, and a table's rows through `TableState::selection`): one flag byte a
/// row and the anchor of a Shift-range. Keep it beside the data it describes and pass it every frame; the widget sizes `rows` to the
/// row count every frame (truncating when it shrinks, so rows that later return start unflagged; growing adds unflagged rows) and
/// updates it in place.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListState {
    /// One flag byte a row, by row index: `ListState::SELECTED` (the widget writes it on clicks, Command, Shift, Space and
    /// Command-A; the app may write it too, for a selection made elsewhere: a route chosen from a menu, a select-all button, a song
    /// revealed by a search, and the widget honours it next frame) and `ListState::DISABLED` (the app writes it: the row takes no
    /// selection, presses or drops and reads as disabled). Other bits are kept and ignored.
    pub rows: Vec<u8>,
    /// The row a Shift-click or Shift-arrow extends a run from (the last row clicked or moved to without Shift); `None` before any.
    pub anchor: Option<usize>,
}

impl ListState {
    /// The row is selected.
    pub const SELECTED: u8 = 1;
    /// The row is disabled.
    pub const DISABLED: u8 = 2;

    /// No rows, no anchor.
    /// Allocates: never (the first frame that sees rows sizes `rows` to them; later frames only when the count grows).
    /// Panics: never.
    ///
    /// ```
    /// let state = pxui::ListState::new();
    /// assert!(state.rows.is_empty() && state.anchor.is_none());
    /// ```
    pub const fn new() -> ListState { ListState { rows: Vec::new(), anchor: None } }
}

/// What a cell callback is told about the row it builds (`Ui::selection_list`, `Ui::table`), a `Copy` value.
#[derive(Clone, Copy, Debug, Default)]
pub struct CellState {
    /// The row is selected (its `ListState::SELECTED` flag).
    pub selected: bool,
    /// The row takes input (its `ListState::DISABLED` flag is clear and the list is not under `Ui::disabled(true, ..)`).
    pub enabled: bool,
    /// The row holds the list's keyboard focus.
    pub focused: bool,
    /// A press is held on the row (a drag may start: the app calls `Ui::carry(Some(&cell.handle), ..)` to carry the row's items).
    pub held: bool,
    /// The row's box (empty for a lead-in or heading): a per-row drop target (`Ui::drop_target(&cell.handle)`) or an anchor.
    pub handle: crate::Handle,
    /// The part's logical content Y (the lead-in 0), as `Ui::section_list` gives it.
    pub y: f32,
}

/// A sorted column's direction; no sort (`None` where a sort is optional) is the rows' original order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    /// Smallest first (a chevron up).
    Ascending,
    /// Largest first (a chevron down).
    Descending,
}

/// One column of `Ui::table` and `Ui::table_head`: its title, whether a click sorts by it, its width rule and its id, a `Copy`
/// value built with `const fn` builders from `Column::new(title)`.
///
/// A column is either point-wide (`w_px`, the default 100 points) or takes a `share` of the width the point-wide columns leave
/// (weighted like `Node::grow`). A point-wide column has a grip at its trailing edge by default (`grip(true)`: `w_px` is its starting
/// width, then the dragged one); without a grip `w_px` is its width every frame, so an app that computes responsive widths passes
/// them each frame. Keep every column in the slice every frame (its index is the cell callback's column): a column the layout hides
/// is `w_px(0.0)` (no heading, no cells built), never dropped from the slice. `TableState::widths` holds every column's laid-out
/// width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column<'a> {
    pub(crate) title: &'a str,
    pub(crate) sortable: bool,
    pub(crate) share: f32,
    pub(crate) w: f32,
    pub(crate) grip: bool,
    pub(crate) id: Option<i64>,
}

impl<'a> Column<'a> {
    /// A column titled `title` (its heading text and, in "Sort by Title, ascending", its accessible name), 100 points wide with a
    /// grip, not sortable, keyed by its index.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const TITLE: pxui::Column = pxui::Column::new("Title");
    /// ```
    pub const fn new(title: &'a str) -> Column<'a> { Column { title, sortable: false, share: 0.0, w: 100.0, grip: true, id: None } }

    /// Makes a click on the heading sort by this column (`true`): none, then ascending, then descending, then none again.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const TITLE: pxui::Column = pxui::Column::new("Title").sortable(true);
    /// ```
    pub const fn sortable(mut self, sortable: bool) -> Column<'a> { self.sortable = sortable; self }

    /// Makes the column take `share` of the width the point-wide columns leave (weights like `Node::grow`; positive and finite), so
    /// it follows the table's width and has no grip.
    /// Allocates: never.
    /// Panics: never (a share that is not positive and finite is refused by the call).
    ///
    /// ```
    /// const TITLE: pxui::Column = pxui::Column::new("Title").share(2.0);
    /// ```
    pub const fn share(mut self, share: f32) -> Column<'a> { self.share = share; self }

    /// Makes the column `points` wide: with a grip (the default) the starting width, kept until the grip is dragged (the call writes it
    /// into `TableState::widths` the first frame it sees the column's id); without one (`grip(false)`) its width every frame. 0 hides
    /// the column (no heading or cells are built; it keeps its index).
    /// Allocates: never.
    /// Panics: never (a width that is negative or not finite is refused by the call).
    ///
    /// ```
    /// const TIME: pxui::Column = pxui::Column::new("Time").w_px(56.0);
    /// ```
    pub const fn w_px(mut self, points: f32) -> Column<'a> { self.w = points; self.share = 0.0; self }

    /// Gives a point-wide column a grip at its trailing edge that resizes it (`true`, the default) or none (`false`: `w_px` is its
    /// width every frame, for widths the app computes from the table's width). A shared column never has a grip.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const TIME: pxui::Column = pxui::Column::new("Time").w_px(56.0).grip(false);
    /// ```
    pub const fn grip(mut self, grip: bool) -> Column<'a> { self.grip = grip; self }

    /// Keys the column by `id` (an app column id, stable across releases and column changes) instead of its index: its heading and
    /// cells, its dragged width (`TableState::widths`) and the sort (`TableState::sort`) follow the id, so a sort or a dragged width
    /// survives a column added before it. Ids must be unique among the columns; a column without one is keyed by its index.
    /// Allocates: never.
    /// Panics: never (a repeated id is refused by the call).
    ///
    /// ```
    /// const TITLE: pxui::Column = pxui::Column::new("Title").id(1);
    /// ```
    pub const fn id(mut self, id: i64) -> Column<'a> { self.id = Some(id); self }
}

/// A table's app-owned state (`Ui::table`, `Ui::table_head`): the columns' laid-out widths, the sort and the rows' selection, passed
/// as one `&mut` every frame. Widths and sort are keyed by column id (`Column::id`; the index for a column without one).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableState {
    /// Each column's `(id, laid-out width in points)`, in the order of the columns given that frame (entry `i` is `columns[i]`): the
    /// heading rewrites it every frame it is built (a gripped column keeps its dragged width, found by id; a grip-less one takes its
    /// `w_px`; shared columns split the rest), so rows built outside a `Ui::table` (a `Ui::selection_list` under a `Ui::table_head`)
    /// read `widths[i].1`. A column id seen for the first time starts at its `Column::w_px`.
    pub widths: Vec<(i64, f32)>,
    /// The sorted column's id and its direction; `None`: the original order. A heading click cycles a sortable column none,
    /// ascending, descending; the app sorts its rows to match (and may write it, for a sort chosen elsewhere).
    pub sort: Option<(i64, Sort)>,
    /// The rows' selection (`Ui::table`).
    pub selection: ListState,
}

impl TableState {
    /// No widths yet, unsorted, nothing selected.
    /// Allocates: never (the first frame sizes `widths` to the columns).
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui::TableState::new().sort.is_none());
    /// ```
    pub const fn new() -> TableState { TableState { widths: Vec::new(), sort: None, selection: ListState::new() } }
}

/// How `Ui::table` varies the kit's virtual table, as a `Copy` value built with `const fn` builders from `Table::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Table {
    pub(crate) multi: bool,
    pub(crate) row_h: f32,
    pub(crate) head: TableHead,
    pub(crate) variant: u16,
}

impl Table {
    /// One row selected at a time, 24-point rows, the default heading, the theme's row body.
    pub const DEFAULT: Table = Table { multi: false, row_h: 24.0, head: TableHead::DEFAULT, variant: 0 };

    /// Lets Command (Ctrl) toggle a row and Shift select a run, as `SelectionList::multi`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const FILES: pxui::Table = pxui::Table::DEFAULT.multi();
    /// ```
    pub const fn multi(mut self) -> Table { self.multi = true; self }

    /// Sets each row's height in points (positive and finite).
    /// Allocates: never.
    /// Panics: never (refused by the call otherwise).
    ///
    /// ```
    /// const FILES: pxui::Table = pxui::Table::DEFAULT.row_h(38.0);
    /// ```
    pub const fn row_h(mut self, points: f32) -> Table { self.row_h = points; self }

    /// Builds the table's heading as `head` does.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const FILES: pxui::Table = pxui::Table::DEFAULT.head(pxui::TableHead::DEFAULT.h_px(28.0));
    /// ```
    pub const fn head(mut self, head: TableHead) -> Table { self.head = head; self }

    /// Builds the rows with variant `variant` of the `table_row` body (0: the theme's).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const FILES: pxui::Table = pxui::Table::DEFAULT.variant(1);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Table { self.variant = variant; self }
}

/// How `Ui::table_head` (and a `Ui::table`'s heading) varies the kit's column heading, as a `Copy` value built with `const fn`
/// builders from `TableHead::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableHead {
    pub(crate) h: f32,
    pub(crate) variant: u16,
}

impl TableHead {
    /// A 24-point heading, the theme's `table_head` body.
    pub const DEFAULT: TableHead = TableHead { h: 24.0, variant: 0 };

    /// Sets the heading's height in points (positive and finite).
    /// Allocates: never.
    /// Panics: never (refused by the call otherwise).
    ///
    /// ```
    /// const HEAD: pxui::TableHead = pxui::TableHead::DEFAULT.h_px(28.0);
    /// ```
    pub const fn h_px(mut self, points: f32) -> TableHead { self.h = points; self }

    /// Builds the column titles with variant `variant` of the `table_head` body (0: the theme's).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const HEAD: pxui::TableHead = pxui::TableHead::DEFAULT.variant(1);
    /// ```
    pub const fn variant(mut self, variant: u16) -> TableHead { self.variant = variant; self }
}

/// What `Ui::table_head` reports this frame: a sort change and a width change apart.
#[derive(Clone, Copy, Debug)]
pub struct HeadResponse {
    /// The heading row's response (`changed`: `sorted` or `resized`).
    pub response: Response,
    /// A heading click changed `TableState::sort` this frame (read the new sort there; `None` is a return to the original order).
    pub sorted: bool,
    /// A grip drag changed a width in `TableState::widths` this frame (never set by a grip-less column's new `w_px`).
    pub resized: bool,
}

/// What `Ui::table` reports this frame: its rows' result and its heading's.
#[derive(Clone, Copy, Debug)]
pub struct TableResponse {
    /// The rows: activated, secondary, the row's box and the scroll box.
    pub rows: ListResponse,
    /// The heading: a sort or width change.
    pub head: HeadResponse,
}

/// Fixed logical cell dimensions for [Ui::grid_list].
///
/// The app builds every cell, so the grid has no body and this style has no variant.
#[derive(Clone, Copy, Debug)]
pub struct GridList {
    w: f32,
    h: f32,
}

impl GridList {
    /// Square, 64-point cells.
    pub const DEFAULT: GridList = GridList { w: 64.0, h: 64.0 };

    /// Sets the cell width in logical points.
    ///
    /// Grid construction checks that the dimension is positive and finite.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const GRID: pxui::GridList = pxui::GridList::DEFAULT.w_px(80.0);
    /// ```
    pub const fn w_px(self, points: f32) -> GridList { GridList { w: points, ..self } }

    /// Sets the cell height in logical points.
    ///
    /// Grid construction checks that the dimension is positive and finite.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const GRID: pxui::GridList = pxui::GridList::DEFAULT.h_px(80.0);
    /// ```
    pub const fn h_px(self, points: f32) -> GridList { GridList { h: points, ..self } }
}

impl Ui {
    /// Declares a scrolling lead-in, a sticky heading and virtual rows in one scroll region filling its parent, and reports the scroll
    /// box (`ListResponse::handle`) and, for a reorderable list (`SectionList::reorder`), a row moved (`moved`) or a carried payload
    /// dropped between rows (`drop_at`); `activated`, `secondary` and `row` stay `None` (the rows are app content and report their own
    /// presses).
    ///
    /// `build(ui, part, y)` builds the lead-in, the heading and each row in view (`y` is the part's logical content Y). Row `i`'s box is
    /// keyed by the item id `row_key(i)`, so a row's state follows its item when the items reorder (React's row key); ids must be unique
    /// among the rows (a repeated id takes a `#dup` key). With `SectionList::measured` each row takes the height it lays out to (the
    /// row height its estimate until built). `reveal(&returned_box, row)` scrolls a row into view (minimum scroll, heading respected).
    /// Allocates: never on a warm frame (a measured list's height cache grows with the count).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); row boxes are `Key::Id(row_key(i))`
    /// under the list's rows.
    /// Panics: when the library refuses the geometry (a row height that is not positive and finite, more than a million rows), or when
    /// `build` or `row_key` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::ui::{SectionList, SectionPart};
    /// let ids = [7i64, 3, 9];
    /// const SONGS: SectionList = SectionList::new(3).row_h(20.0).lead(40.0).overlay_scrollbar();
    /// pxui::doc::frames(2, &mut |ui| {
    ///     ui.section_list("songs", &SONGS, &|i| ids[i], &mut |ui, part, _y| if let SectionPart::Row(i) = part { ui.label_fmt(format_args!("{}", ids[i]), pxui::Label::BODY); });
    ///     ui.section_list("notes", &SectionList::new(3).measured(), &|i| i as i64, &mut |ui, part, _y| if let SectionPart::Row(_) = part { ui.label("note", pxui::Label::BODY.wrap()); });
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn section_list<K: Into<Key>>(&mut self, key: K, section: &SectionList, row_key: &dyn Fn(usize) -> i64, build: &mut dyn FnMut(&mut Ui, SectionPart, f32)) -> ListResponse {
        self.section_list_key(key.into(), section, row_key, build)
    }

    /// Declares a virtual grid of `count` cells sized by the GridList, filling its parent: as many columns as fit, rows
    /// scrolling; `build(ui, i)` builds cell `i` in view and the scroll box is returned.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); cells are keyed by index.
    /// Panics: when the library refuses the call (outside a frame, a cell size that is not positive and finite), or when `build` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| { ui.grid_list("covers", 100, &pxui::GridList::DEFAULT, &mut |ui, i| { ui.label_fmt(format_args!("{i}"), pxui::Label::BODY); }); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn grid_list<K: Into<Key>>(&mut self, key: K, count: usize, grid: &GridList, build: &mut dyn FnMut(&mut Ui, usize)) -> Handle {
        self.grid_list_key(key.into(), count, grid.w, grid.h, build)
    }

    /// Declares a selectable virtual list of `count` rows filling its parent, with the `SectionList` geometry the `SelectionList` gives
    /// (lead-in, sticky heading, measured rows, trailer): `cell(ui, part, cell)` builds the lead-in, the heading and each row in view
    /// inside the row node the `selection_list` body shapes (`item_text` for the kit's text row; any widgets otherwise), told the row's
    /// `CellState` (selected, enabled, focused, its box, its content Y).
    ///
    /// Selection lives in the widget and the app-owned `state`: a click selects a row alone and sets the anchor; with
    /// `SelectionList::multi` Command (Ctrl) toggles one and Shift selects the run from the anchor; arrows, Page keys, Home and End move
    /// the keyboard's row (Shift extending), Space toggles it, Command-A selects every enabled row. A double click or Enter reports
    /// `activated`, a secondary press (right click, Control-click, long press) `secondary`, both with the row's box in `row`. Rows the
    /// app flags `ListState::DISABLED` take no selection or presses and read as disabled. With `SelectionList::reorder` rows move as
    /// `SectionList::reorder` describes (`moved`, `drop_at`). One Tab stop.
    ///
    /// Row `i`'s box is keyed by the item id `row_key(i)`, so a row's state follows its item when the items reorder; ids must be unique
    /// among the rows (a repeated id takes a `#dup` key). `state` is by row index: when the app reorders its items it reorders `state`
    /// too (a list reorder already has).
    /// Allocates: never on a warm frame (`state.rows` grows when the count does; a measured list's height cache as `SectionList::measured`).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); rows are keyed by the item id `row_key(i)` (never the display text).
    /// Panics: when the library refuses the call (outside a frame, a row height that is not positive and finite, more than a million
    /// rows), or when `cell` or `row_key` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::{ListState, SelectionList, ui::SectionPart};
    /// let mut state = ListState::new();
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let r = ui.selection_list("files", 3, &|i| i as i64, &mut state, &SelectionList::DEFAULT.multi(), &mut |ui, part, _cell| {
    ///         if let SectionPart::Row(row) = part { ui.item_text(["a", "b", "c"][row]) }
    ///     });
    ///     assert_eq!(r.activated, None);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn selection_list<K: Into<Key>>(&mut self, key: K, count: usize, row_key: &dyn Fn(usize) -> i64, state: &mut ListState, list: &SelectionList, cell: &mut dyn FnMut(&mut Ui, SectionPart, CellState)) -> ListResponse {
        self.selection_list_key(key.into(), count, row_key, state, list, cell)
    }

    /// Declares a preorder virtual tree: `cell(ui, node)` builds the row that shows tree node `node`, after the disclosure mark
    /// (`item_text` for the kit's text row, which names the row); reports the node a double click or Enter activated and the rows shown.
    ///
    /// `open` holds one flag a node (not 0: expanded) and `selected` the selected node, both updated in place; the library writes the
    /// shown rows into the start of `visible` (at least a row a node).
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); rows are keyed by index.
    /// Panics: when the library refuses the call (outside a frame, `open` or `visible` too short), or when `cell` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::TreeNode;
    /// let nodes = [TreeNode { depth: 0, children: 1 }, TreeNode { depth: 1, children: 0 }];
    /// let (mut open, mut selected, mut visible) = ([1u8, 0], None, [0u32; 2]);
    /// pxui::doc::frames(1, &mut |ui| { assert_eq!(ui.tree("outline", &nodes, &mut open, &mut selected, &mut visible, &mut |ui, node| ui.item_text(["root", "leaf"][node])).shown, 2); });
    /// assert_eq!(visible, [0, 1]);
    /// ```
    #[inline]
    #[track_caller]
    pub fn tree<K: Into<Key>>(&mut self, key: K, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], cell: &mut dyn FnMut(&mut Ui, usize)) -> TreeResponse {
        self.tree_key(key.into(), nodes, open, selected, visible, cell)
    }

    /// Declares a virtual table filling its parent: a `Ui::table_head` of `columns` over `rows` rows; `cell(ui, row, column, cell)`
    /// builds one cell (`item_text` for the kit's text cell, which names the cell) `state.widths[column].1` wide, told the row's
    /// `CellState`. Selection, activation, secondary presses and disabled rows behave as `Ui::selection_list` describes, with
    /// `state.selection`; the heading sorts and resizes as `Ui::table_head` describes, with `state`. Reports both results.
    ///
    /// Row `i`'s box is keyed by the item id `row_key(i)` (unique among the rows), so a row's state follows its item when the rows are
    /// sorted; a cell is keyed by its column's `Column::id` (its index without one).
    /// Allocates: never on a warm frame (`state` grows when the row or column count does).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); rows are keyed by the item id `row_key(i)` (never the display text).
    /// Panics: when the library refuses the call (outside a frame, a row height that is not positive and finite, a column the heading
    /// refuses), or when `cell` or `row_key` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::{Column, Table, TableState};
    /// const COLUMNS: [Column; 2] = [Column::new("Name").sortable(true).share(1.0), Column::new("Tag").w_px(60.0)];
    /// let mut state = TableState::new();
    /// pxui::doc::frames(1, &mut |ui| {
    ///     ui.table("points", &COLUMNS, 2, &|i| i as i64, &mut state, &Table::DEFAULT, &mut |ui, row, column, _cell| ui.item_text(["a", "b", "c", "d"][row * 2 + column]));
    /// });
    /// ```
    #[allow(clippy::too_many_arguments)]
    #[inline]
    #[track_caller]
    pub fn table<K: Into<Key>>(&mut self, key: K, columns: &[Column<'_>], rows: usize, row_key: &dyn Fn(usize) -> i64, state: &mut TableState, table: &Table, cell: &mut dyn FnMut(&mut Ui, usize, usize, CellState)) -> TableResponse {
        self.table_key(key.into(), columns, rows, row_key, state, table, cell)
    }

    /// Declares a row of column headings `table_head`-body built, one for each of `columns`, laid out to the parent's width: point-wide
    /// columns at their width (a gripped one's dragged width from `state.widths` by id, else its `w_px`; a grip at its trailing edge
    /// resizes it), shared columns splitting the rest by their weights, `w_px(0.0)` columns not built; rewrites `state.widths` with every
    /// column's `(id, laid-out width)` in slice order, so rows built elsewhere (a `Ui::selection_list` with this heading as its
    /// `SectionPart::Header`) share them. A click on a sortable column's heading cycles `state.sort` (by column id): none, ascending,
    /// descending, none (the app sorts its rows); the heading shows the direction and is named "Sort by Title, ascending" (the text
    /// pxui forms, never per frame), role column header with the sort state.
    /// Allocates: never on a warm frame (`state.widths` grows when the column count does).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); a heading is keyed by its column's
    /// `Column::id` (its index without one).
    /// Panics: when the library refuses the call (outside a frame, a width that is negative or not finite, a share that is not positive
    /// and finite, a repeated column id; a sort naming no column reads as the original order). A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::{Column, Sort, TableHead, TableState};
    /// const COLUMNS: [Column; 2] = [Column::new("Title").sortable(true).share(1.0), Column::new("Time").sortable(true).w_px(56.0)];
    /// let mut state = TableState::new();
    /// pxui::doc::frames(1, &mut |ui| { ui.table_head("songs-head", &COLUMNS, &mut state, &TableHead::DEFAULT); });
    /// assert_eq!(state.widths.len(), 2);
    /// assert!(state.sort.is_none() || matches!(state.sort, Some((_, Sort::Ascending | Sort::Descending))));
    /// ```
    #[inline]
    #[track_caller]
    pub fn table_head<K: Into<Key>>(&mut self, key: K, columns: &[Column<'_>], state: &mut TableState, head: &TableHead) -> HeadResponse {
        self.table_head_key(key.into(), columns, state, head)
    }
}

#[cfg(test)]
mod tests {
    use crate::bodies::{SelectionListConfig, TableHeadConfig, TableRowConfig, TreeConfig};
    use crate::ui::{Node, Role, SectionList, SectionPart, Size};
    use crate::{testing, CellState, Column, Event, FrameIn, KeyCode, ListResponse, ListState, SelectionList, Sort, Table, TableHead, TableState, TreeNode, Ui};
    use std::cell::{Cell, RefCell};
    use crate::bodies::BodyConfig;
    use pxui_types::a11y::state;

    thread_local! { static SEEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) }; }
    fn seen(what: String) { SEEN.with(|s| s.borrow_mut().push(what)); }
    /// Each registered row body builds one marker node of its own, named by its kind, so the custom structure shows in the tree.
    fn marker(ui: &mut Ui, kind: &'static str) { let n = ui.node(kind, Node { w: Size::Px(4.0), h: Size::Px(4.0), role: Role::Image, ..Node::DEFAULT }); ui.a11y_name(&n, kind); }
    fn list_row(ui: &mut Ui, c: &SelectionListConfig) { seen(format!("list {}", c.index)); marker(ui, "custom list row"); ui.slot(c.content); }
    fn tree_row(ui: &mut Ui, c: &TreeConfig) { seen(format!("tree {}", c.node)); marker(ui, "custom tree row"); ui.slot(c.content); }
    fn table_row(ui: &mut Ui, c: &TableRowConfig) { seen(format!("table row {}", c.index)); marker(ui, "custom table row"); ui.slot(c.content); }
    fn head(ui: &mut Ui, c: &TableHeadConfig) { seen(format!("head {} {}{}", c.text_of(&c.title), c.has_ascending, c.ascending)); marker(ui, "custom head"); }

    /// The collections are built through their bodies: with a registered body for each kind, the custom structure is in the tree, the
    /// caller's cells run through the row bodies' slots, and a heading body is told its title and sort; a standalone heading too.
    #[test]
    fn each_collection_is_built_through_its_registered_bodies() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.set_a11y(true);
        ui.register_selection_list_body("c", list_row).unwrap();
        ui.register_tree_body("c", tree_row).unwrap();
        ui.register_table_row_body("c", table_row).unwrap();
        ui.register_table_head_body("c", head).unwrap();
        let text = "selection_list.body = c\ntree.body = c\ntable_row.body = c\ntable_head.body = c";
        ui.set_theme(&crate::ThemeHandle::parse(&text).unwrap()).unwrap();
        let nodes = [TreeNode { depth: 0, children: 0 }];
        let (mut list, mut open, mut visible, mut selected, mut table, mut heading) = (ListState::new(), [0u8; 1], [0u32; 1], None, TableState::new(), TableState::new());
        table.sort = Some((0, Sort::Descending));
        const COLUMNS: [Column; 1] = [Column::new("Name").sortable(true).share(1.0)];
        SEEN.with(|s| s.borrow_mut().clear());
        for _ in 0..2 {
            ui.frame(&FrameIn::new(400.0, 600.0), &mut |ui: &mut Ui| {
                for (key, kind) in [("lists", 0), ("trees", 1), ("tables", 2), ("heads", 3)] {
                    ui.id(key, |ui| { ui.column(Node { h: Size::Px(80.0), ..Node::DEFAULT }, |ui| match kind {
                        0 => { ui.selection_list("list", 1, &|i| i as i64, &mut list, &SelectionList::DEFAULT, &mut |ui, part, _| if let SectionPart::Row(_) = part { ui.item_text("list row") }); }
                        1 => { ui.tree("tree", &nodes, &mut open, &mut selected, &mut visible, &mut |ui, _| ui.item_text("tree row")); }
                        2 => { ui.table("table", &COLUMNS, 1, &|i| i as i64, &mut table, &Table::DEFAULT, &mut |ui, _, _, _| ui.item_text("table cell")); }
                        _ => { ui.table_head("head", &[Column::new("Alone")], &mut heading, &TableHead::DEFAULT); }
                    }); });
                }
            }).unwrap();
        }
        let got = SEEN.with(|s| s.borrow().clone());
        for want in ["list 0", "tree 0", "table row 0", "head Name 10", "head Alone 00"] {
            assert!(got.iter().any(|g| g == want), "{want} in {got:?}");
        }
        for kind in ["custom list row", "custom tree row", "custom table row", "custom head"] {
            assert!(testing::find(&ui, Role::Image, kind).is_some(), "the {kind} structure is used");
        }
        assert!(testing::find(&ui, Role::Cell, "table cell").is_some(), "the caller's cell runs in the custom row");
    }

    /// `selection_list` and `table` key row `i` by the item id `id(i)`: a node built in an item's row keeps its key when the items
    /// reorder, in the list (a row) and in the table (a cell).
    #[test]
    fn list_and_table_rows_are_keyed_by_item_id() {
        let (ids, keys) = (RefCell::new([10i64, 20, 30]), RefCell::new(([0i64; 3], [0i64; 3])));
        let (mut list, mut table) = (ListState::new(), TableState::new());
        let mut build = |ui: &mut Ui| {
            ui.column(Node { h: Size::Px(100.0), ..Node::DEFAULT }, |ui| {
                ui.selection_list("list", 3, &|i| ids.borrow()[i], &mut list, &SelectionList::DEFAULT, &mut |ui, part, _| if let SectionPart::Row(row) = part {
                    keys.borrow_mut().0[row] = ui.node("mark", Node { w: Size::Px(4.0), h: Size::Px(4.0), ..Node::DEFAULT }).box_key;
                });
            });
            ui.column(Node { h: Size::Px(100.0), ..Node::DEFAULT }, |ui| {
                ui.table("table", &[Column::new("A").w_px(60.0)], 3, &|i| ids.borrow()[i], &mut table, &Table::DEFAULT, &mut |ui, row, _, _| {
                    keys.borrow_mut().1[row] = ui.node("mark", Node { w: Size::Px(4.0), h: Size::Px(4.0), ..Node::DEFAULT }).box_key;
                });
            });
        };
        let mut ui = crate::doc::frames(2, &mut build);
        let before = *keys.borrow();
        ids.borrow_mut().reverse();
        testing::frame(&mut ui, &[], &mut build);
        let after = *keys.borrow();
        for (b, a) in [(before.0, after.0), (before.1, after.1)] {
            assert_eq!((a[0], a[2]), (b[2], b[0]), "each id's row keeps its key: {b:?} {a:?}");
            assert_ne!(b[0], b[1]);
        }
    }

    /// A frame at time `t` (seconds) with `events`; a frame's time keeps two clicks apart unless they are meant as a double click.
    fn at(ui: &mut Ui, t: f64, events: &[Event], build: &mut dyn FnMut(&mut Ui)) {
        let mut input = FrameIn::new(320.0, 240.0).time(t);
        (input.wire.events, input.wire.event_count) = (events.as_ptr(), events.len());
        ui.frame(&input, build).expect("frame");
    }
    fn key(kind: u32, k: KeyCode) -> Event { Event::key(kind, k.wire()) }
    /// A click at the centre of `rect` from time `t` with `mods` held (left button, or the right one with `secondary`).
    fn click_at(ui: &mut Ui, t: f64, rect: crate::geom::Rect, mods: &[KeyCode], secondary: bool, build: &mut dyn FnMut(&mut Ui)) {
        let (x, y) = ((rect.x + rect.w / 2.0) as f64, (rect.y + rect.h / 2.0) as f64);
        let mut down: Vec<Event> = mods.iter().map(|m| key(Event::KEY_PRESSED, *m)).collect();
        down.push(Event::pointer(Event::POINTER_PRESSED, secondary as u32, x, y));
        at(ui, t, &down, build);
        let mut up = vec![Event::pointer(Event::POINTER_RELEASED, secondary as u32, x, y)];
        up.extend(mods.iter().map(|m| key(Event::KEY_RELEASED, *m)));
        at(ui, t + 0.05, &up, build);
    }
    /// `k` pressed and released at time `t` with `mods` held.
    fn press(ui: &mut Ui, t: f64, k: KeyCode, mods: &[KeyCode], build: &mut dyn FnMut(&mut Ui)) {
        let mut events: Vec<Event> = mods.iter().map(|m| key(Event::KEY_PRESSED, *m)).collect();
        events.extend([key(Event::KEY_PRESSED, k), key(Event::KEY_RELEASED, k)]);
        events.extend(mods.iter().map(|m| key(Event::KEY_RELEASED, *m)));
        at(ui, t, &events, build);
    }
    /// Keeps a list's response when it reports something (a frame a click changes earlier rows in is built twice; the first build
    /// reports the click).
    fn keep(last: &Cell<ListResponse>, r: ListResponse) { if r.selected.is_some() || r.activated.is_some() || r.secondary.is_some() { last.set(r); } }
    fn items(ui: &Ui, role: Role) -> Vec<pxui_types::ffi::A11yNode> { ui.a11y_nodes().iter().filter(|n| n.is_role(role)).copied().collect() }
    fn on(state: &ListState) -> Vec<usize> { (0..state.rows.len()).filter(|i| state.rows[*i] & ListState::SELECTED != 0).collect() }

    /// The selection lives in the widget: a click selects a row alone, Command toggles one, Shift selects the run from the anchor; arrows
    /// select (Shift extending), Space toggles the keyboard's row, Command-A selects every enabled row, Enter and a double click activate,
    /// a right click reports `secondary`; `selected` names the row of every change the widget made; rows read as selectable list items
    /// with SELECTED on the selected ones, in both link modes.
    #[test]
    fn selection_list_selects_with_the_mouse_and_the_keyboard() {
        let state = RefCell::new(ListState::new());
        let last: Cell<ListResponse> = Cell::new(ListResponse::default());
        let mut build = |ui: &mut Ui| { keep(&last, ui.selection_list("files", 6, &|i| i as i64 * 7, &mut state.borrow_mut(), &SelectionList::DEFAULT.multi(), &mut |ui, part, _| if let SectionPart::Row(row) = part { ui.item_text(["a", "b", "c", "d", "e", "f"][row]) })); };
        let mut ui = crate::doc::frames(2, &mut build);
        let rows = items(&ui, Role::ListItem);
        assert_eq!(rows.len(), 6);
        assert!(rows.iter().all(|r| r.states & state::SELECTABLE != 0 && r.states & state::SELECTED == 0), "selectable rows, none selected");
        let mut t = 1.0;
        let mut click = |ui: &mut Ui, row: usize, mods: &[KeyCode], secondary: bool, build: &mut dyn FnMut(&mut Ui)| { t += 1.0; click_at(ui, t, rows[row].rect, mods, secondary, build); };
        last.set(ListResponse::default());
        click(&mut ui, 1, &[], false, &mut build);
        assert_eq!((on(&state.borrow()), state.borrow().anchor, last.get().selected), (vec![1], Some(1), Some(1)), "a click selects a row alone");
        last.set(ListResponse::default());
        click(&mut ui, 3, &[KeyCode::Meta], false, &mut build);
        assert_eq!((on(&state.borrow()), last.get().selected), (vec![1, 3], Some(3)), "Command adds a row");
        last.set(ListResponse::default());
        click(&mut ui, 1, &[KeyCode::Meta], false, &mut build);
        assert_eq!((on(&state.borrow()), last.get().selected), (vec![3], Some(1)), "Command toggles one off, and reports it");
        last.set(ListResponse::default());
        click(&mut ui, 5, &[KeyCode::Shift], false, &mut build);
        assert_eq!(on(&state.borrow()), vec![1, 2, 3, 4, 5], "Shift selects the run from the anchor (the row Command last toggled)");
        let rows_now = items(&ui, Role::ListItem);
        assert_eq!(rows_now.iter().filter(|r| r.states & state::SELECTED != 0).count(), 5, "the tree shows the selection");
        last.set(ListResponse::default());
        click(&mut ui, 2, &[], true, &mut build);
        assert_eq!((last.get().secondary, last.get().row.is_some()), (Some(2), true), "a right click reports the row");
        last.set(ListResponse::default());
        press(&mut ui, 20.0, KeyCode::ArrowUp, &[], &mut build);
        assert_eq!((on(&state.borrow()), last.get().selected), (vec![4], Some(4)), "Up selects the row before the keyboard's alone");
        last.set(ListResponse::default());
        press(&mut ui, 21.0, KeyCode::ArrowUp, &[KeyCode::Shift], &mut build);
        assert_eq!(on(&state.borrow()), vec![3, 4], "Shift-Up extends from the anchor");
        last.set(ListResponse::default());
        press(&mut ui, 22.0, KeyCode::Space, &[], &mut build);
        assert_eq!((on(&state.borrow()), last.get().selected), (vec![4], Some(3)), "Space toggles the keyboard's row");
        last.set(ListResponse::default());
        press(&mut ui, 23.0, KeyCode::Char('a'), &[KeyCode::Meta], &mut build);
        assert_eq!(on(&state.borrow()), vec![0, 1, 2, 3, 4, 5], "Command-A selects every row");
        last.set(ListResponse::default());
        press(&mut ui, 24.0, KeyCode::Enter, &[], &mut build);
        assert_eq!(last.get().activated, Some(3), "Enter activates the keyboard's row");
        last.set(ListResponse::default());
        at(&mut ui, 25.0, &[], &mut build);
        assert_eq!((last.get().activated, last.get().selected), (None, None), "a quiet frame reports nothing");
        let r = rows[0].rect;
        let (x, y) = ((r.x + 4.0) as f64, (r.y + 4.0) as f64);
        for (dt, kind) in [(0.0, Event::POINTER_PRESSED), (0.05, Event::POINTER_RELEASED), (0.1, Event::POINTER_PRESSED), (0.15, Event::POINTER_RELEASED)] {
            at(&mut ui, 30.0 + dt, &[Event::pointer(kind, 0, x, y)], &mut build);
            if last.get().activated.is_some() { break; }
        }
        assert_eq!(last.get().activated, Some(0), "a double click activates");
    }

    /// Rows the app flags DISABLED take no selection or presses and read as disabled; `open_on_click` makes a click activate while arrows
    /// only select; a lead-in, heading and trailer reach the cell builder as their `SectionPart`s.
    #[test]
    fn disabled_rows_open_on_click_and_the_section_parts() {
        let state = RefCell::new(ListState::new());
        state.borrow_mut().rows = vec![0, ListState::DISABLED, 0];
        let last: Cell<ListResponse> = Cell::new(ListResponse::default());
        let parts = Cell::new([false; 3]);
        const OPEN: SelectionList = SelectionList::DEFAULT.open_on_click(true).lead(10.0).header(20.0).trail(30.0);
        let mut build = |ui: &mut Ui| { keep(&last, ui.selection_list("nav", 3, &|i| i as i64, &mut state.borrow_mut(), &OPEN, &mut |ui, part, cell: CellState| {
            let mut p = parts.get();
            match part {
                SectionPart::Leading => p[0] = true,
                SectionPart::Header => p[1] = true,
                SectionPart::Trailing => p[2] = true,
                SectionPart::Row(row) => { assert_eq!(cell.enabled, row != 1); ui.item_text(["x", "y", "z"][row]) }
            }
            parts.set(p);
        })); };
        let mut ui = crate::doc::frames(2, &mut build);
        assert_eq!(parts.get(), [true; 3], "lead-in, heading and trailer are built");
        let rows = items(&ui, Role::ListItem);
        assert!(rows[1].states & state::ENABLED == 0 && rows[0].states & state::ENABLED != 0, "the disabled row reads as disabled");
        last.set(ListResponse::default());
        click_at(&mut ui, 1.0, rows[1].rect, &[], false, &mut build);
        assert_eq!((on(&state.borrow()), last.get().activated, last.get().selected), (vec![], None, None), "a disabled row takes no click");
        last.set(ListResponse::default());
        click_at(&mut ui, 2.0, rows[0].rect, &[], false, &mut build);
        assert_eq!((on(&state.borrow()), last.get().activated), (vec![0], Some(0)), "open_on_click: a click selects and activates");
        last.set(ListResponse::default());
        press(&mut ui, 3.0, KeyCode::ArrowDown, &[], &mut build);
        assert_eq!((on(&state.borrow()), last.get().activated, last.get().selected), (vec![2], None, Some(2)), "arrows only select, stepping over the disabled row");
        assert_eq!(state.borrow().rows[1] & ListState::SELECTED, 0, "the keyboard never selects a disabled row");
    }

    /// The table's heading cycles the sort by column id (none, ascending, descending, none); shared columns split the width the point
    /// columns leave by weight; a `w_px(0.0)` column is not built and lays out to 0; a grip drag widens its column and reports `resized`.
    #[test]
    fn table_sorts_lays_out_shares_hides_and_resizes() {
        let state = RefCell::new(TableState::new());
        const COLUMNS: [Column; 4] = [Column::new("Title").sortable(true).share(1.0).id(10), Column::new("Album").sortable(true).share(2.0).id(11),
            Column::new("Hidden").w_px(0.0).id(12), Column::new("Time").w_px(60.0).id(13)];
        let last = Cell::new(None);
        let mut build = |ui: &mut Ui| { last.set(Some(ui.table("songs", &COLUMNS, 4, &|i| i as i64, &mut state.borrow_mut(), &Table::DEFAULT.multi(), &mut |ui, row, column, _| ui.item_text(["a", "b", "c", "d"][(row + column) % 4])).head)); };
        let mut ui = crate::doc::frames(2, &mut build);
        let w = |state: &TableState, id: i64| state.widths.iter().find(|(c, _)| *c == id).map(|(_, w)| *w).unwrap();
        assert_eq!(state.borrow().widths.len(), 4);
        assert_eq!(w(&state.borrow(), 12), 0.0, "a hidden column lays out to 0");
        assert_eq!(w(&state.borrow(), 13), 60.0);
        let (title, album) = (w(&state.borrow(), 10), w(&state.borrow(), 11));
        assert!(title > 0.0 && (album - 2.0 * title).abs() <= 1.0, "shares split by weight: {title} {album}");
        assert!(testing::find(&ui, Role::ColumnHeader, "Hidden").is_none() && testing::find(&ui, Role::ColumnHeader, "Sort by Hidden").is_none(), "a hidden column has no heading");
        let mut t = 1.0;
        for want in [Some((10, Sort::Ascending)), Some((10, Sort::Descending)), None, Some((10, Sort::Ascending))] {
            let name = match state.borrow().sort { Some((10, Sort::Ascending)) => "Sort by Title, ascending", Some((10, Sort::Descending)) => "Sort by Title, descending", _ => "Sort by Title" };
            let rect = testing::find(&ui, Role::ColumnHeader, name).unwrap_or_else(|| panic!("{name}"));
            t += 1.0;
            click_at(&mut ui, t, rect, &[], false, &mut build);
            assert_eq!(state.borrow().sort, want, "the heading cycles the sort");
        }
        let time = testing::find(&ui, Role::ColumnHeader, "Time").expect("a non-sortable heading is named by its title");
        let grip = (time.x + time.w) as f64;
        let y = (time.y + time.h / 2.0) as f64;
        let mut resized = false;
        for (i, (kind, x)) in [(Event::POINTER_MOVED, grip), (Event::POINTER_PRESSED, grip), (Event::POINTER_MOVED, grip + 20.0), (Event::POINTER_RELEASED, grip + 20.0)].into_iter().enumerate() {
            at(&mut ui, 20.0 + i as f64 * 0.1, &[Event::pointer(kind, 0, x, y)], &mut build);
            resized |= last.get().unwrap().resized;
        }
        assert!(resized && (w(&state.borrow(), 13) - 80.0).abs() < 0.5, "the grip widens Time: {}", w(&state.borrow(), 13));
        assert!(!last.get().unwrap().sorted);
    }

    /// Warm frames of a selection list and a table allocate nothing in either link mode, including frames where the keyboard moves the
    /// selection and the sort changes its accessible name.
    #[test]
    fn warm_list_and_table_frames_allocate_nothing() {
        mem_sys::set_peer(crate::library_counter);
        let (mut list, mut table) = (ListState::new(), TableState::new());
        const COLUMNS: [Column; 2] = [Column::new("Name").sortable(true).share(1.0), Column::new("Size").w_px(50.0)];
        let mut build = |ui: &mut Ui| {
            ui.column(Node { h: Size::Px(100.0), ..Node::DEFAULT }, |ui| {
                ui.selection_list("list", 1000, &|i| i as i64, &mut list, &SelectionList::DEFAULT.multi().header(20.0), &mut |ui, part, _| if let SectionPart::Row(_) = part { ui.item_text("row") });
            });
            ui.column(Node { h: Size::Px(100.0), ..Node::DEFAULT }, |ui| {
                ui.table("table", &COLUMNS, 1000, &|i| i as i64, &mut table, &Table::DEFAULT, &mut |ui, _, _, _| ui.item_text("cell"));
            });
        };
        let mut ui = crate::doc::frames(2, &mut build);
        let first = items(&ui, Role::ListItem)[0].rect;
        click_at(&mut ui, 1.0, first, &[], false, &mut build);
        let head = testing::find(&ui, Role::ColumnHeader, "Sort by Name").unwrap();
        click_at(&mut ui, 2.0, head, &[], false, &mut build);
        click_at(&mut ui, 3.0, head, &[], false, &mut build);
        for i in 0..20 { press(&mut ui, 4.0 + i as f64, if i % 2 == 0 { KeyCode::ArrowDown } else { KeyCode::ArrowUp }, &[], &mut build); }
        let scope = mem_sys::warm_scope();
        let (down, up) = ([key(Event::KEY_PRESSED, KeyCode::ArrowDown), key(Event::KEY_RELEASED, KeyCode::ArrowDown)], [key(Event::KEY_PRESSED, KeyCode::ArrowUp), key(Event::KEY_RELEASED, KeyCode::ArrowUp)]);
        for i in 0..40 {
            mem_sys::local_reset();
            at(&mut ui, 30.0 + i as f64, if i % 2 == 0 { &down } else { &up }, &mut build);
            assert_eq!(mem_sys::local_alloc_count(), 0, "a warm keyboard frame allocated");
        }
        let rect = testing::find(&ui, Role::ColumnHeader, "Sort by Name, descending").unwrap();
        let (x, y) = ((rect.x + 4.0) as f64, (rect.y + 4.0) as f64);
        for (i, kind) in [Event::POINTER_PRESSED, Event::POINTER_RELEASED, Event::POINTER_PRESSED, Event::POINTER_RELEASED, Event::POINTER_PRESSED, Event::POINTER_RELEASED].into_iter().enumerate() {
            mem_sys::local_reset();
            at(&mut ui, 80.0 + i as f64, &[Event::pointer(kind, 0, x, y)], &mut build);
            assert_eq!(mem_sys::local_alloc_count(), 0, "a warm sort change allocated");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    /// A clickable 40-point source at the top of the window that carries a "song" payload while held.
    fn source(ui: &mut Ui) {
        let src = ui.node("source", Node { flags: crate::Flags::CLICKABLE, h: Size::Px(40.0), ..Node::DEFAULT });
        if ui.signal(&src).held { ui.carry(Some(&src), "song", "7"); }
    }
    /// Presses the source, drags to (`x`, `y`) over three frames and releases there, calling `seen` after each frame.
    fn drag_to(ui: &mut Ui, t: f64, x: f64, y: f64, build: &mut dyn FnMut(&mut Ui), seen: &mut dyn FnMut(&Ui, bool)) {
        let p = |kind, x, y| [Event::pointer(kind, 0, x, y)];
        at(ui, t, &p(Event::POINTER_MOVED, 20.0, 20.0), build);
        at(ui, t + 0.1, &p(Event::POINTER_PRESSED, 20.0, 20.0), build);
        for (i, (px, py)) in [(20.0, 30.0), (x, y), (x, y + 0.5), (x, y)].into_iter().enumerate() {
            at(ui, t + 0.2 + i as f64 * 0.1, &p(Event::POINTER_MOVED, px, py), build);
            seen(ui, false);
        }
        at(ui, t + 1.0, &p(Event::POINTER_RELEASED, x, y), build);
        seen(ui, true);
    }

    /// U2b.2: a node without input flags takes a drop: asking `drop_target` makes it a drop zone, so the pointer's hit over it (or
    /// over a child) reports a hover, then the drop in the release frame.
    #[test]
    fn a_plain_node_takes_a_drop() {
        let got = Cell::new((false, false));
        let mut build = |ui: &mut Ui| {
            source(ui);
            let target = ui.node("target", Node { h: Size::Px(100.0), ..Node::DEFAULT });
            ui.within(&target, &mut |ui| { ui.node("child", Node { w: Size::Px(20.0), h: Size::Px(20.0), ..Node::DEFAULT }); });
            match ui.drop_target(&target) {
                Some(crate::DropRef::Hover(p)) => { assert_eq!((p.kind, p.value), ("song", "7")); got.set((true, got.get().1)); }
                Some(crate::DropRef::Dropped(_)) => got.set((got.get().0, true)),
                None => {}
            }
        };
        let mut ui = crate::doc::frames(2, &mut build);
        drag_to(&mut ui, 1.0, 100.0, 90.0, &mut build, &mut |_, _| {});
        assert_eq!(got.get(), (true, true), "a plain node saw the hover and took the drop");
    }

    /// U2b.2: a reorderable `section_list` reports the insertion index a carried payload is dropped at (before a row over its upper
    /// half, after it over its lower half) and draws the insertion line meanwhile; a selection list with a drop look tells the row
    /// body `drop` 3 over a row and the app takes the drop on the row's box; a reorderable selection list tells the body 1 or 2.
    #[test]
    fn lists_report_where_a_payload_lands() {
        let dropped = Cell::new(None);
        let mut build = |ui: &mut Ui| {
            source(ui);
            let r = ui.section_list("queue", &SectionList::new(5).row_h(20.0).reorder(true), &|i| i as i64 * 10, &mut |ui, part, _| if let SectionPart::Row(_) = part { ui.node("cell", Node { h: Size::Px(20.0), ..Node::DEFAULT }); });
            if r.drop_at.is_some() { dropped.set(r.drop_at); }
        };
        let mut ui = crate::doc::frames(2, &mut build);
        // the list starts at y 40: row 2 spans 80..100, its lower half 90..100
        let mut counts = Vec::new();
        drag_to(&mut ui, 1.0, 50.0, 95.0, &mut build, &mut |ui, done| if !done { counts.push(ui.built_nodes()) });
        assert_eq!(dropped.get(), Some(3), "dropped over row 2's lower half: inserted at 3");
        // the first carried frame is not over a row yet; the hover frames build one node more, the insertion line
        assert!(counts[1..].iter().all(|n| *n == counts[0] + 1), "{counts:?}");
        dropped.set(None);
        drag_to(&mut ui, 3.0, 50.0, 82.0, &mut build, &mut |_, _| {});
        assert_eq!(dropped.get(), Some(2), "over row 2's upper half: inserted at 2");

        thread_local! { static DROPS: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) }; }
        fn drop_row(ui: &mut Ui, c: &SelectionListConfig) { DROPS.with(|d| d.borrow_mut().push(c.drop)); ui.slot(c.content); }
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.set_a11y(true);
        ui.register_selection_list_body("drops", drop_row).unwrap();
        ui.set_theme(&crate::ThemeHandle::parse("selection_list.body = drops").unwrap()).unwrap();
        let (state, took, mode) = (RefCell::new(ListState::new()), Cell::new(None), Cell::new(0));
        const LOOK: crate::theme::Look = crate::theme::Look::NONE.fill(crate::Rgba::rgb(0, 128, 255));
        let mut build = |ui: &mut Ui| {
            source(ui);
            let list = if mode.get() == 0 { SelectionList::DEFAULT.row_h(20.0).drop_look(LOOK) } else { SelectionList::DEFAULT.row_h(20.0).reorder(true) };
            let r = ui.selection_list("lists", 5, &|i| i as i64, &mut state.borrow_mut(), &list, &mut |ui, part, cell| if let SectionPart::Row(i) = part {
                ui.item_text("row");
                if let Some(crate::DropRef::Dropped(_)) = ui.drop_target(&cell.handle) { took.set(Some(i)); }
            });
            if r.drop_at.is_some() { took.set(r.drop_at.map(|i| i + 100)); }
        };
        for n in 0..2 { at(&mut ui, n as f64, &[], &mut build); }
        DROPS.with(|d| d.borrow_mut().clear());
        drag_to(&mut ui, 2.0, 50.0, 41.0 + 20.0 + 10.0, &mut build, &mut |_, _| {});
        assert_eq!(took.get(), Some(1), "the app took the drop on row 1's box");
        assert!(DROPS.with(|d| d.borrow().contains(&3)), "the row body was told the payload is over the row");
        mode.set(1);
        DROPS.with(|d| d.borrow_mut().clear());
        drag_to(&mut ui, 4.0, 50.0, 41.0 + 20.0 + 15.0, &mut build, &mut |_, _| {});
        assert_eq!(took.get(), Some(102), "a reorderable selection list reports the insertion index");
        assert!(DROPS.with(|d| d.borrow().contains(&2)), "the row body was told the payload lands after the row");
    }

    /// U2b.2: carrying a payload across a reorderable list and a drop-look selection list allocates nothing once warm (both link modes).
    #[test]
    fn a_warm_drag_over_lists_allocates_nothing() {
        mem_sys::set_peer(crate::library_counter);
        let state = RefCell::new(ListState::new());
        const LOOK: crate::theme::Look = crate::theme::Look::NONE.fill(crate::Rgba::rgb(0, 128, 255));
        let mut build = |ui: &mut Ui| {
            source(ui);
            ui.column(Node { h: Size::Px(100.0), ..Node::DEFAULT }, |ui| {
                ui.section_list("queue", &SectionList::new(500).row_h(20.0).reorder(true), &|i| i as i64, &mut |ui, part, _| if let SectionPart::Row(_) = part { ui.node("cell", Node { h: Size::Px(20.0), ..Node::DEFAULT }); });
            });
            ui.column(Node { h: Size::Px(100.0), ..Node::DEFAULT }, |ui| {
                ui.selection_list("lists", 500, &|i| i as i64, &mut state.borrow_mut(), &SelectionList::DEFAULT.row_h(20.0).drop_look(LOOK), &mut |ui, part, cell| if let SectionPart::Row(_) = part {
                    ui.item_text("row");
                    let _ = ui.drop_target(&cell.handle);
                });
            });
        };
        let mut ui = crate::doc::frames(2, &mut build);
        let p = |kind, y| [Event::pointer(kind, 0, 50.0, y)];
        at(&mut ui, 1.0, &p(Event::POINTER_MOVED, 20.0), &mut build);
        at(&mut ui, 1.1, &p(Event::POINTER_PRESSED, 20.0), &mut build);
        let y = |i: usize| 45.0 + (i * 7 % 190) as f64;
        for i in 0..40 { at(&mut ui, 1.2 + i as f64 * 0.02, &p(Event::POINTER_MOVED, y(i)), &mut build); }
        let scope = mem_sys::warm_scope();
        for i in 40..80 {
            mem_sys::local_reset();
            at(&mut ui, 1.2 + i as f64 * 0.02, &p(Event::POINTER_MOVED, y(i)), &mut build);
            assert_eq!(mem_sys::local_alloc_count(), 0, "a warm drag frame over the lists allocated");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    /// One `section_list` serves fixed and measured rows: a fixed row's `y` is the lead-in, heading and rows above it, and a measured
    /// row takes the height it lays out to.
    #[test]
    fn section_list_serves_fixed_and_measured_rows() {
        let ys = RefCell::new(Vec::new());
        let tall = Cell::new(None);
        let mut build = |ui: &mut Ui| {
            ys.borrow_mut().clear();
            ui.column(Node { h: Size::Px(200.0), ..Node::DEFAULT }, |ui| {
                ui.section_list("fixed", &SectionList::new(3).row_h(20.0).lead(30.0).header(10.0), &|i| i as i64, &mut |ui, part, y| {
                    if let SectionPart::Row(i) = part { ys.borrow_mut().push((i, y)); ui.node("r", Node { h: Size::Grow, ..Node::DEFAULT }); }
                });
            });
            ui.column(Node { h: Size::Px(200.0), ..Node::DEFAULT }, |ui| {
                ui.section_list("measured", &SectionList::new(1).row_h(20.0).measured(), &|i| i as i64, &mut |ui, part, _| {
                    if let SectionPart::Row(_) = part { tall.set(Some(ui.node("tall", Node { h: Size::Px(50.0), ..Node::DEFAULT }))); }
                });
            });
        };
        let mut ui = crate::doc::frames(2, &mut build);
        testing::frame(&mut ui, &[], &mut build);
        assert_eq!(ys.borrow()[..3], [(0, 40.0), (1, 60.0), (2, 80.0)]);
        assert_eq!(ui.rect(&tall.get().unwrap()).h, 50.0, "a measured row keeps its own height");
    }
}
