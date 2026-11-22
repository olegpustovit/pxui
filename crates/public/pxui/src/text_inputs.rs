//! Text widgets with options: the text field, line field, value field (slide, scrub and validity callbacks), the text area, and the text edit the `Ui` keeps.
//! The non-generic bodies of the key-first calls in `author/fields.rs`.
use core::mem::MaybeUninit;
use crate::Handle;
use crate::composite::{defer_callback_panic, deferred};
use crate::{as_str, as_text, check, lent_str, lent_string, live, AreaCallbacks, Key, LineField, LineFieldResponse, Response, TextArea, TextAreaResponse, TextField, TextFieldResponse, Ui, ValueCallbacks, ValueField, ValueFieldResponse};
use core::ffi::c_void;
use pxui_types::{geom::Rect, ffi::{AreaNotify, PxuiAreaCallbacks, PxuiResponse, PxuiColorSpan, PxuiCompletion, PxuiDescription, PxuiMaybeRange, PxuiNote, PxuiRange, PxuiRewrite, PxuiSplice, PxuiTextAreaDesc, PxuiTextFieldDesc, PxuiValueFieldDesc, Slice}, ui::{CodeContext, LineStep, TextEdit}, PxuiBox, Str};

pub use pxui_types::ui::ScrubPhase;

/// The most errors, notes, spans, splices and chips one `text_area` call takes (and half as many bracket pairs): they cross in stack buffers.
pub const AREA_DATA: usize = 64;

library!(lib {
    fn pxui_ui_text_area(ui: *mut PxuiUi, desc: *const PxuiTextAreaDesc, node: *const pxui_types::Node, cbs: *const PxuiAreaCallbacks, name: Str, out_text: *mut Str, out_changed: *mut u8, out_submitted: *mut u8, response: *mut PxuiResponse) -> i32;
    fn pxui_ui_value_field(ui: *mut PxuiUi, desc: *const PxuiValueFieldDesc, valid: Option<extern "C" fn(*mut c_void, Str) -> u8>, slide: Option<extern "C" fn(*mut c_void, f32, *mut Str)>, scrub: Option<extern "C" fn(*mut c_void, Str, f32, u8, *mut Str)>, user: *mut c_void, name: Str, out_text: *mut Str, out_changed: *mut u8, out_editing: *mut u8, response: *mut PxuiResponse) -> i32;
    fn pxui_ui_patch_field(ui: *mut PxuiUi, box_: *const PxuiBox, desc: *const PxuiValueFieldDesc, text: Str, out: *mut u8) -> i32;
    fn pxui_ui_line_field(ui: *mut PxuiUi, x: f32, y: f32, w: f32, h: f32, grab: u8, placeholder: Str, label: Str, value: Str, name: Str, text: *mut Str, step: *mut u32, response: *mut PxuiResponse) -> i32;
    fn pxui_ui_text_field(ui: *mut PxuiUi, desc: *const PxuiTextFieldDesc, changed: *mut Str, out_changed: *mut u8, response: *mut PxuiResponse) -> i32;
    fn pxui_ui_load_text_edit(ui: *mut PxuiUi, key: i64, text: Str, out_text: *mut Str, caret: *mut u64, anchor: *mut u64) -> i32;
    fn pxui_ui_remember(ui: *mut PxuiUi, key: i64, text: Str, caret: u64, anchor: u64, typing: u8) -> i32;
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
/// The callback stores its result through its captures; on a skip or a panic the caller's fallback stays.
fn guarded(f: &mut dyn FnMut()) {
    if deferred() { return; }
    if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) { defer_callback_panic(panic); }
}

extern "C" fn valid_call(user: *mut c_void, value: Str) -> u8 {
    let field = unsafe { &*user.cast::<Field>() };
    let Some(value) = text(value) else { return 0 };
    { let mut out = 0; guarded(&mut || out = (field.valid)(value) as u8); out }
}

extern "C" fn slide_call(user: *mut c_void, fraction: f32, out: *mut Str) {
    let field = unsafe { &mut *user.cast::<Field>() };
    let Some(slide) = field.slide else { return };
    field.produced = { let mut out = String::new(); guarded(&mut || out = slide(fraction)); out };
    unsafe { *out = as_str(&field.produced); }
}

extern "C" fn scrub_call(user: *mut c_void, origin: Str, dx: f32, coarse: u8, out: *mut Str) {
    let field = unsafe { &mut *user.cast::<Field>() };
    let (Some(scrub), Some(origin)) = (field.scrub, text(origin)) else { return };
    field.produced = { let mut out = String::new(); guarded(&mut || out = scrub(origin, dx, coarse != 0)); out };
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
    colors: Vec<PxuiColorSpan>, ranges: Vec<PxuiRange>, completions: Vec<PxuiCompletion>, completion_texts: Vec<pxui_types::ui::Completion>, produced: String, description: Option<String>,
}

thread_local! { static DESCRIPTION: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) }; }

