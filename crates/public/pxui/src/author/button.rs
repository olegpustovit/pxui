//! The kit button with options: `ui.button_with(key, text, &Button)` reports a `Response`; `ui.button(text)` is `Button::DEFAULT`.
use super::{Key, Response};
use crate::Ui;
use pxui_types::{geom::Edges, layout::{Align, Justify}, theme::{BodyKind, State, States}, Axis, Label, Size};

/// How `Ui::button_with` varies the kit's button, as a `Copy` value built with `const fn` builders from `Button::DEFAULT`.
///
/// The default is the plain `Ui::button`: the theme's look, 20 points high, as wide as its text plus 14, 5 points in from the start of its line.
/// `'a` is the borrow of a `name` given at run time (`'static` for a const button).
#[derive(Clone, Copy)]
pub struct Button<'a> {
    pub(crate) w: Size,
    pub(crate) h: Size,
    pub(crate) look: Option<States>,
    pub(crate) enabled: bool,
    pub(crate) selected: bool,
    pub(crate) danger: bool,
    pub(crate) name: Option<&'a str>,
    pub(crate) align: Justify,
    pub(crate) pad: Edges,
    pub(crate) variant: u16,
    pub(crate) centered: bool,
    pub(crate) label: Option<Label>,
    pub(crate) axis: Option<Axis>,
    pub(crate) gap: f32,
    pub(crate) content_align: Option<Align>,
    pub(crate) focus_on_click: bool,
}

