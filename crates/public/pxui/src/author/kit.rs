//! The kit's small widgets keyed apart from their text: `ui.checkbox(key, text, on, &Checkbox)`, `ui.radio`, `ui.tabs`,
//! `ui.dropdown` and `ui.slider` report a `Response` (and the choice or value); `ui.progress`, `ui.spinner` and `ui.badge` declare
//! stateless leaves and return their node. Each is built through its widget body, so a registered body replaces its structure.
use crate::{Handle, PickResponse, Segment, SliderResponse};
use super::{Key, Response};
use crate::Ui;
use pxui_types::ui::{Checkbox, Dropdown, Slider};

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

/// The tab bar's accessible name, variant and press focus used by [Ui::tabs].
#[derive(Clone, Copy, Debug)]
pub struct Tabs<'a> {
    pub(crate) name: &'a str,
    pub(crate) variant: u16,
    pub(crate) focus_on_click: bool,
}

impl<'a> Tabs<'a> {
    /// The theme's default body variant, an unnamed bar, and a press that chooses a tab without focusing the bar.
    pub const DEFAULT: Tabs<'a> = Tabs { name: "", variant: 0, focus_on_click: false };

    /// Names the bar for assistive technology.
    ///
    /// The name is borrowed for the build call.
    /// Allocates: never.
    /// Identity: The name supplies accessible text. The bar is identified by its key.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Tabs = pxui::Tabs::DEFAULT.name("Library");
    /// ```
    pub const fn name(mut self, name: &'a str) -> Tabs<'a> { self.name = name; self }

    /// Selects the variant the registered tabs body receives.
    ///
    /// Zero uses the theme's variant.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Tabs = pxui::Tabs::DEFAULT.variant(2);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Tabs<'a> { self.variant = variant; self }

    /// Makes a press on a tab focus the bar on the press itself (false by default: a press chooses without moving the focus, as the kit's
    /// buttons leave it). The bar keeps the focus when the caller does not apply the choice.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const TOUCH: pxui::Tabs = pxui::Tabs::DEFAULT.focus_on_click(true);
    /// ```
    pub const fn focus_on_click(mut self, focus: bool) -> Tabs<'a> { self.focus_on_click = focus; self }
}

/// The section index's accessible name and body variant used by [Ui::section_index].
#[derive(Clone, Copy, Debug)]
pub struct SectionIndex<'a> {
    pub(crate) name: &'a str,
    pub(crate) variant: u16,
}

impl<'a> SectionIndex<'a> {
    /// An unnamed strip, the theme's `section_index` body with its default variant.
    pub const DEFAULT: SectionIndex<'a> = SectionIndex { name: "", variant: 0 };

    /// Names the strip for assistive technology (a slider named so).
    ///
    /// The name is borrowed for the build call.
    /// Allocates: never.
    /// Identity: The name supplies accessible text. The strip is identified by its key.
    /// Panics: never.
    ///
    /// ```
    /// const INDEX: pxui::SectionIndex = pxui::SectionIndex::DEFAULT.name("Jump to letter");
    /// ```
    pub const fn name(mut self, name: &'a str) -> SectionIndex<'a> { self.name = name; self }

    /// Selects the `section_index` body's `variant` (`PxuiSectionIndexConfig.variant`; 0 is the default).
    ///
    /// Allocates: never.
    /// Identity: none; the strip is identified by its key.
    /// Panics: never.
    ///
    /// ```
    /// const COMPACT: pxui::SectionIndex = pxui::SectionIndex::DEFAULT.variant(1);
    /// ```
    pub const fn variant(mut self, variant: u16) -> SectionIndex<'a> { self.variant = variant; self }
}

/// The disclosure's header variant used by [Ui::disclosure].
#[derive(Clone, Copy, Debug)]
pub struct Disclosure {
    pub(crate) variant: u16,
}

impl Disclosure {
    /// The theme's `inspector_section` body with its default variant.
    pub const DEFAULT: Disclosure = Disclosure { variant: 0 };

