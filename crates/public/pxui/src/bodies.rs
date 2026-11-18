//! Widget bodies: an app builds a widget kind's nodes itself. `Ui::register_toggle_body(name, f)` and its siblings (one per kind,
//! generated from `pxui_types::body_kinds!`) register `f`, a plain `fn(&mut Ui, &KindConfig)` (`ToggleConfig`, `MenuRowConfig`, ...: one wrapper per kind, named for the kind), through the one export
//! `pxui_body_register`; the theme's `toggle.body = NAME` line then picks
//! it. The body runs as a callback from the library under the callback panic rule: a panic in it is carried to the call that can run
//! callbacks (the frame) and the body calls after it are skipped. The config is the kind's C record behind its wrapper: borrowed until the body returns,
//! text fields are read with the wrapper's `text_of` (`BodyConfig`), handles are `Handle`s the body may restyle and enter, and slots run the caller's content.
use crate::Handle;
use crate::composite::{defer_callback_panic, deferred};
use crate::{as_text, check, status, Error, Ui};
use core::{ffi::c_void, mem::ManuallyDrop, ptr::NonNull};
use pxui_types::{geom::Rect, theme::{BodyId, BodyKind, Curve, Part, States}, ffi::{PxuiBodyFn, PxuiHintDesc, PxuiSlot, Slice}, Node, PxuiBox, Str};

use pxui_types::ffi::{PxuiToggleConfig, PxuiSliderConfig, PxuiProgressConfig, PxuiButtonConfig, PxuiIconButtonConfig, PxuiCheckConfig, PxuiSpinnerConfig, PxuiBadgeConfig, PxuiLabelConfig, PxuiFooterConfig, PxuiGripConfig, PxuiPanelConfig, PxuiDialogConfig, PxuiSectionConfig, PxuiInspectorRowConfig, PxuiSplitConfig, PxuiScrollConfig, PxuiToastConfig, PxuiTipConfig, PxuiTabConfig, PxuiSegmentConfig, PxuiDropdownConfig, PxuiMenuItemConfig, PxuiMenuRowConfig, PxuiRowConfig, PxuiTreeRowConfig, PxuiTableHeadConfig, PxuiPickerRowConfig, PxuiSearchFieldConfig, PxuiTextFieldConfig, PxuiLineFieldConfig, PxuiValueFieldConfig, PxuiPickerFieldConfig, PxuiSectionIndexConfig};

mod sealed {
    pub trait Sealed { fn bounds(&self) -> (usize, usize); }
}

/// What a registered Rust body reads a config through: the config wrapper (`ToggleConfig`, `MenuRowConfig`, ... one per C record,
/// `Deref` to its `Pxui*Config` record for the plain fields) and the accessors for its pointer fields. A wrapper cannot be built by
/// safe code and its record cannot be changed through it, and an accessor reads only a field that lies inside the wrapper's own
/// record (a copy of a field elsewhere reads as empty), so a body needs no `unsafe` and safe code cannot point one at memory.
///
/// ```
/// use pxui::bodies::{BodyConfig, ToggleConfig};
/// fn toggle(_ui: &mut pxui::Ui, config: &ToggleConfig) { let _label: &str = config.text_of(&config.label); }
/// let _register: fn(&mut pxui::Ui, &str) -> Result<_, pxui::Error> = |ui, name| ui.register_toggle_body(name, toggle);
/// ```
pub trait BodyConfig: sealed::Sealed {
    /// The text of a config field (`config.text_of(&config.label)`), borrowed from the config (valid until the body returns).
    ///
    /// Allocates: never.
    /// Panics: never; a field that is not part of this config, or text that is not UTF-8, reads as "".
    ///
    /// ```
    /// use pxui::bodies::BodyConfig;
    /// fn label(_ui: &mut pxui::Ui, config: &pxui::bodies::LabelConfig) { let _ = config.text_of(&config.text); }
    /// ```
    fn text_of(&self, field: &Str) -> &str {
        if !inside(self.bounds(), field as *const _ as usize, core::mem::size_of_val(field)) { return ""; }
        str_of(field)
    }

    /// Item `index` of a config's text list (a dialog body's `buttons`, a tooltip body's `lines`), borrowed from the config (valid until
    /// the body returns); "" past the end, for text that is not UTF-8 and for a list that is not part of this config.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::{BodyConfig, DialogConfig};
    /// fn dialog(_ui: &mut pxui::Ui, config: &DialogConfig) { let _ = config.text_at(&config.buttons, 1); }
    /// ```
    fn text_at(&self, list: &Slice<Str>, index: usize) -> &str {
        if !inside(self.bounds(), list as *const _ as usize, core::mem::size_of_val(list)) || index >= list.len || list.ptr.is_null() { return ""; }
        str_of(unsafe { &*list.ptr.add(index) })
    }

    /// Item `index` of a footer body's `hints` as (key, value) text, borrowed from the config; `None` past the end or for a list that is
    /// not part of this config.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::{BodyConfig, FooterConfig};
    /// fn footer(_ui: &mut pxui::Ui, config: &FooterConfig) { let _ = config.hint_at(&config.hints, 0); }
    /// ```
    fn hint_at(&self, list: &Slice<PxuiHintDesc>, index: usize) -> Option<(&str, &str)> {
        if !inside(self.bounds(), list as *const _ as usize, core::mem::size_of_val(list)) || index >= list.len || list.ptr.is_null() { return None; }
        let hint = unsafe { &*list.ptr.add(index) };
        Some((str_of(&hint.key), str_of(&hint.value)))
    }

