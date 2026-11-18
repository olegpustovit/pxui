use crate::state::{MAX_TOTAL_TEXT_BYTES, PxuiUi, boxed, checked, checked_text, fail, finite, get};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str};
use core::{cell::Cell, ffi::c_void};
use pxui_types::ffi::{PxuiAreaCallbacks, PxuiDescription, PxuiMaybeRange, PxuiRewrite, PxuiTextAreaDesc, PxuiValueFieldDesc, Slice};
use std::rc::Rc;
use pxui_types::{Node, Size};
use pxui_core::ui::Drop;
use pxui_core::ui_text::{CodeContext, Completion, Language, ScrubPhase, TextAreaOpts, ValueFieldOpts};

pub type TextValid = Option<extern "C" fn(*mut c_void, Str) -> u8>;

/// `values` as a slice, or None when a null pointer or an over-long span makes it invalid.
unsafe fn span_of<'a, T>(values: pxui_types::ffi::Slice<T>, max: usize) -> Option<&'a [T]> {
    if values.len > max || (values.len != 0 && values.ptr.is_null()) { return None; }
    Some(if values.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(values.ptr, values.len) } })
}

/// The `TextAreaOpts` a text area descriptor and its callbacks describe, or None when a span or text is invalid. The closures
/// hold `user` and run only during the widget's call.
unsafe fn area_opts(d: &PxuiTextAreaDesc, cb: Option<&PxuiAreaCallbacks>, o: &mut TextAreaOpts, ad: &mut AreaAdapters) -> Option<()> {
    const MAX: usize = 1 << 16;
    let has = |flag: u32| d.flags & flag != 0;
    // byte offsets and lines are never negative
    let ok = |v: &[i64]| v.iter().all(|n| *n >= 0);
    let ranges_ok = |v: &[pxui_types::ffi::PxuiRange]| v.iter().all(|r| r.start >= 0 && r.end >= 0);
    if d.reveal < 0 || d.select.start < 0 || d.select.end < 0 { return None; }
    let (errors, spans, messages, splices, chips) = (unsafe { span_of(d.errors, MAX) }?, unsafe { span_of(d.spans, MAX) }?, unsafe { span_of(d.messages, MAX) }?, unsafe { span_of(d.splice, MAX) }?, unsafe { span_of(d.chips, MAX) }?);
    if !ok(errors) || !ranges_ok(spans) { return None; }
    if !messages.iter().all(|n| n.line >= 0 && n.start >= 0 && n.end >= 0) || !splices.iter().all(|n| n.start >= 0 && n.end >= 0) || !chips.iter().all(|n| n.start >= 0 && n.end >= 0) { return None; }
    // the data refills the scratch options in place: storage kept from earlier calls is reused (no allocation once warm)
    o.wrap = has(PxuiTextAreaDesc::WRAP);
    o.errors.clear(); o.errors.extend_from_slice(errors);
    // messages (labels) and splices (editor text) are copied: `MAX_TOTAL_TEXT_BYTES` of them together
    let total = messages.iter().map(|n| n.text.len).chain(splices.iter().map(|n| n.text.len)).fold(0usize, usize::saturating_add);
    if total > MAX_TOTAL_TEXT_BYTES { return None; }
    for (i, n) in messages.iter().enumerate() {
        let text = unsafe { checked(n.text) }?;
        let entry = (n.line, (n.has_range != 0).then_some((n.start, n.end)));
        if let Some(slot) = o.messages.get_mut(i) { (slot.0, slot.1) = entry; slot.2.clear(); slot.2.push_str(text); } else { o.messages.push((entry.0, entry.1, text.to_owned())); }
    }
    o.messages.truncate(messages.len());
    o.spans.clear(); o.spans.extend(spans.iter().map(|r| (r.start, r.end)));
    for (i, sp) in splices.iter().enumerate() {
        let text = unsafe { checked_text(sp.text) }?;
        if let Some(slot) = o.splice.get_mut(i) { (slot.0, slot.1) = (sp.start, sp.end); slot.2.clear(); slot.2.push_str(text); } else { o.splice.push((sp.start, sp.end, text.to_owned())); }
    }
    o.splice.truncate(splices.len());
    o.chips.clear(); o.chips.extend(chips.iter().map(|c| (c.start, c.end, c.color)));
    o.reveal = has(PxuiTextAreaDesc::HAS_REVEAL).then_some(d.reveal);
    o.reveal_nonce = d.reveal_nonce;
    o.select = has(PxuiTextAreaDesc::HAS_SELECT).then_some((d.select.start, d.select.end));
    // callbacks and the language are per call: closures that hold this call's `user`
    (o.on_context, o.on_scrub, o.on_scrub_edit, o.on_click, o.on_caret, o.on_caret_move, o.on_select, o.on_drop, o.language) = (None, None, None, None, None, None, None, None, None);
    let Some(cb) = cb else { return if has(PxuiTextAreaDesc::HAS_LANGUAGE) { None } else { Some(()) } };
    // the adapters are made once (`AreaAdapters`) and rebound to this call's functions: nothing is allocated per call
    ad.ctx.cb.set(*cb);
    if cb.on_context.is_some() { o.on_context = Some(Rc::clone(&ad.on_context)); }
    if cb.on_scrub.is_some() { o.on_scrub = Some(Rc::clone(&ad.on_scrub)); }
    if cb.on_scrub_edit.is_some() { o.on_scrub_edit = Some(Rc::clone(&ad.on_scrub_edit)); }
    if cb.on_click.is_some() { o.on_click = Some(Rc::clone(&ad.on_click)); }
    if cb.on_caret.is_some() { o.on_caret = Some(Rc::clone(&ad.on_caret)); }
    if cb.on_caret_move.is_some() { o.on_caret_move = Some(Rc::clone(&ad.on_caret_move)); }
    if cb.on_select.is_some() { o.on_select = Some(Rc::clone(&ad.on_select)); }
    if cb.on_drop.is_some() { o.on_drop = Some(Rc::clone(&ad.on_drop)); }
    if has(PxuiTextAreaDesc::HAS_LANGUAGE) {
        if cb.colorize.is_none() || cb.brackets.is_none() || cb.indent.is_none() || cb.complete.is_none() || cb.describe.is_none() || cb.number_at.is_none() || cb.context_at.is_none() { return None; }
        let comment = unsafe { checked(d.comment) }?;
        let pairs = unsafe { span_of(d.pairs, 64) }?;
        if pairs.len() % 2 != 0 || !pairs.iter().all(|c| char::from_u32(*c).is_some()) { return None; }
        if has(PxuiTextAreaDesc::HAS_REWRITE) && cb.rewrite.is_none() { return None; }
        let mut lang = ad.language.take()?;
        lang.pairs.clear();
        lang.pairs.extend(pairs.chunks_exact(2).filter_map(|p| Some((char::from_u32(p[0])?, char::from_u32(p[1])?))));
        lang.comment.clear(); lang.comment.push_str(comment);
        lang.rewrite = has(PxuiTextAreaDesc::HAS_REWRITE).then(|| Rc::clone(&ad.rewrite));
        o.language = Some(lang);
    }
    Some(())
}

