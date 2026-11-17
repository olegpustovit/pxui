//! The modal dialog: `ui.dialog(key, title, &buttons, &Dialog, body)` reports a `DialogResponse`.
use super::Key;
use crate::{Dialog, DialogButton, DialogResponse, Ui};

impl Ui {
    /// Shows a modal dialog titled `title` with `body` between its title bar and its `buttons`, and reports what the frame did.
    ///
    /// Call it every frame the dialog is open and stop when it reports `chosen` or `dismissed`. The buttons sit at the end of the last row
    /// in the order given, built through the dialog and button bodies. Return chooses the `default` button, Escape (and a press outside
    /// the dialog, a lost window focus: `dismissed`) the `cancel` one; a disabled button takes no press and is skipped by both.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the title.
    /// Panics: when the library refuses the arguments (outside a frame, more than 64 buttons, a negative or infinite width).
    ///
    /// ```
    /// use pxui::{Dialog, DialogButton};
    /// const BUTTONS: [DialogButton; 2] = [DialogButton::new("Cancel").cancel(), DialogButton::new("Delete").default().danger()];
    /// let mut chosen = None;
    /// let mut ui = pxui::doc::frames(2, &mut |ui: &mut pxui::Ui| {
    ///     chosen = ui.dialog("delete", "Delete layer", &BUTTONS, &Dialog::DEFAULT, &mut |ui: &mut pxui::Ui| { ui.label("This cannot be undone.", pxui::Label::DEFAULT); }).chosen;
    /// });
    /// assert!(chosen.is_none() && ui.a11y_nodes().iter().any(|n| ui.a11y_name_of(n) == "Delete"));
    /// ```
    #[inline]
    #[track_caller]
    pub fn dialog<K: Into<Key>>(&mut self, key: K, title: &str, buttons: &[DialogButton], dialog: &Dialog, body: &mut dyn FnMut(&mut Ui)) -> DialogResponse {
        self.dialog_call(key.into(), title, buttons, dialog, body)
    }

    /// The one body of `dialog`: the engine call, with the dialog's variant set around it.
    #[inline(always)]
    #[track_caller]
    fn dialog_call(&mut self, key: Key, title: &str, buttons: &[DialogButton], dialog: &Dialog, body: &mut dyn FnMut(&mut Ui)) -> DialogResponse {
        if dialog.variant == 0 { return self.dialog_key(key, title, buttons, dialog, body); }
        self.set_variant(crate::BodyKind::dialog, dialog.variant);
        let response = self.dialog_key(key, title, buttons, dialog, body);
        self.set_variant(crate::BodyKind::dialog, 0);
        response
    }

    /// The one body of `dialog`: the key's label, then the engine call.
    #[inline(always)]
    #[track_caller]
    fn dialog_key(&mut self, key: Key, title: &str, buttons: &[DialogButton], dialog: &Dialog, body: &mut dyn FnMut(&mut Ui)) -> DialogResponse {
        let mut id_label = [0u8; 17];
        self.dialog_keyed(key.label(&mut id_label), title, buttons, dialog, body)
    }
}

#[cfg(test)]
mod tests {
    use crate::bodies::PxuiDialogConfig;
    use crate::ui::{Flags, Node, Role, Size};
    use crate::{testing, Dialog, DialogButton, DialogResponse, Event, FrameIn, Part, Ui};
    use core::cell::{Cell, RefCell};

    const ENTER: u32 = pxui_types::KEY_SPECIAL + 5;
    const ESCAPE: u32 = pxui_types::KEY_SPECIAL + 6;

    fn key(ui: &mut Ui, code: u32, build: &mut dyn FnMut(&mut Ui)) -> DialogResponse {
        let events = [Event { kind: Event::KEY_PRESSED, code, ..Default::default() }, Event { kind: Event::KEY_RELEASED, code, ..Default::default() }];
        testing::frame(ui, &events, build);
        LAST.with(Cell::get)
    }

    thread_local! {
        static LAST: Cell<DialogResponse> = const { Cell::new(DialogResponse { chosen: None, dismissed: false }) };
        static BUILT: Cell<u32> = const { Cell::new(0) };
        static SAVE: Cell<bool> = const { Cell::new(true) };
        static SEEN: RefCell<Vec<(usize, u64, u64, i64, i64)>> = const { RefCell::new(Vec::new()) };
    }

