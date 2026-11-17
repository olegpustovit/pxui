//! The kit toggle and segmented control with options: `ui.toggle_with(key, text, on, &Toggle)` and `ui.segmented_with(key, options, selected,
//! &Segmented)` report a `Response` (`changed` when the value moved); `ui.toggle(text, on)` and `ui.segmented(label, options, selected)` are the defaults.
use super::{Key, Response};
use crate::Ui;
use pxui_types::theme::States;

/// How `Ui::toggle_with` varies the kit's toggle, as a `Copy` value built with `const fn` builders from `Toggle::DEFAULT`.
///
/// The default is the plain `Ui::toggle`: the theme's row height, the label at its start, a 28 x 14 switch at its end that takes the press.
#[derive(Clone, Copy)]
pub struct Toggle<'a> {
    pub(crate) row_h: Option<f32>,
    pub(crate) switch: Option<(f32, f32)>,
    pub(crate) track: Option<States>,
    pub(crate) knob: Option<States>,
    pub(crate) enabled: bool,
    pub(crate) detail: &'a str,
    pub(crate) inset: Option<f32>,
    pub(crate) press_row: bool,
    pub(crate) variant: u16,
}

impl<'a> Toggle<'a> {
    /// The plain kit toggle.
    pub const DEFAULT: Toggle<'a> = Toggle { row_h: None, switch: None, track: None, knob: None, enabled: true, detail: "", inset: None, press_row: false, variant: 0 };

    /// Sets the row's height in points; the switch stays centred in it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SETTING: pxui::Toggle = pxui::Toggle::DEFAULT.row_h(52.0);
    /// let _ = SETTING;
    /// ```
    pub const fn row_h(mut self, h: f32) -> Toggle<'a> { self.row_h = Some(h); self }

    /// Sets the switch's width and height in points; its knob is 6 points less than its height, 3 in from each end.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const BIG: pxui::Toggle = pxui::Toggle::DEFAULT.switch(48.0, 28.0);
    /// let _ = BIG;
    /// ```
    pub const fn switch(mut self, w: f32, h: f32) -> Toggle<'a> { self.switch = Some((w, h)); self }

    /// Sets the switch's look per state (the `checked` look is on) in place of the theme's `switch`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::{Look, States}, Toggle};
    /// const TRACK: States = States::all(Look::NONE);
    /// const T: Toggle = Toggle::DEFAULT.track(&TRACK);
    /// let _ = T;
    /// ```
    pub const fn track(mut self, look: &States) -> Toggle<'a> { self.track = Some(*look); self }

    /// Sets the knob's look per state (the `checked` look is on) in place of the theme's `knob`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::{Look, States}, Toggle};
    /// const KNOB: States = States::all(Look::NONE);
    /// const T: Toggle = Toggle::DEFAULT.knob(&KNOB);
    /// let _ = T;
    /// ```
    pub const fn knob(mut self, look: &States) -> Toggle<'a> { self.knob = Some(*look); self }

    /// Sets whether the toggle takes input: disabled takes no press or focus, shows the theme's `disabled` look and reads as disabled to
    /// accessibility, and still blocks what is under it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let t = pxui::Toggle::DEFAULT.enabled(false);
    /// let _ = t;
    /// ```
    pub const fn enabled(mut self, enabled: bool) -> Toggle<'a> { self.enabled = enabled; self }

