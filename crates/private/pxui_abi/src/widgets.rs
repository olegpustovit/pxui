use crate::state::{PxuiUi, checked, fail, get, strings};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str, trusted};
use pxui_types::ffi::Slice;

const MAX_OPTIONS: usize = 4096;

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
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if changed.is_null() || out.is_null() { return unsafe { fail(ui, "null search field output") }; }
    unsafe { *changed = 0; *out = Str::default(); }
    let (Some(label), Some(icon), Some(value), Some(placeholder)) = (
        unsafe { checked(label) }, unsafe { checked(icon) }, unsafe { checked(value) }, unsafe { checked(placeholder) },
    ) else { return unsafe { fail(ui, "invalid search field text") }; };
    let mut found = None;
    let rc = unsafe { trusted::search_field(ui, label, icon, value, placeholder, &mut found) };
    if let Some(value) = found { unsafe { *changed = 1; *out = state.give(value); } }
    rc
});
