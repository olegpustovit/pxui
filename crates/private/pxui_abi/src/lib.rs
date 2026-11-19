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
pub use pxui_types::ffi::PxuiSignal;
pub use pxui_types::{PXUI_ERR_ARGUMENT, PXUI_ERR_GPU, PXUI_ERR_PANIC, PXUI_ERR_STALE, PXUI_ERR_STATE};
pub const ABI_VERSION: u32 = 65644; // major 1 (high 16 bits), minor 108 (D28, U4a.6 fix): `ui_style` takes the one look the state picks, not six; minor 107 (D28, U2b.4.14): a kept list row past the viewport (culled) keeps its build until it scrolls into view, a reorderable list's too; minor 106 (D28, U2b.4.13): `PxuiSelectionListDesc.has_revision`/`revision` (216 -> 224) and `PxuiTableDesc.has_revision`/`revision` (96 -> 104), kept list rows, the row flag 4 (live); minor 105 (D28, U2b.4.12): `ui_reordering`, a list row lifted by its handle; minor 104 (D28, U2b.4.11): `PxuiListResult.focused` (in its reserved bytes), the list holds keyboard focus; minor 103 (D28, U2b.4.10): `ui_section_list`'s build is a `PxuiListCell` (a row's box and content Y; `PxuiSectionBuild` deleted); minor 102 (D28, U2b.4.8): `PxuiSearchFieldConfig.line` (88 -> 120), the field's line a body sizes; minor 101 (D28, U2b.4.6): `PxuiIconButtonConfig.built_in` (144 -> 152), the kit's built-in icon button body as a slot a registered body runs; minor 100 (D28, U2b.4.3): `PxuiSelectionListDesc.reserved` is `bare` (a list with no frame look and no inset); minor 99 (D28, U2b.4.2): `PxuiButtonConfig.built_in` (472 -> 480), the kit's built-in button body as a slot a registered body runs; minor 98 (D28, U2c review): a refused call writes none of its outputs (`ui_search_field`, `ui_line_field`, `ui_value_field`, `ui_text_area`, `ui_split`, `ui_modal`, `ui_popup`, `ui_toasts`, `ui_inspector_section`, `ui_inspector_toggle`, `ui_inspector_button`), `ui_section_index_result` renamed `ui_section_index`; minor 97 (D28, U2c): the resolved-style text twins (`text_width`, `_spans`, `text_quads`, `text_rasterize_rgba`, `text_prewarm`, `ui_text_spans`, `ui_text_line_box`; the `ui_label_*` calls are the calls), the integer-size paint text calls (`paint_text*`, `paint_cap*`), `ui_text_width`, `ui_cap_width`, `ui_text_top` and `ui_label` (no box; `ui_label_node`) deleted; minor 96 (D28, U2c): the 1.x theme, unit, constructor and setter twins deleted, each successor under the plain name (`ui_theme` the exact snapshot, `paint_image`, `renderer_create`, `body_register`, `ui_a11y_name`, `ui_a11y_number`); the integer theme projection, `ui_set_theme` (`PxuiThemeSource`), `ui_set_colors`, `theme_set`, `theme_load`, `ui_theme_apply`, `ui_set_look`, `ui_font_size`, `ui_row_height`, `ui_set_font_size` and `verify_new_views` deleted; minor 95 (D28, U2c): the 1.x text input twins deleted, each successor under the plain name (`ui_text_field`, `ui_search_field`, `ui_line_field`, `ui_value_field`, `ui_text_area`, `ui_remember`); `text_field`, `text_field_with_options`, `ui_text_field_keyed`, `ui_text_area_submit`, `ui_text_area_node` and `ui_input_box` deleted; minor 94 (D28, U2c): the 1.x container and overlay twins deleted, each successor under the plain name (`ui_panel`, `ui_popup`, `ui_modal`, `ui_anchored`, `ui_scroll_view`, `ui_view`, `ui_split`, `ui_menu_bar`, `ui_image`); `pxui_col`, `ui_box` (`PxuiBoxDesc`), `ui_enter`/`ui_leave` (`PxuiSaved`), `ui_list_measured`, `ui_tooltip` and ext `ui_inspector_section_view`/`_toggle_value` deleted; minor 93 (D28, U2c): the 1.x widget twins deleted, each successor under the plain name (`ui_checkbox`, `ui_segmented`, `ui_tabs`, `ui_slider`, `ui_scrub`, `ui_progress`, `ui_spinner`, `ui_badge`, `ui_toasts`, `ui_picker`, `ui_dialog`, `ui_tree`, `ui_button`, `ui_icon_button`, `ui_toggle`, `ui_key_pressed`), `PxuiDialogOut` and the text-callback tree record deleted (`PxuiTreeViewWithDesc` is `PxuiTreeViewDesc`); minor 92 (D28: changed in place): a `PxuiUi` has a public head, `theme` (the handle's theme in effect, filled by a theme read, read in place of the call); minor 91 (D28: changed in place): `PxuiButtonDesc.icon_size` (424 -> 432), `PxuiButtonConfig.lead_word`/`icon_size` (464 -> 472), `PxuiIconButtonConfig.draw_word` (136 -> 144), `ui_search_field_response` takes the return key, colour tokens carry an opacity (D19) and `ColorToken::ACCENT` (64); minor 90 (D28: changed in place): `PxuiSheetDesc.top`/`max_h` (20 -> 28), present 3 (top-anchored modal; `PxuiPanelConfig.present` 3); minor 89 (D28: changed in place): reorderable rows, `PxuiListResult.moved_from`/`moved_to` (112 -> 128; `PxuiTableResult` 176 -> 192), `A11yAction` codes 128/256 (move up/down), `pxui_part` builds `Part::HANDLE` in a reorderable row outside a body; minor 88 (D28: changed in place): drop zones on any node, `PxuiSectionListDesc.reorder`, `PxuiSelectionListDesc.reorder`/`has_drop_look`/`drop_look` (64 -> 216), `PxuiListResult.drop_at` (104 -> 112; `PxuiTableResult` 168 -> 176), `PxuiRowConfig.drop` (in its padding); minor 87 (D28: changed in place): `ui_selection_list`, `ui_table_head` and `PxuiCellState`, `PxuiSelectionListDesc`, `PxuiListResult`, `PxuiColumn`, `PxuiColumnWidth`, `PxuiTableHeadDesc`, `PxuiHeadResult`, `PxuiTableResult` added; `ui_section_list` (keyed, builds its trailer) and `ui_table` (the column table, `PxuiTableDesc`) reshaped; `ui_list_view`/`_with`/`_rows`, `ui_table_view`/`_with`/`_keyed`/`_rows`, `ui_section_list_measured`, the text table and the `table_cell` body kind deleted (later body kinds renumbered); minor 86: `ui_modal_sheet` and `PxuiSheetDesc` (bottom and full-screen modals), `PxuiPickRow` picker fills (extension set: art and headings), `PxuiPanelConfig.present` (72 -> 80); minor 85: `ui_checkbox_response` and `PxuiCheckboxDesc`, `ui_section_index_result`, `ui_disclosure`, `PxuiCheckConfig.states`/`box_size`/`passive` (72 -> 88), the `section_index` body kind and `PxuiSectionIndexConfig` (`PxuiThemeSnapshot` 56888 -> 56896); minor 84: `ui_tabs_result` (tabs of pick-one items: ids, details, none in use, focus on press), `PxuiTabConfig.detail` (80 -> 96); minor 83: `ui_list_view_rows`, `ui_table_view_rows` (rows keyed by item id); minor 82: text input `_response` exports; minor 81: ui_layout_node, positional container/label/image keys; minor 80: ui_scroll_view_node, ui_text_area_node; minor 79: PxuiFontDesc.size_points; minor 78: fractional font/row/padding queries and setter, exact theme, paint label measure and spans; minor 77: lookup key seed, keyed remember and image, resolved key labels; minor 76: pick-one items, slider/scrub phases with responses, overlay roots, split responses and menu-bar dismissal; minor 75: modifier-mask press query; minor 74: checked renderer/body creation and checked accessibility name/number; minor 73: detached theme source, palette, density and reduced motion; minor 72 (provisional): `ui_view_keyed` (the authoring view, sized by its parent), `PXUI_FEATURE_VIEW_KEYED`; minor 71 (provisional): `PxuiButtonDesc` leading icon/detail/accessory (192 -> 424), `PxuiButtonConfig` the same (232 -> 464), `PxuiIconButtonDesc.checkable`/`checked` (104 -> 112), `PxuiDialogDesc.details` (88 -> 104), `PxuiDialogConfig.details` (88 -> 104), `PXUI_FEATURE_BUTTON_PARTS`; minor 70 (provisional): `ui_image_node` (an image with no key, its name accessibility only), `paint_image_checked` (reports a refused image), `PXUI_FEATURE_IMAGE_CHECKED` (27); minor 69 (provisional): `PxuiSegmentedDesc.focus_on_click` (104 -> 112), `PxuiSegmentConfig.stored`/`has_stored` (184 -> 200), `PXUI_FEATURE_SEGMENTED_STORED_FOCUS`; minor 68 (provisional): `PxuiKeyedDesc` and the `ui_*_keyed` calls (checkbox, choice, slider, progress, spinner, badge, panel, popup, modal, anchored, scroll_view, toasts, table_view, picker); minor 67: `PxuiMenuExtra.max_h`/`reserved4` (184 -> 192), `PxuiSegmentedDesc`/`PxuiSegmentConfig.has_selected` (96 -> 104, 176 -> 184), `ui_segmented_result` and `PxuiSegmentedResult` (no choice; the option a secondary press began on), segmented controls are radio groups (every option focusable to assistive technology, one Tab stop: the chosen option), `PXUI_FEATURE_MENU_MAX_H`/`SEGMENTED_RESULT`; minor 66: per-role string caps, event/frame/surface limits, builders refused outside a frame (PXUI_ERR_STATE), bounded returned strings, poisoned shapers/themes/bridges, `ui_calls` (per-Ui call count; `counter(0)` reads 0), shapers without a font refused; minor 65: `ui_label_text_spans`, `ui_label_text_line_box`, `ui_label_measure`, `ui_label_quads`, `ui_label_rasterize_rgba`, `ui_label_prewarm`, `PXUI_FEATURE_LABEL_TEXT` (22) (text calls on a label style, resolved in the same call); minor 64: `ui_text_field_keyed`, `ui_input_box`, `PxuiTextFieldDesc`; minor 63: font adds are capped and refuse unparseable fonts, native-image identities are per `Ui`, renderer records are read after their size, `pxui_renderer_resize` caps at the device limit; minor 62: `text_worker_create`, `text_worker_destroy`, `ui_set_text_worker`, `text_prewarm`, `PXUI_FEATURE_TEXT_WORKER` (20) (the prepare queue); minor 61: a look's colour may name a colour token (alpha 0, red 0x54, green 0x4b, blue the token; final), resolved where a look is applied; `ui_icon_word`, `paint_icon_word`; `PXUI_FEATURE_LOOK_TOKENS`/`ICON_WORD`; minor 60: `PxuiUiDesc.abi_built`, `pxui_supports` and the `PXUI_FEATURE_*` ids, recorded sizes for grown input records; minor 59: `PXUI_RESPONSE_SECONDARY` (a secondary press began on the box); minor 58: `PxuiMenuExtra.focus_on_click`/`ids`/`has_id`, `PxuiToggleConfig.stored`/`clicked`, `PxuiSegmentConfig.line`/`strip`; minor 57: `FrameIn.surface` (reserved, ignored); minor 56: `ui_button_content`, `PxuiButtonBuild`, `PxuiButtonDesc`/`PxuiButtonConfig` grow by the text's label style, the contents' axis, gap and place (and the desc by `focus_on_click`); minor 55: `PxuiMenuExtra.padding`/`name`/`selected`, `PxuiToggleDesc`/`PxuiToggleConfig` title style, `PxuiSegmentedDesc.w`/`padding`/`min_segment`/`names`, `PxuiSegmentConfig.padding`/`min_segment`; minor 54: `pxui_ui_label_node`, `PxuiLabelBox`, `PxuiLabelConfig.label_box`, `PxuiButtonDesc`/`PxuiButtonConfig.centered`, `PxuiIconButtonDesc`/`PxuiIconButtonConfig.press`; minor 53: `ui_dialog_response` (`PxuiDialogDesc`, `PxuiDialogResult`, `PxuiDialogConfig` grows); minor 52: `ui_variant`, `variant` ends every `Pxui*Config`; minor 51: `ui_toggle_plain`, `ui_toggle_response`, `ui_segmented_plain`, `ui_segmented_response`, `ui_disabled`, `PxuiToggleDesc`, `PxuiSegmentedDesc`, `PxuiToggleConfig` and `PxuiSegmentConfig` grow; before: minor 50: `PxuiMenuExtra.row_h`/`max_rows`/`checked` and `PxuiMenuRowConfig.row_h` (`Ui::menu`)
