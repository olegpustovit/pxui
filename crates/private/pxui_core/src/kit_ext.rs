//! The kit extension: the inspector sheet (rows with a label column 0.3 of the panel less 16 and a pin dot, collapsible sections,
//! the head block, the hint bar, toggles, buttons, readouts and messages in rows), the picker and the line and value fields. This is
//! one app's vocabulary (an inspector sheet), not generic widgetry, kept apart from the generic kit (`ui_kit`, `ui_text`,
//! `ui_widgets`) so it leaves the core with the extension set (`abi/pxui.ext.txt`, no stability guarantee); the generic widgets never
//! call into it, and only the body registry names its built-in bodies. Moved verbatim from `ui_kit` and `ui_text`.
//! Staging only: it stays in `pxui_core` until the 2.0 cut moves the extension set out of the library (its own crate on the public
//! ABI), with its body kinds; nothing new is added here.
use crate::ui_core::*;
use crate::ui_kit::*;
use crate::ui_text::*;
use crate::bodies::{body_of, in_body, in_item, in_place, InspectorRowConfig, SectionConfig, PickerRowConfig, LineFieldConfig, PickerFieldConfig, ValueFieldConfig};
use crate::a11y::state as st;
use crate::theme;
use input::{Event, Key, MouseButton};
use crate::geom::Rgba;
use std::rc::Rc;

const CLEAR: Rgba = Rgba::CLEAR;

/// The built-in inspector row: the hover fill, the label fitted to its column and the pin dot.
pub(crate) fn default_inspector_row(this: &mut Ui, c: &InspectorRowConfig, label: &str) {
    let (look, dot) = (*this.theme().row.pick(c.state), this.theme().pin);
    this.set_look(&c.row, look, c.state);
    let shown = this.fit_text(this.font_px(), c.label_w, label);
    this.text(&c.row, &shown);
    // the pin dot: filled ink when the row is on its card, an ink-3 ring when it is not
    if let Some(on) = c.pin {
        let rh = this.row_px() as f32;
        let saved = this.enter(&c.row);
        this.placed(int_key(c.row.box_key, 0), (12.0, rh / 2.0 - 3.0), Size::Px(6.0), 6.0, *dot.pick(State { checked: on, ..State::default() }), Edges::default(), "");
        this.leave(saved);
    }
}

/// The built-in section header: the section look, the label, the disclosure chevron at the end, then the content when open.
pub(crate) fn default_section(this: &mut Ui, c: &SectionConfig, label: &str, content: &mut dyn FnMut(&mut Ui)) {
    let (title, rh) = (c.title, this.row_px() as f32);
    // the sheet has no hover state on a section header
    let (look, chevron) = (this.theme().section.idle, this.theme().chevron.idle);
    this.set_look(&title, look, State::default());
    this.node_mut(&title).inset = Edges { l: this.space(2), t: c.gap, ..Edges::default() };
    this.text(&title, label);
    this.end_row(&title, this.space(2));
    let saved = this.enter(&title);
    let mark = this.flow_part(int_key(title.box_key, 0), 6.0, rh, Edges { t: c.gap, ..Edges::ZERO }, chevron, "");
    this.node_mut(&mark).mark = Mark::Chevron(if c.open { Dir::Down } else { Dir::Right });
    this.leave(saved);
    if c.open {
        content(this);
    }
}

/// The label column is 0.3 of the panel less 16 (98 at 380 wide, 80 at 320); a longer label
/// ellipsizes in it, never stacks.
pub fn inspector_label_width(width: f32) -> f32 {
    56.0f32.max((width * 0.3).floor() - 16.0)
}

pub fn inspector_control_x(width: f32) -> f32 {
    12.0 + 6.0 + 8.0 + inspector_label_width(width) + 8.0
}

/// The built-in line field: the field look (the placeholder's ink while an unfocused field is empty), 2 points of inset and the shown
/// text on the box; a focused field's text is painted by editing.
pub(crate) fn default_line_field(this: &mut Ui, c: &LineFieldConfig, shown: &str) {
    let t = this.theme();
    let field = *t.field.pick(c.state);
    let look = if c.state.focused || !c.empty { field } else { Look { text: t.placeholder.idle.text, ..field } };
    this.set_look(&c.field, look, c.state);
    this.node_mut(&c.field).inset = Edges { l: 2.0, ..Edges::default() };
    if !c.state.focused {
        this.text(&c.field, shown);
    }
}

// ------------------------------------------------------------------ picker

impl Ui {
    /// A focused search row (the label is its placeholder and key) over the `(label, detail)` rows
    /// for the current query, windowed to `limit` (default 10) around the cursor. Rows are
    /// recomputed as typing changes the query, so indices refer to the rows of the returned query.
    /// Returns the edited query and at most one result: `Pick` on Enter or a row click, `Submit` on
    /// Enter with no rows, `Delete` on a second Delete over the same (red, armed) row, `Back` on
    /// Backspace or Left with an empty query, and `Cancel` on Escape. `mark` gives a row its type
    /// square. An `off` row (one that cannot be chosen here) reads in ink-3 whole, square included,
    /// and neither Enter nor a click picks it. With `at_rest` the field keeps the keyboard but looks
    /// at rest (a hairline, no caret) until something is typed in it. Build it inside a panel or
    /// `modal`.
    /// Keyed by `key` (hashed under the parent's seed), `placeholder` its placeholder and accessible name. The picker asks for its rows
    /// by `fill(query, rows)` pushing them into `rows`, which it keeps between frames, so a settled picker copies no row into new
    /// storage; the edited query is lent until the next call.
    pub fn picker(&mut self, key: &str, placeholder: &str, query: &str, o: &PickerOpts, fill: &mut dyn FnMut(&str, &mut PickRows)) -> PickerResponse<'_> {
        let (query, pick) = picker_body(self, o, key, placeholder, query, fill);
        self.returned = query;
        PickerResponse { query: &self.returned, pick }
    }
}

