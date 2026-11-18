//! The key-first calls of the text inputs: thin generic shims over the one non-generic body each (`text_inputs.rs`, `widgets.rs`).
//!
//! `line_field`, `value_field` and `text_area` place their field absolutely, at a `Rect` in the parent's space: they are the caller-painted
//! inspector widgets (a field in a row the caller laid out), and the library lays no flow node out for them.
use super::Key;
use crate::{AreaCallbacks, LineField, LineFieldResponse, Scrub, Scrubber, SearchField, SearchFieldResponse, TextArea, TextAreaResponse, TextField, TextFieldResponse, Ui, ValueCallbacks, ValueField, ValueFieldResponse};
use pxui_types::geom::Rect;

impl Ui {
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
    ///     let line = ui.line_field("find", "abc", Rect::new(0.0, 0.0, 120.0, 20.0), &LINE);
    ///     assert_eq!((line.changed, line.step), (None, LineStep::None));
    /// }).unwrap();
    /// ```
    #[inline]
    #[track_caller]
    pub fn line_field<K: Into<Key>>(&mut self, key: K, value: &str, rect: Rect, field: &LineField<'_>) -> LineFieldResponse<'_> { self.line_field_key(key.into(), value, rect, field) }

    /// A multiline text area at `rect`: its response, the new text when it changed (else none) and whether it was submitted. The closures in
    /// `callbacks` run during this call on this thread and are not kept; the text is borrowed from the library, valid until the next call on `ui`.
    ///
    /// Placement: absolute, at `rect` in the parent's space (the caller-painted code editor pane; no flow node).
    /// Allocates: nothing on a warm frame with the same `area` data, callbacks and language (the adapters are made once per `Ui` and rebound each call);
    /// results a callback returns (colours, completions, text) and typing allocate.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never the area's text.
    /// Panics: when the `Ui` has failed, like every `Ui` call, when an offset in `area` is over `i64::MAX`, or when `area` holds more than 64 errors, A key or identity label over 256 bytes panics (a programmer error).
    /// notes, spans, splices or chips, or the language more than 32 bracket pairs (they cross in stack buffers).
    ///
    /// ```
    /// use pxui::{ui::Rect, AreaCallbacks, TextArea};
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
    ///     let area = ui.text_area("area", "hello", Rect::new(0.0, 0.0, 100.0, 60.0), &TextArea::DEFAULT, &AreaCallbacks::DEFAULT);
    ///     assert_eq!((area.changed, area.submitted), (None, false));
    /// }).unwrap();
    /// ```
    #[inline]
    #[track_caller]
    pub fn text_area<K: Into<Key>>(&mut self, key: K, value: &str, rect: Rect, area: &TextArea<'_>, callbacks: &AreaCallbacks<'_>) -> TextAreaResponse<'_> { self.text_area_key(key.into(), value, rect, area, callbacks) }

    /// A compact field at `rect`: its response, the new text when it changed (else none) and whether a text editor is open. Only text the
    /// `callbacks` accept commits. The text is borrowed from the library, valid until the next call on `ui`.
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
    ///     let field = ui.value_field("amount", "0.25", Rect::new(0.0, 0.0, 100.0, 20.0), &ValueField::DEFAULT, &ValueCallbacks::DEFAULT.valid(&accept));
    ///     assert_eq!((field.changed, field.editing), (None, false));
    /// }).unwrap();
    /// ```
    #[inline]
    #[track_caller]
    pub fn value_field<K: Into<Key>>(&mut self, key: K, value: &str, rect: Rect, field: &ValueField<'_>, callbacks: &ValueCallbacks<'_>) -> ValueFieldResponse<'_> { self.value_field_key(key.into(), value, rect, field, callbacks) }

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
    /// `scrubber` carries the arrow-key step and the accessible name (the scrubber is unnamed without one).
    /// Allocates: never on a warm frame.
    /// Identity: `key` is unique among its siblings, resolved under the parent's seed (see `Key`); never display text.
    /// Panics: when the library refuses the call (outside a frame, a non-finite value or step). A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
    ///     let scrub = ui.scrub("position", 5.0, 0.0..=10.0, &pxui::Scrubber::DEFAULT.step(1.0));
    ///     assert!(scrub.preview.is_none() && scrub.commit.is_none() && !scrub.held);
    /// }).unwrap();
    /// ```
    #[inline]
    #[track_caller]
    pub fn scrub<K: Into<Key>>(&mut self, key: K, value: f32, range: core::ops::RangeInclusive<f32>, scrubber: &Scrubber<'_>) -> Scrub { self.scrub_key(key.into(), value, range, scrubber) }
}
