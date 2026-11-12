//! Widget bodies: an app builds a widget kind's nodes itself. `Ui::register_toggle_body(name, f)` and its siblings (one per kind,
//! generated from `pxui_types::body_kinds!`) register `f`, a plain `fn(&mut Ui, &Config)`, through the one export
//! `pxui_body_register`; a theme (`theme.body.toggle = id`, or `toggle.body = NAME` in a theme file applied with `Theme::apply`) then picks
//! it. The body runs as a callback from the library under the callback panic rule: a panic in it is carried to the call that can run
//! callbacks (the frame) and the body calls after it are skipped. The config is the kind's C record: borrowed until the body returns,
//! text fields are read with `text_of`, handles are `PxuiBox`es the body may restyle and enter, and slots run the caller's content.
use crate::composite::{defer_callback_panic, deferred};
use crate::{as_str, check, Ui};
use core::{ffi::c_void, mem::ManuallyDrop, ptr::NonNull};
use pxui_types::{geom::Rect, theme::{BodyId, BodyKind, Part}, ffi::{PxuiBodyFn, PxuiSlot}, Node, PxuiBox, Str};

pub use pxui_types::ffi::{PxuiToggleConfig, PxuiSliderConfig, PxuiProgressConfig, PxuiButtonConfig, PxuiIconButtonConfig, PxuiCheckConfig, PxuiSpinnerConfig, PxuiBadgeConfig, PxuiLabelConfig, PxuiFooterConfig, PxuiGripConfig, PxuiPanelConfig, PxuiDialogConfig, PxuiSectionConfig, PxuiInspectorRowConfig, PxuiSplitConfig, PxuiScrollConfig, PxuiToastConfig, PxuiTipConfig, PxuiTabConfig, PxuiSegmentConfig, PxuiDropdownConfig, PxuiMenuItemConfig, PxuiMenuRowConfig, PxuiRowConfig, PxuiTreeRowConfig, PxuiTableHeadConfig, PxuiTableCellConfig, PxuiPickerRowConfig, PxuiSearchFieldConfig, PxuiTextFieldConfig, PxuiLineFieldConfig, PxuiValueFieldConfig, PxuiPickerFieldConfig};

library!(sys {
    fn pxui_body_register(ui: *mut PxuiUi, kind: u32, name: Str, body: PxuiBodyFn, user: *mut c_void) -> u16;
    fn pxui_part(ui: *mut PxuiUi, part: u32, node: *const Node, out: *mut PxuiBox) -> i32;
    fn pxui_part_box(ui: *mut PxuiUi, owner: i64, part: u32, out: *mut PxuiBox) -> i32;
    #[trusted] fn pxui_slot(ui: *mut PxuiUi, slot: PxuiSlot, out: *mut u8) -> i32;
});

/// The text of a config field, borrowed from the config (valid until the body returns).
pub fn text_of(value: &Str) -> &str {
    if value.len == 0 || value.ptr.is_null() { return ""; }
    core::str::from_utf8(unsafe { core::slice::from_raw_parts(value.ptr, value.len) }).unwrap_or("")
}

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
    ($($kind:ident $register:ident $body:ident $config:ident),* $(,)?) => {
        impl Ui {
            $(
                #[doc = concat!("Registers `body` as the ", stringify!($kind), " body called `name`, for a theme's `body.", stringify!($kind), "` (`", stringify!($kind), ".body = NAME` in a theme file). Registering a name again replaces its function and keeps its id; a name has the same id in every `Ui` (a theme carried to another `Ui` selects that name's body there, the built-in body where it has none); the error is the library's reason (`default` and names with a space, `=` or `#` are refused, and so is a call inside a frame).")]
                pub fn $register(&mut self, name: &str, body: fn(&mut Ui, &$config)) -> Result<BodyId, String> {
                    extern "C" fn entry(user: *mut c_void, raw: *mut c_void, config: *const c_void) {
                        enter(raw, config, &mut |ui| (unsafe { core::mem::transmute::<*mut c_void, fn(&mut Ui, &$config)>(user) })(ui, unsafe { &*config.cast::<$config>() }));
                    }
                    self.register_body(BodyKind::$kind, name, entry, body as *mut c_void)
                }
            )*
        }
    };
}
pxui_types::body_kinds!(typed);

impl Ui {
    fn register_body(&mut self, kind: BodyKind, name: &str, entry: extern "C" fn(*mut c_void, *mut c_void, *const c_void), user: *mut c_void) -> Result<BodyId, String> {
        match unsafe { sys::pxui_body_register(self.0.as_ptr(), kind as u32, as_str(name), Some(entry), user) } {
            0 => Err(self.last_error().to_owned()),
            id => Ok(BodyId(id)),
        }
    }

