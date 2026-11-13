//! Text widgets: single-line fields, numeric and value fields, the inspector head, the multiline text
//! area with its language hooks, the picker and the context menu. Every text widget edits as the
//! host's conventions say (Apple: Command and Option; elsewhere Ctrl): a click places the caret and
//! Shift-click or a drag extends the selection, a double click selects the word (a triple the line),
//! the word keys move and delete by words, Command-arrows (Apple) and Home/End move by lines,
//! Command-A/C/X/V select all, copy, cut and paste, and Command-Z, Shift-Command-Z (or Command-Y)
//! undo and redo the focused text.
use crate::bodies::{body_of, in_body, in_item, MenuRowConfig, PickerRowConfig, LineFieldConfig, PickerFieldConfig, TextFieldConfig, ValueFieldConfig};
use crate::theme;

const CLEAR: Color = Color { r: 0, g: 0, b: 0, a: 0 };
use crate::ui_core::*;
use crate::ui_kit::*;
pub use crate::opts::{Language, PickerOpts, TextAreaOpts, ValueFieldOpts};
pub use pxui_types::ui::{scrubbed, ScrubPhase, MenuOpts, Submenu, TextEdit, LineStep, fuzzy_match, fuzzy_score, line_end, line_start, CodeContext, Completion, MenuResult, Pick};
use input::{Event, Key, MouseButton};
use crate::geom::{color, Rgba as Color};
use std::rc::Rc;

pub fn previous_utf8(text: &str, index: usize) -> usize {
    if index == 0 {
        return 0;
    }
    let bytes = text.as_bytes();
    let mut i = index - 1;
    loop {
        if i == 0 {
            return 0;
        }
        if bytes[i] & 0xc0 != 0x80 {
            return i;
        }
        i -= 1;
    }
}

pub fn next_utf8(text: &str, index: usize) -> usize {
    let length = text.len();
    if index >= length {
        return length;
    }
    let bytes = text.as_bytes();
    let mut i = index + 1;
    loop {
        if i >= length {
            return length;
        }
        if bytes[i] & 0xc0 != 0x80 {
            return i;
        }
        i += 1;
    }
}

fn numeric_character(c: char) -> bool {
    matches!(c, '0'..='9' | '+' | '-' | '.' | 'e' | 'E')
}

fn clipboard_command(command: bool, event: &Event) -> Option<char> {
    match event {
        Event::KeyPressed(Key::KeyChar(key)) if command => Some(key.to_ascii_lowercase()),
        _ => None,
    }
}

/// Snapshots hold the whole text, and five stacks of 200 at 100 KB is 100 MB: so one cap covers the
/// open stacks and the shelf together. The shelf's oldest stack goes first, then the open text's
/// oldest step; the newest step stays, so a text over the cap still undoes once.
pub const TEXT_UNDO_BYTE_CAP: usize = 8 * 1024 * 1024;

fn stack_bytes(stack: &[(String, usize, usize)]) -> usize {
    stack.iter().map(|(text, _, _)| text.len()).sum()
}

pub fn text_selection(edit: &TextEdit) -> (usize, usize) {
    (edit.caret.min(edit.anchor), edit.caret.max(edit.anchor))
}

pub fn replace_text(edit: &mut TextEdit, inserted: &str) {
    let (start, stop) = text_selection(edit);
    edit.text = format!("{}{}{}", &edit.text[..start], inserted, &edit.text[stop..]);
    edit.caret = start + inserted.len();
    edit.anchor = edit.caret;
}

/// Words as macOS counts them: letters, digits, underscores and every non-ASCII byte.
fn word_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c >= 128
}

/// Option-Left: back over the separators, then over the word
pub fn word_left(text: &str, i: usize) -> usize {
    let b = text.as_bytes();
    let mut i = i;
    while i > 0 && !word_char(b[i - 1]) {
        i -= 1;
    }
    while i > 0 && word_char(b[i - 1]) {
        i -= 1;
    }
    i
}

/// Option-Right: over the separators, then to the end of the word
pub fn word_right(text: &str, i: usize) -> usize {
    let b = text.as_bytes();
    let n = b.len();
    let mut i = i;
    while i < n && !word_char(b[i]) {
        i += 1;
    }
    while i < n && word_char(b[i]) {
        i += 1;
    }
    i
}

// ---- line and form edits of the text area ----

/// the lines a selection touches, as the byte range `[first, stop)` without the last break
fn line_block(text: &str, a: usize, b: usize) -> (usize, usize) {
    let end = if b > a && text.as_bytes()[b - 1] == b'\n' { b - 1 } else { b };
    (line_start(text, a), line_end(text, end))
}

/// the bytes `[first, stop)` of the edit's text replaced by `replacement`
fn replace_block(edit: &mut TextEdit, first: usize, stop: usize, replacement: &str) {
    edit.text = format!("{}{}{}", &edit.text[..first], replacement, &edit.text[stop..]);
}

/// a selection over the replacement afterwards keeps its direction
fn keep_selection(edit: &mut TextEdit, first: usize, last: usize) {
    if edit.caret >= edit.anchor {
        edit.anchor = first;
        edit.caret = last;
    } else {
        edit.anchor = last;
        edit.caret = first;
    }
}