impl<'a> Button<'a> {
    /// The plain kit button.
    pub const DEFAULT: Button<'a> = Button { w: Size::Fit, h: Size::Fit, look: None, enabled: true, selected: false, danger: false, name: None, align: Justify::Start, pad: Edges::ZERO, variant: 0, centered: false,
        label: None, axis: None, gap: 0.0, content_align: None, focus_on_click: false };

    /// A button exactly `w` by `h` points with its text centred in it: exactly `Button::DEFAULT.size(Size::Px(w), Size::Px(h)).centered()`.
    /// Allocates: never.
    /// Panics: never (a negative or non-finite size is refused when the button is declared).
    ///
    /// ```
    /// const ACTION: pxui::Button = pxui::Button::sized(120.0, 44.0);
    /// let _ = ACTION;
    /// ```
    pub const fn sized(w: f32, h: f32) -> Button<'a> { Button::DEFAULT.size(Size::Px(w), Size::Px(h)).centered() }

    /// Makes the box exactly its size (`size`) with nothing around it in its line and its text centred across and down on a node of
    /// its own, as wide as the text in whole pixels (a tab, a sheet's action); `align` still places the box in a line wider than it.
    /// `content_align` overrides the centring across the box (down it the contents stay centred).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::Size, Button};
    /// const TAB: Button = Button::DEFAULT.size(Size::Grow, Size::Px(44.0)).centered();
    /// let _ = TAB;
    /// ```
    pub const fn centered(mut self) -> Button<'a> { self.centered = true; self }

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
    pub const fn size(mut self, w: Size, h: Size) -> Button<'a> { self.w = w; self.h = h; self }

    /// Sets the look per state (idle, hover, pressed, focus, disabled, checked) in place of the theme's button. Its font draws the
    /// kit's text; to style the text (size, weight, line box) use `label`, which then takes only the look's text colour.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::{Look, States}, Button};
    /// const PLATE: States = States::all(Look::NONE);
    /// const B: Button = Button::DEFAULT.look(&PLATE);
    /// let _ = B;
    /// ```
    pub const fn look(mut self, look: &States) -> Button<'a> { self.look = Some(*look); self }

    /// Sets whether the button takes input: disabled takes no press or focus, shows the `disabled` look and reads as disabled to
    /// accessibility (its node lacks the ENABLED and FOCUSABLE states), and still blocks what is under it. A disabled button never
    /// reports `clicked` or `activated`: not for a press begun while it was enabled, nor for an assistive technology's press.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let b = pxui::Button::DEFAULT.enabled(false);
    /// let _ = b;
    /// ```
    pub const fn enabled(mut self, enabled: bool) -> Button<'a> { self.enabled = enabled; self }

    /// Marks the button as the current one of a group: the `checked` look and the accessible SELECTED state.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let b = pxui::Button::DEFAULT.selected(true);
    /// let _ = b;
    /// ```
    pub const fn selected(mut self, selected: bool) -> Button<'a> { self.selected = selected; self }

    /// Inks the text in the theme's invalid colour in every state but disabled.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const DELETE: pxui::Button = pxui::Button::DEFAULT.danger();
    /// let _ = DELETE;
    /// ```
    pub const fn danger(mut self) -> Button<'a> { self.danger = true; self }

    /// Sets the accessible name; the text is the name without it. The name is borrowed for the button's life: a name made at run time
    /// (a song's title) needs no copy.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const B: pxui::Button = pxui::Button::DEFAULT.name("Close dialog");
    /// let title = String::from("Play Blue in Green");
    /// let row = pxui::Button::DEFAULT.name(&title);
    /// let _ = (B, row);
    /// ```
    pub const fn name(mut self, name: &'a str) -> Button<'a> { self.name = Some(name); self }

    /// Sets where the box sits in its line: `Start` (5 points in), `Center` or `End` (8 points from the end).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::Justify, Button};
    /// const B: Button = Button::DEFAULT.align(Justify::End);
    /// let _ = B;
    /// ```
    pub const fn align(mut self, align: Justify) -> Button<'a> { self.align = align; self }

    /// Adds padding inside the box (left, top, right, bottom); with a `Fit` width it widens the box by left and right. It insets the
    /// box's contents; a `label`'s own `pad` and width are the text node's, inside that padding.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::Edges, Button};
    /// const B: Button = Button::DEFAULT.pad(Edges { l: 4.0, t: 0.0, r: 4.0, b: 0.0 });
    /// let _ = B;
    /// ```
    pub const fn pad(mut self, pad: Edges) -> Button<'a> { self.pad = pad; self }

    /// Sets the variant a registered body reads from its config (`variant`, 0 by default): one body builds different structures by it, so the app
    /// picks per call without comparing text. The built-in body ignores it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const PILL: pxui::Button = pxui::Button::DEFAULT.variant(1);
    /// let _ = PILL;
    /// ```
    pub const fn variant(mut self, variant: u16) -> Button<'a> { self.variant = variant; self }

    /// Draws the button's text through the label line box in `style`, as `Ui::label` draws it (its size, weight, `h`, `line_height`,
    /// `whole_pixels` apply), in place of the kit's text: an unset width is the text's own, an unset pad none and an unset colour the
    /// look's text colour for the button's state. The text sits on a node of its own placed by `content_align`.
    ///
    /// Once a label is set the look's font is not used: the label's style is the text's, and the look gives only its per-state text
    /// colour (the theme's invalid ink with `danger`) when the label sets no colour. The label's `pad` and width are the text node's
    /// (its inset and width inside the box); the box's are `Button::pad` and `size`.
    /// Allocates: never.
    /// Panics: never (a style that does not resolve is refused when the button is declared).
    ///
    /// ```
    /// use pxui::{Button, Label};
    /// const ROW: Button = Button::sized(200.0, 32.0).label(Label::BODY.size(14.0).h_px(19.6)).content_align(pxui::ui::Align::Start);
    /// let _ = ROW;
    /// ```
    pub const fn label(mut self, style: Label) -> Button<'a> { self.label = Some(style); self }

    /// Sets the axis the box lays its contents along (the text, or what `Ui::button_content` builds): `Row` side by side, `Column`
    /// one under the other, `Stack` on top of each other. Unset, the kit's: the text alone, the contents in a column.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::Axis, Button};
    /// const TILE: Button = Button::sized(64.0, 64.0).axis(Axis::Column).gap(4.0);
    /// let _ = TILE;
    /// ```
    pub const fn axis(mut self, axis: Axis) -> Button<'a> { self.axis = Some(axis); self }

    /// Sets the space between the box's contents in points (0 by default).
    /// Allocates: never.
    /// Panics: never (a negative or non-finite gap is refused when the button is declared).
    ///
    /// ```
    /// const PILL: pxui::Button = pxui::Button::sized(90.0, 30.0).gap(6.0);
    /// let _ = PILL;
    /// ```
    pub const fn gap(mut self, gap: f32) -> Button<'a> { self.gap = gap; self }

    /// Sets where the contents sit across the box: `Start`, `Center` or `End` (along a row, across a column); down the box they are
    /// centred. Unset, the kit's: centred in a `centered` button, at the start otherwise. `align` places the box itself in its line.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::Align, Button};
    /// const ACTION: Button = Button::sized(200.0, 28.0).content_align(Align::Start);
    /// let _ = ACTION;
    /// ```
    pub const fn content_align(mut self, align: Align) -> Button<'a> { self.content_align = Some(align); self }

    /// Sets whether a press focuses the button (`false` by default: only Tab focuses it, so a click leaves the focus where it was).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const FILE: pxui::Button = pxui::Button::sized(120.0, 36.0).focus_on_click(true);
    /// let _ = FILE;
    /// ```
    pub const fn focus_on_click(mut self, focus: bool) -> Button<'a> { self.focus_on_click = focus; self }
}