fn picker_body(this: &mut Ui, o: &PickerOpts, label: &str, placeholder: &str, query: &str, fill: &mut dyn FnMut(&str, &mut PickRows)) -> (String, Pick) {
    let off = |i: i64| o.off.as_ref().map(|f| f(i)).unwrap_or(false);
    let mut rows = core::mem::take(&mut this.picker_rows);
    rows.clear();
    fill(query, &mut rows);
    // the search row: a 20-point field in 4 points of padding, a [/] before the query
    // a 20-point field in 4 points of margin
    let search = this.box_(label, BoxOpts::default().flags(Flags::CLICKABLE | Flags::FOCUSABLE | Flags::BLOCKING | Flags::FOCUS_MARK).w(Size::Grow).h(Size::Px(this.control_h())));
    this.node_mut(&search).margin = Edges::all(4.0);
    if this.focus_key() == 0 || this.slot_rw(search.box_slot) == 0.0 {
        this.focus_box(&search);
    }
    let list = this.box_keyed(int_key(search.box_key, 0), BoxOpts::default().w(Size::Grow).h(Size::Fit).axis(Axis::Column));
    let search_signal = this.signal_owned(&search);
    if search_signal.hovered {
        this.request_cursor(Cursor::Text);
    }
    let keys = this.key_events(&search);
    let clamp = |cursor: i64, count: usize| if count == 0 { 0 } else { 0.max((count as i64 - 1).min(cursor)) };
    let mut cursor = clamp(this.state(&search, 0), rows.len());
    let mut armed = this.state(&list, -1);
    let mut query = query.to_owned();
    let mut result = Pick::None;
    let mut edit = this.load_text_edit(search.box_key, &query);
    let (sx, _, sw, _) = ints(this.rect(&search));
    let prefix: i64 = if o.slash { 13 } else { 0 };
    // the slash is 6.5 wide and 6 of gap: the query starts 14.5 in from the field's edge
    let inset = if o.slash { 14.5 - prefix as f32 } else { 2.0 };
    let shift = this.press_shift(&search);
    this.point_text_caret(None, &mut edit, &search_signal, shift, (sx + 4 + prefix) as f32 + inset, (sx + sw - 4 - 2) as f32);
    for (event, modifiers) in keys {
        let count = rows.len() as i64;
        if result == Pick::None {
            match &event {
                Event::KeyPressed(Key::Backspace) if edit.text.is_empty() => result = Pick::Back,
                Event::KeyPressed(Key::ArrowLeft) if edit.text.is_empty() => result = Pick::Back,
                Event::KeyPressed(Key::ArrowDown) if count > 0 => {
                    cursor = (cursor + 1) % count;
                    armed = -1;
                }
                Event::KeyPressed(Key::ArrowUp) if count > 0 => {
                    cursor = (cursor + count - 1) % count;
                    armed = -1;
                }
                Event::KeyPressed(Key::Enter) => {
                    result = if count > 0 {
                        if off(cursor) { Pick::None } else { Pick::Pick(cursor) }
                    } else {
                        Pick::Submit
                    };
                }
                Event::KeyPressed(Key::Delete) if count > 0 && edit.caret == edit.anchor => {
                    if armed == cursor {
                        result = Pick::Delete(cursor);
                        armed = -1;
                    } else {
                        armed = cursor;
                    }
                }
                Event::KeyPressed(Key::Escape) => result = Pick::Cancel,
                event => {
                    this.edit_text_event(false, &mut edit, &|_| true, &modifiers, event);
                    if edit.text != query {
                        query = edit.text.clone();
                        rows.clear();
                        fill(&query, &mut rows);
                        cursor = 0;
                        armed = -1;
                    }
                }
            }
        }
    }
    this.save_text_edit(&edit);
    let count = rows.len() as i64;
    let length = o.limit.min(count);
    let start = if length == count { 0 } else { 0.max((count - length).min(cursor - length / 2)) };
    let composition = this.composition().to_owned();
    let font_size = this.font_px();
    let (slash, at_rest) = (o.slash, o.at_rest);
    let t = this.theme();
    // a field that has not been typed in yet can sit at rest: the hairline and no caret
    let resting = at_rest && edit.text.is_empty();
    let state = State { focused: !resting, ..State::default() };
    let field = *t.field.pick(state);
    this.set_role(&search, Role::TextField, crate::a11y::state::EDITABLE);
    this.a11y_name(&search, placeholder);
    this.a11y_value(&search, &edit.text, (edit.text[..edit.anchor.min(edit.text.len())].chars().count() as u32, edit.text[..edit.caret.min(edit.text.len())].chars().count() as u32));
    this.set_role(&list, Role::List, 0);
    let config = PickerFieldConfig { state, empty: edit.text.is_empty(), slash, prefix: prefix as f32, inset, search };
    in_body!(this, search.box_key, search, picker_field, default_picker_field(&config, placeholder));
    {
        let edit = edit.clone();
        let marks = if edit.text.is_empty() { None } else { Some((field.selection, field.caret)) };
        this.canvas_over(&search, move |paint, rect| {
            let (cx, cy, cw, ch) = ints(rect);
            let control = (cx + prefix, cy, 1.max(cw - prefix), ch);
            paint_text_edit(paint, None, false, inset, marks, control, label_y(font_size, cy, ch), &composition, &edit);
        });
    }
    let saved = this.enter(&list);
    let row_height = this.row_px();
    let body = body_of!(this, picker);
    for visible in 0..length {
        let index = start + visible;
        // a row that cannot be picked leaves the keyboard with the search field
        let row = this.box_keyed(
            int_key(list.box_key, visible),
            BoxOpts::default()
                .flags(Flags::CLICKABLE | Flags::FOCUSABLE | Flags::BLOCKING | if off(index) { Flags::KEEP_FOCUS } else { Flags::NONE })
                .w(Size::Grow)
                .h(Size::Px(row_height as f32)),
        );
        let row_signal = this.signal_owned(&row);
        if result == Pick::None && row_signal.clicked && !off(index) {
            result = Pick::Pick(index);
        }
        let current = index == cursor;
        let doomed = index == armed;
        let unavailable = off(index);
        let (text, detail) = rows.get(index as usize).unwrap_or_default();
        let swatch = o.mark.as_ref().and_then(|m| m(index)).map(|c| this.resolve_color(c));
        // the keyboard cursor is the control fill; a row armed for deletion reads in the error colour
        let state = State { hovered: row_signal.hovered, checked: current, ..State::default() };
        this.set_role(&row, Role::ListItem, if current { crate::a11y::state::SELECTED } else { 0 });
        this.a11y_name(&row, text);
        let config = PickerRowConfig { state, index, current, doomed, unavailable, marked: o.mark.is_some(), swatch, row };
        in_item!(this, body, row.box_key, row, default_picker_row(&config, text, detail, &query));
    }
    this.leave(saved);
    this.set_state(&search, cursor);
    this.set_state(&list, armed);
    this.picker_rows = rows;
    (query, result)
}

/// The built-in picker search field: the field look (the hairline, no caret ink) on the box, the `/` before the query when `slash`, and the
/// placeholder while it is empty.
pub(crate) fn default_picker_field(this: &mut Ui, c: &PickerFieldConfig, placeholder: &str) {
    let t = this.theme();
    let (hint, place_look, field) = (t.hint.idle, t.placeholder.idle, *t.field.pick(c.state));
    let (search, ch) = (c.search, this.control_h());
    this.set_look(&search, Look { text: CLEAR, ..field }, c.state);
    if c.slash {
        this.placed(int_key(search.box_key, 1), (0.0, 0.0), Size::Px(c.prefix), ch, hint, Edges { l: 2.0, ..Edges::default() }, "/");
    }
    if c.empty {
        let pw = this.text_width(None, placeholder);
        this.placed(int_key(search.box_key, 2), (c.prefix, 0.0), Size::Px(pw + c.inset), ch, place_look, Edges { l: c.inset, ..Edges::default() }, placeholder);
    }
}

