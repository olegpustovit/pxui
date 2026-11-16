//! The authoring layer: thin generic shims over one non-generic body each, so a call site costs one call and the body is compiled once.
mod button;
mod icon_button;
mod layout;
mod menu;
mod press;
mod state;
mod text;
mod toggle;
mod view;
pub use button::Button;
pub use icon_button::{Icon, IconButton};
pub use layout::Key;
pub use press::Response;
pub use toggle::{Segmented, Toggle};
pub use view::hash_of;