    /// Adds a secondary line under the text, in the theme's label look; the row's accessible name stays the text. `detail` is borrowed
    /// (a literal, a constant or text the frame owns) for as long as the `Toggle` value lives.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SETTING: pxui::Toggle = pxui::Toggle::DEFAULT.row_h(60.0).detail("Fetch the next two cloud songs");
    /// let _ = SETTING;
    /// let text = String::from("Fetch the next two cloud songs");
    /// let _ = pxui::Toggle::DEFAULT.detail(&text);
    /// ```
    pub const fn detail(mut self, detail: &'a str) -> Toggle<'a> { self.detail = detail; self }

    /// Sets the distance of the text and of the switch from the row's sides in points (the theme's second space step, 12 at 1x, by default).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SETTING: pxui::Toggle = pxui::Toggle::DEFAULT.inset(16.0);
    /// let _ = SETTING;
    /// ```
    pub const fn inset(mut self, inset: f32) -> Toggle<'a> { self.inset = Some(inset); self }

    /// Sets whether a press anywhere on the row toggles it; by default only a press on the switch does.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SETTING: pxui::Toggle = pxui::Toggle::DEFAULT.press_row(true);
    /// let _ = SETTING;
    /// ```
    pub const fn press_row(mut self, press_row: bool) -> Toggle<'a> { self.press_row = press_row; self }

    /// Sets the variant a registered body reads from its config (`variant`, 0 by default): one body builds different structures by it, so the app
    /// picks per call without comparing text. The built-in body ignores it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const V: pxui::Toggle = pxui::Toggle::DEFAULT.variant(1);
    /// let _ = V;
    /// ```
    pub const fn variant(mut self, variant: u16) -> Toggle<'a> { self.variant = variant; self }
}

/// How `Ui::segmented_with` varies the kit's segmented control, as a `Copy` value built with `const fn` builders from `Segmented::DEFAULT`.
///
/// The default is the plain `Ui::segmented`: the theme's control height, each segment as wide as its text plus 16.
#[derive(Clone, Copy)]
pub struct Segmented<'a> {
    pub(crate) h: Option<f32>,
    pub(crate) grow: bool,
    pub(crate) look: Option<States>,
    pub(crate) enabled: bool,
    pub(crate) name: Option<&'a str>,
    pub(crate) variant: u16,
}

impl<'a> Segmented<'a> {
    /// The plain kit segmented control.
    pub const DEFAULT: Segmented<'a> = Segmented { h: None, grow: false, look: None, enabled: true, name: None, variant: 0 };

    /// Sets the strip's height in points.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const CHIPS: pxui::Segmented = pxui::Segmented::DEFAULT.h(28.0);
    /// let _ = CHIPS;
    /// ```
    pub const fn h(mut self, h: f32) -> Segmented<'a> { self.h = Some(h); self }

    /// Makes the strip fill its line, its segments sharing the width equally, instead of each being as wide as its text.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const WIDE: pxui::Segmented = pxui::Segmented::DEFAULT.grow();
    /// let _ = WIDE;
    /// ```
    pub const fn grow(mut self) -> Segmented<'a> { self.grow = true; self }

    /// Sets every segment's look per state (idle, hover, pressed, focus, disabled, checked for the choice) in place of the theme's `segment`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::{Look, States}, Segmented};
    /// const CHIP: States = States::all(Look::NONE);
    /// const S: Segmented = Segmented::DEFAULT.look(&CHIP);
    /// let _ = S;
    /// ```
    pub const fn look(mut self, look: &States) -> Segmented<'a> { self.look = Some(*look); self }

    /// Sets whether the strip takes input: disabled takes no press, key or focus, shows the `disabled` look and reads as disabled to
    /// accessibility.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let s = pxui::Segmented::DEFAULT.enabled(false);
    /// let _ = s;
    /// ```
    pub const fn enabled(mut self, enabled: bool) -> Segmented<'a> { self.enabled = enabled; self }

    /// Sets the variant a registered body reads from its config (`variant`, 0 by default): one body builds different structures by it, so the app
    /// picks per call without comparing text. The built-in body ignores it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const V: pxui::Segmented = pxui::Segmented::DEFAULT.variant(1);
    /// let _ = V;
    /// ```
    pub const fn variant(mut self, variant: u16) -> Segmented<'a> { self.variant = variant; self }