    /// Delete layer: Cancel (Escape), Save (Return, off while `save` is false), Delete in the danger ink.
    fn build_with(ui: &mut Ui) {
        let buttons = [DialogButton::new("Cancel").cancel(), DialogButton::new("Save").default().enabled(SAVE.get()), DialogButton::new("Delete").danger()];
        let r = ui.dialog("layer", "Delete layer", &buttons, &Dialog::DEFAULT, &mut |ui: &mut Ui| { BUILT.set(BUILT.get() + 1); ui.label("This cannot be undone.", crate::Label::DEFAULT); });
        if r.chosen.is_some() || r.dismissed { LAST.set(r); }
    }

    fn click_button(ui: &mut Ui, name: &str, build: &mut dyn FnMut(&mut Ui)) -> DialogResponse {
        LAST.set(DialogResponse { chosen: None, dismissed: false });
        let r = testing::find(ui, Role::Button, name).unwrap_or_else(|| panic!("no button {name}"));
        testing::click(ui, f64::from(r.x + r.w / 2.0), f64::from(r.y + r.h / 2.0), build);
        LAST.with(Cell::get)
    }

    /// Return chooses the default button, Escape the cancel one (dismissed), a click the one clicked; the body closure builds inside
    /// the dialog's sheet, the sheet is named by the title; a disabled button takes no click and Return skips it.
    #[test]
    fn return_escape_and_clicks_choose_and_a_disabled_button_is_inert() {
        SAVE.set(true);
        let mut build = build_with;
        let mut ui = crate::doc::frames(3, &mut build);
        assert!(BUILT.get() > 0 && testing::find(&ui, Role::Dialog, "Delete layer").is_some(), "the body builds inside the modal sheet");
        assert!(testing::find(&ui, Role::Heading, "Delete layer").is_some());
        let none = DialogResponse { chosen: None, dismissed: false };
        assert_eq!(click_button(&mut ui, "Delete", &mut build), DialogResponse { chosen: Some(2), dismissed: false });
        assert_eq!(click_button(&mut ui, "Cancel", &mut build), DialogResponse { chosen: Some(0), dismissed: false });
        LAST.set(none);
        assert_eq!(key(&mut ui, ENTER, &mut build), DialogResponse { chosen: Some(1), dismissed: false });
        LAST.set(none);
        assert_eq!(key(&mut ui, ESCAPE, &mut build), DialogResponse { chosen: Some(0), dismissed: true });
        let mut ui = crate::doc::frames(3, &mut build);
        // Save off: no click, and Return finds no enabled default
        SAVE.set(false);
        let mut off = build_with;
        testing::frame(&mut ui, &[], &mut off);
        testing::frame(&mut ui, &[], &mut off);
        assert_eq!(click_button(&mut ui, "Save", &mut off), none);
        LAST.set(none);
        assert_eq!(key(&mut ui, ENTER, &mut off), none);
    }

    /// A dialog that has no cancel button reports `dismissed` alone on Escape.
    #[test]
    fn escape_without_a_cancel_button_only_dismisses() {
        let mut build = |ui: &mut Ui| {
            let r = ui.dialog("d", "Title", &[DialogButton::new("OK").default()], &Dialog::DEFAULT.width(200.0), &mut |_: &mut Ui| {});
            if r.chosen.is_some() || r.dismissed { LAST.set(r); }
        };
        let mut ui = crate::doc::frames(3, &mut build);
        assert_eq!(key(&mut ui, ESCAPE, &mut build), DialogResponse { chosen: None, dismissed: true });
    }

    fn column(ui: &mut Ui, c: &PxuiDialogConfig) {
        assert!(c.size as usize >= core::mem::size_of::<PxuiDialogConfig>(), "this library carries the flags");
        ui.slot(c.content);
        for i in 0..c.buttons.len {
            SEEN.with(|s| s.borrow_mut().push((i, c.danger, c.disabled, c.default_at, c.cancel_at)));
            let n = ui.part(Part::button(i as u8), Node { flags: Flags::CLICKABLE | Flags::FOCUSABLE | Flags::BLOCKING | Flags::TAB_ONLY, w: Size::Grow, h: Size::Px(24.0), ..Node::DEFAULT });
            ui.text(&n, "");
        }
    }