    /// The looks per state a config field points at (`states` of a button, icon button or segmented config, `track` and `knob` of a
    /// toggle config: the caller's or the theme's), borrowed from the config; `None` for a null field or one that is not part of this config.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::{BodyConfig, ButtonConfig};
    /// fn button(_ui: &mut pxui::Ui, config: &ButtonConfig) { let _ = config.states_of(&config.states); }
    /// ```
    fn states_of(&self, field: &*const States) -> Option<&States> {
        if !inside(self.bounds(), field as *const _ as usize, core::mem::size_of_val(field)) || field.is_null() { return None; }
        Some(unsafe { &**field })
    }

    /// The press curves an icon button config's `press` points at (the scale's in on press and out on release), borrowed from the
    /// config; `None` when it is null (the kit's springs) or not part of this config.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::bodies::{BodyConfig, IconButtonConfig};
    /// fn icon_button(_ui: &mut pxui::Ui, config: &IconButtonConfig) { let _ = config.curves_of(&config.press); }
    /// ```
    fn curves_of(&self, field: &*const Curve) -> Option<&[Curve; 2]> {
        if !inside(self.bounds(), field as *const _ as usize, core::mem::size_of_val(field)) || field.is_null() { return None; }
        Some(unsafe { &*field.cast::<[Curve; 2]>() })
    }
}

/// The text a library-written `Str` names; "" when empty or not UTF-8. Callers vouch that `field` came from the config's records or lists.
fn str_of<'a>(field: &Str) -> &'a str {
    if field.len == 0 || field.ptr.is_null() { return ""; }
    core::str::from_utf8(unsafe { core::slice::from_raw_parts(field.ptr, field.len) }).unwrap_or("")
}

/// Whether the `size` bytes at address `at` lie wholly inside the record `bounds` (start, length) names.
fn inside((start, len): (usize, usize), at: usize, size: usize) -> bool { at >= start && at + size <= start + len }

library!(sys {
    fn pxui_body_register_checked(ui: *mut PxuiUi, kind: u32, name: Str, body: PxuiBodyFn, user: *mut c_void, out: *mut u16) -> i32;
    fn pxui_part(ui: *mut PxuiUi, part: u32, node: *const Node, out: *mut PxuiBox) -> i32;
    fn pxui_part_box(ui: *mut PxuiUi, owner: i64, part: u32, out: *mut PxuiBox) -> i32;
    #[trusted] fn pxui_slot(ui: *mut PxuiUi, slot: PxuiSlot, out: *mut u8) -> i32;
});

/// What every registered body's C entry does: skip once a panic is pending, otherwise run `body` on the borrowed `Ui` under `catch_unwind`.
fn enter(raw: *mut c_void, config: *const c_void, body: &mut dyn FnMut(&mut Ui)) {
    if raw.is_null() || config.is_null() || deferred() { return; }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ui = ManuallyDrop::new(Ui(NonNull::new(raw.cast()).unwrap()));
        body(&mut ui);
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}

macro_rules! typed {
    ($($kind:ident $register:ident $body:ident $config:ident $wrapper:ident),* $(,)?) => {
        $(
            #[doc = concat!("The `", stringify!($config), "` record a registered `", stringify!($kind), "` body receives (borrowed until the body returns): its plain fields read through `Deref`, its text, lists and looks through `BodyConfig`.")]
            #[repr(transparent)]
            pub struct $wrapper($config);
            impl core::ops::Deref for $wrapper { type Target = $config; fn deref(&self) -> &$config { &self.0 } }
            impl sealed::Sealed for $wrapper { fn bounds(&self) -> (usize, usize) { (self as *const Self as usize, core::mem::size_of::<$config>()) } }
            impl BodyConfig for $wrapper {}
        )*
        impl Ui {
            $(
                #[doc = concat!(
                    "Registers a ", stringify!($kind), " body for use in theme files.\n\n",
                    "Call between frames. A name is local to this Ui and kind. Registering it again replaces the function and keeps its id. ",
                    "Each kind accepts up to 65,535 names. An invalid name returns `Error::InvalidInput`. ",
                    "A registration during a frame or beyond that capacity returns `Error::State`. ",
                    "`Ui::last_error` gives the reason.\n",
                    "Allocates: when the registry grows.\nPanics: never.\n\n",
                    "```\nlet mut ui = pxui::doc::frames(1, &mut |_| {});\n",
                    "ui.", stringify!($register), "(\"doc-body\", |_, _| {}).unwrap();\n```"
                )]
                pub fn $register(&mut self, name: &str, body: fn(&mut Ui, &$wrapper)) -> Result<BodyId, Error> {
                    extern "C" fn entry(user: *mut c_void, raw: *mut c_void, config: *const c_void) {
                        enter(raw, config, &mut |ui| (unsafe { core::mem::transmute::<*mut c_void, fn(&mut Ui, &$wrapper)>(user) })(ui, unsafe { &*config.cast::<$wrapper>() }));
                    }
                    self.register_body(BodyKind::$kind, name, entry, body as *mut c_void)
                }
            )*
        }
    };
}
pxui_types::body_kinds!(typed);