/// What the callbacks of a text area read: the functions of the call in progress, set at its start (they run only during it).
pub(crate) struct AreaCtx { cb: Cell<PxuiAreaCallbacks> }

/// The text area's adapters to the caller's C functions, made once per `Ui` and rebound each call through `AreaCtx`: a warm call allocates
/// none. The `Language` lives here between calls (its lists refilled in place), and the options hold it only during one.
pub(crate) struct AreaAdapters {
    ctx: Rc<AreaCtx>,
    on_context: Rc<dyn Fn((f32, f32))>,
    on_scrub: Rc<dyn Fn(ScrubPhase)>,
    on_scrub_edit: Rc<dyn Fn((i64, i64), &str)>,
    on_click: Rc<dyn Fn(i64, bool)>,
    on_caret: Rc<dyn Fn(i64)>,
    on_caret_move: Rc<dyn Fn(i64)>,
    on_select: Rc<dyn Fn((i64, i64))>,
    on_drop: Rc<dyn Fn(i64, Drop)>,
    rewrite: Rc<dyn Fn(&str, i64) -> (String, i64)>,
    pub(crate) language: Option<Language>,
}

fn str_of(t: &str) -> Str { Str { ptr: t.as_ptr(), len: t.len() } }

pub(crate) fn area_adapters() -> AreaAdapters {
    let ctx = Rc::new(AreaCtx { cb: Cell::new(PxuiAreaCallbacks { size: core::mem::size_of::<PxuiAreaCallbacks>() as u32, reserved: 0, user: core::ptr::null_mut(), on_context: None, on_scrub: None, on_scrub_edit: None, on_click: None, on_caret: None, on_caret_move: None, on_select: None, on_drop: None, colorize: None, brackets: None, indent: None, complete: None, describe: None, number_at: None, context_at: None, rewrite: None }) });
    let c = |ctx: &Rc<AreaCtx>| Rc::clone(ctx);
    let (x1, x2, x3, x4, x5, x6, x7, x8, x9) = (c(&ctx), c(&ctx), c(&ctx), c(&ctx), c(&ctx), c(&ctx), c(&ctx), c(&ctx), c(&ctx));
    let language = Language {
        colorize: { let ctx = c(&ctx); Rc::new(move |t: &str| { let cb = ctx.cb.get(); let Some(f) = cb.colorize else { return Vec::new() }; let mut out = Slice::default(); f(cb.user, str_of(t), &mut out); unsafe { span_of(out, 1 << 20) }.map_or_else(Vec::new, |v| v.iter().map(|c| (c.start, c.end, c.color)).collect()) }) },
        brackets: { let ctx = c(&ctx); Rc::new(move |t: &str| { let cb = ctx.cb.get(); let Some(f) = cb.brackets else { return Vec::new() }; let mut out = Slice::default(); f(cb.user, str_of(t), &mut out); unsafe { span_of(out, 1 << 20) }.map_or_else(Vec::new, |v| v.iter().map(|r| (r.start, r.end)).collect()) }) },
        indent: { let ctx = c(&ctx); Rc::new(move |t: &str, caret| { let cb = ctx.cb.get(); let Some(f) = cb.indent else { return String::new() }; let mut out = Str::default(); f(cb.user, str_of(t), caret, &mut out); unsafe { checked_text(out) }.unwrap_or("").to_owned() }) },
        pairs: Vec::new(),
        complete: { let ctx = c(&ctx); Rc::new(move |t: &str, caret| {
            let cb = ctx.cb.get(); let Some(f) = cb.complete else { return Vec::new() };
            let own = |s: Str| unsafe { checked(s) }.map(str::to_owned);
            let mut out = Slice::default(); f(cb.user, str_of(t), caret, &mut out);
            unsafe { span_of(out, 4096) }.map_or_else(Vec::new, |v| v.iter().filter_map(|c| Some(Completion { replace: (usize::try_from(c.start).ok()?, usize::try_from(c.end).ok()?), insert: own(c.insert)?, label: own(c.label)?, detail: own(c.detail)?, doc: own(c.doc)? })).collect())
        }) },
        describe: { let ctx = c(&ctx); Rc::new(move |t: &str, at| {
            let cb = ctx.cb.get(); let f = cb.describe?;
            let mut out = PxuiDescription { has: 0, reserved: [0; 7], start: 0, end: 0, text: Str::default() };
            f(cb.user, str_of(t), at, &mut out);
            if out.has == 0 { None } else { Some((out.start, out.end, unsafe { checked(out.text) }?.to_owned())) }
        }) },
        number_at: { let ctx = c(&ctx); Rc::new(move |t: &str, at| { let cb = ctx.cb.get(); let f = cb.number_at?; let mut out = PxuiMaybeRange::default(); f(cb.user, str_of(t), at, &mut out); (out.has != 0).then_some((out.start, out.end)) }) },
        context_at: { let ctx = c(&ctx); Rc::new(move |t: &str, caret| { let cb = ctx.cb.get(); match cb.context_at.map_or(0, |f| f(cb.user, str_of(t), caret)) { 1 => CodeContext::String, 2 => CodeContext::Comment, _ => CodeContext::Code } }) },
        comment: String::new(),
        rewrite: None,
    };
    AreaAdapters {
        on_context: Rc::new(move |(x, y)| { let cb = x1.cb.get(); if let Some(f) = cb.on_context { f(cb.user, x, y) } }),
        on_scrub: Rc::new(move |phase| { let cb = x2.cb.get(); if let Some(f) = cb.on_scrub { f(cb.user, (phase == ScrubPhase::Done) as u32) } }),
        on_scrub_edit: Rc::new(move |(a, b), t: &str| { let cb = x3.cb.get(); if let Some(f) = cb.on_scrub_edit { f(cb.user, a, b, str_of(t)) } }),
        on_click: Rc::new(move |p, flag| { let cb = x4.cb.get(); if let Some(f) = cb.on_click { f(cb.user, p, flag as u8) } }),
        on_caret: Rc::new(move |p| { let cb = x5.cb.get(); if let Some(f) = cb.on_caret { f(cb.user, p) } }),
        on_caret_move: Rc::new(move |p| { let cb = x6.cb.get(); if let Some(f) = cb.on_caret_move { f(cb.user, p) } }),
        on_select: Rc::new(move |(a, b)| { let cb = x7.cb.get(); if let Some(f) = cb.on_select { f(cb.user, a, b) } }),
        on_drop: Rc::new(move |p, drop: Drop| { let cb = x8.cb.get(); if let Some(f) = cb.on_drop { let (dropped, payload) = match &drop { Drop::Hover(x) => (0, x), Drop::Dropped(x) => (1, x) }; f(cb.user, p, dropped, str_of(&payload.kind), str_of(&payload.value)) } }),
        rewrite: Rc::new(move |t: &str, caret| { let cb = x9.cb.get(); let Some(f) = cb.rewrite else { return (t.to_owned(), caret) }; let mut out = PxuiRewrite { text: Str::default(), caret }; f(cb.user, str_of(t), caret, &mut out); (unsafe { checked_text(out.text) }.unwrap_or(t).to_owned(), out.caret) }),
        language: Some(language),
        ctx,
    }
}

