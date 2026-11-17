//! Text widgets with options: the text field, line field, value field (slide, scrub and validity callbacks), the text area, and the text edit the `Ui` keeps.
//! The non-generic bodies of the key-first calls in `author/fields.rs`.
use crate::composite::{defer_callback_panic, deferred};
use crate::{as_str, as_text, check, lent_str, lent_string, live, AreaCallbacks, Key, LineField, LineFieldResponse, Response, TextArea, TextAreaResponse, TextField, TextFieldResponse, Ui, ValueCallbacks, ValueField, ValueFieldResponse};
use core::ffi::c_void;
use pxui_types::{geom::Rect, ffi::{AreaNotify, PxuiAreaCallbacks, PxuiColorSpan, PxuiCompletion, PxuiDescription, PxuiMaybeRange, PxuiNote, PxuiRange, PxuiRewrite, PxuiSplice, PxuiTextAreaDesc, PxuiTextFieldDesc, PxuiValueFieldDesc, Slice}, ui::{CodeContext, LineStep, TextEdit}, PxuiBox, Str};

pub use pxui_types::ui::ScrubPhase;

/// The most errors, notes, spans, splices and chips one `text_area` call takes (and half as many bracket pairs): they cross in stack buffers.
pub const AREA_DATA: usize = 64;

library!(lib {
    fn pxui_ui_text_area_submit(ui: *mut PxuiUi, desc: *const PxuiTextAreaDesc, cbs: *const PxuiAreaCallbacks, out_text: *mut Str, out_changed: *mut u8, out_submitted: *mut u8) -> i32;
    fn pxui_ui_value_field(ui: *mut PxuiUi, desc: *const PxuiValueFieldDesc, valid: Option<extern "C" fn(*mut c_void, Str) -> u8>, slide: Option<extern "C" fn(*mut c_void, f32, *mut Str)>, scrub: Option<extern "C" fn(*mut c_void, Str, f32, u8, *mut Str)>, user: *mut c_void, out_text: *mut Str, out_changed: *mut u8, out_editing: *mut u8) -> i32;
    fn pxui_ui_patch_field(ui: *mut PxuiUi, box_: *const PxuiBox, desc: *const PxuiValueFieldDesc, text: Str, out: *mut u8) -> i32;
    fn pxui_ui_line_field(ui: *mut PxuiUi, x: f32, y: f32, w: f32, h: f32, grab: u8, placeholder: Str, label: Str, value: Str, text: *mut Str, step: *mut u32) -> i32;
    fn pxui_ui_text_field_keyed(ui: *mut PxuiUi, desc: *const PxuiTextFieldDesc, changed: *mut Str, out_changed: *mut u8) -> i32;
    fn pxui_ui_input_box(ui: *mut PxuiUi, out: *mut PxuiBox) -> i32;
    fn pxui_ui_load_text_edit(ui: *mut PxuiUi, key: i64, text: Str, out_text: *mut Str, caret: *mut u64, anchor: *mut u64) -> i32;
    fn pxui_ui_remember(ui: *mut PxuiUi, text: Str, caret: u64, anchor: u64, typing: u8) -> i32;
});

// What the library's callbacks reach during one `value_field` call: the app's closures and the text a callback produced
// (lent to the library until the next callback).
struct Field<'a> {
    valid: &'a dyn Fn(&str) -> bool,
    slide: Option<&'a crate::SlideFn<'a>>,
    scrub: Option<&'a crate::ScrubFn<'a>>,
    produced: String,
}

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

