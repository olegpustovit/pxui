#![allow(clippy::all)]

#[macro_use]
mod macros;
mod state;
mod bodies;
/// The entries the static wrapper calls without the C checks (see the module).
pub mod trusted;
mod ui;
mod text;
mod node;
mod paint;
mod widgets;
mod composite;
mod theme;
mod overlays;
mod collections;
mod image;
mod text_inputs;
mod kit;
mod a11y;
mod a11y_state;
mod style;
mod text_width;
mod interaction;
mod ui_state;
mod renderer;
mod box_ops;
mod kit_ext;
/// The engine behind a wrapper `Ui` (its `id()`), for the checks that compare the wrapper's drawing with the engine's own. Not part of
/// the C ABI and not exported; the pointer is valid while the `Ui` lives.
#[doc(hidden)]
pub fn engine_of(id: usize) -> *const pxui_core::ui::Ui { unsafe { (*(id as *const state::PxuiUi)).engine } }
/// Poisons the `Ui` behind a wrapper `Ui` (its `id()`) as a panic the export macro caught would, for the wrapper's checks of a poisoned
/// `Ui`: no argument causes one any more (`fuzz.rs`). Not part of the C ABI and not exported.
#[doc(hidden)]
pub fn poison_of(id: usize) { unsafe { state::panicked((*(id as *const state::PxuiUi)).shared, Box::new("injected"), false) } }

#[cfg(test)]
mod safety;
#[cfg(test)]
mod fuzz;
pub use text::*;
pub use node::*;
pub use paint::*;
pub use widgets::*;
pub use composite::*;
pub use theme::*;
pub use overlays::*;
pub use collections::*;
pub use image::*;
pub use text_inputs::*;
pub use kit::*;
pub use a11y::*;
pub use a11y_state::*;
pub use style::*;
pub use text_width::*;
pub use interaction::*;
pub use ui_state::*;
pub use renderer::*;
pub use box_ops::*;
pub use kit_ext::*;
pub use ui::*;
pub use bodies::*;
pub use state::{Build, PxuiUi};
pub(crate) use state::{core_box, fail, get, text};

pub use pxui_types::{FrameIn, FrameOut, PxuiBox, PxuiPanel, PxuiTargetDesc, PxuiUiDesc, Str};
pub use pxui_types::ffi::{PxuiSaved, PxuiSignal};
pub use pxui_types::{PXUI_ERR_ARGUMENT, PXUI_ERR_GPU, PXUI_ERR_PANIC, PXUI_ERR_STALE, PXUI_ERR_STATE};
pub use pxui_types::ffi::PxuiDialogOut;
pub const ABI_VERSION: u32 = 65607; // major 1 (high 16 bits), minor 71 (provisional): `PxuiButtonDesc` leading icon/detail/accessory (192 -> 424), `PxuiButtonConfig` the same (232 -> 464), `PxuiIconButtonDesc.checkable`/`checked` (104 -> 112), `PxuiDialogDesc.details` (88 -> 104), `PxuiDialogConfig.details` (88 -> 104), `PXUI_FEATURE_BUTTON_PARTS`; minor 70 (provisional): `ui_image_node` (an image with no key, its name accessibility only), `paint_image_checked` (reports a refused image), `PXUI_FEATURE_IMAGE_CHECKED` (27); minor 69 (provisional): `PxuiSegmentedDesc.focus_on_click` (104 -> 112), `PxuiSegmentConfig.stored`/`has_stored` (184 -> 200), `PXUI_FEATURE_SEGMENTED_STORED_FOCUS`; minor 68 (provisional): `PxuiKeyedDesc` and the `ui_*_keyed` calls (checkbox, choice, slider, progress, spinner, badge, panel, popup, modal, anchored, scroll_view, toasts, table_view, picker); minor 67: `PxuiMenuExtra.max_h`/`reserved4` (184 -> 192), `PxuiSegmentedDesc`/`PxuiSegmentConfig.has_selected` (96 -> 104, 176 -> 184), `ui_segmented_result` and `PxuiSegmentedResult` (no choice; the option a secondary press began on), segmented controls are radio groups (every option focusable to assistive technology, one Tab stop: the chosen option), `PXUI_FEATURE_MENU_MAX_H`/`SEGMENTED_RESULT`; minor 66: per-role string caps, event/frame/surface limits, builders refused outside a frame (PXUI_ERR_STATE), bounded returned strings, poisoned shapers/themes/bridges, `ui_calls` (per-Ui call count; `counter(0)` reads 0), shapers without a font refused; minor 65: `ui_label_text_spans`, `ui_label_text_line_box`, `ui_label_measure`, `ui_label_quads`, `ui_label_rasterize_rgba`, `ui_label_prewarm`, `PXUI_FEATURE_LABEL_TEXT` (22) (text calls on a label style, resolved in the same call); minor 64: `ui_text_field_keyed`, `ui_input_box`, `PxuiTextFieldDesc`; minor 63: font adds are capped and refuse unparseable fonts, native-image identities are per `Ui`, renderer records are read after their size, `pxui_renderer_resize` caps at the device limit; minor 62: `text_worker_create`, `text_worker_destroy`, `ui_set_text_worker`, `text_prewarm`, `PXUI_FEATURE_TEXT_WORKER` (20) (the prepare queue); minor 61: a look's colour may name a colour token (alpha 0, red 0x54, green 0x4b, blue the token; final), resolved where a look is applied; `ui_icon_word`, `paint_icon_word`; `PXUI_FEATURE_LOOK_TOKENS`/`ICON_WORD`; minor 60: `PxuiUiDesc.abi_built`, `pxui_supports` and the `PXUI_FEATURE_*` ids, recorded sizes for grown input records; minor 59: `PXUI_RESPONSE_SECONDARY` (a secondary press began on the box); minor 58: `PxuiMenuExtra.focus_on_click`/`ids`/`has_id`, `PxuiToggleConfig.stored`/`clicked`, `PxuiSegmentConfig.line`/`strip`; minor 57: `FrameIn.surface` (reserved, ignored); minor 56: `ui_button_content`, `PxuiButtonBuild`, `PxuiButtonDesc`/`PxuiButtonConfig` grow by the text's label style, the contents' axis, gap and place (and the desc by `focus_on_click`); minor 55: `PxuiMenuExtra.padding`/`name`/`selected`, `PxuiToggleDesc`/`PxuiToggleConfig` title style, `PxuiSegmentedDesc.w`/`padding`/`min_segment`/`names`, `PxuiSegmentConfig.padding`/`min_segment`; minor 54: `pxui_ui_label_node`, `PxuiLabelBox`, `PxuiLabelConfig.label_box`, `PxuiButtonDesc`/`PxuiButtonConfig.centered`, `PxuiIconButtonDesc`/`PxuiIconButtonConfig.press`; minor 53: `ui_dialog_response` (`PxuiDialogDesc`, `PxuiDialogResult`, `PxuiDialogConfig` grows); minor 52: `ui_variant`, `variant` ends every `Pxui*Config`; minor 51: `ui_toggle_plain`, `ui_toggle_response`, `ui_segmented_plain`, `ui_segmented_response`, `ui_disabled`, `PxuiToggleDesc`, `PxuiSegmentedDesc`, `PxuiToggleConfig` and `PxuiSegmentConfig` grow; before: minor 50: `PxuiMenuExtra.row_h`/`max_rows`/`checked` and `PxuiMenuRowConfig.row_h` (`Ui::menu`)
