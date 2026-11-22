//! A pressable node: `ui.pressable` declares a clickable, focusable node, looks it by its state and reports what happened to it.
use crate::Handle;
use pxui_types::{a11y::state::SELECTED, Node};
use pxui_types::theme::{State, States};
use super::Key;
use crate::Ui;
use pxui_types::Flags;

/// What happened to a widget this frame (a pressable node, a button, a toggle, a segmented control), read from its signal when it was
/// declared.
///
/// It is the widget-level summary of `Signal`, the raw per-node record: `clicked`, `hovered` and `held` are the signal's, the rest is what
/// the widget adds (focus, keyboard activation, an edited value, a context request). `ui.signal(&response.handle)` reads the rest of the
/// raw record (drag, wheel, pinch, press points, click count). A one-off choice (`menu`, `dialog`) answers with `Chosen` instead.
#[derive(Clone, Copy, Debug)]
// C layout, the engine's own record (`trusted::EngineResponse`) with `changed` in its padding: the static wrapper copies it whole
#[repr(C)]
pub struct Response {
    /// The node was clicked: the primary button released over it after a press on it, or keyboard activation (Enter or Space) while focused.
    pub clicked: bool,
    /// The pointer is over the node.
    pub hovered: bool,
    /// The pointer is pressed on the node.
    pub held: bool,
    /// The node has keyboard focus.
    pub focused: bool,
    /// The click came from the keyboard (Enter or Space on the focused node), not the pointer; implies `clicked`.
    pub activated: bool,
    /// A secondary press (the right button, or the two-finger or control click the platform reports as one) began on the node this
    /// frame: the request for its context menu. It never clicks; `ui.signal(&handle).press_point` is where it landed.
    pub secondary: bool,
    /// The value the widget edits changed this frame: a toggle flipped, a segmented control's choice moved. Always false for a button
    /// or a pressable node, which hold no value.
    pub changed: bool,
    /// The node, valid this frame: style it, read more of its signal.
    pub handle: Handle,
}

/// How `Ui::pressable` varies its node, as a `Copy` value built with `const fn` builders from `Pressable::DEFAULT`.
///
/// The default takes clicks and keyboard focus (a tab stop), is enabled and not selected, and keeps the node's own look. `'a` is the
/// borrow of the looks given to `look` (`'static` for a const).
/// Identity: none of its own; the node is keyed by the `key` given to `Ui::pressable`, never by these options, so changing them keeps
/// the node's hover, press, focus and transition state.
/// Panics: never; building one is const, and `Ui::pressable` refuses nothing in it.
#[derive(Clone, Copy, Debug)]
pub struct Pressable<'a> {
    look: Option<&'a States>,
    enabled: bool,
    tab_stop: bool,
    selected: bool,
    focus_on_click: bool,
}

impl<'a> Pressable<'a> {
    /// A pressable that keeps its node's look: enabled, a tab stop that only Tab focuses, not selected.
    pub const DEFAULT: Pressable<'a> = Pressable { look: None, enabled: true, tab_stop: true, selected: false, focus_on_click: false };

