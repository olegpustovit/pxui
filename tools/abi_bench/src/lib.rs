//! The gallery pages compiled against the engine directly: the page source is the gallery's own, and the `pxui` module below names
//! the engine's types where the wrapper names its own. The text inputs go through `inputs`, which the gallery implements over the wrapper and
//! this tool over the engine. A tool for the A7 bench, so it may name the private crates.
pub mod pxui {
    pub use pxui_core::text;
    pub use pxui_core::ui::*;
    pub use pxui_core::ui_widgets::TreeNode;
    pub use pxui_types::ui::{Dialog, DialogButton, Chosen, Menu, MenuItem, Pick, PickRow};
    pub mod collections { pub use pxui_core::ui_widgets::TreeNode; }
    pub use pxui_types::theme;
    pub use pxui_core::ui::Handle;
    pub use super::shapes::{Badge, Button, GridList, MenuBar, MenuBarMenu, OverlayResponse, PickResponse, Progress, Radio, SliderResponse, Spinner, Split, SplitResponse, Tabs, Toggle, Ui, ListState, TableState, Column, Table, TableHead, Sort, CellState, TableResponse, HeadResponse};
    /// The extension set as the wrapper names it (`pxui::ext`): free functions over the engine's own calls.
    pub mod ext {
        use super::{Handle, Ui};
        pub fn inspector_section(ui: &mut Ui, key: &str, expanded: bool, set: Option<bool>, label: &str, f: &mut dyn FnMut(&mut Ui)) -> bool { ui.inspector_section(key, expanded, set, label, f) }
        pub fn inspector_toggle(ui: &mut Ui, key: &str, label: &str, value: bool) -> bool { ui.inspector_toggle(key, label, value) }
        pub fn inspector_readout(ui: &mut Ui, width: Option<f32>, key: &str, label: &str, value: &str) { ui.inspector_readout(width, key, label, value) }
        pub fn inspector_row(ui: &mut Ui, width: Option<f32>, pin: Option<bool>, key: &str, label: &str) -> (Handle, f32, f32, f32) { ui.inspector_row(width, pin, key, label) }
        pub fn inspector_button(ui: &mut Ui, key: &str, label: &str) -> bool { ui.inspector_button(key, label) }
        pub fn inspector_message(ui: &mut Ui, key: &str, message: &str) { ui.inspector_message(key, message) }
        pub fn footer(ui: &mut Ui, right: Option<&str>, hints: &[(&str, &str)]) { ui.footer(right, hints) }
    }
}

/// The wrapper's call shapes over the engine's own calls, for the page source: the engine keeps one call per job, and this tool's
/// `Ui` (the engine `Ui` itself, `repr(transparent)`, dereferencing to it) adapts the arguments and results the wrapper reshaped.
/// Each adapter is a few moves around the engine call, so the direct side still measures the engine.
pub mod shapes {
    use core::ops::{Deref, DerefMut};
    use core::ops::RangeInclusive;
    use pxui_core::ui::{Axis, Handle, Node, Rect, Response, Segment, SegmentedOpts};
    use pxui_core::ui_widgets::TreeNode;
    use pxui_types::theme::BodyKind;
    use pxui_types::ui::{Chosen, Dialog, DialogButton, Dropdown, ListResponse, MenuItem, Overlay, SectionPart, Segmented, SelectionList, Slider, TreeResponse};
    use pxui_core::ui_lists::{ColumnSpec, ListCell, ListResult, MAX_COLUMNS};
    use pxui_types::ffi::PxuiColumnWidth;

    pub type Engine = pxui_core::ui::Ui;

