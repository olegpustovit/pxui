use crate::state::{Build, PxuiUi, checked, fail, get, strings};
use core::ffi::c_void;
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str, trusted};
use pxui_types::ffi::{PxuiKeyedDesc, PxuiResponse, Slice};
use pxui_types::ui::Slider;

const MAX_OPTIONS: usize = 4096;

#[cfg(test)]
mod scrub_tests {
    use super::*;
    use pxui_types::ffi::PxuiScrubOut;

    #[test]
    fn scrub_validates_before_building_or_writing_and_preserves_callback_ownership() {
        extern "C" fn build(user: *mut core::ffi::c_void, raw: *mut core::ffi::c_void) {
            let ui = raw.cast();
            let label = Str { ptr: b"Scrub".as_ptr(), len: 5 };
            let sentinel = || PxuiScrubOut { flags: 99, node: PxuiBox { box_key: 777, ..Default::default() }, preview: 3.0, commit: 4.0, ..Default::default() };
            let mut out = sentinel();
            let mut r = PxuiResponse::EMPTY;
            for (value, min, max, step) in [(f32::NAN,0.0,1.0,0.1),(0.5,f32::NEG_INFINITY,1.0,0.1),(0.5,0.0,f32::INFINITY,0.1),(0.5,1.0,0.0,0.1),(0.0,-f32::MAX,f32::MAX,0.1),(0.5,0.0,1.0,0.0),(0.5,0.0,1.0,-0.1),(0.5,0.0,1.0,f32::NAN),(0.5,0.0,1.0,f32::INFINITY)] {
                assert_eq!(unsafe { pxui_ui_scrub(ui,label,value,min,max,step,&mut r,&mut out) }, PXUI_ERR_ARGUMENT);
                assert_eq!((out.flags,out.node.box_key,out.preview,out.commit),(99,777,3.0,4.0));
            }
            let invalid = [255u8];
            assert_eq!(unsafe { pxui_ui_scrub(ui,Str {ptr:invalid.as_ptr(),len:1},0.5,0.0,1.0,0.1,&mut r,&mut out) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { pxui_ui_scrub(ui,Str {ptr:core::ptr::null(),len:1},0.5,0.0,1.0,0.1,&mut r,&mut out) }, PXUI_ERR_ARGUMENT);
            let mut short = 4u32;
            assert_eq!(unsafe { pxui_ui_scrub(ui,label,0.5,0.0,1.0,0.1,&mut r,(&mut short as *mut u32).cast()) }, PXUI_ERR_ARGUMENT);
            assert_eq!(short,4);
            assert_eq!(unsafe { pxui_ui_scrub(ui,label,0.5,0.0,1.0,0.1,&mut r,core::ptr::null_mut()) }, PXUI_ERR_ARGUMENT);
            let key = pxui_types::ui::key_of(pxui_types::ui::ROOT_KEY,"Scrub");
            assert!(unsafe { crate::state::engine(ui) }.unwrap().box_of_key(key).is_none(),"invalid calls build no node");
            let owner = unsafe { *user.cast::<*mut PxuiUi>() };
            assert_eq!(unsafe { pxui_ui_scrub(owner,label,0.5,0.0,1.0,0.1,&mut r,&mut out) }, crate::PXUI_ERR_STATE);
            assert_eq!(out.node.box_key,777);
            #[repr(C)]
            struct Extended { out: PxuiScrubOut, tail: [u8;16] }
            let mut forward = Extended { out: PxuiScrubOut {size:64,..sentinel()}, tail:[77;16] };
            assert_eq!(unsafe { pxui_ui_scrub(ui,label,0.5,0.0,1.0,0.1,&mut r,&mut forward.out) },0);
            assert_eq!(forward.tail,[77;16]);
            assert_eq!(forward.out.flags,0);
            assert_ne!(forward.out.node.box_key,777);
        }
        let mut ui = unsafe { crate::pxui_ui_create(&crate::PxuiUiDesc::default()) };
        let mut out = PxuiScrubOut {flags:99,..Default::default()};
        assert_eq!(unsafe { pxui_ui_scrub(ui,Str::default(),0.5,0.0,1.0,0.1,&mut { PxuiResponse::EMPTY },&mut out) },crate::PXUI_ERR_STATE);
        assert_eq!(out.flags,99);
        let mut frame = crate::FrameOut::default();
        assert_eq!(unsafe { crate::pxui_frame(ui,&crate::FrameIn {width:320.0,height:240.0,..Default::default()},Some(build),(&mut ui as *mut *mut PxuiUi).cast(),&mut frame) },0);
        assert_eq!(unsafe { crate::pxui_ui_destroy(ui) },0);
    }
}

#[cfg(test)]
mod keyed_tests {
    use super::*;