    /// Sets the look per state (idle, hover, pressed, focus, disabled, checked), applied by the node's state each frame so the theme's
    /// state transitions run; without one the node keeps the look it was declared with. The looks are borrowed, not copied.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::{Look, States}, Pressable};
    /// const ROW_LOOKS: States = States::of(Look::NONE);
    /// const ROW: Pressable = Pressable::DEFAULT.look(&ROW_LOOKS);
    /// let _ = ROW;
    /// ```
    pub const fn look(mut self, look: &'a States) -> Pressable<'a> { self.look = Some(look); self }

    /// Sets whether the node takes input: disabled it is neither clickable nor focusable, shows the `disabled` look, reads as disabled to
    /// accessibility and still blocks what is under it. A disabled pressable never reports `clicked`, `activated` or `secondary`: not
    /// for a press begun while it was enabled, nor for an assistive technology's press.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let p = pxui::Pressable::DEFAULT.disabled(true);
    /// let _ = p;
    /// ```
    pub const fn disabled(mut self, disabled: bool) -> Pressable<'a> { self.enabled = !disabled; self }

    /// Sets whether Tab moves keyboard focus to the node. A drop target or a gesture's root takes the pointer but is no tab stop; it
    /// then takes no keyboard focus at all (no keyboard activation either).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const DROP: pxui::Pressable = pxui::Pressable::DEFAULT.tab_stop(false);
    /// let _ = DROP;
    /// ```
    pub const fn tab_stop(mut self, tab_stop: bool) -> Pressable<'a> { self.tab_stop = tab_stop; self }

    /// Marks the node as the selected or current one of a group (a marked row, the open page): the `checked` look and the accessible
    /// SELECTED state.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let p = pxui::Pressable::DEFAULT.selected(true);
    /// let _ = p;
    /// ```
    pub const fn selected(mut self, selected: bool) -> Pressable<'a> { self.selected = selected; self }

    /// Sets whether a press on the node focuses it (false by default: only Tab focuses it, as a kit button), as `Button::focus_on_click`
    /// and `Menu::focus_on_click` do. It applies to a tab stop only: with `tab_stop(false)` the node takes no focus at all.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const ROW: pxui::Pressable = pxui::Pressable::DEFAULT.focus_on_click(true);
    /// let _ = ROW;
    /// ```
    pub const fn focus_on_click(mut self, focus: bool) -> Pressable<'a> { self.focus_on_click = focus; self }
}

impl Ui {
    /// Declares `node` as a pressable under the current parent, looks it by its state, runs `f` inside it with that state and reports
    /// what happened.
    ///
    /// `node` is used as given, except that `pressable` owns its input flags: `CLICKABLE`, plus `TAB_STOP` unless `tab_stop(false)`
    /// (`FOCUSABLE` alone with `focus_on_click(true)`, so a press focuses it); disabled, `BLOCKING` only. The state (hovered, pressed, focused, disabled, checked) is what input did to the node before `f`
    /// ran; `pressed` is also on while a touch gesture's highlight shows (a `long_press` row). The look `pressable.look` picks for it
    /// is applied before `f` runs. A gesture is taken on `handle` with `ui.gesture`, in this build after `pressable` returns (call
    /// order does not matter: gestures route the next input). There is no body kind: the structure is the caller's `node` and `f`,
    /// so there is nothing for a registered body to replace; style it with `look` or the theme's colour tokens.
    /// Allocates: as `node`, `signal` and `within` (never on a warm frame).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the display text.
    /// Panics: when the library refuses the node (outside a frame, an invalid transition curve), or when `f` panics. A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// use pxui::{ui::{Look, Node, Role, States}, Pressable};
    /// const ROW: Node = Node::row().h_px(20.0).w_px(50.0).role(Role::Button);
    /// const LOOKS: States = States::of(Look::NONE);
    /// let mut clicked = false;
    /// let mut build = |ui: &mut pxui::Ui| {
    ///     clicked |= ui.pressable("save", ROW, &Pressable::DEFAULT.look(&LOOKS), |_, state| { let _ = state.hovered; }).clicked;
    /// };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// pxui::testing::click(&mut ui, 10.0, 10.0, &mut build);
    /// assert!(clicked);
    /// ```
    #[inline]
    #[track_caller]
    pub fn pressable<K: Into<Key>, F: FnMut(&mut Ui, State)>(&mut self, key: K, node: Node, pressable: &Pressable<'_>, mut f: F) -> Response {
        self.press(key.into(), node, pressable, &mut f)
    }

    /// Builds `f` with every interactive node it declares disabled when `disabled`: no press, no focus, and the accessible disabled state.
    ///
    /// It covers whatever `f` declares (`button`, `toggle`, `segmented`, `pressable`, any node with `CLICKABLE` or
    /// `FOCUSABLE`); the kit widgets also show their `disabled` look (a `pressable` shows the look of its own `enabled`). Scopes nest: a
    /// node is disabled when any scope around it is. A node still blocks what is under it. A view cached inside rebuilds when the flag
    /// flips. `f` runs even when `disabled` is false.
    /// Allocates: never (the scope is a flag in the library's frame state, nothing is pushed).
    /// Panics: when `f` panics, or when the library refuses the call (outside a frame).
    ///
    /// ```
    /// use pxui::Button;
    /// let mut clicked = false;
    /// let mut build = |ui: &mut pxui::Ui| {
    ///     ui.disabled(true, |ui| { clicked |= ui.button("clear", "Clear cache", &Button::DEFAULT.danger()).clicked; });
    /// };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// pxui::testing::click(&mut ui, 30.0, 12.0, &mut build);
    /// assert!(!clicked);
    /// ```
    #[inline]
    #[track_caller]
    pub fn disabled<F: FnMut(&mut Ui)>(&mut self, disabled: bool, mut f: F) { self.disabled_scope(disabled, &mut f) }

