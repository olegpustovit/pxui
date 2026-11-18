//! Containers that declare a node with its axis set and build their children inside it: `ui.row`, `ui.column`, `ui.stack`.
use crate::Handle;
use pxui_types::Node;
use pxui_types::node::Axis;
use pxui_types::ui::{SectionList, SectionPart};
use crate::Ui;
pub use pxui_types::ui::Key;

impl Ui {
    /// Builds two panes separated by a draggable divider.
    ///
    /// Keep the ratio in the caller. The divider reports a new ratio while held and on release (store the release value; it is final).
    /// A registered split body that builds no grip makes the split fixed: `ratio` is then always none. The panes borrow their callbacks
    /// for the call.
    /// Allocates: never on a warm frame.
    /// Identity: The key identifies the split under its parent.
    /// Panics: an invalid axis, ratio, minimum size or key, or when a pane's callback panics.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let _ = ui.split("editor", 0.5, &pxui::Split::DEFAULT, &mut |_| {}, &mut |_| {});
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn split<K: Into<Key>>(&mut self, key: K, ratio: f32, split: &crate::Split, a: &mut dyn FnMut(&mut Ui), b: &mut dyn FnMut(&mut Ui)) -> crate::SplitResponse {
        self.split_key(key.into(), ratio, split, a, b)
    }

    /// Builds a keyed leaf that displays a host-owned GPU image.
    ///
    /// The host keeps the native texture alive through all GPU uses. Name supplies the accessibility label verbatim.
    /// Invalid images, dimensions or names return InvalidInput. Outside a frame returns State.
    /// Allocates: never on a warm frame.
    /// Identity: The key identifies the node among siblings. Changing its accessible name preserves the node.
    /// Panics: an authored key whose label exceeds 256 bytes.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let result = ui.image("cover", "Cover art", &pxui::Image::default(), &pxui::ImageStyle::DEFAULT);
    ///     assert_eq!(result.err(), Some(pxui::Error::InvalidInput));
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn image<K: Into<Key>>(&mut self, key: K, name: &str, image: &crate::Image, style: &crate::ImageStyle) -> Result<Handle, crate::Error> {
        self.image_key(key.into(), name, image, style)
    }

    /// Whether the pointer has rested, unmoved, on the target named `key` for the hover delay (the tooltip wait); call it every frame the pointer is over the target.
    /// A different `key`, a moved pointer, a missed frame, a touch, a press in progress, a modal or focus loss restart the wait. The first call for a key is always false.
    /// There is one wait per `Ui`; the tooltip of an owner uses the same timer, keyed by the owner's box.
    /// Allocates: never; the wait stores the resolved key, so resets and pointer moves copy no text.
    /// Identity: `key` resolves under the active scope seed like `node`, so the same label under two parents names two targets.
    /// `Key::Resolved` names an existing box. The 1.x C text-keyed export keeps its own behavior.
    /// Panics: when the library refuses the call (a null `Ui`, or `key` over the text limit). A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| assert!(!ui.hover_delay("play")));
    /// ```
    #[inline]
    #[track_caller]
    pub fn hover_delay<K: Into<Key>>(&mut self, key: K) -> bool { self.hover_delay_key(key.into()) }

    /// The state of the inspector section built under `key` in the current parent: `Some(true)` expanded, `Some(false)` collapsed, `None` when no section of that key has been built there.
    /// Read it where the section is built (inside the same parent); it is `None` until the section has first been built there, and keeps its state from frame to frame.
    /// Allocates: never.
    /// Identity: `key` is the key given to `inspector_section`, resolved under the current parent's seed as for any label.
    /// Panics: when the library refuses the call (a null `Ui`, or `key` over the text limit). A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     assert_eq!(ui.expanded("details"), None); // not built yet
    ///     ui.inspector_section("details", true, None, "Details", &mut |ui| { ui.label("body", pxui::Label::DEFAULT); });
    ///     assert_eq!(ui.expanded("details"), Some(true));
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn expanded<K: Into<Key>>(&self, key: K) -> Option<bool> { self.expanded_key(key.into()) }

