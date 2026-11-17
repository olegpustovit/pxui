//! Containers that declare a node with its axis set and build their children inside it: `ui.row`, `ui.column`, `ui.stack`.
use crate::Handle;
use pxui_types::Node;
use pxui_types::node::Axis;
use pxui_types::ui::{SectionList, SectionPart};
use crate::Ui;
pub use pxui_types::ui::Key;

impl Ui {
    /// Declares `node` laid out as a row under the current parent and runs `f` to build its children inside it.
    ///
    /// `node` is used as given with `axis` set to `Axis::Row`; the returned handle is the row's, valid this frame (style it,
    /// read its signal). Same as `node` then `within`.
    /// Allocates: as `node` and `within` (never on a warm frame).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); an id never collides with a
    /// printable label.
    /// Panics: when the library refuses the node (outside a frame, an invalid transition curve), or when `f` panics. A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// use pxui::ui::{Node, Size};
    /// let (mut a, mut b) = (None, None);
    /// let ui = pxui::doc::frames(2, &mut |ui| {
    ///     ui.row("toolbar", Node { h: Size::Px(24.0), ..Node::DEFAULT }, |ui| {
    ///         a = Some(ui.node("a".into(), Node { w: Size::Px(10.0), ..Node::DEFAULT }));
    ///         b = Some(ui.node("b".into(), Node { w: Size::Px(10.0), ..Node::DEFAULT }));
    ///     });
    /// });
    /// assert_eq!(ui.rect(&b.unwrap()).x, ui.rect(&a.unwrap()).x + 10.0);
    /// ```
    #[inline]
    #[track_caller]
    pub fn row<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, mut f: F) -> Handle { self.container(key.into(), Axis::Row, node, &mut f) }

    /// Declares `node` laid out as a column under the current parent and runs `f` to build its children inside it.
    ///
    /// `node` is used as given with `axis` set to `Axis::Column`; the returned handle is the column's, valid this frame. Same as
    /// `node` then `within`.
    /// Allocates: as `node` and `within` (never on a warm frame).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); an id never collides with a
    /// printable label.
    /// Panics: when the library refuses the node (outside a frame, an invalid transition curve), or when `f` panics. A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// use pxui::ui::{Node, Size};
    /// let songs: [u64; 2] = [41, 42];
    /// let mut rows = Vec::new();
    /// let ui = pxui::doc::frames(2, &mut |ui| {
    ///     rows.clear();
    ///     ui.column("songs", Node::DEFAULT, |ui| {
    ///         for id in songs { rows.push(ui.row(id, Node { h: Size::Px(24.0), ..Node::DEFAULT }, |_| {})); }
    ///     });
    /// });
    /// assert_eq!(ui.rect(&rows[1]).y, ui.rect(&rows[0]).y + 24.0);
    /// ```
    #[inline]
    #[track_caller]
    pub fn column<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, mut f: F) -> Handle { self.container(key.into(), Axis::Column, node, &mut f) }

    /// Declares `node` as a stack under the current parent and runs `f` to build its children inside it, overlapping.
    ///
    /// `node` is used as given with `axis` set to `Axis::Stack`: children sit at the padding origin in `z` order (equal `z`: build
    /// order). The returned handle is the stack's, valid this frame. Same as `node` then `within`.
    /// Allocates: as `node` and `within` (never on a warm frame).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); an id never collides with a
    /// printable label.
    /// Panics: when the library refuses the node (outside a frame, an invalid transition curve), or when `f` panics. A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// use pxui::ui::{Node, Size};
    /// let (mut a, mut b) = (None, None);
    /// let ui = pxui::doc::frames(2, &mut |ui| {
    ///     ui.stack("badge", Node::DEFAULT, |ui| {
    ///         a = Some(ui.node("art".into(), Node { w: Size::Px(40.0), h: Size::Px(40.0), ..Node::DEFAULT }));
    ///         b = Some(ui.node("dot".into(), Node { w: Size::Px(8.0), h: Size::Px(8.0), ..Node::DEFAULT }));
    ///     });
    /// });
    /// assert_eq!((ui.rect(&a.unwrap()).x, ui.rect(&a.unwrap()).y), (ui.rect(&b.unwrap()).x, ui.rect(&b.unwrap()).y));
    /// ```
    #[inline]
    #[track_caller]
    pub fn stack<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, mut f: F) -> Handle { self.container(key.into(), Axis::Stack, node, &mut f) }

    /// Declares a virtual list of `count` rows `row_h` points high that fills its parent; `f(ui, i)` builds row `i` inside its row box,
    /// which is keyed by the item id `row_key(i)`.
    ///
    /// The fixed-height list of app-built rows. Only the rows in view (and a few past each edge) are built, so the list costs the same
    /// at ten rows or a million. Like React's row key, `row_key(i)` is the item's own id (never its index or text): a row's state and
    /// its children's identities follow the item when the items reorder; the rows are the app's own structure, so `list` has no body
    /// kind. The returned handle is the list's scroll box (`reveal(&list, i)` scrolls row `i` into view). The other list jobs:
    /// `section_list` (a lead-in, a sticky heading, rows of differing heights with `SectionList::measured`), `grid_list` (cells in
    /// columns), `selection_list` (kit rows that keep a selection), `tree` and `table_view`.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); each row box is
    /// `Key::Id(row_key(i))` under the list, so ids must be unique among the rows (a repeated id takes a `#dup` key).
    /// Panics: when the library refuses the list (outside a frame, `row_h` not positive and finite, more than a million rows), or when
    /// `row_key` or `f` panics. A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// use std::cell::Cell;
    /// let songs: Vec<u64> = (1000..101_000).collect();
    /// let built = Cell::new(0);
    /// pxui::doc::frames(2, &mut |ui| {
    ///     ui.list("songs", songs.len(), 44.0, &|i| songs[i] as i64, |ui, i| { built.set(built.get() + 1); ui.label_fmt(format_args!("Song {}", songs[i]), pxui::Label::BODY); });
    /// });
    /// assert!(built.get() < 200, "only the rows in view are built");
    /// ```
    #[inline]
    #[track_caller]
    pub fn list<K: Into<Key>, F: FnMut(&mut Ui, usize)>(&mut self, key: K, count: usize, row_h: f32, row_key: &dyn Fn(usize) -> i64, mut f: F) -> Handle { self.list_rows(key.into(), count, row_h, row_key, &mut f) }

    /// The one body of `list`: a section list with no lead-in, heading or trailer, its rows handed to `f`.
    #[track_caller]
    fn list_rows(&mut self, key: Key, count: usize, row_h: f32, row_key: &dyn Fn(usize) -> i64, f: &mut dyn FnMut(&mut Ui, usize)) -> Handle {
        self.section_list_key(key, &SectionList::new(count).row_h(row_h), row_key, &mut |ui, part, _| if let SectionPart::Row(i) = part { f(ui, i) })
    }

    /// The one body of `row`, `column` and `stack`: `node` with `axis` set, declared by `key`, then `f` inside it.
    // Inline: an out-of-line body retires about 110 instructions more per container than `node` + `within` (2.2% on the abi_bench
    // Author page, over its 1% limit), because the node checks and the frame-entry loads no longer fold at the call site.
    #[inline]
    #[track_caller]
    fn container(&mut self, key: Key, axis: Axis, mut node: Node, f: &mut dyn FnMut(&mut Ui)) -> Handle {
        node.axis = axis;
        let b = self.node(key.into(), node);
        self.within(&b, f);
        b
    }
}