impl Ui {
    fn register_body(&mut self, kind: BodyKind, name: &str, entry: extern "C" fn(*mut c_void, *mut c_void, *const c_void), user: *mut c_void) -> Result<BodyId, Error> {
        let mut id = 0;
        status(unsafe { sys::pxui_body_register_checked(self.0.as_ptr(), kind as u32, as_text(name), Some(entry), user, &mut id) })?;
        Ok(BodyId(id))
    }

    /// Builds a node as `part` of the widget whose body is running: behaviour finds it again by role (see the config's documentation
    /// for the parts its widget reads). Only inside a body.
    #[track_caller]
    pub fn part(&mut self, part: Part, node: Node) -> Handle {
        let mut out = Handle::default();
        check(self, unsafe { sys::pxui_part(self.0.as_ptr(), part.0, &node, &mut out) });
        out
    }

    /// Returns a named part of a widget from its last build.
    ///
    /// A stale owner or missing part returns None.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("owner", pxui::Node::DEFAULT);
    ///     assert!(ui.part_box(&node, pxui::Part::HIT).is_none());
    /// });
    /// ```
    pub fn part_box(&self, owner: &Handle, part: Part) -> Option<Handle> {
        let live = self.box_of_resolved_key(owner.box_key)?;
        if (live.box_slot, live.generation) != (owner.box_slot, owner.generation) { return None; }
        let mut out = Handle::default();
        (unsafe { sys::pxui_part_box(self.0.as_ptr(), owner.box_key, part.0, &mut out) } == 0).then_some(out)
    }

    /// Returns the part's surface rectangle from the last layout.
    ///
    /// A missing part, stale owner or part without a positive laid-out size returns None.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("owner", pxui::Node::DEFAULT);
    ///     assert!(ui.part_rect(&node, pxui::Part::HIT).is_none());
    /// });
    /// ```
    pub fn part_rect(&self, owner: &Handle, part: Part) -> Option<Rect> {
        self.part_box(owner, part).map(|b| self.rect(&b)).filter(|r| r.w > 0.0 && r.h > 0.0)
    }

    /// Runs a config's slot (the caller's content, built where the body calls this); for a search field's editor slot the answer is
    /// whether the query is not empty, otherwise false. A slot is valid only during the body call that received it, and not inside its own content or a nested body: else this panics.
    #[track_caller]
    pub fn slot(&mut self, slot: PxuiSlot) -> bool {
        let mut out = 0u8;
        check(self, unsafe { sys::pxui_slot(self.0.as_ptr(), slot, &mut out) });
        out != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ThemeHandle;
    use core::cell::{Cell, RefCell};
    use crate::FrameIn;
    use pxui_types::{Size};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    thread_local! {
        static TOGGLES: RefCell<Vec<(String, bool, bool)>> = const { RefCell::new(Vec::new()) };
        static ROWS: Cell<i64> = const { Cell::new(0) };
        static CONTENT: Cell<u32> = const { Cell::new(0) };
        static SLOT: Cell<PxuiSlot> = const { Cell::new(PxuiSlot { serial: 0 }) };
        static HINTS: RefCell<Option<(Option<(String, String)>, bool, bool, bool, bool)>> = const { RefCell::new(None) };
        static FORGED: Cell<(bool, bool)> = const { Cell::new((false, false)) };
        static BOOM: Cell<bool> = const { Cell::new(true) };
        static TAB: RefCell<Option<Handle>> = const { RefCell::new(None) };
        static TAB_DETAIL: RefCell<String> = const { RefCell::new(String::new()) };
    }

    fn input(time: f64) -> FrameIn<'static> { FrameIn::new(320.0, 240.0).time(time) }
    fn source(text: &str) -> ThemeHandle { ThemeHandle::parse(text).expect("theme source") }

    fn probe(ui: &mut Ui, c: &crate::bodies::ToggleConfig) {
        TOGGLES.with(|t| t.borrow_mut().push((c.text_of(&c.label).to_owned(), c.on != 0, c.state.hovered != 0)));
        ROWS.with(|r| r.set(c.row.box_key));
        let b = ui.part(Part::HIT, Node { w: Size::Px(12.0), h: Size::Px(12.0), ..Node::DEFAULT });
        ui.text(&b, "x");
    }

    /// A registered body is called with its kind's record (text, value, state, the row it owns), builds a part behaviour then finds, is picked
    /// by theme text (the Ui's own and a scoped theme) and only inside the scope that names it.
    #[test]
    fn a_registered_body_is_selected_by_theme_text_and_gets_its_config() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        let id = ui.register_toggle_body("probe", probe).expect("registered");
        assert_eq!(id, BodyId(1));
        assert_eq!(ui.register_toggle_body("probe", probe).expect("again"), id, "a name again keeps its id");
        let scoped = crate::ThemeHandle::parse("toggle.body = probe").expect("scoped theme");
        // outside the scope the built-in body builds
        for n in 0..2 { ui.frame(&input(n as f64), &mut |ui: &mut Ui| { ui.toggle("toggle", "plain", true, &crate::Toggle::DEFAULT); }).expect("frame"); }
        assert!(TOGGLES.with(|t| t.borrow().is_empty()), "the body ran with no theme naming it");
        for n in 2..4 {
            ui.frame(&input(n as f64), &mut |ui: &mut Ui| { ui.toggle("toggle", "outside", false, &crate::Toggle::DEFAULT); ui.themed(&scoped, &mut |ui: &mut Ui| { ui.toggle("toggle", "Wi-Fi", true, &crate::Toggle::DEFAULT); }); }).expect("frame");
        }
        let seen = TOGGLES.with(|t| t.borrow().clone());
        assert!(!seen.is_empty() && seen.iter().all(|s| *s == ("Wi-Fi".to_owned(), true, false)), "only the scoped toggle ran the body: {seen:?}");
        let row = ui.box_of_key(crate::Key::Resolved(ROWS.with(Cell::get))).expect("the row");
        assert!(ui.part_rect(&row, Part::HIT).is_some(), "the part the body built is found by its widget's handle");
        assert!(ui.part_box(&row, Part::TRACK).is_none());
        // the Ui's own theme, from the theme text
        TOGGLES.with(|t| t.borrow_mut().clear());
        ui.set_theme(&source("toggle.body = probe")).expect("theme text");
        assert_eq!(ui.theme().body.toggle, id);
        ui.frame(&input(5.0), &mut |ui: &mut Ui| { ui.toggle("toggle", "Own", false, &crate::Toggle::DEFAULT); }).expect("frame");
        assert_eq!(TOGGLES.with(|t| t.borrow().clone()), vec![("Own".to_owned(), false, false)]);
    }

