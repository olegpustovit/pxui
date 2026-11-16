//! Containers that declare a node with its axis set and build their children inside it: `ui.row`, `ui.column`, `ui.stack`.
use pxui_types::{Node, PxuiBox};
use pxui_types::node::Axis;
use crate::Ui;

/// A node's key among its siblings: a fixed label or an item id, never display text.
///
/// Like React's `key`: unique among the children of one parent, so list rows pass their item id. A key is resolved under the
/// parent's seed, so the same key under two parents names two nodes. Two siblings with one key take order-stable `#dup` keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A fixed label, hashed exactly as `Ui::node` hashes its `label` (a `###` suffix replaces it).
    Label(&'static str),
    /// An item id, hashed as the label of a NUL byte and its 16 lowercase hex digits under the parent's seed.
    Id(i64),
}
impl From<&'static str> for Key { #[inline] fn from(label: &'static str) -> Key { Key::Label(label) } }
impl From<i64> for Key { #[inline] fn from(id: i64) -> Key { Key::Id(id) } }
impl From<u64> for Key { #[inline] fn from(id: u64) -> Key { Key::Id(id as i64) } }

impl Ui {
    /// Declares `node` laid out as a row under the current parent and runs `f` to build its children inside it.
    ///
    /// `node` is used as given with `axis` set to `Axis::Row`; the returned handle is the row's, valid this frame (style it,
    /// read its signal). Same as `node` then `within`.
    /// Allocates: as `node` and `within` (never on a warm frame).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); an id never collides with a
    /// printable label.
    /// Panics: when the library refuses the node (outside a frame, an invalid transition curve), or when `f` panics.
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
    pub fn row<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, mut f: F) -> PxuiBox { self.container(key.into(), Axis::Row, node, &mut f) }

    /// Declares `node` laid out as a column under the current parent and runs `f` to build its children inside it.
    ///
    /// `node` is used as given with `axis` set to `Axis::Column`; the returned handle is the column's, valid this frame. Same as
    /// `node` then `within`.
    /// Allocates: as `node` and `within` (never on a warm frame).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); an id never collides with a
    /// printable label.
    /// Panics: when the library refuses the node (outside a frame, an invalid transition curve), or when `f` panics.
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
    pub fn column<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, mut f: F) -> PxuiBox { self.container(key.into(), Axis::Column, node, &mut f) }

    /// Declares `node` as a stack under the current parent and runs `f` to build its children inside it, overlapping.
    ///
    /// `node` is used as given with `axis` set to `Axis::Stack`: children sit at the padding origin in `z` order (equal `z`: build
    /// order). The returned handle is the stack's, valid this frame. Same as `node` then `within`.
    /// Allocates: as `node` and `within` (never on a warm frame).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); an id never collides with a
    /// printable label.
    /// Panics: when the library refuses the node (outside a frame, an invalid transition curve), or when `f` panics.
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
    pub fn stack<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, mut f: F) -> PxuiBox { self.container(key.into(), Axis::Stack, node, &mut f) }

    /// The one body of `row`, `column` and `stack`: `node` with `axis` set, declared by `key`, then `f` inside it.
    // Inline: an out-of-line body retires about 110 instructions more per container than `node` + `within` (2.2% on the abi_bench
    // Author page, over its 1% limit), because the node checks and the frame-entry loads no longer fold at the call site.
    #[inline]
    #[track_caller]
    fn container(&mut self, key: Key, axis: Axis, mut node: Node, f: &mut dyn FnMut(&mut Ui)) -> PxuiBox {
        node.axis = axis;
        let mut id_label = [0u8; 17];
        let label = match key { Key::Label(label) => label, Key::Id(id) => id_text(id, &mut id_label) };
        let b = self.node(label, node);
        self.within(&b, f);
        b
    }
}

/// `id` as a label: a NUL byte and its 16 lowercase hex digits, written into `out`. Out of line: label keys inline without it.
#[cold]
#[inline(never)]
fn id_text(id: i64, out: &mut [u8; 17]) -> &str {
    for (at, digit) in out[1..].iter_mut().enumerate() { *digit = b"0123456789abcdef"[(id as u64 >> (60 - 4 * at)) as usize & 15]; }
    // NUL and ASCII hex digits: always UTF-8
    core::str::from_utf8(out).unwrap_or_default()
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

    /// The label path keys a container exactly as `node` does, so moving a site from `node` + `within` to `row` keeps its state.
    #[test]
    fn container_label_matches_node() {
        let (mut by_node, mut by_row) = (None, None);
        crate::doc::frames(1, &mut |ui| { by_node = Some(ui.node("x", Node::DEFAULT)); });
        crate::doc::frames(1, &mut |ui| { by_row = Some(ui.row("x", Node::DEFAULT, |_| {})); });
        assert_eq!(by_node.unwrap().box_key, by_row.unwrap().box_key);
    }
}