fn value_desc<'a>(rect: Rect, o: &ValueField<'a>, label: &'a str, value: &'a str) -> PxuiValueFieldDesc {
        let has = |present: bool, flag: u32| if present { flag } else { 0 };
        let (lead, lead_color) = o.lead.unwrap_or(("", Default::default()));
        let (trail, trail_color) = o.trail.unwrap_or(("", Default::default()));
    PxuiValueFieldDesc {
            size: core::mem::size_of::<PxuiValueFieldDesc>() as u32,
            flags: has(o.left, PxuiValueFieldDesc::LEFT) | has(o.edit, PxuiValueFieldDesc::EDIT) | has(o.bare, PxuiValueFieldDesc::BARE) | has(o.kept_drag, PxuiValueFieldDesc::KEPT_DRAG)
                | has(o.size.is_some(), PxuiValueFieldDesc::HAS_SIZE) | has(o.fraction.is_some(), PxuiValueFieldDesc::HAS_FRACTION) | has(o.tracking.is_some(), PxuiValueFieldDesc::HAS_TRACKING)
                | has(o.line.is_some(), PxuiValueFieldDesc::HAS_LINE) | has(o.lead.is_some(), PxuiValueFieldDesc::HAS_LEAD) | has(o.trail.is_some(), PxuiValueFieldDesc::HAS_TRAIL)
                | has(o.display.is_some(), PxuiValueFieldDesc::HAS_DISPLAY) | has(o.placeholder.is_some(), PxuiValueFieldDesc::HAS_PLACEHOLDER),
            font_size: o.size.map_or(0, engine), fraction: o.fraction.unwrap_or_default(), tracking: o.tracking.unwrap_or_default(),
            line: o.line.unwrap_or_default(), lead_color, trail_color, reserved: [0; 4], x: rect.x, y: rect.y, width: rect.w, height: rect.h,
            display: as_str(o.display.unwrap_or("")), placeholder: as_str(o.placeholder.unwrap_or("")),
            lead: as_str(lead), trail: as_str(trail), label: as_str(label), value: as_str(value),
        }
}

/// What the library's text area callbacks reach: the app's closures, and the data a callback produced (lent to the library
/// until its next callback).
struct Area<'a> {
    cb: &'a AreaCallbacks<'a>,
    colors: Vec<PxuiColorSpan>, ranges: Vec<PxuiRange>, completions: Vec<PxuiCompletion>, completion_texts: Vec<pxui_types::ui::Completion>, produced: String,
}

fn area<'a>(user: *mut c_void) -> &'a mut Area<'a> { unsafe { &mut *user.cast::<Area>() } }
fn lend(value: &str) -> Str { as_str(value) }
/// A byte offset the library reports as the engine counts it (`i64`, never negative) as the wrapper's `usize`.
fn offset(at: i64) -> usize { at.max(0) as usize }
/// A byte offset of the wrapper's values as the library counts it.
///
/// Panics: when `at` is over `i64::MAX` (documented on the calls that take offsets; in a callback it surfaces at the widget call).
fn engine(at: usize) -> i64 { i64::try_from(at).expect("pxui: a byte offset over i64::MAX") }
fn utf8<'a>(value: Str) -> &'a str {
    text(value).unwrap_or_else(|| { defer_callback_panic(Box::new("pxui: the library lent text that is not UTF-8")); "" })
}

