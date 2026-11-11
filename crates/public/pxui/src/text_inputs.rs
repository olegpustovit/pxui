//! Text widgets with options: the value field (slide, scrub and validity callbacks), the text area, and the text edit the `Ui` keeps.
use crate::composite::{defer_callback_panic, deferred};
use crate::{as_str, check, lent_string, live, Ui};
use core::ffi::c_void;
use pxui_types::{ffi::{AreaNotify, PxuiAreaCallbacks, PxuiColorSpan, PxuiCompletion, PxuiDescription, PxuiMaybeRange, PxuiNote, PxuiRange, PxuiRewrite, PxuiSplice, PxuiTextAreaDesc, PxuiValueFieldDesc, Slice}, ui::{CodeContext, Drop, Payload, TextEdit}, PxuiBox, Str};

pub use pxui_types::ui::ScrubPhase;
pub use crate::opts::{Language, TextAreaOpts, ValueFieldOpts};

library!(lib {
    fn pxui_ui_text_area_submit(ui: *mut PxuiUi, desc: *const PxuiTextAreaDesc, cbs: *const PxuiAreaCallbacks, out_text: *mut Str, out_changed: *mut u8, out_submitted: *mut u8) -> i32;
    fn pxui_ui_value_field(ui: *mut PxuiUi, desc: *const PxuiValueFieldDesc, valid: Option<extern "C" fn(*mut c_void, Str) -> u8>, slide: Option<extern "C" fn(*mut c_void, f32, *mut Str)>, scrub: Option<extern "C" fn(*mut c_void, Str, f32, u8, *mut Str)>, user: *mut c_void, out_text: *mut Str, out_changed: *mut u8, out_editing: *mut u8) -> i32;
    fn pxui_ui_patch_field(ui: *mut PxuiUi, box_: *const PxuiBox, desc: *const PxuiValueFieldDesc, text: Str, out: *mut u8) -> i32;
    fn pxui_ui_load_text_edit(ui: *mut PxuiUi, key: i64, text: Str, out_text: *mut Str, caret: *mut u64, anchor: *mut u64) -> i32;
    fn pxui_ui_remember(ui: *mut PxuiUi, text: Str, caret: u64, anchor: u64, typing: u8) -> i32;
});

// What the library's callbacks reach during one `value_field` call: the app's closures and the text a callback produced
// (lent to the library until the next callback).
struct Field<'a> { valid: &'a dyn Fn(&str) -> bool, slide: Option<&'a dyn Fn(f32) -> String>, scrub: Option<&'a dyn Fn(&str, f32, bool) -> String>, produced: String }

fn text<'a>(value: Str) -> Option<&'a str> {
    if value.len != 0 && value.ptr.is_null() { return None; }
    core::str::from_utf8(if value.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(value.ptr, value.len) } }).ok()
}

/// Runs an app closure for a library callback: nothing once a panic is waiting, and a panic is kept to resume at the call.
fn guarded<R>(fallback: R, f: &mut dyn FnMut() -> R) -> R {
    if deferred() { return fallback; }
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) { Ok(value) => value, Err(panic) => { defer_callback_panic(panic); fallback } }
}

extern "C" fn valid_call(user: *mut c_void, value: Str) -> u8 {
    let field = unsafe { &*user.cast::<Field>() };
    let Some(value) = text(value) else { return 0 };
    guarded(0, &mut || (field.valid)(value) as u8)
}

extern "C" fn slide_call(user: *mut c_void, fraction: f32, out: *mut Str) {
    let field = unsafe { &mut *user.cast::<Field>() };
    let Some(slide) = field.slide else { return };
    field.produced = guarded(String::new(), &mut || slide(fraction));
    unsafe { *out = as_str(&field.produced); }
}

extern "C" fn scrub_call(user: *mut c_void, origin: Str, dx: f32, coarse: u8, out: *mut Str) {
    let field = unsafe { &mut *user.cast::<Field>() };
    let (Some(scrub), Some(origin)) = (field.scrub, text(origin)) else { return };
    field.produced = guarded(String::new(), &mut || scrub(origin, dx, coarse != 0));
    unsafe { *out = as_str(&field.produced); }
}

