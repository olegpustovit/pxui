//! The kit button with options: `ui.button_with(key, text, &Button)` reports a `Response`; `ui.button(text)` is `Button::DEFAULT`.
use super::{layout::id_text, Key, Response};
use crate::Ui;
use pxui_types::{geom::Edges, layout::Justify, theme::States, Size};

/// How `Ui::button_with` varies the kit's button, as a `Copy` value built with `const fn` builders from `Button::DEFAULT`.
///
/// The default is the plain `Ui::button`: the theme's look, 20 points high, as wide as its text plus 14, 5 points in from the start of its line.
#[derive(Clone, Copy)]
pub struct Button {
    pub(crate) w: Size,
    pub(crate) h: Size,
    pub(crate) look: Option<States>,
    pub(crate) enabled: bool,
    pub(crate) selected: bool,
    pub(crate) danger: bool,
    pub(crate) name: Option<&'static str>,
    pub(crate) align: Justify,
    pub(crate) pad: Edges,
}

impl Button {
    /// The plain kit button.
    pub const DEFAULT: Button = Button { w: Size::Fit, h: Size::Fit, look: None, enabled: true, selected: false, danger: false, name: None, align: Justify::Start, pad: Edges::ZERO };

    /// Sets the box's width and height: `Size::Px` points, `Size::Grow` to fill its line, `Size::Fit` the kit's (text plus 14 wide, 20 high).
    /// `Pct` and `Rel` are refused when the button is declared.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::Size, Button};
    /// const WIDE: Button = Button::DEFAULT.size(Size::Grow, Size::Px(44.0));
    /// let _ = WIDE;
    /// ```
    pub const fn size(mut self, w: Size, h: Size) -> Button { self.w = w; self.h = h; self }

    /// Sets the look per state (idle, hover, pressed, focus, disabled, checked) in place of the theme's button.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::{Look, States}, Button};
    /// const PLATE: States = States::all(Look::NONE);
    /// const B: Button = Button::DEFAULT.look(&PLATE);
    /// let _ = B;
    /// ```
    pub const fn look(mut self, look: &States) -> Button { self.look = Some(*look); self }

    /// Sets whether the button takes input: disabled takes no press or focus, shows the `disabled` look and reads as disabled to
    /// accessibility, and still blocks what is under it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let b = pxui::Button::DEFAULT.enabled(false);
    /// let _ = b;
    /// ```
    pub const fn enabled(mut self, enabled: bool) -> Button { self.enabled = enabled; self }

    /// Marks the button as the current one of a group: the `checked` look and the accessible SELECTED state.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let b = pxui::Button::DEFAULT.selected(true);
    /// let _ = b;
    /// ```
    pub const fn selected(mut self, selected: bool) -> Button { self.selected = selected; self }

    /// Inks the text in the theme's invalid colour in every state but disabled.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const DELETE: pxui::Button = pxui::Button::DEFAULT.danger();
    /// let _ = DELETE;
    /// ```
    pub const fn danger(mut self) -> Button { self.danger = true; self }

    /// Sets the accessible name; the text is the name without it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const B: pxui::Button = pxui::Button::DEFAULT.name("Close dialog");
    /// let _ = B;
    /// ```
    pub const fn name(mut self, name: &'static str) -> Button { self.name = Some(name); self }

    /// Sets where the box sits in its line: `Start` (5 points in), `Center` or `End` (8 points from the end).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::Justify, Button};
    /// const B: Button = Button::DEFAULT.align(Justify::End);
    /// let _ = B;
    /// ```
    pub const fn align(mut self, align: Justify) -> Button { self.align = align; self }

    /// Adds padding inside the box (left, top, right, bottom); with a `Fit` width it widens the box by left and right.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::Edges, Button};
    /// const B: Button = Button::DEFAULT.pad(Edges { l: 4.0, t: 0.0, r: 4.0, b: 0.0 });
    /// let _ = B;
    /// ```
    pub const fn pad(mut self, pad: Edges) -> Button { self.pad = pad; self }
}

impl Ui {
    /// Declares a kit button under the current parent as `button` does, varied by `button`, and reports what happened to it.
    ///
    /// The box is the button's node (role Button, named `button.name` or `text`); a disabled one takes no press or focus. `clicked` is the
    /// press and release inside it or Enter/Space while focused, `activated` the keyboard one.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the display text.
    /// Panics: when the library refuses the arguments (outside a frame, a `Pct` or `Rel` size, a negative padding).
    ///
    /// ```
    /// use pxui::{ui::{Size, Justify}, Button};
    /// let mut clicked = false;
    /// let mut build = |ui: &mut pxui::Ui| {
    ///     let save = Button::DEFAULT.size(Size::Px(90.0), Size::Px(30.0)).align(Justify::Center).selected(true);
    ///     clicked |= ui.button_with("save", "Save", &save).clicked;
    /// };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// pxui::testing::click(&mut ui, 60.0, 12.0, &mut build);
    /// assert!(clicked);
    /// ```
    #[inline]
    #[track_caller]
    pub fn button_with<K: Into<Key>>(&mut self, key: K, text: &str, button: &Button) -> Response {
        self.button_key(key.into(), text, button, None)
    }

