//! The gallery pages compiled against the engine directly: the page source is the gallery's own, and the `pxui` module below names
//! the engine's types where the wrapper names its own. A tool for the A7 bench, so it may name the private crates.
pub mod pxui {
    pub use pxui_core::opts::{PickerOpts, TextAreaOpts, ValueFieldOpts};
    pub use pxui_core::text;
    pub use pxui_core::ui::*;
    pub use pxui_core::ui_kit::PanelOpts;
    pub use pxui_core::ui_widgets::{DialogOut, TreeNode};
    pub use pxui_types::ui::{MenuOpts, MenuResult, Pick};
    pub mod collections { pub use pxui_core::ui_widgets::TreeNode; }
    pub mod ui_kit { pub use pxui_core::ui_kit::PanelOpts; }
    pub use pxui_types::theme;
    pub type PxuiBox = pxui_core::ui::UiBox;
}

/// The engine's shapes of the two calls the pages route through `compat`.
mod compat {
    use super::pxui::Ui;
    use pxui_core::ui::UiBox;
    use pxui_types::{theme::Curve, ui::Target};

    pub fn table(ui: &mut Ui, at: (f32, f32), w: f32, h: f32, headers: &[String], rows: i64, cell: &mut dyn FnMut(i64, i64) -> String, label: &str) -> Option<i64> {
        // the engine's `table` takes an Rc closure; `table_with` borrows like the wrapper's does
        ui.table_with(at, w, h, &headers.iter().map(String::as_str).collect::<Vec<_>>(), rows, &mut |r, c, emit| emit(&cell(r, c)), label).0
    }

    pub fn bounce(ui: &mut Ui, c: &UiBox) {
        let ease = Curve::Bezier { duration: 0.0, points: [0.25, 0.1, 0.25, 1.0] };
        let keys = [
            (0.2, Target::Offset(120.0, 0.0), ease), (0.35, Target::Opacity(0.3), ease),
            (0.6, Target::Offset(0.0, 0.0), Curve::Spring { omega: 18.0 }), (0.7, Target::Opacity(1.0), ease),
        ];
        let _ = ui.animate_keys(c, &keys);
    }
}

pub mod gallery {
    use super::pxui;
    include!("../../pxui_gallery/src/pages.rs");
}
