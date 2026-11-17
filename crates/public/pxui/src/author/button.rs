//! The kit button with options: `ui.button_with(key, text, &Button)` reports a `Response`; `ui.button(text)` is `Button::DEFAULT`.
use super::{Key, Response};
use crate::Ui;
use pxui_types::{geom::Edges, layout::Justify, theme::{BodyKind, States}, Size};

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
    pub(crate) variant: u16,
    pub(crate) centered: bool,
}

impl Button {
    /// The plain kit button.
    pub const DEFAULT: Button = Button { w: Size::Fit, h: Size::Fit, look: None, enabled: true, selected: false, danger: false, name: None, align: Justify::Start, pad: Edges::ZERO, variant: 0, centered: false };

    /// A button exactly `w` by `h` points with its text centred in it: exactly `Button::DEFAULT.size(Size::Px(w), Size::Px(h)).centered()`.
    /// Allocates: never.
    /// Panics: never (a negative or non-finite size is refused when the button is declared).
    ///
    /// ```
    /// const ACTION: pxui::Button = pxui::Button::sized(120.0, 44.0);
    /// let _ = ACTION;
    /// ```
    pub const fn sized(w: f32, h: f32) -> Button { Button::DEFAULT.size(Size::Px(w), Size::Px(h)).centered() }

    /// Makes the box exactly its size (`size`) with nothing around it in its line and its text centred across and down on a node of
    /// its own, as wide as the text in whole pixels (a tab, a sheet's action); `align` still places the box in a line wider than it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::Size, Button};
    /// const TAB: Button = Button::DEFAULT.size(Size::Grow, Size::Px(44.0)).centered();
    /// let _ = TAB;
    /// ```
    pub const fn centered(mut self) -> Button { self.centered = true; self }

    /// Sets the box's width and height: `Size::Px` points, `Size::Grow` to fill its line, `Size::Fit` the kit's (text plus 14 wide, 20 high).
    /// `Pct` and `Rel` are refused when the button is declared. It leaves `centered` as it is.
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

