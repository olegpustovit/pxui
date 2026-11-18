//! The kit's small widgets keyed apart from their text: `ui.checkbox(key, text, on, &Checkbox)`, `ui.radio`, `ui.tabs`,
//! `ui.dropdown` and `ui.slider` report a `Response` (and the choice or value); `ui.progress`, `ui.spinner` and `ui.badge` declare
//! stateless leaves and return their node. Each is built through its widget body, so a registered body replaces its structure.
use crate::{Handle, PickResponse, Segment, SliderResponse};
use super::{Key, Response};
use crate::Ui;
use pxui_types::{theme::BodyKind, ui::{Checkbox, Choice, Dropdown, Slider}};

/// The radio group's accessible name and variant used by [Ui::radio].
#[derive(Clone, Copy, Debug)]
pub struct Radio<'a> {
    pub(crate) name: &'a str,
    pub(crate) variant: u16,
}

impl<'a> Radio<'a> {
    /// The theme's default body variant, with no accessible group name.
    pub const DEFAULT: Radio<'a> = Radio { name: "", variant: 0 };

    /// Names the group for assistive technology.
    ///
    /// The label is borrowed for the build call.
    /// Allocates: never.
    /// Identity: The name supplies accessible text. The group is identified by its key.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Radio = pxui::Radio::DEFAULT.name("Mode");
    /// ```
    pub const fn name(mut self, name: &'a str) -> Radio<'a> { self.name = name; self }

    /// Selects the variant the registered body receives.
    ///
    /// Zero uses the theme's variant.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Radio = pxui::Radio::DEFAULT.variant(2);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Radio<'a> { self.variant = variant; self }
}

impl Ui {
    /// Declares a kit checkbox (a row with the 12-point box and `text`) under the current parent and reports what happened to it.
    ///
    /// The row is the checkbox's node (role CheckBox, named `text`, CHECKABLE and CHECKED by `on`); the caller owns the value: `changed`
    /// (also `clicked`) is a press and release on the row or Enter/Space while it is focused, and the new value is `on != changed`.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the display text.
    /// Panics: when the library refuses the call (outside a frame). A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::Checkbox;
    /// let mut snap = false;
    /// let mut build = |ui: &mut pxui::Ui| {
    ///     if ui.checkbox("snap", "Snap to grid", snap, &Checkbox::DEFAULT).changed { snap = !snap; }
    /// };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// pxui::testing::click(&mut ui, 20.0, 12.0, &mut build);
    /// assert!(snap);
    /// ```
    #[inline]
    #[track_caller]
    pub fn checkbox<K: Into<Key>>(&mut self, key: K, text: &str, on: bool, checkbox: &Checkbox) -> Response { self.checkbox_key(key.into(), text, on, checkbox) }

    /// Builds a radio group and reports its current choice and interaction.
    ///
    /// The caller supplies its current selection. Arrows or a press choose an item. None leaves the group unselected.
    /// Each option uses its explicit name when supplied and otherwise uses its text.
    /// Allocates: never on a warm frame within the inline item capacity.
    /// Identity: The key identifies this control among siblings. Items are keyed as in [Ui::segmented]: an item id (never display
    /// text) follows the item through a reorder for node creation, focus and secondary presses, and unnamed items use a positional
    /// namespace distinct from ids.
    /// Panics: an invalid build or a key whose label exceeds 256 bytes.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let _ = ui.radio("mode", &[pxui::Segment::new("Auto")], None, &pxui::Radio::DEFAULT);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn radio<K: Into<Key>>(&mut self, key: K, items: &[Segment<'_>], selected: Option<usize>, radio: &Radio<'_>) -> PickResponse {
        self.pick_one_key(0, key.into(), radio.name, items, selected, radio.variant)
    }

    /// Declares a kit tab bar (`tabs` side by side, the one in use underlined) and reports the bar's response and the tab in use.
    ///
    /// One Tab stop; Left and Right (Home, End) switch, a press on a tab chooses it. `selected` is clamped to the tabs; `changed` says
    /// the returned tab differs from `selected`. Each tab is named by its text, the bar by `Choice::name`.
    /// Allocates: never on a warm frame (up to 32 tabs).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never a tab's text.
    /// Panics: as `radio`. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// let mut page = 0;
    /// let mut build = |ui: &mut pxui::Ui| { page = ui.tabs("pages", &["Layer", "Mask"], page, &pxui::Choice::DEFAULT).1; };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// let mask = pxui::testing::find(&ui, pxui::Role::Tab, "Mask").expect("the Mask tab");
    /// pxui::testing::click(&mut ui, f64::from(mask.x + 4.0), f64::from(mask.y + 4.0), &mut build);
    /// assert_eq!(page, 1);
    /// ```
    #[inline]
    #[track_caller]
    pub fn tabs<K: Into<Key>>(&mut self, key: K, tabs: &[&str], selected: usize, choice: &Choice<'_>) -> (Response, usize) {
        self.choice_key(1, BodyKind::tabs, key.into(), choice.name, tabs, selected, choice.variant)
    }

