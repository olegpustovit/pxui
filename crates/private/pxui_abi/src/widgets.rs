use crate::state::{PxuiUi, checked, fail, get, strings};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str, trusted};
use pxui_types::ffi::Slice;

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

export!(build pub unsafe fn pxui_ui_scrub(ui: *mut PxuiUi, label: Str, value: f32, min: f32, max: f32, step: f32, out: *mut pxui_types::ffi::PxuiScrubOut) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "invalid scrub output") }; };
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid scrub label") }; };
    if !value.is_finite() || !min.is_finite() || !max.is_finite() || min > max || !(max - min).is_finite() || !step.is_finite() || step <= 0.0 {
        return unsafe { fail(ui, "invalid scrub range, value or step") };
    }
    if !state.shared().in_frame { return crate::PXUI_ERR_STATE; }
    let result = unsafe { crate::state::engine(ui) }.unwrap().scrub(label, value, min..=max, step);
    *out = pxui_types::ffi::PxuiScrubOut {
        flags: u32::from(result.preview.is_some()) | (u32::from(result.commit.is_some()) << 1) | (u32::from(result.held) << 2),
        node: crate::state::to_abi_box(result.node), preview: result.preview.unwrap_or_default(), commit: result.commit.unwrap_or_default(),
        ..Default::default()
    };
    0
});

export!(trusted pub unsafe fn pxui_ui_checkbox(ui: *mut PxuiUi, label: Str, value: u8, out: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() || value > 1 { return unsafe { fail(ui, "invalid checkbox arguments") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid checkbox label") }; };
    let mut checked_out = false;
    let rc = unsafe { trusted::checkbox(ui, label, value != 0, &mut checked_out) };
    unsafe { *out = checked_out as u8; }
    rc
});

unsafe fn choice(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize, kind: u32) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null widget output") }; }
    let mut list = state.shared().take_strs();
    let Some(label) = (unsafe { checked(label) }).filter(|_| unsafe { strings(options, MAX_OPTIONS, &mut list) }) else {
        state.shared().put_strs(list);
        return unsafe { fail(ui, "invalid widget strings") };
    };
    let rc = unsafe { trusted::choice(ui, kind, label, &list, selected, &mut *out) };
    state.shared().put_strs(list);
    rc
}

export!(trusted pub unsafe fn pxui_ui_radio(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { choice(ui, label, options, selected, out, 0) } });
export!(trusted pub unsafe fn pxui_ui_tabs(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { choice(ui, label, options, selected, out, 1) } });
export!(trusted pub unsafe fn pxui_ui_segmented(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { choice(ui, label, options, selected, out, 2) } });
export!(trusted pub unsafe fn pxui_ui_dropdown(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { choice(ui, label, options, selected, out, 3) } });

export!(trusted pub unsafe fn pxui_ui_slider(ui: *mut PxuiUi, label: Str, value: f32, min: f32, max: f32, out: *mut f32) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "invalid slider arguments") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid slider label") }; };
    unsafe { trusted::slider(ui, label, value, min, max, &mut *out) }
});

export!(trusted pub unsafe fn pxui_ui_progress(ui: *mut PxuiUi, label: Str, has_value: u8, value: f32, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() || has_value > 1 { return unsafe { fail(ui, "invalid progress arguments") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid progress label") }; };
    unsafe { trusted::progress(ui, label, (has_value != 0).then_some(value), &mut *out) }
});

export!(trusted pub unsafe fn pxui_ui_spinner(ui: *mut PxuiUi, label: Str, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null spinner output") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid spinner label") }; };
    unsafe { trusted::spinner(ui, label, &mut *out) }
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

export!(trusted pub unsafe fn pxui_ui_badge(ui: *mut PxuiUi, label: Str, value: Str, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null badge output") }; }
    let (Some(label), Some(value)) = (unsafe { checked(label) }, unsafe { checked(value) }) else { return unsafe { fail(ui, "invalid badge text") }; };
    unsafe { trusted::badge(ui, label, value, &mut *out) }
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