fn value_desc<'a>(at: (f32, f32), width: f32, height: f32, o: &'a ValueFieldOpts, label: &'a str, value: &'a str) -> PxuiValueFieldDesc {
        let has = |present: bool, flag: u32| if present { flag } else { 0 };
        let (lead, lead_color) = o.lead.as_ref().map_or(("", Default::default()), |(text, color)| (text.as_str(), *color));
        let (trail, trail_color) = o.trail.as_ref().map_or(("", Default::default()), |(text, color)| (text.as_str(), *color));
    PxuiValueFieldDesc {
            size: core::mem::size_of::<PxuiValueFieldDesc>() as u32,
            flags: has(o.left, PxuiValueFieldDesc::LEFT) | has(o.edit, PxuiValueFieldDesc::EDIT) | has(o.bare, PxuiValueFieldDesc::BARE) | has(o.kept_drag, PxuiValueFieldDesc::KEPT_DRAG)
                | has(o.size.is_some(), PxuiValueFieldDesc::HAS_SIZE) | has(o.fraction.is_some(), PxuiValueFieldDesc::HAS_FRACTION) | has(o.tracking.is_some(), PxuiValueFieldDesc::HAS_TRACKING)
                | has(o.line.is_some(), PxuiValueFieldDesc::HAS_LINE) | has(o.lead.is_some(), PxuiValueFieldDesc::HAS_LEAD) | has(o.trail.is_some(), PxuiValueFieldDesc::HAS_TRAIL)
                | has(o.display.is_some(), PxuiValueFieldDesc::HAS_DISPLAY) | has(o.placeholder.is_some(), PxuiValueFieldDesc::HAS_PLACEHOLDER),
            font_size: o.size.unwrap_or_default(), fraction: o.fraction.unwrap_or_default(), tracking: o.tracking.unwrap_or_default(),
            line: o.line.unwrap_or_default(), lead_color, trail_color, reserved: [0; 4], x: at.0, y: at.1, width, height,
            display: as_str(o.display.as_deref().unwrap_or("")), placeholder: as_str(o.placeholder.as_deref().unwrap_or("")),
            lead: as_str(lead), trail: as_str(trail), label: as_str(label), value: as_str(value),
        }
}

/// What the library's text area callbacks reach: the app's closures, and the data a callback produced (lent to the library
/// until its next callback).
struct Area<'a> {
    opts: &'a TextAreaOpts,
    colors: Vec<PxuiColorSpan>, ranges: Vec<PxuiRange>, completions: Vec<PxuiCompletion>, completion_texts: Vec<pxui_types::ui::Completion>, produced: String,
}

fn area<'a>(user: *mut c_void) -> &'a mut Area<'a> { unsafe { &mut *user.cast::<Area>() } }
fn lend(value: &str) -> Str { as_str(value) }
fn utf8<'a>(value: Str) -> &'a str {
    text(value).unwrap_or_else(|| { defer_callback_panic(Box::new("pxui: the library lent text that is not UTF-8")); "" })
}