    /// Builds a node as `part` of the widget whose body is running: behaviour finds it again by role (see the config's documentation
    /// for the parts its widget reads). Only inside a body.
    #[track_caller]
    pub fn part(&mut self, part: Part, node: Node) -> PxuiBox {
        let mut out = PxuiBox::default();
        check(self, unsafe { sys::pxui_part(self.0.as_ptr(), part.0, &node, &mut out) });
        out
    }

    /// The box of `part` of the widget keyed `owner`, as the last build left it; `None` if its body built none.
    pub fn part_box(&self, owner: i64, part: Part) -> Option<PxuiBox> {
        let mut out = PxuiBox::default();
        (unsafe { sys::pxui_part_box(self.0.as_ptr(), owner, part.0, &mut out) } == 0).then_some(out)
    }

    /// The rectangle `part` of the widget keyed `owner` had last frame; `None` before it has been laid out or if the body builds none.
    pub fn part_rect(&self, owner: i64, part: Part) -> Option<Rect> {
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
    use crate::Theme;
    use core::cell::{Cell, RefCell};
    use pxui_types::{ffi::ThemeSource, FrameIn, Size};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    thread_local! {
        static TOGGLES: RefCell<Vec<(String, bool, bool)>> = const { RefCell::new(Vec::new()) };
        static ROWS: Cell<i64> = const { Cell::new(0) };
        static CONTENT: Cell<u32> = const { Cell::new(0) };
        static SLOT: Cell<PxuiSlot> = const { Cell::new(PxuiSlot { serial: 0 }) };
        static BOOM: Cell<bool> = const { Cell::new(true) };
        static TAB: RefCell<Option<PxuiBox>> = const { RefCell::new(None) };
    }

    fn input(time: f64) -> FrameIn { FrameIn { width: 320.0, height: 240.0, time, ..FrameIn::default() } }
    fn source(text: &str) -> ThemeSource { ThemeSource { size: core::mem::size_of::<ThemeSource>() as u32, flags: 0, source: Str { ptr: text.as_ptr(), len: text.len() } } }

    fn probe(ui: &mut Ui, c: &crate::bodies::PxuiToggleConfig) {
        TOGGLES.with(|t| t.borrow_mut().push((text_of(&c.label).to_owned(), c.on != 0, c.state.hovered != 0)));
        ROWS.with(|r| r.set(c.row.box_key));
        let b = ui.part(Part::HIT, Node { w: Size::Px(12.0), h: Size::Px(12.0), ..Node::DEFAULT });
        ui.text(&b, "x");
    }

    /// A registered body is called with its kind's record (text, value, state, the row it owns), builds a part behaviour then finds, is picked
    /// by theme text (the Ui's own and a scoped `Theme`) and only inside the scope that names it.
    #[test]
    fn a_registered_body_is_selected_by_theme_text_and_gets_its_config() {
        let mut ui = Ui::new().expect("library");
        let id = ui.register_toggle_body("probe", probe).expect("registered");
        assert_eq!(id, BodyId(1));
        assert_eq!(ui.register_toggle_body("probe", probe).expect("again"), id, "a name again keeps its id");
        let scoped = {
            let mut theme = Theme::default();
            theme.apply(&ui, "toggle.body = probe").expect("scoped theme");
            theme
        };
        // outside the scope the built-in body builds
        for n in 0..2 { ui.frame(&input(n as f64), &mut |ui: &mut Ui| { ui.toggle("plain", true); }).expect("frame"); }
        assert!(TOGGLES.with(|t| t.borrow().is_empty()), "the body ran with no theme naming it");
        for n in 2..4 {
            ui.frame(&input(n as f64), &mut |ui: &mut Ui| { ui.toggle("outside", false); ui.themed(&scoped, &mut |ui: &mut Ui| { ui.toggle("Wi-Fi", true); }); }).expect("frame");
        }
        let seen = TOGGLES.with(|t| t.borrow().clone());
        assert!(!seen.is_empty() && seen.iter().all(|s| *s == ("Wi-Fi".to_owned(), true, false)), "only the scoped toggle ran the body: {seen:?}");
        assert!(ui.part_rect(ROWS.with(Cell::get), Part::HIT).is_some(), "the part the body built is found by its widget's key");
        assert!(ui.part_box(ROWS.with(Cell::get), Part::TRACK).is_none());
        // the Ui's own theme, from the theme text
        TOGGLES.with(|t| t.borrow_mut().clear());
        ui.set_theme(&source("toggle.body = probe")).expect("theme text");
        assert_eq!(ui.theme().body.toggle, id);
        ui.frame(&input(5.0), &mut |ui: &mut Ui| { ui.toggle("Own", false); }).expect("frame");
        assert_eq!(TOGGLES.with(|t| t.borrow().clone()), vec![("Own".to_owned(), false, false)]);
    }

    /// A theme text that names no registered body, and every bad registration, are errors, not crashes.
    #[test]
    fn unknown_body_names_and_bad_registrations_are_errors() {
        let mut ui = Ui::new().expect("library");
        for name in ["default", "", "two words", "a=b", "x#"] { assert!(ui.register_toggle_body(name, probe).is_err(), "{name:?}"); }
        let mut theme = Theme::default();
        let error = theme.apply(&ui, "toggle.body = nope").expect_err("an unregistered body");
        assert!(error.contains("nope"), "{error}");
        assert_eq!(ui.set_theme(&source("toggle.body = nope")), Err(pxui_types::PXUI_ERR_ARGUMENT));
        assert!(ui.last_error().contains("nope"), "{}", ui.last_error());
        // a body of another kind is not this kind's
        ui.register_slider_body("probe", |_, _| {}).expect("slider");
        assert!(theme.apply(&ui, "toggle.body = probe").is_err());
        let mut late = None;
        ui.frame(&input(0.0), &mut |ui: &mut Ui| { late = Some(ui.register_toggle_body("late", probe)); }).expect("frame");
        assert!(late.expect("ran").is_err(), "bodies are registered outside a frame");
        // a part outside a body is a caller bug
        let refused = catch_unwind(AssertUnwindSafe(|| ui.part(Part::HIT, Node::DEFAULT)));
        assert!(refused.is_err());
        assert!(ui.last_error().contains("inside a body"), "{}", ui.last_error());
        ui.frame(&input(1.0), &mut |ui: &mut Ui| ui.label("still usable")).expect("the Ui is not poisoned");
    }

    fn boom(_: &mut Ui, _: &crate::bodies::PxuiToggleConfig) {
        TOGGLES.with(|t| t.borrow_mut().push((String::new(), false, false)));
        if BOOM.with(Cell::get) { panic!("boom") }
    }

    /// A body that panics follows the callback panic rule: the panic is carried to the frame and resumed there, the bodies after it
    /// are skipped, and the next frame builds again.
    #[test]
    fn a_body_that_panics_follows_the_callback_panic_rule() {
        let mut ui = Ui::new().expect("library");
        ui.register_toggle_body("boom", boom).expect("registered");
        ui.set_theme(&source("toggle.body = boom")).expect("theme");
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = ui.frame(&input(0.0), &mut |ui: &mut Ui| { ui.toggle("first", false); ui.toggle("second", true); });
        }));
        assert_eq!(result.expect_err("the panic surfaces at frame()").downcast_ref::<&str>(), Some(&"boom"));
        assert_eq!(TOGGLES.with(|t| t.borrow().len()), 1, "the second body ran on a half-built tree");
        BOOM.with(|b| b.set(false));
        ui.frame(&input(1.0), &mut |ui: &mut Ui| { ui.toggle("first", false); ui.toggle("second", true); }).expect("the next frame renders");
        assert_eq!(TOGGLES.with(|t| t.borrow().len()), 3);
    }

    fn scroll(ui: &mut Ui, c: &crate::bodies::PxuiScrollConfig) {
        SLOT.with(|s| s.set(c.content));
        assert!(!ui.slot(c.content), "a content slot answers false");
    }

    /// A body runs the caller's content through its slot, once; the slot is stale after the body call and refused, without poisoning.
    #[test]
    fn a_slot_runs_the_callers_content_and_is_refused_once_stale() {
        let mut ui = Ui::new().expect("library");
        ui.register_scroll_view_body("probe", scroll).expect("registered");
        ui.set_theme(&source("scroll_view.body = probe")).expect("theme");
        ui.frame(&input(0.0), &mut |ui: &mut Ui| { ui.scroll_view("list", Size::Grow, Size::Grow, &mut |ui: &mut Ui| { CONTENT.with(|c| c.set(c.get() + 1)); ui.label("row"); }); }).expect("frame");
        assert_eq!(CONTENT.with(Cell::get), 1);
        let stale = catch_unwind(AssertUnwindSafe(|| ui.slot(SLOT.with(Cell::get))));
        assert!(stale.is_err());
        assert!(ui.last_error().contains("stale slot"), "{}", ui.last_error());
        ui.frame(&input(1.0), &mut |ui: &mut Ui| ui.label("still usable")).expect("the Ui is not poisoned");
    }

    /// Content run through a slot that panics follows the callback panic rule too (in the static build it unwinds through the slot to
    /// the body's entry): the panic surfaces at the frame, and the next frame's body runs its slot again.
    #[test]
    fn a_slot_whose_content_panics_follows_the_callback_panic_rule() {
        let mut ui = Ui::new().expect("library");
        ui.register_scroll_view_body("probe", |ui, c| { ui.slot(c.content); }).expect("registered");
        ui.set_theme(&source("scroll_view.body = probe")).expect("theme");
        let ran = Cell::new(0);
        let build = |ui: &mut Ui, boom: bool| { ui.scroll_view("list", Size::Grow, Size::Grow, &mut |_: &mut Ui| { ran.set(ran.get() + 1); if boom { panic!("content") } }); };
        let result = catch_unwind(AssertUnwindSafe(|| { let _ = ui.frame(&input(0.0), &mut |ui: &mut Ui| build(ui, true)); }));
        assert_eq!(result.expect_err("the panic surfaces at frame()").downcast_ref::<&str>(), Some(&"content"));
        ui.frame(&input(1.0), &mut |ui: &mut Ui| build(ui, false)).expect("the next frame renders");
        assert_eq!(ran.get(), 2, "the slot ran again the frame after");
    }

    fn wide_tab(ui: &mut Ui, c: &crate::bodies::PxuiTabConfig) {
        let mut node = ui.node_layout(&c.node);
        node.w = Size::Px(100.0);
        ui.set_node_layout(&c.node, &node);
        ui.text(&c.node, text_of(&c.text));
        if c.index == 1 { TAB.with(|t| *t.borrow_mut() = Some(c.node)); }
    }

    /// A body sizes the node its widget gave it (`set_node_layout`): the second tab is 100 points wide.
    #[test]
    fn a_body_sizes_its_node_with_node_layout() {
        let mut ui = Ui::new().expect("library");
        ui.register_tab_body("wide", wide_tab).expect("registered");
        ui.set_theme(&source("tab.body = wide")).expect("theme");
        for n in 0..3 { ui.frame(&input(n as f64), &mut |ui: &mut Ui| { ui.tabs("T", &["a", "b"], 0); }).expect("frame"); }
        let tab = TAB.with(|t| t.borrow().expect("the body ran"));
        assert_eq!(ui.rect(&tab).w, 100.0);
    }

    /// The `_with` forms build the caller's own children for each row and cell, in both link modes.
    #[test]
    fn the_with_forms_build_the_callers_cells() {
        let mut ui = Ui::new().expect("library");
        let (mut list, mut open, mut visible) = (vec![0u8; 5], vec![0u8; 3], vec![0u32; 3]);
        let nodes = [crate::TreeNode { depth: 0, children: 1 }, crate::TreeNode { depth: 1, children: 0 }, crate::TreeNode { depth: 0, children: 0 }];
        let (mut widths, mut selected, mut tree_selected) = ([80.0f32, 80.0], None, None);
        let (rows, cells, nodes_built) = (Cell::new(0), Cell::new(0), Cell::new(0));
        for n in 0..3 {
            ui.frame(&input(n as f64), &mut |ui: &mut Ui| {
                ui.list_view_with("list", 5, false, &mut list, &mut |ui: &mut Ui, row| { rows.set(rows.get() + 1); ui.label(&format!("row {row}")); });
                let shown = ui.tree_with("tree", &nodes, &mut open, &mut tree_selected, &mut visible, &mut |ui: &mut Ui, node| { nodes_built.set(nodes_built.get() + 1); ui.label(&format!("node {node}")); }).1;
                assert_eq!(shown, 2, "the first node's child is closed");
                let _ = ui.table_view_with("table", &["a", "b"], &mut widths, 3, None, &mut selected, &mut |ui: &mut Ui, row, column| { cells.set(cells.get() + 1); ui.label(&format!("{row}:{column}")); });
            }).expect("frame");
        }
        assert!(rows.get() > 0 && nodes_built.get() > 0 && cells.get() > 0, "rows {}, nodes {}, cells {}", rows.get(), nodes_built.get(), cells.get());
        assert_eq!(visible[..2], [0, 2]);
    }
}
