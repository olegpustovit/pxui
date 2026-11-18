//! The kit icon button: `ui.icon_button(key, icon, name, &IconButton)` reports a `Response`.
use super::{Key, Response};
use crate::{Paint, Ui};
use pxui_types::geom::{Rect, Rgba};
use pxui_types::{theme::{Curve, States}, Size};

/// What an icon button shows: a glyph of the icon font its look names, or the app's own drawing.
///
/// The app owns its icon set: pxui names none. A drawing is a plain function that paints into the 20-point square it is given, in the
/// ink it is given (the look's text colour for the button's state), told which icon by the word the `Icon` carries: one function draws
/// a whole icon set by id, and a stateful icon (a filled heart) is one more id.
#[derive(Clone, Copy)]
pub enum Icon {
    /// A character of the icon font.
    Glyph(char),
    /// The app's drawing and its word: the function is called with the painter, the square to draw in, the ink and the word.
    Draw(fn(&mut Paint<'_>, Rect, Rgba, u32), u32),
}

impl Icon {
    /// The glyph `ch`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const PLAY: pxui::Icon = pxui::Icon::glyph('\u{e000}');
    /// let _ = PLAY;
    /// ```
    pub const fn glyph(ch: char) -> Icon { Icon::Glyph(ch) }

    /// The drawing `f` of icon `word`: `f` is called at paint time with the painter, the 20-point square at the centre of the plate, the
    /// ink and `word` (the app's icon id, any value).
    ///
    /// `f` crosses the library as the drawing callback's user pointer and `word` as the call's icon word (`pxui_ui_icon_word`, read back
    /// with `pxui_paint_icon_word`); a change of either repaints the icon.
    /// Allocates: never.
    /// Panics: never (a panic inside `f` is re-raised from the frame).
    ///
    /// ```
    /// use pxui::{geom::{Rect, Rgba}, Fill, Icon, Paint};
    /// const DOT: u32 = 0;
    /// const BAR: u32 = 1;
    /// fn icon(paint: &mut Paint<'_>, rect: Rect, ink: Rgba, word: u32) {
    ///     match word {
    ///         DOT => paint.fill(Rect::new(rect.x + 8.0, rect.y + 8.0, 4.0, 4.0), Fill::new(ink)),
    ///         _ => paint.fill(Rect::new(rect.x + 4.0, rect.y + 9.0, 12.0, 2.0), Fill::new(ink)),
    ///     }
    /// }
    /// const ICONS: [Icon; 2] = [Icon::draw(icon, DOT), Icon::draw(icon, BAR)];
    /// let _ = ICONS;
    /// ```
    pub const fn draw(f: fn(&mut Paint<'_>, Rect, Rgba, u32), word: u32) -> Icon { Icon::Draw(f, word) }
}

/// How `Ui::icon_button` varies the kit's icon button, as a `Copy` value built with `const fn` builders from `IconButton::DEFAULT`.
///
/// The default is the theme's 20-point square with its glyph centred. `'a` is the borrow of the looks given to `look` (`'static` for a
/// const): a style value holds a table of per-state looks by reference, every other setting by value.
#[derive(Clone, Copy)]
pub struct IconButton<'a> {
    pub(crate) w: Size,
    pub(crate) h: Size,
    pub(crate) look: Option<&'a States>,
    pub(crate) enabled: bool,
    pub(crate) press_scale: f32,
    pub(crate) variant: u16,
    pub(crate) press: Option<&'static [Curve; 2]>,
    pub(crate) selected: Option<bool>,
}

impl<'a> IconButton<'a> {
    /// The plain kit icon button.
    pub const DEFAULT: IconButton<'a> = IconButton {
        w: Size::Fit, h: Size::Fit, look: None, enabled: true, press_scale: 1.0, press: None, variant: 0, selected: None,
    };

    /// A button `w` by `h` points.
    /// Allocates: never.
    /// Panics: never (a negative or non-finite size is refused when the button is declared).
    ///
    /// ```
    /// const KEY: pxui::IconButton = pxui::IconButton::sized(36.0, 36.0);
    /// let _ = KEY;
    /// ```
    pub const fn sized(w: f32, h: f32) -> IconButton<'a> { IconButton::DEFAULT.size(Size::Px(w), Size::Px(h)) }

    /// Sets the box's width and height: `Size::Px` points, `Size::Grow` to fill its line, `Size::Fit` the kit's (the theme's 20-point
    /// square); `IconButton::sized(w, h)` is `DEFAULT.size(Size::Px(w), Size::Px(h))`. `Pct` and `Rel` are refused when the button is declared.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::Size, IconButton};
    /// const WIDE: IconButton = IconButton::DEFAULT.size(Size::Grow, Size::Px(44.0));
    /// let _ = WIDE;
    /// ```
    pub const fn size(mut self, w: Size, h: Size) -> IconButton<'a> { self.w = w; self.h = h; self }

    /// Sets the look per state (idle, hover, pressed, focus, disabled, checked) in place of the theme's `icon_button`; its font draws the glyph.
    /// The looks are borrowed, not copied.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::{Look, States}, IconButton};
    /// const PLATE: States = States::of(Look::NONE);
    /// const B: IconButton = IconButton::DEFAULT.look(&PLATE);
    /// let _ = B;
    /// ```
    pub const fn look(mut self, look: &'a States) -> IconButton<'a> { self.look = Some(look); self }

    /// Sets the scale the plate and glyph spring to while the button is held (0.96 is a slight dip) and back to on release.
    /// Reduced motion makes it instant. 1.0 is no animation.
    /// Allocates: never.
    /// Panics: never (a scale that is not finite and above zero is refused when the button is declared).
    ///
    /// ```
    /// const B: pxui::IconButton = pxui::IconButton::sized(36.0, 36.0).press_scale(0.96);
    /// let _ = B;
    /// ```
    pub const fn press_scale(mut self, scale: f32) -> IconButton<'a> { self.press_scale = scale; self }

    /// Sets the curves the press scale runs: `curves[0]` from rest to `press_scale` when the button is pressed, `curves[1]` back on
    /// release, in place of the kit's springs (`Curve::Spring` of 60 in, 20 out); reduced motion still makes both instant. The pair is
    /// a `static` (a `const` button's `&[..]` is promoted to one), so the button carries a pointer, not the curves.
    /// Allocates: never.
    /// Panics: never (a curve that is not `Curve::valid` is refused when the button is declared).
    ///
    /// ```
    /// use pxui::{theme::Curve, IconButton};
    /// const EASE: Curve = Curve::Bezier { duration: 0.24, points: [0.16, 1.0, 0.3, 1.0] };
    /// const KEY: IconButton = IconButton::sized(36.0, 36.0).press_scale(0.96).press_curves(&[Curve::Spring { omega: 80.0 }, EASE]);
    /// let _ = KEY;
    /// ```
    pub const fn press_curves(mut self, curves: &'static [Curve; 2]) -> IconButton<'a> { self.press = Some(curves); self }

    /// Sets whether the button takes input: disabled takes no press or focus, shows the `disabled` look and reads as disabled to accessibility
    /// (its node lacks the ENABLED and FOCUSABLE states). A disabled button never reports `clicked` or `activated`, from any input.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let b = pxui::IconButton::DEFAULT.enabled(false);
    /// let _ = b;
    /// ```
    pub const fn enabled(mut self, enabled: bool) -> IconButton<'a> { self.enabled = enabled; self }

    /// Sets the variant a registered body reads from its config (`variant`, 0 by default): one body builds different structures by it, so the app
    /// picks per call without comparing text. The built-in body ignores it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const B: pxui::IconButton = pxui::IconButton::sized(36.0, 36.0).variant(1);
    /// let _ = B;
    /// ```
    pub const fn variant(mut self, variant: u16) -> IconButton<'a> { self.variant = variant; self }

    /// Makes the button an on/off key (shuffle, repeat, a panel's toggle) that is on when `on`: it shows its look's `checked` state while
    /// on and reads to accessibility as checkable, checked while on; a registered body sees it as `state.checked`. A click still only
    /// reports `clicked`: the caller flips its value and passes it back next frame. Without it the button is a plain one.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{Icon, IconButton};
    /// const KEY: IconButton = IconButton::sized(36.0, 36.0);
    /// let mut shuffle = false;
    /// let mut ui = pxui::doc::frames(1, &mut |ui: &mut pxui::Ui| {
    ///     if ui.icon_button("shuffle", Icon::glyph('~'), "Shuffle", &KEY.selected(shuffle)).clicked { shuffle = !shuffle; }
    /// });
    /// let node = *ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == "Shuffle").unwrap();
    /// assert!(node.states & pxui::A11yStates::CHECKABLE.bits() != 0 && node.states & pxui::A11yStates::CHECKED.bits() == 0);
    /// ```
    pub const fn selected(mut self, on: bool) -> IconButton<'a> { self.selected = Some(on); self }
}

impl Ui {
    /// Declares an icon button under the current parent drawing `icon`, named `name` to accessibility, and reports what happened to it.
    ///
    /// `clicked` is the press and release inside it or Enter/Space while focused. The icon-only control needs `name`: it is the only text it has.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the display name.
    /// Panics: when the library refuses the arguments (outside a frame, a `Pct` or `Rel` size, a press scale that is not above zero). A key or identity label over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::{Icon, IconButton};
    /// let mut clicked = false;
    /// let mut build = |ui: &mut pxui::Ui| { clicked |= ui.icon_button("help", Icon::Glyph('?'), "Help", &IconButton::DEFAULT).clicked; };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// assert!(ui.a11y_nodes().iter().any(|n| ui.a11y_name_of(n) == "Help"));
    /// ```
    #[inline]
    #[track_caller]
    pub fn icon_button<K: Into<Key>>(&mut self, key: K, icon: Icon, name: &str, button: &IconButton<'_>) -> Response { self.icon_button_key(key.into(), icon, name, button) }

    /// The one body of `icon_button`: the key's label and the glyph's text, then the engine call.
    #[inline(always)]
    #[track_caller]
    fn icon_button_key(&mut self, key: Key, icon: Icon, name: &str, button: &IconButton<'_>) -> Response {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        if button.variant == 0 { return self.icon_button_keyed(label, icon, name, button); }
        self.set_variant(pxui_types::theme::BodyKind::icon_button, button.variant);
        let response = self.icon_button_keyed(label, icon, name, button);
        self.set_variant(pxui_types::theme::BodyKind::icon_button, 0);
        response
    }
}

#[cfg(test)]
mod tests {
    use super::{Icon, IconButton};
    use pxui_types::a11y::state;
    use crate::ui::Role;
    use crate::{testing, Response, Ui};
    use std::cell::Cell;

    const KEY: IconButton = IconButton::sized(36.0, 36.0).press_scale(0.96);
    const PLAY: Icon = Icon::glyph('>');

    /// The transport key: 36 points square, a button named by its third argument, clicks reported through `Response`.
    #[test]
    fn transport_key_is_a_named_square_button() {
        let hits = Cell::new(0);
        let last: Cell<Option<Response>> = Cell::new(None);
        let mut build = |ui: &mut Ui| {
            let r = ui.icon_button("play", PLAY, "Play", &KEY);
            if r.clicked { hits.set(hits.get() + 1); }
            last.set(Some(r));
        };
        let mut ui = crate::doc::frames(3, &mut build);
        let n = *ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == "Play").expect("named Play");
        assert!(n.is_role(Role::Button) && (n.rect.w, n.rect.h) == (36.0, 36.0));
        assert!(n.states & state::ENABLED != 0 && n.states & state::FOCUSABLE != 0);
        let r = ui.rect(&last.get().unwrap().handle);
        testing::click(&mut ui, f64::from(r.x + r.w / 2.0), f64::from(r.y + r.h / 2.0), &mut build);
        assert_eq!(hits.get(), 1);
    }

    /// A disabled key reads as disabled and takes no press; the default key is the kit's 20-point square; ids key like labels.
    #[test]
    fn disabled_blocks_input_and_default_is_the_kit_square() {
        let hits = Cell::new(0);
        let mut build = |ui: &mut Ui| {
            if ui.icon_button(7i64, PLAY, "Off", &KEY.enabled(false)).clicked { hits.set(hits.get() + 1); }
            ui.icon_button("on", Icon::Glyph('?'), "Help", &IconButton::DEFAULT);
        };
        let mut ui = crate::doc::frames(3, &mut build);
        let off = *ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == "Off").unwrap();
        assert!(off.states & state::ENABLED == 0 && off.states & state::FOCUSABLE == 0);
        testing::click(&mut ui, f64::from(off.rect.x + 18.0), f64::from(off.rect.y + 18.0), &mut build);
        assert_eq!(hits.get(), 0);
        let on = ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == "Help").unwrap();
        assert_eq!((on.rect.w, on.rect.h), (20.0, 20.0));
    }

    /// `size` takes any `Size`: a key filling its 200-point bar, 44 high; `sized` is `size` in points.
    #[test]
    fn size_fills_its_line() {
        use crate::ui::{Node, Size};
        let mut build = |ui: &mut Ui| {
            ui.row("bar", Node { w: Size::Px(200.0), ..Node::DEFAULT }, |ui| { ui.icon_button("wide", PLAY, "Wide", &IconButton::DEFAULT.size(Size::Grow, Size::Px(44.0))); });
            ui.icon_button("key", PLAY, "Key", &IconButton::DEFAULT.size(Size::Px(36.0), Size::Px(36.0)));
        };
        let ui = crate::doc::frames(2, &mut build);
        let size = |name: &str| { let n = ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == name).unwrap(); (n.rect.w, n.rect.h) };
        assert_eq!(size("Wide"), (200.0, 44.0));
        assert_eq!(size("Key"), (36.0, 36.0));
    }

    thread_local! { static SEEN: Cell<(u32, f32, f32, u8)> = const { Cell::new((0, 0.0, 0.0, 0)) }; }
    thread_local! { static WORDS: Cell<[u32; 2]> = const { Cell::new([0; 2]) }; }

    /// LocalWave's play drawing, through the C ABI: the app's function is called with the 20-point square at the centre of the plate and
    /// the look's text ink, and paints one triangle.
    fn play(paint: &mut crate::Paint<'_>, rect: crate::geom::Rect, ink: crate::geom::Rgba, word: u32) {
        WORDS.with(|w| w.set([word, w.get()[1]]));
        SEEN.with(|s| s.set((s.get().0 + 1, rect.w, rect.h, ink.r)));
        let scale = rect.w.min(rect.h) / 24.0;
        let at = |(x, y): (f32, f32)| (rect.x + x * scale, rect.y + y * scale);
        paint.triangle(at((7.0, 4.6)), at((7.0, 19.4)), at((19.4, 12.0)), crate::Fill::new(ink));
    }

    /// A second drawing function, so each must be called with its own icon's word.
    fn bar(paint: &mut crate::Paint<'_>, rect: crate::geom::Rect, ink: crate::geom::Rgba, word: u32) {
        WORDS.with(|w| w.set([w.get()[0], word]));
        paint.fill(crate::geom::Rect::new(rect.x + 4.0, rect.y + 9.0, 12.0, 2.0), crate::Fill::new(ink));
    }

    #[test]
    fn a_drawn_icon_is_painted_in_the_plate_with_the_states_ink_and_its_word() {
        use crate::ui::{Look, States};
        let looks = States::of(Look { text: crate::geom::Rgba { r: 33, g: 0, b: 0, a: 255 }, ..Look::NONE });
        let key = KEY.look(&looks);
        let mut build = |ui: &mut Ui| {
            ui.icon_button("play", Icon::draw(play, 7), "Play", &key);
            ui.icon_button("bar", Icon::draw(bar, 9), "Bar", &key);
        };
        let mut ui = crate::doc::frames(1, &mut build);
        let n = *ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == "Play").expect("named Play");
        assert_eq!((n.rect.w, n.rect.h), (36.0, 36.0));
        SEEN.with(|s| s.set((0, 0.0, 0.0, 0)));
        testing::frame(&mut ui, &[], &mut build);
        testing::frame(&mut ui, &[], &mut build);
        let (calls, w, h, ink) = SEEN.with(|s| s.get());
        assert!(calls > 0 && (w, h, ink) == (20.0, 20.0, 33), "{:?}", (calls, w, h, ink));
        assert_eq!(WORDS.with(|w| w.get()), [7, 9], "each function is called with its own icon's word");
        // a new word repaints the icon with it
        let mut build = |ui: &mut Ui| {
            ui.icon_button("play", Icon::draw(play, 8), "Play", &key);
            ui.icon_button("bar", Icon::draw(bar, 9), "Bar", &key);
        };
        testing::frame(&mut ui, &[], &mut build);
        testing::frame(&mut ui, &[], &mut build);
        assert_eq!(WORDS.with(|w| w.get()), [8, 9]);
        for round in 0..20 {
            mem_sys::local_reset();
            testing::frame(&mut ui, &[], &mut build);
            if round > 2 { assert_eq!(mem_sys::local_alloc_count(), 0); }
        }
    }