extern "C" fn on_context(user: *mut c_void, x: f32, y: f32) { let a = area(user); if let Some(f) = &a.opts.on_context { guarded((), &mut || f((x, y))) } }
extern "C" fn on_scrub(user: *mut c_void, phase: u32) { let a = area(user); if let Some(f) = &a.opts.on_scrub { guarded((), &mut || f(if phase == 0 { ScrubPhase::Live } else { ScrubPhase::Done })) } }
extern "C" fn on_scrub_edit(user: *mut c_void, start: i64, end: i64, value: Str) { let a = area(user); if let Some(f) = &a.opts.on_scrub_edit { guarded((), &mut || f((start, end), utf8(value))) } }
extern "C" fn on_click(user: *mut c_void, at: i64, flag: u8) { let a = area(user); if let Some(f) = &a.opts.on_click { guarded((), &mut || f(at, flag != 0)) } }
extern "C" fn on_caret(user: *mut c_void, at: i64) { let a = area(user); if let Some(f) = &a.opts.on_caret { guarded((), &mut || f(at)) } }
extern "C" fn on_caret_move(user: *mut c_void, at: i64) { let a = area(user); if let Some(f) = &a.opts.on_caret_move { guarded((), &mut || f(at)) } }
extern "C" fn on_select(user: *mut c_void, start: i64, end: i64) { let a = area(user); if let Some(f) = &a.opts.on_select { guarded((), &mut || f((start, end))) } }
extern "C" fn on_drop(user: *mut c_void, at: i64, dropped: u8, kind: Str, value: Str) {
    let a = area(user);
    if let Some(f) = &a.opts.on_drop {
        let payload = Payload { kind: utf8(kind).to_owned(), value: utf8(value).to_owned() };
        guarded((), &mut || f(at, if dropped == 0 { Drop::Hover(payload.clone()) } else { Drop::Dropped(payload.clone()) }))
    }
}
extern "C" fn colorize(user: *mut c_void, value: Str, out: *mut Slice<PxuiColorSpan>) {
    let a = area(user);
    let Some(l) = &a.opts.language else { return };
    let spans = guarded(Vec::new(), &mut || (l.colorize)(utf8(value)));
    a.colors = spans.into_iter().map(|(start, end, color)| PxuiColorSpan { start, end, color, reserved: 0 }).collect();
    unsafe { *out = Slice { ptr: a.colors.as_ptr(), len: a.colors.len() }; }
}
extern "C" fn brackets(user: *mut c_void, value: Str, out: *mut Slice<PxuiRange>) {
    let a = area(user);
    let Some(l) = &a.opts.language else { return };
    let pairs = guarded(Vec::new(), &mut || (l.brackets)(utf8(value)));
    a.ranges = pairs.into_iter().map(|(start, end)| PxuiRange { start, end }).collect();
    unsafe { *out = Slice { ptr: a.ranges.as_ptr(), len: a.ranges.len() }; }
}
extern "C" fn indent(user: *mut c_void, value: Str, caret: i64, out: *mut Str) {
    let a = area(user);
    let Some(l) = &a.opts.language else { return };
    a.produced = guarded(String::new(), &mut || (l.indent)(utf8(value), caret));
    unsafe { *out = lend(&a.produced); }
}
extern "C" fn complete(user: *mut c_void, value: Str, caret: i64, out: *mut Slice<PxuiCompletion>) {
    let a = area(user);
    let Some(l) = &a.opts.language else { return };
    a.completion_texts = guarded(Vec::new(), &mut || (l.complete)(utf8(value), caret));
    a.completions = a.completion_texts.iter().map(|c| PxuiCompletion { start: c.replace.0, end: c.replace.1, insert: lend(&c.insert), label: lend(&c.label), detail: lend(&c.detail), doc: lend(&c.doc) }).collect();
    unsafe { *out = Slice { ptr: a.completions.as_ptr(), len: a.completions.len() }; }
}
extern "C" fn describe(user: *mut c_void, value: Str, at: i64, out: *mut PxuiDescription) {
    let a = area(user);
    let Some(l) = &a.opts.language else { return };
    match guarded(None, &mut || (l.describe)(utf8(value), at)) {
        Some((start, end, text)) => { a.produced = text; unsafe { *out = PxuiDescription { has: 1, reserved: [0; 7], start, end, text: lend(&a.produced) }; } }
        None => unsafe { (*out).has = 0 },
    }
}
extern "C" fn number_at(user: *mut c_void, value: Str, at: i64, out: *mut PxuiMaybeRange) {
    let a = area(user);
    let Some(l) = &a.opts.language else { return };
    unsafe { *out = match guarded(None, &mut || (l.number_at)(utf8(value), at)) { Some((start, end)) => PxuiMaybeRange { has: 1, reserved: [0; 7], start, end }, None => PxuiMaybeRange::default() }; }
}
extern "C" fn context_at(user: *mut c_void, value: Str, caret: i64) -> u32 {
    let a = area(user);
    let Some(l) = &a.opts.language else { return 0 };
    match guarded(CodeContext::Code, &mut || (l.context_at)(utf8(value), caret)) { CodeContext::Code => 0, CodeContext::String => 1, CodeContext::Comment => 2 }
}
extern "C" fn rewrite(user: *mut c_void, value: Str, caret: i64, out: *mut PxuiRewrite) {
    let a = area(user);
    let Some(f) = a.opts.language.as_ref().and_then(|l| l.rewrite.as_ref()) else { return };
    let (text, caret) = guarded((utf8(value).to_owned(), caret), &mut || f(utf8(value), caret));
    a.produced = text;
    unsafe { *out = PxuiRewrite { text: lend(&a.produced), caret }; }
}

