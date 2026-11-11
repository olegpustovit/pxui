use crate::state::{PxuiUi, checked, fail, finite, get};
use crate::{PXUI_ERR_ARGUMENT, Str};
use core::{cell::Cell, ffi::c_void};
use pxui_types::ffi::{PxuiAreaCallbacks, PxuiDescription, PxuiMaybeRange, PxuiRewrite, PxuiTextAreaDesc, PxuiValueFieldDesc, Slice};
use std::rc::Rc;
use pxui_types::ui::Drop;
use pxui_core::ui_text::{CodeContext, Completion, Language, ScrubPhase, TextAreaOpts, ValueFieldOpts};

pub type TextValid = Option<extern "C" fn(*mut c_void, Str) -> u8>;

/// `values` as a slice, or None when a null pointer or an over-long span makes it invalid.
unsafe fn span_of<'a, T>(values: pxui_types::ffi::Slice<T>, max: usize) -> Option<&'a [T]> {
    if values.len > max || (values.len != 0 && values.ptr.is_null()) { return None; }
    Some(if values.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(values.ptr, values.len) } })
}

/// The `TextAreaOpts` a text area descriptor and its callbacks describe, or None when a span or text is invalid. The closures
/// hold `user` and run only during the widget's call.
unsafe fn area_opts(d: &PxuiTextAreaDesc, cb: Option<&PxuiAreaCallbacks>) -> Option<TextAreaOpts> {
    const MAX: usize = 1 << 16;
    let has = |flag: u32| d.flags & flag != 0;
    let own = |s: Str| unsafe { checked(s) }.map(str::to_owned);
    let mut o = TextAreaOpts { wrap: has(PxuiTextAreaDesc::WRAP), ..TextAreaOpts::default() };
    o.errors = unsafe { span_of(d.errors, MAX) }?.to_vec();
    for n in unsafe { span_of(d.messages, MAX) }? { o.messages.push((n.line, (n.has_range != 0).then_some((n.start, n.end)), own(n.text)?)); }
    o.spans = unsafe { span_of(d.spans, MAX) }?.iter().map(|r| (r.start, r.end)).collect();
    for sp in unsafe { span_of(d.splice, MAX) }? { o.splice.push((sp.start, sp.end, own(sp.text)?)); }
    o.chips = unsafe { span_of(d.chips, MAX) }?.iter().map(|c| (c.start, c.end, c.color)).collect();
    o.reveal = has(PxuiTextAreaDesc::HAS_REVEAL).then_some(d.reveal);
    o.reveal_nonce = d.reveal_nonce;
    o.select = has(PxuiTextAreaDesc::HAS_SELECT).then_some((d.select.start, d.select.end));
    let Some(cb) = cb else { return if has(PxuiTextAreaDesc::HAS_LANGUAGE) { None } else { Some(o) } };
    let user = cb.user as usize;
    let u = move || user as *mut c_void;
    let s = |t: &str| Str { ptr: t.as_ptr(), len: t.len() };
    if let Some(f) = cb.on_context { o.on_context = Some(Rc::new(move |(x, y)| f(u(), x, y))); }
    if let Some(f) = cb.on_scrub { o.on_scrub = Some(Rc::new(move |phase| f(u(), (phase == ScrubPhase::Done) as u32))); }
    if let Some(f) = cb.on_scrub_edit { o.on_scrub_edit = Some(Rc::new(move |(a, b), t: &str| f(u(), a, b, s(t)))); }
    if let Some(f) = cb.on_click { o.on_click = Some(Rc::new(move |p, flag| f(u(), p, flag as u8))); }
    if let Some(f) = cb.on_caret { o.on_caret = Some(Rc::new(move |p| f(u(), p))); }
    if let Some(f) = cb.on_caret_move { o.on_caret_move = Some(Rc::new(move |p| f(u(), p))); }
    if let Some(f) = cb.on_select { o.on_select = Some(Rc::new(move |(a, b)| f(u(), a, b))); }
    if let Some(f) = cb.on_drop {
        o.on_drop = Some(Rc::new(move |p, drop: Drop| { let (dropped, payload) = match &drop { Drop::Hover(x) => (0, x), Drop::Dropped(x) => (1, x) }; f(u(), p, dropped, s(&payload.kind), s(&payload.value)) }));
    }
    if has(PxuiTextAreaDesc::HAS_LANGUAGE) {
        let (Some(colorize), Some(brackets), Some(indent), Some(complete), Some(describe), Some(number_at), Some(context_at)) =
            (cb.colorize, cb.brackets, cb.indent, cb.complete, cb.describe, cb.number_at, cb.context_at) else { return None };
        let (comment, pairs) = (own(d.comment)?, unsafe { span_of(d.pairs, 64) }?);
        if pairs.len() % 2 != 0 { return None; }
        let pairs = pairs.chunks_exact(2).map(|p| Some((char::from_u32(p[0])?, char::from_u32(p[1])?))).collect::<Option<Vec<_>>>()?;
        o.language = Some(Language {
            colorize: Rc::new(move |t| { let mut out = Slice::default(); colorize(u(), s(t), &mut out); unsafe { span_of(out, 1 << 20) }.map_or_else(Vec::new, |v| v.iter().map(|c| (c.start, c.end, c.color)).collect()) }),
            brackets: Rc::new(move |t| { let mut out = Slice::default(); brackets(u(), s(t), &mut out); unsafe { span_of(out, 1 << 20) }.map_or_else(Vec::new, |v| v.iter().map(|r| (r.start, r.end)).collect()) }),
            indent: Rc::new(move |t, caret| { let mut out = Str::default(); indent(u(), s(t), caret, &mut out); unsafe { checked(out) }.unwrap_or("").to_owned() }),
            pairs,
            complete: Rc::new(move |t, caret| {
                let mut out = Slice::default(); complete(u(), s(t), caret, &mut out);
                unsafe { span_of(out, 4096) }.map_or_else(Vec::new, |v| v.iter().filter_map(|c| Some(Completion { replace: (c.start, c.end), insert: own(c.insert)?, label: own(c.label)?, detail: own(c.detail)?, doc: own(c.doc)? })).collect())
            }),
            describe: Rc::new(move |t, at| {
                let mut out = PxuiDescription { has: 0, reserved: [0; 7], start: 0, end: 0, text: Str::default() };
                describe(u(), s(t), at, &mut out);
                if out.has == 0 { None } else { Some((out.start, out.end, own(out.text)?)) }
            }),
            number_at: Rc::new(move |t, at| { let mut out = PxuiMaybeRange::default(); number_at(u(), s(t), at, &mut out); (out.has != 0).then_some((out.start, out.end)) }),
            context_at: Rc::new(move |t, caret| match context_at(u(), s(t), caret) { 1 => CodeContext::String, 2 => CodeContext::Comment, _ => CodeContext::Code }),
            comment,
            rewrite: if has(PxuiTextAreaDesc::HAS_REWRITE) {
                let f = cb.rewrite?;
                Some(Rc::new(move |t: &str, caret| { let mut out = PxuiRewrite { text: Str::default(), caret }; f(u(), s(t), caret, &mut out); (unsafe { checked(out.text) }.unwrap_or(t).to_owned(), out.caret) }))
            } else { None },
        });
    }
    Some(o)
}