    /// The one body of `pressable`: the node with its flags and fixed state, its signal, its look by state, then `f` inside it.
    // Inline: listed in the plan (section 3) from its measurement before U1; no abi_bench page builds a pressable, so U1c.1 could not
    // re-measure it (out of line, the label body moved the pages by at most 0.02% and lost its #[inline]).
    #[inline]
    #[track_caller]
    fn press(&mut self, key: Key, mut node: Node, p: &Pressable<'_>, f: &mut dyn FnMut(&mut Ui, State)) -> Response {
        let input = Flags(Flags::CLICKABLE.0 | Flags::TAB_STOP.0 | Flags::BLOCKING.0);
        node.flags = Flags(node.flags.0 & !input.0) | match (p.enabled, p.tab_stop) {
            (false, _) => Flags::BLOCKING,
            (true, true) if p.focus_on_click => Flags::CLICKABLE | Flags::FOCUSABLE,
            (true, true) => Flags::CLICKABLE | Flags::TAB_STOP,
            (true, false) => Flags::CLICKABLE,
        };
        (node.state.disabled, node.state.checked) = (!p.enabled, p.selected);
        if p.selected { node.a11y |= SELECTED; }
        let b = self.node_key(key, node);
        let signal = self.signal(&b);
        let focused = p.enabled && p.tab_stop && self.focused(&b);
        // a touch gesture's highlight (taken with `ui.gesture` on this node) shows as the press it is
        let highlighted = self.gesture_state(&b).highlighted;
        let state = State { hovered: signal.hovered, pressed: signal.held || highlighted, focused, disabled: !p.enabled, checked: p.selected };
        if let Some(looks) = p.look { self.style(&b, &crate::theme::States::of(*looks.pick(state)), state); }
        self.within(&b, &mut |ui| f(ui, state));
        let clicked = signal.clicked && p.enabled;
        Response { clicked, hovered: signal.hovered, held: signal.held, focused, activated: clicked && !signal.released, changed: false,
            secondary: p.enabled && signal.pressed && signal.button == Some(crate::Signal::BUTTON_SECONDARY), handle: b }
    }
}

#[cfg(test)]
mod tests {
    use super::Pressable;
    use crate::ui::{Look, Node, Role, Size, State, States};
    use crate::{testing, Event, FrameIn, GestureOpts, Touch, Str, Ui};
    use pxui_types::{a11y::state, geom::Rgba, Flags};
    use std::cell::Cell;

    fn node() -> Node { Node::DEFAULT.w(Size::Px(50.0)).h(Size::Px(20.0)).role(Role::Button) }
    const P: Pressable<'static> = Pressable::DEFAULT;

    /// Runs `events` as one frame over a pressable and returns its response.
    fn respond(ui: &mut Ui, events: &[Event]) -> super::Response {
        let mut out = None;
        testing::frame(ui, events, &mut |ui| { out = Some(ui.pressable("p", node(), &P, |_, _| {})); });
        out.unwrap()
    }
    fn pointer(kind: u32) -> Event { Event::pointer(kind, 0, 10.0, 10.0) }

    #[test]
    fn click_hover_hold() {
        let mut ui = crate::doc::frames(2, &mut |ui| { ui.pressable("p", node(), &P, |_, _| {}); });
        let away = Event::pointer(Event::POINTER_MOVED, 0, 300.0, 200.0);
        assert!(!respond(&mut ui, &[away]).hovered);
        let r = respond(&mut ui, &[Event::pointer(Event::POINTER_MOVED, 0, 10.0, 10.0)]);
        assert!(r.hovered && !r.held && !r.clicked);
        let r = respond(&mut ui, &[pointer(Event::POINTER_PRESSED)]);
        assert!(r.held && r.hovered && !r.clicked && !r.secondary);
        let r = respond(&mut ui, &[pointer(Event::POINTER_RELEASED)]);
        assert!(r.clicked && !r.activated && !r.held);
        assert!(!respond(&mut ui, &[]).clicked);
    }