impl Ui {
    /// A multiline text area: the new text and whether it was submitted. The app's closures in `options` run during this call.
    #[track_caller]
    pub fn text_area_submit(&mut self, at: (f32, f32), width: f32, height: f32, o: &TextAreaOpts, label: &str, value: &str) -> (String, bool) {
        let ranges = |v: &[(i64, i64)]| -> Vec<PxuiRange> { v.iter().map(|(start, end)| PxuiRange { start: *start, end: *end }).collect() };
        let messages: Vec<PxuiNote> = o.messages.iter().map(|(line, range, text)| PxuiNote { line: *line, start: range.map_or(0, |r| r.0), end: range.map_or(0, |r| r.1), has_range: range.is_some() as u8, reserved: [0; 7], text: as_str(text) }).collect();
        let spans = ranges(&o.spans);
        let splice: Vec<PxuiSplice> = o.splice.iter().map(|(start, end, text)| PxuiSplice { start: *start, end: *end, text: as_str(text) }).collect();
        let chips: Vec<PxuiColorSpan> = o.chips.iter().map(|(start, end, color)| PxuiColorSpan { start: *start, end: *end, color: *color, reserved: 0 }).collect();
        let pairs: Vec<u32> = o.language.as_ref().map_or_else(Vec::new, |l| l.pairs.iter().flat_map(|(a, b)| [*a as u32, *b as u32]).collect());
        let lang = o.language.as_ref();
        let has = |on: bool, flag: u32| if on { flag } else { 0 };
        let desc = PxuiTextAreaDesc {
            size: core::mem::size_of::<PxuiTextAreaDesc>() as u32,
            flags: has(o.wrap, PxuiTextAreaDesc::WRAP) | has(o.reveal.is_some(), PxuiTextAreaDesc::HAS_REVEAL) | has(o.select.is_some(), PxuiTextAreaDesc::HAS_SELECT)
                | has(lang.is_some(), PxuiTextAreaDesc::HAS_LANGUAGE) | has(lang.is_some_and(|l| l.rewrite.is_some()), PxuiTextAreaDesc::HAS_REWRITE),
            x: at.0, y: at.1, width, height, reveal: o.reveal.unwrap_or_default(), reveal_nonce: o.reveal_nonce,
            select: o.select.map_or(PxuiRange::default(), |(start, end)| PxuiRange { start, end }), label: as_str(label), value: as_str(value),
            errors: Slice { ptr: o.errors.as_ptr(), len: o.errors.len() }, messages: Slice { ptr: messages.as_ptr(), len: messages.len() },
            spans: Slice { ptr: spans.as_ptr(), len: spans.len() }, splice: Slice { ptr: splice.as_ptr(), len: splice.len() }, chips: Slice { ptr: chips.as_ptr(), len: chips.len() },
            comment: as_str(lang.map_or("", |l| l.comment.as_str())), pairs: Slice { ptr: pairs.as_ptr(), len: pairs.len() },
        };
        let mut ctx = Area { opts: o, colors: Vec::new(), ranges: Vec::new(), completions: Vec::new(), completion_texts: Vec::new(), produced: String::new() };

        let cbs = PxuiAreaCallbacks {
            size: core::mem::size_of::<PxuiAreaCallbacks>() as u32, reserved: 0, user: &mut ctx as *mut Area as *mut c_void,
            on_context: o.on_context.is_some().then_some(on_context as extern "C" fn(*mut c_void, f32, f32)),
            on_scrub: o.on_scrub.is_some().then_some(on_scrub as extern "C" fn(*mut c_void, u32)),
            on_scrub_edit: o.on_scrub_edit.is_some().then_some(on_scrub_edit as extern "C" fn(*mut c_void, i64, i64, Str)),
            on_click: o.on_click.is_some().then_some(on_click as extern "C" fn(*mut c_void, i64, u8)),
            on_caret: o.on_caret.is_some().then_some(on_caret as extern "C" fn(*mut c_void, i64)),
            on_caret_move: o.on_caret_move.is_some().then_some(on_caret_move as extern "C" fn(*mut c_void, i64)),
            on_select: o.on_select.is_some().then_some(on_select as extern "C" fn(*mut c_void, i64, i64)) as AreaNotify,
            on_drop: o.on_drop.is_some().then_some(on_drop as extern "C" fn(*mut c_void, i64, u8, Str, Str)),
            colorize: lang.map(|_| colorize as extern "C" fn(*mut c_void, Str, *mut Slice<PxuiColorSpan>)),
            brackets: lang.map(|_| brackets as extern "C" fn(*mut c_void, Str, *mut Slice<PxuiRange>)),
            indent: lang.map(|_| indent as extern "C" fn(*mut c_void, Str, i64, *mut Str)),
            complete: lang.map(|_| complete as extern "C" fn(*mut c_void, Str, i64, *mut Slice<PxuiCompletion>)),
            describe: lang.map(|_| describe as extern "C" fn(*mut c_void, Str, i64, *mut PxuiDescription)),
            number_at: lang.map(|_| number_at as extern "C" fn(*mut c_void, Str, i64, *mut PxuiMaybeRange)),
            context_at: lang.map(|_| context_at as extern "C" fn(*mut c_void, Str, i64) -> u32),
            rewrite: lang.and_then(|l| l.rewrite.as_ref()).map(|_| rewrite as extern "C" fn(*mut c_void, Str, i64, *mut PxuiRewrite)),
        };
        let (mut out, mut changed, mut submitted) = (Str::default(), 0, 0);
        check(self, unsafe { lib::pxui_ui_text_area_submit(self.0.as_ptr(), &desc, &cbs, &mut out, &mut changed, &mut submitted) });
        (lent_string(out), submitted != 0)
    }