// A text area with its options. `cbs` (may be null) holds the app's closures: they run only during this call, with `cbs.user`.
export!(build pub unsafe fn pxui_ui_text_area_submit(
    ui: *mut PxuiUi, desc: *const PxuiTextAreaDesc, cbs: *const PxuiAreaCallbacks,
    out_text: *mut Str, out_changed: *mut u8, out_submitted: *mut u8,
) -> i32 {
    unsafe { text_area(ui, desc, None, cbs, out_text, out_changed, out_submitted) }
});

// ABI 1.80: `pxui_ui_text_area_submit` laid out by the checked `node` under the current parent (no grid) instead of the descriptor's
// x, y, width and height, which are ignored. The wrap width is the node's `Px` width, else the area's width from the last layout.
export!(build pub unsafe fn pxui_ui_text_area_node(
    ui: *mut PxuiUi, desc: *const PxuiTextAreaDesc, node: *const Node, cbs: *const PxuiAreaCallbacks,
    out_text: *mut Str, out_changed: *mut u8, out_submitted: *mut u8,
) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(node) = (unsafe { crate::node::checked_node(node) }) else { return unsafe { fail(ui, "invalid text area node") } };
    let bad_curve = matches!(&node.transition, pxui_types::node::OptTransition::Some(curve) if !curve.valid());
    if node.grid.is_some() || bad_curve { return unsafe { fail(ui, "invalid text area node") }; }
    unsafe { text_area(ui, desc, Some(*node), cbs, out_text, out_changed, out_submitted) }
});

