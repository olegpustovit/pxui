//! The authoring layer: thin generic shims over one non-generic body each, so a call site costs one call and the body is compiled once.
mod layout;
mod press;
mod text;
mod view;
pub use layout::Key;
pub use press::Response;
pub use text::text_style;
pub use view::hash_of;