    /// Builds a dropdown and reports the button interaction and current selection.
    ///
    /// None leaves the group unselected. The button is named by text. Click, Enter, Space or Down opens the menu.
    /// Arrows move through the menu. Enter picks. Escape and an outside press close the menu.
    /// Allocates: never on a warm frame within the inline item capacity.
    /// Identity: The key identifies this control among siblings. Items are keyed as in [Ui::segmented]: an item id (never display
    /// text) follows the item through a reorder for node creation, focus and secondary presses, and unnamed items use a positional
    /// namespace distinct from ids.
    /// Panics: an invalid build or a key whose label exceeds 256 bytes.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let _ = ui.dropdown("blend", "Blend", &[pxui::Segment::new("Normal")], None, &pxui::Dropdown::DEFAULT);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn dropdown<K: Into<Key>>(&mut self, key: K, text: &str, items: &[Segment<'_>], selected: Option<usize>, dropdown: &Dropdown) -> PickResponse {
        self.pick_one_key(3, key.into(), text, items, selected, dropdown.variant)
    }

    /// Builds a slider and reports the resulting value and interaction.
    ///
    /// The value is clamped to the finite, ordered range. Dragging previews it. Release, keyboard and accessibility edits commit it,
    /// and on that frame `preview` equals `commit` (see [crate::SliderResponse] for which frames supply each).
    /// The track's accessible name is text.
    /// Allocates: never on a warm frame.
    /// Identity: The key identifies this control among siblings. An item's id is independent of its display text.
    /// Panics: an invalid build, a nonfinite value or bound, a reversed range, or an overlong key.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let _ = ui.slider("volume", "Volume", 0.5, 0.0..=1.0, &pxui::Slider::DEFAULT);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn slider<K: Into<Key>>(&mut self, key: K, text: &str, value: f32, range: core::ops::RangeInclusive<f32>, slider: &Slider) -> SliderResponse {
        self.slider_key(key.into(), text, value, range, slider)
    }

    /// Declares a kit progress row (`text` and a 4-point track) and returns its node: a bar `value` (0 to 1) of the way, or with `None`
    /// a busy bar that `busy_step` sweeps across.
    ///
    /// The track is named `text`, role Progress (BUSY without a value). It holds no state; pass the node to `busy_step` between frames.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the display text.
    /// Panics: when the library refuses the call (outside a frame, a value that is not finite). A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| { ui.progress("export", "Export", Some(0.4)); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn progress<K: Into<Key>>(&mut self, key: K, text: &str, value: Option<f32>) -> Handle { self.progress_key(key.into(), text, value) }

    /// Declares a kit spinner (eight dots round a 16-point circle) named `name` and returns its node, which `busy_step` turns.
    ///
    /// Role Progress, BUSY. It holds no state.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`).
    /// Panics: when the library refuses the call (outside a frame). A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| { ui.spinner("loading", "Loading"); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn spinner<K: Into<Key>>(&mut self, key: K, name: &str) -> Handle { self.spinner_key(key.into(), name) }

    /// Declares a kit badge (`text` on a 16-point pill in the badge look) named `name` and returns its node; `patch_text` on it updates a
    /// count without a rebuild.
    ///
    /// Role Label, named `name` (empty: none), with `text` as its accessible value. It holds no state.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the display text.
    /// Panics: when the library refuses the call (outside a frame). A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| { ui.badge("unread", "Unread", "3"); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn badge<K: Into<Key>>(&mut self, key: K, name: &str, text: &str) -> Handle { self.badge_key(key.into(), name, text) }
}

#[cfg(test)]
mod tests {
    use pxui_types::ui::{Checkbox, Choice, Dropdown, Slider};
    use crate::bodies::{BadgeConfig, CheckboxConfig, DropdownConfig, RadioConfig, ProgressConfig, SliderConfig, SpinnerConfig, TabsConfig};
    use crate::ui::{Node, Role, Size};
    use crate::{testing, FrameIn, Ui};
    use std::cell::RefCell;