/// The body of both text area exports: `place` (checked by the caller) or the descriptor's rectangle lays the area out.
unsafe fn text_area(
    ui: *mut PxuiUi, desc: *const PxuiTextAreaDesc, place: Option<Node>, cbs: *const PxuiAreaCallbacks,
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
    if d.flags & !PxuiTextAreaDesc::KNOWN != 0 || (place.is_none() && (!finite(&[d.x, d.y, d.width, d.height]) || d.width < 0.0 || d.height < 0.0)) {
        return unsafe { fail(ui, "invalid text area arguments") };
    }
    let place = place.unwrap_or(Node { at: Some((d.x, d.y)).into(), w: Size::Px(d.width), h: Size::Px(d.height), ..Node::DEFAULT });
    unsafe { *out_text = Str::default(); *out_changed = 0; *out_submitted = 0; }
    let (Some(label), Some(value)) = (unsafe { crate::state::checked_label(d.label) }, unsafe { checked_text(d.value) }) else {
        return unsafe { fail(ui, "invalid text area strings") };
    };
    let shared = state.shared();
    let (opts, ad) = (&mut shared.area, shared.area_ad.get_or_insert_with(area_adapters));
    if unsafe { area_opts(&d, cbs.as_ref(), opts, ad) }.is_none() { return unsafe { fail(ui, "invalid text area options") }; }
    let (text, submitted) = unsafe { (&mut *state.engine).text_area_submit(place, opts, label, value) };
    // the language goes back to its adapters for the next call
    if let Some(lang) = opts.language.take() { ad.language = Some(lang); }
    unsafe { *out_changed = text.is_some() as u8; *out_submitted = submitted as u8; }
    // an unchanged text is the caller's own value: nothing is copied into the library's storage
    let text = match text { Some(text) => match state.lend(text) { Ok(text) => text, Err(code) => return code }, None => d.value };
    unsafe { *out_text = text; }
    0
}

