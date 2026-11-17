//! The kit menu: `ui.menu(key, at, &ITEMS, &Menu::DEFAULT)` reports a `MenuResponse`.
use super::Key;
use crate::{ui::{Menu, MenuItem, MenuResponse}, Ui};

impl Ui {
    /// Shows a floating menu of `items` at `at` (window points, moved inside the window) and reports what happened to it this frame.
    ///
    /// Call it every frame the menu is open and stop when `dismissed` or `chosen` comes back: a `chosen` index is the row clicked (or
    /// Enter on the focused row) and the menu has closed itself, `chosen_id` that row's `MenuItem::id`; Escape, focus loss or a press
    /// outside set `dismissed`. A disabled row is never chosen and empty text is a separator. Build it before the content it shields; it
    /// floats at the root, moved inside the view (`at` needs no clamping). The menu is as wide as its longest row (`Menu::width` is the
    /// least). Rows with a submenu open it beside the menu; its rows are numbered after the menu's own. Rows are drawn by the registered
    /// `menu_row` body; a `MenuItem::selected` row reads as the current choice. The sheet is built by the registered `popup` body, which
    /// may give it a role of its own (else it is a Menu); it is named `Menu::name` (none without), has `Menu::padding` above its first row
    /// and below its last, and past `Menu::max_rows` or `Menu::max_h` (the smaller) it scrolls itself.
    /// Allocates: never on a warm frame, except that rows with a submenu allocate per call.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); it is never the accessible name (`Menu::name`).
    /// A row is keyed under the menu by its `MenuItem::id`, else by its position, never by its text: rows of one text are distinct.
    /// Panics: when the library refuses the arguments (outside a frame, a non-finite position or width, a negative or non-finite padding or
    /// `max_h`). A key or identity label over 256 bytes panics (a programmer error). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// use pxui::ui::{Menu, MenuItem};
    /// const ITEMS: [MenuItem; 3] = [MenuItem::new("Play next"), MenuItem::new("Add to playlist"), MenuItem::new("Delete").danger()];
    /// let mut chosen = None;
    /// let mut build = |ui: &mut pxui::Ui| { chosen = ui.menu("song-menu", (10.0, 10.0), &ITEMS, &Menu::DEFAULT.row_h(36.0)).chosen; };
    /// let mut ui = pxui::doc::frames(2, &mut build);
    /// pxui::testing::click(&mut ui, 40.0, 10.0 + 7.0 + 18.0, &mut build);
    /// assert_eq!(chosen, Some(0));
    /// ```
    #[inline]
    #[track_caller]
    pub fn menu<K: Into<Key>>(&mut self, key: K, at: (f32, f32), items: &[MenuItem], menu: &Menu) -> MenuResponse {
        self.menu_call(key.into(), at, items, menu)
    }

    /// The one body of `menu`: the engine call, with the menu's variant set around it, then the chosen row's id.
    #[inline(always)]
    #[track_caller]
    fn menu_call(&mut self, key: Key, at: (f32, f32), items: &[MenuItem], menu: &Menu) -> MenuResponse {
        if menu.variant != 0 { self.set_variant(pxui_types::theme::BodyKind::menu_row, menu.variant); }
        let mut response = self.menu_keyed(key, at, items, menu);
        if menu.variant != 0 { self.set_variant(pxui_types::theme::BodyKind::menu_row, 0); }
        // a chosen index past the menu's rows is a submenu's row, numbered in the order of the rows that open them
        response.chosen_id = response.chosen.and_then(|index| match items.get(index) {
            Some(item) => item.id,
            None => items.iter().flat_map(|item| item.submenu).nth(index - items.len()).and_then(|item| item.id),
        });
        response
    }
}

#[cfg(test)]
mod tests {
    use crate::bodies::MenuRowConfig;
    use crate::ui::{Menu, MenuItem, MenuResponse, Role};
    use crate::{testing, FrameIn, Ui};
    use pxui_types::{Event, KEY_SPECIAL};
    use std::cell::{Cell, RefCell};
    use crate::bodies::BodyConfig;