    thread_local! { static SEEN: RefCell<Vec<(&'static str, u16)>> = const { RefCell::new(Vec::new()) }; }
    fn seen(kind: &'static str, variant: u16) { SEEN.with(|s| s.borrow_mut().push((kind, variant))); }
    /// Each registered body builds one marker node of its own, named by its kind, so the custom structure shows in the tree.
    fn marker(ui: &mut Ui, kind: &'static str) { let n = ui.node(kind.into(), Node { w: Size::Px(10.0), h: Size::Px(10.0), role: Role::Image, ..Node::DEFAULT }); ui.a11y_name(&n, kind); }
    fn check(ui: &mut Ui, c: &CheckboxConfig) { seen("checkbox", c.variant); marker(ui, "custom checkbox"); }
    fn radio(ui: &mut Ui, c: &RadioConfig) { seen("radio", c.variant); marker(ui, "custom radio"); }
    fn tab(ui: &mut Ui, c: &TabsConfig) { seen("tab", c.variant); marker(ui, "custom tab"); }
    fn dropdown(ui: &mut Ui, c: &DropdownConfig) { seen("dropdown", c.variant); marker(ui, "custom dropdown"); }
    fn slider(ui: &mut Ui, c: &SliderConfig) { seen("slider", c.variant); marker(ui, "custom slider"); }
    fn progress(ui: &mut Ui, c: &ProgressConfig) { seen("progress", c.variant); marker(ui, "custom progress"); }
    fn spinner(ui: &mut Ui, c: &SpinnerConfig) { seen("spinner", c.variant); marker(ui, "custom spinner"); }
    fn badge(ui: &mut Ui, c: &BadgeConfig) { seen("badge", c.variant); marker(ui, "custom badge"); }

    fn build(ui: &mut Ui) {
        ui.checkbox("check", "Check", false, &Checkbox::DEFAULT.variant(1));
        ui.radio("radio", &[crate::Segment::new("A"), crate::Segment::new("B")], Some(0), &crate::Radio::DEFAULT.variant(2));
        ui.tabs("tabs", &["T"], 0, &Choice::DEFAULT.variant(3));
        ui.dropdown("drop", "Drop", &[crate::Segment::new("x")], Some(0), &Dropdown::DEFAULT.variant(4));
        ui.slider("slide", "Slide", 0.5, 0.0..=1.0, &Slider::DEFAULT.variant(5));
        ui.progress("prog", "Prog", None);
        ui.spinner("spin", "Spin");
        ui.badge("badge", "Badge", "7");
    }

    /// Every widget of the family is built through its body: with a registered body for each kind, the custom structure is what the tree
    /// holds, and each style's variant reaches its own body's config (the leaves, which have no style, are given 0).
    #[test]
    fn each_kit_widget_is_built_through_its_registered_body_with_its_variant() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.set_a11y(true);
        ui.register_checkbox_body("c", check).unwrap();
        ui.register_radio_body("c", radio).unwrap();
        ui.register_tabs_body("c", tab).unwrap();
        ui.register_dropdown_body("c", dropdown).unwrap();
        ui.register_slider_body("c", slider).unwrap();
        ui.register_progress_body("c", progress).unwrap();
        ui.register_spinner_body("c", spinner).unwrap();
        ui.register_badge_body("c", badge).unwrap();
        let text = "checkbox.body = c\nradio.body = c\ntabs.body = c\ndropdown.body = c\nslider.body = c\nprogress.body = c\nspinner.body = c\nbadge.body = c";
        ui.set_theme(&crate::ThemeHandle::parse(&text).unwrap()).unwrap();
        SEEN.with(|s| s.borrow_mut().clear());
        ui.frame(&FrameIn::new(400.0, 400.0), &mut build).unwrap();
        let mut got = SEEN.with(|s| s.borrow().clone());
        got.dedup();
        assert_eq!(got, [("checkbox", 1), ("radio", 2), ("tab", 3), ("dropdown", 4), ("slider", 5), ("progress", 0), ("spinner", 0), ("badge", 0)]);
        for kind in ["checkbox", "radio", "tab", "dropdown", "slider", "progress", "spinner", "badge"] {
            assert!(testing::find(&ui, Role::Image, &format!("custom {kind}")).is_some(), "the {kind} body's structure is used");
        }
    }

    /// The key is identity and the text what shows: two checkboxes of one text under two keys are two nodes, and a checkbox keeps its node
    /// when its text changes; the leaves are named by their text, not their key.
    #[test]
    fn keys_are_identity_and_text_is_shown() {
        let text = std::cell::Cell::new("Shuffle");
        let boxes = std::cell::Cell::new([0i64; 3]);
        let mut build = |ui: &mut Ui| {
            let a = ui.checkbox("a", text.get(), false, &Checkbox::DEFAULT).handle.box_key;
            let b = ui.checkbox("b", "Shuffle", false, &Checkbox::DEFAULT).handle.box_key;
            let p = ui.progress("export", "Export", Some(0.5)).box_key;
            boxes.set([a, b, p]);
        };
        let mut ui = crate::doc::frames(1, &mut build);
        let first = boxes.get();
        assert_ne!(first[0], first[1]);
        assert!(testing::find(&ui, Role::CheckBox, "Shuffle").is_some());
        assert!(testing::find(&ui, Role::Progress, "Export").is_some(), "the track is named by the text, not the key");
        text.set("Repeat");
        testing::frame(&mut ui, &[], &mut build);
        assert_eq!(boxes.get()[0], first[0], "a new text keeps the key's node");
        assert!(testing::find(&ui, Role::CheckBox, "Repeat").is_some());
    }
}