    /// The keyed calls validate before building: a short or reserved-bit `PxuiKeyedDesc`, an invalid checkbox record, a reversed slider
    /// range, a named view and a view without an output are refused; valid calls key by `key` (not the name) and report a response.
    #[test]
    fn keyed_calls_validate_and_key_by_key() {
        extern "C" fn build(user: *mut core::ffi::c_void, raw: *mut core::ffi::c_void) {
            let ui = raw.cast();
            let s = |t: &'static str| Str { ptr: t.as_ptr(), len: t.len() };
            let k = |key: &'static str, name: &'static str| PxuiKeyedDesc::new(key, name);
            let (mut out, mut b) = (PxuiResponse::EMPTY, PxuiBox::default());
            assert_eq!(unsafe { pxui_ui_progress(ui, &PxuiKeyedDesc { size: 8, ..k("c", "Check") }, 0, 0.0, &mut b) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { pxui_ui_progress(ui, &PxuiKeyedDesc { reserved: 1, ..k("c", "Check") }, 0, 0.0, &mut b) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { pxui_ui_progress(ui, core::ptr::null(), 0, 0.0, &mut b) }, PXUI_ERR_ARGUMENT);
            let d = pxui_types::ffi::PxuiCheckboxDesc { size: core::mem::size_of::<pxui_types::ffi::PxuiCheckboxDesc>() as u32, disabled: 0, on: 0, passive: 0, reserved: 0, box_size: 0.0, look: core::ptr::null(), key: k("cx", "").key, text: s("Check") };
            for bad in [pxui_types::ffi::PxuiCheckboxDesc { box_size: -1.0, ..d }, pxui_types::ffi::PxuiCheckboxDesc { box_size: f32::NAN, ..d }, pxui_types::ffi::PxuiCheckboxDesc { reserved: 1, ..d },
                pxui_types::ffi::PxuiCheckboxDesc { passive: 2, ..d }, pxui_types::ffi::PxuiCheckboxDesc { on: 2, ..d }, pxui_types::ffi::PxuiCheckboxDesc { size: 8, ..d }] {
                assert_eq!(unsafe { crate::kit_ext::pxui_ui_checkbox(ui, &bad, &mut out) }, PXUI_ERR_ARGUMENT);
            }
            assert_eq!(unsafe { crate::kit_ext::pxui_ui_checkbox(ui, &d, &mut out) }, 0);
            assert_eq!(out.node.box_key, pxui_types::ui::key_of(pxui_types::ui::ROOT_KEY, "cx"), "keyed by `key`, not the text");
            let mut value = pxui_types::ffi::PxuiScrubOut::default();
            assert_eq!(unsafe { pxui_ui_slider(ui, &k("v", "Level"), 0.5, 1.0, 0.0, &mut out, &mut value) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { pxui_ui_slider(ui, &k("v", "Level"), 2.0, 0.0, 1.0, &mut out, &mut value) }, 0);
            assert_eq!(value.preview, 1.0, "held in the range");
            assert_eq!(unsafe { pxui_ui_progress(ui, &k("p", "Export###x"), 0, 0.0, &mut b) }, 0);
            assert_eq!(b.box_key, pxui_types::ui::key_of(pxui_types::ui::ROOT_KEY, "p"), "the name is never parsed for a key");
            assert_eq!(unsafe { pxui_ui_spinner(ui, &k("s", "Loading"), &mut b) }, 0);
            assert_eq!(unsafe { pxui_ui_badge(ui, &k("b", "Unread"), s("3"), &mut b) }, 0);
            // 1.72: a view has no name, and its output is required; a valid one keys by `key` and runs its build
            assert_eq!(unsafe { crate::composite::pxui_ui_view(ui, &k("view", "Named"), 1, None, core::ptr::null_mut(), &mut b) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { crate::composite::pxui_ui_view(ui, &k("view", ""), 1, None, core::ptr::null_mut(), core::ptr::null_mut()) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { crate::composite::pxui_ui_view(ui, &k("view", ""), 1, None, core::ptr::null_mut(), &mut b) }, 0);
            assert_eq!(b.box_key, pxui_types::ui::key_of(pxui_types::ui::ROOT_KEY, "view"));
            let _ = user;
        }
        let mut ui = unsafe { crate::pxui_ui_create(&crate::PxuiUiDesc::default()) };
        let mut frame = crate::FrameOut::default();
        assert_eq!(unsafe { crate::pxui_frame(ui, &crate::FrameIn { width: 320.0, height: 240.0, ..Default::default() }, Some(build), (&mut ui as *mut *mut PxuiUi).cast(), &mut frame) }, 0);
        assert_eq!(unsafe { crate::pxui_ui_destroy(ui) }, 0);
    }
}

// ABI 1.76: `pxui_ui_scrub` that also reports the scrubber's complete `PxuiResponse` (its focus, hover, press and secondary press).
export!(build pub unsafe fn pxui_ui_scrub(ui: *mut PxuiUi, label: Str, value: f32, min: f32, max: f32, step: f32, response: *mut PxuiResponse, out: *mut pxui_types::ffi::PxuiScrubOut) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(response) = sized_mut!(response) else { return unsafe { fail(ui, "invalid scrub response") }; };
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "invalid scrub output") }; };
    let Some(label) = (unsafe { crate::state::checked_label(label) }) else { return unsafe { fail(ui, "invalid scrub label") }; };
    if !value.is_finite() || !min.is_finite() || !max.is_finite() || min > max || !(max - min).is_finite() || !step.is_finite() || step <= 0.0 {
        return unsafe { fail(ui, "invalid scrub range, value or step") };
    }
    if !state.shared().in_frame { return crate::PXUI_ERR_STATE; }
    let engine = unsafe { crate::state::engine(ui) }.unwrap();
    let result = engine.scrub(label, value, min..=max, step);
    let signal = engine.signal(&result.node);
    let r = pxui_core::ui::Response { clicked: signal.clicked, hovered: signal.hovered, held: result.held, focused: engine.focused(&result.node), activated: signal.clicked && !signal.released,
        secondary: signal.pressed && signal.button == Some(input::MouseButton::RightButton), handle: result.node };
    trusted::respond(&r, response);
    write_scrub(&result, out);
    0
});

/// A scrub's phases as the 1.x record carries them.
fn write_scrub(result: &pxui_core::ui::Scrub, out: &mut pxui_types::ffi::PxuiScrubOut) {
    *out = pxui_types::ffi::PxuiScrubOut {
        flags: u32::from(result.preview.is_some()) | (u32::from(result.commit.is_some()) << 1) | (u32::from(result.held) << 2),
        node: crate::state::to_abi_box(result.node), preview: result.preview.unwrap_or_default(), commit: result.commit.unwrap_or_default(),
        ..Default::default()
    };
}

/// A `PxuiKeyedDesc` read whole: `size` at least the record's, `reserved` zero, `key` an identifier (UTF-8, at most `MAX_KEY_BYTES`) and
/// `name` display text (UTF-8, at most `MAX_LABEL_BYTES`); `None` refuses the call.
pub(crate) unsafe fn read_keyed<'a>(desc: *const PxuiKeyedDesc) -> Option<(&'a str, &'a str)> {
    let d = sized!(desc)?;
    if d.reserved != 0 { return None; }
    Some((unsafe { crate::state::checked_key(d.key) }?, unsafe { checked(d.name) }?))
}

// ABI 1.76: the kit slider keyed by `keyed.key`, showing and named `keyed.name`, reporting into one `PxuiResponse` and a
// `PxuiScrubOut`: flags 1 a held preview, 2 a commit (a release, a key or an assistive SetValue, `preview` equal to it), 4 held;
// `preview` is the value shown (the held one, the committed one, or the caller's value held in the range).
export!(trusted pub unsafe fn pxui_ui_slider(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, value: f32, min: f32, max: f32, out: *mut PxuiResponse, result: *mut pxui_types::ffi::PxuiScrubOut) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(result) = sized_mut!(result) else { return unsafe { fail(ui, "null or short slider result") } };
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short slider response") } };
    let Some((key, text)) = (unsafe { read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { trusted::slider_result(ui, key, text, value, min, max, &Slider::DEFAULT, out, result) }
});

// ABI 1.85: the kit section index keyed by `keyed.key`, named `keyed.name`, one strip child per string of `letters` (each checked once,
// at most 4096), reporting into one `PxuiResponse` and a `PxuiSegmentedResult` (the letter chosen this frame, `has_chosen` 0 when nothing
// moved it; never a secondary).
export!(trusted pub unsafe fn pxui_ui_section_index_result(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, letters: Slice<Str>, out: *mut PxuiResponse, result: *mut pxui_types::ffi::PxuiSegmentedResult) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short section index response") } };
    let Some(result) = sized_mut!(result) else { return unsafe { fail(ui, "null or short section index result") } };
    let Some((key, name)) = (unsafe { read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    let mut list = state.shared().take_strs();
    if !unsafe { strings(letters, MAX_OPTIONS, &mut list) } { state.shared().put_strs(list); return unsafe { fail(ui, "invalid widget strings") }; }
    let mut at = None;
    let rc = unsafe { trusted::section_index_checked(ui, key, name, &list, out, &mut at) };
    state.shared().put_strs(list);
    if rc == 0 { (result.has_chosen, result.chosen, result.has_secondary, result.secondary, result.reserved) = (at.is_some() as u8, at.unwrap_or(0) as u64, 0, 0, [0; 2]); }
    rc
});

// ABI 1.85: a collapsible section keyed by `keyed.key`, its header showing and named `keyed.name`, built through the `inspector_section`
// body: `open` (0 or 1) is the caller's value, `build(user, ui)` builds the content while open. `out` is the header's response; a click
// (or Enter/Space while focused) is a toggle, and the caller keeps the value.
export!(build pub unsafe fn pxui_ui_disclosure(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, open: u8, build: Build, user: *mut c_void, out: *mut PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short disclosure response") } };
    if open > 1 { return unsafe { fail(ui, "invalid disclosure arguments") }; }
    let Some((key, text)) = (unsafe { read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { trusted::disclosure(ui, key, text, open != 0, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, build, user), out) }
});

// ABI 1.68: a progress row keyed by `keyed.key`, showing and named `keyed.name`.
export!(trusted pub unsafe fn pxui_ui_progress(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, has_value: u8, value: f32, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() || has_value > 1 { return unsafe { fail(ui, "invalid progress arguments") }; }
    let Some((key, text)) = (unsafe { read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { trusted::progress(ui, key, text, (has_value != 0).then_some(value), &mut *out) }
});

// ABI 1.68: a spinner keyed by `keyed.key`, named `keyed.name`.
export!(trusted pub unsafe fn pxui_ui_spinner(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null spinner output") }; }
    let Some((key, name)) = (unsafe { read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { trusted::spinner(ui, key, name, &mut *out) }
});

// ABI 1.68: a badge keyed by `keyed.key`, named `keyed.name`, `value` its text.
export!(trusted pub unsafe fn pxui_ui_badge(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, value: Str, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null badge output") }; }
    let Some((key, name)) = (unsafe { read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    let Some(value) = (unsafe { checked(value) }) else { return unsafe { fail(ui, "invalid badge text") }; };
    unsafe { trusted::badge(ui, key, name, value, &mut *out) }
});

export!(trusted pub unsafe fn pxui_ui_search_field(ui: *mut PxuiUi, label: Str, icon: Str, value: Str, placeholder: Str, changed: *mut u8, out: *mut Str) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if changed.is_null() || out.is_null() { return unsafe { fail(ui, "null search field output") }; }
    unsafe { *changed = 0; *out = Str::default(); }
    let (Some(label), Some(icon), Some(value), Some(placeholder)) = (
        unsafe { crate::state::checked_label(label) }, unsafe { checked(icon) }, unsafe { crate::state::checked_text(value) }, unsafe { checked(placeholder) },
    ) else { return unsafe { fail(ui, "invalid search field text") }; };
    let mut found = None;
    let rc = unsafe { trusted::search_field(ui, label, icon, value, placeholder, pxui_types::TextTraits::SEARCH, &mut found, None) };
    if let Some(value) = found { unsafe { *changed = 1; *out = value; } }
    rc
});

// ABI 1.82: `pxui_ui_search_field` that also names the field `name` to a screen reader and reports what happened to its box in `response`,
// the result a caller would otherwise read with `pxui_ui_input_box`, `pxui_ui_a11y_name_checked`, `pxui_ui_signal` and `pxui_ui_focused`.
export!(trusted pub unsafe fn pxui_ui_search_field_response(ui: *mut PxuiUi, label: Str, icon: Str, value: Str, placeholder: Str, name: Str, action: u8, changed: *mut u8, out: *mut Str, response: *mut PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if changed.is_null() || out.is_null() { return unsafe { fail(ui, "null search field output") }; }
    let Some(traits) = pxui_types::TextTraits::from_numbers(0, 0, u32::from(action)) else { return unsafe { fail(ui, "invalid search field return key") } };
    let Some(response) = sized_mut!(response) else { return unsafe { fail(ui, "null or short search field response") } };
    unsafe { *changed = 0; *out = Str::default(); }
    let (Some(label), Some(icon), Some(value), Some(placeholder), Some(name)) = (
        unsafe { crate::state::checked_label(label) }, unsafe { checked(icon) }, unsafe { crate::state::checked_text(value) }, unsafe { checked(placeholder) }, unsafe { checked(name) },
    ) else { return unsafe { fail(ui, "invalid search field text") }; };
    let mut found = None;
    let rc = unsafe { trusted::search_field(ui, label, icon, value, placeholder, traits, &mut found, Some((name, response))) };
    if let Some(value) = found { unsafe { *changed = 1; *out = value; } }
    rc
});
