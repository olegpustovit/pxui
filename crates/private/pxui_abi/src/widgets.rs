use crate::state::{PxuiUi, checked, fail, get, strings};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str, trusted};
use pxui_types::ffi::{PxuiKeyedDesc, PxuiResponse, Slice};
use pxui_types::ui::{shown, Checkbox, Slider};

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
            for (value, min, max, step) in [(f32::NAN,0.0,1.0,0.1),(0.5,f32::NEG_INFINITY,1.0,0.1),(0.5,0.0,f32::INFINITY,0.1),(0.5,1.0,0.0,0.1),(0.0,-f32::MAX,f32::MAX,0.1),(0.5,0.0,1.0,0.0),(0.5,0.0,1.0,-0.1),(0.5,0.0,1.0,f32::NAN),(0.5,0.0,1.0,f32::INFINITY)] {
                assert_eq!(unsafe { pxui_ui_scrub(ui,label,value,min,max,step,&mut out) }, PXUI_ERR_ARGUMENT);
                assert_eq!((out.flags,out.node.box_key,out.preview,out.commit),(99,777,3.0,4.0));
            }
            let invalid = [255u8];
            assert_eq!(unsafe { pxui_ui_scrub(ui,Str {ptr:invalid.as_ptr(),len:1},0.5,0.0,1.0,0.1,&mut out) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { pxui_ui_scrub(ui,Str {ptr:core::ptr::null(),len:1},0.5,0.0,1.0,0.1,&mut out) }, PXUI_ERR_ARGUMENT);
            let mut short = 4u32;
            assert_eq!(unsafe { pxui_ui_scrub(ui,label,0.5,0.0,1.0,0.1,(&mut short as *mut u32).cast()) }, PXUI_ERR_ARGUMENT);
            assert_eq!(short,4);
            assert_eq!(unsafe { pxui_ui_scrub(ui,label,0.5,0.0,1.0,0.1,core::ptr::null_mut()) }, PXUI_ERR_ARGUMENT);
            let key = pxui_types::ui::key_of(pxui_types::ui::ROOT_KEY,"Scrub");
            assert!(unsafe { crate::state::engine(ui) }.unwrap().box_of_key(key).is_none(),"invalid calls build no node");
            let owner = unsafe { *user.cast::<*mut PxuiUi>() };
            assert_eq!(unsafe { pxui_ui_scrub(owner,label,0.5,0.0,1.0,0.1,&mut out) }, crate::PXUI_ERR_STATE);
            assert_eq!(out.node.box_key,777);
            #[repr(C)]
            struct Extended { out: PxuiScrubOut, tail: [u8;16] }
            let mut forward = Extended { out: PxuiScrubOut {size:64,..sentinel()}, tail:[77;16] };
            assert_eq!(unsafe { pxui_ui_scrub(ui,label,0.5,0.0,1.0,0.1,&mut forward.out) },0);
            assert_eq!(forward.tail,[77;16]);
            assert_eq!(forward.out.flags,0);
            assert_ne!(forward.out.node.box_key,777);
        }
        let mut ui = unsafe { crate::pxui_ui_create(&crate::PxuiUiDesc::default()) };
        let mut out = PxuiScrubOut {flags:99,..Default::default()};
        assert_eq!(unsafe { pxui_ui_scrub(ui,Str::default(),0.5,0.0,1.0,0.1,&mut out) },crate::PXUI_ERR_STATE);
        assert_eq!(out.flags,99);
        let mut frame = crate::FrameOut::default();
        assert_eq!(unsafe { crate::pxui_frame(ui,&crate::FrameIn {width:320.0,height:240.0,..Default::default()},Some(build),(&mut ui as *mut *mut PxuiUi).cast(),&mut frame) },0);
        assert_eq!(unsafe { crate::pxui_ui_destroy(ui) },0);
    }
}

#[cfg(test)]
mod keyed_tests {
    use super::*;

