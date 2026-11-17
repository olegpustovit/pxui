//! The gallery pages compiled against the engine directly: the page source is the gallery's own, and the `pxui` module below names
//! the engine's types where the wrapper names its own. The text inputs go through `inputs`, which the gallery implements over the wrapper and
//! this tool over the engine. A tool for the A7 bench, so it may name the private crates.
pub mod pxui {
    pub use pxui_core::text;
    pub use pxui_core::ui::*;
    pub use pxui_core::ui_kit::PanelOpts;
    pub use pxui_core::ui_widgets::TreeNode;
    pub use pxui_types::ui::{Dialog, DialogButton, DialogResponse, Menu, MenuItem, MenuResponse, Pick};
    pub mod collections { pub use pxui_core::ui_widgets::TreeNode; }
    pub mod ui_kit { pub use pxui_core::ui_kit::PanelOpts; }
    pub use pxui_types::theme;
    pub type PxuiBox = pxui_core::ui::UiBox;
}

/// The gallery's text inputs over the engine (the wrapper's version is `pxui_gallery::inputs`).
pub mod inputs {
    use super::pxui::{Pick, Ui};
    use pxui_core::opts::{PickerOpts, TextAreaOpts, ValueFieldOpts};

    pub fn caption(ui: &mut Ui, caption: &mut String) {
        if let Some(v) = ui.text_field("Caption", caption) { *caption = v; }
    }
    pub fn scale(ui: &mut Ui, (x, y, w): (f32, f32, f32), scale: &mut String) {
        if let (Some(v), _) = ui.value_field((x, y), w, 20.0, &ValueFieldOpts::default(), &|t: &str| t.parse::<f64>().is_ok(), "scale-value", scale) { *scale = v.to_owned(); }
    }
    pub fn notes(ui: &mut Ui, notes: &mut String) {
        if let (Some(text), _) = ui.text_area_submit((12.0, 4.0), 270.0, 56.0, &TextAreaOpts::default(), "notes-area", notes) { *notes = text.to_owned(); }
    }
    pub fn layer_search(ui: &mut Ui, search: &mut String) {
        if let Some(q) = ui.search_field("Find", "/", search, "Search layers") { *search = q.to_owned(); }
    }
    pub fn presets(ui: &mut Ui, rows: &dyn Fn(&str) -> Vec<(String, String)>) -> Pick {
        ui.picker(&PickerOpts { limit: 4, ..PickerOpts::default() }, "Search presets", "", rows).1
    }
}

pub mod gallery {
    use super::pxui;
    include!("../../pxui_gallery/src/pages.rs");
}