/// What the callbacks of a value field reach: the caller's `user` and functions of the call in progress (they run only during it).
pub(crate) struct FieldCtx { user: Cell<*mut c_void>, slide: Cell<SlideText>, scrub: Cell<ScrubText>, invalid: Cell<bool> }
pub(crate) type FieldCallbacks = (Rc<FieldCtx>, Rc<dyn Fn(f32) -> String>, Rc<dyn Fn(&str, f32, bool) -> String>);

/// Text a callback wrote, copied into the engine's `String` (refused, and the call fails, when it is not UTF-8 or too long).
fn produced(ctx: &FieldCtx, out: Str) -> String {
    match unsafe { checked(out) } { Some(text) => text.to_owned(), None => { ctx.invalid.set(true); String::new() } }
}

fn field_callbacks() -> FieldCallbacks {
    let ctx = Rc::new(FieldCtx { user: Cell::new(core::ptr::null_mut()), slide: Cell::new(None), scrub: Cell::new(None), invalid: Cell::new(false) });
    let slide = { let ctx = Rc::clone(&ctx); Rc::new(move |fraction: f32| {
        let Some(slide) = ctx.slide.get() else { return String::new() };
        let mut out = Str::default();
        slide(ctx.user.get(), fraction, &mut out);
        produced(&ctx, out)
    }) as Rc<dyn Fn(f32) -> String> };
    let scrub = { let ctx = Rc::clone(&ctx); Rc::new(move |origin: &str, dx: f32, coarse: bool| {
        let Some(scrub) = ctx.scrub.get() else { return String::new() };
        let mut out = Str::default();
        scrub(ctx.user.get(), Str { ptr: origin.as_ptr(), len: origin.len() }, dx, coarse as u8, &mut out);
        produced(&ctx, out)
    }) as Rc<dyn Fn(&str, f32, bool) -> String> };
    (ctx, slide, scrub)
}

pub type SlideText = Option<extern "C" fn(*mut c_void, f32, *mut Str)>;
pub type ScrubText = Option<extern "C" fn(*mut c_void, Str, f32, u8, *mut Str)>;

