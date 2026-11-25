//! The key-first calls of the text inputs: thin generic shims over the one non-generic body each (`text_inputs.rs`, `widgets.rs`).
//!
//! Line and value fields take a rectangle in the parent's space. A text area takes its layout from a node.
use super::Key;
use crate::{AreaCallbacks, LineField, LineFieldResponse, SliderResponse, Scrubber, SearchField, SearchFieldResponse, TextArea, TextAreaResponse, TextField, TextFieldResponse, Ui, ValueCallbacks, ValueField, ValueFieldResponse};
use pxui_types::geom::Rect;

impl Ui {
    /// Stages `edit` for the field’s next replacement. With `typing`, consecutive typing at one caret can share one undo step.
    /// Recording a change clears redo. The history is bounded in bytes overall (`text_undo_bytes`).
    /// Allocates: a copy of the text to stage the edit.
    /// Identity: the key identifies the field whose undo history receives the snapshot.
    /// Panics: when the library refuses the call (a null `Ui`, or `edit.text` over the limit).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let field = ui.node("field", pxui::Node::DEFAULT);
    ///     let edit = ui.load_text_edit(pxui::Key::Resolved(field.box_key), "hello");
    ///     ui.remember(pxui::Key::Resolved(field.box_key), &edit, false);
    ///     assert!(ui.text_undo_bytes() > 0);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn remember<K: Into<Key>>(&mut self, key: K, edit: &pxui_types::ui::TextEdit, typing: bool) { self.remember_key(key.into(), edit, typing) }

    /// The edit the `Ui` keeps for the field under `key`, loaded with `text`: its caret, selection and undo history follow the key while the text is unchanged.
    /// When `key` or `text` differs from the last load, the previous edit's undo history is shelved (the three latest are kept) and this one starts at the end of `text`, or resumes a shelved history for the same key and text.
    /// Allocates: the returned `TextEdit` owns a copy of the text.
    /// Identity: the key resolves under the current seed. Use Key::Resolved for a live field's box key.
    /// Panics: when the library refuses the call (a null `Ui`, or `text` over the limit).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let field = ui.node("field", pxui::Node::DEFAULT);
    ///     let edit = ui.load_text_edit(pxui::Key::Resolved(field.box_key), "hello");
    ///     assert_eq!((edit.text.as_str(), edit.caret, edit.anchor), ("hello", 5, 5));
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn load_text_edit<K: Into<Key>>(&mut self, key: K, text: &str) -> pxui_types::ui::TextEdit { self.load_edit_key(key.into(), text) }

