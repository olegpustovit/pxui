//! The gallery's text inputs through the wrapper: the one place the pages' inputs are called, so `abi_bench` can supply the same functions over
//! the engine and compile the page source against either.
use pxui::{ui::{Pick, Rect}, AreaCallbacks, PickerOpts, SearchField, TextArea, TextField, Ui, ValueCallbacks, ValueField};

/// The caption field: edits `caption` in place.
pub fn caption(ui: &mut Ui, caption: &mut String) {
    const FIELD: TextField = TextField::DEFAULT.name("Caption");
    if let Some(v) = ui.text_field("caption", caption, &FIELD).changed { *caption = v.to_owned(); }
}

/// The inspector's scale value at (x, y) of width w: a number field that accepts only numbers.
pub fn scale(ui: &mut Ui, (x, y, w): (f32, f32, f32), scale: &mut String) {
    const FIELD: ValueField = ValueField::DEFAULT.name("Scale");
    let accept = |t: &str| t.parse::<f64>().is_ok();
    let callbacks = ValueCallbacks::DEFAULT.valid(&accept);
    if let Some(v) = ui.value_field("scale-value", scale, Rect::new(x, y, w, 20.0), &FIELD, &callbacks).changed { *scale = v.to_owned(); }
}

/// The inspector's notes area.
pub fn notes(ui: &mut Ui, notes: &mut String) {
    const AREA: TextArea = TextArea::DEFAULT.name("Notes");
    // None: unchanged. Some: the new text, borrowed until the next call on `ui`.
    if let Some(text) = ui.text_area("notes-area", notes, Rect::new(12.0, 4.0, 270.0, 56.0), &AREA, &AreaCallbacks::DEFAULT).changed { *notes = text.to_owned(); }
}

/// The layers search field.
pub fn layer_search(ui: &mut Ui, search: &mut String) {
    const FIELD: SearchField = SearchField::DEFAULT.icon("/").placeholder("Search layers").name("Find");
    if let Some(q) = ui.search_field("find", search, &FIELD).changed { *search = q.to_owned(); }
}

/// The presets picker: four rows from `rows(query)`.
pub fn presets(ui: &mut Ui, rows: &dyn Fn(&str) -> Vec<(String, String)>) -> Pick {
    const PICKER: PickerOpts = PickerOpts::DEFAULT.limit(4);
    ui.picker(&PICKER, "Search presets", "", rows).1
}