// A text area with its options. `cbs` (may be null) holds the app's closures: they run only during this call, with `cbs.user`.
export!(pub unsafe fn pxui_ui_text_area_submit(
    ui: *mut PxuiUi, desc: *const PxuiTextAreaDesc, cbs: *const PxuiAreaCallbacks,
    out_text: *mut Str, out_changed: *mut u8, out_submitted: *mut u8,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() || out_text.is_null() || out_changed.is_null() || out_submitted.is_null() {
        return unsafe { fail(ui, "invalid text area arguments") };
    }
    // the sizes are read before either descriptor is copied
    if unsafe { (*desc).size } < core::mem::size_of::<PxuiTextAreaDesc>() as u32 || (!cbs.is_null() && unsafe { (*cbs).size } < core::mem::size_of::<PxuiAreaCallbacks>() as u32) {
        return unsafe { fail(ui, "invalid text area descriptor") };
    }
    let d = unsafe { *desc };
    if d.flags & !PxuiTextAreaDesc::KNOWN != 0 || !finite(&[d.x, d.y, d.width, d.height]) || d.width < 0.0 || d.height < 0.0 {
        return unsafe { fail(ui, "invalid text area arguments") };
    }
    unsafe { *out_text = Str::default(); *out_changed = 0; *out_submitted = 0; }
    let (Some(label), Some(value)) = (unsafe { checked(d.label) }, unsafe { checked(d.value) }) else {
        return unsafe { fail(ui, "invalid text area strings") };
    };
    let Some(opts) = (unsafe { area_opts(&d, cbs.as_ref()) }) else { return unsafe { fail(ui, "invalid text area options") }; };
    let (text, submitted) = unsafe { (&mut *state.engine).text_area_submit((d.x, d.y), d.width, d.height, &opts, label, value) };
    unsafe { *out_changed = (text != value) as u8; *out_submitted = submitted as u8; }
    unsafe { *out_text = state.give(text); }
    0
});

