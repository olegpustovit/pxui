//! A pressable node: `ui.pressable` declares a clickable, focusable node, looks it by its state and reports what happened to it.
use pxui_types::{Node, PxuiBox};
use pxui_types::theme::{Look, State};
use super::Key;
use crate::Ui;
use pxui_types::Flags;

/// What happened to a pressable node this frame, read from its signal when it was declared.
#[derive(Clone, Copy, Debug)]
pub struct Response {
    /// The node was clicked: pointer released over it after a press on it, or keyboard activation (Enter or Space) while focused.
    pub clicked: bool,
    /// The pointer is over the node.
    pub hovered: bool,
    /// The pointer is pressed on the node.
    pub held: bool,
    /// The node has keyboard focus.
    pub focused: bool,
    /// The click came from the keyboard (Enter or Space on the focused node), not the pointer; implies `clicked`.
    pub activated: bool,
    /// The value the widget edits changed this frame: a toggle flipped, a segmented control's choice moved. Always false for a button
    /// or a pressable node, which hold no value.
    pub changed: bool,
    /// The node, valid this frame: style it, read more of its signal.
    pub handle: PxuiBox,
}

impl Ui {
    /// Declares `node` as a pressable under the current parent, applies `look` by its state, runs `f` inside it and reports what happened.
    ///
    /// `node` is used as given with `CLICKABLE | TAB_STOP` added. `look` is applied with the hovered, pressed (held) and focused state, so
    /// the theme's state transitions run; the response is the one the node earned before `f` ran, so `f` may use it for its own children.
    /// Allocates: as `node`, `signal` and `within` (never on a warm frame).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`).
    /// Panics: when the library refuses the node (outside a frame, an invalid transition curve), or when `f` panics.
    ///
    /// ```
    /// use pxui::ui::{Node, Size, Look, Role};
    /// let mut clicked = false;
    /// let mut build = |ui: &mut pxui::Ui| {
    ///     let node = Node { w: Size::Px(50.0), h: Size::Px(20.0), role: Role::Button, ..Node::DEFAULT };
    ///     clicked |= ui.pressable("save", node, &Look::NONE, |_| {}).clicked;
    /// };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// pxui::testing::click(&mut ui, 10.0, 10.0, &mut build);
    /// assert!(clicked);
    /// ```
    #[inline]
    #[track_caller]
    pub fn pressable<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, look: &Look, mut f: F) -> Response { self.press(key.into(), node, look, &mut f) }

    /// Builds `f` with every interactive node it declares disabled when `disabled`: no press, no focus, and the accessible disabled state.
    ///
    /// It covers whatever `f` declares (`button_with`, `toggle_with`, `segmented_with`, `pressable`, any node with `CLICKABLE` or
    /// `FOCUSABLE`); the kit widgets also show their `disabled` look. Scopes nest: a node is disabled when any scope around it is. A node
    /// still blocks what is under it. A view cached inside rebuilds when the flag flips. `f` runs even when `disabled` is false.
    /// Allocates: never (the scope is a flag in the library's frame state, nothing is pushed).
    /// Panics: when `f` panics, or when the library refuses the call (outside a frame).
    ///
    /// ```
    /// use pxui::Button;
    /// let mut clicked = false;
    /// let mut build = |ui: &mut pxui::Ui| {
    ///     ui.disabled(true, |ui| { clicked |= ui.button_with("clear", "Clear cache", &Button::DEFAULT.danger()).clicked; });
    /// };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// pxui::testing::click(&mut ui, 30.0, 12.0, &mut build);
    /// assert!(!clicked);
    /// ```
    #[inline]
    #[track_caller]
    pub fn disabled<F: FnMut(&mut Ui)>(&mut self, disabled: bool, mut f: F) { self.disabled_scope(disabled, &mut f) }

    /// The one body of `pressable`: the node with its flags, its signal, its look by state, then `f` inside it.
    #[inline]
    #[track_caller]
    fn press(&mut self, key: Key, mut node: Node, look: &Look, f: &mut dyn FnMut(&mut Ui)) -> Response {
        node.flags |= Flags::CLICKABLE | Flags::TAB_STOP;
        let b = self.node(key.into(), node);
        let signal = self.signal(&b);
        let focused = self.focused(&b);
        self.set_look(&b, *look, State { hovered: signal.hovered, pressed: signal.held, focused, ..State::default() });
        self.within(&b, f);
        Response { clicked: signal.clicked, hovered: signal.hovered, held: signal.held, focused, activated: signal.clicked && !signal.released, changed: false, handle: b }
    }
}

