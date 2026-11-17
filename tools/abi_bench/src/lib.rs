//! The gallery pages compiled against the engine directly: the page source is the gallery's own, and the `pxui` module below names
//! the engine's types where the wrapper names its own. A tool for the A7 bench, so it may name the private crates.
pub mod pxui {
    pub use pxui_core::opts::{PickerOpts, TextAreaOpts, ValueFieldOpts};
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

pub mod gallery {
    use super::pxui;
    include!("../../pxui_gallery/src/pages.rs");
}