#[cfg(test)]
mod tests {
    use super::Key;
    use crate::ui::{Node, Size};

    /// An id key is scoped by its parent like a label: the same id under two parents names two nodes, and id 7 and label "7" differ.
    #[test]
    fn container_keys_are_scoped_by_parent() {
        let mut handles = Vec::new();
        crate::doc::frames(1, &mut |ui| {
            handles.clear();
            for parent in ["a", "b"] { ui.column(parent, Node::DEFAULT, |ui| { handles.push(ui.row(7i64, Node { h: Size::Px(4.0), ..Node::DEFAULT }, |_| {})); }); }
            ui.column("c", Node::DEFAULT, |ui| { handles.push(ui.row(7u64, Node::DEFAULT, |_| {})); handles.push(ui.row("7", Node::DEFAULT, |_| {})); });
        });
        assert_ne!(handles[0].box_key, handles[1].box_key);
        assert_ne!(handles[2].box_key, handles[3].box_key);
        assert_eq!(Key::from(7u64), Key::Id(7));
    }

    /// A list of 10,000 fixed rows builds only the rows in view, places row `i` at `i * row_h` and allocates nothing once warm.
    #[test]
    fn list_builds_the_rows_in_view_without_warm_allocations() {
        use std::cell::Cell;
        let rows: Vec<Cell<Option<crate::Handle>>> = (0..10_000).map(|_| Cell::new(None)).collect();
        let (built, list) = (Cell::new(0), Cell::new(None));
        let mut ui = crate::Ui::new().expect("library");
        let mut build = |ui: &mut crate::Ui| {
            let column = ui.node("column".into(), Node { w: Size::Px(100.0), h: Size::Px(200.0), ..Node::DEFAULT });
            ui.within(&column, &mut |ui| {
                list.set(Some(ui.list("rows", 10_000, 20.0, &|i| i as i64 * 3 + 1, |ui, i| {
                    built.set(built.get() + 1);
                    rows[i].set(Some(ui.node("cell".into(), Node { h: Size::Px(20.0), ..Node::DEFAULT })));
                })));
            });
        };
        let input = crate::FrameIn { width: 320.0, height: 240.0, ..crate::FrameIn::default() };
        for _ in 0..4 { ui.frame(&input, &mut build).expect("frame"); }
        assert!(built.get() < 4 * 40, "built {} rows", built.get());
        let top = ui.rect(&list.get().unwrap()).y;
        for i in [0, 1, 5] { assert_eq!(ui.rect(&rows[i].get().unwrap()).y, top + 20.0 * i as f32); }
        ui.set_scroll_offset(&list.get().unwrap(), 20.0 * 5_000.0);
        for _ in 0..2 { ui.frame(&input, &mut build).expect("frame"); }
        assert_eq!(ui.rect(&rows[5_000].get().unwrap()).y, top);
        mem_sys::local_reset();
        for _ in 0..4 { ui.frame(&input, &mut build).expect("frame"); }
        assert_eq!(mem_sys::local_alloc_count(), 0);
    }

