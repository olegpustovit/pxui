//! The authoring layer: thin generic shims over one non-generic body each, so a call site costs one call and the body is compiled once.
mod button;
mod collection;
mod dialog;
mod field_types;
pub(crate) mod fields;
mod icon_button;
mod kit;
mod layout;
mod menu;
mod overlay;
mod press;
mod results;
#[cfg(test)]
mod state;
mod text;
mod toggle;
mod view;
pub use button::Button;
pub use field_types::{AreaCallbacks, AreaNote, BracketsFn, CompleteFn, ColorizeFn, ContextAtFn, DescribeFn, IndentFn, Language, LineField, LineFieldResponse, NumberAtFn, Picker, Reveal, RewriteFn, ScrubFn, Scrubber, SearchField, SearchFieldResponse, SlideFn, Splice, TextArea, TextAreaResponse, TextField, TextFieldResponse, ValidFn, ValueCallbacks, ValueField, ValueFieldResponse};
pub use icon_button::{Icon, IconButton};
pub use layout::Key;
pub use press::{Pressable, Response};
pub use pxui_types::ui::Segmented;
pub use toggle::Toggle;
pub use kit::{Badge, Progress, Radio, Spinner};
pub use collection::GridList;
pub use results::{OverlayResponse, PickResponse, SliderResponse, SplitResponse};
pub use view::hash_of;