    /// The 1.68 keyed calls validate before building: a short or reserved-bit `PxuiKeyedDesc`, a choice kind other than 0, 1 or 3, a
    /// checkbox value over 1, a reversed slider range, a named view and a view without an output are refused; valid calls key by `key` (not the name) and report a response.
    #[test]
    fn keyed_calls_validate_and_key_by_key() {
        extern "C" fn build(user: *mut core::ffi::c_void, raw: *mut core::ffi::c_void) {
            let ui = raw.cast();
            let s = |t: &'static str| Str { ptr: t.as_ptr(), len: t.len() };
            let k = |key: &'static str, name: &'static str| PxuiKeyedDesc::new(key, name);
            let options = [s("A"), s("B")];
            let span = Slice { ptr: options.as_ptr(), len: 2 };
            let (mut out, mut chosen, mut value, mut b) = (PxuiResponse::EMPTY, 9usize, 0.0f32, PxuiBox::default());
            assert_eq!(unsafe { pxui_ui_choice_keyed(ui, &k("k", "Name"), 2, span, 1, &mut out, &mut chosen) }, PXUI_ERR_ARGUMENT);
            assert_eq!(chosen, 9, "a refused call writes nothing");
            assert_eq!(unsafe { pxui_ui_checkbox_keyed(ui, &k("c", "Check"), 2, &mut out) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { pxui_ui_checkbox_keyed(ui, &PxuiKeyedDesc { size: 8, ..k("c", "Check") }, 1, &mut out) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { pxui_ui_checkbox_keyed(ui, &PxuiKeyedDesc { reserved: 1, ..k("c", "Check") }, 1, &mut out) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { pxui_ui_checkbox_keyed(ui, core::ptr::null(), 1, &mut out) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { pxui_ui_slider_keyed(ui, &k("v", "Level"), 0.5, 1.0, 0.0, &mut out, &mut value) }, PXUI_ERR_ARGUMENT);
            for kind in [0, 1, 3] {
                assert_eq!(unsafe { pxui_ui_choice_keyed(ui, &k(["r", "t", "", "d"][kind as usize], "Name"), kind, span, 1, &mut out, &mut chosen) }, 0);
                assert_eq!(chosen, 1);
            }
            assert_eq!(unsafe { pxui_ui_checkbox_keyed(ui, &k("c", "Check"), 1, &mut out) }, 0);
            assert_eq!(out.node.box_key, pxui_types::ui::key_of(pxui_types::ui::ROOT_KEY, "c"), "keyed by `key`, not the name");
            assert_eq!(unsafe { pxui_ui_slider_keyed(ui, &k("v", "Level"), 2.0, 0.0, 1.0, &mut out, &mut value) }, 0);
            assert_eq!(value, 1.0, "held in the range");
            assert_eq!(unsafe { pxui_ui_progress_keyed(ui, &k("p", "Export###x"), 0, 0.0, &mut b) }, 0);
            assert_eq!(b.box_key, pxui_types::ui::key_of(pxui_types::ui::ROOT_KEY, "p"), "the name is never parsed for a key");
            assert_eq!(unsafe { pxui_ui_spinner_keyed(ui, &k("s", "Loading"), &mut b) }, 0);
            assert_eq!(unsafe { pxui_ui_badge_keyed(ui, &k("b", "Unread"), s("3"), &mut b) }, 0);
            // 1.72: a view has no name, and its output is required; a valid one keys by `key` and runs its build
            assert_eq!(unsafe { crate::composite::pxui_ui_view_keyed(ui, &k("view", "Named"), 1, None, core::ptr::null_mut(), &mut b) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { crate::composite::pxui_ui_view_keyed(ui, &k("view", ""), 1, None, core::ptr::null_mut(), core::ptr::null_mut()) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { crate::composite::pxui_ui_view_keyed(ui, &k("view", ""), 1, None, core::ptr::null_mut(), &mut b) }, 0);
            assert_eq!(b.box_key, pxui_types::ui::key_of(pxui_types::ui::ROOT_KEY, "view"));
            let _ = user;
        }
        let mut ui = unsafe { crate::pxui_ui_create(&crate::PxuiUiDesc::default()) };
        let mut frame = crate::FrameOut::default();
        assert_eq!(unsafe { crate::pxui_frame(ui, &crate::FrameIn { width: 320.0, height: 240.0, ..Default::default() }, Some(build), (&mut ui as *mut *mut PxuiUi).cast(), &mut frame) }, 0);
        assert_eq!(unsafe { crate::pxui_ui_destroy(ui) }, 0);
    }
}

export!(build pub unsafe fn pxui_ui_scrub(ui: *mut PxuiUi, label: Str, value: f32, min: f32, max: f32, step: f32, out: *mut pxui_types::ffi::PxuiScrubOut) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "invalid scrub output") }; };
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid scrub label") }; };
    if !value.is_finite() || !min.is_finite() || !max.is_finite() || min > max || !(max - min).is_finite() || !step.is_finite() || step <= 0.0 {
        return unsafe { fail(ui, "invalid scrub range, value or step") };
    }
    if !state.shared().in_frame { return crate::PXUI_ERR_STATE; }
    let result = unsafe { crate::state::engine(ui) }.unwrap().scrub(label, value, min..=max, step);
    write_scrub(&result, out);
    0
});

// ABI 1.76: `pxui_ui_scrub` that also reports the scrubber's complete `PxuiResponse` (its focus, hover, press and secondary press).
export!(build pub unsafe fn pxui_ui_scrub_result(ui: *mut PxuiUi, label: Str, value: f32, min: f32, max: f32, step: f32, response: *mut PxuiResponse, out: *mut pxui_types::ffi::PxuiScrubOut) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(response) = sized_mut!(response) else { return unsafe { fail(ui, "invalid scrub response") }; };
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "invalid scrub output") }; };
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid scrub label") }; };
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