    // The wrapper's app-owned list and table state, columns and results (`pxui::author::collection`), as the page source names them.
    #[derive(Clone, Debug, Default)] pub struct ListState { pub rows: Vec<u8>, pub anchor: Option<usize> }
    impl ListState { pub const SELECTED: u8 = 1; pub const DISABLED: u8 = 2; pub const fn new() -> ListState { ListState { rows: Vec::new(), anchor: None } } }
    #[derive(Clone, Copy, Debug, Default)] pub struct CellState { pub selected: bool, pub enabled: bool, pub focused: bool, pub held: bool, pub handle: Handle, pub y: f32 }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Sort { Ascending, Descending }
    #[derive(Clone, Copy, Debug)] pub struct Column<'a> { title: &'a str, sortable: bool, share: f32, w: f32, grip: bool, id: Option<i64> }
    impl<'a> Column<'a> {
        pub const fn new(title: &'a str) -> Column<'a> { Column { title, sortable: false, share: 0.0, w: 100.0, grip: true, id: None } }
        pub const fn sortable(mut self, sortable: bool) -> Column<'a> { self.sortable = sortable; self }
        pub const fn share(mut self, share: f32) -> Column<'a> { self.share = share; self }
        pub const fn w_px(mut self, points: f32) -> Column<'a> { self.w = points; self.share = 0.0; self }
        pub const fn grip(mut self, grip: bool) -> Column<'a> { self.grip = grip; self }
        pub const fn id(mut self, id: i64) -> Column<'a> { self.id = Some(id); self }
    }
    #[derive(Clone, Debug, Default)] pub struct TableState { pub widths: Vec<(i64, f32)>, pub sort: Option<(i64, Sort)>, pub selection: ListState }
    impl TableState { pub const fn new() -> TableState { TableState { widths: Vec::new(), sort: None, selection: ListState::new() } } }
    #[derive(Clone, Copy, Debug)] pub struct TableHead { h: f32, variant: u16 }
    impl TableHead { pub const DEFAULT: TableHead = TableHead { h: 24.0, variant: 0 }; pub const fn h_px(mut self, h: f32) -> TableHead { self.h = h; self } pub const fn variant(mut self, v: u16) -> TableHead { self.variant = v; self } }
    #[derive(Clone, Copy, Debug)] pub struct Table { multi: bool, row_h: f32, head: TableHead, variant: u16 }
    impl Table {
        pub const DEFAULT: Table = Table { multi: false, row_h: 24.0, head: TableHead::DEFAULT, variant: 0 };
        pub const fn multi(mut self) -> Table { self.multi = true; self }
        pub const fn row_h(mut self, h: f32) -> Table { self.row_h = h; self }
        pub const fn head(mut self, head: TableHead) -> Table { self.head = head; self }
        pub const fn variant(mut self, v: u16) -> Table { self.variant = v; self }
    }
    #[derive(Clone, Copy, Debug, Default)] pub struct HeadResponse { pub response: Response, pub sorted: bool, pub resized: bool }
    #[derive(Clone, Copy, Debug, Default)] pub struct TableResponse { pub rows: ListResponse, pub head: HeadResponse }
    fn part_of(index: usize) -> SectionPart {
        match index { usize::MAX => SectionPart::Leading, i if i == usize::MAX - 1 => SectionPart::Header, i if i == usize::MAX - 2 => SectionPart::Trailing, i => SectionPart::Row(i) }
    }
    fn cell_of(c: &ListCell) -> CellState { CellState { selected: c.selected, enabled: c.enabled, focused: c.focused, held: c.held, handle: c.handle, y: c.y } }
    fn abi(b: Handle) -> pxui_types::PxuiBox { pxui_types::PxuiBox { index: b.index as u64, box_key: b.box_key, box_slot: b.box_slot as u64, generation: b.generation } }
    fn list_response(r: &ListResult) -> ListResponse { ListResponse { handle: abi(r.handle), activated: r.activated, selected: r.selected, secondary: r.secondary, row: r.row.map(abi), ..ListResponse::default() } }

    #[repr(transparent)]
    pub struct Ui(Engine);

    impl Deref for Ui { type Target = Engine; fn deref(&self) -> &Engine { &self.0 } }
    impl DerefMut for Ui { fn deref_mut(&mut self) -> &mut Engine { &mut self.0 } }

    /// A page closure over this `Ui` as the engine closure the engine's containers take.
    macro_rules! page { ($f:expr) => { &mut |e: &mut Engine| $f(Ui::of(e)) }; }

