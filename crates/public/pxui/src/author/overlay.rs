//! The overlay family: `ui.panel`, `ui.popup`, `ui.modal` and `ui.anchored` take a key and one `Overlay` style; `ui.scroll_view`,
//! `ui.toasts` and `ui.picker` take a key. Each is built through its body (panel, window, popup, modal, scroll_view, toast,
//! picker_field), so a registered body replaces its structure.
use crate::Handle;
use super::Key;
use crate::{Chosen, OverlayResponse, Picker, Ui};
use pxui_types::{geom::Rect, ui::{Overlay, PickRows, PickerResponse, Toast}, Node};

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
    /// const SETTINGS: Overlay = Overlay::DEFAULT.w_px(296.0).name("Settings");
    /// pxui::doc::frames(1, &mut |ui| { assert!(ui.panel("settings", &SETTINGS, |ui| { ui.label("Display", pxui::Label::DEFAULT); }).open); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn panel<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, mut f: F) -> OverlayResponse { self.panel_key(key.into(), overlay, &mut f) }

    /// Declares a popup at `Overlay::at`, `Overlay::w` wide (at least 180), builds `f` in it and reports its root and open state.
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
    /// pxui::doc::frames(1, &mut |ui| { ran = ui.popup("song-menu", &Overlay::DEFAULT.at(pxui::Point::new(10.0, 10.0)).w_px(200.0).h_px(60.0), |_| {}).open; });
    /// assert!(ran);
    /// ```
    #[inline]
    #[track_caller]
    pub fn popup<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, mut f: F) -> OverlayResponse { self.popup_key(key.into(), overlay, &mut f) }

    /// Declares a modal sheet centred in the content area, `Overlay::w` wide (320 by default), builds `f` in it and reports its root and open state.
    ///
    /// Escape, window focus loss or a press outside it dismisses it: `f` is skipped and `false` returned. Build it before the body it
    /// shields, at the root level; it paints on top. Named by `Overlay::name` (none without).
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the name.
    /// Panics: when the library refuses the overlay (outside a frame, a width that is not finite or is negative), or when `f` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::Overlay;
    /// const CONFIRM: Overlay = Overlay::DEFAULT.w_px(260.0).name("Delete song?");
    /// pxui::doc::frames(1, &mut |ui| { ui.modal("confirm-delete", &CONFIRM, |ui| { ui.label("Delete?", pxui::Label::BODY); }); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn modal<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, mut f: F) -> OverlayResponse { self.modal_key(key.into(), overlay, &mut f) }

    /// Declares a sheet placed by `anchor` (window points, e.g. `ui.rect(&field)`) at `Overlay::place`, `Overlay::w` by `Overlay::h`
    /// (each its content's when unset), builds `f` in it and reports its root and open state.
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
    pub fn anchored<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, anchor: Rect, overlay: &Overlay<'_>, mut f: F) -> OverlayResponse {
        self.anchored_key(key.into(), anchor, overlay, &mut f)
    }

    /// Builds a scroll area laid out by `node` and returns its handle. The content (`f`'s children) flows on `node`'s axis, through
    /// the registered scroll view body, and scrolls on both axes with the theme's thumbs. The scroll view adds its scroll flags, look,
    /// thumbs, row scroll step and role (ScrollArea, unnamed), and keeps a one-point frame inset inside `node`'s padding.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`).
    /// Panics: when the library refuses the call (outside a frame, a size that is not finite, a grid node or an invalid transition), or
    /// when `f` panics. A key over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// use pxui::Node;
    /// pxui::doc::frames(1, &mut |ui| { ui.scroll_view("log", Node::column().h_px(96.0), |ui| { ui.label("line", pxui::Label::BODY); }); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn scroll_view<K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, mut f: F) -> Handle { self.scroll_view_node(key.into(), node, &mut f) }

    /// Stacks the toasts `items` in the window's lower end corner (the newest lowest) and reports one clicked away or expired
    /// this frame. The caller drops the item indicated by chosen or chosen_id. The stack itself remains open.
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
    /// pxui::doc::frames(1, &mut |ui| { assert_eq!(ui.toasts("toasts", &[Toast::new(1, "Saved")], 3.0).chosen, None); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn toasts<K: Into<Key>>(&mut self, key: K, items: &[Toast<'_>], seconds: f64) -> Chosen { self.toasts_key(key.into(), items, seconds) }

    /// A focused search row over the `(label, detail)` rows `fill(query, rows)` pushes into `rows` for the current query (kept between
    /// frames, their strings reused), windowed to `Picker::limit` around the cursor; reports the edited query and what happened.
    ///
    /// Indices in `Pick` refer to the rows of the returned query. `Pick::Pick` comes on Enter or a row click, `Submit` on Enter with no
    /// rows, `Delete` on a second Delete over the same armed row, `Back` on Backspace or Left with an empty query, `Cancel` on Escape.
    /// `placeholder` shows while the query is empty and names the field; focused, the field asks for the search keyboard
    /// (`TextTraits::SEARCH`). The query is lent until the next call on `ui`. Build it inside a panel or `modal`.
    ///
    /// Fill contract: `fill` runs every frame the picker is built and copies rows the app already has; it does no search, sort,
    /// formatting or I/O. An app whose matches come from a worker sends the query as an intent when `query` changes, keeps the worker's
    /// last result (labels, details, art ids, headings as owned strings prepared on the worker) and pushes those borrowed strings,
    /// in order, every frame; until the result for the new query arrives it pushes the previous one. The built-in row highlights the
    /// query's letters in place (ASCII case folded, byte by byte), allocating nothing.
    /// Allocates: never once the rows have settled (the row strings and their span are kept on the thread, reused while no row is
    /// longer and there are no more rows than before); a first fill, or one with more or longer rows, allocates them.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the placeholder.
    /// Panics: when the library refuses the call (outside a frame), or when `fill` panics. A key over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::ui::Pick;
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let r = ui.picker("find", "Search", "", &pxui::Picker::DEFAULT, &mut |_query, rows| rows.push(pxui::PickRow::new("Song").detail("Artist")));
    ///     assert_eq!((r.query, r.pick), ("", Pick::None));
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn picker<K: Into<Key>>(&mut self, key: K, placeholder: &str, query: &str, picker: &Picker<'_>, fill: &mut dyn FnMut(&str, &mut PickRows)) -> PickerResponse<'_> {
        self.picker_key(key.into(), placeholder, query, picker, fill)
    }
}