/// `block` with every line commented by `prefix`, or with it taken off when each non-blank line
/// has it (and the space after it); a comment goes in at the block's smallest indent
fn toggle_comment(prefix: &str, block: &str) -> String {
    let lines: Vec<&str> = block.split('\n').collect();
    let blank = |l: &str| l.trim().is_empty();
    let indent = |l: &str| l.bytes().take_while(|c| *c == b' ' || *c == b'\t').count();
    let body: Vec<&str> = lines.iter().copied().filter(|l| !blank(l)).collect();
    let n = prefix.len();
    let commented = |l: &str| l[indent(l)..].starts_with(prefix);
    let all_commented = !body.is_empty() && body.iter().all(|l| commented(l));
    let col = body.iter().map(|l| indent(l)).min().unwrap_or(usize::MAX);
    lines
        .iter()
        .map(|l| {
            if blank(l) {
                l.to_string()
            } else {
                let i = indent(l);
                if all_commented {
                    let n = if i + n < l.len() && l.as_bytes()[i + n] == b' ' { n + 1 } else { n };
                    format!("{}{}", &l[..i], &l[i + n..])
                } else {
                    format!("{}{} {}", &l[..col], prefix, &l[col..])
                }
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// the innermost bracket pair that holds `[a, b)` and is larger than it, as (open, past the close)
fn enclosing_pair(pairs: &[(usize, usize)], a: usize, b: usize) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    for (o, c) in pairs {
        let (o, c) = (*o, *c);
        if o <= a && b <= c + 1 && (o, c + 1) != (a, b) {
            match best {
                Some((bo, bc)) if bc - bo <= c + 1 - o => {}
                _ => best = Some((o, c + 1)),
            }
        }
    }
    best
}

/// the caret across the bracket beside it, else out to the opener of the pair around it
fn bracket_jump(pairs: &[(usize, usize)], caret: usize) -> Option<usize> {
    for (o, c) in pairs {
        if *o == caret {
            return Some(c + 1);
        } else if c + 1 == caret {
            return Some(*o);
        }
    }
    enclosing_pair(pairs, caret, caret).map(|(o, _)| o)
}

/// what a double click selects at `i`: the word there, else the run of blanks, else the one
/// punctuation character; nothing on a line break
pub fn word_at(text: &str, i: usize) -> (usize, usize) {
    let b = text.as_bytes();
    let n = b.len();
    let class_of = |c: u8| -> i32 {
        if word_char(c) {
            1
        } else if c == b' ' || c == b'\t' {
            2
        } else if c == b'\n' || c == b'\r' {
            0
        } else {
            3
        }
    };
    let pick = if i < n && class_of(b[i]) != 0 {
        Some(i)
    } else if i > 0 && class_of(b[i - 1]) != 0 {
        Some(i - 1)
    } else {
        None
    };
    match pick {
        None => (i, i),
        Some(p) if class_of(b[p]) == 3 => (p, p + 1),
        Some(p) => {
            let k = class_of(b[p]);
            let mut a = p;
            let mut e = p + 1;
            while a > 0 && class_of(b[a - 1]) == k {
                a -= 1;
            }
            while e < n && class_of(b[e]) == k {
                e += 1;
            }
            (a, e)
        }
    }
}


// ---- the retained edit state ----

impl Ui {
    /// The byte of kit text whose caret is nearest `x` points from its start (pxui_text's hit test).
    pub(crate) fn text_caret_at(&mut self, size: Option<i64>, text: &str, x: f32) -> usize {
        let style = kit_style(size.unwrap_or(self.font_size()), 0.0);
        let t = self.text_mut();
        let (key, m) = lay_out(t, &style, 0.0, text);
        t.hit(key, x, m.h / 2.0).min(text.len())
    }

    pub fn text_undo_bytes(&self) -> usize {
        stack_bytes(&self.edit_undo)
            + stack_bytes(&self.edit_redo)
            + self.edit_shelf.iter().map(|(_, _, undo, redo)| stack_bytes(undo) + stack_bytes(redo)).sum::<usize>()
    }

    fn enforce_text_undo(&mut self) {
        loop {
            let bytes = self.text_undo_bytes();
            let dropped = bytes > TEXT_UNDO_BYTE_CAP
                && if !self.edit_shelf.is_empty() {
                    self.edit_shelf.pop();
                    true
                } else if self.edit_undo.len() >= 2 {
                    self.edit_undo.pop();
                    true
                } else {
                    false
                };
            if !dropped {
                // `Ui::bytes` reports the stacks' one running total
                self.text_undo_held = bytes as i64;
                return;
            }
        }
    }

    /// A text starts being edited: the caret at its end, the selection from `anchor`, and nothing of
    /// the text edited before it (scroll, undo stacks, a double click's selection unit).
    pub(crate) fn reset_edit(&mut self, key: i64, text: &str, anchor: usize) {
        self.edit_focus = key;
        self.edit_value = text.to_owned();
        self.edit_caret = text.len();
        self.edit_anchor = anchor;
        self.edit_scroll_x = 0.0;
        self.edit_undo.clear();
        self.edit_redo.clear();
        self.edit_group = -1;
        self.edit_unit = None;
        self.edit_column = -1.0;
        self.enforce_text_undo();
    }

    pub fn load_text_edit(&mut self, key: i64, text: &str) -> TextEdit {
        self.load_text_edit_into(key, text, String::new())
    }

    fn load_text_edit_into(&mut self, key: i64, text: &str, mut buffer: String) -> TextEdit {
        if self.edit_focus != key || self.edit_value != text {
            let left: ShelfEntry = (self.edit_focus, self.edit_value.clone(), self.edit_undo.clone(), self.edit_redo.clone());
            let back = self.edit_shelf.iter().find(|(k, v, _, _)| *k == key && v == text).cloned();
            let mut shelf: Vec<ShelfEntry> = self.edit_shelf.iter().filter(|e| Some(*e) != back.as_ref()).cloned().collect();
            if !(self.edit_undo.is_empty() && self.edit_redo.is_empty()) {
                shelf.insert(0, left);
            }
            shelf.truncate(4);
            self.edit_shelf = shelf;
            self.reset_edit(key, text, text.len());
            if let Some((_, _, undo, redo)) = back {
                self.edit_undo = undo;
                self.edit_redo = redo;
            }
            self.enforce_text_undo();
        }
        // a screen reader's Select (AT-SPI SetCaretOffset/SetSelection) lands in whichever field loads its edit
        if let Some((anchor, caret)) = self.a11y_select_of(key) {
            let byte = |chars: u32| text.char_indices().nth(chars as usize).map_or(text.len(), |(b, _)| b);
            (self.edit_anchor, self.edit_caret) = (byte(anchor), byte(caret));
        }
        buffer.clear();
        buffer.push_str(text);
        TextEdit { text: buffer, caret: self.edit_caret, anchor: self.edit_anchor }
    }

    pub(crate) fn save_text_edit(&mut self, edit: &TextEdit) {
        if self.edit_value != edit.text { self.edit_value.clone_from(&edit.text); }
        self.edit_caret = edit.caret;
        self.edit_anchor = edit.anchor;
    }

    // ---- undo inside the focused text: snapshots before each change, consecutive typing as one

    pub fn remember(&mut self, edit: &TextEdit, typing: bool) {
        if !(typing && edit.caret == edit.anchor && self.edit_group == edit.caret as i64) {
            self.edit_undo.insert(0, (edit.text.clone(), edit.caret, edit.anchor));
            self.edit_undo.truncate(200);
            self.edit_redo.clear();
            self.enforce_text_undo();
        }
        self.edit_group = -1;
    }

    fn undo_text(&mut self, edit: &mut TextEdit, redo: bool) -> bool {
        let stack = if redo { &mut self.edit_redo } else { &mut self.edit_undo };
        if stack.is_empty() {
            return false;
        }
        let (text, caret, anchor) = stack.remove(0);
        let now = (edit.text.clone(), edit.caret, edit.anchor);
        if redo {
            self.edit_undo.insert(0, now);
        } else {
            self.edit_redo.insert(0, now);
        }
        edit.text = text;
        edit.caret = caret;
        edit.anchor = anchor;
        self.edit_group = -1;
        self.enforce_text_undo();
        true
    }

    /// an exhausted stack hands the key to the host's own undo (`passed_undo`)
    fn pass_undo(&mut self, edit: &mut TextEdit, redo: bool) -> bool {
        if self.undo_text(edit, redo) {
            return true;
        }
        self.set_passed_undo(if redo { UndoKind::Redo } else { UndoKind::Undo });
        false
    }

    /// A press places the caret (Shift extends the selection), a double click selects the word
    /// there, a triple click the `line_of` range; the drag that follows extends by the same unit.
    pub(crate) fn press_select(&mut self, edit: &mut TextEdit, shift: bool, clicks: i64, line_of: &dyn Fn(&TextEdit, usize) -> (usize, usize), at: usize) {
        if clicks < 2 {
            edit.caret = at;
            if !shift {
                edit.anchor = at;
            }
            self.edit_unit = None;
        } else {
            let (a, b) = if clicks == 2 { word_at(&edit.text, at) } else { line_of(edit, at) };
            edit.anchor = a;
            edit.caret = b;
            self.edit_unit = Some((a, b, clicks));
        }
    }

    pub(crate) fn drag_select(&mut self, edit: &mut TextEdit, line_of: &dyn Fn(&TextEdit, usize) -> (usize, usize), at: usize) {
        match self.edit_unit {
            None => edit.caret = at,
            Some((a, b, clicks)) => {
                let (c, d) = if clicks == 2 { word_at(&edit.text, at) } else { line_of(edit, at) };
                if c < a {
                    edit.anchor = b;
                    edit.caret = c;
                } else {
                    edit.anchor = a;
                    edit.caret = b.max(d);
                }
            }
        }
    }

    pub(crate) fn point_text_caret(&mut self, size: Option<i64>, edit: &mut TextEdit, signal: &Signal, shift: bool, x: f32, right: f32) {
        let scroll = self.edit_scroll_x;
        let at = |ui: &mut Ui, px: f32, text: &str| ui.text_caret_at(size, text, 0.0f32.max(right.min(px) - x + scroll));
        let line_of = |e: &TextEdit, _: usize| (0, e.text.len());
        if signal.pressed {
            let a = at(self, signal.press_point.0, &edit.text);
            self.press_select(edit, shift, signal.clicks, &line_of, a);
        }
        if signal.dragging || (signal.held && signal.pointer != signal.press_point) {
            let px = signal.pointer.0;
            if px < x {
                self.edit_scroll_x = 0.0f32.max(self.edit_scroll_x - 1.0f32.max(24.0f32.min((x - px) * 0.25)));
            } else if px > right {
                self.edit_scroll_x += 1.0f32.max(24.0f32.min((px - right) * 0.25));
            }
            let scroll = self.edit_scroll_x;
            let a = self.text_caret_at(size, &edit.text, 0.0f32.max(right.min(px) - x + scroll));
            self.drag_select(edit, &line_of, a);
        }
    }

    /// The keys every text widget shares, as the host binds them. On Apple: Command-A/C/X/V,
    /// Command-Z and Shift-Command-Z (or Command-Y) undo and redo, Option-arrows move by words,
    /// Command-arrows and Home/End by lines, Option-Backspace/Delete take a word,
    /// Command-Backspace/Delete the line to the caret. Elsewhere Ctrl replaces Command and Option
    /// (Ctrl-arrows and Ctrl-Backspace move and delete by words; Home and End are the lines).
    /// Returns whether the text changed.
    pub(crate) fn edit_text_event(&mut self, multiline: bool, edit: &mut TextEdit, accept: &dyn Fn(&str) -> bool, modifiers: &[Key], event: &Event) -> bool {
        let command = self.command_modifiers(modifiers);
        let shift = modifiers.contains(&Key::Shift);
        // Command moves and deletes by lines on Apple only; elsewhere Ctrl moves by words
        let line = self.line_modifiers(modifiers);
        let word = input::word_held(self.platform(), modifiers);
        let selected = |e: &TextEdit| e.caret != e.anchor;
        let clip = clipboard_command(command, event);
        match clip {
            Some('a') => {
                edit.anchor = 0;
                edit.caret = edit.text.len();
                return false;
            }
            Some('c') => {
                let (start, stop) = text_selection(edit);
                let copied = if selected(edit) { &edit.text[start..stop] } else { &edit.text };
                self.copy(copied);
                return false;
            }
            Some('x') => {
                let (start, stop) = text_selection(edit);
                self.copy(if selected(edit) { &edit.text[start..stop] } else { &edit.text });
                // the cut is one undo step: text the clipboard did not take comes back with Undo
                self.remember(edit, false);
                if selected(edit) {
                    replace_text(edit, "");
                } else {
                    edit.text.clear();
                    edit.caret = 0;
                    edit.anchor = 0;
                }
                return true;
            }
            Some('v') => {
                self.request_paste();
                return false;
            }
            Some('z') => return self.pass_undo(edit, shift),
            Some('y') => return self.pass_undo(edit, true),
            _ => {}
        }
        match event {
            // a field of one line takes a copied line without its break
            Event::Paste(text) => {
                let text: String = if multiline { text.clone() } else { text.chars().filter(|c| *c != '\n' && *c != '\r').collect() };
                if accept(&text) {
                    self.remember(edit, false);
                    replace_text(edit, &text);
                    true
                } else {
                    false
                }
            }
            Event::TextInput(text) if accept(text) => {
                let typing = !text.chars().any(|c| c == ' ' || c == '\t' || c == '\n');
                self.remember(edit, typing);
                replace_text(edit, text);
                if typing {
                    self.edit_group = edit.caret as i64;
                }
                true
            }
            Event::KeyPressed(Key::Backspace) => {
                let target = if line {
                    line_start(&edit.text, edit.caret)
                } else if word {
                    word_left(&edit.text, edit.caret)
                } else {
                    previous_utf8(&edit.text, edit.caret)
                };
                self.delete_to(edit, target)
            }
            Event::KeyPressed(Key::Delete) => {
                let target = if line {
                    line_end(&edit.text, edit.caret)
                } else if word {
                    word_right(&edit.text, edit.caret)
                } else {
                    next_utf8(&edit.text, edit.caret)
                };
                self.delete_to(edit, target)
            }
            Event::KeyPressed(Key::ArrowLeft) => {
                let target = if line {
                    line_start(&edit.text, edit.caret)
                } else if word {
                    word_left(&edit.text, edit.caret)
                } else if selected(edit) && !shift {
                    text_selection(edit).0
                } else {
                    previous_utf8(&edit.text, edit.caret)
                };
                move_caret(edit, target, shift);
                false
            }
            Event::KeyPressed(Key::ArrowRight) => {
                let target = if line {
                    line_end(&edit.text, edit.caret)
                } else if word {
                    word_right(&edit.text, edit.caret)
                } else if selected(edit) && !shift {
                    text_selection(edit).1
                } else {
                    next_utf8(&edit.text, edit.caret)
                };
                move_caret(edit, target, shift);
                false
            }
            Event::KeyPressed(Key::Home) => {
                let t = line_start(&edit.text, edit.caret);
                move_caret(edit, t, shift);
                false
            }
            Event::KeyPressed(Key::End) => {
                let t = line_end(&edit.text, edit.caret);
                move_caret(edit, t, shift);
                false
            }
            _ => false,
        }
    }

    /// the selection, else from the caret to `target`
    fn delete_to(&mut self, edit: &mut TextEdit, target: usize) -> bool {
        let selected = edit.caret != edit.anchor;
        if !selected && target == edit.caret {
            false
        } else {
            self.remember(edit, false);
            if !selected {
                edit.anchor = target;
            }
            replace_text(edit, "");
            true
        }
    }
}

pub(crate) fn move_caret(edit: &mut TextEdit, target: usize, shift: bool) {
    edit.caret = target;
    if !shift {
        edit.anchor = target;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn paint_text_edit(
    paint: &mut Paint,
    size: Option<i64>,
    right: bool,
    inset: f32,
    marks: Option<(Color, Color)>,
    control: IRect,
    y: i64,
    composition: &str,
    edit: &TextEdit,
) {
    paint_text_edit_ref(paint,size,right,inset,marks,control,y,composition,&EditRef { text:&edit.text,caret:edit.caret,anchor:edit.anchor });
}

struct EditRef<'a> { text:&'a str,caret:usize,anchor:usize }

#[allow(clippy::too_many_arguments)]
fn paint_text_edit_ref(paint:&mut Paint,size:Option<i64>,right:bool,inset:f32,marks:Option<(Color,Color)>,control:IRect,y:i64,composition:&str,edit:&EditRef<'_>) {
    let (cx, cy, cw, ch) = control;
    // the selection and caret colours (the field look's); `None` hides the caret
    let (tint, caret) = match marks {
        Some((selection, caret)) => (selection, Some(caret)),
        None => (theme::tint(&paint.theme), None),
    };
    let before = &edit.text[..edit.caret];
    let visible = 1.max(cw - 4) as f32;
    let full = paint.text_width(size, &edit.text);
    let scroll = 0.0f32.max(paint.edit_scroll_x.min(0.0f32.max(full - visible)));
    let caret_width = paint.caret_x(size, &edit.text, edit.caret);
    let scroll = if caret_width < scroll {
        caret_width
    } else if caret_width - scroll > visible {
        caret_width - visible
    } else {
        scroll
    };
    paint.edit_scroll_x = scroll;
    // a number keeps its right edge while it is edited, until it outgrows the field
    let origin = if right && full <= visible { (cx + cw - 2) as f32 - full } else { cx as f32 + inset };
    let text_x = origin - scroll;
    let caret_x = text_x + caret_width;
    paint.input_region(caret_x - cx as f32, cx as f32, cy as f32, cw as f32, ch as f32);
    let previous_clip = paint.clip_rect;
    paint.clip_rect = intersect(previous_clip, Rect::new(cx as f32, cy as f32, cw as f32, ch as f32));
    if edit.caret != edit.anchor {
        let (start, stop) = (edit.caret.min(edit.anchor),edit.caret.max(edit.anchor));
        let mut spans = [(0.0, 0.0); 4];
        let mut n = 0;
        paint.selection(size, &edit.text, start, stop, &mut |x0, w| {
            if n < spans.len() {
                spans[n] = (x0, w);
                n += 1;
            }
        });
        for &(x0, w) in &spans[..n] {
            paint.fill(text_x + x0, cy as f32 + 0.5, w, 1.max(ch - 2) as f32, tint);
        }
    }
    if composition.is_empty() {
        paint.text_with((text_x, y as f32), size, 0.0, None, &edit.text);
    } else {
        paint.text_with((text_x, y as f32), size, 0.0, None, &before);
        paint.text_with((caret_x, y as f32), size, 0.0, None, composition);
        let cwid = paint.text_width(size, composition);
        paint.text_with((caret_x + cwid, y as f32), size, 0.0, None, &edit.text[edit.caret..]);
    }
    // the caret is the line of the text it sits in: 14 of a 20-point field at body size
    let (caret_y, caret_h) = match size {
        None => (cy as f32 + 2.5, 14.0),
        Some(size) => {
            let h = size as f32 * 1.1;
            (cy as f32 + (ch as f32 - h) / 2.0, h)
        }
    };
    if let Some(color) = caret {
        paint.fill(caret_x, caret_y, 1.0, caret_h, color);
    }
    paint.clip_rect = previous_clip;
}

pub struct NumParams {
    pub size: Option<i64>,
    pub control: Option<IRect>,
    pub click_to_edit: bool,
    pub edit_request: bool,
    pub alt_to_edit: bool,
    pub align_right: bool,
}

impl Default for NumParams {
    fn default() -> NumParams {
        NumParams { size: None, control: None, click_to_edit: false, edit_request: false, alt_to_edit: false, align_right: false }
    }
}

impl Ui {
    /// Numeric label editing shared by float and integer sliders. The retained state records
    /// validity; `parse` validates the edit buffer. Returns the committed value and whether an
    /// editor is open.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn numeric_editor(
        &mut self,
        p: &NumParams,
        accept: &dyn Fn(&str) -> bool,
        row: &UiBox,
        signal: &Signal,
        keys: &[(Event, Vec<Key>)],
        current: &dyn Fn() -> String,
        parse: &dyn Fn(&str) -> Option<String>,
    ) -> (Option<String>, bool) {
        numeric_editor_body(self, p, accept, row, signal, keys, current, parse)
    }
}

impl Ui {
    /// Compact field. Numeric sliders follow the pointer with `slide`; Option-click, a
    /// double-click or `edit` opens text entry. `scrub` handles fields without a track. `left`
    /// aligns text values to the left (default false for numeric fields). `lead` and `trail` frame
    /// the value at the left and right with 6 points between; `line` is the idle hairline's colour;
    /// `bare` leaves the hairline out until the field is edited or invalid; `placeholder` is shown
    /// in ink-3 where an empty value would be. Only valid text commits; the boolean reports an open
    /// text editor.
    #[allow(clippy::too_many_arguments)]
    pub fn value_field(&mut self, at: (f32, f32), w: f32, h: f32, o: &ValueFieldOpts, valid: &dyn Fn(&str) -> bool, label: &str, value: &str) -> (String, bool) {
        value_field_body(self, at, w, h, o, valid, label, value)
    }

    /// A `kept_drag` slider field held and dragged by the left button this frame, its view kept: the
    /// fraction of its track under the pointer (what `slide` takes) and the pointer's points from
    /// the press (what `scrub` takes), as its build would read them. `None` on any other frame (a
    /// press, a release, typing: its view builds and the field reads them itself).
    pub fn field_drag(&self, field: &UiBox) -> Option<(f32, f32)> {
        if !self.live(field) || self.box_flags(field) & DRAG_FREE == 0 {
            return None;
        }
        let signal = self.signal(field);
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
            BoxOpts::default().flags(if enabled { CLICKABLE | TAB_STOP } else { NO_FLAGS }).at((x, y_buttons)).w(Size::Px(w)).h(Size::Px(self.control_h())),
        );
        let signal = self.signal(&b);
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
            let font_size = self.font_size();
            let at = 1.0 + pad + mono_width(0.0, font_size, label) + 6.0;
            let hw = self.text_width(Some(key_look.font.points(font_size)), hint);
            let n = self.node_keyed(int_key(b.box_key, 0), Node { w: Size::Px(hw.max(1.0)), h: Size::Px(self.control_h()), at: Some((at, 0.0)).into(), look: key_look, ..Node::DEFAULT });
            self.text(&n, hint);
            self.leave(inner);
        }
        self.leave(saved);
    }

    /// A single-line field. A click places the caret and Shift-click or a drag extends the
    /// selection, a double click selects the word (a triple the line), the word keys move and
    /// delete by words, Command-arrows (Apple) and Home/End move by lines, Command-A/C/X/V select
    /// all, copy, cut and paste, and Command-Z, Shift-Command-Z (or Command-Y) undo and redo the
    /// focused text. The result is the new value on the frames that change it.
    pub fn text_field(&mut self, text: &str, value: &str) -> Option<String> {
        text_field_body(self, text, value)
    }

    /// A one-line search field at `at` (the first string is its key, the second its value) with a
    /// hairline: the new value and what its keys asked, Enter or Down `Next`, Shift-Enter or Up
    /// `Previous`, Escape `Close`. `grab` takes the keyboard for it.
    #[allow(clippy::too_many_arguments)]
    pub fn line_field(&mut self, at: (f32, f32), w: f32, h: f32, grab: bool, placeholder: &str, label: &str, value: &str) -> (String, LineStep) {
        let field_box = self.box_(label, BoxOpts::default().flags(CLICKABLE | FOCUSABLE | BLOCKING | FOCUS_MARK).at(at).w(Size::Px(w)).h(Size::Px(h)));
        let field = field_box;
        if grab {
            self.focus_box(&field);
        }
        let signal = self.signal(&field);
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
            let font_size = self.font_size();
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

pub fn context_clicked(signal: &Signal) -> bool {
    let ((px, py), (rx, ry)) = (signal.press_point, signal.release_point);
    signal.released && signal.button == Some(MouseButton::RightButton) && (px - rx) * (px - rx) + (py - ry) * (py - ry) < 16.0
}

// ------------------------------------------------------------------ text area

type Rows = Rc<Vec<(usize, usize, usize)>>;
type Notes = Vec<(usize, Option<(i64, i64)>, String)>;

fn start_of(rows: &Rows, i: usize) -> usize {
    rows[i].0
}

fn stop_of(rows: &Rows, i: usize) -> usize {
    rows[i].1
}

fn row_text(text: &str, rows: &Rows, i: usize) -> String {
    text[start_of(rows, i)..stop_of(rows, i)].to_owned()
}

/// the row holding byte `index`
pub fn row_at(rows: &[(usize, usize, usize)], index: usize) -> usize {
    let (mut low, mut high) = (0usize, rows.len() - 1);
    while low < high {
        let middle = (low + high + 1) / 2;
        if rows[middle].0 <= index {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    low
}

fn slot_of(notes: &Notes, i: usize) -> usize {
    i + notes.iter().filter(|(after, _, _)| *after < i).count()
}

fn row_at_slot(notes: &Notes, count: usize, k: i64) -> usize {
    if notes.is_empty() {
        0.max((count as i64 - 1).min(k)) as usize
    } else {
        let mut i = 0usize;
        while i + 1 < count && (slot_of(notes, i + 1) as i64) <= k {
            i += 1;
        }
        i
    }
}

fn gutter_of(char_w: f32, lines: usize) -> f32 {
    36.0 + char_w * 0.max(lines.to_string().len() as i64 - 3) as f32
}

fn clamp_pos(v: i64) -> usize {
    v.max(0) as usize
}

/// How a text area wraps: the gutter width follows the number of logical lines, the wrap width
/// follows the gutter.
#[derive(Clone, Copy)]
struct Wrap {
    wrap: bool,
    w: f32,
    char_w: f32,
}

#[derive(Default)]
struct AreaState {
    selected: usize,
    chosen: bool,
    closed: bool,
    submitted: bool,
    changed: bool,
    typed: bool,
}

type Scrubbing = Option<(usize, String, f32, bool)>;

impl Ui {
    /// The rows of a text: (start, stop, logical line) with `stop` exclusive and before the
    /// newline. Without `cols` a row is a line; with it a line continues every `cols` characters.
    /// An unchanged text costs no walk: the handle keeps the rows of the last few texts it laid
    /// out, least recently used first out past `set_text_panes` (8 by default: the panes on screen).
    pub fn text_rows(&mut self, cols: Option<usize>, text: &str) -> Rows {
        if let Some(k) = self.text_rows_seen.iter().position(|(c, t, _)| *c == cols && t == text) {
            self.text_rows_seen[..=k].rotate_right(1);
            return self.text_rows_seen[0].2.clone();
        }
        let n = text.len();
        let mut rows: Vec<(usize, usize, usize)> = Vec::new();
        let mut start = 0usize;
        let mut logical = 0usize;
        loop {
            let stop = text[start..].find('\n').map(|i| start + i).unwrap_or(n);
            let mut from = start;
            loop {
                match cols {
                    None => {
                        rows.push((from, stop, logical));
                        break;
                    }
                    Some(cols) => {
                        let mut i = from;
                        let mut k = 1.max(cols);
                        while k > 0 && i < stop {
                            i += text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                            k -= 1;
                        }
                        if i >= stop {
                            rows.push((from, stop, logical));
                            break;
                        }
                        rows.push((from, i, logical));
                        from = i;
                    }
                }
            }
            if stop < n {
                start = stop + 1;
                logical += 1;
            } else {
                break;
            }
        }
        let rows = Rc::new(rows);
        self.text_rows_seen.insert(0, (cols, text.to_owned(), rows.clone()));
        self.text_rows_seen.truncate(self.text_panes);
        rows
    }

    /// How many texts `text_rows` and `text_colors` keep (at least 1): the text panes a UI shows at
    /// once. Each keeps a copy of its text.
    pub fn set_text_panes(&mut self, n: usize) {
        self.text_panes = n.max(1);
        self.text_rows_seen.truncate(self.text_panes);
        self.text_colors_seen.truncate(self.text_panes);
    }

    fn area_rows(&mut self, cfg: Wrap, text: &str) -> Rows {
        let logical = 1 + text.bytes().filter(|c| *c == b'\n').count();
        let cols = if !cfg.wrap {
            None
        } else {
            let gutter = gutter_of(cfg.char_w, logical);
            Some(((cfg.w - gutter - 16.0).max(1.0) / cfg.char_w.max(1.0)).floor() as usize)
        };
        self.text_rows(cols, text)
    }

    /// A language's colours for a text, kept like its rows: one list a pane on screen. A theme is
    /// part of the colours. Least recently used first out past `set_text_panes`.
    pub fn text_colors(&mut self, language: &Language, text: &str) -> Rc<Vec<(i64, i64, Color)>> {
        let theme = self.tokens();
        if let Some(k) = self.text_colors_seen.iter().position(|(seen, known, _)| *seen == theme && known == text) {
            self.text_colors_seen[..=k].rotate_right(1);
            return self.text_colors_seen[0].2.clone();
        }
        let colors = Rc::new((language.colorize)(text));
        self.text_colors_seen.insert(0, (theme, text.to_owned(), colors.clone()));
        self.text_colors_seen.truncate(self.text_panes);
        colors
    }

    /// A box at the root, laid out and painted after every pane (like a tooltip), so a popup under
    /// a caret is never clipped by its pane.
    fn overlay_box(&mut self, flags: Flags, at: (f32, f32), w: f32, h: f32, label: &str) -> UiBox {
        let saved = self.enter_root();
        let overlay = self.box_(label, BoxOpts::default().flags(flags).w(Size::Px(w)).h(Size::Px(h)).at(at));
        self.push_overlay(overlay.index);
        self.leave(saved);
        overlay
    }

    fn accept_completion(&mut self, edit: &mut TextEdit, suggest: &UiBox, st: &mut AreaState, c: &Completion) {
        self.remember(edit, false);
        let (a, b) = c.replace;
        let a = (a.max(0) as usize).min(edit.text.len());
        let b = (b.max(0) as usize).min(edit.text.len());
        edit.text = format!("{}{}{}", &edit.text[..a], c.insert, &edit.text[b..]);
        edit.caret = a + c.insert.len();
        edit.anchor = edit.caret;
        self.set_state(suggest, 0);
        st.closed = true;
    }

    /// a dragged number: the literal at the press follows the pointer
    fn apply_scrub(&mut self, scrubbing: &Scrubbing, on_scrub: &Option<Rc<dyn Fn(ScrubPhase)>>, suggest: &UiBox, on_scrub_edit: &Option<Rc<dyn Fn((i64, i64), &str)>>, edit: &mut TextEdit) {
        if let Some((start, literal, dx, first)) = scrubbing {
            let start = *start;
            if start <= edit.text.len() && !literal.is_empty() {
                // the whole drag is one step of the editor's undo
                if *first {
                    self.remember(edit, false);
                }
                let mut stop = start;
                let bytes = edit.text.as_bytes();
                while stop < bytes.len() && numeric_character(bytes[stop] as char) {
                    stop += 1;
                }
                let coarse = match self.input_frame() {
                    Some(frame) => frame.input.keys.contains(&Key::Shift),
                    None => false,
                };
                let next = scrubbed(literal, *dx, coarse);
                if let Some(f) = on_scrub_edit {
                    f((start as i64, stop as i64), &next);
                }
                edit.text = format!("{}{}{}", &edit.text[..start], next, &edit.text[stop..]);
                edit.caret = start + next.len();
                edit.anchor = edit.caret;
                self.set_state(suggest, 0);
                if let Some(f) = on_scrub {
                    f(ScrubPhase::Live);
                }
            }
        }
    }

    /// the keys of a focused text area: completion, pairs, indent, line edits, movement, typing
    #[allow(clippy::too_many_arguments)]
    fn handle_keys(
        &mut self,
        body: &UiBox,
        cfg: Wrap,
        page: i64,
        language: &Option<Language>,
        suggest: &UiBox,
        open: i64,
        items: &[Completion],
        shown: usize,
        st: &mut AreaState,
        edit: &mut TextEdit,
    ) {
        handle_keys_body(self, body, cfg, page, language, suggest, open, items, shown, st, edit)
    }

    fn change(&mut self, edit: &TextEdit, st: &mut AreaState) {
        st.changed = true;
        self.remember(edit, false);
    }

    fn vertical(&mut self, edit: &mut TextEdit, rows: &Rows, count: usize, line: usize, step: i64, shift: bool) {
        let target = line as i64 + step;
        let x = if self.edit_column >= 0.0 {
            self.edit_column
        } else {
            let s = start_of(rows, line);
            self.caret_x(None, &row_text(&edit.text, rows, line), edit.caret - s)
        };
        self.edit_column = x;
        if target < 0 {
            move_caret(edit, 0, shift);
        } else if target as usize >= count {
            move_caret(edit, edit.text.len(), shift);
        } else {
            let t = target as usize;
            let rt = row_text(&edit.text, rows, t);
            let at = start_of(rows, t) + self.text_caret_at(None, &rt, x);
            move_caret(edit, at, shift);
        }
    }

    /// A text area that also reports Command- or Ctrl-Enter pressed in it this frame (the host's
    /// "apply"). Tab inserts two spaces and Shift-Tab takes up to two leading spaces off the line
    /// (the editor keeps Tab instead of moving the focus). With `wrap` a long line continues on the
    /// next row, so nothing scrolls sideways; the gutter numbers logical lines and `errors` are
    /// logical lines. A `messages` entry (1-based line, the wrong byte span, the text) is a row of
    /// its own in the error colour under that line, with the span underlined. `language` colours
    /// the text, lights the bracket pair at the caret, indents after Enter, pairs brackets,
    /// completes the token at the caret and describes the token under the pointer. A numeric
    /// literal of the language dragged sideways follows the pointer (a float by a tenth of its last
    /// decimal place per point, an integer by one per six points, Shift ten times faster): `on_scrub
    /// Live` is called on each frame the returned text changed that way and `Done` when the drag
    /// ends. `reveal` scrolls the offset into view once per (offset, text length, `reveal_nonce`);
    /// `select` (anchor, caret) takes the keyboard for the area and places the selection; `splice`
    /// is a list of (start, stop, replacement) byte ranges of the given text applied together as
    /// one undo step before `select`.
    #[allow(clippy::too_many_arguments)]
    pub fn text_area_submit(&mut self, at: (f32, f32), w: f32, h: f32, o: &TextAreaOpts, label: &str, text: &str) -> (String, bool) {
        text_area_submit_body(self, at, w, h, o, label, text)
    }
}

struct AreaPaint {
    colors: Rc<Vec<(i64, i64, Color)>>,
    composition: String,
    messages: Vec<(i64, Option<(i64, i64)>, String)>,
    chips: Vec<(i64, i64, Color)>,
    hovered_number: Option<(usize, usize)>,
    scrubbing: bool,
    matched: Option<(usize, usize)>,
    spans: Vec<(i64, i64)>,
    errors: Vec<i64>,
    notes: Notes,
    char_w: f32,
    final_text: String,
    rows: Rows,
    count: usize,
    focused: bool,
    edit: TextEdit,
    row: f32,
    pad: f32,
    offset: f32,
    horizontal: f32,
    gutter: f32,
    theme: Tokens,
    /// the area's ground, gutter rule (`border`), text, selection and caret
    look: Look,
}

impl AreaPaint {
    fn paint(&self, paint: &mut Paint, Rect { x: bx, y: by, w: bw, h: bh }: Rect) {
        let theme = self.theme;
        let final_text = self.final_text.as_str();
        let rows = &self.rows;
        let row = self.row;
        let char_w = self.char_w;
        let (gutter, offset, pad) = (self.gutter, self.offset, self.pad);
        let edit = &self.edit;
        let look = self.look;
        paint.fill(bx, by, bw, bh, look.fill);
        let previous = paint.clip_rect;
        let text_x = bx + gutter - self.horizontal;
        paint.fill(bx + gutter, by, 1.0, bh, look.border);
        let small = paint.label_size();
        let first = 0.max(((offset - pad) / row).floor() as i64 - self.notes.len() as i64);
        let last = (self.count as i64 - 1).min(((offset + bh) / row).floor() as i64);
        let caret_line = row_at(rows, edit.caret);
        let selected = self.focused && edit.caret != edit.anchor;
        let (s0, s1) = text_selection(edit);
        let width = |paint: &mut Paint, t: &str| paint.text_width(None, t);
        // where the caret before byte `i` of the row `ls..le` sits, from the row's start
        let at = |paint: &mut Paint, ls: usize, le: usize, i: usize| paint.caret_x(None, &final_text[ls..le], i.min(le) - ls);
        let clip = |paint: &mut Paint, x: f32, w: f32| paint.clip_rect = intersect(previous, Rect::new(x, by, w, bh));
        let band = |paint: &mut Paint, line: usize, start: usize, stop: usize, color: Color, y: f32| {
            let ls = start_of(rows, line);
            let le = stop_of(rows, line);
            // a selection runs on past the end of a row that ends a line
            let newline = le < final_text.len() && final_text.as_bytes()[le] == b'\n';
            let start = start.max(ls);
            let stop = stop.min(if newline { le + 1 } else { le });
            if start < stop || (start <= le && stop > le) {
                let x0 = at(paint, ls, le, start);
                let x1 = if stop > le { at(paint, ls, le, le) + char_w } else { at(paint, ls, le, stop) };
                paint.fill(text_x + x0, y, 1.0f32.max(x1 - x0), row, color);
            }
        };
        // the colours are sorted and the rows come in order: one walk of them, whatever the rows
        let mut colors_ahead = 0usize;
        let colors = &*self.colors;
        let mut line = first;
        while line <= last {
            let lu = line as usize;
            let y = by - offset + pad + slot_of(&self.notes, lu) as f32 * row;
            let text_y = y + 0.max((paint.font_size + (paint.font_size + 1) / 2 - paint.font_size) / 2) as f32;
            let logical = rows[lu].2;
            let starts_line = lu == 0 || rows[lu - 1].2 != logical;
            clip(paint, bx, bw);
            let wrong = self.errors.contains(&(logical as i64 + 1));
            let current = self.focused && lu == caret_line;
            if wrong {
                paint.fill(bx, y, bw, row, color::with_alpha(theme::invalid(), 18));
            } else if current {
                paint.fill(bx, y, bw, row, theme::faint_border(&theme));
            }
            if starts_line {
                let number = (logical + 1).to_string();
                let number_x = bx + gutter - 10.0 - paint.text_width(Some(small), &number);
                if wrong {
                    paint.fill(number_x - 10.0, y + row / 2.0 - 3.0, 6.0, 6.0, theme::invalid());
                }
                paint.text_s(
                    (number_x, text_y + 1.0),
                    small,
                    if wrong {
                        theme::invalid()
                    } else if current {
                        theme.foreground
                    } else {
                        theme::ink_3(&theme)
                    },
                    &number,
                );
            }
            clip(paint, bx + gutter, bw - gutter);
            for (a, b) in &self.spans {
                band(paint, lu, clamp_pos(*a), clamp_pos(*b), look.selection, y);
            }
            if selected {
                band(paint, lu, s0, s1, look.selection, y);
            }
            // the bracket pair at the caret: an accent outline
            if let Some((o, k)) = self.matched {
                for b in [o, k] {
                    let ls = start_of(rows, lu);
                    let le = stop_of(rows, lu);
                    if b >= ls && b < le {
                        // the glyph's box (6.5 x 14) with a 1-point outline outside it: 8.5 x 16
                        let x = text_x + at(paint, ls, le, b) - 1.0;
                        paint.frame(x, y + 2.0, char_w + 2.0, 16.0, theme.accent);
                    }
                }
            }
            // a number under the pointer (or being dragged) wears an accent underline
            if let Some((a, b)) = self.hovered_number {
                let ls = start_of(rows, lu);
                let le = stop_of(rows, lu);
                if a >= ls && a < le {
                    let x0 = at(paint, ls, le, a);
                    let x1 = at(paint, ls, le, b);
                    paint.fill(
                        text_x + x0,
                        y + row - 4.0,
                        1.0f32.max(x1 - x0),
                        2.0,
                        if self.scrubbing { theme.accent } else { color::with_alpha(theme.accent, 160) },
                    );
                }
            }
            let ls = start_of(rows, lu);
            let le = stop_of(rows, lu);
            // a colour literal wears its colour as a bar under it
            for (a, b, c) in &self.chips {
                let (a, b) = (clamp_pos(*a), clamp_pos(*b));
                if a >= ls && a < le && b <= final_text.len() {
                    let x0 = at(paint, ls, le, a);
                    let w1 = at(paint, ls, le, b) - x0;
                    paint.fill(text_x + x0, y + row - 5.0, w1, 4.0, *c);
                }
            }
            // the wrong span of a diagnostic is underlined in the error colour
            for (_, span, _) in &self.messages {
                if let Some((a, b)) = span {
                    let (a, b) = (clamp_pos(*a), clamp_pos(*b));
                    if a < le && b > ls && b > a && b <= final_text.len() {
                        let a = a.max(ls);
                        let b = b.min(le);
                        let x0 = at(paint, ls, le, a);
                        let w1 = 1.0f32.max(at(paint, ls, le, b) - x0);
                        paint.fill(text_x + x0, y + row - 3.0, w1, 1.0, theme::invalid());
                    }
                }
            }
            let line_str = &final_text[ls..le];
            if self.focused && lu == caret_line && !self.composition.is_empty() {
                let before = &final_text[ls..edit.caret];
                let caret_x = text_x + at(paint, ls, le, edit.caret);
                paint.text_c((text_x, text_y), theme.foreground, before);
                paint.text_c((caret_x, text_y), theme.foreground, &self.composition);
                let cw = width(paint, &self.composition);
                paint.text_c((caret_x + cw, text_y), theme.foreground, &line_str[before.len()..]);
            } else if colors.is_empty() {
                paint.text_c((text_x, text_y), theme.foreground, line_str);
            } else {
                // the row in coloured runs, the gaps in the foreground colour
                while colors_ahead < colors.len() && (colors[colors_ahead].1 as usize) <= ls {
                    colors_ahead += 1;
                }
                let mut pos = ls;
                let mut k = colors_ahead;
                let run = |paint: &mut Paint, a: usize, b: usize, c: Color| {
                    if a < b {
                        let ax = text_x + at(paint, ls, le, a);
                        paint.text_c((ax, text_y), c, &final_text[a..b]);
                    }
                };
                while k < colors.len() && (colors[k].0 as usize) < le {
                    let (a, b, c) = colors[k];
                    let a = (a as usize).max(ls);
                    let b = (b as usize).min(le);
                    if a >= b || a < pos {
                        k += 1;
                        continue;
                    }
                    run(paint, pos, a, theme.foreground);
                    run(paint, a, b, c);
                    pos = b;
                    k += 1;
                }
                run(paint, pos, le, theme.foreground);
            }
            line += 1;
        }
        // the message rows: the diagnostic at the label size, under the indent of its line
        clip(paint, bx + gutter, bw - gutter);
        for (k, (after, _, message)) in self.notes.iter().enumerate() {
            let y = by - offset + pad + (*after + 1 + k) as f32 * row;
            if y + row > by && y < by + bh {
                let ls = start_of(rows, *after);
                let mut indent = 0;
                while ls + indent < final_text.len() && final_text.as_bytes()[ls + indent] == b' ' {
                    indent += 1;
                }
                let iw = width(paint, &" ".repeat(indent));
                let ty = paint.text_top(Some(small), y, row);
                paint.text_s((text_x + iw, ty), small, theme::invalid(), message);
            }
        }
        clip(paint, bx + gutter, bw - gutter);
        if self.focused {
            let y = by - offset + pad + slot_of(&self.notes, caret_line) as f32 * row;
            let (ls, le) = (start_of(rows, caret_line), stop_of(rows, caret_line));
            let caret_x = text_x + at(paint, ls, le, edit.caret);
            paint.input_region(caret_x - bx, bx, y, bw, row);
            paint.fill(caret_x.round(), y + 2.0, 1.0, row - 4.0, look.caret);
        }
        paint.clip_rect = previous;
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
    pub fn picker(&mut self, o: &PickerOpts, label: &str, query: &str, rows_of: &dyn Fn(&str) -> Vec<(String, String)>) -> (String, Pick) {
        picker_body(self, o, label, query, &mut |query, rows| *rows = rows_of(query))
    }

    /// `picker` that asks for its rows by filling a vector it keeps between frames: `fill(query, rows)` replaces `rows`
    /// with the rows for `query`, reusing the strings already in it, so a settled picker copies no row into new storage.
    pub fn picker_filled(&mut self, o: &PickerOpts, label: &str, query: &str, fill: &mut dyn FnMut(&str, &mut Vec<(String, String)>)) -> (String, Pick) {
        picker_body(self, o, label, query, fill)
    }
}

// ------------------------------------------------------------------ context menu

struct MenuRows {
    items: Vec<(String, bool)>,
    keys: Vec<String>,
    selected: Option<i64>,
    swatches: Vec<Option<Color>>,
    danger: Vec<i64>,
    opened: Option<i64>,
    chevrons: Vec<i64>,
    lead_from: i64,
    swatch_pad: f32,
    gap: f32,
}

impl Ui {
    /// the rows of one menu: the row picked, and the row the pointer is on
    fn menu_rows(&mut self, m: &MenuRows) -> (Option<i64>, Option<i64>) {
        menu_rows_body(self, m)
    }

    fn menu_measure(&mut self, lead: f32, keys: &[String], chevrons: &[i64], base: f32, items: &[(String, bool)], swatch_pad: f32, key_size: i64) -> f32 {
        // the 1-point edge, 12 points of padding, then the label, 24 points and the key or chevron
        let mut w = base;
        for (index, (text, _)) in items.iter().enumerate() {
            let extra = match keys.get(index).filter(|k| !k.is_empty()) {
                Some(key) => 24.0 + self.text_width(Some(key_size), key),
                None => if chevrons.contains(&(index as i64)) { 32.5 } else { 0.0 },
            };
            w = w.max(self.text_width(None, text) + 26.0 + lead + swatch_pad + extra);
        }
        w
    }

    /// A floating menu at `at` with `(label, enabled)` rows, as wide as its longest row and at
    /// least `width` wide, capped to the frame. An empty label is a separator line (never picked).
    /// `selected` marks the current choice; `swatches` gives a row a small colour square before its
    /// label. The host keeps it open while this returns `Open`; a row commits on press and release
    /// inside it, and Escape, focus loss, or a press outside return `Dismiss`. Build it before
    /// content it shields; it floats at the root regardless of its parent. `keys` are the shortcuts
    /// in ink-2 at the right; `danger` rows (a delete) read in the error ink. A row named by a
    /// `Submenu` has a chevron and, while the pointer is on it, opens a second menu overlapping the
    /// first by a point, level with the row; the rows of the submenus are numbered after those of
    /// the menu, in the order the submenus are given. With `selected`, the rows from `lead_from`
    /// (default 0) on keep a slot for the accent square before their label. A press outside the
    /// menu in the frame that first builds it dismisses it.
    pub fn context_menu(&mut self, at: (f32, f32), o: &MenuOpts, label: &str, items: &[(String, bool)]) -> MenuResult {
        context_menu_body(self, at, o, label, items)
    }
}

// Method bodies outside `impl` blocks: each module is its own codegen unit, so the big bodies
// compile in parallel with the rest of the crate (rustc places methods with their type).
fn text_area_submit_body(this: &mut Ui, at: (f32, f32), w: f32, h: f32, o: &TextAreaOpts, label: &str, text: &str) -> (String, bool) {
    // a line of code is one and a half times its text, as code editors set it (17 points at
    // 11, 20 at 13): the 24-point row is a control's, twice the text
    let row = this.text_line_height() as f32;
    let body = this.box_(
        label,
        BoxOpts::default().flags(CLICKABLE | FOCUSABLE | BLOCKING | SCROLL | CLIP | KEEP_TAB).at(at).w(Size::Px(w)).h(Size::Px(h)).scroll_step(row),
    );
    // a `select` request takes the keyboard for the area
    if o.select.is_some() {
        this.focus_box(&body);
    }
    this.set_role(&body, Role::TextArea, crate::a11y::state::EDITABLE | crate::a11y::state::MULTILINE);
    this.a11y_value(&body, text, (0, 0));
    let char_w = this.text_width(None, "0");
    // the sheet's code area: a 36-point gutter (wider only past 999 lines), the text at its
    // edge, 6 points of padding above the first row
    let pad = 6.0;
    let cfg = Wrap { wrap: o.wrap, w, char_w };
    let rows = this.area_rows(cfg, text);
    let count = rows.len();
    let language = o.language.clone();
    // a diagnostic's message is a row of its own under the last row of its line
    let note_rows = |rows: &Rows| -> Notes {
        if o.messages.is_empty() {
            return Vec::new();
        }
        let mut found: Notes = Vec::new();
        for (line, span, message) in &o.messages {
            let mut last: Option<usize> = None;
            for (i, (_, _, logical)) in rows.iter().enumerate() {
                if *logical as i64 == line - 1 {
                    last = Some(i);
                }
            }
            if let Some(l) = last {
                found.push((l, *span, message.clone()));
            }
        }
        found.sort_by(|a, b| a.0.cmp(&b.0));
        found
    };
    let logical_count = |rows: &Rows| rows[rows.len() - 1].2 + 1;
    // the content box scrolls; two empty boxes keep the completion popup's and the number
    // scrub's state between frames
    let saved = this.enter(&body);
    let content = this.box_(
        &format!("{}-content", label),
        BoxOpts::default().w(Size::Px(w)).h(Size::Px((count + note_rows(&rows).len()) as f32 * row + pad)),
    );
    let suggest = this.box_fmt(format_args!("{}-suggest", label), BoxOpts::default().w(Size::Px(0.0)).h(Size::Px(0.0)).at((0.0, 0.0)));
    let scrub = this.box_fmt(format_args!("{}-scrub", label), BoxOpts::default().w(Size::Px(0.0)).h(Size::Px(0.0)).at((0.0, 0.0)));
    this.leave(saved);
    let signal = this.signal(&body);
    // the I-beam over text; a number under the pointer asks for the scrub cursor further down,
    // and the last request wins
    if signal.hovered {
        this.request_cursor(Cursor::Text);
    }
    let focused = this.focused(&body);
    let gutter = gutter_of(char_w, logical_count(&rows));
    let Rect { x: bx, y: by, w: bw, h: bh } = this.rect(&body);
    let horizontal0 = this.state(&body, 0) as f32;
    let scroll_pos = this.scroll_position(&body);
    // the byte at a point; `strict` answers only over the row's glyphs
    let point_in = |ui: &mut Ui, strict: bool, text: &str, rows: &Rows, (px, py): (f32, f32)| -> Option<usize> {
        let count = rows.len();
        let notes = note_rows(rows);
        let line = row_at_slot(&notes, count, ((py - by + scroll_pos - pad) / row).floor() as i64);
        let x = px - bx - gutter + horizontal0;
        let line_text = row_text(text, rows, line);
        if strict && (x < 0.0 || x > ui.text_width(None, &line_text) || py < by || py > by + bh) {
            None
        } else {
            Some(start_of(rows, line) + ui.text_caret_at(None, &line_text, 0.0f32.max(x)))
        }
    };
    // a number is dragged by its underline: the lowest 9 points of its row, so a drag that
    // selects text from a number does not rewrite it
    let on_handle = |(_, py): (f32, f32)| {
        let y = py - by + scroll_pos - pad;
        y - row * (y / row).floor() >= row - 9.0
    };
    let mut edit = if focused {
        this.load_text_edit(body.box_key, text)
    } else {
        TextEdit { text: text.to_owned(), caret: text.len(), anchor: text.len() }
    };
    let mut moved = false;
    let mut st = AreaState::default();
    if let Some(f) = &o.on_context {
        if context_clicked(&signal) {
            f(signal.release_point);
        }
    }
    let scrub_state = this.state(&scrub, 0);
    let mut scrubbing: Scrubbing = None;
    // arm_scrub: a press on a number arms a scrub (its state is the start + 1, negated once
    // dragging); three points sideways make it one, three points down a selection as before
    if let Some(l) = &language {
        if signal.pressed && signal.button == Some(MouseButton::LeftButton) {
            let number = if on_handle(signal.press_point) {
                let byte = point_in(this, false, text, &rows, signal.press_point).unwrap();
                (l.number_at)(text, byte as i64)
            } else {
                None
            };
            match number {
                Some((a, b)) => {
                    this.set_state(&scrub, a + 1);
                    this.set_text_state(&scrub, Some(text[clamp_pos(a)..clamp_pos(b)].to_owned()));
                }
                None => this.set_state(&scrub, 0),
            }
        } else if scrub_state != 0 && signal.held {
            let dx = signal.pointer.0 - signal.press_point.0;
            let dy = signal.pointer.1 - signal.press_point.1;
            if scrub_state < 0 || (dx.abs() >= 3.0 && dx.abs() >= dy.abs()) {
                let start = scrub_state.abs() - 1;
                this.set_state(&scrub, -(start + 1));
                scrubbing = Some((start as usize, this.text_state(&scrub).unwrap_or_default(), dx, scrub_state > 0));
            } else if dy.abs() >= 3.0 {
                this.set_state(&scrub, 0);
            }
        } else if scrub_state != 0 && !signal.held {
            this.set_state(&scrub, 0);
            if scrub_state < 0 {
                if let Some(f) = &o.on_scrub {
                    f(ScrubPhase::Done);
                }
            }
        }
    } else if scrub_state != 0 && !signal.held {
        this.set_state(&scrub, 0);
        if scrub_state < 0 {
            if let Some(f) = &o.on_scrub {
                f(ScrubPhase::Done);
            }
        }
    }
    // the completion popup: its state is (token start + 1) * 512 + 256 once an arrow has moved
    // the selection + the selected row; the rows are built before the keys so a click on one
    // is seen this frame
    let open = if focused {
        this.state(&suggest, 0)
    } else {
        this.set_state(&suggest, 0);
        0
    };
    let items: Vec<Completion> = match &language {
        Some(l) if open != 0 => (l.complete)(&edit.text, edit.caret as i64),
        _ => Vec::new(),
    };
    let shown = 8.min(items.len());
    st.selected = ((open & 255) as usize).min(items.len().saturating_sub(1));
    // Enter takes the row only after Up or Down chose it; Tab takes it always
    st.chosen = open & 256 != 0;
    let first = (st.selected + 1).saturating_sub(shown);
    let popup: Option<(UiBox, Vec<UiBox>, f32, f32)> = if shown == 0 {
        None
    } else {
        let doc_empty = items[st.selected].doc.is_empty();
        let mut width: f32 = 220.0;
        for c in &items {
            width = width.max(this.text_width(None, &c.label) + this.text_width(None, &c.detail) + 44.0);
        }
        let (view_w, _) = this.view_size();
        let width = width.min(120.0f32.max(view_w - 16.0));
        let height = (shown as i64 * this.row_height()) as f32 + if doc_empty { 4.0 } else { 22.0 };
        let container = this.overlay_box(BLOCKING, (0.0, 0.0), width, height, &format!("{}-completions", label));
        let saved = this.enter(&container);
        let mut boxes = Vec::new();
        for i in 0..shown {
            boxes.push(this.kit_row(Some(CLICKABLE | BLOCKING | KEEP_FOCUS), None, &format!("row-{}", i)));
        }
        this.leave(saved);
        for (i, r) in boxes.iter().enumerate() {
            if this.signal(r).clicked {
                let c = items[first + i].clone();
                this.accept_completion(&mut edit, &suggest, &mut st, &c);
            }
        }
        Some((container, boxes, width, height))
    };
    if focused {
        let caret0 = edit.caret;
        // a triple click takes the logical line with its break
        let line_of = |e: &TextEdit, at: usize| {
            let s = line_start(&e.text, at);
            let en = line_end(&e.text, at);
            (s, if en < e.text.len() { en + 1 } else { en })
        };
        if signal.pressed && signal.button == Some(MouseButton::LeftButton) {
            this.edit_column = -1.0;
            let shift = this.press_shift(&body);
            let at = point_in(this, false, text, &rows, signal.press_point).unwrap();
            this.press_select(&mut edit, shift, signal.clicks, &line_of, at);
            // a double click on true / false flips it, as dragging flips a number
            if signal.clicks == 2 && language.is_some() {
                let (a, b) = text_selection(&edit);
                let flipped = match &edit.text[a..b] {
                    "true" => Some("false"),
                    "false" => Some("true"),
                    _ => None,
                };
                if let Some(word) = flipped {
                    this.remember(&edit, false);
                    replace_text(&mut edit, word);
                    edit.anchor = a;
                    edit.caret = a + word.len();
                }
            }
        } else if scrubbing.is_none() && scrub_state == 0 && (signal.dragging || (signal.held && signal.pointer != signal.press_point)) {
            let at = point_in(this, false, text, &rows, signal.pointer).unwrap();
            this.drag_select(&mut edit, &line_of, at);
        }
        // the host's replacements (one undo step, last first so the offsets hold), then its selection
        if !o.splice.is_empty() {
            this.remember(&edit, false);
            let mut splice = o.splice.clone();
            splice.sort_by(|a, b| b.0.cmp(&a.0));
            for (a, b, by_) in &splice {
                if 0 <= *a && a <= b && *b as usize <= edit.text.len() {
                    edit.text = format!("{}{}{}", &edit.text[..*a as usize], by_, &edit.text[*b as usize..]);
                }
            }
            st.changed = true;
        }
        if let Some((a, b)) = o.select {
            let n = edit.text.len() as i64;
            edit.anchor = a.max(0).min(n) as usize;
            edit.caret = b.max(0).min(n) as usize;
        }
        let page = 1.max((bh / row) as i64 - 1);
        this.handle_keys(&body, cfg, page, &language, &suggest, open, &items, shown, &mut st, &mut edit);
        // a dragged number: the literal at the press follows the pointer (the caret sits after it)
        this.apply_scrub(&scrubbing, &o.on_scrub, &suggest, &o.on_scrub_edit, &mut edit);
        // typing opens the popup on the token at the caret (the selection starts over); a popup
        // left open follows the caret and closes when the caret leaves its token
        if let Some(l) = &language {
            if st.typed || (open != 0 && !st.closed) {
                let found = (l.complete)(&edit.text, edit.caret as i64);
                match found.first() {
                    Some(c) if st.typed => this.set_state(&suggest, (c.replace.0 + 1) * 512),
                    Some(c) if c.replace.0 == (((open as u64 >> 9) as i64)) - 1 => {
                        this.set_state(&suggest, (((open as u64 >> 9) as i64)) * 512 + if st.chosen { 256 } else { 0 } + st.selected as i64)
                    }
                    _ => this.set_state(&suggest, 0),
                }
            }
        }
        // the language's rewrite (parinfer) after this frame's edits; the anchor maps like the caret
        if let Some(l) = &language {
            if let Some(rewrite) = &l.rewrite {
                if st.changed {
                    let (text2, caret2) = rewrite(&edit.text, edit.caret as i64);
                    let anchor2 = if edit.anchor == edit.caret { caret2 } else { rewrite(&edit.text, edit.anchor as i64).1 };
                    edit.text = text2;
                    edit.caret = caret2 as usize;
                    edit.anchor = anchor2 as usize;
                }
            }
        }
        this.save_text_edit(&edit);
        moved = edit.caret != caret0 || edit.text != text || signal.pressed;
    }
    // scrolling: the wheel, then whatever keeps the caret (or `reveal`) in view
    let final_text = edit.text.clone();
    if focused {
        if let Some(f) = &o.on_caret {
            f(edit.caret as i64);
        }
        if let Some(f) = &o.on_select {
            let (a, b) = text_selection(&edit);
            f((a as i64, b as i64));
        }
    }
    if focused && moved && (signal.pressed || signal.dragging || !this.key_events(&body).is_empty()) {
        if let Some(f) = &o.on_caret_move {
            f(edit.caret as i64);
        }
    }
    if let Some(f) = &o.on_click {
        if signal.clicked && signal.button == Some(MouseButton::LeftButton) && scrub_state >= 0 {
            if let Some(byte) = point_in(this, true, text, &rows, signal.release_point) {
                let c = this.command_modifiers(&this.press_keys(&body));
                f(byte as i64, c);
            }
        }
    }
    // a payload in flight over the area: the byte under the pointer, hovering or released
    if let Some(f) = &o.on_drop {
        if let Some(drop) = this.drop_target(&body) {
            if let Some(byte) = point_in(this, false, text, &rows, signal.pointer) {
                f(byte as i64, drop);
            }
        }
    }
    let rows = if final_text == text { rows } else { this.area_rows(cfg, &final_text) };
    let count = rows.len();
    let notes = note_rows(&rows);
    // `reveal` scrolls once per (offset, length, nonce): the content box remembers it, and
    // forgets it while there is nothing to reveal
    let revealed = match o.reveal {
        Some(index)
            if bw > 0.0
                && this.state(&content, 0)
                    != index.wrapping_mul(1_000_003) + final_text.len() as i64 + 1 + o.reveal_nonce.wrapping_mul(1_000_000_007) =>
        {
            this.set_state(&content, index.wrapping_mul(1_000_003) + final_text.len() as i64 + 1 + o.reveal_nonce.wrapping_mul(1_000_000_007));
            Some(index)
        }
        None => {
            this.set_state(&content, 0);
            None
        }
        _ => None,
    };
    let target = if moved { Some(edit.caret) } else { revealed.map(|index| final_text.len().min(clamp_pos(index))) };
    let longest = rows.iter().map(|(s, e, _)| e - s).max().unwrap_or(0);
    let visible = 1.0f32.max(bw - gutter - 16.0);
    let max_x = if o.wrap { 0.0 } else { 0.0f32.max(longest as f32 * char_w - visible) };
    let mut horizontal = 0.0f32.max(max_x.min(horizontal0 + signal.scroll.0 * row));
    let mut vertical = 0.0f32.max((0.0f32.max((count + notes.len()) as f32 * row + pad - bh)).min(this.scroll_offset(&body)));
    if let Some(index) = target {
        let line = row_at(&rows, index);
        let top = pad + slot_of(&notes, line) as f32 * row;
        if top < vertical {
            vertical = top;
        } else if top + row > vertical + bh {
            vertical = top + row - bh;
        }
        let (ls, le) = (start_of(&rows, line), stop_of(&rows, line));
        let x = this.caret_x(None, &final_text[ls..le], index.min(le) - ls);
        if x < horizontal {
            horizontal = x;
        } else if x - horizontal > visible {
            horizontal = x - visible;
        }
    }
    this.set_state(&body, horizontal as i64);
    if vertical != this.scroll_offset(&body) {
        this.set_scroll_offset(&body, vertical);
    }
    // painted where the shared elastic scroll puts the content, overshoot included
    let offset = this.scroll_position(&body);
    let theme = this.tokens();
    let area_look = this.theme().text_area.idle;
    let composition = this.composition().to_owned();
    // the language's colours and the bracket pair at the caret, once per frame
    let colors: Rc<Vec<(i64, i64, Color)>> = match &language {
        Some(l) => this.text_colors(l, &final_text),
        None => Rc::new(Vec::new()),
    };
    let matched: Option<(usize, usize)> = match &language {
        Some(l) if focused => {
            let c = edit.caret as i64;
            (l.brackets)(&final_text).into_iter().find(|(o, k)| *o == c - 1 || *k == c - 1 || *o == c || *k == c).map(|(a, b)| (clamp_pos(a), clamp_pos(b)))
        }
        _ => None,
    };
    // the pointer over a number invites a drag; over any other token it describes it after a rest
    let under_pointer = match &language {
        Some(_) if signal.hovered && this.active().is_none() => point_in(this, true, &final_text, &rows, signal.pointer),
        _ => None,
    };
    let hovered_number: Option<(usize, usize)> = match (&language, under_pointer, &scrubbing) {
        (_, _, Some((start, _, _, _))) => {
            let mut stop = *start;
            let b = final_text.as_bytes();
            while stop < b.len() && numeric_character(b[stop] as char) {
                stop += 1;
            }
            Some((*start, stop))
        }
        (Some(l), Some(byte), None) if on_handle(signal.pointer) => (l.number_at)(&final_text, byte as i64).map(|(a, b)| (clamp_pos(a), clamp_pos(b))),
        _ => None,
    };
    if hovered_number.is_some() {
        this.request_cursor(Cursor::HorizontalResize);
    }
    if let (Some(l), Some(byte)) = (&language, under_pointer) {
        if hovered_number.is_none() && open == 0 {
            if let Some((start, _, doc)) = (l.describe)(&final_text, byte as i64) {
                if !doc.is_empty() {
                    this.tooltip(&format!("{}#{}", label, start), &doc);
                }
            }
        }
    }
    // the completion popup sits under the caret's row (above it near the bottom of the view)
    let text_x0 = bx + gutter - horizontal;
    if let Some((container, boxes, width, height)) = &popup {
        let caret_line = row_at(&rows, edit.caret);
        let index = clamp_pos(items[st.selected].replace.0);
        let line = row_at(&rows, index);
        let (ls, le) = (start_of(&rows, line), stop_of(&rows, line));
        let anchor = text_x0 + this.caret_x(None, &final_text[ls..le], index.min(le) - ls);
        let (view_w, view_h) = this.view_size();
        let line_top = by - offset + pad + slot_of(&notes, caret_line) as f32 * row;
        let placement = Placement { side: Side::Below, align: Align::Start, gap: 0.0, margin: 8.0, flip: true };
        let r = place_anchored(Rect::new(anchor - 8.0, line_top, 0.0, row), (*width, *height), placement, Rect::new(0.0, 0.0, view_w, view_h));
        this.set_at(container, (r.x, r.y));
        let selected = st.selected;
        let t = this.theme();
        let (sheet, list, hint) = (t.popup.idle, t.list, t.hint.idle);
        this.set_look(container, sheet, State::default());
        this.set_role(container, Role::List, 0);
        let doc = &items[selected].doc;
        if !doc.is_empty() {
            let saved = this.enter(container);
            let size = this.font_size() as f32;
            let n = this.node_keyed(int_key(container.box_key, -1), Node { w: Size::Px((width - 8.0).max(1.0)), h: Size::Px(size), at: Some((8.0, height - 18.0)).into(), look: hint, ..Node::DEFAULT });
            this.text(&n, &display(doc));
            this.leave(saved);
        }
        for (i, r) in boxes.iter().enumerate() {
            let c = &items[first + i];
            let state = State { hovered: this.signal(r).hovered, checked: first + i == selected, ..State::default() };
            this.style(r, &list, state);
            this.set_role(r, Role::ListItem, if first + i == selected { crate::a11y::state::SELECTED } else { 0 });
            this.node_mut(r).inset = Edges { l: this.space(2), ..Edges::default() };
            this.text(r, &c.label);
            let dw = this.text_width(None, &c.detail) as i64 as f32;
            let rh = this.row_height() as f32;
            this.end_row(r, this.space(2));
            let saved = this.enter(r);
            this.flow_part(int_key(r.box_key, 0), dw, rh, Edges::ZERO, hint, &c.detail);
            this.leave(saved);
        }
    }
    // the text area's painting: gutter, selection, runs, diagnostics, caret
    let painted = AreaPaint {
        colors,
        composition,
        messages: o.messages.clone(),
        chips: o.chips.clone(),
        hovered_number,
        scrubbing: scrubbing.is_some(),
        matched,
        spans: o.spans.clone(),
        errors: o.errors.clone(),
        notes,
        char_w,
        final_text,
        rows,
        count,
        focused,
        edit: edit.clone(),
        row,
        pad,
        offset,
        horizontal,
        gutter,
        theme,
        look: area_look,
    };
    this.draw(&body, move |paint, rect| painted.paint(paint, rect));
    (edit.text, st.submitted)
}

fn handle_keys_body(this: &mut Ui, body: &UiBox, cfg: Wrap, page: i64, language: &Option<Language>, suggest: &UiBox, open: i64, items: &[Completion], shown: usize, st: &mut AreaState, edit: &mut TextEdit) {
    for (event, modifiers) in this.key_events(body) {
        let command = this.command_modifiers(&modifiers);
        let shift = modifiers.contains(&Key::Shift);
        let rows = this.area_rows(cfg, &edit.text);
        let count = rows.len();
        let line = row_at(&rows, edit.caret);
        // Up and Down keep the column they started from across shorter rows
        match &event {
            Event::KeyPressed(Key::ArrowUp) | Event::KeyPressed(Key::ArrowDown) if !command => {}
            Event::KeyPressed(Key::PageUp | Key::PageDown | Key::Shift | Key::Ctrl | Key::Alt | Key::Meta) => {}
            _ => this.edit_column = -1.0,
        }
        let listing = shown > 0 && !st.closed;
        match &event {
            Event::KeyPressed(Key::Enter) if command => st.submitted = true,
            // the completion popup takes Up, Down, Tab, Enter and Escape while it is open
            Event::KeyPressed(key @ (Key::ArrowUp | Key::ArrowDown)) if listing => {
                let n = items.len();
                st.chosen = true;
                st.selected = (st.selected + if *key == Key::ArrowUp { n - 1 } else { 1 }) % n;
            }
            Event::KeyPressed(Key::Tab) if listing && !command => {
                let c = items[st.selected].clone();
                this.accept_completion(edit, suggest, st, &c);
            }
            Event::KeyPressed(Key::Enter) if listing && st.chosen => {
                let c = items[st.selected].clone();
                this.accept_completion(edit, suggest, st, &c);
            }
            Event::KeyPressed(Key::Escape) if open != 0 && !st.closed => {
                this.set_state(suggest, 0);
                st.closed = true;
            }
            Event::KeyPressed(Key::Escape) => this.unfocus(),
            Event::KeyPressed(Key::Enter) => {
                let indent = match language {
                    Some(l) => (l.indent)(&edit.text, text_selection(edit).0 as i64),
                    None => String::new(),
                };
                this.change(edit, st);
                replace_text(edit, &format!("\n{}", indent));
                if open != 0 {
                    this.set_state(suggest, 0);
                    st.closed = true;
                }
            }
            // brackets come in pairs: an opener wraps the selection or inserts both, a closer
            // typed before itself steps over it, Backspace between an empty pair takes both
            Event::TextInput(s) if language.is_some() && s.len() == 1 && !command => {
                let language_ref = language.as_ref().unwrap();
                let c = s.chars().next().unwrap();
                let pairs = &language_ref.pairs;
                let (start, stop) = text_selection(edit);
                let next = if edit.caret < edit.text.len() { Some(edit.text.as_bytes()[edit.caret] as char) } else { None };
                // a bracket is a bracket only in code: in a string only a quote steps over its
                // closing quote, and a comment takes every character as typed; in code a quote
                // before a quote is the next string's opening one
                let context = (language_ref.context_at)(&edit.text, edit.caret as i64);
                let quote = pairs.contains(&(c, c));
                let step_over = next == Some(c)
                    && match context {
                        CodeContext::Code => pairs.iter().any(|(_, close)| *close == c) && !quote,
                        CodeContext::String => quote,
                        CodeContext::Comment => false,
                    };
                let close = pairs.iter().find(|(o, _)| *o == c).map(|(_, k)| *k);
                match (context, close) {
                    (CodeContext::Code, Some(close)) if start != stop => {
                        this.change(edit, st);
                        let inner = edit.text[start..stop].to_owned();
                        replace_text(edit, &format!("{}{}{}", c, inner, close));
                        edit.anchor = start + 1;
                        edit.caret = stop + 1;
                    }
                    _ if step_over => {
                        edit.caret += 1;
                        edit.anchor = edit.caret;
                    }
                    (CodeContext::Code, Some(close)) => {
                        this.change(edit, st);
                        replace_text(edit, &format!("{}{}", c, close));
                        edit.caret -= 1;
                        edit.anchor = edit.caret;
                    }
                    _ => {
                        let changed = this.edit_text_event(true, edit, &|_| true, &modifiers, &event);
                        st.changed = changed || st.changed;
                    }
                }
                st.typed = true;
            }
            Event::KeyPressed(Key::Backspace)
                if language.is_some()
                    && !command
                    && edit.caret == edit.anchor
                    && edit.caret > 0
                    && edit.caret < edit.text.len()
                    && language
                        .as_ref()
                        .unwrap()
                        .pairs
                        .contains(&(edit.text.as_bytes()[edit.caret - 1] as char, edit.text.as_bytes()[edit.caret] as char)) =>
            {
                this.change(edit, st);
                edit.anchor = edit.caret - 1;
                edit.caret += 1;
                replace_text(edit, "");
                st.typed = true;
            }
            // Command-X / C with no selection take the caret's whole logical line, newline included
            Event::KeyPressed(Key::KeyChar(key @ ('x' | 'X' | 'c' | 'C'))) if command && edit.caret == edit.anchor => {
                let s = line_start(&edit.text, edit.caret);
                let e = edit.text.len().min(line_end(&edit.text, edit.caret) + 1);
                this.copy(&edit.text[s..e]);
                if *key == 'x' || *key == 'X' {
                    this.change(edit, st);
                    edit.anchor = s;
                    edit.caret = e;
                    replace_text(edit, "");
                    st.typed = true;
                }
            }
            Event::KeyPressed(Key::Tab) if !command && edit.caret != edit.anchor => {
                // Tab and Shift-Tab with a selection indent and dedent every line it touches
                let (a, b) = text_selection(edit);
                let (first, stop) = line_block(&edit.text, a, b);
                let block = edit.text[first..stop].to_owned();
                let shifted = block
                    .split('\n')
                    .map(|l| {
                        if !shift {
                            if l.is_empty() { l.to_owned() } else { format!("  {}", l) }
                        } else if let Some(rest) = l.strip_prefix("  ") {
                            rest.to_owned()
                        } else if let Some(rest) = l.strip_prefix(' ') {
                            rest.to_owned()
                        } else {
                            l.to_owned()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if shifted != block {
                    this.change(edit, st);
                    replace_block(edit, first, stop, &shifted);
                    keep_selection(edit, first, first + shifted.len());
                }
            }
            Event::KeyPressed(Key::Tab) if !command => {
                if !shift {
                    this.change(edit, st);
                    replace_text(edit, "  ");
                } else {
                    // Shift-Tab: up to two spaces leave the start of the line
                    let ls = line_start(&edit.text, edit.caret);
                    let b = edit.text.as_bytes();
                    let spaces = if ls < b.len() && b[ls] == b' ' {
                        if ls + 1 < b.len() && b[ls + 1] == b' ' { 2 } else { 1 }
                    } else {
                        0
                    };
                    if spaces > 0 {
                        this.change(edit, st);
                        edit.text = format!("{}{}", &edit.text[..ls], &edit.text[ls + spaces..]);
                        edit.caret = ls.max(edit.caret.saturating_sub(spaces));
                        edit.anchor = edit.caret;
                    }
                }
            }
            // line and form edits: Command-/ toggles comments, Option-Up and Down move the
            // lines, with Shift duplicate them, Command-Option-Up selects the enclosing form,
            // Command-Shift-\ jumps to the matching bracket
            Event::KeyPressed(Key::KeyChar('/')) if command && language.is_some() => {
                let (a, b) = text_selection(edit);
                let (first, stop) = line_block(&edit.text, a, b);
                let block = edit.text[first..stop].to_owned();
                let toggled = toggle_comment(&language.as_ref().unwrap().comment, &block);
                if toggled != block {
                    this.change(edit, st);
                    replace_block(edit, first, stop, &toggled);
                    let last = first + toggled.len();
                    if a == b {
                        edit.caret = first.max((a + last).saturating_sub(stop));
                        edit.anchor = edit.caret;
                    } else {
                        keep_selection(edit, first, last);
                    }
                }
            }
            Event::KeyPressed(key @ (Key::ArrowUp | Key::ArrowDown)) if modifiers.contains(&Key::Alt) && !command => {
                let (a, b) = text_selection(edit);
                let (first, stop) = line_block(&edit.text, a, b);
                let n = edit.text.len();
                let block = edit.text[first..stop].to_owned();
                let up = *key == Key::ArrowUp;
                let shift_by = |edit: &mut TextEdit, d: i64| {
                    edit.anchor = (edit.anchor as i64 + d) as usize;
                    edit.caret = (edit.caret as i64 + d) as usize;
                };
                if shift {
                    // a copy below keeps the selection on the copy, a copy above on the original
                    this.change(edit, st);
                    replace_block(edit, stop, stop, &format!("\n{}", block));
                    if !up {
                        shift_by(edit, block.len() as i64 + 1);
                    }
                } else if up && first > 0 {
                    let before = line_start(&edit.text, first - 1);
                    this.change(edit, st);
                    let above = edit.text[before..first - 1].to_owned();
                    replace_block(edit, before, stop, &format!("{}\n{}", block, above));
                    shift_by(edit, before as i64 - first as i64);
                } else if !up && stop < n {
                    let after = line_end(&edit.text, stop + 1);
                    this.change(edit, st);
                    let below = edit.text[stop + 1..after].to_owned();
                    replace_block(edit, first, after, &format!("{}\n{}", below, block));
                    shift_by(edit, after as i64 - stop as i64);
                }
            }
            Event::KeyPressed(Key::ArrowUp) if command && modifiers.contains(&Key::Alt) => {
                let (a, b) = text_selection(edit);
                if let Some(l) = language {
                    let pairs: Vec<(usize, usize)> = (l.brackets)(&edit.text).iter().map(|(o, c)| (clamp_pos(*o), clamp_pos(*c))).collect();
                    if let Some((o, c)) = enclosing_pair(&pairs, a, b) {
                        edit.anchor = o;
                        edit.caret = c;
                    }
                }
            }
            Event::KeyPressed(Key::KeyChar('\\')) if command && shift && language.is_some() => {
                let l = language.as_ref().unwrap();
                let pairs: Vec<(usize, usize)> = (l.brackets)(&edit.text).iter().map(|(o, c)| (clamp_pos(*o), clamp_pos(*c))).collect();
                if let Some(at) = bracket_jump(&pairs, edit.caret) {
                    edit.caret = at;
                    edit.anchor = at;
                }
            }
            Event::KeyPressed(Key::ArrowUp) if command => move_caret(edit, 0, shift),
            Event::KeyPressed(Key::ArrowDown) if command => move_caret(edit, edit.text.len(), shift),
            Event::KeyPressed(Key::ArrowUp) => this.vertical(edit, &rows, count, line, -1, shift),
            Event::KeyPressed(Key::ArrowDown) => this.vertical(edit, &rows, count, line, 1, shift),
            Event::KeyPressed(Key::PageUp) => this.vertical(edit, &rows, count, line, -page, shift),
            Event::KeyPressed(Key::PageDown) => this.vertical(edit, &rows, count, line, page, shift),
            Event::KeyPressed(Key::Home | Key::ArrowLeft)
                if this.line_modifiers(&modifiers) || event == Event::KeyPressed(Key::Home) =>
            {
                move_caret(edit, start_of(&rows, line), shift)
            }
            Event::KeyPressed(Key::End | Key::ArrowRight)
                if this.line_modifiers(&modifiers) || event == Event::KeyPressed(Key::End) =>
            {
                move_caret(edit, stop_of(&rows, line), shift)
            }
            event => {
                if this.edit_text_event(true, edit, &|_| true, &modifiers, event) {
                    st.changed = true;
                    if matches!(event, Event::TextInput(_) | Event::KeyPressed(Key::Backspace | Key::Delete)) {
                        st.typed = true;
                    }
                }
            }
        }
    }
}

fn picker_body(this: &mut Ui, o: &PickerOpts, label: &str, query: &str, fill: &mut dyn FnMut(&str, &mut Vec<(String, String)>)) -> (String, Pick) {
    let off = |i: i64| o.off.as_ref().map(|f| f(i)).unwrap_or(false);
    let mut rows = core::mem::take(&mut this.picker_rows);
    fill(query, &mut rows);
    // the search row: a 20-point field in 4 points of padding, a [/] before the query
    // a 20-point field in 4 points of margin
    let search = this.box_(label, BoxOpts::default().flags(CLICKABLE | FOCUSABLE | BLOCKING | FOCUS_MARK).w(Size::Grow).h(Size::Px(this.control_h())));
    this.node_mut(&search).margin = Edges::all(4.0);
    if this.focus_key() == 0 || this.slot_rw(search.box_slot) == 0.0 {
        this.focus_box(&search);
    }
    let list = this.box_keyed(int_key(search.box_key, 0), BoxOpts::default().w(Size::Grow).h(Size::Fit).axis(Axis::Column));
    let search_signal = this.signal(&search);
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
    let placeholder = display(label);
    let font_size = this.font_size();
    let (slash, at_rest) = (o.slash, o.at_rest);
    let t = this.theme();
    // a field that has not been typed in yet can sit at rest: the hairline and no caret
    let resting = at_rest && edit.text.is_empty();
    let state = State { focused: !resting, ..State::default() };
    let field = *t.field.pick(state);
    this.set_role(&search, Role::TextField, crate::a11y::state::EDITABLE);
    this.a11y_name(&search, label);
    this.a11y_value(&search, &edit.text, (edit.text[..edit.anchor.min(edit.text.len())].chars().count() as u32, edit.text[..edit.caret.min(edit.text.len())].chars().count() as u32));
    this.set_role(&list, Role::List, 0);
    let config = PickerFieldConfig { state, empty: edit.text.is_empty(), slash, prefix: prefix as f32, inset, search };
    in_body!(this, search.box_key, search, picker_field, default_picker_field(&config, &placeholder));
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
    let row_height = this.row_height();
    let body = body_of!(this, picker);
    for visible in 0..length {
        let index = start + visible;
        // a row that cannot be picked leaves the keyboard with the search field
        let row = this.box_keyed(
            int_key(list.box_key, visible),
            BoxOpts::default()
                .flags(CLICKABLE | FOCUSABLE | BLOCKING | if off(index) { KEEP_FOCUS } else { 0 })
                .w(Size::Grow)
                .h(Size::Px(row_height as f32)),
        );
        let row_signal = this.signal(&row);
        if result == Pick::None && row_signal.clicked && !off(index) {
            result = Pick::Pick(index);
        }
        let current = index == cursor;
        let doomed = index == armed;
        let unavailable = off(index);
        let (text, detail) = &rows[index as usize];
        let swatch = o.mark.as_ref().and_then(|m| m(index));
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
    let (font_size, row_height) = (this.font_size(), this.row_height());
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
    let key_size = key_look.font.points(font_size);
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

fn value_field_body(this: &mut Ui, at: (f32, f32), w: f32, h: f32, o: &ValueFieldOpts, valid: &dyn Fn(&str) -> bool, label: &str, value: &str) -> (String, bool) {
    let slider = o.slide.is_some() || o.scrub.is_some();
    let drag = if slider && o.kept_drag { DRAG_FREE } else { NO_FLAGS };
    let bx = this.box_(
        label,
        BoxOpts::default().flags(CLICKABLE | TAB_STOP | BLOCKING | CLIP | drag).at(at).w(Size::Px(w)).h(Size::Px(h)),
    );
    let signal = this.signal(&bx);
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
    // (an idle field allocates nothing but the value it returns)
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
        this.set_flags_of_box(&bx, CLICKABLE | FOCUSABLE | BLOCKING | CLIP);
    }
    if signal.pressed {
        this.scrub_origin = Some((bx.box_key, value.to_owned()));
    }
    let press_keys = this.press_keys(&bx);
    let typed_is_some = typed.is_some();
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
        let (size, fs) = (o.size, this.font_size());
        let ty = 0.max((h as i64 - size.unwrap_or(fs)) / 2);
        let (selection, caret) = (field.selection, field.caret);
        this.canvas(&bx, move |paint, rect| {
            paint_text_edit(paint, size, false, 2.0, Some((selection, caret)), ints(rect), rect.y as i64 + ty, &composition, &edit);
        });
    }
    (value.into_owned(), editing)
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
    let fs = this.font_size();
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
            let part = |ui: &mut Ui, n: i64, right: bool, text: &str, color: Color| -> f32 {
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
    let font_size = this.font_size();
    // a window's head (windows.html): the 20-point title and one detail line, 8 above, no
    // chips, buttons or hairline
    let display_size = font_size * (if win { theme::TITLE_SIZE } else { theme::DISPLAY_SIZE }) / theme::FONT_SIZE;
    let small = 8.max(font_size - 2);
    let line = this.row_height() as f32;
    let title_h = if win { this.row_height() as f32 } else { (display_size + 8) as f32 };
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
    let header = this.box_(key, BoxOpts::default().flags(CLIP).w(Size::Grow).h(Size::Px(height)));
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
        let n = this.node_keyed(k(3), Node { flags: FIT_TEXT, h: Size::Px(display_size as f32), at: Some((0.0, y_title + dy)).into(), look, inset: Edges { l: text_x, r: text_x, ..Edges::default() }, ..Node::DEFAULT });
        this.text(&n, title);
    }
    let ty = text_top_for(font_size, None, 0.0, line);
    let n = this.node_keyed(k(4), Node { flags: FIT_TEXT, h: Size::Px(font_size as f32), at: Some((0.0, y_detail + ty)).into(), look: hint, inset: Edges { l: text_x, r: text_x, ..Edges::default() }, ..Node::DEFAULT });
    this.text(&n, detail);
    this.leave(saved);
    Head { renamed: title_shown, chosen: pressed, reset_pressed }
}

fn numeric_editor_body(this: &mut Ui, p: &NumParams, accept: &dyn Fn(&str) -> bool, row: &UiBox, signal: &Signal, keys: &[(Event, Vec<Key>)], current: &dyn Fn() -> String, parse: &dyn Fn(&str) -> Option<String>) -> (Option<String>, bool) {
    let bounds = ints(this.rect(row));
    let control = p.control.unwrap_or_else(|| value_control_at(bounds, this.space(2) as i64));
    let inside_control = |point: (f32, f32)| contains(floats(control), point);
    let editing = this.text_state(row);
    let state = this.state(row, 1);
    match editing {
        Some(text) if !this.focused(row) || (signal.pressed && !inside_control(signal.press_point)) => {
            // Focus moved away: commit a valid value, drop an invalid one.
            this.set_text_state(row, None);
            this.set_state(row, 1);
            if this.focused(row) {
                this.unfocus();
            }
            (parse(&text), false)
        }
        Some(text) => {
            let mut edit = this.load_text_edit(row.box_key, &text);
            let mut committed: Option<String> = None;
            let mut state = state;
            let mut cancelled = false;
            let (cx, _, cw, _) = control;
            let left = if p.align_right {
                ((cx + 2) as f32).max((cx + cw - 2) as f32 - this.text_width(p.size, &edit.text))
            } else {
                (cx + 2) as f32
            };
            let shift = this.press_shift(row);
            this.point_text_caret(p.size, &mut edit, signal, shift, left, (cx + cw - 2) as f32);
            for (event, modifiers) in keys {
                match event {
                    Event::KeyPressed(Key::Enter) if !this.command_modifiers(modifiers) => match parse(&edit.text) {
                        Some(value) => committed = Some(value),
                        None => state = 0,
                    },
                    Event::KeyPressed(Key::Escape) => cancelled = true,
                    event => {
                        if this.edit_text_event(false, &mut edit, accept, modifiers, event) {
                            state = if parse(&edit.text).is_some() { 1 } else { 0 };
                        }
                    }
                }
            }
            if cancelled {
                this.set_text_state(row, None);
                this.set_state(row, 1);
                this.unfocus();
                (None, false)
            } else if let Some(value) = committed {
                this.set_text_state(row, None);
                this.set_state(row, 1);
                this.unfocus();
                (Some(value), false)
            } else {
                this.save_text_edit(&edit);
                this.set_text_state(row, Some(edit.text.clone()));
                this.set_state(row, if state & 1 != 0 { 1 } else { 0 });
                (None, true)
            }
        }
        None => {
            let click = p.click_to_edit
                && signal.clicked
                && (signal.release_point.0 - signal.press_point.0).abs() < 4.0
                && (signal.release_point.1 - signal.press_point.1).abs() < 4.0;
            let platform = this.platform();
            let enter = move |(event, modifiers): &(Event, Vec<Key>)| {
                matches!(event, Event::KeyPressed(Key::Enter)) && !input::command_held(platform, modifiers)
            };
            let enter_pressed = keys.iter().any(|k| enter(k));
            if p.edit_request
                || (p.alt_to_edit && signal.clicked && this.press_keys(row).contains(&Key::Alt))
                || click
                || (signal.double_clicked && !inside_control(signal.press_point))
                || (this.focused(row) && enter_pressed)
            {
                let text = current();
                this.set_text_state(row, Some(text.clone()));
                this.set_state(row, 1);
                this.focus_box(row);
                this.reset_edit(row.box_key, &text, 0);
                let keys: Vec<(Event, Vec<Key>)> = if signal.double_clicked || signal.clicked {
                    keys.to_vec()
                } else {
                    // after_enter: what follows the first Enter
                    match keys.iter().position(|k| enter(k)) {
                        Some(at) => keys[at + 1..].to_vec(),
                        None => Vec::new(),
                    }
                };
                let mut again = signal.clone();
                again.keys = keys.iter().map(|(e, _)| e.clone()).collect();
                again.pressed = false;
                again.clicked = false;
                again.double_clicked = false;
                let np = NumParams { size: p.size, control: Some(control), click_to_edit: p.click_to_edit, edit_request: false, alt_to_edit: false, align_right: p.align_right };
                this.numeric_editor(&np, accept, row, &again, &keys, current, parse)
            } else {
                (None, false)
            }
        }
    }
}

fn menu_rows_body(this: &mut Ui, m: &MenuRows) -> (Option<i64>, Option<i64>) {
    let mut hovered = None;
    let mut picked = None;
    let body = body_of!(this, context_menu);
    let lead_of = |index: i64| if m.selected.is_none() || index < m.lead_from { 0 } else { 14 };
    for (index, (text, enabled)) in m.items.iter().enumerate() {
        let index = index as i64;
        let t = this.theme();
        if text.is_empty() {
            let look = t.separator.idle;
            // a hairline 4 points down in a `gap`-point space
            let line = this.box_fmt(format_args!("separator-{}", index), BoxOpts::default().flags(BLOCKING).w(Size::Grow).h(Size::Px(1.0)));
            this.set_look(&line, look, State::default());
            this.set_role(&line, Role::Separator, 0);
            this.node_mut(&line).margin = Edges { t: 4.0, b: m.gap - 5.0, ..Edges::ZERO };
        } else {
            let enabled = *enabled;
            let danger = m.danger.contains(&index);
            let row = this.kit_row(Some(if enabled { CLICKABLE | FOCUSABLE | BLOCKING | TAB_ONLY } else { BLOCKING }), None, text);
            let signal = this.signal(&row);
            if enabled && signal.hovered {
                hovered = Some(index);
            }
            let chevron = m.chevrons.contains(&index);
            let state = State { hovered: signal.hovered || m.opened == Some(index), disabled: !enabled, ..State::default() };
            this.set_role(&row, Role::MenuItem, if m.selected == Some(index) { crate::a11y::state::CHECKED } else { 0 } | if chevron { crate::a11y::state::EXPANDABLE } else { 0 });
            this.a11y_name(&row, shown(text));
            let config = MenuRowConfig { state, index, row, selected: m.selected == Some(index), chevron, danger, lead: lead_of(index) as f32, swatch_pad: m.swatch_pad, swatch: m.swatches.get(index as usize).copied().flatten() };
            let key = m.keys.get(index as usize).map(String::as_str).filter(|k| !k.is_empty());
            in_item!(this, body, row.box_key, row, default_menu_row(&config, text, key));
            if enabled && signal.clicked && !chevron && picked.is_none() {
                picked = Some(index);
            }
        }
    }
    (picked, hovered)
}

/// The built-in context menu row: the menu look (the error one for `danger`) for its state, the label `lead` and `swatch_pad` in from the
/// start, then the choice's mark, the swatch, the submenu chevron and the shortcut.
pub(crate) fn default_menu_row(this: &mut Ui, c: &MenuRowConfig, text: &str, key: Option<&str>) {
    let t = this.theme();
    let look = *(if c.danger { t.menu_danger } else { t.menu }).pick(c.state);
    let (mark, swatch_look, chevron_look, key_look) = (t.mark.idle, t.swatch.idle, t.hint.idle, t.menu_key.idle);
    let (font_size, rh) = (this.font_size(), this.row_height() as f32);
    this.set_look(&c.row, look, c.state);
    this.node_mut(&c.row).inset = Edges { l: this.space(2) + c.lead + c.swatch_pad, ..Edges::default() };
    this.text(&c.row, shown(text));
    let k = |n: i64| int_key(c.row.box_key, n);
    this.end_row(&c.row, 0.0);
    // the current choice: a 6-point accent square before the label
    if c.selected {
        this.placed(k(0), (this.space(2), rh / 2.0 - 3.0), Size::Px(6.0), 6.0, mark, Edges::default(), "");
    }
    if let Some(color) = c.swatch {
        this.placed(k(1), (this.space(2) + c.lead, rh / 2.0 - 4.0), Size::Px(8.0), 8.0, Look { fill: color, ..swatch_look }, Edges::default(), "");
    }
    if c.chevron {
        let n = this.flow_part(k(2), 6.0, rh, Edges { r: 15.25 - 3.0, ..Edges::ZERO }, chevron_look, "");
        this.node_mut(&n).mark = Mark::Chevron(Dir::Right);
    }
    if let Some(key) = key {
        let kw = this.text_width(Some(key_look.font.points(font_size)), key);
        this.flow_part(k(3), kw, rh, Edges { r: this.space(2), ..Edges::ZERO }, key_look, key);
    }
}

fn text_field_body(this: &mut Ui, text: &str, value: &str) -> Option<String> {
    // the control, its hit shape and its text sit `space(2)` in from the row's ends at any density (until the body's own editor part has
    // been laid out)
    let side = this.space(2) as i64;
    let row = this.kit_row(Some(CLICKABLE | FOCUSABLE | BLOCKING | FOCUS_MARK), None, text);
    this.hit_part(&row, Part::EDITOR);
    let signal = this.signal(&row);
    let focused = this.focused(&row);
    let control_at = |ui: &Ui| ui.part_rect(row.box_key, Part::EDITOR).map_or_else(|| value_control_at(ints(ui.rect(&row)), side), |r| ints(r));
    // an unfocused field paints `value` as it is (kept in its slot); only editing copies it
    let edit = if focused {
        let buffer = std::mem::take(&mut this.edit_work);
        let mut edit = this.load_text_edit_into(row.box_key, value, buffer);
        let (cx, _, cw, _) = control_at(this);
        let shift = this.press_shift(&row);
        this.point_text_caret(None, &mut edit, &signal, shift, (cx + 2) as f32, (cx + cw - 2) as f32);
        for (event, modifiers) in this.key_events(&row) {
            this.edit_text_event(false, &mut edit, &|_| true, &modifiers, &event);
        }
        this.save_text_edit(&edit);
        Some(edit)
    } else {
        None
    };
    // a screen reader's SetValue replaces the value (one change, one undo step for the host)
    let changed = match signal.a11y_value {
        Some(v) => Some(cfmt::g(v, 6)),
        None => edit.as_ref().filter(|e| e.text != value).map(|e| e.text.clone()),
    };
    {
        let shown_value = changed.as_deref().unwrap_or(value);
        let chars = |at: usize| shown_value[..at.min(shown_value.len())].chars().count() as u32;
        let selection = edit.as_ref().map_or((0, 0), |e| (chars(e.anchor), chars(e.caret)));
        this.set_role(&row, Role::TextField, crate::a11y::state::EDITABLE);
        this.a11y_name(&row, text);
        this.a11y_value(&row, shown_value, selection);
    }
    let hovered = signal.hovered;
    // the I-beam over the value, as in any text field
    {
        let (px, py) = this.pointer();
        let (cx, cy, cw, ch) = control_at(this);
        if hovered && px >= cx as f32 && px < (cx + cw) as f32 && py >= cy as f32 && py < (cy + ch) as f32 {
            this.request_cursor(Cursor::Text);
        }
    }
    let config = TextFieldConfig { state: State { hovered, focused, ..State::default() }, row };
    in_body!(this, row.box_key, row, text_field, default_text_field(&config, text, value));
    if let Some(edit) = edit {
        let control = this.part_built(row.box_key, Part::EDITOR).unwrap_or(row);
        let field = *this.theme().field.pick(State { focused, ..State::default() });
        let font_size = this.font_size();
        let marks = Some((field.selection, field.caret));
        let (caret, anchor) = (edit.caret, edit.anchor);
        this.canvas_edit(&control, &edit.text, move |paint, rect, text, composition| {
            let (x, y, w, h) = ints(rect);
            paint_text_edit_ref(paint,None,false,2.0,marks,(x,y,w,h),label_y(font_size,y-2,h+4),composition,&EditRef { text,caret,anchor })
        });
        this.edit_work = edit.text;
    }
    changed
}

/// The built-in text field: the row in the row look with its label, and the control (the field look, a hairline, the value) in the
/// value column from the row's end, built as `Part::EDITOR`.
pub(crate) fn default_text_field(this: &mut Ui, c: &TextFieldConfig, text: &str, value: &str) {
    let side = this.space(2) as i64;
    let t = this.theme();
    let hover = State { hovered: c.state.hovered, ..State::default() };
    let (look, field) = (*t.row.pick(hover), *t.field.pick(State { focused: c.state.focused, ..State::default() }));
    this.set_look(&c.row, look, hover);
    this.node_mut(&c.row).inset = Edges { l: side as f32, ..Edges::default() };
    this.text(&c.row, shown(text));
    // the value on its hairline in the control column (`value_control_at` of the row, from its
    // end; the start padding only narrows the room `value_control_w` sees)
    let rh = this.row_height() as f32;
    let pad = (rh - this.control_h()) / 2.0;
    this.end_row(&c.row, side as f32);
    this.node_mut(&c.row).padding.l = side as f32;
    let control = this.part(Part::EDITOR, Node { w: Size::Rel(value_control_w), h: Size::Px(rh - 2.0 * pad), margin: Edges { t: pad, ..Edges::ZERO }, look: field, inset: Edges { l: 2.0, ..Edges::ZERO }, ..Node::DEFAULT });
    this.node_mut(&control).state = State { focused: c.state.focused, ..State::default() };
    if !c.state.focused {
        this.text(&control, value);
    }
}

fn context_menu_body(this: &mut Ui, at: (f32, f32), o: &MenuOpts, label: &str, items: &[(String, bool)]) -> MenuResult {
    let (x, y) = at;
    let submenu_key = key_of(this.current_seed(), &format!("{}-sub", label));
    // the width follows the longest row; an empty label is a separator line; `keys` are the
    // shortcuts at the right of the rows, in ink-2 at the label size
    let row_height = this.row_height() as f32;
    let gap = 9.0;
    let key_size = 8.max(this.font_size() - 2);
    let swatch_pad = if o.swatches.iter().any(|s| s.is_some()) { 14.0 } else { 0.0 };
    let lead = if o.selected.is_none() { 0.0 } else { 14.0 };
    let (view_w, view_h) = this.view_size();
    let chevrons: Vec<i64> = o.submenus.iter().map(|s| s.row).collect();
    let width = view_w.min(this.menu_measure(lead, &o.keys, &chevrons, o.width.unwrap_or(150.0), items, swatch_pad, key_size));
    let height_of = |items: &[(String, bool)]| items.iter().fold(14.0, |h, (text, _)| h + if text.is_empty() { gap } else { row_height });
    let height = height_of(items);
    // a menu opens at the point, moved inside the window
    let bounds = Rect::new(0.0, 0.0, view_w, view_h);
    let Rect { x, y, .. } = place_anchored(Rect::new(x, y, 0.0, 0.0), (width, height), Placement { flip: false, ..Placement::BELOW }, bounds);
    // the submenu of a row: a second menu overlapping this one by a point, level with its row
    let row_top = |index: i64| {
        y + 7.0 + items.iter().take(0.max(index) as usize).fold(0.0, |top, (text, _)| top + if text.is_empty() { gap } else { row_height })
    };
    let sub_geometry = |ui: &mut Ui, sub: &Submenu| -> Rect {
        let sub_width = view_w.min(ui.menu_measure(if sub.current.is_none() { 0.0 } else { 14.0 }, &sub.keys, &[], 0.0, &sub.rows, swatch_pad, key_size));
        let sub_height = height_of(&sub.rows);
        // beside the menu, overlapping it by a point, level with its row; left when no room right
        let beside = Placement { side: Side::Right, align: Align::Start, gap: -1.0, margin: 0.0, flip: true };
        place_anchored(Rect::new(x, row_top(sub.row) - 7.0, width, 0.0), (sub_width, sub_height), beside, bounds)
    };
    // the rows of the submenus are numbered after those of this menu, in order
    let mut bases = Vec::new();
    let mut n = items.len() as i64;
    for sub in &o.submenus {
        bases.push(n);
        n += sub.rows.len() as i64;
    }
    let keep: Vec<Rect> = o.submenus.iter().map(|s| sub_geometry(this, s)).collect();
    let sheet = this.theme().popup.idle;
    let related = if o.submenus.is_empty() { Vec::new() } else { vec![submenu_key] };
    let popts = PopupOpts { look: Some(sheet), keep, related, role: Role::Menu, ..PopupOpts::default() };
    let mut opened: Option<i64> = None;
    let mut picked: Option<i64> = None;
    let menu_items = items.to_vec();
    let shown = this.popup_with_related(popts, (x, y), width, height, label, &mut |ui: &mut Ui| {
        let top = ui.box_("menu-top", BoxOpts::default().flags(BLOCKING).w(Size::Grow).h(Size::Px(6.0)));
        let open_row = ui.state(&top, -1);
        opened = if open_row >= 0 { Some(open_row) } else { None };
        let m = MenuRows {
            items: menu_items.clone(),
            keys: o.keys.clone(),
            selected: o.selected,
            swatches: o.swatches.clone(),
            danger: o.danger.clone(),
            opened,
            chevrons: chevrons.clone(),
            lead_from: o.lead_from,
            swatch_pad,
            gap,
        };
        let (p, hovered) = ui.menu_rows(&m);
        picked = p;
        // the pointer opens the submenu of a row, or closes it on any other
        if let Some(index) = hovered {
            let now = if o.submenus.iter().any(|s| s.row == index) { index } else { -1 };
            if now != open_row {
                ui.set_state(&top, now);
            }
            opened = if now >= 0 { Some(now) } else { None };
        }
        ui.box_("menu-bottom", BoxOpts::default().flags(BLOCKING).w(Size::Grow).h(Size::Px(6.0)));
    });
    if !shown {
        this.dismiss_popup();
        return MenuResult::Dismiss;
    }
    if let Some(index) = picked {
        this.dismiss_popup();
        return MenuResult::Pick(index);
    }
    let sub = opened.and_then(|row| o.submenus.iter().position(|s| s.row == row));
    match sub {
        None => MenuResult::Open,
        Some(si) => {
            let sub = o.submenus[si].clone();
            let Rect { x: sx, y: sy, w: sw, h: sh } = sub_geometry(this, &sub);
            let mut sub_picked: Option<i64> = None;
            let sub_opts = PopupOpts { look: Some(sheet), attached: true, role: Role::Menu, ..PopupOpts::default() };
            this.popup_with_related(sub_opts, (sx, sy), sw, sh, &format!("{}-sub", label), &mut |ui: &mut Ui| {
                ui.box_("menu-top", BoxOpts::default().flags(BLOCKING).w(Size::Grow).h(Size::Px(6.0)));
                let m = MenuRows {
                    items: sub.rows.clone(),
                    keys: sub.keys.clone(),
                    selected: sub.current,
                    swatches: Vec::new(),
                    danger: Vec::new(),
                    opened: None,
                    chevrons: Vec::new(),
                    lead_from: 0,
                    swatch_pad,
                    gap,
                };
                let (p, _) = ui.menu_rows(&m);
                sub_picked = p;
                ui.box_("menu-bottom", BoxOpts::default().flags(BLOCKING).w(Size::Grow).h(Size::Px(6.0)));
            });
            match sub_picked {
                Some(index) => {
                    this.dismiss_popup();
                    MenuResult::Pick(bases[si] + index)
                }
                None => MenuResult::Open,
            }
        }
    }
}
