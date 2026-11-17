//! The collections keyed apart from their text: `ui.section_list` (fixed or measured rows, one call), `ui.grid_list`,
//! `ui.selection_list`, `ui.tree`, `ui.table_view` and `ui.table`. Each reports a small result and is built through its bodies
//! (`list_view`/`tree`/`table_view`/`table_head` rows and cells, `table`), so a registered body replaces its structure.
use super::Key;
use crate::{collections::TreeNode, PxuiBox, Ui};
use pxui_types::{ui::{ListResponse, SectionList, SectionPart, SelectionList, TableResponse, TableView, TreeResponse}};

impl Ui {
    /// Declares a scrolling lead-in, a sticky heading and virtual rows in one scroll region filling its parent, and returns the scroll box.
    ///
    /// `build(ui, part, y)` builds the lead-in, the heading and each row in view (`y` is the part's logical content Y). Row `i`'s box is
    /// keyed by the item id `id(i)`, so a row's state follows its item when the items reorder (React's row key); ids must be unique
    /// among the rows (a repeated id takes a `#dup` key). With `SectionList::measured` each row takes the height it lays out to (the
    /// row height its estimate until built). `reveal(&returned_box, row)` scrolls a row into view (minimum scroll, heading respected).
    /// Allocates: never on a warm frame (a measured list's height cache grows with the count).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); row boxes are `Key::Id(id(i))` under
    /// the list's rows.
    /// Panics: when the library refuses the geometry (a row height that is not positive and finite, more than a million rows), or when
    /// `build` or `id` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
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
    pub fn section_list<K: Into<Key>>(&mut self, key: K, section: &SectionList, id: &dyn Fn(usize) -> i64, build: &mut dyn FnMut(&mut Ui, SectionPart, f32)) -> PxuiBox {
        self.section_list_key(key.into(), section, id, build)
    }

    /// Declares a virtual grid of `count` cells, each `cell_w` by `cell_h` points, filling its parent: as many columns as fit, rows
    /// scrolling; `build(ui, i)` builds cell `i` in view and the scroll box is returned.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); cells are keyed by index.
    /// Panics: when the library refuses the call (outside a frame, a cell size that is not positive and finite), or when `build` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| { ui.grid_list("covers", 100, 64.0, 64.0, &mut |ui, i| { ui.label_fmt(format_args!("{i}"), pxui::Label::BODY); }); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn grid_list<K: Into<Key>>(&mut self, key: K, count: usize, cell_w: f32, cell_h: f32, build: &mut dyn FnMut(&mut Ui, usize)) -> PxuiBox {
        self.grid_list_key(key.into(), count, cell_w, cell_h, build)
    }

    /// Declares a selectable virtual list of `count` rows filling its parent: `cell(ui, row)` builds each row in view inside the row
    /// node the `list_view` body shapes (`item_text` for the kit's text row; any widgets otherwise); reports the row a double click or
    /// Enter activated.
    ///
    /// `selected` holds one flag a row (not 0: selected), updated in place: a click selects a row alone; with `SelectionList::multi`,
    /// Command (Ctrl) toggles one and Shift selects a run. One Tab stop; arrows, Page keys, Home and End move the keyboard's row.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); rows are keyed by index.
    /// Panics: when the library refuses the call (outside a frame, `selected` shorter than `count`), or when `cell` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::SelectionList;
    /// let mut selected = [0u8; 3];
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let r = ui.selection_list("files", 3, &mut selected, &SelectionList::DEFAULT.multi(), &mut |ui, row| ui.item_text(["a", "b", "c"][row]));
    ///     assert_eq!(r.activated, None);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn selection_list<K: Into<Key>>(&mut self, key: K, count: usize, selected: &mut [u8], list: &SelectionList, cell: &mut dyn FnMut(&mut Ui, usize)) -> ListResponse {
        self.selection_list_key(key.into(), count, selected, list, cell)
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
    /// use pxui::collections::TreeNode;
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

    /// Declares a virtual table filling its parent: a header of `titles` over `rows` rows, columns `widths` points wide; `cell(ui, row,
    /// column)` builds one cell (`item_text` for the kit's text cell, which names the cell). Reports the column whose header was clicked
    /// (the sort intent). With `TableView::text(rect)` it is instead the fixed text table at `rect`: a 24-point header, hairlines and
    /// right-aligned cells whose text `cell` gives `item_text` (widths, selection and sort do not apply), reporting the clicked row.
    ///
    /// `widths` and `selected` update in place (dragging a header grip resizes its column; a click selects a row); `TableView::sort`
    /// marks the sorted column with a chevron (up when ascending), the app sorts its rows.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); rows are keyed by index.
    /// Panics: when the library refuses the call (outside a frame, `widths` not one a title, a sort column past them), or when `cell`
    /// panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::TableView;
    /// let (mut widths, mut selected) = ([80.0f32, 60.0], None);
    /// pxui::doc::frames(1, &mut |ui| {
    ///     ui.table_view("points", &["Name", "Tag"], &mut widths, 2, &mut selected, &TableView::DEFAULT.sort(0, true), &mut |ui, row, column| ui.item_text(["a", "b", "c", "d"][row * 2 + column]));
    /// });
    /// ```
    #[allow(clippy::too_many_arguments)]
    #[inline]
    #[track_caller]
    pub fn table_view<K: Into<Key>>(&mut self, key: K, titles: &[&str], widths: &mut [f32], rows: usize, selected: &mut Option<usize>, table: &TableView, cell: &mut dyn FnMut(&mut Ui, usize, usize)) -> TableResponse {
        self.table_view_key(key.into(), titles, widths, rows, selected, table, cell)
    }
}