    /// A single-line text field keyed by `key`: what happened to its row, and the new text when the user changed it.
    ///
    /// `value` is what the field shows; edit it from `changed`. `field` carries the native keyboard traits, the accessible name (the field is
    /// unnamed without one) and the body variant a registered text field body reads. The text is borrowed from the library, valid until the
    /// next call on `ui`.
    /// Allocates: nothing once warm; typing allocates the new text.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the field's text or name.
    /// Panics: when the library refuses the call (outside a frame, text too long). A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
    ///     const FIND: pxui::TextField = pxui::TextField::SEARCH.action(pxui::ReturnKey::Done).name("Find songs");
    ///     let field = ui.text_field("find", "abc", &FIND);
    ///     assert!(field.changed.is_none() && !field.response.focused);
    /// }).unwrap();
    /// ```
    #[inline]
    #[track_caller]
    pub fn text_field<K: Into<Key>>(&mut self, key: K, value: &str, field: &TextField<'_>) -> TextFieldResponse<'_> { self.text_field_key(key.into(), value, field) }

    /// A multiline text area laid out by `node`: its response, the new text when it changed (else none) and whether it was submitted. The
    /// closures in `callbacks` run during this call on this thread and are not kept; the text is borrowed from the library, valid until
    /// the next call on `ui`.
    ///
    /// Placement: `node` is one flow node under the current parent (size, padding and placement as any `Node`; an absolute position is
    /// `Node::at`), and the editor fills it; the area adds its input flags and row scroll step. The wrap width is the node's `Px` width,
    /// else the area's width from the last layout (a new flow-sized area wraps from its second frame).
    /// Allocates: nothing on a warm frame with the same `area` data, callbacks and language (the adapters are made once per `Ui` and rebound each call);
    /// results a callback returns (colours, completions, text) and typing allocate.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the area's text.
    /// Panics: when the `Ui` has failed, like every `Ui` call, when the library refuses `node` (a grid or an invalid transition), when an
    /// offset in `area` is over `i64::MAX`, or when `area` holds more than 64 errors, notes, spans, splices or chips, or the language more
    /// than 32 bracket pairs (they cross in stack buffers). A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// use pxui::{AreaCallbacks, Node, TextArea};
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
    ///     let area = ui.text_area("area", "hello", Node::column().w_px(100.0).h_px(60.0), &TextArea::DEFAULT, &AreaCallbacks::DEFAULT);
    ///     assert_eq!((area.changed, area.submitted), (None, false));
    /// }).unwrap();
    /// ```
    #[inline]
    #[track_caller]
    pub fn text_area<K: Into<Key>>(&mut self, key: K, value: &str, node: crate::Node, area: &TextArea<'_>, callbacks: &AreaCallbacks<'_>) -> TextAreaResponse<'_> { self.text_area_node(key.into(), value, node, area, callbacks) }

    /// A search field keyed by `key`: its response and the text the user changed the value to, borrowed from the library and valid until the
    /// next call on `ui`.
    ///
    /// Allocates: nothing once warm; the changed text is copied into buffers the library reuses.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the field's text.
    /// Panics: when the `Ui` has failed, like every `Ui` call. A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
    ///     const FIND: pxui::SearchField = pxui::SearchField::DEFAULT.icon("?").placeholder("Search");
    ///     assert_eq!(ui.search_field("find", "", &FIND).changed, None);
    /// }).unwrap();
    /// ```
    #[inline]
    #[track_caller]
    pub fn search_field<K: Into<Key>>(&mut self, key: K, value: &str, field: &SearchField<'_>) -> SearchFieldResponse<'_> { self.search_field_key(key.into(), value, field) }

    /// Behavior for a caller-painted scrubber. Pointer capture previews until release; routed arrows/Page/Home/End and accessibility
    /// adjustments take over capture and commit. A collapsed range is read-only. Use `set_node_layout` to size the returned node and `draw`
    /// to paint its body.
    ///
    /// `scrubber` carries the arrow-key step and the accessible name (the scrubber is unnamed without one). A held drag supplies
    /// `preview`; a release, key or accessibility edit supplies `commit` with `preview` set to the same value, so a caller painting from
    /// `preview` shows the committed position until its seek takes effect.
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never display text.
    /// Panics: when the library refuses the call (outside a frame, a non-finite value or step). A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
    ///     let scrub = ui.scrub("position", 5.0, 0.0..=10.0, &pxui::Scrubber::DEFAULT.step(1.0));
    ///     assert!(scrub.preview.is_none() && scrub.commit.is_none() && !scrub.response.held);
    /// }).unwrap();
    /// ```
    #[inline]
    #[track_caller]
    pub fn scrub<K: Into<Key>>(&mut self, key: K, value: f32, range: core::ops::RangeInclusive<f32>, scrubber: &Scrubber<'_>) -> SliderResponse { self.scrub_key(key.into(), value, range, scrubber) }
}

// The extension set (`pxui::ext`): free functions on a `Ui`, out of the prelude, with no stability promise.
/// A one-line field at `rect` with a hairline: its response, the new value when it differs from `value`, and what its keys asked (Enter or
/// Down next, Shift-Enter or Up previous, Escape close). The text is borrowed from the library, valid until the next call on `ui`.
///
/// Placement: absolute, at `rect` in the parent's space (the caller-painted inspector field; no flow node).
/// Allocates: the engine allocates the result text each call; the wrapper nothing.
/// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the field's text.
/// Panics: when the `Ui` has failed, like every `Ui` call. A key or identity label over 256 bytes panics (a programmer error).
///
/// ```
/// use pxui::{ui::{LineStep, Rect}, LineField};
/// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
/// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
///     const LINE: LineField = LineField::DEFAULT.placeholder("line");
///     let line = pxui::ext::line_field(ui, "find", "abc", Rect::new(0.0, 0.0, 120.0, 20.0), &LINE);
///     assert_eq!((line.changed, line.step), (None, LineStep::None));
/// }).unwrap();
/// ```
#[inline]
#[track_caller]
pub fn line_field<'a, K: Into<Key>>(ui: &'a mut Ui, key: K, value: &str, rect: Rect, field: &LineField<'_>) -> LineFieldResponse<'a> { ui.line_field_key(key.into(), value, rect, field) }
/// A compact field at `rect`: its response, the new text when it changed (else none) and whether a text editor is open. Only text the
/// `callbacks` accept commits. The text is borrowed from the library, valid until the next call on `ui`.
/// A changed value is shown immediately. The field requests a frame to display the value supplied by the caller, with
/// kept drags deferring that request until release.
///
/// Placement: absolute, at `rect` in the parent's space (the caller-painted inspector value; no flow node).
/// Allocates: nothing on an idle warm frame; editing, sliding or scrubbing allocates the text.
/// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the field's text.
/// Panics: when the `Ui` has failed, like every `Ui` call, or when a size in `field` is over `i64::MAX`. A key or identity label over 256 bytes panics (a programmer error).
///
/// ```
/// use pxui::{ui::Rect, ValueCallbacks, ValueField};
/// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
/// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
///     let accept = |t: &str| t.parse::<f32>().is_ok();
///     let field = pxui::ext::value_field(ui, "amount", "0.25", Rect::new(0.0, 0.0, 100.0, 20.0), &ValueField::DEFAULT, &ValueCallbacks::DEFAULT.valid(&accept));
///     assert_eq!((field.changed, field.editing), (None, false));
/// }).unwrap();
/// ```
#[inline]
#[track_caller]
pub fn value_field<'a, K: Into<Key>>(ui: &'a mut Ui, key: K, value: &str, rect: Rect, field: &ValueField<'_>, callbacks: &ValueCallbacks<'_>) -> ValueFieldResponse<'a> { ui.value_field_key(key.into(), value, rect, field, callbacks) }
