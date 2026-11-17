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
