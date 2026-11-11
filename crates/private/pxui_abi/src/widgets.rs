use crate::state::{PxuiUi, checked, fail, get, strings, to_abi_box};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str};
use pxui_types::ffi::Slice;

const MAX_OPTIONS: usize = 4096;

export!(pub unsafe fn pxui_ui_checkbox(ui: *mut PxuiUi, label: Str, value: u8, out: *mut u8) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() || value > 1 { return unsafe { fail(ui, "invalid checkbox arguments") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid checkbox label") }; };
    unsafe { *out = (&mut *ui.engine).checkbox(label, value != 0) as u8; }
    0
});

unsafe fn choice(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize, kind: u32) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null widget output") }; }
    let mut list = state.shared().take_strs();
    let Some(label) = (unsafe { checked(label) }).filter(|_| unsafe { strings(options, MAX_OPTIONS, &mut list) }) else {
        state.shared().put_strs(list);
        return unsafe { fail(ui, "invalid widget strings") };
    };
    let engine = unsafe { &mut *state.engine };
    unsafe { *out = match kind { 0 => engine.radio(label, &list, selected), 1 => engine.tabs(label, &list, selected), 2 => engine.segmented(label, &list, selected), _ => engine.dropdown(label, &list, selected) }; }
    state.shared().put_strs(list);
    0
}

export!(pub unsafe fn pxui_ui_radio(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { choice(ui, label, options, selected, out, 0) } });
export!(pub unsafe fn pxui_ui_tabs(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { choice(ui, label, options, selected, out, 1) } });
export!(pub unsafe fn pxui_ui_segmented(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { choice(ui, label, options, selected, out, 2) } });
export!(pub unsafe fn pxui_ui_dropdown(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32 { unsafe { choice(ui, label, options, selected, out, 3) } });

export!(pub unsafe fn pxui_ui_slider(ui: *mut PxuiUi, label: Str, value: f32, min: f32, max: f32, out: *mut f32) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() || !value.is_finite() || !min.is_finite() || !max.is_finite() || min > max { return unsafe { fail(ui, "invalid slider arguments") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid slider label") }; };
    unsafe { *out = (&mut *ui.engine).slider(label, value, min..=max); }
    0
});

export!(pub unsafe fn pxui_ui_progress(ui: *mut PxuiUi, label: Str, has_value: u8, value: f32, out: *mut PxuiBox) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() || has_value > 1 || (has_value != 0 && !value.is_finite()) { return unsafe { fail(ui, "invalid progress arguments") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid progress label") }; };
    unsafe { *out = to_abi_box((&mut *ui.engine).progress(label, (has_value != 0).then_some(value))); }
    0
});

export!(pub unsafe fn pxui_ui_spinner(ui: *mut PxuiUi, label: Str, out: *mut PxuiBox) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null spinner output") }; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid spinner label") }; };
    unsafe { *out = to_abi_box((&mut *ui.engine).spinner(label)); }
    0
});

export!(pub unsafe fn pxui_ui_icon_button(ui: *mut PxuiUi, label: Str, icon: Str, out: *mut u8) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null icon button output") }; }
    let (Some(label), Some(icon)) = (unsafe { checked(label) }, unsafe { checked(icon) }) else { return unsafe { fail(ui, "invalid icon button text") }; };
    unsafe { *out = (&mut *ui.engine).icon_button(label, icon) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_badge(ui: *mut PxuiUi, label: Str, value: Str, out: *mut PxuiBox) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null badge output") }; }
    let (Some(label), Some(value)) = (unsafe { checked(label) }, unsafe { checked(value) }) else { return unsafe { fail(ui, "invalid badge text") }; };
    unsafe { *out = to_abi_box((&mut *ui.engine).badge(label, value)); }
    0
});

export!(pub unsafe fn pxui_ui_search_field(ui: *mut PxuiUi, label: Str, icon: Str, value: Str, placeholder: Str, changed: *mut u8, out: *mut Str) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if changed.is_null() || out.is_null() { return unsafe { fail(ui, "null search field output") }; }
    unsafe { *changed = 0; *out = Str::default(); }
    let (Some(label), Some(icon), Some(value), Some(placeholder)) = (
        unsafe { checked(label) }, unsafe { checked(icon) }, unsafe { checked(value) }, unsafe { checked(placeholder) },
    ) else { return unsafe { fail(ui, "invalid search field text") }; };
    let Some(value) = (unsafe { &mut *ui.engine }).search_field(label, icon, value, placeholder) else {
        unsafe { *changed = 0; *out = Str::default(); }
        return 0;
    };
    unsafe { *changed = 1; *out = ui.give(value); }
    0
});