impl Drop for Area<'_> {
    fn drop(&mut self) {
        if let Some(mut doc) = self.description.take() {
            // Oversized callback output is clipped for display. Keep only display-sized scratch.
            if doc.capacity() <= crate::MAX_LABEL_BYTES * 2 {
                doc.clear();
                let _ = DESCRIPTION.try_with(|pool| pool.borrow_mut().push(doc));
            }
        }
    }
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

extern "C" fn on_context(user: *mut c_void, x: f32, y: f32) { let a = area(user); if let Some(f) = &a.cb.on_context { guarded(&mut || f((x, y))) } }
extern "C" fn on_scrub(user: *mut c_void, phase: u32) { let a = area(user); if let Some(f) = &a.cb.on_scrub { guarded(&mut || f(if phase == 0 { ScrubPhase::Live } else { ScrubPhase::Done })) } }
extern "C" fn on_scrub_edit(user: *mut c_void, start: i64, end: i64, value: Str) { let a = area(user); if let Some(f) = &a.cb.on_scrub_edit { guarded(&mut || f((offset(start), offset(end)), utf8(value))) } }
extern "C" fn on_click(user: *mut c_void, at: i64, flag: u8) { let a = area(user); if let Some(f) = &a.cb.on_click { guarded(&mut || f(offset(at), flag != 0)) } }
extern "C" fn on_caret(user: *mut c_void, at: i64) { let a = area(user); if let Some(f) = &a.cb.on_caret { guarded(&mut || f(offset(at))) } }
extern "C" fn on_caret_move(user: *mut c_void, at: i64) { let a = area(user); if let Some(f) = &a.cb.on_caret_move { guarded(&mut || f(offset(at))) } }
extern "C" fn on_select(user: *mut c_void, start: i64, end: i64) { let a = area(user); if let Some(f) = &a.cb.on_select { guarded(&mut || f((offset(start), offset(end)))) } }
extern "C" fn on_drop(user: *mut c_void, at: i64, dropped: u8, kind: Str, value: Str) {
    let a = area(user);
    if let Some(f) = &a.cb.on_drop {
        let payload = crate::PayloadRef { kind: utf8(kind), value: utf8(value) };
        guarded(&mut || f(offset(at), if dropped == 0 { crate::DropRef::Hover(payload) } else { crate::DropRef::Dropped(payload) }))
    }
}
extern "C" fn colorize(user: *mut c_void, value: Str, out: *mut Slice<PxuiColorSpan>) {
    let a = area(user);
    let Some(l) = &a.cb.language else { return };
    let value = utf8(value);
    a.colors = { let mut out = Vec::new(); guarded(&mut || out = (l.colorize)(value).into_iter().map(|(start, end, color)| PxuiColorSpan { start: engine(start), end: engine(end), color, reserved: 0 }).collect()); out };
    unsafe { *out = Slice { ptr: a.colors.as_ptr(), len: a.colors.len() }; }
}
extern "C" fn brackets(user: *mut c_void, value: Str, out: *mut Slice<PxuiRange>) {
    let a = area(user);
    let Some(l) = &a.cb.language else { return };
    let value = utf8(value);
    a.ranges = { let mut out = Vec::new(); guarded(&mut || out = (l.brackets)(value).into_iter().map(|(start, end)| PxuiRange { start: engine(start), end: engine(end) }).collect()); out };
    unsafe { *out = Slice { ptr: a.ranges.as_ptr(), len: a.ranges.len() }; }
}
extern "C" fn indent(user: *mut c_void, value: Str, caret: i64, out: *mut Str) {
    let a = area(user);
    let Some(l) = &a.cb.language else { return };
    a.produced = { let mut out = String::new(); guarded(&mut || out = (l.indent)(utf8(value), offset(caret))); out };
    unsafe { *out = lend(&a.produced); }
}
extern "C" fn complete(user: *mut c_void, value: Str, caret: i64, out: *mut Slice<PxuiCompletion>) {
    let a = area(user);
    let Some(l) = &a.cb.language else { return };
    let value = utf8(value);
    a.completion_texts = { let mut out = Vec::new(); guarded(&mut || out = (l.complete)(value, offset(caret))); out };
    a.completions = a.completion_texts.iter().map(|c| PxuiCompletion { start: { let mut out = 0; guarded(&mut || out = engine(c.replace.0)); out }, end: { let mut out = 0; guarded(&mut || out = engine(c.replace.1)); out }, insert: lend(&c.insert), label: lend(&c.label), detail: lend(&c.detail), doc: lend(&c.doc) }).collect();
    unsafe { *out = Slice { ptr: a.completions.as_ptr(), len: a.completions.len() }; }
}
extern "C" fn describe(user: *mut c_void, value: Str, at: i64, out: *mut PxuiDescription) {
    let a = area(user);
    let Some(l) = &a.cb.language else { return };
    let value = utf8(value);
    let doc = a.description.get_or_insert_with(|| DESCRIPTION.try_with(|pool| pool.borrow_mut().pop()).ok().flatten().unwrap_or_default());
    doc.clear();
    match { let mut out = None; guarded(&mut || out = (l.describe)(value, offset(at), doc).map(|(start, end)| (engine(start), engine(end)))); out } {
        Some((start, end)) => { unsafe { *out = PxuiDescription { has: 1, reserved: [0; 7], start, end, text: lend(doc) }; } }
        None => unsafe { (*out).has = 0 },
    }
}
extern "C" fn number_at(user: *mut c_void, value: Str, at: i64, out: *mut PxuiMaybeRange) {
    let a = area(user);
    let Some(l) = &a.cb.language else { return };
    let value = utf8(value);
    unsafe { *out = match { let mut out = None; guarded(&mut || out = (l.number_at)(value, offset(at)).map(|(start, end)| (engine(start), engine(end)))); out } { Some((start, end)) => PxuiMaybeRange { has: 1, reserved: [0; 7], start, end }, None => PxuiMaybeRange::default() }; }
}
extern "C" fn context_at(user: *mut c_void, value: Str, caret: i64) -> u32 {
    let a = area(user);
    let Some(l) = &a.cb.language else { return 0 };
    match { let mut out = CodeContext::Code; guarded(&mut || out = (l.context_at)(utf8(value), offset(caret))); out } { CodeContext::Code => 0, CodeContext::String => 1, CodeContext::Comment => 2 }
}
extern "C" fn rewrite(user: *mut c_void, value: Str, caret: i64, out: *mut PxuiRewrite) {
    let a = area(user);
    let Some(f) = a.cb.language.as_ref().and_then(|l| l.rewrite) else { return };
    let value = utf8(value);
    let (text, caret) = { let mut out = (value.to_owned(), caret); guarded(&mut || out = { let (text, at) = f(value, offset(caret)); (text, engine(at)) }); out };
    a.produced = text;
    unsafe { *out = PxuiRewrite { text: lend(&a.produced), caret }; }
}