    /// Finds the live node for a key.
    ///
    /// Returns None when the node is absent from the UI's retained tables.
    /// Allocates: never.
    /// Identity: Authored labels and ids resolve as `node` resolves them: under the active scope seed during a build, under the root
    /// seed outside one. Use Key::Resolved with an existing box key; it is never hashed again.
    /// Panics: an invalid authored key.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     assert!(ui.box_of_key("missing").is_none());
    /// });
    /// ```
    #[inline]
    pub fn box_of_key<K: Into<Key>>(&self, key: K) -> Option<Handle> { self.box_of_author_key(key.into()) }

    /// Declares `node` under the current parent by `key` and returns its handle, valid this frame.
    ///
    /// The node crosses as it is, by reference: the library checks it where it lies and copies it once into the engine (a grid's
    /// tracks are copied too).
    /// Allocates: never on a warm frame (a new identity takes a slot the first time).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`): a label hashes as `key_of` does,
    /// an id as its NUL-and-hex label, so an id never collides with a printable label; a repeated key takes a `#dup` key.
    /// Panics: when the library refuses the node (outside a frame, an invalid transition curve or grid). A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// use pxui::ui::{Node, Size};
    /// let mut rows = Vec::new();
    /// pxui::doc::frames(1, &mut |ui| {
    ///     rows.clear();
    ///     let list = ui.node("list", Node::DEFAULT);
    ///     ui.within(&list, &mut |ui| for id in [41u64, 42] { rows.push(ui.node(id, Node { h: Size::Px(24.0), ..Node::DEFAULT })); });
    /// });
    /// assert_ne!(rows[0].box_key, rows[1].box_key);
    /// ```
    #[inline]
    #[track_caller]
    pub fn node<K: Into<Key>>(&mut self, key: K, node: Node) -> Handle { self.node_key(key.into(), node) }

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
    ///         a = Some(ui.node("a", Node { w: Size::Px(10.0), ..Node::DEFAULT }));
    ///         b = Some(ui.node("b", Node { w: Size::Px(10.0), ..Node::DEFAULT }));
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
    ///         a = Some(ui.node("art", Node { w: Size::Px(40.0), h: Size::Px(40.0), ..Node::DEFAULT }));
    ///         b = Some(ui.node("dot", Node { w: Size::Px(8.0), h: Size::Px(8.0), ..Node::DEFAULT }));
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
        let b = self.node_key(key, node);
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
        let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        let mut build = |ui: &mut crate::Ui| {
            let column = ui.node("column", Node { w: Size::Px(100.0), h: Size::Px(200.0), ..Node::DEFAULT });
            ui.within(&column, &mut |ui| {
                list.set(Some(ui.list("rows", 10_000, 20.0, &|i| i as i64 * 3 + 1, |ui, i| {
                    built.set(built.get() + 1);
                    rows[i].set(Some(ui.node("cell", Node { h: Size::Px(20.0), ..Node::DEFAULT })));
                })));
            });
        };
        let input = crate::FrameIn::new(320.0, 240.0);
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
        let mut ui = crate::Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        let mut build = |ui: &mut crate::Ui| {
            let ids = ids.borrow();
            ui.list("items", ids.len(), 20.0, &|i| ids[i], |ui, i| {
                let child = ui.node("title", Node { h: Size::Px(20.0), ..Node::DEFAULT });
                seen[i].set((ids[i], child.box_key));
            });
        };
        let input = crate::FrameIn::new(320.0, 240.0);
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
        crate::doc::frames(1, &mut |ui| { by_node = Some(ui.node("x", Node::DEFAULT)); });
        crate::doc::frames(1, &mut |ui| { by_row = Some(ui.row("x", Node::DEFAULT, |_| {})); });
        assert_eq!(by_node.unwrap().box_key, by_row.unwrap().box_key);
    }
}