    /// U1c.3 (D6): the extension set's inspector toggle and button are built through the toggle and button bodies, so a registered body
    /// draws them: the switch is a toggle without text, the button a button with its label.
    #[test]
    fn the_inspector_toggle_and_button_are_built_through_their_bodies() {
        thread_local! { static BUTTONS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) }; }
        fn counting(_: &mut Ui, c: &crate::bodies::ButtonConfig) { BUTTONS.with(|b| b.borrow_mut().push(c.text_of(&c.label).to_owned())); }
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_toggle_body("probe", probe).expect("toggle body");
        ui.register_button_body("counting", counting).expect("button body");
        ui.set_theme(&source("toggle.body = probe\nbutton.body = counting")).expect("theme text");
        TOGGLES.with(|t| t.borrow_mut().clear());
        ui.frame(&input(0.0), &mut |ui: &mut Ui| {
            let _ = crate::ext::inspector_toggle(ui, "snap", "Snap", true);
            let _ = crate::ext::inspector_button(ui, "reset", "Reset");
        }).expect("frame");
        assert_eq!(TOGGLES.with(|t| t.borrow().clone()), vec![(String::new(), true, false)], "the switch is the toggle body's");
        assert_eq!(BUTTONS.with(|b| b.borrow().clone()), vec!["Reset".to_owned()], "the button is the button body's");
    }

    /// A theme text that names no registered body, and every bad registration, are errors, not crashes.
    #[test]
    fn unknown_body_names_and_bad_registrations_are_errors() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        for name in ["default", "", "two words", "a=b", "x#"] {
            assert_eq!(ui.register_toggle_body(name, probe), Err(crate::Error::InvalidInput), "{name:?}");
            assert!(ui.last_error().contains("invalid body name"));
        }
        ui.register_toggle_body("probe", probe).unwrap();
        let before = *ui.theme();
        let invalid = source("tokens.accent = #ff0000\ntoggle.body = probe\nlabel.body = nope");
        assert_eq!(ui.set_theme(&invalid), Err(crate::Error::InvalidInput));
        assert!(ui.last_error().contains("nope"), "{}", ui.last_error());
        assert_eq!(*ui.theme(), before, "body validation precedes theme installation");
        // a body of another kind is not this kind's
        ui.register_slider_body("slider", |_, _| {}).expect("slider");
        assert_eq!(ui.set_theme(&source("toggle.body = slider")), Err(crate::Error::InvalidInput));
        let mut late = None;
        ui.frame(&input(0.0), &mut |ui: &mut Ui| { late = Some(ui.register_toggle_body("late", probe)); }).expect("frame");
        assert_eq!(late.expect("ran"), Err(crate::Error::State));
        assert!(ui.last_error().contains("outside a frame"));
        // a part outside a body is a caller bug (outside a frame, it is refused as every builder is)
        let refused = catch_unwind(AssertUnwindSafe(|| ui.part(Part::HIT, Node::DEFAULT)));
        assert!(refused.is_err());
        assert!(ui.last_error().contains("frame callback"), "{}", ui.last_error());
        ui.frame(&input(1.0), &mut |ui: &mut Ui| { ui.label("still usable", crate::Label::DEFAULT); }).expect("the Ui is not poisoned");
    }

    #[test]
    fn detached_theme_resolves_per_ui_and_scopes_use_the_builtin_fallback() {
        let theme = source("toggle.body = probe");
        let mut unregistered = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        assert_eq!(unregistered.set_theme(&theme), Err(crate::Error::InvalidInput));
        unregistered.frame(&input(0.0), &mut |ui| ui.themed(&theme, &mut |ui| {
            assert_eq!(ui.theme().body.toggle, BodyId::DEFAULT);
            ui.toggle("toggle", "Builtin", false, &crate::Toggle::DEFAULT);
        })).unwrap();

        let mut registered = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        registered.register_toggle_body("earlier", probe).unwrap();
        let id = registered.register_toggle_body("probe", probe).unwrap();
        assert_ne!(id, BodyId::DEFAULT);
        registered.set_theme(&theme).unwrap();
        assert_eq!(registered.theme().body.toggle, id);
    }

    fn boom(_: &mut Ui, _: &crate::bodies::ToggleConfig) {
        TOGGLES.with(|t| t.borrow_mut().push((String::new(), false, false)));
        if BOOM.with(Cell::get) { panic!("boom") }
    }

    /// A body that panics follows the callback panic rule: the panic is carried to the frame and resumed there, the bodies after it
    /// are skipped, and the next frame builds again.
    #[test]
    fn a_body_that_panics_follows_the_callback_panic_rule() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_toggle_body("boom", boom).expect("registered");
        ui.set_theme(&source("toggle.body = boom")).expect("theme");
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = ui.frame(&input(0.0), &mut |ui: &mut Ui| { ui.toggle("toggle", "first", false, &crate::Toggle::DEFAULT); ui.toggle("toggle", "second", true, &crate::Toggle::DEFAULT); });
        }));
        assert_eq!(result.expect_err("the panic surfaces at frame()").downcast_ref::<&str>(), Some(&"boom"));
        assert_eq!(TOGGLES.with(|t| t.borrow().len()), 1, "the second body ran on a half-built tree");
        BOOM.with(|b| b.set(false));
        ui.frame(&input(1.0), &mut |ui: &mut Ui| { ui.toggle("toggle", "first", false, &crate::Toggle::DEFAULT); ui.toggle("toggle", "second", true, &crate::Toggle::DEFAULT); }).expect("the next frame renders");
        assert_eq!(TOGGLES.with(|t| t.borrow().len()), 3);
    }

    fn forger(_: &mut Ui, c: &crate::bodies::ToggleConfig) {
        let mut copy: pxui_types::ffi::PxuiToggleConfig = **c;
        copy.label = Str { ptr: "forged".as_ptr(), len: 6 };
        FORGED.with(|f| f.set((c.text_of(&c.label) == "Real", c.text_of(&copy.label).is_empty())));
    }

    /// An accessor reads only fields inside the config the library gave: a forged or copied field reads as empty.
    #[test]
    fn accessors_refuse_fields_outside_their_config() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_toggle_body("forger", forger).expect("registered");
        ui.set_theme(&source("toggle.body = forger")).expect("theme");
        ui.frame(&input(0.0), &mut |ui: &mut Ui| { ui.toggle("toggle", "Real", true, &crate::Toggle::DEFAULT); }).expect("frame");
        assert_eq!(FORGED.with(Cell::get), (true, true));
    }

    fn hinter(_: &mut Ui, c: &crate::bodies::FooterConfig) {
        let real = c.hint_at(&c.hints, 0).map(|(k, v)| (k.to_owned(), v.to_owned()));
        let past = c.hint_at(&c.hints, c.hints.len).is_none();
        let copy = c.hints;
        let texts = [Str { ptr: "forged".as_ptr(), len: 6 }];
        let forged = pxui_types::ffi::Slice { ptr: texts.as_ptr(), len: 1 };
        let forged_hints = pxui_types::ffi::Slice::<pxui_types::ffi::PxuiHintDesc> { ptr: c.hints.ptr, len: c.hints.len };
        HINTS.with(|h| *h.borrow_mut() = Some((real, past, c.hint_at(&copy, 0).is_none(), c.text_at(&forged, 0).is_empty(), c.hint_at(&forged_hints, 0).is_none())));
    }

    /// A footer body reads its hints' real text; a list copied out of the config, or built by the app, reads as nothing.
    #[test]
    fn hint_and_list_accessors_read_library_lists_and_refuse_forged_ones() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_footer_body("hinter", hinter).expect("registered");
        ui.set_theme(&source("footer.body = hinter")).expect("theme");
        ui.frame(&input(0.0), &mut |ui: &mut Ui| { crate::ext::footer(ui, None, &[("Esc", "close")]); }).expect("frame");
        let seen = HINTS.with(|h| h.borrow().clone()).expect("the body ran");
        assert_eq!(seen, (Some(("Esc".to_owned(), "close".to_owned())), true, true, true, true));
    }

    fn scroll(ui: &mut Ui, c: &crate::bodies::ScrollViewConfig) {
        SLOT.with(|s| s.set(c.content));
        assert!(!ui.slot(c.content), "a content slot answers false");
    }

    /// A body runs the caller's content through its slot, once; the slot is stale after the body call and refused, without poisoning.
    #[test]
    fn a_slot_runs_the_callers_content_and_is_refused_once_stale() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_scroll_view_body("probe", scroll).expect("registered");
        ui.set_theme(&source("scroll_view.body = probe")).expect("theme");
        ui.frame(&input(0.0), &mut |ui: &mut Ui| { ui.scroll_view("list", crate::Node::DEFAULT.h(Size::Grow), &mut |ui: &mut Ui| { CONTENT.with(|c| c.set(c.get() + 1)); ui.label("row", crate::Label::DEFAULT); }); }).expect("frame");
        assert_eq!(CONTENT.with(Cell::get), 1);
        let stale = catch_unwind(AssertUnwindSafe(|| ui.slot(SLOT.with(Cell::get))));
        assert!(stale.is_err());
        assert!(ui.last_error().contains("stale slot"), "{}", ui.last_error());
        ui.frame(&input(1.0), &mut |ui: &mut Ui| { ui.label("still usable", crate::Label::DEFAULT); }).expect("the Ui is not poisoned");
    }

    /// Content run through a slot that panics follows the callback panic rule too (in the static build it unwinds through the slot to
    /// the body's entry): the panic surfaces at the frame, and the next frame's body runs its slot again.
    #[test]
    fn a_slot_whose_content_panics_follows_the_callback_panic_rule() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_scroll_view_body("probe", |ui, c| { ui.slot(c.content); }).expect("registered");
        ui.set_theme(&source("scroll_view.body = probe")).expect("theme");
        let ran = Cell::new(0);
        let build = |ui: &mut Ui, boom: bool| { ui.scroll_view("list", crate::Node::DEFAULT.h(Size::Grow), &mut |_: &mut Ui| { ran.set(ran.get() + 1); if boom { panic!("content") } }); };
        let result = catch_unwind(AssertUnwindSafe(|| { let _ = ui.frame(&input(0.0), &mut |ui: &mut Ui| build(ui, true)); }));
        assert_eq!(result.expect_err("the panic surfaces at frame()").downcast_ref::<&str>(), Some(&"content"));
        ui.frame(&input(1.0), &mut |ui: &mut Ui| build(ui, false)).expect("the next frame renders");
        assert_eq!(ran.get(), 2, "the slot ran again the frame after");
    }

    fn wide_tab(ui: &mut Ui, c: &crate::bodies::TabsConfig) {
        let mut node = ui.node_layout(&c.node);
        node.w = Size::Px(100.0);
        ui.set_node_layout(&c.node, &node);
        ui.text(&c.node, c.text_of(&c.text));
        if c.index == 1 { TAB.with(|t| *t.borrow_mut() = Some(c.node)); TAB_DETAIL.with(|d| *d.borrow_mut() = c.text_of(&c.detail).to_string()); }
    }

    /// A body sizes the node its widget gave it (`set_node_layout`): the second tab is 100 points wide. A registered tab body is given
    /// the tab's detail (`Segment::detail`, `PxuiTabConfig.detail`).
    #[test]
    fn a_body_sizes_its_node_with_node_layout() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_tabs_body("wide", wide_tab).expect("registered");
        ui.set_theme(&source("tabs.body = wide")).expect("theme");
        for n in 0..3 { ui.frame(&input(n as f64), &mut |ui: &mut Ui| { ui.tabs("t", &[crate::Segment::new("a"), crate::Segment::new("b").detail("12")], Some(0), &crate::Tabs::DEFAULT); }).expect("frame"); }
        let tab = TAB.with(|t| t.borrow().expect("the body ran"));
        assert_eq!(ui.rect(&tab).w, 100.0);
        assert_eq!(TAB_DETAIL.with(|d| d.borrow().clone()), "12");
    }

    /// The `_with` forms build the caller's own children for each row and cell, in both link modes.
    #[test]
    fn collections_build_the_callers_cells() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        let (mut list, mut open, mut visible) = (crate::ListState::new(), vec![0u8; 3], vec![0u32; 3]);
        let nodes = [crate::TreeNode { depth: 0, children: 1 }, crate::TreeNode { depth: 1, children: 0 }, crate::TreeNode { depth: 0, children: 0 }];
        let (mut table, mut tree_selected) = (crate::TableState::new(), None);
        const COLUMNS: [crate::Column; 2] = [crate::Column::new("a").w_px(80.0), crate::Column::new("b").w_px(80.0)];
        let (rows, cells, nodes_built) = (Cell::new(0), Cell::new(0), Cell::new(0));
        for n in 0..3 {
            ui.frame(&input(n as f64), &mut |ui: &mut Ui| {
                ui.selection_list("list", 5, &|i| i as i64, &mut list, &crate::SelectionList::DEFAULT, &mut |ui: &mut Ui, part, _| if let crate::ui::SectionPart::Row(row) = part { rows.set(rows.get() + 1); ui.label(&format!("row {row}"), crate::Label::DEFAULT); });
                let shown = ui.tree("tree", &nodes, &mut open, &mut tree_selected, &mut visible, &mut |ui: &mut Ui, node| { nodes_built.set(nodes_built.get() + 1); ui.label(&format!("node {node}"), crate::Label::DEFAULT); }).shown;
                assert_eq!(shown, 2, "the first node's child is closed");
                let _ = ui.table("table", &COLUMNS, 3, &|i| i as i64, &mut table, &crate::Table::DEFAULT, &mut |ui: &mut Ui, row, column, _| { cells.set(cells.get() + 1); ui.label(&format!("{row}:{column}"), crate::Label::DEFAULT); });
            }).expect("frame");
        }
        assert!(rows.get() > 0 && nodes_built.get() > 0 && cells.get() > 0, "rows {}, nodes {}, cells {}", rows.get(), nodes_built.get(), cells.get());
        assert_eq!(visible[..2], [0, 2]);
    }

    thread_local! { static VARIANTS: RefCell<Vec<(&'static str, u16)>> = const { RefCell::new(Vec::new()) }; }

    /// One body per kind that builds two structures by `variant`: variant 1 a 120-point node, else a 40-point one.
    fn sized_label(ui: &mut Ui, c: &crate::bodies::LabelConfig) {
        VARIANTS.with(|v| v.borrow_mut().push(("label", c.variant)));
        let mut node = ui.node_layout(&c.node);
        node.w = Size::Px(if c.variant == 1 { 120.0 } else { 40.0 });
        ui.set_node_layout(&c.node, &node);
    }
    fn sized_button(ui: &mut Ui, c: &crate::bodies::ButtonConfig) {
        VARIANTS.with(|v| v.borrow_mut().push(("button", c.variant)));
        let mut node = ui.node_layout(&c.row);
        node.w = Size::Px(if c.variant == 1 { 120.0 } else { 40.0 });
        ui.set_node_layout(&c.row, &node);
        ui.text(&c.row, c.text_of(&c.label));
    }
    fn sized_field(_: &mut Ui, c: &crate::bodies::TextFieldConfig) { VARIANTS.with(|v| v.borrow_mut().push(("field", c.variant))); }
    fn sized_icon_button(_: &mut Ui, c: &crate::bodies::IconButtonConfig) { VARIANTS.with(|v| v.borrow_mut().push(("icon", c.variant))); }

    /// A call's style `variant` reaches the registered body's config (labels, buttons, icon buttons, text fields; 0 when the call sets none), so one body
    /// builds two structures and the app picks per call without comparing text; the variant never reaches a later widget, a built-in body
    /// ignores it, and warm frames allocate nothing.
    #[test]
    fn a_style_variant_reaches_the_registered_body_and_picks_its_structure() {
        use crate::{Button, Icon, IconButton, Label};
        mem_sys::set_peer(crate::library_counter);
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_label_body("sized", sized_label).expect("label body");
        ui.register_button_body("sized", sized_button).expect("button body");
        ui.register_icon_button_body("sized", sized_icon_button).expect("icon body");
        ui.register_text_field_body("sized", sized_field).expect("field body");
        let scoped = crate::ThemeHandle::parse("label.body = sized\nbutton.body = sized\nicon_button.body = sized\ntext_field.body = sized").expect("theme");
        let (plain, wide) = (Label::DEFAULT, Label::DEFAULT.variant(1));
        let handles = Cell::new((None, None));
        let mut build = |ui: &mut Ui| {
            // built-in bodies ignore the variant and leave nothing behind for the next widget
            ui.label("built-in", wide);
            ui.themed(&scoped, &mut |ui: &mut Ui| {
                ui.label("plain", plain);
                ui.label("wide", wide);
                handles.set((Some(ui.button("narrow", "Narrow", &Button::DEFAULT).handle), Some(ui.button("wide", "Wide", &Button::DEFAULT.variant(1)).handle)));
                ui.icon_button("icon", Icon::Glyph('x'), "Icon", &IconButton::DEFAULT.variant(7));
                ui.icon_button("icon2", Icon::Glyph('x'), "Icon", &IconButton::DEFAULT);
                ui.text_field("Find", "", &crate::TextField::SEARCH.variant(3));
                ui.text_field("Other", "", &crate::TextField::SEARCH);
            });
        };
        for n in 0..3 { VARIANTS.with(|v| v.borrow_mut().clear()); ui.frame(&input(n as f64), &mut build).expect("frame"); }
        let seen = VARIANTS.with(|v| v.borrow().clone());
        assert_eq!(seen, vec![("label", 0), ("label", 1), ("button", 0), ("button", 1), ("icon", 7), ("icon", 0), ("field", 3), ("field", 0)], "each call's own variant, none leaked");
        let (narrow, wide) = handles.get();
        assert_eq!((ui.rect(&narrow.unwrap()).w, ui.rect(&wide.unwrap()).w), (40.0, 120.0), "one body, two structures by variant");
        let scope = mem_sys::warm_scope();
        for n in 3..60 {
            VARIANTS.with(|v| v.borrow_mut().clear());
            mem_sys::local_reset();
            ui.frame(&input(n as f64), &mut build).expect("frame");
            assert_eq!(mem_sys::local_alloc_count(), 0, "warm frames with variants allocate nothing");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    fn menu_row(_: &mut Ui, c: &crate::bodies::MenuRowConfig) { VARIANTS.with(|v| v.borrow_mut().push(("row", c.variant))); }
    fn toggle_probe(_: &mut Ui, c: &crate::bodies::ToggleConfig) { VARIANTS.with(|v| v.borrow_mut().push(("toggle", c.variant))); }
    fn segment_probe(_: &mut Ui, c: &crate::bodies::SegmentedConfig) { VARIANTS.with(|v| v.borrow_mut().push(("segment", c.variant))); }
    fn nested_label(_: &mut Ui, c: &crate::bodies::LabelConfig) { VARIANTS.with(|v| v.borrow_mut().push(("nested", c.variant))); }

    /// A menu's variant reaches every row's body, a toggle's and a segmented control's reach theirs (every segment); the contents of a button with a variant are given none.
    #[test]
    fn a_variant_reaches_every_menu_row_and_never_a_nested_widget() {
        use crate::{ui::{Menu, MenuItem}, Button, Label};
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_menu_row_body("rows", menu_row).expect("row body");
        ui.register_label_body("nested", nested_label).expect("label body");
        ui.register_toggle_body("t", toggle_probe).expect("toggle body");
        ui.register_segmented_body("s", segment_probe).expect("segment body");
        let scoped = crate::ThemeHandle::parse("menu_row.body = rows\nlabel.body = nested\ntoggle.body = t\nsegmented.body = s").expect("theme");
        const ITEMS: [MenuItem; 3] = [MenuItem::new("A"), MenuItem::new("B"), MenuItem::new("C")];
        let mut build = |ui: &mut Ui| ui.themed(&scoped, &mut |ui: &mut Ui| {
            ui.menu("menu", crate::Point::new(10.0, 10.0), &ITEMS, &Menu::DEFAULT.variant(2));
            ui.menu("plain", crate::Point::new(100.0, 10.0), &ITEMS, &Menu::DEFAULT);
            // the built-in button body runs the contents; the registered label inside is given no variant
            ui.button_content("pill", &Button::DEFAULT.variant(1).size(Size::Px(60.0), Size::Px(30.0)).name("Pill"), |ui, _| { ui.label("inside", Label::DEFAULT); });
            ui.label("after", Label::DEFAULT.variant(5));
            ui.toggle("t", "T", true, &crate::Toggle::DEFAULT.variant(4));
            ui.toggle("t0", "T", true, &crate::Toggle::DEFAULT);
            ui.segmented("s", &[crate::Segment::new("a"), crate::Segment::new("b")], Some(0), &crate::Segmented::DEFAULT.variant(6));
        });
        for n in 0..3 { VARIANTS.with(|v| v.borrow_mut().clear()); ui.frame(&input(n as f64), &mut build).expect("frame"); }
        let seen = VARIANTS.with(|v| v.borrow().clone());
        let rows: Vec<u16> = seen.iter().filter(|s| s.0 == "row").map(|s| s.1).collect();
        assert_eq!(rows, [2, 2, 2, 0, 0, 0], "the menu's three rows, then the plain menu's: {seen:?}");
        let others: Vec<(&str, u16)> = seen.iter().filter(|s| s.0 == "toggle" || s.0 == "segment").copied().collect();
        assert_eq!(others, [("toggle", 4), ("toggle", 0), ("segment", 6), ("segment", 6)], "a toggle's and every segment's body: {seen:?}");
        let nested: Vec<u16> = seen.iter().filter(|s| s.0 == "nested").map(|s| s.1).collect();
        assert_eq!(nested, [0, 5], "the label in the button's contents sees 0, the label with its own variant 5: {seen:?}");
    }

    fn scroll_probe(ui: &mut Ui, c: &crate::bodies::ScrollViewConfig) {
        VARIANTS.with(|v| v.borrow_mut().push(("scroll", c.variant)));
        ui.slot(c.content);
    }

    /// A menu that scrolls (`max_rows`) scrolls its sheet and builds no `scroll_view` inside its call (ABI 1.58); the pending variant is for
    /// the menu's rows, so every row sees it.
    #[test]
    fn a_menu_variant_is_for_its_rows_and_a_long_menu_builds_no_scroll_view() {
        use crate::ui::{Menu, MenuItem};
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_menu_row_body("rows", menu_row).expect("row body");
        ui.register_scroll_view_body("scroll", scroll_probe).expect("scroll body");
        let scoped = crate::ThemeHandle::parse("menu_row.body = rows\nscroll_view.body = scroll").expect("theme");
        const ITEMS: [MenuItem; 5] = [MenuItem::new("A"), MenuItem::new("B"), MenuItem::new("C"), MenuItem::new("D"), MenuItem::new("E")];
        let mut build = |ui: &mut Ui| ui.themed(&scoped, &mut |ui: &mut Ui| { ui.menu("menu", crate::Point::new(10.0, 10.0), &ITEMS, &Menu::DEFAULT.max_rows(2).variant(1)); });
        for n in 0..3 { VARIANTS.with(|v| v.borrow_mut().clear()); ui.frame(&input(n as f64), &mut build).expect("frame"); }
        let seen = VARIANTS.with(|v| v.borrow().clone());
        assert!(!seen.iter().any(|s| s.0 == "scroll"), "the sheet scrolls, no scroll view body runs: {seen:?}");
        let rows: Vec<u16> = seen.iter().filter(|s| s.0 == "row").map(|s| s.1).collect();
        assert!(!rows.is_empty() && rows.iter().all(|v| *v == 1), "every row sees 1: {seen:?}");
    }

    /// A variant a caller left pending between frames is gone when the next frame starts.
    #[test]
    fn a_pending_variant_does_not_survive_into_the_next_frame() {
        use pxui_types::theme::BodyKind;
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_label_body("nested", nested_label).expect("label body");
        let scoped = crate::ThemeHandle::parse("label.body = nested").expect("theme");
        ui.set_variant(BodyKind::label, 9);
        VARIANTS.with(|v| v.borrow_mut().clear());
        ui.frame(&input(0.0), &mut |ui: &mut Ui| ui.themed(&scoped, &mut |ui: &mut Ui| { ui.label("x", crate::Label::DEFAULT); })).expect("frame");
        assert_eq!(VARIANTS.with(|v| v.borrow().clone()), [("nested", 0)]);
    }
}