/// A text input's response as its call reported it, `changed` as the call's text said.
pub(crate) fn input_response(out: &PxuiResponse, changed: bool) -> Response { Response { changed, ..crate::kit_ext::response(out) } }

impl Ui {
    /// The one body of `text_field`: the key's label, the body variant around the engine call.
    #[track_caller]
    pub(crate) fn text_field_key(&mut self, key: Key, value: &str, field: &TextField<'_>) -> TextFieldResponse<'_> {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let mut out = PxuiResponse::EMPTY;
        let variant = field.body_variant();
        let traits = field.traits();
        if variant != 0 { self.set_variant(pxui_types::theme::BodyKind::text_field, variant); }
        // static: a Rust key and value are UTF-8 by their types, so the trusted entry checks only their lengths
        #[cfg(feature = "static")]
        let changed = { let mut changed = None; check(self, unsafe { pxui_abi_link::trusted::text_field(self.0.as_ptr(), label, field.accessible_name(), value, traits, &mut changed, &mut out) }); changed };
        #[cfg(not(feature = "static"))]
        let changed = {
            let (mut changed, mut did_change) = (Str::default(), 0);
            let desc = PxuiTextFieldDesc {
                size: core::mem::size_of::<PxuiTextFieldDesc>() as u32, autocorrect: traits.autocorrects() as u8, capitalization: traits.capitalizes() as u8, action: traits.return_key() as u8, reserved: 0,
                key: as_text(label), name: as_str(field.accessible_name()), value: as_text(value), variant, reserved_variant: [0; 6],
            };
            check(self, unsafe { lib::pxui_ui_text_field(self.0.as_ptr(), &desc, &mut changed, &mut did_change, &mut out) });
            (did_change != 0).then_some(changed)
        };
        if variant != 0 { self.set_variant(pxui_types::theme::BodyKind::text_field, 0); }
        TextFieldResponse { response: input_response(&out, changed.is_some()), changed: changed.map(lent_str) }
    }

    /// The one body of `line_field`.
    #[track_caller]
    pub(crate) fn line_field_key(&mut self, key: Key, value: &str, rect: Rect, field: &LineField<'_>) -> LineFieldResponse<'_> {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let (mut text, mut step, mut out) = (Str::default(), 0, PxuiResponse::EMPTY);
        check(self, unsafe { lib::pxui_ui_line_field(self.0.as_ptr(), rect.x, rect.y, rect.w, rect.h, field.grab as u8, as_str(field.placeholder), as_str(label), as_text(value), as_str(field.name), &mut text, &mut step, &mut out) });
        let edited = lent_str(text);
        let changed = (edited != value).then_some(edited);
        LineFieldResponse { response: input_response(&out, changed.is_some()), changed, step: match step { 1 => LineStep::Next, 2 => LineStep::Previous, 3 => LineStep::Close, _ => LineStep::None } }
    }

    /// The one body of `text_area`.
    #[track_caller]
    pub(crate) fn text_area_node(&mut self, key: Key, value: &str, node: pxui_types::Node, o: &TextArea<'_>, cb: &AreaCallbacks<'_>) -> TextAreaResponse<'_> {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let lang = cb.language.as_ref();
        // the data crosses in fixed stack buffers (no heap per call): at most `AREA_DATA` of each kind, `AREA_DATA / 2` bracket pairs
        assert!(o.errors.len() <= AREA_DATA && o.notes.len() <= AREA_DATA && o.spans.len() <= AREA_DATA && o.splices.len() <= AREA_DATA && o.chips.len() <= AREA_DATA
            && lang.is_none_or(|l| l.pairs.len() <= AREA_DATA / 2), "pxui: a text area takes at most {AREA_DATA} errors, notes, spans, splices and chips and {} bracket pairs", AREA_DATA / 2);
        // only the slots the call's data fills are written (and only they cross): zeroing all of them, about 7 KiB, cost each frame more
        // than the rest of the call
        let mut errors = [const { MaybeUninit::<i64>::uninit() }; AREA_DATA];
        let mut messages = [const { MaybeUninit::<PxuiNote>::uninit() }; AREA_DATA];
        let mut spans = [const { MaybeUninit::<PxuiRange>::uninit() }; AREA_DATA];
        let mut splice = [const { MaybeUninit::<PxuiSplice>::uninit() }; AREA_DATA];
        let mut chips = [const { MaybeUninit::<PxuiColorSpan>::uninit() }; AREA_DATA];
        let mut pairs = [const { MaybeUninit::<u32>::uninit() }; AREA_DATA];
        for (slot, line) in errors.iter_mut().zip(o.errors) { slot.write(engine(*line)); }
        for (slot, n) in messages.iter_mut().zip(o.notes) { slot.write(PxuiNote { line: engine(n.line), start: n.range.map_or(0, |r| engine(r.0)), end: n.range.map_or(0, |r| engine(r.1)), has_range: n.range.is_some() as u8, reserved: [0; 7], text: as_str(n.text) }); }
        for (slot, (start, end)) in spans.iter_mut().zip(o.spans) { slot.write(PxuiRange { start: engine(*start), end: engine(*end) }); }
        for (slot, s) in splice.iter_mut().zip(o.splices) { slot.write(PxuiSplice { start: engine(s.start), end: engine(s.end), text: as_text(s.text) }); }
        for (slot, (start, end, color)) in chips.iter_mut().zip(o.chips) { slot.write(PxuiColorSpan { start: engine(*start), end: engine(*end), color: *color, reserved: 0 }); }
        let pair_count = lang.map_or(0, |l| l.pairs.len());
        if let Some(l) = lang { for (slot, (open, close)) in pairs.chunks_exact_mut(2).zip(l.pairs) { slot[0].write(*open as u32); slot[1].write(*close as u32); } }
        let has = |on: bool, flag: u32| if on { flag } else { 0 };
        let desc = PxuiTextAreaDesc {
            size: core::mem::size_of::<PxuiTextAreaDesc>() as u32,
            flags: has(o.wrap, PxuiTextAreaDesc::WRAP) | has(o.reveal.is_some(), PxuiTextAreaDesc::HAS_REVEAL) | has(o.select.is_some(), PxuiTextAreaDesc::HAS_SELECT)
                | has(lang.is_some(), PxuiTextAreaDesc::HAS_LANGUAGE) | has(lang.is_some_and(|l| l.rewrite.is_some()), PxuiTextAreaDesc::HAS_REWRITE),
            // ignored: `node` lays the area out
            x: 0.0, y: 0.0, width: 0.0, height: 0.0, reveal: o.reveal.map_or(0, |r| engine(r.at)), reveal_nonce: o.reveal.map_or(0, |r| r.request as i64),
            select: o.select.map_or(PxuiRange::default(), |(start, end)| PxuiRange { start: engine(start), end: engine(end) }), label: as_str(label), value: as_text(value),
            errors: Slice { ptr: errors.as_ptr().cast(), len: o.errors.len() }, messages: Slice { ptr: messages.as_ptr().cast(), len: o.notes.len() },
            spans: Slice { ptr: spans.as_ptr().cast(), len: o.spans.len() }, splice: Slice { ptr: splice.as_ptr().cast(), len: o.splices.len() }, chips: Slice { ptr: chips.as_ptr().cast(), len: o.chips.len() },
            comment: as_str(lang.map_or("", |l| l.comment)), pairs: Slice { ptr: pairs.as_ptr().cast(), len: pair_count * 2 },
        };
        let mut ctx = Area { cb, colors: Vec::new(), ranges: Vec::new(), completions: Vec::new(), completion_texts: Vec::new(), produced: String::new(), description: None };

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
        // linked statically, the library's trusted entry takes the descriptor as Rust values
        #[cfg(feature = "static")]
        {
            let (mut changed, mut submitted, mut response) = (None, false, PxuiResponse::EMPTY);
            check(self, unsafe { pxui_abi_link::trusted::text_area(self.0.as_ptr(), &desc, &node, &cbs, o.name, &mut changed, &mut submitted, &mut response) });
            return TextAreaResponse { response: input_response(&response, changed.is_some()), changed: changed.map(lent_str), submitted };
        }
        #[cfg(not(feature = "static"))]
        let (mut out, mut changed, mut submitted, mut response) = (Str::default(), 0, 0, PxuiResponse::EMPTY);
        #[cfg(not(feature = "static"))]
        check(self, unsafe { lib::pxui_ui_text_area(self.0.as_ptr(), &desc, &node, &cbs, as_str(o.name), &mut out, &mut changed, &mut submitted, &mut response) });
        #[cfg(not(feature = "static"))]
        TextAreaResponse { response: input_response(&response, changed != 0), changed: (changed != 0).then(|| lent_str(out)), submitted: submitted != 0 }
    }

    /// The one body of `value_field`.
    #[track_caller]
    pub(crate) fn value_field_key(&mut self, key: Key, value: &str, rect: Rect, o: &ValueField<'_>, cb: &ValueCallbacks<'_>) -> ValueFieldResponse<'_> {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let desc = value_desc(rect, o, label, value);
        let accept = |_: &str| true;
        // linked statically, the library's trusted entry calls the closures directly
        #[cfg(feature = "static")]
        {
            let (mut changed, mut editing, mut response) = (None, false, PxuiResponse::EMPTY);
            let (slide, scrub) = (cb.slide.map(|f| f as &dyn Fn(f32) -> String), cb.scrub.map(|f| f as &dyn Fn(&str, f32, bool) -> String));
            check(self, unsafe { pxui_abi_link::trusted::value_field(self.0.as_ptr(), &desc, cb.valid.unwrap_or(&accept), slide, scrub, o.name, &mut changed, &mut editing, &mut response) });
            return ValueFieldResponse { response: input_response(&response, changed.is_some()), changed: changed.map(lent_str), editing };
        }
        #[cfg(not(feature = "static"))]
        let mut field = Field { valid: cb.valid.unwrap_or(&accept), slide: cb.slide, scrub: cb.scrub, produced: String::new() };
        #[cfg(not(feature = "static"))]
        let (mut out, mut changed, mut editing, mut response) = (Str::default(), 0, 0, PxuiResponse::EMPTY);
        #[cfg(not(feature = "static"))]
        check(self, unsafe { lib::pxui_ui_value_field(self.0.as_ptr(), &desc, Some(valid_call), cb.slide.map(|_| slide_call as extern "C" fn(*mut c_void, f32, *mut Str)), cb.scrub.map(|_| scrub_call as extern "C" fn(*mut c_void, Str, f32, u8, *mut Str)), &mut field as *mut Field as *mut c_void, as_str(o.name), &mut out, &mut changed, &mut editing, &mut response) });
        #[cfg(not(feature = "static"))]
        ValueFieldResponse { response: input_response(&response, changed != 0), changed: (changed != 0).then(|| lent_str(out)), editing: editing != 0 }
    }