    #[test]
    fn keyboard_activation_and_focus() {
        let mut ui = crate::doc::frames(2, &mut |ui| { ui.pressable("p", node(), &P, |_, _| {}); });
        let r = respond(&mut ui, &[]);
        assert!(!r.focused && !r.activated);
        testing::frame(&mut ui, &[], &mut |ui| { let b = ui.pressable("p", node(), &P, |_, _| {}).handle; ui.focus_box(&b); });
        assert!(respond(&mut ui, &[]).focused);
        let enter = [Event::key(Event::KEY_PRESSED, crate::KeyCode::Enter.wire())];
        let r = respond(&mut ui, &enter);
        assert!(r.clicked && r.activated && r.focused);
    }

    /// A secondary (right-button) press is the context-menu request: `secondary` in its frame, never a click; a kit button reports it too.
    #[test]
    fn secondary_press_requests_the_context_menu() {
        let button = Cell::new(None);
        let mut build = |ui: &mut Ui| {
            ui.pressable("p", node(), &P, |_, _| {});
            button.set(Some(ui.button("b", "Button", &crate::Button::DEFAULT)));
        };
        let mut ui = crate::doc::frames(2, &mut build);
        let right = |kind: u32, x: f64, y: f64| Event::pointer(kind, 1, x, y);
        let r = respond(&mut ui, &[right(Event::POINTER_PRESSED, 10.0, 10.0)]);
        assert!(r.secondary && !r.clicked);
        assert_eq!(ui.signal(&r.handle).press_point, (10.0, 10.0), "where it landed is the signal's press point");
        let r = respond(&mut ui, &[right(Event::POINTER_RELEASED, 10.0, 10.0)]);
        assert!(!r.secondary && !r.clicked, "a secondary release never clicks");
        testing::frame(&mut ui, &[], &mut build);
        let b = ui.rect(&button.get().unwrap().handle);
        let (x, y) = (f64::from(b.x + b.w / 2.0), f64::from(b.y + b.h / 2.0));
        testing::frame(&mut ui, &[right(Event::POINTER_PRESSED, x, y)], &mut build);
        assert!(button.get().unwrap().secondary, "the kit button's response carries it through the library");
        testing::frame(&mut ui, &[right(Event::POINTER_RELEASED, x, y)], &mut build);
        assert!(!button.get().unwrap().secondary && !button.get().unwrap().clicked);
    }

    /// The look follows the state through `States::pick`, and `f` is given the same state.
    #[test]
    fn looks_by_state_and_gives_f_the_state() {
        let fill = |r: u8| Look::NONE.fill(Rgba::rgb(r, 0, 0));
        let looks = States::of(fill(10)).hover(fill(20)).pressed(fill(30)).focus(fill(40)).disabled(fill(50)).checked(fill(60));
        let (selected, enabled, seen) = (Cell::new(false), Cell::new(true), Cell::new(State::default()));
        let mut build = |ui: &mut Ui| {
            let p = Pressable::DEFAULT.look(&looks).selected(selected.get()).disabled(!enabled.get());
            ui.pressable("p", node(), &p, |_, state| seen.set(state));
        };
        let shown = |ui: &Ui| ui.layers().flat_map(|l| l.instances.iter()).find(|i| i.color[1..] == [0, 0, 255] && i.color[0] % 10 == 0).map(|i| i.color[0]);
        let mut ui = crate::doc::frames(2, &mut build);
        testing::frame(&mut ui, &[Event::pointer(Event::POINTER_MOVED, 0, 300.0, 200.0)], &mut build);
        assert_eq!((shown(&ui), seen.get()), (Some(10), State::default()));
        testing::frame(&mut ui, &[pointer(Event::POINTER_MOVED)], &mut build);
        assert_eq!((shown(&ui), seen.get().hovered), (Some(20), true));
        testing::frame(&mut ui, &[pointer(Event::POINTER_PRESSED)], &mut build);
        assert_eq!((shown(&ui), seen.get().pressed), (Some(30), true));
        testing::frame(&mut ui, &[pointer(Event::POINTER_RELEASED), Event::pointer(Event::POINTER_MOVED, 0, 300.0, 200.0)], &mut build);
        selected.set(true);
        testing::frame(&mut ui, &[], &mut build);
        assert_eq!((shown(&ui), seen.get().checked), (Some(60), true));
        let a11y = ui.a11y_nodes().iter().find(|n| n.is_role(Role::Button)).unwrap().states;
        assert!(a11y & state::SELECTED != 0, "selected reads SELECTED");
        enabled.set(false);
        testing::frame(&mut ui, &[], &mut build);
        assert_eq!((shown(&ui), seen.get().disabled), (Some(50), true));
    }