/// The options a value field descriptor names, without its callbacks.
fn field_opts(d: &PxuiValueFieldDesc, display: &str, placeholder: &str, lead: &str, trail: &str) -> ValueFieldOpts {
    let has = |flag: u32| d.flags & flag != 0;
    ValueFieldOpts {
        size: has(PxuiValueFieldDesc::HAS_SIZE).then_some(d.font_size),
        display: has(PxuiValueFieldDesc::HAS_DISPLAY).then(|| display.to_owned()),
        fraction: has(PxuiValueFieldDesc::HAS_FRACTION).then_some(d.fraction),
        slide: None, scrub: None,
        left: has(PxuiValueFieldDesc::LEFT), edit: has(PxuiValueFieldDesc::EDIT),
        lead: has(PxuiValueFieldDesc::HAS_LEAD).then(|| (lead.to_owned(), d.lead_color)),
        trail: has(PxuiValueFieldDesc::HAS_TRAIL).then(|| (trail.to_owned(), d.trail_color)),
        line: has(PxuiValueFieldDesc::HAS_LINE).then_some(d.line),
        bare: has(PxuiValueFieldDesc::BARE),
        placeholder: has(PxuiValueFieldDesc::HAS_PLACEHOLDER).then(|| placeholder.to_owned()),
        tracking: has(PxuiValueFieldDesc::HAS_TRACKING).then_some(d.tracking),
        kept_drag: has(PxuiValueFieldDesc::KEPT_DRAG),
    }
}

/// A value field descriptor read and validated: its strings, in the order display, placeholder, lead, trail, label, value.
unsafe fn field_desc(ui: *mut PxuiUi, desc: *const PxuiValueFieldDesc) -> Result<(PxuiValueFieldDesc, [&'static str; 6]), i32> {
    if desc.is_null() { return Err(unsafe { fail(ui, "invalid value field arguments") }); }
    let Some(d) = read_desc!(desc, PxuiValueFieldDesc) else { return Err(unsafe { fail(ui, "invalid value field descriptor") }); };
    if d.flags & !PxuiValueFieldDesc::KNOWN != 0
        || !finite(&[d.x, d.y, d.width, d.height, d.fraction, d.tracking]) || d.width < 0.0 || d.height < 0.0 || d.font_size < 0 {
        return Err(unsafe { fail(ui, "invalid value field descriptor") });
    }
    let (Some(label), Some(value), Some(display), Some(placeholder), Some(lead), Some(trail)) = (
        unsafe { crate::state::checked_label(d.label) }, unsafe { checked(d.value) }, unsafe { checked(d.display) },
        unsafe { checked(d.placeholder) }, unsafe { checked(d.lead) }, unsafe { checked(d.trail) },
    ) else { return Err(unsafe { fail(ui, "invalid value field strings") }); };
    Ok((d, [display, placeholder, lead, trail, label, value]))
}

// A value field with its options. `valid` says whether typed text may commit (null: all may); `slide` and `scrub` turn a
// pointer position into the field's text (they write the text, borrowed from the caller until its next callback). All three
// are called only during this call, with `user`.
export!(build pub unsafe fn pxui_ui_value_field(
    ui: *mut PxuiUi, desc: *const PxuiValueFieldDesc, valid: TextValid, slide: SlideText, scrub: ScrubText, user: *mut c_void,
    out_text: *mut Str, out_changed: *mut u8, out_editing: *mut u8,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out_text.is_null() || out_changed.is_null() || out_editing.is_null() {
        return unsafe { fail(ui, "invalid value field arguments") };
    }
    let (d, [display, placeholder, lead, trail, label, value]) = match unsafe { field_desc(ui, desc) } { Ok(v) => v, Err(code) => return code };
    unsafe { *out_text = Str::default(); *out_changed = 0; *out_editing = 0; }
    let (ctx, slide_cb, scrub_cb) = state.shared().field.get_or_insert_with(field_callbacks).clone();
    ctx.invalid.set(false);
    ctx.user.set(user);
    let slide_cb = slide.map(|slide| { ctx.slide.set(Some(slide)); slide_cb });
    let scrub_cb = scrub.map(|scrub| { ctx.scrub.set(Some(scrub)); scrub_cb });
    let mut opts = field_opts(&d, display, placeholder, lead, trail);
    opts.slide = slide_cb;
    opts.scrub = scrub_cb;
    let accept = |text: &str| {
        let Some(valid) = valid else { return true; };
        let result = valid(user, Str { ptr: text.as_ptr(), len: text.len() });
        if result > 1 { ctx.invalid.set(true); }
        result == 1
    };
    let (text, editing) = unsafe { (&mut *state.engine).value_field((d.x, d.y), d.width, d.height, &opts, &accept, label, value) };
    if ctx.invalid.get() { return unsafe { fail(ui, "invalid value field callback result") }; }
    unsafe { *out_changed = text.is_some() as u8; *out_editing = editing as u8; }
    // the field's text, changed or not, kept by the library until the next frame
    let Ok(text) = state.lend(text.unwrap_or(value)) else { return crate::PXUI_ERR_STATE };
    unsafe { *out_text = text; }
    0
});

// A value field's value (`text`) and soft-range line written by patches, as the field's build lays them out (the engine's
// `patch_field`): `out` says whether the patch could show it; false (a lead, a value too wide beside its trail, a line
// that had no width) means build the field's view. The descriptor is the one the field was built with.
export!(pub unsafe fn pxui_ui_patch_field(ui: *mut PxuiUi, box_: *const PxuiBox, desc: *const PxuiValueFieldDesc, text: Str, out: *mut u8) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    let (Some(text), false) = (unsafe { checked(text) }, out.is_null()) else { return unsafe { fail(ui, "invalid patch field arguments") }; };
    let (d, [display, placeholder, lead, trail, ..]) = match unsafe { field_desc(ui, desc) } { Ok(v) => v, Err(code) => return code };
    unsafe { *out = e.patch_field(&b, &field_opts(&d, display, placeholder, lead, trail), text) as u8; }
    0
});