/// The built-in picker row: the list look for its state (the error one when `doomed`), the swatch, the label cut to leave room for the
/// detail with the query's letters in the accent, the detail, and the Enter hint on the keyboard's row.
pub(crate) fn default_picker_row(this: &mut Ui, c: &PickerRowConfig, text: &str, detail: &str, query: &str) {
    let (row, current, doomed, unavailable, swatch, state) = (c.row, c.current, c.doomed, c.unavailable, c.swatch, c.state);
    let (font_size, row_height) = (this.font_px(), this.row_px());
    let t = this.theme();
    let ink3 = t.placeholder.idle.text;
    let look = if doomed { t.row_doomed.idle } else { *t.list.pick(state) };
    let ink = if doomed { look.text } else if unavailable { t.list.disabled.text } else { t.list.idle.text };
    let (matched, key_look, detail_look) = (t.matched.idle.text, t.key.idle, if unavailable { t.placeholder.idle } else { t.hint.idle });
    this.set_look(&row, look, state);
    let width = this.width_of(&row);
    let rk = |n: i64| int_key(row.box_key, n);
    let rh = row_height as f32;
    let text_x = if !c.marked { this.space(2) } else { this.space(2) + 16.0 };
    // a label that would run into the detail is cut with an ellipsis
    let key_size = crate::ui_core::font_px(&key_look.font, font_size);
    let key_text = "\u{21b5}";
    let kw = this.text_width(Some(key_size), key_text);
    let right = if current { width - this.space(2) - kw - 8.0 } else { width - this.space(2) };
    let dw = this.text_width(None, detail);
    let limit = right - dw - 12.0 - text_x;
    let text = ellipsis(&mut |s| this.text_width(None, s), limit, text);
    if let Some(color) = swatch {
        let fill = if unavailable { ink3 } else { color };
        this.placed(rk(0), (this.space(2), ((row_height - 8) / 2) as f32), Size::Px(8.0), 8.0, Look { fill, ..Look::NONE }, Edges::default(), "");
    }
    // the query's letters, matched in order, in the accent
    let q = query.to_ascii_lowercase();
    let mut next = 0usize;
    let mut pen = text_x;
    let mut start = 0usize;
    let is_matched = |at: usize, next: &mut usize| {
        let hit = *next < q.len() && text.len() > at && text.as_bytes()[at].to_ascii_lowercase() == q.as_bytes()[*next];
        if hit {
            *next += 1;
        }
        hit
    };
    let mut run_key = 10;
    let mut flag = !text.is_empty() && is_matched(0, &mut next);
    while start < text.len() {
        let mut stop = start + 1;
        let mut following = false;
        while stop < text.len() {
            if text.as_bytes()[stop] & 0xc0 == 0x80 {
                stop += 1;
                continue;
            }
            following = is_matched(stop, &mut next);
            if following != flag {
                break;
            }
            stop += 1;
        }
        let run = &text[start..stop];
        let rw = this.text_width(None, run);
        let color = if doomed || unavailable { ink } else if flag { matched } else { ink };
        this.placed(rk(run_key), (pen, 0.0), Size::Px(rw.max(1.0)), rh, Look { text: color, ..Look::NONE }, Edges::default(), run);
        run_key += 1;
        pen += rw;
        start = stop;
        flag = following;
    }
    this.end_row(&row, this.space(2));
    this.flow_part(rk(2), dw, rh, Edges::ZERO, detail_look, detail);
    if current {
        this.flow_part(rk(1), kw, rh, Edges { l: 8.0, ..Edges::ZERO }, key_look, key_text);
    }
}

/// Whether the text changed (the new one is in `returned`) and whether a text editor is open.
fn value_field_body(this: &mut Ui, at: (f32, f32), w: f32, h: f32, o: &ValueFieldOpts, valid: &dyn Fn(&str) -> bool, label: &str, value: &str) -> (bool, bool) {
    let slider = o.slide.is_some() || o.scrub.is_some();
    let drag = if slider && o.kept_drag { Flags::DRAG_FREE } else { Flags::NONE };
    let bx = this.box_(
        label,
        BoxOpts::default().flags(Flags::CLICKABLE | Flags::TAB_STOP | Flags::BLOCKING | Flags::CLIP | drag).at(at).w(Size::Px(w)).h(Size::Px(h)),
    );
    this.input_box = Some(bx);
    let signal = this.signal_owned(&bx);
    let bounds = ints(this.rect(&bx));
    let keys = this.key_events(&bx);
    let np = NumParams {
        size: o.size,
        control: Some(bounds),
        click_to_edit: !slider,
        edit_request: o.edit || (slider && signal.double_clicked),
        alt_to_edit: slider,
        align_right: false,
    };
    // (an idle field allocates nothing)
    let (typed, editing) = this.numeric_editor(
        &np,
        &|_| true,
        &bx,
        &signal,
        &keys,
        &|| value.to_owned(),
        &|text| if valid(text) { Some(text.to_owned()) } else { None },
    );
    if editing {
        this.set_flags_of_box(&bx, Flags::CLICKABLE | Flags::FOCUSABLE | Flags::BLOCKING | Flags::CLIP);
    }
    if signal.pressed {
        this.scrub_origin = Some((bx.box_key, value.to_owned()));
    }
    let press_keys = this.press_keys(&bx);
    let typed_is_some = typed.is_some();
    let given = value;
    let value: std::borrow::Cow<str> = match typed {
        Some(text) => std::borrow::Cow::Owned(text),
        None if !editing
            && (signal.pressed || signal.held || signal.released)
            && signal.button == Some(MouseButton::LeftButton)
            && !press_keys.contains(&Key::Alt) =>
        {
            let pointer = if signal.released { signal.release_point } else { signal.pointer };
            match (&o.slide, &o.scrub) {
                (Some(slide), _) => std::borrow::Cow::Owned(slide(fraction_at(bounds, pointer.0))),
                (None, Some(scrub)) => {
                    let origin = match &this.scrub_origin {
                        Some((key, origin)) if *key == bx.box_key => origin.clone(),
                        _ => value.to_owned(),
                    };
                    std::borrow::Cow::Owned(scrub(&origin, pointer.0 - signal.press_point.0, press_keys.contains(&Key::Shift)))
                }
                (None, None) => std::borrow::Cow::Borrowed(value),
            }
        }
        None => std::borrow::Cow::Borrowed(value),
    };
    if !signal.held && this.scrub_origin.as_ref().is_some_and(|(key, _)| *key == bx.box_key) {
        this.scrub_origin = None;
    }
    let focused_now = this.focused(&bx);
    let edit = if editing && focused_now {
        let buffer = this.text_state(&bx);
        Some((this.load_text_edit(bx.box_key, buffer.as_deref().unwrap_or(&value)), this.composition().to_owned()))
    } else {
        None
    };
    let shown: &str = if typed_is_some || signal.held || signal.released { &value } else { o.display.as_deref().unwrap_or(&value) };
    let invalid = this.state(&bx, 1) & 1 == 0;
    let field = value_field_look(this.theme(), invalid, editing);
    let config = ValueFieldConfig { state: State { focused: editing, ..State::default() }, invalid, w, h, field: bx };
    in_body!(this, bx.box_key, bx, value_field, default_value_field(&config, o, shown));
    if let Some((edit, composition)) = edit {
        let (size, fs) = (o.size, this.font_px());
        let ty = 0.max((h as i64 - size.unwrap_or(fs)) / 2);
        let (selection, caret) = (field.selection, field.caret);
        this.canvas(&bx, move |paint, rect| {
            paint_text_edit(paint, size, false, 2.0, Some((selection, caret)), ints(rect), rect.y as i64 + ty, &composition, &edit);
        });
    }
    let changed = match value {
        std::borrow::Cow::Owned(text) if text != given => {
            this.returned = text;
            true
        }
        _ => false,
    };
    (changed, editing)
}

