//! The gallery pages compiled against the engine directly: the page source is the gallery's own, and the `pxui` module below names
//! the engine's types where the wrapper names its own. The text inputs go through `inputs`, which the gallery implements over the wrapper and
//! this tool over the engine. A tool for the A7 bench, so it may name the private crates.
pub mod pxui {
    pub use pxui_core::text;
    pub use pxui_core::ui::*;
    pub use pxui_core::ui_widgets::TreeNode;
    pub use pxui_types::ui::{Dialog, DialogButton, Chosen, Menu, MenuItem, Pick};
    pub mod collections { pub use pxui_core::ui_widgets::TreeNode; }
    pub use pxui_types::theme;
    pub use pxui_core::ui::Handle;
    pub use super::shapes::{Badge, Button, GridList, Progress, Spinner, Toggle, Ui};
}

/// The wrapper's call shapes over the engine's own calls, for the page source: the engine keeps one call per job, and this tool's
/// `Ui` (the engine `Ui` itself, `repr(transparent)`, dereferencing to it) adapts the arguments and results the wrapper reshaped.
/// Each adapter is a few moves around the engine call, so the direct side still measures the engine.
pub mod shapes {
    use core::ops::{Deref, DerefMut};
    use pxui_core::ui::{Handle, Node, Rect};
    use pxui_core::ui_widgets::TreeNode;
    use pxui_types::theme::BodyKind;
    use pxui_types::ui::{Chosen, Dialog, DialogButton, ListResponse, Overlay, SelectionList, TableResponse, TableView, TreeResponse};

    pub type Engine = pxui_core::ui::Ui;

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
        pub fn modal(&mut self, key: &str, overlay: &Overlay, f: &mut dyn FnMut(&mut Ui)) -> pxui_core::ui_widgets::OverlayResponse { self.0.modal(key, overlay, page!(f)) }
        pub fn anchored(&mut self, key: &str, anchor: Rect, overlay: &Overlay, f: &mut dyn FnMut(&mut Ui)) -> Handle { self.0.anchored(key, anchor, overlay, page!(f)) }
        pub fn inspector_section(&mut self, key: &str, expanded: bool, set: Option<bool>, label: &str, f: &mut dyn FnMut(&mut Ui)) -> bool {
            self.0.inspector_section(key, expanded, set, label, page!(f))
        }
        pub fn dialog(&mut self, key: &str, title: &str, buttons: &[DialogButton], style: &Dialog, f: &mut dyn FnMut(&mut Ui)) -> Chosen {
            self.0.dialog(key, title, buttons, style, page!(f))
        }
        pub fn split(&mut self, key: &str, ratio: f32, split: &pxui_core::ui_widgets::Split, a: &mut dyn FnMut(&mut Ui), b: &mut dyn FnMut(&mut Ui)) -> pxui_core::ui_widgets::SplitResponse {
            self.0.split(key, ratio, split, page!(a), page!(b))
        }
        pub fn selection_list(&mut self, key: &str, count: usize, selected: &mut [u8], list: &SelectionList, f: &mut dyn FnMut(&mut Ui, usize)) -> ListResponse {
            self.0.selection_list(key, count, selected, list, &mut |e: &mut Engine, i| f(Ui::of(e), i))
        }
        pub fn tree(&mut self, key: &str, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], f: &mut dyn FnMut(&mut Ui, usize)) -> TreeResponse {
            self.0.tree(key, nodes, open, selected, visible, &mut |e: &mut Engine, i| f(Ui::of(e), i))
        }
        pub fn table_view(&mut self, key: &str, titles: &[&str], widths: &mut [f32], rows: usize, selected: &mut Option<usize>, table: &TableView, f: &mut dyn FnMut(&mut Ui, usize, usize)) -> TableResponse {
            self.0.table_view(key, titles, widths, rows, selected, table, &mut |e: &mut Engine, r, c| f(Ui::of(e), r, c))
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
        if let (Some(v), _) = ui.value_field((x, y), w, 20.0, &ValueFieldOpts::default(), &|t: &str| t.parse::<f64>().is_ok(), "scale-value", scale) { *scale = v.to_owned(); }
    }
    pub fn notes(ui: &mut Ui, notes: &mut String) {
        if let (Some(text), _) = ui.text_area_submit(pxui_types::Node::DEFAULT.at(12.0, 4.0).w_px(270.0).h_px(56.0), &TextAreaOpts::default(), "notes-area", notes) { *notes = text.to_owned(); }
    }
    pub fn layer_search(ui: &mut Ui, search: &mut String) {
        if let Some(q) = ui.search_field("Find", "/", search, "Search layers") { *search = q.to_owned(); }
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