    /// Sets the variant a registered body reads from its config (`variant`, 0 by default): one body builds different structures by it, so the app
    /// picks per call without comparing text. The built-in body ignores it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const PILL: pxui::Button = pxui::Button::DEFAULT.variant(1);
    /// let _ = PILL;
    /// ```
    pub const fn variant(mut self, variant: u16) -> Button { self.variant = variant; self }
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
    ///     ui.button_content("play", &pill, |ui| { ui.node("icon".into(), Node { w: Size::Px(16.0), h: Size::Px(16.0), ..Node::DEFAULT }); });
    /// });
    /// assert!(ui.a11y_nodes().iter().any(|n| ui.a11y_name_of(n) == "Play all"));
    /// ```
    #[inline]
    #[track_caller]
    pub fn button_content<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, button: &Button, mut f: F) -> Response {
        self.button_key(key.into(), "", button, Some(&mut f))
    }

    /// The one body of `button_with` and `button_content`: the key's label, then the engine call (`text` is empty for contents).
    #[inline(always)]
    #[track_caller]
    fn button_key(&mut self, key: Key, text: &str, button: &Button, content: Option<&mut dyn FnMut(&mut Ui)>) -> Response {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        if button.variant == 0 { return self.button_keyed(label, text, button, content); }
        self.set_variant(BodyKind::button, button.variant);
        // the caller's contents are not part of the button: widgets built in them are given no variant
        let response = match content {
            Some(f) => self.button_keyed(label, text, button, Some(&mut |ui: &mut Ui| { ui.set_variant(BodyKind::button, 0); f(ui) })),
            None => self.button_keyed(label, text, button, None),
        };
        self.set_variant(BodyKind::button, 0);
        response
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
            last.set(Some(ui.button_content("play-all", &b, |ui| { inner.set(Some(ui.node("icon".into(), Node { w: Size::Px(16.0), h: Size::Px(16.0), ..Node::DEFAULT }))); })));
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

    /// A `centered` button is its box: exactly 120 by 44 (`sized`) or its bar's width by 44, nothing around it in its line, the text
    /// centred across and down, the fill of its look over the whole box.
    #[test]
    fn centered_button_is_its_box_with_the_text_centred() {
        use crate::ui::{Look, States};
        let red = crate::geom::Rgba { r: 200, g: 0, b: 0, a: 255 };
        let plate = States::all(Look { fill: red, text: crate::geom::Rgba { r: 1, g: 2, b: 3, a: 255 }, ..Look::NONE });
        let (action, tab) = (Button::sized(120.0, 44.0).look(&plate), Button::DEFAULT.size(Size::Grow, Size::Px(44.0)).centered().look(&plate));
        let last: Cell<Option<Response>> = Cell::new(None);
        let mut build = |ui: &mut Ui| {
            last.set(Some(ui.button_with("action", "Save", &action)));
            ui.row("bar", Node { w: Size::Px(200.0), ..Node::DEFAULT }, |ui| { ui.button_with("tab", "Songs", &tab); });
        };
        let ui = crate::doc::frames(3, &mut build);
        let (_, _, w, h) = node(&ui, "Save");
        assert_eq!((w, h), (120.0, 44.0));
        let box_ = ui.rect(&last.get().unwrap().handle);
        let (_, _, w, h) = node(&ui, "Songs");
        assert_eq!((w, h), (200.0, 44.0), "the tab fills its bar");
        let fills: Vec<[f32; 4]> = ui.layers().flat_map(|l| l.instances.iter()).filter(|i| i.color == [200, 0, 0, 255]).map(|i| i.rect).collect();
        assert!(fills.contains(&[box_.x, box_.y, box_.x + 120.0, box_.y + 44.0]), "the plate is the whole box: {box_:?} {fills:?}");
        let ink: Vec<[f32; 4]> = ui.layers().flat_map(|l| l.instances.iter()).filter(|i| i.color[..3] == [1, 2, 3] && i.rect[1] < box_.y + box_.h).map(|i| i.rect).collect();
        let (l, t, r, b) = ink.iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |(l, t, r, b), g| (l.min(g[0]), t.min(g[1]), r.max(g[2]), b.max(g[3])));
        assert!(!ink.is_empty() && ((l + r) / 2.0 - (box_.x + 60.0)).abs() <= 2.0 && ((t + b) / 2.0 - (box_.y + 22.0)).abs() <= 3.0, "the text centred: {l} {t} {r} {b} in {box_:?}");
    }

    thread_local! { static SIZED: Cell<(u8, u8, f32, f32)> = const { Cell::new((9, 9, 0.0, 0.0)) }; }

    /// A body that records the size kinds and `centered` its config carries.
    fn recording(_ui: &mut Ui, c: &crate::bodies::PxuiButtonConfig) {
        assert!(c.size as usize >= core::mem::size_of::<crate::bodies::PxuiButtonConfig>(), "this library carries `centered`");
        SIZED.with(|s| s.set((c.centered, c.w_kind, c.w, c.h)));
    }

    /// A registered button body is given `centered` in its config.
    #[test]
    fn a_registered_body_is_given_centered() {
        let mut ui = Ui::new().unwrap();
        let id = ui.register_button_body("recording", recording).expect("registered");
        let text = "button.body = recording";
        ui.set_theme(&pxui_types::ffi::ThemeSource { size: core::mem::size_of::<pxui_types::ffi::ThemeSource>() as u32, flags: 0, source: pxui_types::Str { ptr: text.as_ptr(), len: text.len() } }).unwrap();
        assert_eq!(ui.theme().body.button, id);
        for (button, seen) in [(Button::sized(120.0, 44.0), (1, 1, 120.0, 44.0)), (Button::DEFAULT.size(Size::Px(120.0), Size::Px(44.0)), (0, 1, 120.0, 44.0))] {
            testing::frame(&mut ui, &[], &mut |ui| { ui.button_with("b", "Go", &button); });
            assert_eq!(SIZED.with(|s| s.get()), seen);
        }
    }

    #[test]
    fn warm_frames_allocate_nothing() {
        let content = |ui: &mut Ui| { ui.node("i".into(), Node { w: Size::Px(8.0), h: Size::Px(8.0), ..Node::DEFAULT }); };
        let states = crate::doc::frames(1, &mut |_| {}).theme().button;
        let mut ui = crate::doc::frames(1, &mut |_| {});
        let mut build = |ui: &mut Ui| {
            ui.button("Go");
            ui.button_content(1i64, &Button::DEFAULT.size(Size::Px(90.0), Size::Px(30.0)).look(&states).danger(), &content);
            ui.button_with("off", "Off", &Button::DEFAULT.enabled(false).selected(true).name("Off button").align(Justify::End));
            ui.button_with("tab", "Songs", &Button::DEFAULT.size(Size::Grow, Size::Px(44.0)).centered().selected(true));
        };
        for round in 0..20 {
            mem_sys::local_reset();
            testing::frame(&mut ui, &[], &mut build);
            if round > 2 { assert_eq!(mem_sys::local_alloc_count(), 0); }
        }
    }
}