pub type SlideText = Option<extern "C" fn(*mut c_void, f32, *mut Str)>;
pub type ScrubText = Option<extern "C" fn(*mut c_void, Str, f32, u8, *mut Str)>;

// A value field with its options. `valid` says whether typed text may commit (null: all may); `slide` and `scrub` turn a
// pointer position into the field's text (they write the text, borrowed from the caller until its next callback). All three
// are called only during this call, with `user`.
export!(pub unsafe fn pxui_ui_value_field(
    ui: *mut PxuiUi, desc: *const PxuiValueFieldDesc, valid: TextValid, slide: SlideText, scrub: ScrubText, user: *mut c_void,
    out_text: *mut Str, out_changed: *mut u8, out_editing: *mut u8,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() || out_text.is_null() || out_changed.is_null() || out_editing.is_null() {
        return unsafe { fail(ui, "invalid value field arguments") };
    }
    let Some(d) = read_desc!(desc, PxuiValueFieldDesc) else { return unsafe { fail(ui, "invalid value field descriptor") }; };
    if d.flags & !PxuiValueFieldDesc::KNOWN != 0
        || !finite(&[d.x, d.y, d.width, d.height, d.fraction, d.tracking]) || d.width < 0.0 || d.height < 0.0 {
        return unsafe { fail(ui, "invalid value field descriptor") };
    }
    unsafe { *out_text = Str::default(); *out_changed = 0; *out_editing = 0; }
    let has = |flag: u32| d.flags & flag != 0;
    let (Some(label), Some(value), Some(display), Some(placeholder), Some(lead), Some(trail)) = (
        unsafe { checked(d.label) }, unsafe { checked(d.value) }, unsafe { checked(d.display) },
        unsafe { checked(d.placeholder) }, unsafe { checked(d.lead) }, unsafe { checked(d.trail) },
    ) else { return unsafe { fail(ui, "invalid value field strings") }; };
    let invalid_result = Rc::new(Cell::new(false));
    // text a callback wrote, copied into the engine's `String` (and refused when it is not UTF-8 or too long)
    let produced = {
        let invalid = Rc::clone(&invalid_result);
        move |out: Str| -> String { match unsafe { checked(out) } { Some(text) => text.to_owned(), None => { invalid.set(true); String::new() } } }
    };
    let slide: Option<Rc<dyn Fn(f32) -> String>> = slide.map(|slide| {
        let produced = produced.clone();
        Rc::new(move |fraction: f32| { let mut out = Str::default(); slide(user, fraction, &mut out); produced(out) }) as Rc<dyn Fn(f32) -> String>
    });
    let scrub: Option<Rc<dyn Fn(&str, f32, bool) -> String>> = scrub.map(|scrub| {
        let produced = produced.clone();
        Rc::new(move |origin: &str, dx: f32, coarse: bool| {
            let mut out = Str::default();
            scrub(user, Str { ptr: origin.as_ptr(), len: origin.len() }, dx, coarse as u8, &mut out);
            produced(out)
        }) as Rc<dyn Fn(&str, f32, bool) -> String>
    });
    let opts = ValueFieldOpts {
        size: has(PxuiValueFieldDesc::HAS_SIZE).then_some(d.font_size),
        display: has(PxuiValueFieldDesc::HAS_DISPLAY).then(|| display.to_owned()),
        fraction: has(PxuiValueFieldDesc::HAS_FRACTION).then_some(d.fraction),
        slide, scrub,
        left: has(PxuiValueFieldDesc::LEFT), edit: has(PxuiValueFieldDesc::EDIT),
        lead: has(PxuiValueFieldDesc::HAS_LEAD).then(|| (lead.to_owned(), d.lead_color)),
        trail: has(PxuiValueFieldDesc::HAS_TRAIL).then(|| (trail.to_owned(), d.trail_color)),
        line: has(PxuiValueFieldDesc::HAS_LINE).then_some(d.line),
        bare: has(PxuiValueFieldDesc::BARE),
        placeholder: has(PxuiValueFieldDesc::HAS_PLACEHOLDER).then(|| placeholder.to_owned()),
        tracking: has(PxuiValueFieldDesc::HAS_TRACKING).then_some(d.tracking),
        kept_drag: has(PxuiValueFieldDesc::KEPT_DRAG),
    };
    let accept = |text: &str| {
        let Some(valid) = valid else { return true; };
        let result = valid(user, Str { ptr: text.as_ptr(), len: text.len() });
        if result > 1 { invalid_result.set(true); }
        result == 1
    };
    let (text, editing) = unsafe { (&mut *state.engine).value_field((d.x, d.y), d.width, d.height, &opts, &accept, label, value) };
    if invalid_result.get() { return unsafe { fail(ui, "invalid value field callback result") }; }
    unsafe { *out_changed = (text != value) as u8; *out_editing = editing as u8; }
    unsafe { *out_text = state.give(text); }
    0
});