    /// Declares a kit button as `button_with` does, with `f` building its contents instead of a text.
    ///
    /// `f` runs inside the box; give the box a size (`Button::size`), since the kit's fit width is that of an empty text, and an
    /// accessible name (`Button::name`): without one the node has none.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the display text.
    /// Panics: as `button_with`, or when `f` panics.
    ///
    /// ```
    /// use pxui::{ui::{Node, Size}, Button};
    /// let mut ui = pxui::doc::frames(1, &mut |ui: &mut pxui::Ui| {
    ///     let pill = Button::DEFAULT.size(Size::Px(90.0), Size::Px(30.0)).name("Play all");
    ///     ui.button_content("play", &pill, |ui| { ui.node("icon", Node { w: Size::Px(16.0), h: Size::Px(16.0), ..Node::DEFAULT }); });
    /// });
    /// assert!(ui.a11y_nodes().iter().any(|n| ui.a11y_name_of(n) == "Play all"));
    /// ```
    #[inline]
    #[track_caller]
    pub fn button_content<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, button: &Button, mut f: F) -> Response {
        self.button_key(key.into(), "", button, Some(&mut f))
    }

    /// The one body of `button_with` and `button_content`: the key's label, then the engine call (`text` is empty for contents).
    #[track_caller]
    fn button_key(&mut self, key: Key, text: &str, button: &Button, content: Option<&mut dyn FnMut(&mut Ui)>) -> Response {
        let mut id_label = [0u8; 17];
        let label = match key { Key::Label(label) => label, Key::Id(id) => id_text(id, &mut id_label) };
        self.button_keyed(label, text, button, content)
    }
}

#[cfg(test)]
mod tests {
    use super::Button;
    use crate::a11y::state;
    use crate::ui::{Edges, Justify, Node, Role, Size};
    use crate::{testing, Response, Ui};
    use std::cell::Cell;