#[cfg(test)]
mod tests {
    use crate::bodies::{ModalConfig, PanelConfig, PickerFieldConfig, PopupConfig, ScrollViewConfig, ToastsConfig};
    use crate::ui::{Node, Rect, Role, Size};
    use crate::{testing, FrameIn, Overlay, Picker, Ui};
    use std::cell::RefCell;
    use crate::bodies::BodyConfig;

    thread_local! { static SEEN: RefCell<Vec<(&'static str, String)>> = const { RefCell::new(Vec::new()) }; }
    fn seen(kind: &'static str, text: &str) { SEEN.with(|s| s.borrow_mut().push((kind, text.to_owned()))); }
    /// Each registered body builds one marker node of its own, named by its kind, so the custom structure shows in the tree.
    fn marker(ui: &mut Ui, kind: &'static str) { let n = ui.node(kind, Node { w: Size::Px(10.0), h: Size::Px(10.0), role: Role::Image, ..Node::DEFAULT }); ui.a11y_name(&n, kind); }
    fn sheet(ui: &mut Ui, label: &str, content: pxui_types::ffi::PxuiSlot, kind: &'static str) { seen(kind, label); marker(ui, kind); ui.slot(content); }
    fn panel(ui: &mut Ui, c: &PanelConfig) { sheet(ui, c.text_of(&c.label), c.content, "panel") }
    fn popup(ui: &mut Ui, c: &PopupConfig) { sheet(ui, c.text_of(&c.label), c.content, "popup") }
    fn modal(ui: &mut Ui, c: &ModalConfig) { sheet(ui, c.text_of(&c.label), c.content, "modal") }
    fn scroll(ui: &mut Ui, c: &ScrollViewConfig) { seen("scroll_view", ""); marker(ui, "scroll_view"); ui.slot(c.content); }
    fn toast(ui: &mut Ui, c: &ToastsConfig) { seen("toast", c.text_of(&c.text)); marker(ui, "toast"); }
    fn picker(ui: &mut Ui, c: &PickerFieldConfig) { seen("picker_field", c.text_of(&c.placeholder)); marker(ui, "picker_field"); }

    fn build(ui: &mut Ui) {
        ui.panel("settings", &Overlay::DEFAULT.name("Settings"), |ui| { ui.label("in panel", crate::Label::BODY); });
        ui.popup("menu", &Overlay::DEFAULT.at(crate::Point::new(20.0, 200.0)).w_px(120.0).h_px(40.0), |ui| { ui.label("in popup", crate::Label::BODY); });
        ui.scroll_view("log", Node::DEFAULT.w_px(100.0).h_px(60.0), |ui| { ui.label("in scroll", crate::Label::BODY); });
        ui.toasts("toasts", &[crate::ui::Toast::new(1, "Saved")], 3.0);
        ui.picker("find", "Search", "", &Picker::DEFAULT, &mut |_, _| {});
        ui.modal("confirm", &Overlay::DEFAULT.name("Delete?"), |ui| { ui.label("in modal", crate::Label::BODY); });
    }

    /// Every container of the overlay family is built through its body: with a registered body for each kind, the custom structure is in
    /// the tree, the caller's content runs through the body's slot, and a sheet's body is given its name (never its key).
    #[test]
    fn each_overlay_is_built_through_its_registered_body() {
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.set_a11y(true);
        ui.register_panel_body("c", panel).unwrap();
        ui.register_popup_body("c", popup).unwrap();
        ui.register_modal_body("c", modal).unwrap();
        ui.register_scroll_view_body("c", scroll).unwrap();
        ui.register_toasts_body("c", toast).unwrap();
        ui.register_picker_field_body("c", picker).unwrap();
        let text = "panel.body = c\npopup.body = c\nmodal.body = c\nscroll_view.body = c\ntoasts.body = c\npicker_field.body = c";
        ui.set_theme(&crate::ThemeHandle::parse(&text).unwrap()).unwrap();
        // the picker's results measure their rows (U2b.4.15): the first frame builds again once their heights are known
        ui.frame(&FrameIn::new(400.0, 400.0), &mut build).unwrap();
        SEEN.with(|s| s.borrow_mut().clear());
        ui.frame(&FrameIn::new(400.0, 400.0), &mut build).unwrap();
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
            ui.panel("plain", &Overlay::DEFAULT.at(crate::Point::new(300.0, 12.0)), |_| {});
            let b = ui.anchored("hint", Rect::new(40.0, 40.0, 20.0, 10.0), &Overlay::DEFAULT.w_px(50.0).h_px(20.0), |_| {});
            at.set(ui.rect(&b.handle));
        };
        let mut ui = crate::doc::frames(2, &mut build);
        assert!(testing::find(&ui, Role::Panel, "Settings").is_some());
        assert!(testing::find(&ui, Role::Panel, "plain").is_none(), "the key is never the name");
        testing::frame(&mut ui, &[], &mut build);
        assert_eq!(at.get(), Rect::new(40.0, 50.0, 50.0, 20.0), "below its anchor, start-aligned, the overlay's size");
    }

    const ROWS: [crate::PickRow; 4] = [
        crate::PickRow::new("Song").detail("Artist").art(crate::ui::PickArt::Glyph('\u{266b}')).heading("Songs"),
        crate::PickRow::new("Other"),
        crate::PickRow::new("Album").art(crate::ui::PickArt::Image(7)).heading("Albums"),
        crate::PickRow::new("Folder").art(crate::ui::PickArt::Draw(5)),
    ];
    fn fill(_: &str, rows: &mut crate::PickRows) { for row in ROWS { rows.push(row); } }

    thread_local! { static ARTS: RefCell<Vec<Option<crate::ui::PickArt>>> = const { RefCell::new(Vec::new()) }; }
    fn picker_row(ui: &mut Ui, c: &crate::bodies::PickerConfig) {
        ARTS.with(|a| a.borrow_mut().push(c.art()));
        marker(ui, "picker row");
    }

    /// A row's heading is a Heading caption above the row that starts its group (not a row: rows stay the picks), the built-in row draws
    /// a glyph in the art slot before its label, and a registered row body is given each row's art: a glyph, an image id or the app's
    /// icon word (U2b.4.19, `PickArt::Draw`). Both link modes.
    #[test]
    fn picker_rows_draw_headings_and_give_their_art_to_the_body() {
        let mut build = |ui: &mut Ui| { ui.picker("find", "Search", "", &Picker::DEFAULT, &mut fill); };
        let ui = crate::doc::frames(2, &mut build);
        for heading in ["Songs", "Albums"] { assert!(testing::find(&ui, Role::Heading, heading).is_some(), "{heading}"); }
        let rows = ui.a11y_nodes().iter().filter(|n| n.is_role(Role::ListItem)).count();
        assert_eq!(rows, 4, "headings are not rows");
        let (songs, song) = (testing::find(&ui, Role::Heading, "Songs").unwrap(), testing::find(&ui, Role::ListItem, "Song").unwrap());
        assert!(songs.y + songs.h <= song.y, "the heading sits above its row");
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.register_picker_body("c", picker_row).unwrap();
        ui.set_theme(&crate::ThemeHandle::parse("picker.body = c").unwrap()).unwrap();
        // the results measure their rows (U2b.4.15): the first frame builds them again once their heights are known
        ui.frame(&FrameIn::new(400.0, 400.0), &mut build).unwrap();
        ARTS.with(|a| a.borrow_mut().clear());
        ui.frame(&FrameIn::new(400.0, 400.0), &mut build).unwrap();
        assert_eq!(ARTS.with(|a| a.borrow().clone()), [ROWS[0].art, None, ROWS[2].art, ROWS[3].art]);
    }

    /// The picker's query is a search: focused, it asks for the search keyboard (no autocorrect or capitalisation, the Search key), as
    /// `search_field` does by default. Both link modes.
    #[test]
    fn a_focused_picker_asks_for_the_search_keyboard() {
        let mut build = |ui: &mut Ui| { ui.picker("find", "Search", "", &Picker::DEFAULT, &mut fill); };
        let ui = crate::doc::frames(2, &mut build);
        assert!(ui.text_input_focused());
        assert_eq!(ui.text_traits(), crate::TextTraits::SEARCH);
    }

    /// Moving the picker's keyboard row reads the frame's keys in place: warm arrow-key frames allocate nothing (a scrolling,
    /// wrapping cursor). Both link modes.
    #[test]
    fn warm_picker_arrow_keys_allocate_nothing() {
        mem_sys::set_peer(crate::library_counter);
        const NAMES: [&str; 24] = ["P00", "P01", "P02", "P03", "P04", "P05", "P06", "P07", "P08", "P09", "P10", "P11", "P12", "P13", "P14",
            "P15", "P16", "P17", "P18", "P19", "P20", "P21", "P22", "P23"];
        let mut build = |ui: &mut Ui| { ui.picker("find", "Search", "", &Picker::DEFAULT.limit(5), &mut |_, rows| for n in NAMES { rows.push(crate::PickRow::new(n)) }); };
        let mut ui = crate::doc::frames(2, &mut build);
        for _ in 0..30 { testing::key(&mut ui, crate::KeyCode::ArrowDown, &mut build); }
        let scope = mem_sys::warm_scope();
        for round in 0..30 {
            mem_sys::local_reset();
            testing::key(&mut ui, crate::KeyCode::ArrowDown, &mut build);
            assert_eq!(mem_sys::local_alloc_count(), 0, "a warm picker arrow-key frame allocated (round {round})");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    /// U2b.6 fill contract (#82): a picker with a typed query, filled every frame from a worker's last result (owned strings the app
    /// keeps, pushed borrowed), allocates nothing once warm; the built-in row highlights the query case folded byte by byte (it used to
    /// lower the query into a new string on every row of every frame). Both link modes.
    #[test]
    fn a_picker_filled_from_a_kept_result_with_a_query_allocates_nothing() {
        mem_sys::set_peer(crate::library_counter);
        // what the worker sent for the query: prepared off the frame, kept until the next result
        let result: Vec<(String, String)> = (0..40).map(|i| (format!("Song {i}"), format!("Artist {i}"))).collect();
        let mut build = |ui: &mut Ui| {
            ui.picker("find", "Search", "SO", &Picker::DEFAULT, &mut |_, rows| for (label, detail) in &result { rows.push(crate::PickRow::new(label).detail(detail)); });
        };
        let mut ui = crate::doc::frames(3, &mut build);
        let scope = mem_sys::warm_scope();
        for round in 0..4 {
            mem_sys::local_reset();
            testing::frame(&mut ui, &[], &mut build);
            assert_eq!(mem_sys::local_alloc_count(), 0, "a warm picker frame with a query allocated (round {round})");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    /// Warm picker frames with headings and art allocate nothing.
    #[test]
    fn warm_picker_rows_with_headings_and_art_allocate_nothing() {
        let mut build = |ui: &mut Ui| { ui.picker("find", "Search", "", &Picker::DEFAULT, &mut fill); };
        let mut ui = crate::doc::frames(3, &mut build);
        for round in 0..6 {
            mem_sys::local_reset();
            testing::frame(&mut ui, &[], &mut build);
            if round > 1 { assert_eq!(mem_sys::local_alloc_count(), 0, "round {round}"); }
        }
    }

    /// A bottom sheet sits on the content area's bottom edge at the overlay's width, a full sheet fills the content area, and a centred
    /// modal stays centred; each is a modal dialog to a screen reader.
    #[test]
    fn sheets_are_placed_by_their_presentation() {
        use crate::Presentation;
        let rects = RefCell::new([Rect::default(); 3]);
        let present = std::cell::Cell::new(Presentation::Centered);
        let mut build = |ui: &mut Ui| {
            let r = ui.modal("sheet", &Overlay::DEFAULT.name("Sheet").w_px(200.0).h_px(100.0).present(present.get()), |_| {});
            rects.borrow_mut()[present.get() as usize] = ui.rect(&r.handle);
        };
        let mut ui = crate::doc::frames(2, &mut build);
        for p in [Presentation::Bottom, Presentation::Full] { present.set(p); for _ in 0..2 { testing::frame(&mut ui, &[], &mut build); } }
        let view = ui.content_rect();
        let [centred, bottom, full] = *rects.borrow();
        assert_eq!((centred.x, centred.w), (view.x + (view.w - 200.0) / 2.0, 200.0), "centred");
        assert!(centred.y > view.y && centred.y + centred.h < view.y + view.h, "centred vertically");
        assert_eq!((bottom.y + bottom.h, bottom.w), (view.y + view.h, 200.0), "on the bottom edge");
        assert_eq!((full.y, full.h), (view.y, view.h), "the full height");
        let dialog = ui.a11y_nodes().iter().find(|n| n.is_role(Role::Dialog) && ui.a11y_name_of(n) == "Sheet").expect("a dialog");
        assert!(dialog.states & pxui_types::a11y::state::MODAL != 0, "modal");
    }

    /// U2b.5: a top-anchored modal is centred across the content area with its top edge at the overlay's y and its height capped by
    /// `max_h` (through the sheet record in the dynamic link); a registered modal body is told `present` 3.
    #[test]
    fn a_top_modal_sits_at_its_y() {
        use crate::Presentation;
        thread_local! { static PRESENT: std::cell::Cell<Option<Presentation>> = const { std::cell::Cell::new(None) }; }
        fn modal(ui: &mut Ui, c: &ModalConfig) { PRESENT.with(|p| p.set(Some(c.presentation()))); ui.slot(c.content); }
        let rect = std::cell::Cell::new(Rect::default());
        let mut build = |ui: &mut Ui| {
            let r = ui.modal("palette", &Overlay::DEFAULT.name("Palette").w_px(200.0).at(crate::geom::Point::new(0.0, 30.0)).max_h_px(120.0).present(Presentation::Top), |ui| { ui.node("results", Node::DEFAULT.h_px(300.0)); });
            rect.set(ui.rect(&r.handle));
        };
        let mut ui = crate::doc::frames(3, &mut build);
        let view = ui.content_rect();
        let r = rect.get();
        assert_eq!((r.x, r.y, r.w, r.h), (view.x + ((view.w - 200.0) / 2.0).round(), view.y + 30.0, 200.0, 120.0), "top-anchored, centred across, capped");
        ui.register_modal_body("dressed", modal).unwrap();
        ui.set_theme(&crate::ThemeHandle::parse("modal.body = dressed").unwrap()).unwrap();
        for _ in 0..2 { testing::frame(&mut ui, &[], &mut build); }
        assert_eq!(PRESENT.with(|p| p.get()), Some(Presentation::Top), "the body is told it is top-anchored (`presentation()`)");
        mem_sys::set_peer(crate::library_counter);
        let scope = mem_sys::warm_scope();
        for round in 0..4 {
            mem_sys::local_reset();
            testing::frame(&mut ui, &[], &mut build);
            assert_eq!(mem_sys::local_alloc_count(), 0, "a warm top modal frame allocated (round {round})");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty());
    }

    /// A downward drag on a bottom sheet past its fraction dismisses it (the response closes) and a short one springs back; warm bottom
    /// and full sheet frames, dragged or at rest, allocate nothing.
    #[test]
    fn a_bottom_sheet_drags_to_dismiss_and_warm_sheets_allocate_nothing() {
        use crate::{DismissOpts, Event, Presentation};
        let (open, present) = (std::cell::Cell::new(true), std::cell::Cell::new(Presentation::Bottom));
        let sheet = std::cell::Cell::new(Rect::default());
        let mut build = |ui: &mut Ui| {
            if !open.get() { return; }
            // a sheet of 160 points of content: a quarter of its height is 40 points of drag
            let r = ui.modal("sheet", &Overlay::DEFAULT.w_px(200.0).present(present.get()).dismiss(DismissOpts::DEFAULT.fraction(0.25)), |ui| { ui.node("queue", Node::DEFAULT.h_px(160.0)); });
            if !r.open { open.set(false); }
            sheet.set(ui.rect(&r.handle));
        };
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        for n in 0..3 { ui.frame(&FrameIn::new(400.0, 400.0).time(f64::from(n)), &mut build).unwrap(); }
        // a finger: began, moved in four steps, lifted half a second later (slowly: the distance decides), then the sheet settles
        let time = std::cell::Cell::new(10.0);
        let touch = |ui: &mut Ui, build: &mut dyn FnMut(&mut Ui), phase: u32, x: f64, y: f64| {
            time.set(time.get() + 0.016 + if phase == 2 { 0.5 } else { 0.0 });
            let packet = crate::Touch { device: 1, finger: 7, x, y, phase, time: time.get(), pressure: 1.0 };
            let event = Event { kind: Event::TOUCH, text: crate::Str { ptr: (&packet as *const crate::Touch).cast(), len: core::mem::size_of::<crate::Touch>() }, ..Default::default() };
            let mut frame = FrameIn::new(400.0, 400.0).time(time.get());
            frame.wire.events = &event;
            frame.wire.event_count = 1;
            ui.frame(&frame, build).unwrap();
        };
        let drag = |ui: &mut Ui, build: &mut dyn FnMut(&mut Ui), to: f32| {
            let (x, y) = (f64::from(sheet.get().x + 20.0), f64::from(sheet.get().y + 10.0));
            touch(ui, build, 0, x, y);
            for step in 1..=4 { touch(ui, build, 1, x, y + f64::from(to) * f64::from(step) / 4.0); }
            touch(ui, build, 2, x, y + f64::from(to));
            for _ in 0..30 { time.set(time.get() + 0.1); ui.frame(&FrameIn::new(400.0, 400.0).time(time.get()), build).unwrap(); }
        };
        drag(&mut ui, &mut build, 10.0);
        assert!(open.get(), "a short drag springs back");
        for p in [Presentation::Bottom, Presentation::Full] {
            present.set(p);
            for round in 0..5 {
                mem_sys::local_reset();
                time.set(time.get() + 0.1);
                ui.frame(&FrameIn::new(400.0, 400.0).time(time.get()), &mut build).unwrap();
                if round > 2 { assert_eq!(mem_sys::local_alloc_count(), 0, "{p:?} round {round}"); }
            }
        }
        present.set(Presentation::Bottom);
        for _ in 0..3 { time.set(time.get() + 0.1); ui.frame(&FrameIn::new(400.0, 400.0).time(time.get()), &mut build).unwrap(); }
        drag(&mut ui, &mut build, 80.0);
        assert!(!open.get(), "a drag past a quarter of its height dismisses it");
    }

    /// A panel's look in place of the theme's reaches the drawing in either link mode (the keyed record carries it by pointer): the panel
    /// fills in the look's colour, and the panel without one does not.
    #[test]
    fn a_panel_look_is_drawn_and_a_plain_panel_keeps_the_theme_look() {
        use pxui_types::{geom::Rgba, theme::Look};
        static LOOK: Look = Look::NONE.fill(Rgba { r: 201, g: 13, b: 77, a: 255 });
        let painted = |overlay: &Overlay| {
            let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
            let mut build = |ui: &mut Ui| { ui.panel("p", overlay, |ui| { ui.label("x", crate::Label::BODY); }); };
            for _ in 0..2 { ui.frame(&FrameIn::new(400.0, 400.0), &mut build).unwrap(); }
            (0..ui.layer_count()).any(|l| ui.layer(l).unwrap().instances.iter().any(|i| i.color == [201, 13, 77, 255]))
        };
        assert!(painted(&Overlay::DEFAULT.look(&LOOK)), "the panel's own look is drawn");
        assert!(!painted(&Overlay::DEFAULT), "a panel without one is in the theme's");
    }
}