    const ITEMS: [MenuItem; 4] = [MenuItem::new("Play next"), MenuItem::new("Add to playlist").enabled(false), MenuItem::new("Delete").danger(), MenuItem::new("Size").checked(true).key("Ctrl S")];
    const TAB: u32 = KEY_SPECIAL + 8;
    const ENTER: u32 = KEY_SPECIAL + 5;
    const ESCAPE: u32 = KEY_SPECIAL + 6;

    fn centre(ui: &Ui, name: &str) -> (f64, f64) {
        let r = testing::find(ui, Role::MenuItem, name).unwrap_or_else(|| panic!("no row {name}"));
        (f64::from(r.x + r.w / 2.0), f64::from(r.y + r.h / 2.0))
    }

    /// A click on a row chooses it and the menu closes itself; a disabled row takes no click; the separator is not a row.
    #[test]
    fn a_click_chooses_a_row_and_a_disabled_row_is_not_choosable() {
        let (last, menu) = (Cell::new(MenuResponse::default()), Menu::DEFAULT.row_h(36.0));
        let mut build = |ui: &mut Ui| { let got = ui.menu("song-menu", (10.0, 10.0), &ITEMS, &menu); if got != MenuResponse::default() { last.set(got); } };
        let mut ui = crate::doc::frames(3, &mut build);
        let r = testing::find(&ui, Role::MenuItem, "Play next").unwrap();
        assert_eq!(r.h, 36.0, "Menu::row_h is the row's height");
        let (x, y) = centre(&ui, "Add to playlist");
        testing::click(&mut ui, x, y, &mut build);
        assert_eq!(last.get(), MenuResponse::default(), "a disabled row is inert");
        let (x, y) = centre(&ui, "Delete");
        testing::click(&mut ui, x, y, &mut build);
        assert_eq!(last.get(), MenuResponse { chosen: Some(2), ..MenuResponse::default() });
    }

    /// Tab then Enter chooses the first enabled row (never the disabled one before it); Escape and a press outside dismiss.
    #[test]
    fn enter_chooses_the_focused_row_and_escape_or_an_outside_press_dismisses() {
        let (last, menu) = (Cell::new(MenuResponse::default()), Menu::DEFAULT);
        let mut build = |ui: &mut Ui| { let got = ui.menu("song-menu", (10.0, 10.0), &ITEMS, &menu); if got != MenuResponse::default() { last.set(got); } };
        let mut ui = crate::doc::frames(3, &mut build);
        testing::key(&mut ui, TAB, &mut build);
        testing::key(&mut ui, ENTER, &mut build);
        assert_eq!(last.get().chosen, Some(0));
        last.set(MenuResponse::default());
        let mut ui = crate::doc::frames(3, &mut build);
        testing::key(&mut ui, ESCAPE, &mut build);
        assert_eq!(last.get(), MenuResponse { dismissed: true, ..MenuResponse::default() });
        last.set(MenuResponse::default());
        let mut ui = crate::doc::frames(3, &mut build);
        testing::click(&mut ui, 300.0, 230.0, &mut build);
        assert!(last.get().dismissed && last.get().chosen.is_none());
    }

    /// The danger row is inked in the theme's menu_danger text, the others in the menu's; a checked row keeps its slot and shows its shortcut.
    #[test]
    fn the_danger_row_reads_in_the_error_ink() {
        let (last, menu) = (Cell::new(MenuResponse::default()), Menu::DEFAULT);
        let mut danger_ink = None;
        let mut build = |ui: &mut Ui| { danger_ink = Some(ui.theme().menu_danger.idle.text); last.set(ui.menu("song-menu", (10.0, 10.0), &ITEMS, &menu)); };
        let ui = crate::doc::frames(3, &mut build);
        let (plain, danger) = (ui.theme().menu.idle.text, danger_ink.unwrap());
        assert_ne!(plain, danger);
        let ink = |ui: &Ui, rect: crate::geom::Rect, color: crate::geom::Rgba| ui.layers().flat_map(|l| l.instances.iter())
            .filter(|i| i.color == [color.r, color.g, color.b, color.a] && i.rect[0] >= rect.x && i.rect[2] <= rect.x + rect.w && i.rect[1] >= rect.y && i.rect[3] <= rect.y + rect.h).count();
        let (row, other) = (testing::find(&ui, Role::MenuItem, "Delete").unwrap(), testing::find(&ui, Role::MenuItem, "Play next").unwrap());
        assert!(ink(&ui, row, danger) > 0 && ink(&ui, other, danger) == 0, "the error ink is the danger row's alone");
    }