impl Ui {
    /// Declares a kit button under the current parent as `button` does, varied by `button`, and reports what happened to it.
    ///
    /// The box is the button's node (role Button, named `button.name` or `text`); a disabled one takes no press or focus. `clicked` is the
    /// press and release inside it or Enter/Space while focused, `activated` the keyboard one.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the display text.
    /// Panics: when the library refuses the arguments (outside a frame, a `Pct` or `Rel` size, a negative padding). A key or identity label over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::{ui::{Size, Justify}, Button};
    /// use std::cell::Cell;
    /// let (clicked, node) = (Cell::new(false), Cell::new(None));
    /// let mut build = |ui: &mut pxui::Ui| {
    ///     let save = Button::DEFAULT.size(Size::Px(90.0), Size::Px(30.0)).align(Justify::Center).selected(true);
    ///     let response = ui.button_with("save", "Save", &save);
    ///     clicked.set(clicked.get() | response.clicked);
    ///     node.set(Some(response.handle));
    /// };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// pxui::testing::click_node(&mut ui, &node.get().unwrap(), &mut build);
    /// assert!(clicked.get());
    /// ```
    #[inline]
    #[track_caller]
    pub fn button_with<K: Into<Key>>(&mut self, key: K, text: &str, button: &Button<'_>) -> Response {
        self.button_key(key.into(), text, button, None)
    }

    /// Declares a kit button as `button_with` does, with `f` building its contents instead of a text.
    ///
    /// `f` runs inside the box, laid out by `Button::axis`, `gap` and `content_align`, and is given the button's interaction state
    /// this frame by value (hovered, pressed, focused, disabled, checked): a snapshot of what input did to it, read before `f` runs,
    /// so the contents can follow the press (`states.pick(state)`). A disabled button's contents get `disabled: true`. Give the box a size (`Button::size`), since the kit's fit width is that of an empty text, and an
    /// accessible name (`Button::name`): without one the node has none.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the display text.
    /// Panics: as `button_with`, or when `f` panics. A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// use pxui::{ui::{Node, Size}, Button};
    /// let mut ui = pxui::doc::frames(1, &mut |ui: &mut pxui::Ui| {
    ///     let pill = Button::DEFAULT.size(Size::Px(90.0), Size::Px(30.0)).name("Play all");
    ///     ui.button_content("play", &pill, |ui, state| {
    ///         let w = if state.pressed { 14.0 } else { 16.0 };
    ///         ui.node("icon".into(), Node { w: Size::Px(w), h: Size::Px(w), ..Node::DEFAULT });
    ///     });
    /// });
    /// assert!(ui.a11y_nodes().iter().any(|n| ui.a11y_name_of(n) == "Play all"));
    /// ```
    #[inline]
    #[track_caller]
    pub fn button_content<K: Into<Key>, F: FnMut(&mut Ui, State)>(&mut self, key: K, button: &Button<'_>, mut f: F) -> Response {
        self.button_key(key.into(), "", button, Some(&mut f))
    }