/// The look a value field's box takes: the invalid one, the focused field's while editing, else the field's own.
fn value_field_look(t: &Theme, invalid: bool, editing: bool) -> Look {
    if invalid { t.field_invalid.idle } else { *t.field.pick(State { focused: editing, ..State::default() }) }
}

/// The built-in value field: the hairline (invalid, editing, or `line`; left out when `bare`) and, unless it is being edited, the
/// fraction line under the value, the placeholder or the value with its lead and trail.
pub(crate) fn default_value_field(this: &mut Ui, c: &ValueFieldConfig, o: &ValueFieldOpts, shown: &str) {
    let (bx, w, h, invalid, editing) = (c.field, c.w, c.h, c.invalid, c.state.focused);
    let t = this.theme();
    let field = value_field_look(t, invalid, editing);
    let (fraction, placeholder, value_look) = (t.fraction.idle, t.placeholder.idle, t.value.idle);
    // the hairline: invalid, editing, or `line` (left out when `bare`)
    let line = match o.line {
        Some(c) if !invalid && !editing => Look { border: c, ..field },
        _ => field,
    };
    let look = if invalid || editing || !o.bare { Look { text: CLEAR, ..line } } else { Look { border: CLEAR, text: CLEAR, ..line } };
    this.set_look(&bx, look, State { focused: editing, ..State::default() });
    let x = 0.0;
    let size = o.size;
    let fs = this.font_px();
    // the content is centred in the 19 points above the hairline: a body line starts half a point up
    let text_y = match size {
        Some(size) => (0.max((h as i64 - size) / 2)) as f32,
        None => (h - 1.0 - fs as f32) / 2.0 - 0.5,
    };
    let points = size.unwrap_or(0);
    let ink = |look: Look| Look { font: Font { size: points, ..look.font }, ..look };
    if editing {
        return;
    }
    let k = |n: i64| int_key(bx.box_key, n);
    // the 2-point line under the value is its position in the soft range
    if let Some(f) = o.fraction {
        this.placed(k(0), (x, h - 3.0), Size::Px((w * 0.0f32.max(1.0f32.min(f))).max(0.0)), 2.0, fraction, Edges::default(), "");
    }
    let tracking = o.tracking.unwrap_or(0.0);
    let text_w = |ui: &mut Ui, t: &str| ui.text_width(size, t);
    match (&o.lead, &o.trail) {
        (None, None) if shown.is_empty() && o.placeholder.is_some() => {
            let p = o.placeholder.as_deref().unwrap_or("");
            let pw = text_w(this, p);
            this.text_part(k(1), (2.0, text_y), pw, ink(placeholder), p);
        }
        (None, None) => {
            let width = text_w(this, shown);
            let tx = if o.left { 2.0 } else { w - 2.0 - width };
            let em = size.unwrap_or(fs) as f32;
            let look = ink(value_look);
            this.text_part(k(1), (tx, text_y), width, Look { font: Font { tracking: tracking / em, ..look.font }, ..look }, shown);
        }
        _ => {
            // a lead (the expression's f) and a trail (its live value) frame the value: 6 between
            let part = |ui: &mut Ui, n: i64, right: bool, text: &str, color: Rgba| -> f32 {
                let width = text_w(ui, text);
                let tx = if right { w - 2.0 - width } else { 2.0 };
                ui.text_part(k(n), (tx, text_y), width, ink(Look { text: color, ..value_look }), text);
                width
            };
            let lead_w = match &o.lead {
                Some((t, c)) => part(this, 2, false, t, *c) + 6.0,
                None => 0.0,
            };
            let trail_w = match &o.trail {
                Some((t, c)) => part(this, 3, true, t, *c) + 6.0,
                None => 0.0,
            };
            // with no lead the value sits against its trail (a unit: [35 mm])
            let fitted = this.fit_text(size.unwrap_or(fs), w - 4.0 - lead_w - trail_w, shown);
            let fw = text_w(this, &fitted);
            let value_x = if o.lead.is_none() && !o.left { w - 2.0 - trail_w - fw } else { 2.0 + lead_w };
            this.text_part(k(1), (value_x, text_y), fw, ink(value_look), &fitted);
        }
    }
}