extern "C" fn on_context(user: *mut c_void, x: f32, y: f32) { let a = area(user); if let Some(f) = &a.cb.on_context { guarded((), &mut || f((x, y))) } }
extern "C" fn on_scrub(user: *mut c_void, phase: u32) { let a = area(user); if let Some(f) = &a.cb.on_scrub { guarded((), &mut || f(if phase == 0 { ScrubPhase::Live } else { ScrubPhase::Done })) } }
extern "C" fn on_scrub_edit(user: *mut c_void, start: i64, end: i64, value: Str) { let a = area(user); if let Some(f) = &a.cb.on_scrub_edit { guarded((), &mut || f((offset(start), offset(end)), utf8(value))) } }
extern "C" fn on_click(user: *mut c_void, at: i64, flag: u8) { let a = area(user); if let Some(f) = &a.cb.on_click { guarded((), &mut || f(offset(at), flag != 0)) } }
extern "C" fn on_caret(user: *mut c_void, at: i64) { let a = area(user); if let Some(f) = &a.cb.on_caret { guarded((), &mut || f(offset(at))) } }
extern "C" fn on_caret_move(user: *mut c_void, at: i64) { let a = area(user); if let Some(f) = &a.cb.on_caret_move { guarded((), &mut || f(offset(at))) } }
extern "C" fn on_select(user: *mut c_void, start: i64, end: i64) { let a = area(user); if let Some(f) = &a.cb.on_select { guarded((), &mut || f((offset(start), offset(end)))) } }
extern "C" fn on_drop(user: *mut c_void, at: i64, dropped: u8, kind: Str, value: Str) {
    let a = area(user);
    if let Some(f) = &a.cb.on_drop {
        let payload = crate::PayloadRef { kind: utf8(kind), value: utf8(value) };
        guarded((), &mut || f(offset(at), if dropped == 0 { crate::DropRef::Hover(payload) } else { crate::DropRef::Dropped(payload) }))
    }
}
extern "C" fn colorize(user: *mut c_void, value: Str, out: *mut Slice<PxuiColorSpan>) {
    let a = area(user);
    let Some(l) = &a.cb.language else { return };
    let value = utf8(value);
    a.colors = guarded(Vec::new(), &mut || (l.colorize)(value).into_iter().map(|(start, end, color)| PxuiColorSpan { start: engine(start), end: engine(end), color, reserved: 0 }).collect());
    unsafe { *out = Slice { ptr: a.colors.as_ptr(), len: a.colors.len() }; }
}
extern "C" fn brackets(user: *mut c_void, value: Str, out: *mut Slice<PxuiRange>) {
    let a = area(user);
    let Some(l) = &a.cb.language else { return };
    let value = utf8(value);
    a.ranges = guarded(Vec::new(), &mut || (l.brackets)(value).into_iter().map(|(start, end)| PxuiRange { start: engine(start), end: engine(end) }).collect());
    unsafe { *out = Slice { ptr: a.ranges.as_ptr(), len: a.ranges.len() }; }
}
extern "C" fn indent(user: *mut c_void, value: Str, caret: i64, out: *mut Str) {
    let a = area(user);
    let Some(l) = &a.cb.language else { return };
    a.produced = guarded(String::new(), &mut || (l.indent)(utf8(value), offset(caret)));
    unsafe { *out = lend(&a.produced); }
}
extern "C" fn complete(user: *mut c_void, value: Str, caret: i64, out: *mut Slice<PxuiCompletion>) {
    let a = area(user);
    let Some(l) = &a.cb.language else { return };
    let value = utf8(value);
    a.completion_texts = guarded(Vec::new(), &mut || (l.complete)(value, offset(caret)));
    a.completions = a.completion_texts.iter().map(|c| PxuiCompletion { start: guarded(0, &mut || engine(c.replace.0)), end: guarded(0, &mut || engine(c.replace.1)), insert: lend(&c.insert), label: lend(&c.label), detail: lend(&c.detail), doc: lend(&c.doc) }).collect();
    unsafe { *out = Slice { ptr: a.completions.as_ptr(), len: a.completions.len() }; }
}
extern "C" fn describe(user: *mut c_void, value: Str, at: i64, out: *mut PxuiDescription) {
    let a = area(user);
    let Some(l) = &a.cb.language else { return };
    let value = utf8(value);
    match guarded(None, &mut || (l.describe)(value, offset(at)).map(|(start, end, text)| (engine(start), engine(end), text))) {
        Some((start, end, text)) => { a.produced = text; unsafe { *out = PxuiDescription { has: 1, reserved: [0; 7], start, end, text: lend(&a.produced) }; } }
        None => unsafe { (*out).has = 0 },
    }
}
extern "C" fn number_at(user: *mut c_void, value: Str, at: i64, out: *mut PxuiMaybeRange) {
    let a = area(user);
    let Some(l) = &a.cb.language else { return };
    let value = utf8(value);
    unsafe { *out = match guarded(None, &mut || (l.number_at)(value, offset(at)).map(|(start, end)| (engine(start), engine(end)))) { Some((start, end)) => PxuiMaybeRange { has: 1, reserved: [0; 7], start, end }, None => PxuiMaybeRange::default() }; }
}
extern "C" fn context_at(user: *mut c_void, value: Str, caret: i64) -> u32 {
    let a = area(user);
    let Some(l) = &a.cb.language else { return 0 };
    match guarded(CodeContext::Code, &mut || (l.context_at)(utf8(value), offset(caret))) { CodeContext::Code => 0, CodeContext::String => 1, CodeContext::Comment => 2 }
}
extern "C" fn rewrite(user: *mut c_void, value: Str, caret: i64, out: *mut PxuiRewrite) {
    let a = area(user);
    let Some(f) = a.cb.language.as_ref().and_then(|l| l.rewrite) else { return };
    let value = utf8(value);
    let (text, caret) = guarded((value.to_owned(), caret), &mut || { let (text, at) = f(value, offset(caret)); (text, engine(at)) });
    a.produced = text;
    unsafe { *out = PxuiRewrite { text: lend(&a.produced), caret }; }
}

