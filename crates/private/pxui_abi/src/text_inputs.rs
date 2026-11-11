use crate::state::{PxuiUi, checked, fail, finite, get};
use crate::{PXUI_ERR_ARGUMENT, Str};
use core::{cell::Cell, ffi::c_void};
use pxui_types::ffi::PxuiValueFieldDesc;
use std::rc::Rc;
use pxui_core::ui_text::{TextAreaOpts, ValueFieldOpts};

pub type TextValid = Option<extern "C" fn(*mut c_void, Str) -> u8>;

export!(pub unsafe fn pxui_ui_text_area_submit(
    ui: *mut PxuiUi, at: *const f32, width: f32, height: f32, wrap: u8, label: Str, value: Str,
    out_text: *mut Str, out_changed: *mut u8, out_submitted: *mut u8,
) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if at.is_null() || out_text.is_null() || out_changed.is_null() || out_submitted.is_null() || wrap > 1 {
        return unsafe { fail(ui, "invalid text area arguments") };
    }
    let (x, y) = unsafe { (*at, *at.add(1)) };
    if !finite(&[x, y, width, height]) || width < 0.0 || height < 0.0 {
        return unsafe { fail(ui, "invalid text area arguments") };
    }
    unsafe { *out_text = Str::default(); *out_changed = 0; *out_submitted = 0; }
    let (Some(label), Some(value)) = (unsafe { checked(label) }, unsafe { checked(value) }) else {
        return unsafe { fail(ui, "invalid text area strings") };
    };
    let opts = TextAreaOpts { wrap: wrap != 0, ..TextAreaOpts::default() };
    let (text, submitted) = unsafe { (&mut *state.engine).text_area_submit((x, y), width, height, &opts, label, value) };
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
    let d = unsafe { *desc };
    if d.size < core::mem::size_of::<PxuiValueFieldDesc>() as u32 || d.flags & !PxuiValueFieldDesc::KNOWN != 0
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
