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
pub(crate) use state::{fail, get, text};

pub use pxui_types::{FrameIn, FrameOut, PxuiBox, PxuiPanel, PxuiTargetDesc, PxuiUiDesc, Str};
pub use pxui_types::ffi::PxuiSignal;
pub use pxui_types::{PXUI_ERR_ARGUMENT, PXUI_ERR_GPU, PXUI_ERR_PANIC, PXUI_ERR_STALE, PXUI_ERR_STATE, PXUI_ERR_VERSION};
pub const ABI_VERSION: u32 = 131081; // 2.9 (major << 16 | minor), kept in step with ABI_BUILT
