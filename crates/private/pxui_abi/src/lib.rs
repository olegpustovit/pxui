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

#[cfg(test)]
mod safety;
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
pub const ABI_VERSION: u32 = 65547; // major 1 (high 16 bits), minor 11: borrowed carry/drop queries and retained payload buffers