// Legacy (cut at 2.0): keyed by its label, which (up to a `##`) is also its text; `pxui_ui_checkbox_keyed` keys apart.
export!(trusted pub unsafe fn pxui_ui_checkbox(ui: *mut PxuiUi, label: Str, value: u8, out: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() || value > 1 { return unsafe { fail(ui, "invalid checkbox arguments") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid checkbox label") }; };
    let mut r = PxuiResponse::EMPTY;
    let rc = unsafe { trusted::checkbox(ui, label, shown(label), value != 0, &Checkbox::DEFAULT, &mut r) };
    unsafe { *out = (value != 0) as u8 ^ (r.flags & PxuiResponse::CLICKED != 0) as u8; }
    rc
});

unsafe fn choice(ui: *mut PxuiUi, key: &str, name: &str, options: Slice<Str>, selected: usize, kind: u32, out: &mut PxuiResponse, chosen: &mut usize) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let mut list = state.shared().take_strs();
    if !unsafe { strings(options, MAX_OPTIONS, &mut list) } { state.shared().put_strs(list); return unsafe { fail(ui, "invalid widget strings") }; }
    let rc = unsafe { trusted::choice(ui, kind, key, name, &list, selected, 0, out, chosen) };
    state.shared().put_strs(list);
    rc
}

/// The legacy choice exports: keyed by `label`, named by its text before any `##`.
unsafe fn legacy_choice(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize, kind: u32) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null widget output") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid widget strings") }; };
    let mut r = PxuiResponse::EMPTY;
    unsafe { choice(ui, label, shown(label), options, selected, kind, &mut r, &mut *out) }
}

// Legacy (cut at 2.0): keyed by their label; `pxui_ui_choice_keyed` keys apart.
export!(trusted pub unsafe fn pxui_ui_radio(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { legacy_choice(ui, label, options, selected, out, 0) } });
export!(trusted pub unsafe fn pxui_ui_tabs(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { legacy_choice(ui, label, options, selected, out, 1) } });
export!(trusted pub unsafe fn pxui_ui_segmented(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { legacy_choice(ui, label, options, selected, out, 2) } });
export!(trusted pub unsafe fn pxui_ui_dropdown(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { legacy_choice(ui, label, options, selected, out, 3) } });

// Legacy (cut at 2.0): keyed by its label; `pxui_ui_slider_keyed` keys apart.
export!(trusted pub unsafe fn pxui_ui_slider(ui: *mut PxuiUi, label: Str, value: f32, min: f32, max: f32, out: *mut f32) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "invalid slider arguments") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid slider label") }; };
    let mut r = PxuiResponse::EMPTY;
    unsafe { trusted::slider(ui, label, shown(label), value, min, max, &Slider::DEFAULT, &mut r, &mut *out) }
});

