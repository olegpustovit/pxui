//! The overlay family: `ui.panel`, `ui.popup`, `ui.modal` and `ui.anchored` take a key and one `Overlay` style; `ui.scroll_view`,
//! `ui.toasts` and `ui.picker` take a key. Each is built through its body (panel, window, popup, modal, scroll_view, toast,
//! picker_field), so a registered body replaces its structure.
use crate::Handle;
use super::Key;
use crate::{PickerOpts, Ui};
use pxui_types::{geom::Rect, ui::{Overlay, PickRows, PickerResponse, Toast}, Size};

impl Ui {
    /// Declares a floating panel placed and dressed by `overlay` (280 wide at (12, 12) by default) and builds `f` inside it.
    ///
    /// Content sets its height unless `Overlay::h` fixes it; rows past the height or `Overlay::max_h` scroll. `Overlay::window` lays
    /// inspector rows and heads out as a floating window's. It is named by `Overlay::name` (none without).
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the name.
    /// Panics: when the library refuses the overlay (outside a frame, a size or position that is not finite, a negative padding), or
    /// when `f` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::Overlay;
    /// const SETTINGS: Overlay = Overlay::DEFAULT.w(296.0).name("Settings");
    /// pxui::doc::frames(1, &mut |ui| ui.panel("settings", &SETTINGS, |ui| { ui.label("Display", pxui::Label::DEFAULT); }));
    /// ```
    #[inline]
    #[track_caller]
    pub fn panel<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, mut f: F) { self.panel_key(key.into(), overlay, &mut f) }

    /// Declares a popup at `Overlay::at`, `Overlay::w` wide (at least 180), builds `f` in it and returns whether it is open.
    ///
    /// Escape, window focus loss or a press outside its last laid-out bounds dismisses it: `f` is skipped and `false` returned, so the
    /// caller drops its open state. `Overlay::h` is its hit area before it is first laid out; `Overlay::dismiss_initial` lets a press
    /// dismiss it on that first frame; an `Overlay::attached` popup (a submenu) shares the popup built before it. Build it before the
    /// body it shields; it paints on top. Named by `Overlay::name` (none without).
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the name.
    /// Panics: when the library refuses the overlay (outside a frame, a size or position that is not finite), or when `f` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::Overlay;
    /// let mut ran = false;
    /// pxui::doc::frames(1, &mut |ui| { ran = ui.popup("song-menu", &Overlay::DEFAULT.at(10.0, 10.0).w(200.0).h(60.0), |_| {}); });
    /// assert!(ran);
    /// ```
    #[inline]
    #[track_caller]
    pub fn popup<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, mut f: F) -> bool { self.popup_key(key.into(), overlay, &mut f) }

    /// Declares a modal sheet centred in the content area, `Overlay::w` wide (320 by default), builds `f` in it and returns whether it is
    /// open.
    ///
    /// Escape, window focus loss or a press outside it dismisses it: `f` is skipped and `false` returned. Build it before the body it
    /// shields, at the root level; it paints on top. Named by `Overlay::name` (none without).
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the name.
    /// Panics: when the library refuses the overlay (outside a frame, a width that is not finite or is negative), or when `f` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::Overlay;
    /// const CONFIRM: Overlay = Overlay::DEFAULT.w(260.0).name("Delete song?");
    /// pxui::doc::frames(1, &mut |ui| { ui.modal("confirm-delete", &CONFIRM, |ui| { ui.label("Delete?", pxui::Label::BODY); }); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn modal<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, mut f: F) -> bool { self.modal_key(key.into(), overlay, &mut f) }

    /// Declares a sheet placed by `anchor` (window points, e.g. `ui.rect(&field)`) at `Overlay::place`, `Overlay::w` by `Overlay::h`
    /// (each its content's when unset), builds `f` in it and returns its node.
    ///
    /// It opens on the placement's side, flips when it does not fit there and stays inside the window; built at the root wherever it is
    /// called and painted over the rest.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`).
    /// Panics: when the library refuses the call (outside a frame, an anchor, gap or size that is not finite, a negative margin), or
    /// when `f` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::{ui::Rect, Overlay};
    /// pxui::doc::frames(1, &mut |ui| { ui.anchored("hint", Rect::new(20.0, 20.0, 40.0, 20.0), &Overlay::DEFAULT, |ui| { ui.label("Hint", pxui::Label::BODY); }); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn anchored<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, anchor: Rect, overlay: &Overlay<'_>, mut f: F) -> Handle {
        self.anchored_key(key.into(), anchor, overlay, &mut f)
    }

    /// Declares a box `w` by `h` whose content (`f`'s children, laid out as a column) scrolls on both axes with the theme's thumbs, and
    /// returns its node (unnamed; role ScrollArea).
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`).
    /// Panics: when the library refuses the call (outside a frame, a size that is not finite), or when `f` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::ui::Size;
    /// pxui::doc::frames(1, &mut |ui| { ui.scroll_view("log", Size::Grow, Size::Px(96.0), |ui| { ui.label("line", pxui::Label::BODY); }); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn scroll_view<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, w: Size, h: Size, mut f: F) -> Handle { self.scroll_view_key(key.into(), w, h, &mut f) }

    /// Stacks the toasts `items` in the window's lower end corner (the newest lowest) and returns the index of one clicked away or expired
    /// this frame (the caller drops it), or none.
    ///
    /// Each goes `seconds` after it first showed: the library keeps that time by the toast's id and asks for the frame, so the host needs
    /// no timer.
    /// Allocates: never on a warm frame (up to 32 toasts).
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); each toast is keyed by its `Toast::id`
    /// under it, never by its text.
    /// Panics: when the library refuses the call (outside a frame, `seconds` not finite or negative). A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::ui::Toast;
    /// pxui::doc::frames(1, &mut |ui| { assert_eq!(ui.toasts("toasts", &[Toast::new(1, "Saved")], 3.0), None); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn toasts<K: Into<Key>>(&mut self, key: K, items: &[Toast], seconds: f64) -> Option<usize> { self.toasts_key(key.into(), items, seconds) }

    /// A focused search row over the `(label, detail)` rows `fill(query, rows)` pushes into `rows` for the current query (kept between
    /// frames, their strings reused), windowed to `PickerOpts::limit` around the cursor; reports the edited query and what happened.
    ///
    /// Indices in `Pick` refer to the rows of the returned query. `Pick::Pick` comes on Enter or a row click, `Submit` on Enter with no
    /// rows, `Delete` on a second Delete over the same armed row, `Back` on Backspace or Left with an empty query, `Cancel` on Escape.
    /// `placeholder` shows while the query is empty and names the field. The query is lent until the next call on `ui`. Build it inside
    /// a panel or `modal`.
    /// Allocates: never once the rows have settled (the row strings and their span are kept on the thread); a first fill allocates them.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the placeholder.
    /// Panics: when the library refuses the call (outside a frame), or when `fill` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::ui::Pick;
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let r = ui.picker("find", "Search", "", &pxui::PickerOpts::DEFAULT, &mut |_query, rows| rows.push("Song", "Artist"));
    ///     assert_eq!((r.query, r.pick), ("", Pick::None));
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn picker<K: Into<Key>>(&mut self, key: K, placeholder: &str, query: &str, picker: &PickerOpts<'_>, fill: &mut dyn FnMut(&str, &mut PickRows)) -> PickerResponse<'_> {
        self.picker_key(key.into(), placeholder, query, picker, fill)
    }
}

#[cfg(test)]
mod tests {
    use crate::bodies::{ModalConfig, PanelConfig, PickerFieldConfig, PopupConfig, ScrollViewConfig, ToastsConfig};
    use crate::ui::{Node, Rect, Role, Size};
    use crate::{testing, FrameIn, Overlay, PickerOpts, Ui};
    use std::cell::RefCell;
    use crate::bodies::BodyConfig;

    thread_local! { static SEEN: RefCell<Vec<(&'static str, String)>> = const { RefCell::new(Vec::new()) }; }
    fn seen(kind: &'static str, text: &str) { SEEN.with(|s| s.borrow_mut().push((kind, text.to_owned()))); }
    /// Each registered body builds one marker node of its own, named by its kind, so the custom structure shows in the tree.
    fn marker(ui: &mut Ui, kind: &'static str) { let n = ui.node(kind.into(), Node { w: Size::Px(10.0), h: Size::Px(10.0), role: Role::Image, ..Node::DEFAULT }); ui.a11y_name(&n, kind); }
    fn sheet(ui: &mut Ui, label: &str, content: pxui_types::ffi::PxuiSlot, kind: &'static str) { seen(kind, label); marker(ui, kind); ui.slot(content); }
    fn panel(ui: &mut Ui, c: &PanelConfig) { sheet(ui, c.text_of(&c.label), c.content, "panel") }
    fn popup(ui: &mut Ui, c: &PopupConfig) { sheet(ui, c.text_of(&c.label), c.content, "popup") }
    fn modal(ui: &mut Ui, c: &ModalConfig) { sheet(ui, c.text_of(&c.label), c.content, "modal") }
    fn scroll(ui: &mut Ui, c: &ScrollViewConfig) { seen("scroll_view", ""); marker(ui, "scroll_view"); ui.slot(c.content); }
    fn toast(ui: &mut Ui, c: &ToastsConfig) { seen("toast", c.text_of(&c.text)); marker(ui, "toast"); }
    fn picker(ui: &mut Ui, c: &PickerFieldConfig) { seen("picker_field", c.text_of(&c.placeholder)); marker(ui, "picker_field"); }

    fn build(ui: &mut Ui) {
        ui.panel("settings", &Overlay::DEFAULT.name("Settings"), |ui| { ui.label("in panel", crate::Label::BODY); });
        ui.popup("menu", &Overlay::DEFAULT.at(20.0, 200.0).w(120.0).h(40.0), |ui| { ui.label("in popup", crate::Label::BODY); });
        ui.scroll_view("log", Size::Px(100.0), Size::Px(60.0), |ui| { ui.label("in scroll", crate::Label::BODY); });
        ui.toasts("toasts", &[crate::ui::Toast::new(1, "Saved")], 3.0);
        ui.picker("find", "Search", "", &PickerOpts::DEFAULT, &mut |_, _| {});
        ui.modal("confirm", &Overlay::DEFAULT.name("Delete?"), |ui| { ui.label("in modal", crate::Label::BODY); });
    }

    /// Every container of the overlay family is built through its body: with a registered body for each kind, the custom structure is in
    /// the tree, the caller's content runs through the body's slot, and a sheet's body is given its name (never its key).
    #[test]
    fn each_overlay_is_built_through_its_registered_body() {
        let mut ui = Ui::new().expect("library");
        ui.set_a11y(true);
        ui.register_panel_body("c", panel).unwrap();
        ui.register_popup_body("c", popup).unwrap();
        ui.register_modal_body("c", modal).unwrap();
        ui.register_scroll_view_body("c", scroll).unwrap();
        ui.register_toasts_body("c", toast).unwrap();
        ui.register_picker_field_body("c", picker).unwrap();
        let text = "panel.body = c\npopup.body = c\nmodal.body = c\nscroll_view.body = c\ntoasts.body = c\npicker_field.body = c";
        ui.set_theme(&pxui_types::ffi::ThemeSource { size: core::mem::size_of::<pxui_types::ffi::ThemeSource>() as u32, flags: 0, source: pxui_types::Str { ptr: text.as_ptr(), len: text.len() } }).unwrap();
        SEEN.with(|s| s.borrow_mut().clear());
        ui.frame(&FrameIn { width: 400.0, height: 400.0, ..FrameIn::default() }, &mut build).unwrap();
        let got = SEEN.with(|s| s.borrow().clone());
        let want: Vec<(&str, String)> = [("panel", "Settings"), ("popup", ""), ("scroll_view", ""), ("toast", "Saved"), ("picker_field", "Search"), ("modal", "Delete?")]
            .iter().map(|(k, t)| (*k, t.to_string())).collect();
        assert_eq!(got, want);
        for kind in ["panel", "popup", "modal", "scroll_view", "toast", "picker_field"] {
            assert!(testing::find(&ui, Role::Image, kind).is_some(), "the {kind} body's structure is used");
        }
        for content in ["in panel", "in popup", "in scroll", "in modal"] {
            assert!(testing::find(&ui, Role::Label, content).is_some(), "{content}: the content ran through the slot");
        }
    }

    /// The key is identity and the name what is read: a panel and a modal are named by `Overlay::name` only, an anchored sheet sits by
    /// its anchor at the overlay's placement.
    #[test]
    fn overlays_are_named_by_their_name_and_keyed_by_their_key() {
        let at = std::cell::Cell::new(Rect::default());
        let mut build = |ui: &mut Ui| {
            ui.panel("settings", &Overlay::DEFAULT.name("Settings"), |_| {});
            ui.panel("plain", &Overlay::DEFAULT.at(300.0, 12.0), |_| {});
            let b = ui.anchored("hint", Rect::new(40.0, 40.0, 20.0, 10.0), &Overlay::DEFAULT.w(50.0).h(20.0), |_| {});
            at.set(ui.rect(&b));
        };
        let mut ui = crate::doc::frames(2, &mut build);
        assert!(testing::find(&ui, Role::Panel, "Settings").is_some());
        assert!(testing::find(&ui, Role::Panel, "plain").is_none(), "the key is never the name");
        testing::frame(&mut ui, &[], &mut build);
        assert_eq!(at.get(), Rect::new(40.0, 50.0, 50.0, 20.0), "below its anchor, start-aligned, the overlay's size");
    }
}