    /// The one body of `button_with` and `button_content`: the key's label, then the engine call (`text` is empty for contents).
    #[inline(always)]
    #[track_caller]
    fn button_key(&mut self, key: Key, text: &str, button: &Button<'_>, content: Option<&mut dyn FnMut(&mut Ui, State)>) -> Response {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        if button.variant == 0 { return self.button_keyed(label, text, button, content); }
        self.set_variant(BodyKind::button, button.variant);
        // the caller's contents are not part of the button: widgets built in them are given no variant
        let response = match content {
            Some(f) => self.button_keyed(label, text, button, Some(&mut |ui: &mut Ui, state: State| { ui.set_variant(BodyKind::button, 0); f(ui, state) })),
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
            last.set(Some(ui.button_content("play-all", &b, |ui, _| { inner.set(Some(ui.node("icon".into(), Node { w: Size::Px(16.0), h: Size::Px(16.0), ..Node::DEFAULT }))); })));
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
    fn recording(_ui: &mut Ui, c: &crate::bodies::ButtonConfig) {
        assert!(c.size as usize >= core::mem::size_of::<crate::bodies::ButtonConfig>(), "this library carries `centered`");
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

    /// The accessible node named `name` with `role` in the last frame.
    fn a11y(ui: &Ui, role: Role, name: &str) -> crate::A11yNode {
        *ui.a11y_nodes().iter().find(|n| n.is_role(role) && ui.a11y_name_of(n) == name).unwrap_or_else(|| panic!("no {role:?} named {name}"))
    }

    /// An assistive technology's Press on each of `names` (role, name), delivered in the next frame.
    fn press_a11y(ui: &mut Ui, names: &[(Role, &str)], build: &mut dyn FnMut(&mut Ui)) {
        let actions: Vec<crate::A11yAction> = names.iter().map(|(role, name)| crate::A11yAction { id: a11y(ui, *role, name).id, action_code: 1, ..Default::default() }).collect();
        ui.inject_a11y(&actions);
        testing::frame(ui, &[], build);
    }

    /// A disabled button, icon button, toggle and segmented option never report a click: not for an assistive technology's Press (on a
    /// node shown disabled, or on one disabled in the frame the press arrives), nor a pointer press begun while it was enabled, nor
    /// Space while it kept the focus. The same presses on the enabled widgets do report.
    #[test]
    fn disabled_widgets_never_report_a_click() {
        use crate::{Icon, IconButton, Segmented, Toggle};
        let enabled = Cell::new(true);
        let hits = Cell::new([0u32; 4]);
        let mut build = |ui: &mut Ui| {
            let (on, mut h) = (enabled.get(), hits.get());
            let b = ui.button_with("go", "Go", &Button::sized(80.0, 30.0).enabled(on));
            h[0] += (b.clicked || b.activated) as u32;
            let i = ui.icon_button("x", Icon::Glyph('x'), "Close", &IconButton::sized(30.0, 30.0).enabled(on));
            h[1] += (i.clicked || i.activated) as u32;
            h[2] += ui.toggle_with("t", "Wifi", false, &Toggle::DEFAULT.enabled(on)).changed as u32;
            h[3] += (ui.segmented_with("s", &[crate::Segment::new("A"), crate::Segment::new("B")], Some(0), &Segmented::DEFAULT.enabled(on)).chosen != Some(0)) as u32;
            hits.set(h);
        };
        let all = [(Role::Button, "Go"), (Role::Button, "Close"), (Role::Toggle, "Wifi"), (Role::Radio, "B")];
        let mut ui = crate::doc::frames(3, &mut build);
        press_a11y(&mut ui, &all, &mut build);
        assert_eq!(hits.get(), [1; 4], "enabled widgets take the press");
        // shown disabled: the press is dropped before the frame
        (enabled.set(false), hits.set([0; 4]));
        testing::frame(&mut ui, &[], &mut build);
        press_a11y(&mut ui, &all, &mut build);
        assert_eq!(hits.get(), [0; 4], "a node shown disabled takes no press");
        // enabled when the press was made, disabled in the frame it arrives
        enabled.set(true);
        testing::frame(&mut ui, &[], &mut build);
        let actions: Vec<crate::A11yAction> = all.iter().map(|(role, name)| crate::A11yAction { id: a11y(&ui, *role, name).id, action_code: 1, ..Default::default() }).collect();
        ui.inject_a11y(&actions);
        enabled.set(false);
        testing::frame(&mut ui, &[], &mut build);
        assert_eq!(hits.get(), [0; 4], "a widget disabled in the press's frame reports none");
        // a pointer press begun while enabled, released once disabled
        for (role, name) in all {
            enabled.set(true);
            testing::frame(&mut ui, &[], &mut build);
            let r = a11y(&ui, role, name).rect;
            let at = |kind| [crate::Event { kind, x: f64::from(r.x + r.w / 2.0), y: f64::from(r.y + r.h / 2.0), ..Default::default() }];
            testing::frame(&mut ui, &at(crate::Event::POINTER_PRESSED), &mut build);
            enabled.set(false);
            testing::frame(&mut ui, &at(crate::Event::POINTER_RELEASED), &mut build);
        }
        assert_eq!(hits.get(), [0; 4], "a release on a widget disabled since the press reports none");
        // Space on the focused button and icon button once they are disabled (Tab focuses the button, then the icon button)
        let (tab, space) = (pxui_types::KEY_SPECIAL + 8, pxui_types::KEY_SPECIAL + 4);
        for name in ["Go", "Close"] {
            enabled.set(true);
            testing::frame(&mut ui, &[], &mut build);
            for _ in 0..6 {
                if a11y(&ui, Role::Button, name).states & state::FOCUSED != 0 { break; }
                testing::key(&mut ui, tab, &mut build);
                testing::frame(&mut ui, &[], &mut build);
            }
            assert!(a11y(&ui, Role::Button, name).states & state::FOCUSED != 0, "{name} has the focus");
            enabled.set(false);
            testing::key(&mut ui, space, &mut build);
        }
        assert_eq!(hits.get(), [0; 4], "a key on a widget disabled while focused reports none");
        enabled.set(true);
        testing::frame(&mut ui, &[], &mut build);
        testing::key(&mut ui, space, &mut build);
        assert_eq!(hits.get()[1], 1, "the same key on the enabled icon button does");
    }

    /// `Button::label`: the text is drawn through the label line box (its `h` is the text node's height), inked in the look's text
    /// colour, and `content_align` puts it at the start, centre or end of the box.
    #[test]
    fn label_text_is_placed_by_content_align() {
        use crate::ui::{Align, Look, States};
        use crate::Label;
        let ink = crate::geom::Rgba { r: 1, g: 2, b: 3, a: 255 };
        let plate = States::all(Look { text: ink, ..Look::NONE });
        let align = Cell::new(Align::Start);
        let centered = Cell::new(true);
        let last: Cell<Option<Response>> = Cell::new(None);
        let mut build = |ui: &mut Ui| {
            let b = if centered.get() { Button::sized(200.0, 32.0) } else { Button::DEFAULT.size(Size::Px(200.0), Size::Px(32.0)) };
            last.set(Some(ui.button_with("save", "Save", &b.look(&plate).label(Label::BODY.size(14.0).h_px(20.0)).content_align(align.get()))));
        };
        let mut ui = crate::doc::frames(2, &mut build);
        let text = a11y(&ui, Role::Label, "Save");
        assert_eq!(text.rect.h, 20.0, "the label's own box height");
        // (left, right) of the ink inside the box
        let span = |ui: &Ui, box_: crate::ui::Rect| {
            let ink: Vec<[f32; 4]> = ui.layers().flat_map(|l| l.instances.iter()).filter(|i| i.color[..3] == [1, 2, 3] && i.rect[1] < box_.y + box_.h).map(|i| i.rect).collect();
            assert!(!ink.is_empty(), "the text is inked in the look's text colour");
            ink.iter().fold((f32::MAX, f32::MIN), |(l, r), g| (l.min(g[0]), r.max(g[2])))
        };
        for c in [true, false] {
            centered.set(c);
            // the kit's side inset of a button that is not centred
            let side = if c { 0.0 } else { 7.0 };
            for (a, check) in [(Align::Start, 0), (Align::Center, 1), (Align::End, 2)] {
                align.set(a);
                testing::frame(&mut ui, &[], &mut build);
                testing::frame(&mut ui, &[], &mut build);
                let box_ = ui.rect(&last.get().unwrap().handle);
                let (l, r) = span(&ui, box_);
                match check {
                    0 => assert!((l - box_.x - side).abs() <= 2.0, "start ({c}): {l} in {box_:?}"),
                    1 => assert!(((l + r) / 2.0 - (box_.x + box_.w / 2.0)).abs() <= 2.0, "centre ({c}): {l}..{r} in {box_:?}"),
                    _ => assert!((box_.x + box_.w - side - r).abs() <= 2.0, "end ({c}): {r} in {box_:?}"),
                }
            }
        }
    }

    /// `button_content` lays its contents out by `axis` and `gap` (a column 4 apart, centred across) and is given the button's state:
    /// pressed while held, checked when selected, disabled when disabled.
    #[test]
    fn content_is_laid_out_and_given_the_state() {
        use crate::ui::{Axis, State};
        let seen = Cell::new(State::default());
        let (selected, enabled) = (Cell::new(false), Cell::new(true));
        let nodes: Cell<Option<(crate::Handle, crate::Handle, Response)>> = Cell::new(None);
        let mut build = |ui: &mut Ui| {
            let tile = Button::sized(100.0, 60.0).axis(Axis::Column).gap(4.0).name("Tile").selected(selected.get()).enabled(enabled.get());
            let mut inner = None;
            let r = ui.button_content("tile", &tile, |ui, state| {
                seen.set(state);
                let a = ui.node("a".into(), Node { w: Size::Px(10.0), h: Size::Px(10.0), ..Node::DEFAULT });
                let b = ui.node("b".into(), Node { w: Size::Px(20.0), h: Size::Px(10.0), ..Node::DEFAULT });
                inner = Some((a, b));
            });
            let (a, b) = inner.unwrap();
            nodes.set(Some((a, b, r)));
        };
        let mut ui = crate::doc::frames(3, &mut build);
        let (a, b, r) = nodes.get().unwrap();
        let (box_, a, b) = (ui.rect(&r.handle), ui.rect(&a), ui.rect(&b));
        assert_eq!(b.y - a.y, 14.0, "a column 4 apart");
        assert!((a.x + 5.0 - (box_.x + 50.0)).abs() <= 0.5 && (b.x + 10.0 - (box_.x + 50.0)).abs() <= 0.5, "centred across: {a:?} {b:?} in {box_:?}");
        assert!(((a.y + b.y + b.h) / 2.0 - (box_.y + 30.0)).abs() <= 0.5, "centred down: {a:?} {b:?} in {box_:?}");
        assert!(!seen.get().pressed && !seen.get().checked && !seen.get().disabled, "at rest: {:?}", seen.get());
        let at = [crate::Event { kind: crate::Event::POINTER_PRESSED, x: f64::from(box_.x + 50.0), y: f64::from(box_.y + 30.0), ..Default::default() }];
        testing::frame(&mut ui, &at, &mut build);
        assert!(seen.get().pressed && seen.get().hovered, "pressed while held: {:?}", seen.get());
        testing::frame(&mut ui, &[crate::Event { kind: crate::Event::POINTER_RELEASED, ..at[0] }], &mut build);
        selected.set(true);
        testing::frame(&mut ui, &[], &mut build);
        assert!(seen.get().checked && !seen.get().pressed);
        enabled.set(false);
        testing::frame(&mut ui, &[], &mut build);
        assert!(seen.get().disabled);
    }

    /// `focus_on_click(true)`: a click focuses the button; the kit's button keeps the focus where it was.
    #[test]
    fn focus_on_click_focuses_the_clicked_button() {
        for (focus, want) in [(false, 0), (true, state::FOCUSED)] {
            let last: Cell<Option<Response>> = Cell::new(None);
            let mut build = |ui: &mut Ui| { last.set(Some(ui.button_with("open", "Open", &Button::sized(80.0, 30.0).focus_on_click(focus)))); };
            let mut ui = crate::doc::frames(2, &mut build);
            let r = ui.rect(&last.get().unwrap().handle);
            click_centre(&mut ui, r, &mut build);
            testing::frame(&mut ui, &[], &mut build);
            assert_eq!(a11y(&ui, Role::Button, "Open").states & state::FOCUSED, want, "focus_on_click({focus})");
        }
    }

    /// `name` borrows a name made at run time.
    #[test]
    fn a_run_time_name_is_borrowed() {
        let title = format!("Play {}", "Blue in Green");
        let mut build = |ui: &mut Ui| { ui.button_with("play", "Play", &Button::DEFAULT.name(&title)); };
        let ui = crate::doc::frames(1, &mut build);
        assert!(a11y(&ui, Role::Button, "Play Blue in Green").is_role(Role::Button));
    }

    thread_local! { static LAID: Cell<(u8, u8, u8, f32, f32, u32)> = const { Cell::new((9, 9, 9, 0.0, 0.0, 0)) }; }

    /// A body that records the label and the content layout its config carries, and builds a structure of its own: one 33-point node.
    fn laid_out(ui: &mut Ui, c: &crate::bodies::ButtonConfig) {
        assert!(c.size as usize >= core::mem::size_of::<crate::bodies::ButtonConfig>(), "this library carries the label and the layout");
        LAID.with(|s| s.set((c.has_label, c.axis, c.content_align, c.gap, c.text_style.size_points, c.text_box.set)));
        let mut row = ui.node_layout(&c.row);
        (row.w, row.h) = (Size::Px(33.0), Size::Px(33.0));
        ui.set_node_layout(&c.row, &row);
    }

    /// A registered button body is given the caller's label style and content layout, and its structure is the one used.
    #[test]
    fn a_registered_body_is_given_the_label_and_layout() {
        use crate::ui::{Align, Axis};
        let mut ui = Ui::new().unwrap();
        ui.set_a11y(true);
        let id = ui.register_button_body("laid_out", laid_out).expect("registered");
        let text = "button.body = laid_out";
        ui.set_theme(&pxui_types::ffi::ThemeSource { size: core::mem::size_of::<pxui_types::ffi::ThemeSource>() as u32, flags: 0, source: pxui_types::Str { ptr: text.as_ptr(), len: text.len() } }).unwrap();
        assert_eq!(ui.theme().body.button, id);
        let styled = Button::sized(120.0, 44.0).label(crate::Label::BODY.size(14.0).h_px(20.0)).axis(Axis::Column).gap(6.0).content_align(Align::End);
        for (button, seen) in [(styled, (1, 2, 3, 6.0, 14.0, 1)), (Button::sized(120.0, 44.0), (0, 0, 0, 0.0, 0.0, 0))] {
            testing::frame(&mut ui, &[], &mut |ui| { ui.button_with("b", "Go", &button); });
            testing::frame(&mut ui, &[], &mut |ui| { ui.button_with("b", "Go", &button); });
            assert_eq!(LAID.with(|s| s.get()), seen);
            let n = a11y(&ui, Role::Button, "Go");
            assert_eq!((n.rect.w, n.rect.h), (33.0, 33.0), "the registered body's structure");
        }
    }

    #[test]
    fn warm_frames_allocate_nothing() {
        let content = |ui: &mut Ui, _: crate::ui::State| { ui.node("i".into(), Node { w: Size::Px(8.0), h: Size::Px(8.0), ..Node::DEFAULT }); };
        let states = crate::doc::frames(1, &mut |_| {}).theme().button;
        let mut ui = crate::doc::frames(1, &mut |_| {});
        let name = String::from("Row button");
        let name = name.as_str();
        let mut build = |ui: &mut Ui| {
            ui.button("Go");
            ui.button_content(1i64, &Button::DEFAULT.size(Size::Px(90.0), Size::Px(30.0)).look(&states).danger(), &content);
            ui.button_with("off", "Off", &Button::DEFAULT.enabled(false).selected(true).name("Off button").align(Justify::End));
            ui.button_with("tab", "Songs", &Button::DEFAULT.size(Size::Grow, Size::Px(44.0)).centered().selected(true));
            ui.button_with("label", "Label", &Button::sized(90.0, 30.0).label(crate::Label::BODY.size(14.0).h_px(20.0)).content_align(crate::ui::Align::Start).focus_on_click(true));
            ui.button_with("row", "Row", &Button::DEFAULT.size(Size::Px(90.0), Size::Px(30.0)).gap(4.0).content_align(crate::ui::Align::End).name(name));
            ui.button_content("state", &Button::sized(90.0, 30.0).axis(crate::ui::Axis::Column), |ui, state| {
                let w = if state.hovered { 6.0 } else { 8.0 };
                ui.node("i".into(), Node { w: Size::Px(w), h: Size::Px(8.0), ..Node::DEFAULT });
            });
        };
        for round in 0..20 {
            mem_sys::local_reset();
            testing::frame(&mut ui, &[], &mut build);
            if round > 2 { assert_eq!(mem_sys::local_alloc_count(), 0); }
        }
    }
}