#[cfg(test)]
mod tests {
    use crate::ui::{Look, Node, Role, Size};
    use crate::{testing, Event, Ui};
    use pxui_types::Flags;

    fn node() -> Node { Node { w: Size::Px(50.0), h: Size::Px(20.0), role: Role::Button, ..Node::DEFAULT } }

    /// Runs `events` as one frame over a pressable and returns its response.
    fn respond(ui: &mut Ui, events: &[Event]) -> super::Response {
        let mut out = None;
        testing::frame(ui, events, &mut |ui| { out = Some(ui.pressable("p", node(), &Look::NONE, |_| {})); });
        out.unwrap()
    }
    fn pointer(kind: u32) -> Event { Event { kind, x: 10.0, y: 10.0, ..Default::default() } }

    #[test]
    fn click_hover_hold() {
        let mut ui = crate::doc::frames(2, &mut |ui| { ui.pressable("p", node(), &Look::NONE, |_| {}); });
        let away = Event { kind: Event::POINTER_MOVED, x: 300.0, y: 200.0, ..Default::default() };
        assert!(!respond(&mut ui, &[away]).hovered);
        let r = respond(&mut ui, &[Event { kind: Event::POINTER_MOVED, x: 10.0, y: 10.0, ..Default::default() }]);
        assert!(r.hovered && !r.held && !r.clicked);
        let r = respond(&mut ui, &[pointer(Event::POINTER_PRESSED)]);
        assert!(r.held && r.hovered && !r.clicked);
        let r = respond(&mut ui, &[pointer(Event::POINTER_RELEASED)]);
        assert!(r.clicked && !r.activated && !r.held);
        assert!(!respond(&mut ui, &[]).clicked);
    }

    #[test]
    fn keyboard_activation_and_focus() {
        let mut ui = crate::doc::frames(2, &mut |ui| { ui.pressable("p", node(), &Look::NONE, |_| {}); });
        let r = respond(&mut ui, &[]);
        assert!(!r.focused && !r.activated);
        let h = r.handle;
        testing::frame(&mut ui, &[], &mut |ui| { let b = ui.pressable("p", node(), &Look::NONE, |_| {}).handle; ui.focus_box(&b); });
        let _ = h;
        assert!(respond(&mut ui, &[]).focused);
        let enter = [Event { kind: Event::KEY_PRESSED, code: pxui_types::KEY_SPECIAL + 5, ..Default::default() }];
        let r = respond(&mut ui, &enter);
        assert!(r.clicked && r.activated && r.focused);
    }

    /// The pressable is one node: same count as the hand-written node, signal, within.
    #[test]
    fn node_count_matches_hand_written() {
        let hand = crate::doc::frames(2, &mut |ui| {
            let b = ui.node("p".into(), Node { flags: Flags::CLICKABLE | Flags::TAB_STOP, ..node() });
            let _ = ui.signal(&b);
            ui.within(&b, &mut |ui| { ui.node("c".into(), node()); });
        });
        let sugar = crate::doc::frames(2, &mut |ui| { ui.pressable("p", node(), &Look::NONE, |ui| { ui.node("c".into(), node()); }); });
        assert_eq!(hand.a11y_nodes().len(), sugar.a11y_nodes().len());
        assert!(!sugar.a11y_nodes().is_empty());
    }

    #[test]
    fn warm_frames_allocate_nothing() {
        let mut ui = crate::doc::frames(1, &mut |_| {});
        let mut build = |ui: &mut Ui| { ui.pressable("p", node(), &Look::NONE, |ui| { ui.node("c".into(), node()); }); ui.pressable(7i64, node(), &Look::NONE, |_| {}); };
        for round in 0..20 {
            mem_sys::local_reset();
            testing::frame(&mut ui, &[], &mut build);
            if round > 2 { assert_eq!(mem_sys::local_alloc_count(), 0); }
        }
    }
}