fn inspector_header_body(this: &mut Ui, key: &str, kind: Option<&str>, badge: Option<&str>, index: Option<&str>, rename: Option<Rc<dyn Fn(&str) -> bool>>, actions: &[HeadAction], reset: Option<&str>, reset_enabled: bool, title: &str, detail: &str) -> Head {
    let win = this.kit_window_flag();
    let font_size = this.font_px();
    // a window's head (windows.html): the 20-point title and one detail line, 8 above, no
    // chips, buttons or hairline
    let display_size = font_size * (if win { theme::TITLE_SIZE } else { theme::DISPLAY_SIZE }) / theme::FONT_SIZE;
    let small = 8.max(font_size - 2);
    let line = this.row_px() as f32;
    let title_h = if win { this.row_px() as f32 } else { (display_size + 8) as f32 };
    let width = this.inspector_width();
    let chips = !win && (kind.is_some() || badge.is_some() || index.is_some());
    let actions: Vec<HeadAction> = if win { Vec::new() } else { actions.to_vec() };
    let reset = if win { None } else { reset };
    let buttons = !actions.is_empty() || reset.is_some();
    let text_x = if win { 12.0 } else { 8.0 };
    let y_title = if win { 8.0 } else if chips { 12.0 + line + 4.0 } else { 12.0 };
    let y_detail = y_title + title_h + if win { 0.0 } else { 4.0 };
    let y_buttons = y_detail + line + 8.0;
    let height = if win {
        y_detail + line
    } else {
        (if buttons { y_buttons + 20.0 } else { y_detail + line }) + 8.0 + 1.0
    };
    let header = this.box_(key, BoxOpts::default().flags(Flags::CLIP).w(Size::Grow).h(Size::Px(height)));
    let renaming = rename.is_some();
    let title_shown = match &rename {
        Some(valid) => {
            let saved = this.enter(&header);
            let o = ValueFieldOpts { size: Some(display_size), left: true, bare: true, tracking: Some(-0.01 * display_size as f32), ..ValueFieldOpts::default() };
            let (t, _) = this.value_field(
                (text_x - 2.0, if win { y_title - 1.0 } else { y_title - 2.0 }),
                width - 2.0 * (text_x - 2.0),
                title_h,
                &o,
                &**valid,
                &format!("{}-name", key),
                title,
            );
            let t = t.map_or_else(|| title.to_owned(), str::to_owned);
            this.leave(saved);
            t
        }
        None => title.to_owned(),
    };
    // the buttons: a box each, in a row from 8 with 4 between; `reset` is bare at the right
    let mut pressed: Option<i64> = None;
    let mut place = 8.0;
    for (i, a) in actions.iter().enumerate() {
        let w = 14.0 + mono_width(0.0, font_size, &a.caption) + if a.keycap.is_empty() { 0.0 } else { 6.0 + mono_width(0.0, small, &a.keycap) };
        this.head_button(&header, key, i as i64, &a.caption, &a.keycap, a.active, false, a.usable, place, w, 6.0, y_buttons, &mut pressed);
        place += w + 4.0;
    }
    let mut reset_pressed = false;
    if let Some(label) = reset {
        let w = 10.0 + mono_width(0.0, font_size, label);
        let before = pressed;
        this.head_button(&header, key, -1, label, "", false, true, reset_enabled, width - 8.0 - w, w, 4.0, y_buttons, &mut pressed);
        if pressed != before {
            pressed = before;
            reset_pressed = true;
        }
    }
    let t = this.theme();
    let (rule, label_look, badge_look, index_look, title_look, hint) = (t.header.idle, t.label.idle, t.badge.idle, t.index.idle, t.title.idle, t.hint.idle);
    if !win {
        this.set_look(&header, rule, State::default());
    }
    let saved = this.enter(&header);
    let k = |n: i64| int_key(header.box_key, 100 + n);
    if chips {
        let at = 12.0 + text_top_for(font_size, Some(small), 0.0, line);
        let mut cx = 8.0;
        // the kind gives way when the index and badge need the room
        let w = width;
        let per = small as f32 / 2.0 + 0.08 * small as f32;
        let taken = index.map(|t| mono_width(0.0, small, t) + 6.0).unwrap_or(0.0) + badge.map(|t| mono_width(0.08 * small as f32, small, t) + 6.0).unwrap_or(0.0);
        if let Some(text) = kind {
            let room = ((w - 16.0 - taken) / per) as i64;
            let text: String = if text.len() as i64 <= room {
                text.to_owned()
            } else if room <= 1 {
                String::new()
            } else {
                format!("{}.", &text[..(room - 1) as usize])
            };
            let tw = this.cap_width(&text);
            this.text_part(k(0), (cx, at), tw, label_look, &text);
            // the glyphs run a little wider than the design's metrics: never closer than 6
            cx += mono_width(0.08 * small as f32, small, &text).max(tw) + 6.0;
        }
        if let Some(text) = badge {
            let tw = this.cap_width(text);
            this.text_part(k(1), (cx, at), tw, badge_look, text);
        }
        // the sheet's 0.04em tracking follows every letter, the last too, so it ends 8 from the edge
        if let Some(text) = index {
            let tracking = 0.04 * small as f32;
            let count = text.bytes().filter(|b| b & 0xc0 != 0x80).count();
            let tw = this.text_width(Some(small), text);
            this.end_row(&header, 0.0);
            this.flow_part(k(2), tw, small as f32, Edges { t: at, r: 8.0 + tracking * count as f32, ..Edges::ZERO }, index_look, text);
        }
    }
    if !renaming {
        let dy = if win { text_top_for(font_size, Some(display_size), 0.0, title_h) } else { 2.0 };
        let look = Look { font: Font { size: display_size, ..title_look.font }, ..title_look };
        let n = this.node_keyed(k(3), Node { flags: Flags::FIT_TEXT, h: Size::Px(display_size as f32), at: Some((0.0, y_title + dy)).into(), look, inset: Edges { l: text_x, r: text_x, ..Edges::default() }, ..Node::DEFAULT });
        this.text(&n, title);
    }
    let ty = text_top_for(font_size, None, 0.0, line);
    let n = this.node_keyed(k(4), Node { flags: Flags::FIT_TEXT, h: Size::Px(font_size as f32), at: Some((0.0, y_detail + ty)).into(), look: hint, inset: Edges { l: text_x, r: text_x, ..Edges::default() }, ..Node::DEFAULT });
    this.text(&n, detail);
    this.leave(saved);
    Head { renamed: title_shown, chosen: pressed, reset_pressed }
}


impl Ui {
    /// The space above a section header: 4 at the top of its parent (16 under a window's head), 16
    /// after anything else, none when this closed section directly follows a closed one.
    fn section_gap(&self, next_closed: bool, (top, after_closed): (bool, bool)) -> i64 {
        if top {
            if self.kit_window_flag() { 16 } else { 4 }
        } else if after_closed && next_closed {
            0
        } else {
            16
        }
    }

    /// The width of the panel being built: what an inspector row lays its columns out in.
    pub fn inspector_width(&self) -> f32 {
        let mut index = self.current_parent();
        loop {
            if index == 0 {
                return 280.0;
            }
            match self.box_w_px(index) {
                Some(width) => return width,
                None => index = self.box_parent(index),
            }
        }
    }

    /// Where an inspector row's label starts: 26 (after the pin slot), 12 in a window.
    pub fn inspector_label_x(&self) -> f32 {
        if self.kit_window_flag() { 12.0 } else { 26.0 }
    }

    /// The 4 points between a head's rule and the first element of the body, when it is not a
    /// section.
    fn lead_gap(&mut self) {
        if self.last_child_of_parent() == NONE {
            self.box_("lead-gap", BoxOpts::default().w(Size::Grow).h(Size::Px(4.0)));
        }
    }

    /// An inspector row of the kit's inspector sheet and the local x, y, width of its value control
    /// (always 20 high at y 2): 12 of padding, a 6-point slot for the pin dot (drawn when `pin` is
    /// given: filled ink for a row on its card, an ink-3 ring for one that is not), 8, the label
    /// column, 8, the control to 12 from the right edge. The hovered row has the line-1 fill.
    pub fn inspector_row(&mut self, width: Option<f32>, pin: Option<bool>, key: &str, label: &str) -> (UiBox, f32, f32, f32) {
        let width = width.unwrap_or_else(|| self.inspector_width());
        let label_x = self.inspector_label_x();
        let window = self.kit_window_flag();
        let label_w = if window { 88.0 } else { inspector_label_width(width) };
        let control_x = if window { 12.0 + 88.0 + 8.0 } else { inspector_control_x(width) };
        let pin = if window { None } else { pin };
        self.lead_gap();
        let rh = self.row_px() as f32;
        let row = self.node_ref(key, &Node { flags: Flags::CLICKABLE, h: Size::Px(rh), inset: Edges { l: label_x, ..Edges::default() }, ..Node::DEFAULT });
        let state = State { hovered: self.hovered_within(&row), ..State::default() };
        self.set_role(&row, Role::Label, 0);
        self.a11y_name(&row, label);
        let config = InspectorRowConfig { row, state, label_w, pin };
        in_body!(flat; self, row.box_key, row, inspector_row, default_inspector_row(&config, label));
        (row, control_x, 2.0, 40.0f32.max(width - control_x - 12.0))
    }