    thread_local! { static CURVES: Cell<*const crate::theme::Curve> = const { Cell::new(core::ptr::null()) }; }

    /// A body that records the press curves pointer its config carries (null: none, the kit's).
    fn recording(_ui: &mut Ui, c: &crate::bodies::IconButtonConfig) {
        assert!(c.size as usize >= core::mem::size_of::<crate::bodies::IconButtonConfig>(), "this library carries the curves");
        CURVES.with(|s| s.set(c.press));
    }

    static EASED: [crate::theme::Curve; 2] = [crate::theme::Curve::Bezier { duration: 0.08, points: [0.16, 1.0, 0.3, 1.0] }, crate::theme::Curve::Steps { duration: 0.24, count: 2 }];

    /// A registered icon button body is given the caller's press curves (the very `static`, by pointer), or null for the kit's springs.
    #[test]
    fn a_registered_body_is_given_the_press_curves() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        let id = ui.register_icon_button_body("recording", recording).expect("registered");
        let text = "icon_button.body = recording";
        ui.set_theme(&crate::ThemeHandle::parse(&text).unwrap()).unwrap();
        assert_eq!(ui.theme().body.icon_button, id);
        for (button, seen) in [(KEY, core::ptr::null()), (KEY.press_curves(&EASED), EASED.as_ptr()), (IconButton::DEFAULT, core::ptr::null())] {
            testing::frame(&mut ui, &[], &mut |ui| { ui.icon_button("k", PLAY, "Play", &button); });
            assert_eq!(CURVES.with(|s| s.get()), seen);
        }
    }

    thread_local! { static CHECKED: Cell<u8> = const { Cell::new(9) }; }

    /// A body that records the checked state it is given and builds its own structure: a 33-point square.
    fn checked_body(ui: &mut Ui, c: &crate::bodies::IconButtonConfig) {
        CHECKED.with(|s| s.set(c.state.checked));
        let mut node = ui.node_layout(&c.node);
        (node.w, node.h) = (crate::ui::Size::Px(33.0), crate::ui::Size::Px(33.0));
        ui.set_node_layout(&c.node, &node);
    }

    /// `selected`: an on/off key reads as checkable (checked while on); a plain button is not checkable; a registered body sees `state.checked` and its structure is the one used.
    #[test]
    fn selected_is_an_on_off_key() {
        const PLATE: IconButton = IconButton::sized(36.0, 36.0);
        let on = Cell::new(false);
        let mut build = |ui: &mut Ui| {
            if ui.icon_button("shuffle", PLAY, "Shuffle", &PLATE.selected(on.get())).clicked { on.set(!on.get()); }
            ui.icon_button("play", PLAY, "Play", &KEY);
        };
        let mut ui = crate::doc::frames(3, &mut build);
        let find = |ui: &Ui, name: &str| *ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == name).unwrap();
        let off = find(&ui, "Shuffle");
        assert!(off.is_role(Role::Button) && off.states & state::CHECKABLE != 0 && off.states & state::CHECKED == 0);
        assert_eq!(find(&ui, "Play").states & state::CHECKABLE, 0, "a plain icon button is not checkable");
        testing::click(&mut ui, f64::from(off.rect.x + 18.0), f64::from(off.rect.y + 18.0), &mut build);
        testing::frame(&mut ui, &[], &mut build);
        assert!(on.get() && find(&ui, "Shuffle").states & state::CHECKED != 0, "the caller's flip shows checked");
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        ui.set_a11y(true);
        let id = ui.register_icon_button_body("checked_body", checked_body).expect("registered");
        let text = "icon_button.body = checked_body";
        ui.set_theme(&crate::ThemeHandle::parse(&text).unwrap()).unwrap();
        assert_eq!(ui.theme().body.icon_button, id);
        for (key, seen) in [(KEY.selected(true), 1), (KEY.selected(false), 0), (KEY, 0)] {
            testing::frame(&mut ui, &[], &mut |ui| { ui.icon_button("k", PLAY, "Key", &key); });
            assert_eq!(CHECKED.with(|s| s.get()), seen);
            let n = find(&ui, "Key");
            assert_eq!((n.rect.w, n.rect.h), (33.0, 33.0), "the registered body's structure");
            assert_eq!(n.states & state::CHECKED != 0, seen == 1, "the accessible state is behaviour's");
        }
    }

    #[test]
    fn warm_frames_allocate_nothing() {
        let looks = crate::doc::frames(1, &mut |_| {}).theme().icon_button;
        let mut ui = crate::doc::frames(1, &mut |_| {});
        let mut build = |ui: &mut Ui| {
            ui.icon_button("a", PLAY, "Play", &KEY.look(&looks));
            ui.icon_button(2i64, Icon::Glyph('x'), "Close", &IconButton::DEFAULT);
            ui.icon_button("off", PLAY, "Off", &KEY.enabled(false));
            ui.icon_button("eased", PLAY, "Eased", &KEY.press_curves(&EASED));
            ui.icon_button("on", PLAY, "Shuffle", &KEY.selected(true));
            ui.icon_button("off-key", PLAY, "Repeat", &KEY.selected(false).look(&looks));
        };
        for round in 0..20 {
            mem_sys::local_reset();
            testing::frame(&mut ui, &[], &mut build);
            if round > 2 { assert_eq!(mem_sys::local_alloc_count(), 0); }
        }
    }
}