    /// Between builds, rewrites the value text and the soft-range line of the value field `field` to `value_text`, as its build lays them out without a lead; true when the patch shows it.
    /// False when a patch cannot show it: a `lead` in `o`, a value too wide beside its trail, a line that had no width when built, or `field` not being a built value field (or gone). Then build the field's view instead. `o` must be the style the field was built with.
    /// Allocates: never when the text fits what the field's nodes held; a longer text grows them.
    /// Panics: when the library refuses the call (a null `Ui`, or `value_text` over the limit), or a size is over `i64::MAX`.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let plain = ui.node("n", pxui::Node::DEFAULT); // not a value field
    ///     assert!(!ui.patch_field(&plain, &pxui::ValueField::DEFAULT, "1.0"));
    /// });
    /// ```
    #[track_caller]
    pub fn patch_field(&mut self, field: &Handle, o: &ValueField<'_>, value_text: &str) -> bool {
        let desc = value_desc(Rect::new(0.0, 0.0, 0.0, 0.0), o, "", value_text);
        let mut out = 0;
        let rc = unsafe { lib::pxui_ui_patch_field(self.0.as_ptr(), field, &desc, as_str(value_text), &mut out) };
        live(self, rc) && out != 0
    }

    pub(crate) fn load_edit_key(&mut self, key: Key, text: &str) -> TextEdit {
        let key = self.resolve_lookup_key(key);
        let (mut out, mut caret, mut anchor) = (Str::default(), 0, 0);
        check(self, unsafe { lib::pxui_ui_load_text_edit(self.0.as_ptr(), key, as_text(text), &mut out, &mut caret, &mut anchor) });
        TextEdit { text: lent_string(out), caret: caret as usize, anchor: anchor as usize }
    }