    /// The a11y node named `name` of the last frame: (is a button, states, rect w and h).
    fn node(ui: &Ui, name: &str) -> (bool, u32, f32, f32) {
        let n = ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == name).unwrap_or_else(|| panic!("no node named {name}"));
        (n.is_role(Role::Button), n.states, n.rect.w, n.rect.h)
    }

    /// Clicks the centre of `box_`'s rect and reports whether `clicked` went up.
    fn click_centre(ui: &mut Ui, r: crate::ui::Rect, build: &mut dyn FnMut(&mut Ui)) { testing::click(ui, f64::from(r.x + r.w / 2.0), f64::from(r.y + r.h / 2.0), build); }

    /// Settings' and the compact lists' text button: full width, 44 high, named, selected when current.
    #[test]
    fn text_button_fills_its_line_with_a_selected_state() {
        let hits = Cell::new(0);
        let last: Cell<Option<Response>> = Cell::new(None);
        let mut build = |ui: &mut Ui| {
            let b = Button::DEFAULT.size(Size::Grow, Size::Px(44.0)).selected(true).name("Songs tab");
            let r = ui.button_with("songs", "Songs", &b);
            if r.clicked { hits.set(hits.get() + 1); }
            last.set(Some(r));
        };
        let mut ui = crate::doc::frames(3, &mut build);
        let (is_button, states, w, h) = node(&ui, "Songs tab");
        assert!(is_button && states & state::SELECTED != 0 && states & state::ENABLED != 0 && states & state::FOCUSABLE != 0);
        let view_w = ui.view_size().0;
        assert_eq!(h, 44.0);
        assert!(w > 0.8 * view_w, "the box fills its line: {w} of {view_w}");
        let r = ui.rect(&last.get().unwrap().handle);
        click_centre(&mut ui, r, &mut build);
        assert_eq!(hits.get(), 1);
    }

    /// The Files screen's row button: fixed size, disabled takes no press, no focus and reads as disabled.
    #[test]
    fn row_button_disabled_blocks_input() {
        let enabled = Cell::new(false);
        let hits = Cell::new(0);
        let last: Cell<Option<Response>> = Cell::new(None);
        let mut build = |ui: &mut Ui| {
            let b = Button::DEFAULT.size(Size::Px(120.0), Size::Px(36.0)).enabled(enabled.get()).align(Justify::Center);
            let r = ui.button_with(7i64, "Choose", &b);
            if r.clicked { hits.set(hits.get() + 1); }
            last.set(Some(r));
        };
        let mut ui = crate::doc::frames(3, &mut build);
        let (is_button, states, w, h) = node(&ui, "Choose");
        assert!(is_button && (w, h) == (120.0, 36.0));
        assert!(states & state::ENABLED == 0 && states & state::FOCUSABLE == 0, "disabled reads as disabled: {states:#x}");
        let r = ui.rect(&last.get().unwrap().handle);
        click_centre(&mut ui, r, &mut build);
        assert_eq!(hits.get(), 0);
        enabled.set(true);
        testing::frame(&mut ui, &[], &mut build);
        testing::frame(&mut ui, &[], &mut build);
        let (_, states, _, _) = node(&ui, "Choose");
        assert!(states & state::ENABLED != 0 && states & state::FOCUSABLE != 0);
        let r = ui.rect(&last.get().unwrap().handle);
        click_centre(&mut ui, r, &mut build);
        assert_eq!(hits.get(), 1);
    }

    /// Play-all's pill: a fixed 30 high box with its own contents (an icon node) and an accessible name.
    #[test]
    fn pill_builds_its_own_contents() {
        let last: Cell<Option<Response>> = Cell::new(None);
        let inner = Cell::new(None);
        let mut build = |ui: &mut Ui| {
            let b = Button::DEFAULT.size(Size::Px(90.0), Size::Px(30.0)).name("Play all").pad(Edges { l: 2.0, ..Edges::ZERO });
            last.set(Some(ui.button_content("play-all", &b, |ui| { inner.set(Some(ui.node("icon", Node { w: Size::Px(16.0), h: Size::Px(16.0), ..Node::DEFAULT }))); })));
        };
        let ui = crate::doc::frames(3, &mut build);
        let (is_button, _, w, h) = node(&ui, "Play all");
        assert!(is_button && (w, h) == (90.0, 30.0));
        let (outer, icon) = (ui.rect(&last.get().unwrap().handle), ui.rect(&inner.get().unwrap()));
        assert!(icon.x >= outer.x && icon.x + icon.w <= outer.x + outer.w && icon.w == 16.0, "the icon is inside the box: {outer:?} {icon:?}");
    }

    /// `button(text)` is `button_with("button", text, &Button::DEFAULT)` and takes its identity from its position: two of them are
    /// two nodes, and each reports its own click.
    #[test]
    fn button_forwards_to_button_with_and_keys_by_position() {
        let hits = Cell::new((false, false));
        let mut one = |ui: &mut Ui| { let a = ui.button("First"); let b = ui.button("Second"); hits.set((hits.get().0 | a, hits.get().1 | b)); };
        let mut two = |ui: &mut Ui| { ui.button_with("button", "First", &Button::DEFAULT); };
        let mut ui_one = crate::doc::frames(3, &mut one);
        let ui_two = crate::doc::frames(3, &mut two);
        let first = *ui_one.a11y_nodes().iter().find(|n| ui_one.a11y_name_of(n) == "First").unwrap();
        let second = *ui_one.a11y_nodes().iter().find(|n| ui_one.a11y_name_of(n) == "Second").unwrap();
        let twin = ui_two.a11y_nodes().iter().find(|n| ui_two.a11y_name_of(n) == "First").unwrap();
        assert_eq!((first.id, first.rect.w, first.rect.h, first.states), (twin.id, twin.rect.w, twin.rect.h, twin.states));
        assert_ne!(first.id, second.id);
        assert_eq!(first.rect.h, 20.0);
        let at = second.rect;
        testing::click(&mut ui_one, f64::from(at.x + at.w / 2.0), f64::from(at.y + at.h / 2.0), &mut one);
        assert_eq!(hits.get(), (false, true));
    }

    #[test]
    fn warm_frames_allocate_nothing() {
        let content = |ui: &mut Ui| { ui.node("i", Node { w: Size::Px(8.0), h: Size::Px(8.0), ..Node::DEFAULT }); };
        let states = crate::doc::frames(1, &mut |_| {}).theme().button;
        let mut ui = crate::doc::frames(1, &mut |_| {});
        let mut build = |ui: &mut Ui| {
            ui.button("Go");
            ui.button_content(1i64, &Button::DEFAULT.size(Size::Px(90.0), Size::Px(30.0)).look(&states).danger(), &content);
            ui.button_with("off", "Off", &Button::DEFAULT.enabled(false).selected(true).name("Off button").align(Justify::End));
        };
        for round in 0..20 {
            mem_sys::local_reset();
            testing::frame(&mut ui, &[], &mut build);
            if round > 2 { assert_eq!(mem_sys::local_alloc_count(), 0); }
        }
    }
}
