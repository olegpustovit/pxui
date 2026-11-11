//! The PXUI kit gallery: every kit widget, old and new, on four pages, each page a cached view.
//! The page functions are shared by the window app (`main.rs`) and the headless goldens
//! (`tests/gpu_gallery.rs`); the host loop and the GPU half are in `main.rs` and `gpu.rs`.
mod compat;
pub mod gpu;
mod pages;
pub use pages::*;