    /// Selects the variant the registered `inspector_section` body receives (an app's disclosure body builds its header by it).
    ///
    /// Zero uses the theme's variant.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const DRIVE: pxui::Disclosure = pxui::Disclosure::DEFAULT.variant(2);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Disclosure { self.variant = variant; self }
}

/// The progress row's body and variant used by [Ui::progress].
#[derive(Clone, Copy, Debug)]
pub struct Progress {
    pub(crate) variant: u16,
}

impl Progress {
    /// The theme's default body variant.
    pub const DEFAULT: Progress = Progress { variant: 0 };

    /// Selects the variant the registered body receives.
    ///
    /// Zero uses the theme's variant.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Progress = pxui::Progress::DEFAULT.variant(2);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Progress { self.variant = variant; self }
}

/// The spinner's body and variant used by [Ui::spinner].
#[derive(Clone, Copy, Debug)]
pub struct Spinner {
    pub(crate) variant: u16,
}

impl Spinner {
    /// The theme's default body variant.
    pub const DEFAULT: Spinner = Spinner { variant: 0 };

    /// Selects the variant the registered body receives.
    ///
    /// Zero uses the theme's variant.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Spinner = pxui::Spinner::DEFAULT.variant(2);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Spinner { self.variant = variant; self }
}

/// The badge's body and variant used by [Ui::badge].
#[derive(Clone, Copy, Debug)]
pub struct Badge {
    pub(crate) variant: u16,
}

impl Badge {
    /// The theme's default body variant.
    pub const DEFAULT: Badge = Badge { variant: 0 };