    #[derive(Clone, Copy, Debug)] pub struct Progress { variant: u16 }
    impl Progress { pub const DEFAULT: Progress = Progress { variant: 0 }; pub const fn variant(self, variant: u16) -> Progress { Progress { variant } } }
    #[derive(Clone, Copy, Debug)] pub struct Spinner { variant: u16 }
    impl Spinner { pub const DEFAULT: Spinner = Spinner { variant: 0 }; pub const fn variant(self, variant: u16) -> Spinner { Spinner { variant } } }
    #[derive(Clone, Copy, Debug)] pub struct Badge { variant: u16 }
    impl Badge { pub const DEFAULT: Badge = Badge { variant: 0 }; pub const fn variant(self, variant: u16) -> Badge { Badge { variant } } }
    #[derive(Clone, Copy, Debug)] pub struct GridList { w: f32, h: f32 }
    impl GridList {
        pub const DEFAULT: GridList = GridList { w: 64.0, h: 64.0 };
        pub const fn w_px(self, w: f32) -> GridList { GridList { w, ..self } }
        pub const fn h_px(self, h: f32) -> GridList { GridList { h, ..self } }
    }

    /// The wrapper's result and style shapes the pages read (strings are 'static here).
    #[derive(Clone, Copy, Debug)] pub struct PickResponse { pub response: Response, pub chosen: Option<usize>, pub chosen_id: Option<i64>, pub secondary: Option<usize> }
    #[derive(Clone, Copy, Debug)] pub struct SliderResponse { pub response: Response, pub value: f32, pub preview: Option<f32>, pub commit: Option<f32> }
    #[derive(Clone, Copy, Debug)] pub struct OverlayResponse { pub handle: Handle, pub open: bool }
    #[derive(Clone, Copy, Debug)] pub struct SplitResponse { pub response: Response, pub ratio: Option<f32> }
    #[derive(Clone, Copy, Debug)] pub struct Tabs { name: &'static str, variant: u16, focus_on_click: bool }
    impl Tabs {
        pub const DEFAULT: Tabs = Tabs { name: "", variant: 0, focus_on_click: false };
        pub const fn name(self, name: &'static str) -> Tabs { Tabs { name, ..self } }
        pub const fn variant(self, variant: u16) -> Tabs { Tabs { variant, ..self } }
        pub const fn focus_on_click(self, focus_on_click: bool) -> Tabs { Tabs { focus_on_click, ..self } }
    }
    #[derive(Clone, Copy, Debug)] pub struct Radio { name: &'static str, variant: u16 }
    impl Radio {
        pub const DEFAULT: Radio = Radio { name: "", variant: 0 };
        pub const fn name(self, name: &'static str) -> Radio { Radio { name, ..self } }
        pub const fn variant(self, variant: u16) -> Radio { Radio { variant, ..self } }
    }
    #[derive(Clone, Copy, Debug)] pub struct Split { axis: Axis, min_a: f32, min_b: f32 }
    impl Split {
        pub const DEFAULT: Split = Split { axis: Axis::Row, min_a: 0.0, min_b: 0.0 };
        pub const fn axis(self, axis: Axis) -> Split { Split { axis, ..self } }
        pub const fn min_a(self, points: f32) -> Split { Split { min_a: points, ..self } }
        pub const fn min_b(self, points: f32) -> Split { Split { min_b: points, ..self } }
    }
    #[derive(Clone, Copy, Debug)] pub struct MenuBar { variant: u16 }
    impl MenuBar {
        pub const DEFAULT: MenuBar = MenuBar { variant: 0 };
        pub const fn variant(self, variant: u16) -> MenuBar { MenuBar { variant } }
    }
    #[derive(Clone, Copy, Debug)] pub struct MenuBarMenu { title: &'static str, items: &'static [MenuItem<'static>] }
    impl MenuBarMenu { pub const fn new(title: &'static str, items: &'static [MenuItem<'static>]) -> MenuBarMenu { MenuBarMenu { title, items } } }

    /// The wrapper's button and toggle styles as the pages use them (`DEFAULT` only).
    #[derive(Clone, Copy, Debug)] pub struct Button;
    impl Button { pub const DEFAULT: Button = Button; }
    #[derive(Clone, Copy, Debug)] pub struct Toggle;
    impl Toggle { pub const DEFAULT: Toggle = Toggle; }
    /// What the pages read of a toggle's response.
    #[derive(Clone, Copy, Debug)] pub struct Toggled { pub changed: bool, pub handle: Handle }

    impl Ui {
        /// The engine's `Ui` under the page shapes.
        pub fn of(engine: &mut Engine) -> &mut Ui {
            // SAFETY: `Ui` is a `repr(transparent)` wrapper of `Engine`.
            unsafe { &mut *(engine as *mut Engine).cast::<Ui>() }
        }

        fn variant<R>(&mut self, kind: BodyKind, variant: u16, f: impl FnOnce(&mut Engine) -> R) -> R {
            if variant != 0 { self.0.set_variant(kind as u32, variant); }
            let r = f(&mut self.0);
            if variant != 0 { self.0.set_variant(kind as u32, 0); }
            r
        }

        pub fn progress(&mut self, key: &str, text: &str, value: Option<f32>, p: &Progress) -> Handle { self.variant(BodyKind::progress, p.variant, |e| e.progress(key, text, value)) }
        pub fn spinner(&mut self, key: &str, name: &str, s: &Spinner) -> Handle { self.variant(BodyKind::spinner, s.variant, |e| e.spinner(key, name)) }
        pub fn badge(&mut self, key: &str, name: &str, text: &str, b: &Badge) -> Handle { self.variant(BodyKind::badge, b.variant, |e| e.badge(key, name, text)) }
        pub fn grid_list(&mut self, key: &str, count: usize, grid: &GridList, f: &mut dyn FnMut(&mut Ui, usize)) -> Handle {
            self.0.grid_list(key, count, grid.w, grid.h, &mut |e: &mut Engine, i| f(Ui::of(e), i))
        }
        pub fn radio(&mut self, key: &str, items: &[Segment], selected: Option<usize>, radio: &Radio) -> PickResponse {
            let (response, chosen, secondary) = self.0.pick_radio(key, radio.name, items.len(), &|i| items[i], selected, radio.variant);
            PickResponse { response, chosen, chosen_id: chosen.and_then(|i| items[i].id), secondary }
        }
        pub fn tabs(&mut self, key: &str, items: &[Segment], selected: Option<usize>, tabs: &Tabs) -> PickResponse {
            let (response, chosen, secondary) = self.0.tabs(key, tabs.name, items.len(), &|i| items[i], selected, tabs.variant, tabs.focus_on_click);
            PickResponse { response, chosen, chosen_id: chosen.and_then(|i| items[i].id), secondary }
        }
        pub fn dropdown(&mut self, key: &str, text: &str, items: &[Segment], selected: Option<usize>, dropdown: &Dropdown) -> PickResponse {
            let (response, chosen, secondary) = self.0.pick_dropdown(key, text, items.len(), &|i| items[i], selected, dropdown.variant);
            PickResponse { response, chosen, chosen_id: chosen.and_then(|i| items[i].id), secondary }
        }
        pub fn segmented(&mut self, key: &str, items: &[Segment], selected: Option<usize>, s: &Segmented) -> PickResponse {
            let opts = SegmentedOpts { enabled: s.enabled, h: s.h, grow: s.grow, states: s.look.copied(), w: s.w, padding: s.padding, min_segment: s.min_segment, focus_on_click: s.focus_on_click };
            let (response, chosen, secondary) = self.variant(BodyKind::segmented, s.variant, |e| e.segmented_opts(key, s.name.unwrap_or(""), items, selected, &opts));
            PickResponse { response, chosen, chosen_id: chosen.and_then(|i| items[i].id), secondary }
        }
        pub fn slider(&mut self, key: &str, text: &str, value: f32, range: RangeInclusive<f32>, slider: &Slider) -> SliderResponse {
            let (response, value, preview, commit) = self.0.slider_result(key, text, value, range, slider);
            SliderResponse { response, value, preview, commit }
        }
        pub fn menu_bar(&mut self, key: &str, menus: &[MenuBarMenu], bar: &MenuBar) -> Chosen {
            let (picked, dismissed) = self.variant(BodyKind::menu_bar, bar.variant, |e| e.menu_bar_result(key, menus.len(), &|m| (menus[m].title, menus[m].items.len()), &|m, i| menus[m].items[i].text));
            let item = picked.and_then(|(m, i)| Some((menus.get(m)?.items.get(i)?, m, i)));
            Chosen { chosen: item.map(|(_, m, i)| menus[..m].iter().map(|menu| menu.items.len()).sum::<usize>() + i), chosen_id: item.and_then(|(it, _, _)| it.id), dismissed }
        }
        pub fn popup(&mut self, key: &str, overlay: &Overlay, f: &mut dyn FnMut(&mut Ui)) -> OverlayResponse {
            let root = self.0.popup_root(key, overlay, page!(f));
            OverlayResponse { handle: root.unwrap_or_default(), open: root.is_some() }
        }
        pub fn button(&mut self, key: &str, text: &str, _: &Button) -> pxui_core::ui::Response {
            self.0.button_opts(key, text, None, &pxui_core::ui_kit::ButtonOpts::DEFAULT, [""; 3], None, None)
        }
        pub fn toggle(&mut self, key: &str, text: &str, on: bool, _: &Toggle) -> Toggled {
            let r = self.0.toggle_opts(key, text, on, &pxui_core::ui::ToggleOpts::DEFAULT);
            Toggled { changed: r.clicked, handle: r.handle }
        }
        pub fn within(&mut self, b: &Handle, f: &mut dyn FnMut(&mut Ui)) { self.0.within(b, page!(f)) }
        pub fn view_of(&mut self, key: &str, revision: &u64, f: &mut dyn FnMut(&mut Ui)) -> Handle { self.0.view_of(key, revision, page!(f)) }
        pub fn scroll_view(&mut self, key: &str, node: Node, f: &mut dyn FnMut(&mut Ui)) -> Handle { self.0.scroll_view(key, node, page!(f)) }
        pub fn panel(&mut self, key: &str, overlay: &Overlay, f: &mut dyn FnMut(&mut Ui)) { self.0.panel(key, overlay, page!(f)) }
        pub fn modal(&mut self, key: &str, overlay: &Overlay, f: &mut dyn FnMut(&mut Ui)) -> OverlayResponse {
            let root = self.0.modal_root(key, overlay, page!(f));
            OverlayResponse { handle: root.unwrap_or_default(), open: root.is_some() }
        }
        pub fn anchored(&mut self, key: &str, anchor: Rect, overlay: &Overlay, f: &mut dyn FnMut(&mut Ui)) -> Handle { self.0.anchored(key, anchor, overlay, page!(f)) }
        pub fn inspector_section(&mut self, key: &str, expanded: bool, set: Option<bool>, label: &str, f: &mut dyn FnMut(&mut Ui)) -> bool {
            self.0.inspector_section(key, expanded, set, label, page!(f))
        }
        pub fn dialog(&mut self, key: &str, title: &str, buttons: &[DialogButton], style: &Dialog, f: &mut dyn FnMut(&mut Ui)) -> Chosen {
            self.0.dialog(key, title, buttons, style, page!(f))
        }
        pub fn split(&mut self, key: &str, ratio: f32, split: &Split, a: &mut dyn FnMut(&mut Ui), b: &mut dyn FnMut(&mut Ui)) -> SplitResponse {
            let (response, _, ratio) = self.0.split_result(key, split.axis, ratio, split.min_a, split.min_b, page!(a), page!(b));
            SplitResponse { response, ratio }
        }
        pub fn selection_list(&mut self, key: &str, count: usize, row_key: &dyn Fn(usize) -> i64, state: &mut ListState, list: &SelectionList, f: &mut dyn FnMut(&mut Ui, SectionPart, CellState)) -> ListResponse {
            state.rows.resize(count, 0);
            if list.variant != 0 { self.0.set_variant(BodyKind::selection_list as u32, list.variant); }
            let r = self.0.selection_rows(key, count, row_key, &mut state.rows, &mut state.anchor, list, &mut |e: &mut Engine, i, c| f(Ui::of(e), part_of(i), cell_of(c)));
            if list.variant != 0 { self.0.set_variant(BodyKind::selection_list as u32, 0); }
            list_response(&r)
        }
        pub fn tree(&mut self, key: &str, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], f: &mut dyn FnMut(&mut Ui, usize)) -> TreeResponse {
            self.0.tree(key, nodes, open, selected, visible, &mut |e: &mut Engine, i| f(Ui::of(e), i))
        }
        #[allow(clippy::too_many_arguments)]
        pub fn table(&mut self, key: &str, columns: &[Column], rows: usize, row_key: &dyn Fn(usize) -> i64, state: &mut TableState, table: &Table, f: &mut dyn FnMut(&mut Ui, usize, usize, CellState)) -> TableResponse {
            let n = columns.len().min(MAX_COLUMNS);
            let (mut specs, mut titles, mut widths) = ([ColumnSpec::default(); MAX_COLUMNS], [""; MAX_COLUMNS], [PxuiColumnWidth::default(); MAX_COLUMNS]);
            for (i, c) in columns[..n].iter().enumerate() {
                (specs[i], titles[i]) = (ColumnSpec { id: c.id.unwrap_or(i as i64), share: c.share, w: c.w, grip: c.grip, sortable: c.sortable }, c.title);
            }
            state.widths.resize(n, (0, 0.0));
            for (w, s) in widths.iter_mut().zip(&state.widths) { (w.id, w.w) = *s; }
            state.selection.rows.resize(rows, 0);
            let mut sort = state.sort.map(|(id, s)| (id, s == Sort::Ascending)).filter(|(id, _)| specs[..n].iter().any(|c| c.id == *id));
            if table.variant != 0 { self.0.set_variant(BodyKind::table_row as u32, table.variant); }
            let (r, h) = self.0.table(key, &specs[..n], &titles[..n], &mut widths[..n], &mut sort, rows, row_key, &mut state.selection.rows, &mut state.selection.anchor, table.multi, table.row_h, table.head.h, table.head.variant, &mut |e: &mut Engine, row, c, cell| f(Ui::of(e), row, c, cell_of(cell)));
            if table.variant != 0 { self.0.set_variant(BodyKind::table_row as u32, 0); }
            for (s, w) in state.widths.iter_mut().zip(&widths[..n]) { *s = (w.id, w.w); }
            state.sort = sort.map(|(id, a)| (id, if a { Sort::Ascending } else { Sort::Descending }));
            TableResponse { rows: list_response(&r), head: HeadResponse { response: h.response, sorted: h.sorted, resized: h.resized } }
        }
    }
}

pub use shapes::Engine;

/// The gallery's text inputs over the engine (the wrapper's version is `pxui_gallery::inputs`).
pub mod inputs {
    use super::pxui::{Pick, Ui};
    use pxui_core::opts::{PickerOpts, TextAreaOpts, ValueFieldOpts};

