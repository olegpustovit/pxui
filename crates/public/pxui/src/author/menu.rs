//! The kit menu: `ui.menu(key, at, &ITEMS, &Menu::DEFAULT)` reports a `MenuResponse`.
use super::Key;
use crate::{ui::{Menu, MenuItem, MenuResponse}, Ui};

impl Ui {
    /// Shows a floating menu of `items` at `at` (window points, moved inside the window) and reports what happened to it this frame.
    ///
    /// Call it every frame the menu is open and stop when `dismissed` or `chosen` comes back: a `chosen` index is the row clicked (or
    /// Enter on the focused row) and the menu has closed itself; Escape, focus loss or a press outside set `dismissed`. A disabled row
    /// is never chosen and empty text is a separator. Build it before the content it shields; it floats at the root. The menu is as wide
    /// as its longest row (`Menu::width` is the least). Rows with a submenu open it beside the menu; its rows are numbered after the
    /// menu's own. Rows are drawn by the registered `context_menu` body.
    /// Allocates: never on a warm frame, except that rows with a submenu allocate per call.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`) and is the menu's accessible name.
    /// Panics: when the library refuses the arguments (outside a frame, a non-finite position or width).
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
        self.menu_keyed(key.into(), at, items, menu)
    }
}

#[cfg(test)]
mod tests {
    use crate::bodies::PxuiMenuRowConfig;
    use crate::ui::{Menu, MenuItem, MenuResponse, Role};
    use crate::{testing, FrameIn, Ui};
    use pxui_types::{Event, KEY_SPECIAL};
    use std::cell::{Cell, RefCell};

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
        assert_eq!(last.get(), MenuResponse { chosen: Some(2), dismissed: false });
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
        assert_eq!(last.get(), MenuResponse { chosen: None, dismissed: true });
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
    fn marker(ui: &mut Ui, c: &PxuiMenuRowConfig) {
        assert!(c.size as usize >= core::mem::size_of::<PxuiMenuRowConfig>(), "this library carries the row height");
        SEEN.with(|s| s.borrow_mut().push((crate::bodies::text_of(&c.text).to_owned(), c.state.disabled != 0, c.danger != 0, c.selected != 0, c.has_shortcut != 0, c.lead, c.row_h)));
        ui.node("custom".into(), pxui_types::Node { w: pxui_types::Size::Px(7.0), h: pxui_types::Size::Px(9.0), ..pxui_types::Node::DEFAULT });
    }

    /// With a registered `context_menu` body every row goes through it, with the item's flags and the menu's row height; its
    /// structure is what shows.
    #[test]
    fn a_registered_row_body_builds_the_rows() {
        let mut ui = Ui::new().unwrap();
        ui.set_a11y(true);
        ui.register_context_menu_body("marker", marker).expect("registered");
        let text = "context_menu.body = marker\n";
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
        let mut build = |ui: &mut Ui| {
            ui.label("Songs", crate::Label::DEFAULT);
            if shown.get() { ui.menu("song-menu", (10.0, 10.0), &ITEMS, &menu); ui.menu("long-menu", (200.0, 10.0), &LONG, &Menu::DEFAULT.max_rows(8)); }
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
}