    /// Sets the strip's accessible name; without it the strip has none (each option is named by its text).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const S: pxui::Segmented = pxui::Segmented::DEFAULT.name("Appearance");
    /// let _ = S;
    /// ```
    pub const fn name(mut self, name: &'a str) -> Segmented<'a> { self.name = Some(name); self }
}

impl Ui {
    /// Declares a kit toggle (a row with `text` and a switch) under the current parent and reports what happened to it.
    ///
    /// The row is the toggle's node (role Toggle, named `text`, CHECKABLE and CHECKED by `on`); the caller owns the value: `changed` (also
    /// `clicked`) is the press and release inside it or Enter/Space while focused, and the new value is `on != changed`. A disabled toggle
    /// (`Toggle::enabled(false)`, or inside `Ui::disabled`) takes no press or focus.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the display text.
    /// Panics: when the library refuses the arguments (outside a frame, a negative size, text too long).
    ///
    /// ```
    /// use pxui::Toggle;
    /// let mut on = false;
    /// let mut build = |ui: &mut pxui::Ui| {
    ///     const SETTING: Toggle = Toggle::DEFAULT.row_h(52.0);
    ///     if ui.toggle_with("offline", "Offline mode", on, &SETTING).changed { on = !on; }
    /// };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// pxui::testing::click(&mut ui, 300.0, 26.0, &mut build); // on the switch, 12 from the row's end
    /// assert!(on);
    /// ```
    #[inline]
    #[track_caller]
    pub fn toggle_with<K: Into<Key>>(&mut self, key: K, text: &str, on: bool, toggle: &Toggle<'_>) -> Response { self.toggle_key(key.into(), text, on, toggle) }

    /// Declares a kit toggle as `toggle_with` does with `Toggle::DEFAULT` and returns the new value.
    ///
    /// It is `toggle_with("toggle", text, on, &Toggle::DEFAULT)`: its identity is its place among its siblings, not `text`; use
    /// `toggle_with` for any option or a fixed key.
    /// Allocates: never on a warm frame.
    /// Identity: by position among the parent's children (the key `"toggle"`, made unique in order).
    /// Panics: when the library refuses the call (outside a frame, text too long).
    ///
    /// ```
    /// let mut on = false;
    /// let mut build = |ui: &mut pxui::Ui| { on = ui.toggle("Night light", on); };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// pxui::testing::click(&mut ui, 300.0, 12.0, &mut build);
    /// assert!(on);
    /// ```
    #[inline]
    #[track_caller]
    pub fn toggle(&mut self, text: &str, on: bool) -> bool { on != self.toggle_keyed("toggle", text, on, &Toggle::DEFAULT).changed }

    /// Declares a kit segmented control (`options` side by side in one strip, the choice filled) under the current parent and reports the
    /// strip's response and the choice.
    ///
    /// One Tab stop: Left and Right move the choice, a press on an option chooses it. `selected` is clamped to the options; `changed` says
    /// the returned choice differs from `selected`. The strip is named `Segmented::name` (none without), each option by its text, role Radio.
    /// Allocates: never on a warm frame (up to 32 options; more allocate a span).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never an option's text.
    /// Panics: when the library refuses the arguments (outside a frame, more than 4096 options, a negative height).
    ///
    /// ```
    /// use pxui::Segmented;
    /// let mut chosen = 0;
    /// let mut build = |ui: &mut pxui::Ui| {
    ///     const CHIPS: Segmented = Segmented::DEFAULT.h(28.0).name("Appearance");
    ///     let (_, i) = ui.segmented_with("appearance", &["System", "Light", "Dark"], chosen, &CHIPS);
    ///     chosen = i;
    /// };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// let dark = pxui::testing::find(&ui, pxui::a11y::Role::Radio, "Dark").expect("the Dark option");
    /// pxui::testing::click(&mut ui, f64::from(dark.x + 4.0), f64::from(dark.y + 4.0), &mut build);
    /// assert_eq!(chosen, 2);
    /// ```
    #[inline]
    #[track_caller]
    pub fn segmented_with<K: Into<Key>>(&mut self, key: K, options: &[&str], selected: usize, segmented: &Segmented<'_>) -> (Response, usize) { self.segmented_key(key.into(), options, selected, segmented) }

    /// Declares a kit segmented control as `segmented_with` does with `Segmented::DEFAULT`, named `label`, and returns the choice.
    ///
    /// It is `segmented_with("segmented", options, selected, ..)` with `label` as the strip's accessible name: its identity is its place
    /// among its siblings, not `label`.
    /// Allocates: never on a warm frame (up to 32 options).
    /// Identity: by position among the parent's children (the key `"segmented"`, made unique in order).
    /// Panics: as `segmented_with`.
    ///
    /// ```
    /// let mut chosen = 0;
    /// let mut build = |ui: &mut pxui::Ui| { chosen = ui.segmented("Align", &["Left", "Centre", "Right"], chosen); };
    /// let _ = pxui::doc::frames(1, &mut build);
    /// assert_eq!(chosen, 0);
    /// ```
    #[inline]
    #[track_caller]
    pub fn segmented(&mut self, label: &str, options: &[&str], selected: usize) -> usize {
        self.segmented_keyed("segmented", label, options, selected, &Segmented::DEFAULT).1
    }

    /// The one body of `toggle_with`: the key's label, then the engine call.
    #[track_caller]
    fn toggle_key(&mut self, key: Key, text: &str, on: bool, toggle: &Toggle<'_>) -> Response {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        if toggle.variant == 0 { return self.toggle_keyed(label, text, on, toggle); }
        self.set_variant(pxui_types::theme::BodyKind::toggle, toggle.variant);
        let response = self.toggle_keyed(label, text, on, toggle);
        self.set_variant(pxui_types::theme::BodyKind::toggle, 0);
        response
    }

    /// The one body of `segmented_with`: the key's label, then the engine call.
    #[track_caller]
    fn segmented_key(&mut self, key: Key, options: &[&str], selected: usize, segmented: &Segmented<'_>) -> (Response, usize) {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        if segmented.variant == 0 { return self.segmented_keyed(label, segmented.name.unwrap_or(""), options, selected, segmented); }
        self.set_variant(pxui_types::theme::BodyKind::segment, segmented.variant);
        let result = self.segmented_keyed(label, segmented.name.unwrap_or(""), options, selected, segmented);
        self.set_variant(pxui_types::theme::BodyKind::segment, 0);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::{Segmented, Toggle};
    use crate::a11y::state;
    use crate::ui::{Look, Node, Rect, Role, Size, States};
    use crate::{testing, Part, Response, Ui};
    use std::cell::Cell;

    /// LocalWave's settings switch: a 60 point row with a 48 x 28 switch.
    const SETTING: Toggle = Toggle::DEFAULT.row_h(60.0).switch(48.0, 28.0);

    /// The a11y node of `role` named `name` in the last frame: (states, rect).
    fn node(ui: &Ui, role: Role, name: &str) -> (u32, Rect) {
        let n = ui.a11y_nodes().iter().find(|n| n.is_role(role) && ui.a11y_name_of(n) == name).unwrap_or_else(|| panic!("no {role:?} named {name}"));
        (n.states, n.rect)
    }

    /// Clicks the switch at the end of the row at `r` (a press lands on the part the body builds as `Part::HIT`, not on the whole row).
    fn click_switch(ui: &mut Ui, r: Rect, build: &mut dyn FnMut(&mut Ui)) { testing::click(ui, f64::from(r.x + r.w - 20.0), f64::from(r.y + r.h / 2.0), build); }

    fn click_centre(ui: &mut Ui, r: Rect, build: &mut dyn FnMut(&mut Ui)) { testing::click(ui, f64::from(r.x + r.w / 2.0), f64::from(r.y + r.h / 2.0), build); }

    /// The settings switch: a 60 point row, a 48 x 28 switch centred in it, Toggle role named by the text, CHECKED with the value; a click is
    /// `changed` and the caller flips the value.
    #[test]
    fn settings_switch_has_its_sizes_states_and_flips_on_click() {
        let on = Cell::new(false);
        let last: Cell<Option<Response>> = Cell::new(None);
        let mut build = |ui: &mut Ui| {
            let r = ui.toggle_with("offline", "Offline mode", on.get(), &SETTING);
            if r.changed { on.set(!on.get()); }
            last.set(Some(r));
        };
        let mut ui = crate::doc::frames(3, &mut build);
        let (states, rect) = node(&ui, Role::Toggle, "Offline mode");
        assert_eq!(rect.h, 60.0);
        assert!(states & state::CHECKABLE != 0 && states & state::CHECKED == 0 && states & state::ENABLED != 0 && states & state::FOCUSABLE != 0, "{states:#x}");
        let owner = last.get().unwrap().handle.box_key;
        let switch = ui.part_rect(owner, Part::HIT).expect("the switch");
        assert_eq!((switch.w, switch.h), (48.0, 28.0));
        assert_eq!(switch.y - rect.y, 16.0, "centred in the row");
        assert_eq!(rect.x + rect.w - (switch.x + switch.w), 12.0, "12 from the row's end");
        click_switch(&mut ui, rect, &mut build);
        assert!(on.get());
        testing::frame(&mut ui, &[], &mut build);
        assert!(node(&ui, Role::Toggle, "Offline mode").0 & state::CHECKED != 0);
        assert!(!last.get().unwrap().changed, "changed is the frame of the click");
    }

    /// `toggle(text, on)` is `toggle_with("toggle", text, on, DEFAULT)`: the kit's row height and switch, identity by position (two of them are
    /// two nodes, each flipping its own value).
    #[test]
    fn toggle_forwards_to_toggle_with_and_keys_by_position() {
        let values = Cell::new((false, false));
        let mut one = |ui: &mut Ui| { let (a, b) = values.get(); values.set((ui.toggle("First", a), ui.toggle("Second", b))); };
        let mut two = |ui: &mut Ui| { ui.toggle_with("toggle", "First", false, &Toggle::DEFAULT); };
        let mut ui_one = crate::doc::frames(3, &mut one);
        let ui_two = crate::doc::frames(3, &mut two);
        let (first, second) = (node(&ui_one, Role::Toggle, "First"), node(&ui_one, Role::Toggle, "Second"));
        let twin = node(&ui_two, Role::Toggle, "First");
        assert_eq!((first.0, first.1), (twin.0, twin.1));
        assert_eq!(first.1.h, 24.0);
        click_switch(&mut ui_one, second.1, &mut one);
        assert_eq!(values.get(), (false, true));
    }

    /// A disabled toggle takes no press, no focus and reads as disabled; enabled again it flips.
    #[test]
    fn disabled_toggle_blocks_input() {
        let (on, enabled) = (Cell::new(false), Cell::new(false));
        let mut build = |ui: &mut Ui| { if ui.toggle_with("t", "Wi-Fi", on.get(), &Toggle::DEFAULT.enabled(enabled.get())).changed { on.set(!on.get()); } };
        let mut ui = crate::doc::frames(3, &mut build);
        let (states, rect) = node(&ui, Role::Toggle, "Wi-Fi");
        assert!(states & state::ENABLED == 0 && states & state::FOCUSABLE == 0, "{states:#x}");
        click_switch(&mut ui, rect, &mut build);
        assert!(!on.get());
        enabled.set(true);
        testing::frame(&mut ui, &[], &mut build);
        testing::frame(&mut ui, &[], &mut build);
        let (states, rect) = node(&ui, Role::Toggle, "Wi-Fi");
        assert!(states & state::ENABLED != 0 && states & state::FOCUSABLE != 0);
        click_switch(&mut ui, rect, &mut build);
        assert!(on.get());
    }

    /// LocalWave's chips: a 28 point strip filling its line, three equal segments (Radio, named by their text, CHECKED on the choice); a
    /// press on one moves the choice and reports `changed`.
    #[test]
    fn chips_have_their_sizes_states_and_choose_on_click() {
        const CHIPS: Segmented = Segmented::DEFAULT.h(28.0).grow().name("Cache size");
        const OPTIONS: [&str; 3] = ["5 GB", "10 GB", "20 GB"];
        let chosen = Cell::new(1usize);
        let last: Cell<Option<Response>> = Cell::new(None);
        let mut build = |ui: &mut Ui| {
            let (r, i) = ui.segmented_with("cache", &OPTIONS, chosen.get(), &CHIPS);
            chosen.set(i);
            last.set(Some(r));
        };
        let mut ui = crate::doc::frames(3, &mut build);
        let (strip, strip_rect) = node(&ui, Role::Group, "Cache size");
        assert_eq!(strip_rect.h, 28.0);
        assert!(strip & state::FOCUSABLE != 0);
        let chips: Vec<(u32, Rect)> = OPTIONS.iter().map(|o| node(&ui, Role::Radio, o)).collect();
        assert!(chips.iter().all(|c| c.1.h == 28.0 && c.1.w == chips[0].1.w), "equal shares of the strip: {chips:?}");
        assert!(chips[0].1.w * 3.0 > 0.9 * strip_rect.w, "the strip fills its line: {chips:?} of {strip_rect:?}");
        let checked: Vec<bool> = chips.iter().map(|c| c.0 & state::CHECKED != 0).collect();
        assert_eq!(checked, [false, true, false]);
        assert!(chips.iter().all(|c| c.0 & state::CHECKABLE != 0 && c.0 & state::ENABLED != 0));
        assert!(!last.get().unwrap().changed);
        click_centre(&mut ui, chips[2].1, &mut build);
        assert_eq!(chosen.get(), 2);
        assert!(last.get().unwrap().changed);
        testing::frame(&mut ui, &[], &mut build);
        assert!(node(&ui, Role::Radio, "20 GB").0 & state::CHECKED != 0);
        assert!(!last.get().unwrap().changed);
    }

    /// `segmented(label, options, selected)` is `segmented_with("segmented", ..)` named by the label, the theme's size, identity by position.
    #[test]
    fn segmented_forwards_and_a_disabled_strip_blocks_input() {
        let (chosen, enabled) = (Cell::new(0usize), Cell::new(false));
        let mut short = |ui: &mut Ui| { chosen.set(ui.segmented("Align", &["Left", "Right"], chosen.get())); };
        let mut ui = crate::doc::frames(3, &mut short);
        let (_, strip) = node(&ui, Role::Group, "Align");
        assert_eq!(strip.h, 20.0);
        let right = node(&ui, Role::Radio, "Right").1;
        click_centre(&mut ui, right, &mut short);
        assert_eq!(chosen.get(), 1);
        chosen.set(0);
        let mut off = |ui: &mut Ui| { chosen.set(ui.segmented_with("seg", &["Left", "Right"], chosen.get(), &Segmented::DEFAULT.enabled(enabled.get()).name("Align")).1); };
        let mut ui = crate::doc::frames(3, &mut off);
        let (states, _) = node(&ui, Role::Group, "Align");
        let (radio, right) = node(&ui, Role::Radio, "Right");
        assert!(states & (state::ENABLED | state::FOCUSABLE) == 0 && radio & state::ENABLED == 0, "{states:#x} {radio:#x}");
        click_centre(&mut ui, right, &mut off);
        assert_eq!(chosen.get(), 0);
    }

    /// `disabled` blocks every kit widget and a pressable built inside, reads as disabled to accessibility, nests by OR and ends with its scope.
    #[test]
    fn disabled_scope_blocks_input_and_nests() {
        let hits = Cell::new([0u32; 5]);
        let (flag_outer, flag_inner) = (Cell::new(true), Cell::new(false));
        let mut build = |ui: &mut Ui| {
            let bump = |at: usize, hit: bool| { let mut h = hits.get(); h[at] += hit as u32; hits.set(h); };
            ui.disabled(flag_outer.get(), |ui| {
                bump(0, ui.button_with("b", "Button", &crate::Button::DEFAULT).clicked);
                let node = Node { w: Size::Px(60.0), h: Size::Px(20.0), role: Role::Button, ..Node::DEFAULT };
                bump(1, ui.pressable("p", node, &Look::NONE, |ui| { ui.label("Pressable", crate::Label::DEFAULT); }).clicked);
                ui.disabled(flag_inner.get(), |ui| { bump(2, ui.toggle_with("t", "Toggle", false, &Toggle::DEFAULT).changed); });
            });
            bump(3, ui.button_with("after", "After", &crate::Button::DEFAULT).clicked);
        };
        let mut ui = crate::doc::frames(3, &mut build);
        let flags = |ui: &Ui, role: Role, name: &str| { let (s, r) = node(ui, role, name); (s & state::ENABLED != 0, s & state::FOCUSABLE != 0, r) };
        let (button, toggle, after) = (flags(&ui, Role::Button, "Button"), flags(&ui, Role::Toggle, "Toggle"), flags(&ui, Role::Button, "After"));
        assert_eq!((button.0, button.1, toggle.0, toggle.1), (false, false, false, false), "disabled reads disabled with no focus");
        assert_eq!((after.0, after.1), (true, true), "the scope ended");
        let pressable = ui.a11y_nodes().iter().find(|n| n.is_role(Role::Button) && n.states & state::ENABLED == 0 && n.rect.w == 60.0).expect("the pressable reads disabled");
        assert_eq!(pressable.states & state::FOCUSABLE, 0);
        let pressable = pressable.rect;
        for r in [button.2, pressable] { click_centre(&mut ui, r, &mut build); }
        click_switch(&mut ui, toggle.2, &mut build);
        assert_eq!(hits.get(), [0; 5], "nothing inside the scope was pressed");
        click_centre(&mut ui, after.2, &mut build);
        assert_eq!(hits.get()[3], 1, "outside the scope still works");
        // an inner scope alone disables, and an inner false does not undo the outer true
        flag_outer.set(false);
        flag_inner.set(true);
        testing::frame(&mut ui, &[], &mut build);
        testing::frame(&mut ui, &[], &mut build);
        let (button, toggle) = (flags(&ui, Role::Button, "Button"), flags(&ui, Role::Toggle, "Toggle"));
        assert_eq!((button.0, button.1, toggle.0, toggle.1), (true, true, false, false), "only the inner scope disables");
        click_centre(&mut ui, button.2, &mut build);
        assert_eq!(hits.get()[0], 1);
        click_switch(&mut ui, toggle.2, &mut build);
        assert_eq!(hits.get()[2], 0);
        flag_outer.set(true);
        flag_inner.set(false);
        testing::frame(&mut ui, &[], &mut build);
        testing::frame(&mut ui, &[], &mut build);
        let toggle = flags(&ui, Role::Toggle, "Toggle");
        assert_eq!((toggle.0, toggle.1), (false, false), "an inner false inside an outer true stays disabled");
    }

    /// `press_row(true)`: a press on the label side of the row toggles it; by default only the switch does.
    #[test]
    fn press_row_toggles_from_anywhere_on_the_row() {
        let (plain, whole) = (Cell::new(false), Cell::new(false));
        let mut build = |ui: &mut Ui| {
            if ui.toggle_with("a", "Plain", plain.get(), &Toggle::DEFAULT).changed { plain.set(!plain.get()); }
            if ui.toggle_with("b", "Whole", whole.get(), &Toggle::DEFAULT.press_row(true)).changed { whole.set(!whole.get()); }
        };
        let mut ui = crate::doc::frames(3, &mut build);
        for name in ["Plain", "Whole"] {
            let r = node(&ui, Role::Toggle, name).1;
            testing::click(&mut ui, f64::from(r.x + 20.0), f64::from(r.y + r.h / 2.0), &mut build);
        }
        assert_eq!((plain.get(), whole.get()), (false, true));
    }

    /// `inset(16)` puts the switch 16 from the row's end (12 by default).
    #[test]
    fn inset_moves_the_switch_from_the_rows_end() {
        let owner = Cell::new((0, 0));
        let mut build = |ui: &mut Ui| {
            let a = ui.toggle_with("a", "Default", false, &Toggle::DEFAULT).handle.box_key;
            let b = ui.toggle_with("b", "Inset", false, &Toggle::DEFAULT.inset(16.0)).handle.box_key;
            owner.set((a, b));
        };
        let ui = crate::doc::frames(3, &mut build);
        let gap = |name: &str, key: i64| { let row = node(&ui, Role::Toggle, name).1; let s = ui.part_rect(key, Part::HIT).expect("switch"); row.x + row.w - (s.x + s.w) };
        assert_eq!((gap("Default", owner.get().0), gap("Inset", owner.get().1)), (12.0, 16.0));
    }

    /// `detail` adds a second line: the row keeps its height and name, the switch stays centred beside the two lines.
    #[test]
    fn detail_line_keeps_the_name_and_centres_the_switch() {
        const SETTING: Toggle = Toggle::DEFAULT.row_h(60.0).switch(48.0, 28.0).detail("Fetch the next two cloud songs").inset(16.0);
        let owner = Cell::new(0);
        let mut build = |ui: &mut Ui| { owner.set(ui.toggle_with("d", "Download ahead", false, &SETTING).handle.box_key); };
        let ui = crate::doc::frames(3, &mut build);
        let (states, row) = node(&ui, Role::Toggle, "Download ahead");
        assert!(states & state::CHECKABLE != 0 && row.h == 60.0);
        let s = ui.part_rect(owner.get(), Part::HIT).expect("switch");
        assert_eq!((s.w, s.h, s.y - row.y), (48.0, 28.0, 16.0));
        assert_eq!(row.x + row.w - (s.x + s.w), 16.0);
    }

    /// A view cached inside a scope rebuilds when the scope's flag flips.
    #[test]
    fn a_cached_view_follows_the_scope() {
        let off = Cell::new(true);
        let mut build = |ui: &mut Ui| { ui.disabled(off.get(), |ui| { ui.view_of("v", &0u8, |ui| { ui.button_with("b", "Cached", &crate::Button::DEFAULT); }); }); };
        let mut ui = crate::doc::frames(3, &mut build);
        assert_eq!(node(&ui, Role::Button, "Cached").0 & state::ENABLED, 0);
        off.set(false);
        testing::frame(&mut ui, &[], &mut build);
        testing::frame(&mut ui, &[], &mut build);
        assert_ne!(node(&ui, Role::Button, "Cached").0 & state::ENABLED, 0);
    }

    thread_local! {
        static TOGGLE_SEEN: Cell<(u32, u8, f32, f32, f32)> = const { Cell::new((0, 9, 0.0, 0.0, 0.0)) };
        static TOGGLE_MORE: Cell<(f32, u8, usize)> = const { Cell::new((0.0, 9, 0)) };
        static SEGMENT_SEEN: Cell<(u32, u8, usize)> = const { Cell::new((0, 9, 0)) };
    }

    fn toggle_body(ui: &mut Ui, c: &crate::bodies::PxuiToggleConfig) {
        TOGGLE_SEEN.with(|s| s.set((c.size, c.on, c.row_h, c.switch_w, c.switch_h)));
        TOGGLE_MORE.with(|s| s.set((c.inset, c.press_row, c.detail.len)));
        let b = ui.part(Part::HIT, Node { w: Size::Px(10.0), h: Size::Px(10.0), ..Node::DEFAULT });
        ui.text(&b, "custom toggle");
    }

    fn segment_body(ui: &mut Ui, c: &crate::bodies::PxuiSegmentConfig) {
        SEGMENT_SEEN.with(|s| s.set((c.size, c.grow, c.index)));
        ui.node("custom".into(), Node { w: Size::Px(33.0), h: Size::Grow, ..Node::DEFAULT });
    }

    /// `toggle_with` and `segmented_with` are built through their registered bodies, which get the caller's options in their records.
    #[test]
    fn registered_bodies_build_the_widgets_and_see_the_options() {
        let mut ui = Ui::new().expect("library");
        ui.set_a11y(true);
        ui.register_toggle_body("probe", toggle_body).expect("toggle body");
        ui.register_segment_body("probe", segment_body).expect("segment body");
        let source = "toggle.body = probe\nsegment.body = probe\n";
        ui.set_theme(&crate::ThemeSource { size: core::mem::size_of::<crate::ThemeSource>() as u32, flags: 0, source: crate::Str { ptr: source.as_ptr(), len: source.len() } }).expect("theme");
        let owner = Cell::new(0);
        let mut build = |ui: &mut Ui| {
            owner.set(ui.toggle_with("t", "Offline", true, &SETTING.inset(16.0).press_row(true).detail("More")).handle.box_key);
            ui.segmented_with("s", &["A", "B"], 0, &Segmented::DEFAULT.grow());
        };
        for n in 0..3 { ui.frame(&crate::FrameIn { width: 320.0, height: 240.0, time: n as f64, ..crate::FrameIn::default() }, &mut build).expect("frame"); }
        let (size, on, row_h, w, h) = TOGGLE_SEEN.with(Cell::get);
        assert!(size >= 76 && on == 1 && (row_h, w, h) == (60.0, 48.0, 28.0), "the toggle body's record: {size} {on} {row_h} {w} {h}");
        assert_eq!(TOGGLE_MORE.with(Cell::get), (16.0, 1, 4), "the record carries the inset, press_row and detail");
        let hit = ui.part_rect(owner.get(), Part::HIT).expect("the custom body's part");
        assert_eq!((hit.w, hit.h), (10.0, 10.0), "the custom switch, not the kit's 48 x 28");
        let (size, grow, last) = SEGMENT_SEEN.with(Cell::get);
        assert!(size >= 72 && grow == 1 && last == 1, "the segment body ran for both options with the record's grow: {size} {grow} {last}");
        let toggle = ui.a11y_nodes().iter().find(|n| n.is_role(Role::Toggle)).expect("toggle").rect;
        assert_eq!(toggle.h, 60.0, "the row height is behaviour's");
    }

    /// Every option and scope on a warm frame allocates nothing.
    #[test]
    fn warm_frames_allocate_nothing() {
        let looks = States::all(Look::NONE);
        let mut ui = crate::doc::frames(1, &mut |_| {});
        let mut build = |ui: &mut Ui| {
            ui.toggle("Plain", true);
            ui.toggle_with(3i64, "Big", false, &SETTING.track(&looks).knob(&looks).enabled(false));
            ui.segmented("Align", &["Left", "Centre", "Right"], 1);
            ui.segmented_with("chips", &["5 GB", "10 GB"], 0, &Segmented::DEFAULT.h(28.0).grow().look(&looks).name("Cache").enabled(false));
            ui.disabled(true, |ui| { ui.toggle("Inside", false); ui.disabled(false, |ui| { ui.segmented("Inner", &["A", "B"], 0); }); });
        };
        for round in 0..20 {
            mem_sys::local_reset();
            testing::frame(&mut ui, &[], &mut build);
            if round > 2 { assert_eq!(mem_sys::local_alloc_count(), 0); }
        }
    }
}