    pub fn view_size(ui: &Ui) -> (f32, f32) { ui.view_size() }
    pub fn caption(ui: &mut Ui, caption: &mut String) {
        if let Some(v) = ui.text_field("Caption", caption) { *caption = v; }
    }
    pub fn scale(ui: &mut Ui, (x, y, w): (f32, f32, f32), scale: &mut String) {
        if let (Some(v), _) = ui.value_field((x, y), w, 20.0, &ValueFieldOpts::default(), &|t: &str| t.parse::<f64>().is_ok(), None, None, "scale-value", scale) { *scale = v.to_owned(); }
    }
    pub fn notes(ui: &mut Ui, notes: &mut String) {
        if let (Some(text), _) = ui.text_area_submit(pxui_types::Node::DEFAULT.at(12.0, 4.0).w_px(270.0).h_px(56.0), &TextAreaOpts::default(), "notes-area", notes) { *notes = text.to_owned(); }
    }
    /// The field and its response, named, as the wrapper's `search_field` builds them (the page reads only the change).
    pub fn layer_search(ui: &mut Ui, search: &mut String) {
        if let Some(q) = ui.search_field("Find", "/", search, "Search layers", pxui_types::TextTraits::SEARCH) { *search = q.to_owned(); }
        let _ = ui.input_response(Some("Find"));
    }
    pub fn images(ui: &mut Ui, image: &super::pxui::Image) {
        use super::pxui::{Fit, Size};
        for (label, fit) in [("Contain", Fit::Contain), ("Cover", Fit::Cover), ("Fill", Fit::Fill), ("None", Fit::None)] {
            let _ = ui.image(label, image, fit, Size::Px(40.0), Size::Px(40.0));
        }
    }
    pub fn presets(ui: &mut Ui, rows: &mut dyn FnMut(&str, &mut pxui_types::ui::PickRows)) -> Pick {
        ui.picker("presets-search", "Search presets", "", &PickerOpts { limit: 4, ..PickerOpts::default() }, rows).pick
    }
}

pub mod gallery {
    use super::pxui;
    include!("../../pxui_gallery/src/pages.rs");
}