    /// A compact field: the new text and whether a text editor is open. Only text `valid` accepts commits.
    #[allow(clippy::too_many_arguments)]
    #[track_caller]
    pub fn value_field(&mut self, at: (f32, f32), width: f32, height: f32, o: &ValueFieldOpts, valid: &dyn Fn(&str) -> bool, label: &str, value: &str) -> (String, bool) {
        let desc = value_desc(at, width, height, o, label, value);
        let slide = o.slide.as_deref();
        let scrub = o.scrub.as_deref();
        let mut field = Field { valid, slide, scrub, produced: String::new() };
        let (mut out, mut changed, mut editing) = (Str::default(), 0, 0);
        check(self, unsafe { lib::pxui_ui_value_field(self.0.as_ptr(), &desc, Some(valid_call), slide.map(|_| slide_call as extern "C" fn(*mut c_void, f32, *mut Str)), scrub.map(|_| scrub_call as extern "C" fn(*mut c_void, Str, f32, u8, *mut Str)), &mut field as *mut Field as *mut c_void, &mut out, &mut changed, &mut editing) });
        (lent_string(out), editing != 0)
    }

    /// A value field's value and soft-range line written by patches, as its build lays them out without a lead: false when a
    /// patch cannot show it (a lead, a value too wide beside its trail, the line had no width when built, a stale handle).
    #[track_caller]
    pub fn patch_field(&mut self, field: &PxuiBox, o: &ValueFieldOpts, value_text: &str) -> bool {
        let desc = value_desc((0.0, 0.0), 0.0, 0.0, o, "", value_text);
        let mut out = 0;
        let rc = unsafe { lib::pxui_ui_patch_field(self.0.as_ptr(), field, &desc, as_str(value_text), &mut out) };
        live(self, rc) && out != 0
    }

    /// The edit the `Ui` keeps for the field under `key`, loaded with `text`.
    #[track_caller]
    pub fn load_text_edit(&mut self, key: i64, text: &str) -> TextEdit {
        let (mut out, mut caret, mut anchor) = (Str::default(), 0, 0);
        check(self, unsafe { lib::pxui_ui_load_text_edit(self.0.as_ptr(), key, as_str(text), &mut out, &mut caret, &mut anchor) });
        TextEdit { text: lent_string(out), caret: caret as usize, anchor: anchor as usize }
    }

    /// Snapshots `edit` for undo; `typing` joins consecutive typing into one step.
    #[track_caller]
    pub fn remember(&mut self, edit: &TextEdit, typing: bool) {
        check(self, unsafe { lib::pxui_ui_remember(self.0.as_ptr(), as_str(&edit.text), edit.caret as u64, edit.anchor as u64, typing as u8) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validator_panics_are_deferred_across_the_c_callback() {
        let valid = |_: &str| -> bool { panic!("validator failed") };
        let mut field = Field { valid: &valid, slide: None, scrub: None, produced: String::new() };
        let value = b"x";
        assert_eq!(valid_call(&mut field as *mut Field as *mut c_void, Str { ptr: value.as_ptr(), len: value.len() }), 0);
        assert!(crate::composite::take_callback_panic().is_some());
    }
}