    thread_local! { static SEEN: RefCell<Vec<(String, bool, bool, bool, bool, f32, f32)>> = const { RefCell::new(Vec::new()) }; }

    /// A context-menu body that records its config and builds one node of its own.
    fn marker(ui: &mut Ui, c: &MenuRowConfig) {
        assert!(c.size as usize >= core::mem::size_of::<MenuRowConfig>(), "this library carries the row height");
        SEEN.with(|s| s.borrow_mut().push((c.text_of(&c.text).to_owned(), c.state.disabled != 0, c.danger != 0, c.selected != 0, c.has_shortcut != 0, c.lead, c.row_h)));
        ui.node("custom".into(), pxui_types::Node { w: pxui_types::Size::Px(7.0), h: pxui_types::Size::Px(9.0), ..pxui_types::Node::DEFAULT });
    }

    /// With a registered `menu_row` body every row goes through it, with the item's flags and the menu's row height; its
    /// structure is what shows.
    #[test]
    fn a_registered_row_body_builds_the_rows() {
        let mut ui = Ui::new().unwrap();
        ui.set_a11y(true);
        ui.register_menu_row_body("marker", marker).expect("registered");
        let text = "menu_row.body = marker\n";
        ui.set_theme(&pxui_types::ffi::ThemeSource { size: core::mem::size_of::<pxui_types::ffi::ThemeSource>() as u32, flags: 0, source: pxui_types::Str { ptr: text.as_ptr(), len: text.len() } }).unwrap();
        let menu = Menu::DEFAULT.row_h(30.0);
        for _ in 0..3 {
            SEEN.with(|s| s.borrow_mut().clear());
            testing::frame(&mut ui, &[], &mut |ui| { ui.menu("m", (10.0, 10.0), &ITEMS, &menu); });
        }
        let seen = SEEN.with(|s| s.borrow().clone());
        assert_eq!(seen, vec![
            ("Play next".into(), false, false, false, false, 0.0, 30.0), ("Add to playlist".into(), true, false, false, false, 0.0, 30.0),
            ("Delete".into(), false, true, false, false, 0.0, 30.0), ("Size".into(), false, false, true, true, 14.0, 30.0),
        ]);
        assert!(ui.a11y_nodes().iter().filter(|n| n.is_role(Role::MenuItem)).all(|n| n.rect.h == 30.0), "the row node is `row_h` high");
    }

    /// A submenu opens beside the menu while the pointer is on its row; its rows are numbered after the menu's; more than 32 rows work.
    #[test]
    fn submenus_and_long_menus_choose_by_index() {
        const KINDS: [MenuItem; 2] = [MenuItem::new("Draw"), MenuItem::new("Value").checked(true)];
        const ITEMS: [MenuItem; 2] = [MenuItem::new("New").submenu(&KINDS), MenuItem::new("Quit")];
        let (last, menu) = (Cell::new(MenuResponse::default()), Menu::DEFAULT);
        let mut build = |ui: &mut Ui| { let got = ui.menu("song-menu", (10.0, 10.0), &ITEMS, &menu); if got != MenuResponse::default() { last.set(got); } };
        let mut ui = crate::doc::frames(3, &mut build);
        let (x, y) = centre(&ui, "New");
        testing::frame(&mut ui, &[Event { kind: Event::POINTER_MOVED, x, y, ..Default::default() }], &mut build);
        testing::frame(&mut ui, &[], &mut build);
        // near the row's start: the submenu is as wide as its longest row
        let (x, y) = { let (x, y) = centre(&ui, "Value"); (x - 80.0, y) };
        testing::frame(&mut ui, &[Event { kind: Event::POINTER_MOVED, x, y, ..Default::default() }], &mut build);
        testing::click(&mut ui, x, y, &mut build);
        assert_eq!(last.get().chosen, Some(3));
        let long = [MenuItem::new("Row"); 40];
        last.set(MenuResponse::default());
        let mut build = |ui: &mut Ui| { let got = ui.menu("song-menu", (10.0, 10.0), &long, &menu); if got != MenuResponse::default() { last.set(got); } };
        let mut ui = crate::doc::frames(3, &mut build);
        testing::key(&mut ui, TAB, &mut build);
        testing::key(&mut ui, ENTER, &mut build);
        assert_eq!(last.get().chosen, Some(0));
    }