#[cfg(test)]
mod tests {
    use crate::bodies::{text_of, PxuiRowConfig, PxuiTableCellConfig, PxuiTableHeadConfig, PxuiTreeRowConfig};
    use crate::ui::{Node, Rect, Role, SectionList, SectionPart, Size};
    use crate::{testing, FrameIn, SelectionList, TableView, TreeNode, Ui};
    use std::cell::{Cell, RefCell};

    thread_local! { static SEEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) }; }
    fn seen(what: String) { SEEN.with(|s| s.borrow_mut().push(what)); }
    /// Each registered row body builds one marker node of its own, named by its kind, so the custom structure shows in the tree.
    fn marker(ui: &mut Ui, kind: &'static str) { let n = ui.node(kind.into(), Node { w: Size::Px(4.0), h: Size::Px(4.0), role: Role::Image, ..Node::DEFAULT }); ui.a11y_name(&n, kind); }
    fn list_row(ui: &mut Ui, c: &PxuiRowConfig) { seen(format!("list {}", c.index)); marker(ui, "custom list row"); ui.slot(c.content); }
    fn tree_row(ui: &mut Ui, c: &PxuiTreeRowConfig) { seen(format!("tree {}", c.node)); marker(ui, "custom tree row"); ui.slot(c.content); }
    fn table_row(ui: &mut Ui, c: &PxuiRowConfig) { seen(format!("table row {}", c.index)); marker(ui, "custom table row"); ui.slot(c.content); }
    fn head(ui: &mut Ui, c: &PxuiTableHeadConfig) { seen(format!("head {}", text_of(&c.title))); marker(ui, "custom head"); }
    fn cell(_: &mut Ui, c: &PxuiTableCellConfig) { if c.header == 0 { seen(format!("cell {}:{} {}", c.row, c.column, text_of(&c.text))); } }

    /// The collections are built through their bodies: with a registered body for each kind, the custom structure is in the tree, the
    /// caller's cells run through the row bodies' slots, and the text table's cells are given the text the caller wrote.
    #[test]
    fn each_collection_is_built_through_its_registered_bodies() {
        let mut ui = Ui::new().expect("library");
        ui.set_a11y(true);
        ui.register_list_view_body("c", list_row).unwrap();
        ui.register_tree_body("c", tree_row).unwrap();
        ui.register_table_view_body("c", table_row).unwrap();
        ui.register_table_head_body("c", head).unwrap();
        ui.register_table_body("c", cell).unwrap();
        let text = "list_view.body = c\ntree.body = c\ntable_view.body = c\ntable_head.body = c\ntable.body = c";
        ui.set_theme(&pxui_types::ffi::ThemeSource { size: core::mem::size_of::<pxui_types::ffi::ThemeSource>() as u32, flags: 0, source: pxui_types::Str { ptr: text.as_ptr(), len: text.len() } }).unwrap();
        let nodes = [TreeNode { depth: 0, children: 0 }];
        let (mut list, mut open, mut visible, mut selected, mut table_sel, mut widths) = ([0u8; 1], [0u8; 1], [0u32; 1], None, None, [60.0f32]);
        SEEN.with(|s| s.borrow_mut().clear());
        for _ in 0..2 {
            ui.frame(&FrameIn { width: 400.0, height: 600.0, ..FrameIn::default() }, &mut |ui: &mut Ui| {
                for (key, kind) in [("lists", 0), ("trees", 1), ("tables", 2)] {
                    ui.column(key, Node { h: Size::Px(80.0), ..Node::DEFAULT }, |ui| match kind {
                        0 => { ui.selection_list("list", 1, &mut list, &SelectionList::DEFAULT, &mut |ui, _| ui.item_text("list row")); }
                        1 => { ui.tree("tree", &nodes, &mut open, &mut selected, &mut visible, &mut |ui, _| ui.item_text("tree row")); }
                        _ => { ui.table_view("table", &["Name"], &mut widths, 1, &mut table_sel, &TableView::DEFAULT, &mut |ui, _, _| ui.item_text("table cell")); }
                    });
                }
                ui.table_view("text", &["A"], &mut [], 2, &mut None, &TableView::text(Rect::new(0.0, 300.0, 200.0, 100.0)), &mut |ui, row, _| ui.item_text(["r0", "r1"][row]));
            }).unwrap();
        }
        let got = SEEN.with(|s| s.borrow().clone());
        for want in ["list 0", "tree 0", "table row 0", "head Name", "cell 0:0 r0", "cell 1:0 r1"] {
            assert!(got.iter().any(|g| g == want), "{want} in {got:?}");
        }
        for kind in ["custom list row", "custom tree row", "custom table row", "custom head"] {
            assert!(testing::find(&ui, Role::Image, kind).is_some(), "the {kind} structure is used");
        }
    }

    /// One `section_list` serves fixed and measured rows: a fixed row's `y` is the lead-in, heading and rows above it, and a measured
    /// row takes the height it lays out to.
    #[test]
    fn section_list_serves_fixed_and_measured_rows() {
        let ys = RefCell::new(Vec::new());
        let tall = Cell::new(None);
        let mut build = |ui: &mut Ui| {
            ys.borrow_mut().clear();
            ui.column("a", Node { h: Size::Px(200.0), ..Node::DEFAULT }, |ui| {
                ui.section_list("fixed", &SectionList::new(3).row_h(20.0).lead(30.0).header(10.0), &|i| i as i64, &mut |ui, part, y| {
                    if let SectionPart::Row(i) = part { ys.borrow_mut().push((i, y)); ui.node("r".into(), Node { h: Size::Grow, ..Node::DEFAULT }); }
                });
            });
            ui.column("b", Node { h: Size::Px(200.0), ..Node::DEFAULT }, |ui| {
                ui.section_list("measured", &SectionList::new(1).row_h(20.0).measured(), &|i| i as i64, &mut |ui, part, _| {
                    if let SectionPart::Row(_) = part { tall.set(Some(ui.node("tall".into(), Node { h: Size::Px(50.0), ..Node::DEFAULT }))); }
                });
            });
        };
        let mut ui = crate::doc::frames(2, &mut build);
        testing::frame(&mut ui, &[], &mut build);
        assert_eq!(ys.borrow()[..3], [(0, 40.0), (1, 60.0), (2, 80.0)]);
        assert_eq!(ui.rect(&tall.get().unwrap()).h, 50.0, "a measured row keeps its own height");
    }
}
