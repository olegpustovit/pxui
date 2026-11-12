#![allow(clippy::all)]
//! PXUI owns immediate-mode engine, kit and text widgets over pxui_text; ui re-exports the shared palette, typography and widget API.

pub mod a11y;
mod arena;
pub mod bodies;
pub mod layout;
pub mod theme;
pub mod ui_core;
pub mod ui_kit;
pub mod ui_text;
pub mod ui_widgets;
pub mod opts;
pub mod ui;

pub use theme::Theme;
pub use pxui_text as text;
pub use pxui_types::{draw, geom};