    /// `enabled(false)`: not clickable or focusable, reads disabled, never clicks (not even a press begun while enabled) and still
    /// blocks what is under it. `tab_stop(false)`: clickable, not focusable.
    #[test]
    fn disabled_never_clicks_and_tab_stop_is_optional() {
        let (enabled, tab_stop, under, last) = (Cell::new(true), Cell::new(true), Cell::new(false), Cell::new(None));
        let mut build = |ui: &mut Ui| {
            let b = ui.node("under", node().at(0.0, 0.0).flags(Flags::CLICKABLE).role(Role::Unknown));
            under.set(under.get() | ui.signal(&b).clicked);
            let p = P.disabled(!enabled.get()).tab_stop(tab_stop.get());
            last.set(Some(ui.pressable("p", node().at(0.0, 0.0), &p, |_, _| {})));
        };
        let mut ui = crate::doc::frames(2, &mut build);
        let states = |ui: &Ui| ui.a11y_nodes().iter().find(|n| n.is_role(Role::Button)).unwrap().states;
        assert_eq!(states(&ui) & (state::ENABLED | state::FOCUSABLE), state::ENABLED | state::FOCUSABLE);
        testing::frame(&mut ui, &[pointer(Event::POINTER_PRESSED)], &mut build);
        enabled.set(false);
        testing::frame(&mut ui, &[pointer(Event::POINTER_RELEASED)], &mut build);
        assert!(!last.get().unwrap().clicked, "a press begun while enabled does not click once disabled");
        testing::click(&mut ui, 10.0, 10.0, &mut build);
        assert!(!last.get().unwrap().clicked && !under.get(), "disabled: no click, and the node under it is blocked");
        testing::frame(&mut ui, &[pointer(Event::POINTER_PRESSED)], &mut build);
        let code = Event::pointer(Event::POINTER_PRESSED, 1, 10.0, 10.0);
        testing::frame(&mut ui, &[Event::pointer(Event::POINTER_RELEASED, 0, 10.0, 10.0), code], &mut build);
        assert!(!last.get().unwrap().secondary, "disabled: no context menu");
        assert_eq!(states(&ui) & (state::ENABLED | state::FOCUSABLE), 0);
        (enabled.set(true), tab_stop.set(false));
        testing::frame(&mut ui, &[Event::pointer(Event::POINTER_RELEASED, 1, 10.0, 10.0)], &mut build);
        testing::click(&mut ui, 10.0, 10.0, &mut build);
        assert!(last.get().unwrap().clicked, "no tab stop still clicks");
        assert_eq!(states(&ui) & (state::ENABLED | state::FOCUSABLE), state::ENABLED);
    }