impl Ui {
    /// What happened to the text input the last widget call built, read from its box: hover, press, focus; `changed` as given. The box is
    /// named `name` for a screen reader when one is given (a field without a name is unnamed).
    #[track_caller]
    pub(crate) fn input_response(&mut self, changed: bool, name: Option<&str>) -> Response {
        let mut raw = PxuiBox::default();
        check(self, unsafe { lib::pxui_ui_input_box(self.0.as_ptr(), &mut raw) });
        if let Some(name) = name { self.a11y_name(&raw, name); }
        let signal = self.signal(&raw);
        let focused = self.focused(&raw);
        Response { clicked: signal.clicked, hovered: signal.hovered, held: signal.held, focused, activated: signal.clicked && !signal.released, changed, secondary: signal.pressed && signal.button == Some(crate::Signal::BUTTON_SECONDARY), handle: raw }
    }

    /// The one body of `text_field`: the key's label, the body variant around the engine call.
    #[track_caller]
    pub(crate) fn text_field_key(&mut self, key: Key, value: &str, field: &TextField<'_>) -> TextFieldResponse<'_> {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let (mut changed, mut did_change) = (Str::default(), 0);
        let variant = field.body_variant();
        let traits = field.traits();
        let desc = PxuiTextFieldDesc {
            size: core::mem::size_of::<PxuiTextFieldDesc>() as u32, autocorrect: traits.autocorrects() as u8, capitalization: traits.capitalizes() as u8, action: traits.return_key() as u8, reserved: 0,
            key: as_text(label), name: as_str(field.accessible_name()), value: as_text(value),
        };
        if variant != 0 { self.set_variant(pxui_types::theme::BodyKind::text_field, variant); }
        check(self, unsafe { lib::pxui_ui_text_field_keyed(self.0.as_ptr(), &desc, &mut changed, &mut did_change) });
        if variant != 0 { self.set_variant(pxui_types::theme::BodyKind::text_field, 0); }
        let response = self.input_response(did_change != 0, None);
        TextFieldResponse { response, changed: (did_change != 0).then(|| lent_str(changed)) }
    }

    /// The one body of `line_field`.
    #[track_caller]
    pub(crate) fn line_field_key(&mut self, key: Key, value: &str, rect: Rect, field: &LineField<'_>) -> LineFieldResponse<'_> {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let (mut text, mut step) = (Str::default(), 0);
        check(self, unsafe { lib::pxui_ui_line_field(self.0.as_ptr(), rect.x, rect.y, rect.w, rect.h, field.grab as u8, as_str(field.placeholder), as_str(label), as_text(value), &mut text, &mut step) });
        let edited = lent_str(text);
        let changed = (edited != value).then_some(edited);
        let response = self.input_response(changed.is_some(), Some(field.name));
        LineFieldResponse { response, changed, step: match step { 1 => LineStep::Next, 2 => LineStep::Previous, 3 => LineStep::Close, _ => LineStep::None } }
    }

    /// The one body of `text_area`.
    #[track_caller]
    pub(crate) fn text_area_key(&mut self, key: Key, value: &str, rect: Rect, o: &TextArea<'_>, cb: &AreaCallbacks<'_>) -> TextAreaResponse<'_> {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let (at, width, height) = ((rect.x, rect.y), rect.w, rect.h);
        let lang = cb.language.as_ref();
        // the data crosses in fixed stack buffers (no heap per call): at most `AREA_DATA` of each kind, `AREA_DATA / 2` bracket pairs
        assert!(o.errors.len() <= AREA_DATA && o.notes.len() <= AREA_DATA && o.spans.len() <= AREA_DATA && o.splices.len() <= AREA_DATA && o.chips.len() <= AREA_DATA
            && lang.is_none_or(|l| l.pairs.len() <= AREA_DATA / 2), "pxui: a text area takes at most {AREA_DATA} errors, notes, spans, splices and chips and {} bracket pairs", AREA_DATA / 2);
        let mut errors = [0i64; AREA_DATA];
        let mut messages = [PxuiNote { line: 0, start: 0, end: 0, has_range: 0, reserved: [0; 7], text: Str::default() }; AREA_DATA];
        let mut spans = [PxuiRange::default(); AREA_DATA];
        let mut splice = [PxuiSplice { start: 0, end: 0, text: Str::default() }; AREA_DATA];
        let mut chips = [PxuiColorSpan::default(); AREA_DATA];
        let mut pairs = [0u32; AREA_DATA];
        for (slot, line) in errors.iter_mut().zip(o.errors) { *slot = engine(*line); }
        for (slot, n) in messages.iter_mut().zip(o.notes) { *slot = PxuiNote { line: engine(n.line), start: n.range.map_or(0, |r| engine(r.0)), end: n.range.map_or(0, |r| engine(r.1)), has_range: n.range.is_some() as u8, reserved: [0; 7], text: as_str(n.text) }; }
        for (slot, (start, end)) in spans.iter_mut().zip(o.spans) { *slot = PxuiRange { start: engine(*start), end: engine(*end) }; }
        for (slot, s) in splice.iter_mut().zip(o.splices) { *slot = PxuiSplice { start: engine(s.start), end: engine(s.end), text: as_text(s.text) }; }
        for (slot, (start, end, color)) in chips.iter_mut().zip(o.chips) { *slot = PxuiColorSpan { start: engine(*start), end: engine(*end), color: *color, reserved: 0 }; }
        let pair_count = lang.map_or(0, |l| l.pairs.len());
        if let Some(l) = lang { for (slot, (open, close)) in pairs.chunks_exact_mut(2).zip(l.pairs) { slot[0] = *open as u32; slot[1] = *close as u32; } }
        let has = |on: bool, flag: u32| if on { flag } else { 0 };
        let desc = PxuiTextAreaDesc {
            size: core::mem::size_of::<PxuiTextAreaDesc>() as u32,
            flags: has(o.wrap, PxuiTextAreaDesc::WRAP) | has(o.reveal.is_some(), PxuiTextAreaDesc::HAS_REVEAL) | has(o.select.is_some(), PxuiTextAreaDesc::HAS_SELECT)
                | has(lang.is_some(), PxuiTextAreaDesc::HAS_LANGUAGE) | has(lang.is_some_and(|l| l.rewrite.is_some()), PxuiTextAreaDesc::HAS_REWRITE),
            x: at.0, y: at.1, width, height, reveal: o.reveal.map_or(0, |r| engine(r.at)), reveal_nonce: o.reveal.map_or(0, |r| r.request as i64),
            select: o.select.map_or(PxuiRange::default(), |(start, end)| PxuiRange { start: engine(start), end: engine(end) }), label: as_str(label), value: as_text(value),
            errors: Slice { ptr: errors.as_ptr(), len: o.errors.len() }, messages: Slice { ptr: messages.as_ptr(), len: o.notes.len() },
            spans: Slice { ptr: spans.as_ptr(), len: o.spans.len() }, splice: Slice { ptr: splice.as_ptr(), len: o.splices.len() }, chips: Slice { ptr: chips.as_ptr(), len: o.chips.len() },
            comment: as_str(lang.map_or("", |l| l.comment)), pairs: Slice { ptr: pairs.as_ptr(), len: pair_count * 2 },
        };
        let mut ctx = Area { cb, colors: Vec::new(), ranges: Vec::new(), completions: Vec::new(), completion_texts: Vec::new(), produced: String::new() };

        let cbs = PxuiAreaCallbacks {
            size: core::mem::size_of::<PxuiAreaCallbacks>() as u32, reserved: 0, user: &mut ctx as *mut Area as *mut c_void,
            on_context: cb.on_context.is_some().then_some(on_context as extern "C" fn(*mut c_void, f32, f32)),
            on_scrub: cb.on_scrub.is_some().then_some(on_scrub as extern "C" fn(*mut c_void, u32)),
            on_scrub_edit: cb.on_scrub_edit.is_some().then_some(on_scrub_edit as extern "C" fn(*mut c_void, i64, i64, Str)),
            on_click: cb.on_click.is_some().then_some(on_click as extern "C" fn(*mut c_void, i64, u8)),
            on_caret: cb.on_caret.is_some().then_some(on_caret as extern "C" fn(*mut c_void, i64)),
            on_caret_move: cb.on_caret_move.is_some().then_some(on_caret_move as extern "C" fn(*mut c_void, i64)),
            on_select: cb.on_select.is_some().then_some(on_select as extern "C" fn(*mut c_void, i64, i64)) as AreaNotify,
            on_drop: cb.on_drop.is_some().then_some(on_drop as extern "C" fn(*mut c_void, i64, u8, Str, Str)),
            colorize: lang.map(|_| colorize as extern "C" fn(*mut c_void, Str, *mut Slice<PxuiColorSpan>)),
            brackets: lang.map(|_| brackets as extern "C" fn(*mut c_void, Str, *mut Slice<PxuiRange>)),
            indent: lang.map(|_| indent as extern "C" fn(*mut c_void, Str, i64, *mut Str)),
            complete: lang.map(|_| complete as extern "C" fn(*mut c_void, Str, i64, *mut Slice<PxuiCompletion>)),
            describe: lang.map(|_| describe as extern "C" fn(*mut c_void, Str, i64, *mut PxuiDescription)),
            number_at: lang.map(|_| number_at as extern "C" fn(*mut c_void, Str, i64, *mut PxuiMaybeRange)),
            context_at: lang.map(|_| context_at as extern "C" fn(*mut c_void, Str, i64) -> u32),
            rewrite: lang.and_then(|l| l.rewrite).map(|_| rewrite as extern "C" fn(*mut c_void, Str, i64, *mut PxuiRewrite)),
        };
        let (mut out, mut changed, mut submitted) = (Str::default(), 0, 0);
        check(self, unsafe { lib::pxui_ui_text_area_submit(self.0.as_ptr(), &desc, &cbs, &mut out, &mut changed, &mut submitted) });
        let response = self.input_response(changed != 0, Some(o.name));
        TextAreaResponse { response, changed: (changed != 0).then(|| lent_str(out)), submitted: submitted != 0 }
    }

    /// The one body of `value_field`.
    #[track_caller]
    pub(crate) fn value_field_key(&mut self, key: Key, value: &str, rect: Rect, o: &ValueField<'_>, cb: &ValueCallbacks<'_>) -> ValueFieldResponse<'_> {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let desc = value_desc(rect, o, label, value);
        let accept = |_: &str| true;
        let mut field = Field { valid: cb.valid.unwrap_or(&accept), slide: cb.slide, scrub: cb.scrub, produced: String::new() };
        let (mut out, mut changed, mut editing) = (Str::default(), 0, 0);
        check(self, unsafe { lib::pxui_ui_value_field(self.0.as_ptr(), &desc, Some(valid_call), cb.slide.map(|_| slide_call as extern "C" fn(*mut c_void, f32, *mut Str)), cb.scrub.map(|_| scrub_call as extern "C" fn(*mut c_void, Str, f32, u8, *mut Str)), &mut field as *mut Field as *mut c_void, &mut out, &mut changed, &mut editing) });
        let response = self.input_response(changed != 0, Some(o.name));
        ValueFieldResponse { response, changed: (changed != 0).then(|| lent_str(out)), editing: editing != 0 }
    }