// The edit the Ui keeps for the field under `key`: its text (borrowed until the next frame) and caret and anchor byte offsets.
export!(pub unsafe fn pxui_ui_load_text_edit(ui: *mut PxuiUi, key: i64, text: Str, out_text: *mut Str, caret: *mut u64, anchor: *mut u64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(text) = (unsafe { checked(text) }) else { return unsafe { fail(ui, "invalid edit text") }; };
    if out_text.is_null() || caret.is_null() || anchor.is_null() { return unsafe { fail(ui, "null edit output") }; }
    let edit = unsafe { (&mut *state.engine).load_text_edit(key, text) };
    unsafe { *caret = edit.caret as u64; *anchor = edit.anchor as u64; *out_text = state.give(edit.text); }
    0
});

// Snapshots `text` with its caret and anchor for undo; `typing` joins consecutive typing into one step.
export!(pub unsafe fn pxui_ui_remember(ui: *mut PxuiUi, text: Str, caret: u64, anchor: u64, typing: u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(text) = (unsafe { checked(text) }) else { return unsafe { fail(ui, "invalid edit text") }; };
    let at = |offset: u64| usize::try_from(offset).ok().filter(|offset| text.is_char_boundary(*offset));
    let (Some(caret), Some(anchor)) = (at(caret), at(anchor)) else { return unsafe { fail(ui, "caret and anchor must sit on characters of the text") }; };
    if typing > 1 { return unsafe { fail(ui, "invalid typing flag") }; }
    unsafe { (&mut *state.engine).remember(&pxui_core::ui_text::TextEdit { text: text.to_owned(), caret, anchor }, typing != 0); }
    0
});