// The edit the Ui keeps for the field under `key`: its text (borrowed until the next frame) and caret and anchor byte offsets.
export!(pub unsafe fn pxui_ui_load_text_edit(ui: *mut PxuiUi, key: i64, text: Str, out_text: *mut Str, caret: *mut u64, anchor: *mut u64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(text) = (unsafe { checked_text(text) }) else { return unsafe { fail(ui, "invalid edit text") }; };
    if out_text.is_null() || caret.is_null() || anchor.is_null() { return unsafe { fail(ui, "null edit output") }; }
    let edit = unsafe { (&mut *state.engine).load_text_edit(key, text) };
    let (edit_caret, edit_anchor) = (edit.caret as u64, edit.anchor as u64);
    let Ok(edited) = state.give(edit.text) else { return crate::PXUI_ERR_STATE };
    unsafe { *caret = edit_caret; *anchor = edit_anchor; *out_text = edited; }
    0
});

// ABI 1.77: `pxui_ui_remember` for the field keyed `key` (a resolved box key): the active editor's history when `key` owns it, the
// shelved history of that key otherwise (a key with no history is left alone); the active editor never changes.
export!(pub unsafe fn pxui_ui_remember_key(ui: *mut PxuiUi, key: i64, text: Str, caret: u64, anchor: u64, typing: u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(text) = (unsafe { checked_text(text) }) else { return unsafe { fail(ui, "invalid edit text") }; };
    let at = |offset: u64| usize::try_from(offset).ok().filter(|offset| text.is_char_boundary(*offset));
    let (Some(caret), Some(anchor)) = (at(caret), at(anchor)) else { return unsafe { fail(ui, "caret and anchor must sit on characters of the text") }; };
    if typing > 1 { return unsafe { fail(ui, "invalid typing flag") }; }
    unsafe { (&mut *state.engine).remember_key(key, &pxui_core::ui_text::TextEdit { text: text.to_owned(), caret, anchor }, typing != 0); }
    0
});

// Snapshots `text` with its caret and anchor for undo; `typing` joins consecutive typing into one step.
export!(pub unsafe fn pxui_ui_remember(ui: *mut PxuiUi, text: Str, caret: u64, anchor: u64, typing: u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(text) = (unsafe { checked_text(text) }) else { return unsafe { fail(ui, "invalid edit text") }; };
    let at = |offset: u64| usize::try_from(offset).ok().filter(|offset| text.is_char_boundary(*offset));
    let (Some(caret), Some(anchor)) = (at(caret), at(anchor)) else { return unsafe { fail(ui, "caret and anchor must sit on characters of the text") }; };
    if typing > 1 { return unsafe { fail(ui, "invalid typing flag") }; }
    unsafe { (&mut *state.engine).remember(&pxui_core::ui_text::TextEdit { text: text.to_owned(), caret, anchor }, typing != 0); }
    0
});