// ABI 1.68: the kit checkbox keyed by `keyed.key`, showing and named `keyed.name`, reporting into one `PxuiResponse`; a click flips `on`.
export!(trusted pub unsafe fn pxui_ui_checkbox_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, on: u8, out: *mut PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short checkbox response") } };
    let Some((key, text)) = (unsafe { read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    if on > 1 { return unsafe { fail(ui, "invalid checkbox value") }; }
    unsafe { trusted::checkbox(ui, key, text, on != 0, &Checkbox::DEFAULT, out) }
});

// ABI 1.68: a radio group (`kind` 0), tabs (1) or a dropdown (3) keyed by `keyed.key` and named `keyed.name`, reporting into one
// `PxuiResponse` and the choice.
export!(trusted pub unsafe fn pxui_ui_choice_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, kind: u32, options: Slice<Str>, selected: usize, out: *mut PxuiResponse, chosen: *mut usize) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if chosen.is_null() { return unsafe { fail(ui, "null choice output") }; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short choice response") } };
    let Some((key, name)) = (unsafe { read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    if !matches!(kind, 0 | 1 | 3) { return unsafe { fail(ui, "invalid choice kind") }; }
    unsafe { choice(ui, key, name, options, selected, kind, out, &mut *chosen) }
});

// ABI 1.68: the kit slider keyed by `keyed.key`, showing and named `keyed.name`, reporting into one `PxuiResponse` and the value.
export!(trusted pub unsafe fn pxui_ui_slider_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, value: f32, min: f32, max: f32, out: *mut PxuiResponse, value_out: *mut f32) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if value_out.is_null() { return unsafe { fail(ui, "null slider output") }; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short slider response") } };
    let Some((key, text)) = (unsafe { read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { trusted::slider(ui, key, text, value, min, max, &Slider::DEFAULT, out, &mut *value_out) }
});

// ABI 1.76: the kit slider keyed by `keyed.key`, showing and named `keyed.name`, reporting into one `PxuiResponse` and a
// `PxuiScrubOut`: flags 1 a held preview, 2 a commit (a release, a key or an assistive SetValue, `preview` equal to it), 4 held;
// `preview` is the value shown (the held one, the committed one, or the caller's value held in the range).
export!(trusted pub unsafe fn pxui_ui_slider_result(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, value: f32, min: f32, max: f32, out: *mut PxuiResponse, result: *mut pxui_types::ffi::PxuiScrubOut) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(result) = sized_mut!(result) else { return unsafe { fail(ui, "null or short slider result") } };
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short slider response") } };
    let Some((key, text)) = (unsafe { read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { trusted::slider_result(ui, key, text, value, min, max, &Slider::DEFAULT, out, result) }
});

// Legacy (cut at 2.0): keyed by its label, shown up to a `##`; `pxui_ui_progress_keyed` keys apart.
export!(trusted pub unsafe fn pxui_ui_progress(ui: *mut PxuiUi, label: Str, has_value: u8, value: f32, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() || has_value > 1 { return unsafe { fail(ui, "invalid progress arguments") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid progress label") }; };
    unsafe { trusted::progress(ui, label, shown(label), (has_value != 0).then_some(value), &mut *out) }
});

// ABI 1.68: a progress row keyed by `keyed.key`, showing and named `keyed.name`.
export!(trusted pub unsafe fn pxui_ui_progress_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, has_value: u8, value: f32, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() || has_value > 1 { return unsafe { fail(ui, "invalid progress arguments") }; }
    let Some((key, text)) = (unsafe { read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { trusted::progress(ui, key, text, (has_value != 0).then_some(value), &mut *out) }
});

// Legacy (cut at 2.0): keyed by its label, named up to a `##`; `pxui_ui_spinner_keyed` keys apart.
export!(trusted pub unsafe fn pxui_ui_spinner(ui: *mut PxuiUi, label: Str, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null spinner output") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid spinner label") }; };
    unsafe { trusted::spinner(ui, label, shown(label), &mut *out) }
});

// ABI 1.68: a spinner keyed by `keyed.key`, named `keyed.name`.
export!(trusted pub unsafe fn pxui_ui_spinner_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null spinner output") }; }
    let Some((key, name)) = (unsafe { read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { trusted::spinner(ui, key, name, &mut *out) }
});

export!(trusted pub unsafe fn pxui_ui_icon_button(ui: *mut PxuiUi, label: Str, icon: Str, out: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null icon button output") }; }
    let (Some(label), Some(icon)) = (unsafe { checked(label) }, unsafe { checked(icon) }) else { return unsafe { fail(ui, "invalid icon button text") }; };
    let mut pressed = false;
    let rc = unsafe { trusted::icon_button(ui, label, icon, &mut pressed) };
    unsafe { *out = pressed as u8; }
    rc
});

// Legacy (cut at 2.0): keyed by its label, named up to a `##`; `pxui_ui_badge_keyed` keys apart.
export!(trusted pub unsafe fn pxui_ui_badge(ui: *mut PxuiUi, label: Str, value: Str, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null badge output") }; }
    let (Some(label), Some(value)) = (unsafe { checked(label) }, unsafe { checked(value) }) else { return unsafe { fail(ui, "invalid badge text") }; };
    unsafe { trusted::badge(ui, label, shown(label), value, &mut *out) }
});

// ABI 1.68: a badge keyed by `keyed.key`, named `keyed.name`, `value` its text.
export!(trusted pub unsafe fn pxui_ui_badge_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, value: Str, out: *mut PxuiBox) -> i32 {
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
        unsafe { checked(label) }, unsafe { checked(icon) }, unsafe { crate::state::checked_text(value) }, unsafe { checked(placeholder) },
    ) else { return unsafe { fail(ui, "invalid search field text") }; };
    let mut found = None;
    let rc = unsafe { trusted::search_field(ui, label, icon, value, placeholder, &mut found) };
    if let Some(value) = found { unsafe { *changed = 1; *out = value; } }
    rc
});