    /// Between builds, rewrites the value text and the soft-range line of the value field `field` to `value_text`, as its build lays them out without a lead; true when the patch shows it.
    /// False when a patch cannot show it: a `lead` in `o`, a value too wide beside its trail, a line that had no width when built, or `field` not being a built value field (or gone). Then build the field's view instead. `o` must be the style the field was built with.
    /// Allocates: never when the text fits what the field's nodes held; a longer text grows them.
    /// Panics: when the library refuses the call (a null `Ui`, or `value_text` over the limit), or a size is over `i64::MAX`.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let plain = ui.node("n".into(), pxui::Node::DEFAULT); // not a value field
    ///     assert!(!ui.patch_field(&plain, &pxui::ValueField::DEFAULT, "1.0"));
    /// });
    /// ```
    #[track_caller]
    pub fn patch_field(&mut self, field: &PxuiBox, o: &ValueField<'_>, value_text: &str) -> bool {
        let desc = value_desc(Rect::new(0.0, 0.0, 0.0, 0.0), o, "", value_text);
        let mut out = 0;
        let rc = unsafe { lib::pxui_ui_patch_field(self.0.as_ptr(), field, &desc, as_str(value_text), &mut out) };
        live(self, rc) && out != 0
    }

    /// The edit the `Ui` keeps for the field under `key`, loaded with `text`: its caret, selection and undo history follow the key while the text is unchanged.
    /// When `key` or `text` differs from the last load, the previous edit's undo history is shelved (the three latest are kept) and this one starts at the end of `text`, or resumes a shelved history for the same key and text.
    /// Allocates: the returned `TextEdit` owns a copy of the text.
    /// Identity: `key` is the field's box key (`Handle::box_key`), not a label; two fields with one key share one edit.
    /// Panics: when the library refuses the call (a null `Ui`, or `text` over the limit).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let field = ui.node("field".into(), pxui::Node::DEFAULT);
    ///     let edit = ui.load_text_edit(field.box_key, "hello");
    ///     assert_eq!((edit.text.as_str(), edit.caret, edit.anchor), ("hello", 5, 5));
    /// });
    /// ```
    #[track_caller]
    pub fn load_text_edit(&mut self, key: i64, text: &str) -> TextEdit {
        let (mut out, mut caret, mut anchor) = (Str::default(), 0, 0);
        check(self, unsafe { lib::pxui_ui_load_text_edit(self.0.as_ptr(), key, as_text(text), &mut out, &mut caret, &mut anchor) });
        TextEdit { text: lent_string(out), caret: caret as usize, anchor: anchor as usize }
    }