    pub(crate) fn remember_key(&mut self, key: Key, edit: &TextEdit, typing: bool) {
        let key = self.resolve_lookup_key(key);
        check(self, unsafe { lib::pxui_ui_remember(self.0.as_ptr(), key, as_text(&edit.text), edit.caret as u64, edit.anchor as u64, typing as u8) })
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
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.set_a11y(true);
        ui.register_text_field_body("probe", label_probe).expect("field body");
        let themed = crate::ThemeHandle::parse("text_field.body = probe").expect("theme");
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
        let input = |time: f64| FrameIn::new(320.0, 240.0).time(time);
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
        let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
        ui.set_a11y(true);
        let rect = Rect::new(0.0, 0.0, 100.0, 20.0);
        let (mut search, mut line, mut value, mut area) = (None, None, None, None);
        for n in 0..2 {
            let input = crate::FrameIn::new(320.0, 240.0).time(n as f64);
            ui.frame(&input, &mut |ui: &mut Ui| {
                search = Some(ui.search_field("s", "x", &SearchField::DEFAULT.name("Find")).response);
                line = Some(crate::ext::line_field(ui, "l", "x", rect, &LineField::DEFAULT).response);
                value = Some(crate::ext::value_field(ui, "v", "1", Rect::new(0.0, 30.0, 100.0, 20.0), &ValueField::DEFAULT.name("Scale"), &ValueCallbacks::DEFAULT).response);
                area = Some(ui.text_area("a", "x", pxui_types::Node::DEFAULT.at(0.0, 60.0).w_px(100.0).h_px(40.0), &TextArea::DEFAULT, &AreaCallbacks::DEFAULT).response);
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
        assert_eq!(valid_call(&mut field as *mut Field as *mut c_void, { let mut record = Str::default(); record.ptr = value.as_ptr(); record.len = value.len(); record }), 0);
        assert!(crate::composite::take_callback_panic().is_some());
    }
}