    /// A collapsible inspector section. Returns whether it is open (and `f` ran).
    pub fn inspector_section(&mut self, key: &str, expanded: bool, set_expanded: Option<bool>, label: &str, f: &mut dyn FnMut(&mut Ui)) -> bool {
        let at = self.section_place();
        self.section_at(at, key, expanded, set_expanded, label, f).0
    }

    /// A collapsible section: a header row showing `text` that a press (or Enter/Space while focused) toggles, built through the
    /// `inspector_section` body, then `f` under it while open. The caller owns `open` and passes it back each frame; the response is the
    /// header's, `clicked` when this frame's input toggled it (the new value is `!open`).
    pub fn disclosure(&mut self, key: &str, text: &str, open: bool, f: &mut dyn FnMut(&mut Ui)) -> Response {
        let at = self.section_place();
        self.section_at(at, key, open, Some(open), text, f).1
    }

    /// Where a section starts: first in its parent, and right after a closed section.
    fn section_place(&self) -> (bool, bool) {
        let last = self.last_child_of_parent();
        (last == NONE, last != NONE && last == self.closed_section_index())
    }

    /// `inspector_section` as a content-sized cached view (`Ui::view`, `Fit` high): `f` runs only
    /// when `revision` (what the section shows), its place among its siblings, its label or
    /// `set_expanded` change, or input reaches it; kept, its rows hand back nothing. While a payload
    /// is carried it builds every frame (its controls report their drop targets as they build).
    /// `ui.expanded(key)` reads its state as it reads a plain section's.
    pub fn inspector_section_view(&mut self, key: &str, revision: u64, expanded: bool, set_expanded: Option<bool>, label: &str, f: &mut dyn FnMut(&mut Ui)) -> bool {
        let at = self.section_place();
        let carrying = self.carry_frame();
        let flags = at.0 as u64 | (at.1 as u64) << 1 | (expanded as u64) << 2 | (set_expanded.map_or(0, |o| 2 + o as u64)) << 3;
        let revision = key_of(key_of(revision as i64 ^ (flags as i64).rotate_left(48) ^ carrying as i64, label), "") as u64;
        let mut built = None;
        let view = self.view(key, revision, Size::Grow, Size::Fit, &mut |ui| {
            built = Some(ui.section_at(at, key, expanded, set_expanded, label, f).0);
        });
        let open = match built {
            Some(open) => {
                self.set_state(&view, open as i64);
                open
            }
            None => self.state(&view, expanded as i64) != 0,
        };
        if !open {
            self.set_closed_section(view.index);
        }
        open
    }

    fn section_at(&mut self, at: (bool, bool), key: &str, expanded: bool, set_expanded: Option<bool>, label: &str, f: &mut dyn FnMut(&mut Ui)) -> (bool, Response) {
        let rh = self.row_px() as f32;
        let title = self.box_(key, BoxOpts::default().flags(Flags::CLICKABLE | Flags::TAB_STOP).w(Size::Grow).h(Size::Px(rh)));
        let open = self.state(&title, if expanded { 1 } else { 0 }) != 0;
        let open = set_expanded.unwrap_or(open);
        let signal = self.signal_owned(&title);
        let open = if signal.clicked { !open } else { open };
        // the gap above depends on this section's own state, known only now
        let gap = self.section_gap(!open, at) as f32;
        self.set_box_h(&title, Size::Px(gap + rh));
        self.set_state(&title, if open { 1 } else { 0 });
        if !open {
            self.set_closed_section(title.index);
        }
        self.a11y_name(&title, label);
        self.set_role(&title, Role::Heading, st::EXPANDABLE | if open { st::EXPANDED } else { 0 });
        let config = SectionConfig { title, open, gap };
        in_place!(self, title.box_key, inspector_section, default_section(&config, label, f));
        (open, Response::of(&signal, self.focused(&title), title))
    }

    /// The retained state of an inspector section built in the current parent, by its key.
    pub fn expanded(&self, text: &str) -> Option<bool> {
        let key = key_of(self.lookup_seed(), text);
        let slot = self.slot_for(key)?;
        self.slot_state(slot).map(|value| value == 1)
    }

    /// The value control inside an inspector row.
    /// The kit toggle's switch alone, its box keyed `key` at `at` in the current parent, built through the toggle body (an app's
    /// registered toggle body draws it); the toggle inside is keyed `value`.
    fn switch_at(&mut self, key: &str, at: (f32, f32), value: bool) -> Response {
        // the toggle body sets its switch the theme's second space step in from the row's end: the box is that much wider, so the
        // switch's start is `at` in every theme
        let (h, side) = (self.control_h(), self.space(2));
        let control = self.box_(key, BoxOpts::default().at(at).w(Size::Px(28.0 + side)).h(Size::Px(h)));
        let saved = self.enter(&control);
        let response = self.toggle_opts("value", "", value, &ToggleOpts { row_h: Some(h), ..ToggleOpts::DEFAULT });
        self.leave(saved);
        response
    }

    /// A bare inspector switch at `at`, named `key`; the value after this frame's click.
    pub fn inspector_toggle_value(&mut self, key: &str, at: (f32, f32), value: bool) -> bool {
        let response = self.switch_at(key, at, value);
        self.a11y_name(&response.handle, key);
        value ^ response.clicked
    }

    /// A switch row: the switch sits at the start of the control column, not at the row's end, named by the row's `label`.
    pub fn inspector_toggle(&mut self, key: &str, label: &str, value: bool) -> bool {
        let (row, cx, cy, _) = self.inspector_row(None, None, key, label);
        let saved = self.enter(&row);
        let response = self.switch_at("switch", (cx, cy), value);
        self.a11y_name(&response.handle, label);
        self.leave(saved);
        value ^ response.clicked
    }

    /// A text button on a row of its own: the kit button, built through the button body, its text 6 points in from the row's start.
    pub fn inspector_button(&mut self, key: &str, label: &str) -> bool {
        self.lead_gap();
        let h = self.control_h();
        let pad = (self.row_px() as f32 - h) / 2.0;
        let row = self.node_ref(key, &Node { h: Size::Px(self.row_px() as f32), axis: Axis::Row, padding: Edges { l: 6.0, t: pad, r: 6.0, b: pad }, ..Node::DEFAULT });
        // the ground is the label and 6 points each side, as the inspector has always drawn it: the button's looks without their ring
        let tw = self.text_width(None, label);
        let mut states = self.theme().button;
        for look in [&mut states.idle, &mut states.hover, &mut states.pressed, &mut states.focus, &mut states.disabled, &mut states.checked] { look.ring = Ring::NONE; }
        let opts = ButtonOpts { states: Some(states), w: Size::Px(tw + 12.0), h: Size::Px(h), centered: true, ..ButtonOpts::DEFAULT };
        let saved = self.enter(&row);
        let response = self.button_opts("button", label, None, &opts, ["", "", ""], None, None);
        self.leave(saved);
        response.clicked
    }

