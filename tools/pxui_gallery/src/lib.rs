//! The PXUI kit gallery: every kit widget, old and new, on four pages, each page a cached view.
//! The page functions are shared by the window app (`main.rs`) and the headless goldens
//! (`tests/gpu_gallery.rs`); the host loop and the GPU half are in `main.rs` and `gpu.rs`.
mod compat;
pub mod bodies_page;
pub mod gpu;
mod pages;

/// A value field's new text, when it changed (the wrapper borrows it; `abi_bench` compiles `pages.rs` against the engine's owned form).
fn take_value(changed: Option<&str>, value: &mut String) { if let Some(text) = changed { *value = text.to_owned(); } }
pub use pages::*;