    /// Open, held open, closed and reopened, with a mark slot, a shortcut, a danger row and a 40-row scrolling menu, a warm frame allocates nothing, and
    /// neither does the frame a settled menu opens on.
    #[test]
    fn warm_frames_and_the_opening_frame_allocate_nothing() {
        mem_sys::set_peer(crate::library_counter);
        let mut ui = Ui::new().unwrap();
        let input = FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() };
        let (shown, menu) = (Cell::new(true), Menu::DEFAULT.row_h(36.0).width(200.0));
        const LONG: [MenuItem; 40] = [MenuItem::new("Row"); 40];
        const SIZES: [MenuItem; 2] = [MenuItem::new("4 GB"), MenuItem::new("8 GB").selected()];
        let mut build = |ui: &mut Ui| {
            ui.label("Songs", crate::Label::DEFAULT);
            if shown.get() {
                ui.menu("song-menu", (10.0, 10.0), &ITEMS, &menu);
                ui.menu("long-menu", (200.0, 10.0), &LONG, &Menu::DEFAULT.max_rows(8));
                ui.menu("sizes", (100.0, 120.0), &SIZES, &Menu::DEFAULT.padding(10.0).name("Cache size"));
                ui.menu("bounded", (40.0, 60.0), &LONG, &Menu::DEFAULT.max_h(150.0));
            }
        };
        for _ in 0..8 { ui.frame(&input, &mut build).unwrap(); }
        shown.set(false);
        for _ in 0..4 { ui.frame(&input, &mut build).unwrap(); }
        let scope = mem_sys::warm_scope();
        for round in 0..30 {
            // the first frame of a round opens the menu again, the next two keep it open, the last two have it closed
            shown.set(round % 5 < 3);
            mem_sys::local_reset();
            ui.frame(&input, &mut build).unwrap();
            assert_eq!(mem_sys::local_alloc_count(), 0, "round {round}");
        }
        drop(scope);
        assert!(mem_sys::take_reports().is_empty(), "{:?}", mem_sys::take_reports());
    }

    /// A menu of runtime text with 40 rows and `max_rows(8)` is 8 rows tall; its row column scrolls with the wheel and the row it
    /// brings into view is chosen by its index.
    #[test]
    fn a_menu_past_max_rows_scrolls_its_rows() {
        let names: Vec<String> = (0..40).map(|n| format!("Row {n}")).collect();
        let items: Vec<MenuItem> = names.iter().map(|name| MenuItem::new(name)).collect();
        let (last, menu) = (Cell::new(MenuResponse::default()), Menu::DEFAULT.max_rows(8));
        let mut build = |ui: &mut Ui| { let got = ui.menu("long", (10.0, 10.0), &items, &menu); if got != MenuResponse::default() { last.set(got); } };
        let mut ui = crate::doc::frames(3, &mut build);
        let top = |ui: &Ui, name: &str| testing::find(ui, Role::MenuItem, name).unwrap().y;
        let bottom = 10.0 + 7.0 + 8.0 * 24.0;
        assert!(top(&ui, "Row 7") < bottom && top(&ui, "Row 8") >= bottom, "eight rows show: row 8 starts at {}", top(&ui, "Row 8"));
        testing::frame(&mut ui, &[Event { kind: Event::POINTER_MOVED, x: 60.0, y: 100.0, ..Default::default() }], &mut build);
        // one wheel step is a row: 32 of them reach the end of the 40 rows
        for _ in 0..32 {
            testing::frame(&mut ui, &[Event { kind: Event::SCROLLED, x: 0.0, y: -1.0, ..Default::default() }], &mut build);
        }
        for _ in 0..30 { testing::frame(&mut ui, &[], &mut build); }
        assert!(top(&ui, "Row 39") < bottom, "the last row scrolled into view: {}", top(&ui, "Row 39"));
        let r = testing::find(&ui, Role::MenuItem, "Row 39").unwrap();
        testing::click(&mut ui, f64::from(r.x + 20.0), f64::from(r.y + 12.0), &mut build);
        assert_eq!(last.get().chosen, Some(39));
    }

    /// `max_h` bounds the whole sheet (edges and padding included) and it scrolls; with `max_rows` the smaller height wins; a menu
    /// shorter than the bound keeps its height and does not scroll.
    #[test]
    fn max_h_bounds_the_sheet_and_the_smaller_bound_wins() {
        const LONG: [MenuItem; 40] = [MenuItem::new("Row"); 40];
        const TWO: [MenuItem; 2] = [MenuItem::new("A"), MenuItem::new("B")];
        // a row is 24, the padding 6 above and below, the edge a point at each end: n rows are 14 + 24 n
        for (items, menu, h) in [(&LONG[..], Menu::DEFAULT.max_h(120.0), 120.0), (&LONG[..], Menu::DEFAULT.max_rows(3).max_h(200.0), 86.0),
            (&LONG[..], Menu::DEFAULT.max_rows(8).max_h(120.0), 120.0), (&TWO[..], Menu::DEFAULT.max_h(200.0), 62.0), (&LONG[..], Menu::DEFAULT.max_h(0.0).max_rows(2), 62.0)] {
            let mut build = |ui: &mut Ui| { ui.menu("m", (10.0, 10.0), items, &menu.name("Sheet")); };
            let ui = crate::doc::frames(3, &mut build);
            let sheet = testing::find(&ui, Role::Menu, "Sheet").expect("the sheet");
            assert_eq!(sheet.h, h, "{menu:?}");
        }
        // the bounded sheet scrolls its rows: the wheel brings the last row into view
        let (last, menu) = (Cell::new(MenuResponse::default()), Menu::DEFAULT.max_h(120.0));
        let mut build = |ui: &mut Ui| { let got = ui.menu("m", (10.0, 10.0), &LONG, &menu); if got != MenuResponse::default() { last.set(got); } };
        let mut ui = crate::doc::frames(3, &mut build);
        testing::frame(&mut ui, &[Event { kind: Event::POINTER_MOVED, x: 60.0, y: 60.0, ..Default::default() }], &mut build);
        for _ in 0..40 { testing::frame(&mut ui, &[Event { kind: Event::SCROLLED, x: 0.0, y: -1.0, ..Default::default() }], &mut build); }
        for _ in 0..30 { testing::frame(&mut ui, &[], &mut build); }
        let rows: Vec<_> = ui.a11y_nodes().iter().filter(|n| n.is_role(Role::MenuItem)).map(|n| n.rect).collect();
        let r = *rows.last().unwrap();
        assert!(r.y + r.h <= 10.0 + 120.0, "the last row scrolled into the sheet: {r:?}");
        testing::click(&mut ui, f64::from(r.x + 20.0), f64::from(r.y + 12.0), &mut build);
        assert_eq!(last.get().chosen, Some(39));
    }

    thread_local! { static CURRENT: RefCell<Vec<(String, u8, u8)>> = const { RefCell::new(Vec::new()) }; }

    fn current_row(ui: &mut Ui, c: &MenuRowConfig) {
        CURRENT.with(|seen| seen.borrow_mut().push((c.text_of(&c.text).to_owned(), c.state.checked, c.selected)));
        ui.text(&c.row, c.text_of(&c.text));
    }

    /// A `selected` row reaches a registered row body as `state.checked` (apart from `checked`'s mark, `selected` in the record) and reads
    /// CHECKED and SELECTED; the sheet is named `Menu::name`, never by its key, and `padding` moves the first row down by its change.
    #[test]
    fn a_registered_row_body_gets_the_current_choice_and_the_sheet_its_name() {
        use crate::a11y::state;
        const SIZES: [MenuItem; 3] = [MenuItem::new("4 GB"), MenuItem::new("8 GB").selected(), MenuItem::new("16 GB").checked(true)];
        let mut ui = Ui::new().expect("library");
        ui.set_a11y(true);
        ui.register_menu_row_body("current", current_row).expect("row body");
        let scoped = { let mut theme = crate::Theme::default(); theme.apply(&ui, "menu_row.body = current").expect("theme"); theme };
        let menu = Cell::new(Menu::DEFAULT);
        let mut build = |ui: &mut Ui| ui.themed(&scoped, &mut |ui: &mut Ui| { ui.menu("cache-size-menu", (10.0, 10.0), &SIZES, &menu.get()); });
        let run = |ui: &mut Ui, build: &mut dyn FnMut(&mut Ui)| for n in 0..3 { CURRENT.with(|s| s.borrow_mut().clear()); ui.frame(&FrameIn { width: 320.0, height: 240.0, time: n as f64, ..FrameIn::default() }, build).expect("frame"); };
        run(&mut ui, &mut build);
        let seen = CURRENT.with(|s| s.borrow().clone());
        assert_eq!(seen, [("4 GB".to_owned(), 0, 0), ("8 GB".to_owned(), 1, 0), ("16 GB".to_owned(), 0, 1)], "state.checked is the current choice, selected the mark");
        let states = |ui: &Ui, name: &str| ui.a11y_nodes().iter().find(|n| n.is_role(Role::MenuItem) && ui.a11y_name_of(n) == name).map(|n| n.states).expect("row");
        assert_eq!(states(&ui, "8 GB") & (state::CHECKED | state::SELECTED), state::CHECKED | state::SELECTED);
        assert_eq!(states(&ui, "4 GB") & (state::CHECKED | state::SELECTED), 0);
        let sheet = |ui: &Ui| ui.a11y_nodes().iter().find(|n| n.is_role(Role::Menu)).map(|n| (ui.a11y_name_of(n).to_owned(), n.rect)).expect("sheet");
        let (name, plain) = sheet(&ui);
        assert_eq!(name, "", "no name without Menu::name, never the key");
        let top = testing::find(&ui, Role::MenuItem, "4 GB").unwrap().y;
        menu.set(Menu::DEFAULT.name("Cache size").padding(12.0));
        run(&mut ui, &mut build);
        let (name, roomy) = sheet(&ui);
        assert_eq!(name, "Cache size");
        assert_eq!((testing::find(&ui, Role::MenuItem, "4 GB").unwrap().y - top, roomy.h - plain.h), (6.0, 12.0), "padding 12 moves the rows 6 down and grows the sheet 12");
    }

    /// Two rows of one text (two playlists named alike) are distinct rows with distinct answers: rows are keyed by place, or by
    /// `MenuItem::id`, never by text; the chosen row's id comes back as `chosen_id` (a submenu row's too).
    #[test]
    fn rows_of_one_text_are_distinct_and_report_their_ids() {
        const MIXES: [MenuItem; 2] = [MenuItem::new("Mix").id(70), MenuItem::new("Mix").id(71)];
        const ITEMS: [MenuItem; 4] = [MenuItem::new("Mix"), MenuItem::new("Mix"), MenuItem::new("Mix").id(9), MenuItem::new("Add to").submenu(&MIXES)];
        let (last, menu) = (Cell::new(MenuResponse::default()), Menu::DEFAULT);
        let mut build = |ui: &mut Ui| { let got = ui.menu("m", (10.0, 10.0), &ITEMS, &menu); if got != MenuResponse::default() { last.set(got); } };
        let mut ui = crate::doc::frames(3, &mut build);
        let rows: Vec<crate::geom::Rect> = ui.a11y_nodes().iter().filter(|n| n.is_role(Role::MenuItem) && ui.a11y_name_of(n) == "Mix").map(|n| n.rect).collect();
        assert_eq!(rows.len(), 3, "three rows named Mix: {rows:?}");
        assert!(rows[0].y < rows[1].y && rows[1].y < rows[2].y, "each its own row: {rows:?}");
        testing::click(&mut ui, f64::from(rows[1].x + 20.0), f64::from(rows[1].y + rows[1].h / 2.0), &mut build);
        assert_eq!(last.get(), MenuResponse { chosen: Some(1), ..MenuResponse::default() }, "the second Mix, which has no id");
        let mut ui = crate::doc::frames(3, &mut build);
        testing::click(&mut ui, f64::from(rows[2].x + 20.0), f64::from(rows[2].y + rows[2].h / 2.0), &mut build);
        assert_eq!(last.get(), MenuResponse { chosen: Some(2), chosen_id: Some(9), dismissed: false });
        let mut ui = crate::doc::frames(3, &mut build);
        let (x, y) = centre(&ui, "Add to");
        testing::frame(&mut ui, &[Event { kind: Event::POINTER_MOVED, x, y, ..Default::default() }], &mut build);
        testing::frame(&mut ui, &[], &mut build);
        let second = ui.a11y_nodes().iter().filter(|n| n.is_role(Role::MenuItem) && ui.a11y_name_of(n) == "Mix").map(|n| n.rect).nth(4).expect("the submenu's second Mix");
        let (x, y) = (f64::from(second.x + 20.0), f64::from(second.y + second.h / 2.0));
        testing::frame(&mut ui, &[Event { kind: Event::POINTER_MOVED, x, y, ..Default::default() }], &mut build);
        testing::click(&mut ui, x, y, &mut build);
        assert_eq!(last.get(), MenuResponse { chosen: Some(5), chosen_id: Some(71), dismissed: false });
    }

    /// Rows keyed by id keep their focus when rows move: the focused row's id, not its place, decides it.
    #[test]
    fn a_row_keyed_by_id_keeps_its_focus_when_rows_move() {
        let order = Cell::new(false);
        let (a, b) = (MenuItem::new("A").id(1), MenuItem::new("B").id(2));
        let mut build = |ui: &mut Ui| { let items = if order.get() { [b, a] } else { [a, b] }; ui.menu("m", (10.0, 10.0), &items, &Menu::DEFAULT); };
        let mut ui = crate::doc::frames(3, &mut build);
        testing::key(&mut ui, TAB, &mut build);
        let focused = |ui: &Ui| ui.a11y_nodes().iter().find(|n| n.is_role(Role::MenuItem) && n.states & crate::a11y::state::FOCUSED != 0).map(|n| ui.a11y_name_of(n).to_owned());
        assert_eq!(focused(&ui).as_deref(), Some("A"));
        order.set(true);
        testing::frame(&mut ui, &[], &mut build);
        testing::frame(&mut ui, &[], &mut build);
        assert_eq!(focused(&ui).as_deref(), Some("A"), "the focus followed row A to its new place");
    }

    /// `focus_on_click` makes a press focus the row it lands on; without it a press leaves the rows to Tab.
    #[test]
    fn focus_on_click_focuses_the_pressed_row() {
        const ITEMS: [MenuItem; 2] = [MenuItem::new("One"), MenuItem::new("Two")];
        for (menu, focuses) in [(Menu::DEFAULT, false), (Menu::DEFAULT.focus_on_click(true), true)] {
            let mut build = |ui: &mut Ui| { ui.menu("m", (10.0, 10.0), &ITEMS, &menu); };
            let mut ui = crate::doc::frames(3, &mut build);
            let (x, y) = centre(&ui, "Two");
            testing::frame(&mut ui, &[Event { kind: Event::POINTER_PRESSED, x, y, ..Default::default() }], &mut build);
            testing::frame(&mut ui, &[], &mut build);
            let states = ui.a11y_nodes().iter().find(|n| n.is_role(Role::MenuItem) && ui.a11y_name_of(n) == "Two").map(|n| n.states).expect("row");
            assert_eq!(states & crate::a11y::state::FOCUSED != 0, focuses, "focus_on_click {}", menu.focus_on_click);
        }
    }

    thread_local! { static SCROLL_BODIES: Cell<u32> = const { Cell::new(0) }; }

    fn listing_sheet(ui: &mut Ui, c: &crate::bodies::PopupConfig) {
        ui.set_role(&c.node, Role::List, 0);
        ui.slot(c.content);
    }

    fn counted_scroll(ui: &mut Ui, c: &crate::bodies::ScrollViewConfig) {
        SCROLL_BODIES.with(|n| n.set(n.get() + 1));
        ui.slot(c.content);
    }

    /// A registered popup body sets the menu sheet's role (behaviour gives Menu only to a sheet whose body set none); a menu past
    /// `max_rows` scrolls in the sheet itself, building no scroll view of its own.
    #[test]
    fn a_sheet_body_keeps_its_role_and_a_long_menu_scrolls_in_the_sheet() {
        let mut ui = Ui::new().expect("library");
        ui.set_a11y(true);
        ui.register_popup_body("listing", listing_sheet).expect("popup body");
        ui.register_scroll_view_body("counted", counted_scroll).expect("scroll body");
        let scoped = { let mut theme = crate::Theme::default(); theme.apply(&ui, "popup.body = listing\nscroll_view.body = counted").expect("theme"); theme };
        const LONG: [MenuItem; 20] = [MenuItem::new("Row"); 20];
        let mut build = |ui: &mut Ui| ui.themed(&scoped, &mut |ui: &mut Ui| { ui.menu("m", (10.0, 10.0), &LONG, &Menu::DEFAULT.max_rows(4)); });
        for _ in 0..3 { testing::frame(&mut ui, &[], &mut build); }
        assert!(ui.a11y_nodes().iter().any(|n| n.is_role(Role::List)) && !ui.a11y_nodes().iter().any(|n| n.is_role(Role::Menu)), "the body's role stays");
        assert_eq!(SCROLL_BODIES.with(Cell::get), 0, "no scroll view inside the sheet");
        let sheet = ui.a11y_nodes().iter().find(|n| n.is_role(Role::List)).map(|n| n.rect).expect("sheet");
        assert_eq!(sheet.h, 2.0 + 12.0 + 4.0 * 24.0, "four rows tall");
        let plain = crate::doc::frames(3, &mut |ui: &mut Ui| { ui.menu("m", (10.0, 10.0), &LONG, &Menu::DEFAULT.max_rows(4)); });
        assert!(plain.a11y_nodes().iter().any(|n| n.is_role(Role::Menu)), "the built-in sheet is a Menu");
    }

    /// A menu asked for at the view's far corner is moved inside the view, so the caller clamps nothing.
    #[test]
    fn a_menu_at_the_corner_stays_inside_the_view() {
        let mut build = |ui: &mut Ui| { ui.menu("m", (315.0, 235.0), &ITEMS, &Menu::DEFAULT.row_h(36.0)); };
        let ui = crate::doc::frames(3, &mut build);
        let sheet = ui.a11y_nodes().iter().find(|n| n.is_role(Role::Menu)).map(|n| n.rect).expect("sheet");
        let (w, h) = ui.view_size();
        assert!(sheet.x >= 0.0 && sheet.y >= 0.0 && sheet.x + sheet.w <= w && sheet.y + sheet.h <= h, "{sheet:?} inside {w} x {h}");
    }
}