    /// A read-only value in the same responsive inspector row.
    pub fn inspector_readout(&mut self, width: Option<f32>, key: &str, label: &str, value: &str) {
        let (row, control_x, _, control_width) = self.inspector_row(width, None, key, label);
        let look = self.theme().value.idle;
        self.a11y_value(&row, value, (0, 0));
        let shown = self.fit_text(self.font_px(), control_width, value);
        let tw = self.text_width(None, &shown);
        let rh = self.row_px() as f32;
        let saved = self.enter(&row);
        self.placed(int_key(row.box_key, 1), (control_x + control_width - tw, 0.0), Size::Px(tw.max(1.0)), rh, look, Edges::default(), &shown);
        self.leave(saved);
    }

    /// The scrolling part of an inspector under its head (the head stays): a column that takes the
    /// rest of the panel and scrolls, its 4-point line-3 thumb 2 from the right edge.
    pub fn inspector_body(&mut self, f: &mut dyn FnMut(&mut Ui)) {
        let rh = self.row_px() as f32;
        let t = self.theme();
        let bar = Bar::new(t.scrollbar.idle.fill, t.scrollbar_w).radius(t.scrollbar.idle.radius[0]).gap(t.scrollbar_gap).margin(6.0);
        let body = self.node("inspector-body".into(), Node { flags: Flags::SCROLL | Flags::CLIP | Flags::OVER_THUMB, h: Size::Grow, scroll_step: rh, bar: Some(bar).into(), role: Role::ScrollArea, ..Node::DEFAULT });
        self.within(&body, f);
    }

    /// The bar under an inspector (inspector.html): a line-2 hairline and a 24-point bar of hints,
    /// each a key in ink-3 at the label size and what it does in ink-2, and at the right a 6-point
    /// dot and a count as a label.
    pub fn inspector_bar(&mut self, hints: &[(&str, &str)], count: i64) {
        let small = 8.max(self.font_px() - 2);
        let font_size = self.font_px();
        let line = self.row_px() as f32;
        let bar = self.box_("inspector-bar", BoxOpts::default().w(Size::Grow).h(Size::Px(line + 1.0)));
        let t = self.theme();
        let (rule, key, hint, label, dot) = (t.footer.idle, t.key.idle, t.hint.idle, t.label.idle, *t.pin.pick(State { checked: true, ..State::default() }));
        self.set_look(&bar, rule, State::default());
        self.set_role(&bar, Role::StatusBar, 0);
        let saved = self.enter(&bar);
        let mut at = 12.0;
        for (i, (k, what)) in hints.iter().enumerate() {
            let kw = mono_width(0.0, small, k);
            self.placed(int_key(bar.box_key, 2 * i as i64), (at, 1.0), Size::Px(kw.max(1.0)), line, key, Edges::default(), k);
            at += kw + 8.0;
            let ww = mono_width(0.0, font_size, what);
            self.placed(int_key(bar.box_key, 2 * i as i64 + 1), (at, 1.0), Size::Px(ww.max(1.0)), line, hint, Edges::default(), what);
            at += ww + 8.0;
        }
        let text = format!("{} on card", count);
        // the sheet's tracking follows the last letter too: the ink ends that much before the 12
        let label_w = self.cap_width(&text) + 0.08 * small as f32;
        self.end_row(&bar, 12.0);
        self.flow_part(int_key(bar.box_key, -2), 6.0, 6.0, Edges { t: 1.0 + line / 2.0 - 3.0, r: 8.0, ..Edges::ZERO }, dot, "");
        self.flow_part(int_key(bar.box_key, -1), label_w, line, Edges { t: 1.0, ..Edges::ZERO }, label, &text);
        self.leave(saved);
    }

    pub fn inspector_message(&mut self, key: &str, message: &str) {
        self.lead_gap();
        let rh = self.row_px() as f32;
        let (dot, hint) = (self.theme().notice.idle, self.theme().hint.idle);
        let row = self.node_ref(key, &Node { flags: Flags::FIT_TEXT, h: Size::Px(rh), look: hint, inset: Edges { l: 26.0, r: 12.0, ..Edges::default() }, role: Role::Label, ..Node::DEFAULT });
        self.text(&row, message);
        let saved = self.enter(&row);
        self.placed(int_key(row.box_key, 0), (12.0, rh / 2.0 - 3.0), Size::Px(6.0), 6.0, dot, Edges::default(), "");
        self.leave(saved);
    }

    /// Compact field. Numeric sliders follow the pointer with `slide`; Option-click, a
    /// double-click or `edit` opens text entry. `scrub` handles fields without a track. `left`
    /// aligns text values to the left (default false for numeric fields). `lead` and `trail` frame
    /// the value at the left and right with 6 points between; `line` is the idle hairline's colour;
    /// `bare` leaves the hairline out until the field is edited or invalid; `placeholder` is shown
    /// in ink-3 where an empty value would be. Only valid text commits. Returns the new text on the
    /// frames it differs from `value` (borrowed until the next call on the Ui) and whether a text
    /// editor is open.
    #[allow(clippy::too_many_arguments)]
    pub fn value_field(&mut self, at: (f32, f32), w: f32, h: f32, o: &ValueFieldOpts, valid: &dyn Fn(&str) -> bool, label: &str, value: &str) -> (Option<&str>, bool) {
        let (changed, editing) = value_field_body(self, at, w, h, o, valid, label, value);
        (changed.then_some(self.returned.as_str()), editing)
    }

    /// A `kept_drag` slider field held and dragged by the left button this frame, its view kept: the
    /// fraction of its track under the pointer (what `slide` takes) and the pointer's points from
    /// the press (what `scrub` takes), as its build would read them. `None` on any other frame (a
    /// press, a release, typing: its view builds and the field reads them itself).
    pub fn field_drag(&self, field: &UiBox) -> Option<(f32, f32)> {
        if !self.live(field) || !self.box_flags(field).any(Flags::DRAG_FREE) {
            return None;
        }
        let signal = self.signal_owned(field);
        let pure = signal.held && !signal.pressed && !signal.released && signal.button == Some(MouseButton::LeftButton) && signal.keys.is_empty();
        let alt = self.press_keys(field).contains(&Key::Alt);
        (pure && !alt).then(|| (fraction_at(ints(self.rect(field)), signal.pointer.0), signal.pointer.0 - signal.press_point.0))
    }

