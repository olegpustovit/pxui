//! The authoring layer: thin generic shims over one non-generic body each, so a call site costs one call and the body is compiled once.
mod layout;
mod text;
pub use layout::Key;
mod view;
pub use text::text_style;
pub use view::hash_of;