    /// A registered dialog body builds the dialog: it is given the buttons' flags, its structure is what shows (full-width buttons), a
    /// click on its part chooses the button, and the press of a disabled one is ignored.
    #[test]
    fn a_registered_dialog_body_builds_the_dialog() {
        let mut ui = Ui::new().unwrap();
        ui.set_a11y(true);
        let id = ui.register_dialog_body("column", column).expect("registered");
        let text = "dialog.body = column";
        ui.set_theme(&pxui_types::ffi::ThemeSource { size: core::mem::size_of::<pxui_types::ffi::ThemeSource>() as u32, flags: 0, source: pxui_types::Str { ptr: text.as_ptr(), len: text.len() } }).unwrap();
        assert_eq!(ui.theme().body.dialog, id);
        SAVE.set(false);
        let mut build = build_with;
        for _ in 0..3 { ui.frame(&FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() }, &mut build).unwrap(); }
        let seen = SEEN.with(|s| s.borrow().clone());
        assert!(seen.len() >= 3 && seen[seen.len() - 3..] == [(0, 4, 2, 1, 0), (1, 4, 2, 1, 0), (2, 4, 2, 1, 0)], "{seen:?}");
        let cancel = testing::find(&ui, Role::Button, "Cancel").unwrap();
        assert!(cancel.w > 200.0, "the body's structure shows: {cancel:?}");
        let none = DialogResponse { chosen: None, dismissed: false };
        assert_eq!(click_button(&mut ui, "Delete", &mut build), DialogResponse { chosen: Some(2), dismissed: false });
        assert_eq!(click_button(&mut ui, "Save", &mut build), none, "a disabled part's press is ignored");
    }

    thread_local! { static VARIANTS: RefCell<Vec<(&'static str, u16)>> = const { RefCell::new(Vec::new()) }; }

    fn variant_dialog(ui: &mut Ui, c: &PxuiDialogConfig) {
        VARIANTS.with(|v| v.borrow_mut().push(("dialog", c.variant)));
        ui.slot(c.content);
    }

    fn variant_label(_: &mut Ui, c: &crate::bodies::PxuiLabelConfig) { VARIANTS.with(|v| v.borrow_mut().push(("label", c.variant))); }

    /// `Dialog::variant` reaches the registered dialog body's config and nothing else: the label built in the dialog's content, inside
    /// the body, sees 0, and a dialog without a variant is given 0.
    #[test]
    fn a_dialog_variant_reaches_the_dialog_body_and_not_its_content() {
        let mut ui = Ui::new().unwrap();
        ui.register_dialog_body("v", variant_dialog).expect("dialog body");
        ui.register_label_body("l", variant_label).expect("label body");
        let scoped = { let mut theme = crate::Theme::default(); theme.apply(&ui, "dialog.body = v\nlabel.body = l").expect("theme"); theme };
        let mut build = |ui: &mut Ui| ui.themed(&scoped, &mut |ui: &mut Ui| {
            for (key, style) in [("a", Dialog::DEFAULT.variant(3)), ("b", Dialog::DEFAULT)] {
                ui.dialog(key, "T", &[DialogButton::new("OK").default()], &style, &mut |ui: &mut Ui| { ui.label("inside", crate::Label::DEFAULT); });
            }
        });
        for _ in 0..3 { VARIANTS.with(|v| v.borrow_mut().clear()); ui.frame(&FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() }, &mut build).unwrap(); }
        let seen = VARIANTS.with(|v| v.borrow().clone());
        let of = |name: &str| seen.iter().filter(|s| s.0 == name).map(|s| s.1).collect::<Vec<_>>();
        assert_eq!(of("dialog"), [3, 0], "the dialog body is given its call's variant: {seen:?}");
        assert!(!of("label").is_empty() && of("label").iter().all(|v| *v == 0), "the content's label sees 0: {seen:?}");
    }

    /// An open dialog (title, body, three buttons) allocates nothing on a warm frame, with and without hover.
    #[test]
    fn warm_frames_allocate_nothing() {
        SAVE.set(true);
        let mut build = build_with;
        let mut ui = crate::doc::frames(1, &mut |_| {});
        for round in 0..20 {
            mem_sys::local_reset();
            testing::frame(&mut ui, &[], &mut build);
            if round > 3 { assert_eq!(mem_sys::local_alloc_count(), 0, "round {round}"); }
        }
    }
}