    /// A `value_field`'s value (`text`) and soft-range line (`o.fraction`) written by patches, as its
    /// build lays them out without a lead (`o.left`, `o.size`, `o.trail`). False when a patch cannot
    /// show it (a lead, a value too wide beside its trail, the line had no width when built, the
    /// handle is stale): build the field's view.
    pub fn patch_field(&mut self, field: &UiBox, o: &ValueFieldOpts, text: &str) -> bool {
        let (Some(value), true, None) = (self.box_of_key(int_key(field.box_key, 1)), self.live(field), &o.lead) else { return false };
        let at = self.local_rect(field);
        let mut ok = true;
        if let Some(f) = o.fraction {
            let width = (at.w * 0.0f32.max(1.0f32.min(f))).max(0.0);
            match self.box_of_key(int_key(field.box_key, 0)) {
                Some(line) if self.local_rect(&line).w > 0.0 || width == 0.0 => {
                    let r = self.local_rect(&line);
                    if r.w != width {
                        ok &= self.patch_rect(&line, Rect::new(r.x, r.y, width, r.h));
                    }
                }
                _ => return false,
            }
        }
        // a trail (a unit) stays put: the value sits against it, as long as it fits unshortened
        let trail = match &o.trail {
            Some((t, _)) => self.text_width(o.size, t) + 6.0,
            None => 0.0,
        };
        let width = self.text_width(o.size, text);
        if trail > 0.0 && width > at.w - 4.0 - trail {
            return false;
        }
        let r = self.local_rect(&value);
        let x = at.x + if o.left { 2.0 } else { at.w - 2.0 - trail - width };
        if (r.x, r.w) != (x, width.max(1.0)) {
            ok &= self.patch_rect(&value, Rect::new(x, r.y, width.max(1.0), r.h));
        }
        ok && self.patch_text(&value, text)
    }

    /// The head block of a panel, from the kit's inspector sheet: 12 above, a chips row (the kind
    /// in ink-2, a badge in the accent, an index at the right in ink-3), the name at the display
    /// size (edited in place when `rename` says which names are valid), one detail line in ink-2, a
    /// row of text buttons with their keys and a trailing `reset` button, 8 below and a line-2
    /// hairline.
    #[allow(clippy::too_many_arguments)]
    pub fn inspector_header(
        &mut self,
        key: &str,
        kind: Option<&str>,
        badge: Option<&str>,
        index: Option<&str>,
        rename: Option<Rc<dyn Fn(&str) -> bool>>,
        actions: &[HeadAction],
        reset: Option<&str>,
        reset_enabled: bool,
        title: &str,
        detail: &str,
    ) -> Head {
        inspector_header_body(self, key, kind, badge, index, rename, actions, reset, reset_enabled, title, detail)
    }

    #[allow(clippy::too_many_arguments)]
    fn head_button(
        &mut self,
        header: &UiBox,
        key: &str,
        index: i64,
        label: &str,
        hint: &str,
        on: bool,
        dim: bool,
        enabled: bool,
        x: f32,
        w: f32,
        pad: f32,
        y_buttons: f32,
        pressed: &mut Option<i64>,
    ) {
        let saved = self.enter(header);
        let b = self.box_(
            &format!("{}-button-{}", key, index),
            BoxOpts::default().flags(if enabled { Flags::CLICKABLE | Flags::TAB_STOP } else { Flags::NONE }).at((x, y_buttons)).w(Size::Px(w)).h(Size::Px(self.control_h())),
        );
        let signal = self.signal_owned(&b);
        if enabled && signal.clicked {
            *pressed = Some(index);
        }
        let t = self.theme();
        let states = if dim { t.button_bare } else { t.button };
        let key_look = t.key.idle;
        let state = State { hovered: enabled && signal.hovered, pressed: enabled && signal.held, checked: on, disabled: !enabled, ..State::default() };
        // a button that cannot be pressed keeps the fill it is on with
        let look = if !enabled && on { Look { text: states.disabled.text, ..states.checked } } else { *states.pick(state) };
        self.set_look(&b, look, state);
        self.node_mut(&b).inset = Edges { l: 1.0 + pad, ..Edges::default() };
        self.text(&b, label);
        if !hint.is_empty() {
            let inner = self.enter(&b);
            let font_size = self.font_px();
            let at = 1.0 + pad + mono_width(0.0, font_size, label) + 6.0;
            let hw = self.text_width(Some(crate::ui_core::font_px(&key_look.font, font_size)), hint);
            let n = self.node_keyed(int_key(b.box_key, 0), Node { w: Size::Px(hw.max(1.0)), h: Size::Px(self.control_h()), at: Some((at, 0.0)).into(), look: key_look, ..Node::DEFAULT });
            self.text(&n, hint);
            self.leave(inner);
        }
        self.leave(saved);
    }


    /// A one-line search field at `at` (the first string is its key, the second its value) with a
    /// hairline: the new value and what its keys asked, Enter or Down `Next`, Shift-Enter or Up
    /// `Previous`, Escape `Close`. `grab` takes the keyboard for it.
    #[allow(clippy::too_many_arguments)]
    pub fn line_field(&mut self, at: (f32, f32), w: f32, h: f32, grab: bool, placeholder: &str, label: &str, value: &str) -> (String, LineStep) {
        let field_box = self.box_(label, BoxOpts::default().flags(Flags::CLICKABLE | Flags::FOCUSABLE | Flags::BLOCKING | Flags::FOCUS_MARK).at(at).w(Size::Px(w)).h(Size::Px(h)));
        let field = field_box;
        self.input_box = Some(field);
        if grab {
            self.focus_box(&field);
        }
        let signal = self.signal_owned(&field);
        let focused = self.focused(&field);
        let mut edit = if focused {
            self.load_text_edit(field.box_key, value)
        } else {
            TextEdit { text: value.to_owned(), caret: value.len(), anchor: value.len() }
        };
        let mut step = LineStep::None;
        if focused {
            let (fx, _, fw, _) = ints(self.rect(&field));
            let shift = self.press_shift(&field);
            self.point_text_caret(None, &mut edit, &signal, shift, (fx + 2) as f32, (fx + fw - 2) as f32);
            for (event, modifiers) in self.key_events(&field) {
                match &event {
                    Event::KeyPressed(Key::Enter) if !self.command_modifiers(&modifiers) => {
                        step = if modifiers.contains(&Key::Shift) { LineStep::Previous } else { LineStep::Next };
                    }
                    Event::KeyPressed(Key::ArrowDown) => step = LineStep::Next,
                    Event::KeyPressed(Key::ArrowUp) => step = LineStep::Previous,
                    Event::KeyPressed(Key::Escape) => step = LineStep::Close,
                    event => {
                        self.edit_text_event(false, &mut edit, &|_| true, &modifiers, event);
                    }
                }
            }
            self.save_text_edit(&edit);
        }
        if signal.hovered {
            self.request_cursor(Cursor::Text);
        }
        let state = State { focused, ..State::default() };
        let config = LineFieldConfig { state, empty: edit.text.is_empty(), field: field_box };
        let shown = if edit.text.is_empty() { placeholder } else { &edit.text };
        in_body!(flat; self, field_box.box_key, field_box, line_field, default_line_field(&config, shown));
        if focused {
            let field = *self.theme().field.pick(state);
            let composition = self.composition().to_owned();
            let font_size = self.font_px();
            let pedit = edit.clone();
            let marks = Some((field.selection, field.caret));
            self.canvas(&field_box, move |paint, rect| {
                let control = ints(rect);
                let (_, y, _, h) = control;
                paint_text_edit(paint, None, false, 2.0, marks, control, label_y(font_size, y, h), &composition, &pedit);
            });
        }
        (edit.text, step)
    }
}
