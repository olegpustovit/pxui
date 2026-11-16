//! The authoring layer: thin generic shims over one non-generic body each, so a call site costs one call and the body is compiled once.
mod layout;
mod text;
pub use layout::Key;
pub use text::text_style;