    /// Snapshots `edit` for undo; with `typing`, consecutive typing at one caret joins the previous snapshot into one step.
    /// Taking a snapshot clears the redo history. The history holds the latest 200 steps and is bounded in bytes overall (`text_undo_bytes`).
    /// Allocates: a copy of the text for each snapshot that is not joined.
    /// Panics: when the library refuses the call (a null `Ui`, or `edit.text` over the limit).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let field = ui.node("field".into(), pxui::Node::DEFAULT);
    ///     let edit = ui.load_text_edit(field.box_key, "hello");
    ///     ui.remember(&edit, false);
    ///     assert!(ui.text_undo_bytes() > 0);
    /// });
    /// ```
    #[track_caller]
    pub fn remember(&mut self, edit: &TextEdit, typing: bool) {
        check(self, unsafe { lib::pxui_ui_remember(self.0.as_ptr(), as_text(&edit.text), edit.caret as u64, edit.anchor as u64, typing as u8) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bodies::BodyConfig;
    use std::cell::RefCell;

    thread_local! { static LABELS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) }; }
    /// A registered text field body that records the label its config carries.
    fn label_probe(_: &mut Ui, c: &crate::bodies::TextFieldConfig) { LABELS.with(|l| l.borrow_mut().push(c.text_of(&c.label).to_owned())); }

    /// A text field is identified by its key, named by `TextField::name` (unnamed without one: never by its key) in the tree and in a registered
    /// body's config, and reports what happened to its row; two fields of one name under different keys do not collide.
    #[test]
    fn a_text_field_is_keyed_by_its_key_named_by_its_name_and_a_registered_body_builds_it() {
        use crate::{FrameIn, TextField, ui::Role};
        let mut ui = Ui::new().expect("library");
        ui.set_a11y(true);
        ui.register_text_field_body("probe", label_probe).expect("field body");
        let themed = { let mut theme = crate::Theme::default(); theme.apply(&ui, "text_field.body = probe").expect("theme"); theme };
        const NAMED: TextField = TextField::DEFAULT.name("Find songs");
        let mut handles = Vec::new();
        let mut build = |ui: &mut Ui| {
            handles.clear();
            ui.themed(&themed, &mut |ui: &mut Ui| {
                for key in ["a", "b"] {
                    let field = ui.text_field(key, "abc", &NAMED);
                    assert!(field.changed.is_none() && !field.response.changed && !field.response.focused);
                    handles.push(field.response.handle);
                }
                ui.text_field("plain", "abc", &TextField::DEFAULT);
            });
        };
        let input = |time: f64| FrameIn { width: 320.0, height: 240.0, time, ..FrameIn::default() };
        for n in 0..2 { LABELS.with(|l| l.borrow_mut().clear()); ui.frame(&input(n as f64), &mut build).expect("frame"); }
        assert_eq!(LABELS.with(|l| l.borrow().clone()), ["Find songs", "Find songs", ""], "the body's config carries the name; an unnamed field is unnamed, never named by its key");
        assert_ne!(handles[0].box_key, handles[1].box_key, "one name under two keys is two fields");
        let names: Vec<&str> = ui.a11y_nodes().iter().filter(|n| n.is_role(Role::TextField)).map(|n| ui.a11y_name_of(n)).collect();
        assert_eq!(names, ["Find songs", "Find songs", ""]);
    }

    /// The five field calls report a `Response` whose `changed` agrees with `changed.is_some()`, name their row only when the style does, and refuse
    /// an offset over `i64::MAX` loudly.
    #[test]
    fn field_responses_agree_with_their_text_name_their_row_only_by_style_and_refuse_huge_offsets() {
        use crate::{ui::{Rect, Role}, AreaCallbacks, LineField, SearchField, TextArea, ValueCallbacks, ValueField};
        let mut ui = Ui::new().expect("library");
        ui.set_a11y(true);
        let rect = Rect::new(0.0, 0.0, 100.0, 20.0);
        let (mut search, mut line, mut value, mut area) = (None, None, None, None);
        for n in 0..2 {
            let input = crate::FrameIn { width: 320.0, height: 240.0, time: n as f64, ..crate::FrameIn::default() };
            ui.frame(&input, &mut |ui: &mut Ui| {
                search = Some(ui.search_field("s", "x", &SearchField::DEFAULT.name("Find")).response);
                line = Some(ui.line_field("l", "x", rect, &LineField::DEFAULT).response);
                value = Some(ui.value_field("v", "1", Rect::new(0.0, 30.0, 100.0, 20.0), &ValueField::DEFAULT.name("Scale"), &ValueCallbacks::DEFAULT).response);
                area = Some(ui.text_area("a", "x", Rect::new(0.0, 60.0, 100.0, 40.0), &TextArea::DEFAULT, &AreaCallbacks::DEFAULT).response);
            }).expect("frame");
        }
        for r in [search, line, value, area].into_iter().flatten() { assert!(!r.changed && !r.clicked); }
        let names = |ui: &Ui, role| ui.a11y_nodes().iter().filter(|n| n.is_role(role)).map(|n| ui.a11y_name_of(n).to_owned()).collect::<Vec<_>>();
        assert!(names(&ui, Role::TextField).iter().any(|n| n == "Find"), "a named search field is named");
        assert!(names(&ui, Role::TextArea).iter().all(|n| n.is_empty()), "an unnamed area is unnamed");
        let huge = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| engine(usize::MAX)));
        assert!(huge.is_err(), "an offset over i64::MAX is refused");
    }

    #[test]
    fn validator_panics_are_deferred_across_the_c_callback() {
        let valid = |_: &str| -> bool { panic!("validator failed") };
        let mut field = Field { valid: &valid, slide: None, scrub: None, produced: String::new() };
        let value = b"x";
        assert_eq!(valid_call(&mut field as *mut Field as *mut c_void, Str { ptr: value.as_ptr(), len: value.len() }), 0);
        assert!(crate::composite::take_callback_panic().is_some());
    }
}