    /// A gesture taken with `ui.gesture` after `pressable` returns works as one taken first: call order does not matter. The long
    /// press's highlight reaches the look and `f` as `pressed`; a touch drag holds the node.
    #[test]
    fn a_gesture_taken_after_pressable_routes_the_next_input() {
        let (opts, seen, gesture, held) = (Cell::new(GestureOpts::DEFAULT.long_press()), Cell::new(State::default()), Cell::new(Default::default()), Cell::new(false));
        let mut build = |ui: &mut Ui| {
            let r = ui.pressable("row", Node::DEFAULT.at(0.0, 0.0).w(Size::Px(200.0)).h(Size::Px(50.0)), &P.tab_stop(false), |_, state| seen.set(state));
            held.set(r.held);
            gesture.set(ui.gesture(&r.handle, &opts.get()));
        };
        let input = |time: f64| FrameIn::new(320.0, 240.0).time(time);
        let touch = |ui: &mut Ui, phase: u32, at: f64, build: &mut dyn FnMut(&mut Ui)| {
            let packet = Touch::new(1, 7, [100.0, 25.0], at, 1.0, phase);
            let event = { let mut record = Event::default(); record.kind = Event::TOUCH; record.text = { let mut record = Str::default(); record.ptr = (&packet as *const Touch).cast(); record.len = core::mem::size_of::<Touch>(); record }; record };
            ui.frame(&{
                let mut frame = input(at);
                frame.events = core::slice::from_ref(&event);
                frame
            }, build).unwrap();
        };
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        ui.frame(&input(0.0), &mut build).unwrap();
        touch(&mut ui, 0, 1.0, &mut build);
        ui.frame(&input(1.2), &mut build).unwrap();
        assert!(gesture.get().highlighted && seen.get().pressed && !held.get(), "the highlight is the look's press, not a held pointer");
        ui.frame(&input(1.6), &mut build).unwrap();
        assert!(gesture.get().long_pressed);
        touch(&mut ui, 2, 1.7, &mut build);
        opts.set(GestureOpts::DEFAULT.touch_drag());
        ui.frame(&input(2.0), &mut build).unwrap();
        touch(&mut ui, 0, 3.0, &mut build);
        assert!(held.get(), "a touch drag taken after pressable holds it at once");
        touch(&mut ui, 2, 3.1, &mut build);
    }

    /// `focus_on_click(true)`: a click focuses the node (and its response says so); by default only Tab does, and with `tab_stop(false)`
    /// nothing does.
    #[test]
    fn focus_on_click_focuses_the_pressed_node() {
        for (p, focuses) in [(P, false), (P.focus_on_click(true), true), (P.focus_on_click(true).tab_stop(false), false)] {
            let last = Cell::new(None);
            let mut build = |ui: &mut Ui| { last.set(Some(ui.pressable("p", node(), &p, |_, _| {}))); };
            let mut ui = crate::doc::frames(2, &mut build);
            testing::click(&mut ui, 10.0, 10.0, &mut build);
            testing::frame(&mut ui, &[], &mut build);
            assert_eq!(last.get().unwrap().focused, focuses, "{p:?}");
            let states = ui.a11y_nodes().iter().find(|n| n.is_role(Role::Button)).unwrap().states;
            assert_eq!(states & state::FOCUSED != 0, focuses, "{p:?}");
            assert_eq!(states & state::FOCUSABLE != 0, p.tab_stop, "{p:?}: a tab stop either way");
        }
    }

    /// The pressable is one node: same count as the hand-written node, signal, within.
    #[test]
    fn node_count_matches_hand_written() {
        let hand = crate::doc::frames(2, &mut |ui| {
            let b = ui.node("p", node().flags(Flags::CLICKABLE | Flags::TAB_STOP));
            let _ = ui.signal(&b);
            ui.within(&b, &mut |ui| { ui.node("c", node()); });
        });
        let sugar = crate::doc::frames(2, &mut |ui| { ui.pressable("p", node(), &P, |ui, _| { ui.node("c", node()); }); });
        assert_eq!(hand.a11y_nodes().len(), sugar.a11y_nodes().len());
        assert!(!sugar.a11y_nodes().is_empty());
    }

    #[test]
    fn warm_frames_allocate_nothing() {
        let mut ui = crate::doc::frames(1, &mut |_| {});
        let looks = States::of(Look::NONE);
        let mut build = |ui: &mut Ui| {
            ui.pressable("p", node(), &P.look(&looks).selected(true), |ui, _| { ui.node("c", node()); });
            let r = ui.pressable(7i64, node(), &P.disabled(true), |_, _| {});
            ui.gesture(&r.handle, &GestureOpts::DEFAULT.long_press());
        };
        for round in 0..20 {
            mem_sys::local_reset();
            testing::frame(&mut ui, &[pointer(Event::POINTER_MOVED)], &mut build);
            if round > 2 { assert_eq!(mem_sys::local_alloc_count(), 0); }
        }
    }
}