    /// A list row box is keyed by its item id: when the items reorder, each item's row (and what it built inside) keeps its key.
    #[test]
    fn list_rows_follow_their_item_ids_when_the_items_reorder() {
        use std::cell::{Cell, RefCell};
        let ids = RefCell::new(vec![10i64, 20, 30]);
        let seen: [Cell<(i64, i64)>; 3] = Default::default();
        let mut ui = crate::Ui::new().expect("library");
        let mut build = |ui: &mut crate::Ui| {
            let ids = ids.borrow();
            ui.list("items", ids.len(), 20.0, &|i| ids[i], |ui, i| {
                let child = ui.node("title".into(), Node { h: Size::Px(20.0), ..Node::DEFAULT });
                seen[i].set((ids[i], child.box_key));
            });
        };
        let input = crate::FrameIn { width: 320.0, height: 240.0, ..crate::FrameIn::default() };
        ui.frame(&input, &mut build).expect("frame");
        let before: Vec<(i64, i64)> = seen.iter().map(Cell::get).collect();
        ids.borrow_mut().reverse();
        ui.frame(&input, &mut build).expect("frame");
        for (id, key) in seen.iter().map(Cell::get) {
            assert_eq!(before.iter().find(|(old, _)| *old == id).map(|(_, key)| *key), Some(key), "item {id} kept its identity");
        }
        assert_ne!(before[0].1, before[1].1);
    }

    /// The label path keys a container exactly as `node` does, so moving a site from `node` + `within` to `row` keeps its state.
    #[test]
    fn container_label_matches_node() {
        let (mut by_node, mut by_row) = (None, None);
        crate::doc::frames(1, &mut |ui| { by_node = Some(ui.node("x".into(), Node::DEFAULT)); });
        crate::doc::frames(1, &mut |ui| { by_row = Some(ui.row("x", Node::DEFAULT, |_| {})); });
        assert_eq!(by_node.unwrap().box_key, by_row.unwrap().box_key);
    }
}