    /// Selects the variant the registered body receives.
    ///
    /// Zero uses the theme's variant.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Badge = pxui::Badge::DEFAULT.variant(2);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Badge { self.variant = variant; self }
}

impl Ui {
    /// Declares a kit checkbox (a row with the box, 12 points unless `Checkbox::size` says otherwise, and `text`) under the current parent and reports what happened to it.
    ///
    /// `Checkbox::disabled(true)` shows the disabled look and takes no press or focus; `Checkbox::passive(true)` makes a mark with no input of its own
    /// (for a row that owns the press), whose `Response` never clicks.
    ///
    /// The row is the checkbox's node (role CheckBox, named `text`, CHECKABLE and CHECKED by `on`); the caller owns the value: `changed`
    /// (also `clicked`) is a press and release on the row or Enter/Space while it is focused, and the new value is `on != changed`.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the display text.
    /// Panics: when the library refuses the call (outside a frame, or a `Checkbox::size` that is negative or not finite). A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
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
    pub fn checkbox<K: Into<Key>>(&mut self, key: K, text: &str, on: bool, checkbox: &Checkbox<'_>) -> Response { self.checkbox_key(key.into(), text, on, checkbox) }

    /// Builds a radio group and reports its current choice and interaction.
    ///
    /// The caller supplies its current selection. One Tab stop; the arrows move the choice and a press on a row chooses it. None
    /// leaves the group unselected. Each option uses its explicit name when supplied and otherwise uses its text.
    /// Allocates: never on a warm frame within the inline item capacity.
    /// Identity: The key identifies this control among siblings. Items are keyed as in [Ui::segmented]: an item id (never display
    /// text) follows the item through a reorder for node creation, focus and secondary presses, and unnamed items use a positional
    /// namespace distinct from ids.
    /// Panics: an invalid build or a key whose label exceeds 256 bytes.
    ///
    /// ```
    /// use pxui::{Radio, Segment};
    /// let mut mode = Some(0);
    /// let mut build = |ui: &mut pxui::Ui| {
    ///     const MODE: Radio = Radio::DEFAULT.name("Mode");
    ///     let items = [Segment::new("Auto"), Segment::new("Light"), Segment::new("Dark")];
    ///     mode = ui.radio("mode", &items, mode, &MODE).chosen;
    /// };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// let dark = pxui::testing::find(&ui, pxui::Role::Radio, "Dark").expect("the Dark row");
    /// pxui::testing::click(&mut ui, f64::from(dark.x + 4.0), f64::from(dark.y + 4.0), &mut build);
    /// assert_eq!(mode, Some(2));
    /// ```
    #[inline]
    #[track_caller]
    pub fn radio<K: Into<Key>>(&mut self, key: K, items: &[Segment<'_>], selected: Option<usize>, radio: &Radio<'_>) -> PickResponse {
        self.pick_one_key(0, key.into(), radio.name, items, selected, radio.variant, false)
    }

    /// Builds a kit tab bar (`items` side by side, the one in use underlined) and reports the bar's response and the tab in use.
    ///
    /// One Tab stop, the bar; Left and Right (Home, End) switch the tab in use (from none, Right takes the first and Left the last), a
    /// press on a tab chooses it. `selected` is the tab in use (`None`: none is, as a bottom bar while another screen shows; past the
    /// tabs: the last); `response.changed` says `chosen` differs from it, `chosen_id` is the chosen item's `Segment::id`, and
    /// `secondary` is the tab a secondary press began on (never a choice). Each tab is named by its `Segment::name`, else its text; its
    /// `Segment::detail` (a count) is drawn after the text by the built-in body. The bar is named by `Tabs::name`; with
    /// `Tabs::focus_on_click` a press focuses the bar on the press itself, and the bar keeps the focus when the caller does not apply the
    /// choice.
    /// Allocates: never on a warm frame within the inline item capacity (32 tabs).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never a tab's text. Tabs are keyed
    /// as in [Ui::segmented]: an item id follows its tab through a reorder, and a tab without one is keyed by its place in a separate
    /// namespace.
    /// Panics: an invalid build, more than 4096 tabs, or a key whose label exceeds 256 bytes. Display text past 64 KiB is clipped at a
    /// character boundary, never refused.
    ///
    /// ```
    /// use pxui::{Segment, Tabs};
    /// let mut page = Some(0);
    /// let mut build = |ui: &mut pxui::Ui| {
    ///     const PAGES: Tabs = Tabs::DEFAULT.name("Pages");
    ///     let items = [Segment::new("Layer").id(1), Segment::new("Mask").id(2).detail("3")];
    ///     let r = ui.tabs("pages", &items, page, &PAGES);
    ///     if r.response.changed { page = r.chosen; }
    /// };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// let mask = pxui::testing::find(&ui, pxui::Role::Tab, "Mask").expect("the Mask tab");
    /// pxui::testing::click(&mut ui, f64::from(mask.x + 4.0), f64::from(mask.y + 4.0), &mut build);
    /// assert_eq!(page, Some(1));
    /// ```
    #[inline]
    #[track_caller]
    pub fn tabs<K: Into<Key>>(&mut self, key: K, items: &[Segment<'_>], selected: Option<usize>, tabs: &Tabs<'_>) -> PickResponse {
        self.pick_one_key(1, key.into(), tabs.name, items, selected, tabs.variant, tabs.focus_on_click)
    }

    /// Builds a vertical section index (the A–Z strip beside a long list) and reports the strip's response and the letter chosen this
    /// frame.
    ///
    /// `letters` share the strip's height; a press or drag on it chooses the letter under the pointer, and the arrows, Home/End and an
    /// assistive technology's value move the choice (the strip is a focusable slider over the letters, named by `SectionIndex::name`).
    /// `chosen` is `Some(i)` while the pointer holds the strip, on its release and for a key or an assistive value, `None` when nothing
    /// moved it, and `response.changed` says the same; `chosen_id` and `secondary` are `None`. Size it with its parent (it is as wide as a
    /// control and as tall as the room it is given). An empty `letters` is a disabled strip. The letters are drawn by the `section_index`
    /// body (`Ui::register_section_index_body`, `section_index.body = NAME` in a theme).
    /// Allocates: never on a warm frame (up to 32 letters; more allocate a span).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never a letter.
    /// Panics: an invalid build (outside a frame), more than 4096 letters, or a key whose label exceeds 256 bytes. Display text past
    /// 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::SectionIndex;
    /// let mut letter = None;
    /// pxui::doc::frames(1, &mut |ui| {
    ///     const INDEX: SectionIndex = SectionIndex::DEFAULT.name("Jump to letter");
    ///     letter = ui.section_index("index", &["A", "B", "C"], &INDEX).chosen;
    /// });
    /// assert_eq!(letter, None);
    /// ```
    #[inline]
    #[track_caller]
    pub fn section_index<K: Into<Key>>(&mut self, key: K, letters: &[&str], index: &SectionIndex<'_>) -> PickResponse {
        self.section_index_key(key.into(), letters, index)
    }

    /// Builds a collapsible section: a header row showing `text` that a press (or Enter/Space while focused) toggles, and `f` under it
    /// while open. The caller owns `open` and passes it back each frame.
    ///
    /// The response is the header's; `changed` (with `clicked`) is a toggle, so the new value is `open != changed`. The header is built
    /// through the `inspector_section` body (the theme's section look and a chevron by default; a registered body or a theme file's
    /// `inspector_section.body` replaces it, `Disclosure::variant` reaching its config), role Heading, named `text`, EXPANDABLE and
    /// EXPANDED by `open`. `f` runs only while open, its nodes the header's following siblings.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the text.
    /// Panics: an invalid build (outside a frame) or a key whose label exceeds 256 bytes; a panic in `f` follows the callback panic rule.
    ///
    /// ```
    /// use pxui::Disclosure;
    /// let mut open = false;
    /// let mut build = |ui: &mut pxui::Ui| {
    ///     if ui.disclosure("more", "More options", open, &Disclosure::DEFAULT, &mut |ui| { ui.label("Body", pxui::Label::BODY); }).changed { open = !open; }
    /// };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// pxui::testing::click(&mut ui, 20.0, 12.0, &mut build);
    /// assert!(open);
    /// ```
    #[inline]
    #[track_caller]
    pub fn disclosure<K: Into<Key>>(&mut self, key: K, text: &str, open: bool, disclosure: &Disclosure, f: &mut dyn FnMut(&mut Ui)) -> Response {
        self.disclosure_key(key.into(), text, open, disclosure.variant, f)
    }

    /// Builds a dropdown and reports the button interaction and current selection.
    ///
    /// None leaves the group unselected. The button is named by text and shows the chosen option and a chevron. A click, Enter,
    /// Space or Down opens its menu below (Up and Down move in it, Enter picks, Escape or a press outside closes it). While open, the
    /// menu escapes a cached view, so that view rebuilds each frame until it closes.
    /// Allocates: never on a warm frame within the inline item capacity.
    /// Identity: The key identifies this control among siblings. Items are keyed as in [Ui::segmented]: an item id (never display
    /// text) follows the item through a reorder for node creation, focus and secondary presses, and unnamed items use a positional
    /// namespace distinct from ids.
    /// Panics: an invalid build or a key whose label exceeds 256 bytes.
    ///
    /// ```
    /// use pxui::Segment;
    /// let mut blend = Some(1);
    /// pxui::doc::frames(1, &mut |ui| {
    ///     blend = ui.dropdown("blend", "Blend", &[Segment::new("Normal"), Segment::new("Multiply")], blend, &pxui::Dropdown::DEFAULT).chosen;
    /// });
    /// assert_eq!(blend, Some(1));
    /// ```
    #[inline]
    #[track_caller]
    pub fn dropdown<K: Into<Key>>(&mut self, key: K, text: &str, items: &[Segment<'_>], selected: Option<usize>, dropdown: &Dropdown) -> PickResponse {
        self.pick_one_key(3, key.into(), text, items, selected, dropdown.variant, false)
    }

    /// Builds a slider and reports the resulting value and interaction.
    ///
    /// The value is clamped to the finite, ordered range. Dragging previews it. Release, the arrows (a hundredth of the range; Page
    /// keys a tenth; Home and End the ends) and an assistive technology's SetValue commit it, and on that frame `preview` equals
    /// `commit` (see [crate::SliderResponse] for which frames supply each).
    /// The track's accessible name is text.
    /// Allocates: never on a warm frame.
    /// Identity: The key identifies this control among siblings. An item's id is independent of its display text.
    /// Panics: an invalid build, a nonfinite value or bound, a reversed range, or an overlong key.
    ///
    /// ```
    /// let mut volume = 0.5;
    /// let mut moved = None;
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let slider = ui.slider("volume", "Volume", volume, 0.0..=1.0, &pxui::Slider::DEFAULT);
    ///     (volume, moved) = (slider.value, slider.commit);
    /// });
    /// assert_eq!((volume, moved), (0.5, None));
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
    /// pxui::doc::frames(1, &mut |ui| { ui.progress("export", "Export", Some(0.4), &pxui::Progress::DEFAULT); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn progress<K: Into<Key>>(&mut self, key: K, text: &str, value: Option<f32>, progress: &Progress) -> Handle { self.progress_key(key.into(), text, value, progress.variant) }

    /// Declares a kit spinner (eight dots round a 16-point circle) named `name` and returns its node, which `busy_step` turns.
    ///
    /// Role Progress, BUSY. It holds no state.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`).
    /// Panics: when the library refuses the call (outside a frame). A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| { ui.spinner("loading", "Loading", &pxui::Spinner::DEFAULT); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn spinner<K: Into<Key>>(&mut self, key: K, name: &str, spinner: &Spinner) -> Handle { self.spinner_key(key.into(), name, spinner.variant) }

    /// Declares a kit badge (`text` on a 16-point pill in the badge look) named `name` and returns its node; `patch_text` on it updates a
    /// count without a rebuild.
    ///
    /// Role Label, named `name` (empty: none), with `text` as its accessible value. It holds no state.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the display text.
    /// Panics: when the library refuses the call (outside a frame). A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| { ui.badge("unread", "Unread", "3", &pxui::Badge::DEFAULT); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn badge<K: Into<Key>>(&mut self, key: K, name: &str, text: &str, badge: &Badge) -> Handle { self.badge_key(key.into(), name, text, badge.variant) }
}

#[cfg(test)]
mod tests {
    use pxui_types::ui::{Checkbox, Dropdown, Slider};
    use crate::{Segment, Tabs};
    use crate::bodies::{BadgeConfig, CheckboxConfig, DropdownConfig, RadioConfig, ProgressConfig, SliderConfig, SpinnerConfig, TabsConfig};
    use crate::ui::{Node, Role, Size};
    use crate::{testing, FrameIn, Ui};
    use std::cell::RefCell;

    thread_local! { static SEEN: RefCell<Vec<(&'static str, u16)>> = const { RefCell::new(Vec::new()) }; }
    fn seen(kind: &'static str, variant: u16) { SEEN.with(|s| s.borrow_mut().push((kind, variant))); }
    /// Each registered body builds one marker node of its own, named by its kind, so the custom structure shows in the tree.
    fn marker(ui: &mut Ui, kind: &'static str) { let n = ui.node(kind, Node { w: Size::Px(10.0), h: Size::Px(10.0), role: Role::Image, ..Node::DEFAULT }); ui.a11y_name(&n, kind); }
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
        ui.tabs("tabs", &[Segment::new("T")], Some(0), &Tabs::DEFAULT.variant(3));
        ui.dropdown("drop", "Drop", &[crate::Segment::new("x")], Some(0), &Dropdown::DEFAULT.variant(4));
        ui.slider("slide", "Slide", 0.5, 0.0..=1.0, &Slider::DEFAULT.variant(5));
        ui.progress("prog", "Prog", None, &crate::Progress::DEFAULT.variant(6));
        ui.spinner("spin", "Spin", &crate::Spinner::DEFAULT.variant(7));
        ui.badge("badge", "Badge", "7", &crate::Badge::DEFAULT.variant(8));
    }

    /// Every widget of the family is built through its body: with a registered body for each kind, the custom structure is what the tree
    /// holds, and each style's variant reaches its own body's config.
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
        assert_eq!(got, [("checkbox", 1), ("radio", 2), ("tab", 3), ("dropdown", 4), ("slider", 5), ("progress", 6), ("spinner", 7), ("badge", 8)]);
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
            let p = ui.progress("export", "Export", Some(0.5), &crate::Progress::DEFAULT).box_key;
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

    const TABS: [Segment; 3] = [Segment::new("Songs").detail("12").id(7), Segment::new("Albums").detail("3").id(8), Segment::new("Artists").id(9)];

    /// No tab is in use for `None`; a press puts one in use and, with `focus_on_click`, focuses the bar on the press; ids key the tabs
    /// (reordering keeps each tab's node and `chosen_id` names the item); the built-in body draws the detail after the text.
    #[test]
    fn tabs_have_no_selection_ids_and_details() {
        let chosen = std::cell::Cell::new(None);
        let (order, last_id) = (std::cell::Cell::new(false), std::cell::Cell::new(None));
        let mut build = |ui: &mut Ui| {
            const TOUCH: Tabs = Tabs::DEFAULT.focus_on_click(true);
            let r = if order.get() { ui.tabs("t", &[TABS[1], TABS[0], TABS[2]], chosen.get(), &TOUCH) } else { ui.tabs("t", &TABS, chosen.get(), &TOUCH) };
            if r.response.changed { chosen.set(r.chosen); last_id.set(r.chosen_id); }
        };
        let mut ui = crate::doc::frames(3, &mut build);
        let selected = |ui: &Ui| ui.a11y_nodes().iter().filter(|n| n.is_role(Role::Tab) && n.states & pxui_types::a11y::state::SELECTED != 0).count();
        assert_eq!(selected(&ui), 0, "none in use");
        let find = |ui: &Ui, name: &str| ui.a11y_nodes().iter().find(|n| n.is_role(Role::Tab) && ui.a11y_name_of(n) == name).map(|n| (n.id, n.rect)).expect("tab");
        let (songs, albums) = (find(&ui, "Songs"), find(&ui, "Albums"));
        let (x, y) = (f64::from(albums.1.x + 4.0), f64::from(albums.1.y + 4.0));
        // the press alone, before the release, already focuses the bar
        testing::frame(&mut ui, &[pxui_types::Event { kind: pxui_types::Event::POINTER_PRESSED, x, y, ..Default::default() }], &mut build);
        assert!(ui.a11y_nodes().iter().any(|n| n.is_role(Role::TabList) && n.states & pxui_types::a11y::state::FOCUSED != 0), "focused on press");
        testing::frame(&mut ui, &[pxui_types::Event { kind: pxui_types::Event::POINTER_RELEASED, x, y, ..Default::default() }], &mut build);
        testing::frame(&mut ui, &[], &mut build);
        assert_eq!((chosen.get(), last_id.get()), (Some(1), Some(8)));
        assert_eq!(selected(&ui), 1);
        order.set(true);
        chosen.set(Some(0));
        testing::frame(&mut ui, &[], &mut build);
        assert_eq!((find(&ui, "Songs").0, find(&ui, "Albums").0), (songs.0, albums.0), "an id keeps its node when the tabs reorder");
        let mut plain = |ui: &mut Ui| { ui.tabs("p", &[Segment::new("Songs")], None, &Tabs::DEFAULT); };
        testing::frame(&mut ui, &[], &mut plain);
        assert!(find(&ui, "Songs").1.w < songs.1.w, "the detail adds width");
    }

    /// A press the caller does not apply (its choice stays) leaves the focus where the press put it, the bar.
    #[test]
    fn a_refused_tab_press_keeps_the_focus() {
        let mut build = |ui: &mut Ui| { ui.tabs("t", &TABS, Some(0), &Tabs::DEFAULT.focus_on_click(true)); };
        let mut ui = crate::doc::frames(2, &mut build);
        let albums = testing::find(&ui, Role::Tab, "Albums").expect("tab");
        testing::click(&mut ui, f64::from(albums.x + 4.0), f64::from(albums.y + 4.0), &mut build);
        for _ in 0..3 { testing::frame(&mut ui, &[], &mut build); }
        assert!(ui.a11y_nodes().iter().any(|n| n.is_role(Role::TabList) && n.states & pxui_types::a11y::state::FOCUSED != 0), "the bar keeps the focus");
        let songs = ui.a11y_nodes().iter().find(|n| n.is_role(Role::Tab) && ui.a11y_name_of(n) == "Songs").expect("tab").states;
        assert!(songs & pxui_types::a11y::state::SELECTED != 0, "the caller's choice stands");
    }

    /// A warm frame of tab bars (ids, details, none in use) allocates nothing.
    #[test]
    fn warm_tab_frames_allocate_nothing() {
        let mut ui = crate::doc::frames(1, &mut |_| {});
        let mut build = |ui: &mut Ui| {
            ui.tabs("a", &TABS, None, &Tabs::DEFAULT);
            ui.tabs("b", &TABS, Some(1), &Tabs::DEFAULT.name("Main").focus_on_click(true));
            ui.tabs("c", &[Segment::new("One"), Segment::new("Two")], Some(0), &Tabs::DEFAULT);
        };
        for round in 0..20 {
            mem_sys::local_reset();
            testing::frame(&mut ui, &[], &mut build);
            if round > 2 { assert_eq!(mem_sys::local_alloc_count(), 0, "round {round}"); }
        }
    }
}
