//! Queries and switches on the `Ui` as a whole: font and row size, focus, carries, timing and the frame counters.
use crate::state::{PxuiUi, checked, engine, fail, finite};
use crate::ui::key;
use crate::{PXUI_ERR_ARGUMENT, Str};
use pxui_core::ui::{Cursor, UndoKind};

export!(pub unsafe fn pxui_ui_font_size(ui: *mut PxuiUi) -> i64 { unsafe { engine(ui) }.map_or(0, |e| e.font_size()) });
export!(pub unsafe fn pxui_ui_row_height(ui: *mut PxuiUi) -> i64 { unsafe { engine(ui) }.map_or(0, |e| e.row_height()) });
export!(pub unsafe fn pxui_ui_density(ui: *mut PxuiUi) -> i64 { unsafe { engine(ui) }.map_or(0, |e| e.density()) });
export!(pub unsafe fn pxui_ui_now(ui: *mut PxuiUi) -> f64 { unsafe { engine(ui) }.map_or(0.0, |e| e.now()) });
export!(pub unsafe fn pxui_ui_frame_index(ui: *mut PxuiUi) -> u64 { unsafe { engine(ui) }.map_or(0, |e| e.frame_index()) });
export!(pub unsafe fn pxui_ui_captured(ui: *mut PxuiUi) -> u8 { unsafe { engine(ui) }.is_some_and(|e| e.captured()) as u8 });
export!(pub unsafe fn pxui_ui_text_input_focused(ui: *mut PxuiUi) -> u8 { unsafe { engine(ui) }.is_some_and(|e| e.text_input_focused()) as u8 });
export!(pub unsafe fn pxui_ui_verifying(ui: *mut PxuiUi) -> u8 { unsafe { engine(ui) }.is_some_and(|e| e.verifying()) as u8 });
export!(pub unsafe fn pxui_ui_text_undo_bytes(ui: *mut PxuiUi) -> u64 { unsafe { engine(ui) }.map_or(0, |e| e.text_undo_bytes() as u64) });
export!(pub unsafe fn pxui_ui_inspector_width(ui: *mut PxuiUi) -> f32 { unsafe { engine(ui) }.map_or(0.0, |e| e.inspector_width()) });
export!(pub unsafe fn pxui_ui_inspector_label_x(ui: *mut PxuiUi) -> f32 { unsafe { engine(ui) }.map_or(0.0, |e| e.inspector_label_x()) });

// 0 for none, 1 for undo, 2 for redo.
export!(pub unsafe fn pxui_ui_passed_undo(ui: *mut PxuiUi) -> u32 {
    match unsafe { engine(ui) }.and_then(|e| e.passed_undo()) { None => 0, Some(UndoKind::Undo) => 1, Some(UndoKind::Redo) => 2 }
});

// Whether the key (a `pxui.h` key code) was pressed in this frame's events.
export!(pub unsafe fn pxui_ui_key_pressed(ui: *mut PxuiUi, code: u32, out: *mut u8) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(k) = key(code) else { return unsafe { fail(ui, "unknown key code") }; };
    if out.is_null() { return unsafe { fail(ui, "null key output") }; }
    unsafe { *out = e.key_pressed(k) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_key_down(ui: *mut PxuiUi, code: u32, out: *mut u8) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(k) = key(code) else { return unsafe { fail(ui, "unknown key code") }; };
    if out.is_null() { return unsafe { fail(ui, "null key output") }; }
    unsafe { *out = e.key_down(k) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_key_pressed_with(ui: *mut PxuiUi, code: u32, modifier: u32, out: *mut u8) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(k) = key(code) else { return unsafe { fail(ui, "unknown key code") }; };
    let Some(m) = key(modifier).filter(|m| matches!(m, input::Key::Shift | input::Key::Ctrl | input::Key::Alt | input::Key::Meta)) else { return unsafe { fail(ui, "expected Shift, Ctrl, Alt or Meta modifier") }; };
    if out.is_null() { return unsafe { fail(ui, "null key output") }; }
    unsafe { *out = e.key_pressed_with(k, m) as u8; }
    0
});

#[cfg(test)]
mod held_key_tests {
    use super::*;
    #[test]
    fn held_key_queries_validate_handle_key_and_output_without_changing_failed_output() {
        let ui=unsafe { crate::pxui_ui_create(&crate::PxuiUiDesc::default()) };
        assert!(!ui.is_null());
        let mut out=99u8;
        assert_eq!(unsafe { pxui_ui_key_down(std::ptr::null_mut(),b'a' as u32,&mut out) },PXUI_ERR_ARGUMENT);
        assert_eq!(out,99);
        assert_eq!(unsafe { pxui_ui_key_down(ui,0,&mut out) },PXUI_ERR_ARGUMENT);
        assert_eq!(out,99);
        assert_eq!(unsafe { pxui_ui_key_down(ui,b'a' as u32,std::ptr::null_mut()) },PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { pxui_ui_key_down(ui,b'a' as u32,&mut out) },0);
        assert_eq!(out,0);
        out=99;
        assert_eq!(unsafe { pxui_ui_key_pressed_with(std::ptr::null_mut(),b'k' as u32,pxui_types::KEY_SPECIAL+12,&mut out) },PXUI_ERR_ARGUMENT);
        assert_eq!(out,99);
        for (code,modifier) in [(0,pxui_types::KEY_SPECIAL+12),(b'k' as u32,0),(b'k' as u32,b'a' as u32)] {
            assert_eq!(unsafe { pxui_ui_key_pressed_with(ui,code,modifier,&mut out) },PXUI_ERR_ARGUMENT);
            assert_eq!(out,99);
        }
        assert_eq!(unsafe { pxui_ui_key_pressed_with(ui,b'k' as u32,pxui_types::KEY_SPECIAL+12,std::ptr::null_mut()) },PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { pxui_ui_key_pressed_with(ui,b'k' as u32,pxui_types::KEY_SPECIAL+12,&mut out) },0);
        assert_eq!(out,0);
        assert_eq!(unsafe { crate::pxui_ui_destroy(ui) },0);
    }
}

export!(pub unsafe fn pxui_ui_set_font_size(ui: *mut PxuiUi, size: i64) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if !(1..=1024).contains(&size) { return unsafe { fail(ui, "font size out of range") }; }
    e.set_font_size(size);
    0
});

export!(pub unsafe fn pxui_ui_unfocus(ui: *mut PxuiUi) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    e.unfocus();
    0
});

export!(pub unsafe fn pxui_ui_dismiss_popup(ui: *mut PxuiUi) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    e.dismiss_popup();
    0
});

// `shape`: 1 horizontal resize, 2 vertical resize, 3 text.
export!(pub unsafe fn pxui_ui_request_cursor(ui: *mut PxuiUi, shape: u32) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let shape = match shape { 1 => Cursor::HorizontalResize, 2 => Cursor::VerticalResize, 3 => Cursor::Text, _ => return unsafe { fail(ui, "unknown cursor shape") } };
    e.request_cursor(shape);
    0
});

export!(pub unsafe fn pxui_ui_repaint_after(ui: *mut PxuiUi, seconds: f64) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if !seconds.is_finite() { return unsafe { fail(ui, "repaint time is not finite") }; }
    e.repaint_after(seconds);
    0
});

// `verify_views(true)` for every `Ui` created from now on: a test harness turns it on once, for the UIs its code under test creates.
export!(bare pub unsafe fn pxui_verify_new_views(on: u8) -> i32 {
    if on > 1 { return PXUI_ERR_ARGUMENT; }
    pxui_core::ui::Ui::verify_new_views(on != 0);
    0
});

export!(pub unsafe fn pxui_ui_verify_views(ui: *mut PxuiUi, on: u8) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if on > 1 { return unsafe { fail(ui, "invalid flag") }; }
    e.verify_views(on != 0);
    0
});

export!(pub unsafe fn pxui_ui_set_timing(ui: *mut PxuiUi, on: u8) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if on > 1 { return unsafe { fail(ui, "invalid flag") }; }
    e.set_timing(on != 0);
    0
});

// The seven phase times of the last frame in nanoseconds; `len` must be 7.
export!(pub unsafe fn pxui_ui_phase_times(ui: *mut PxuiUi, out: *mut u64, len: usize) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() || len != pxui_core::ui::PHASES { return unsafe { fail(ui, "phase times need seven slots") }; }
    for (i, t) in e.phase_times().iter().enumerate() { unsafe { *out.add(i) = t.as_nanos() as u64; } }
    0
});

// The heap the `Ui` holds, as `[atlas, glyph_lists, text_undo]`; `len` must be 3.
export!(pub unsafe fn pxui_ui_bytes(ui: *mut PxuiUi, out: *mut i64, len: usize) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() || len != 3 { return unsafe { fail(ui, "bytes need three slots") }; }
    let b = e.bytes();
    unsafe { *out = b.atlas; *out.add(1) = b.glyph_lists; *out.add(2) = b.text_undo; }
    0
});

// 0 for other, 1 for Apple.
export!(pub unsafe fn pxui_ui_platform(ui: *mut PxuiUi) -> u32 {
    unsafe { engine(ui) }.map_or(0, |e| (e.platform() == input::Platform::Apple) as u32)
});

export!(pub unsafe fn pxui_ui_rebuilt_named(ui: *mut PxuiUi, prefix: Str, out: *mut u64) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(prefix) = (unsafe { checked(prefix) }) else { return unsafe { fail(ui, "invalid view prefix") }; };
    if out.is_null() { return unsafe { fail(ui, "null count output") }; }
    unsafe { *out = e.rebuilt_named(prefix) as u64; }
    0
});

// An inspector section's state by key: 0 unknown, 1 collapsed, 2 expanded.
export!(pub unsafe fn pxui_ui_expanded(ui: *mut PxuiUi, key: Str, out: *mut u8) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(key) = (unsafe { checked(key) }) else { return unsafe { fail(ui, "invalid section key") }; };
    if out.is_null() { return unsafe { fail(ui, "null section output") }; }
    unsafe { *out = match e.expanded(key) { None => 0, Some(false) => 1, Some(true) => 2 }; }
    0
});

export!(pub unsafe fn pxui_ui_hover_delay(ui: *mut PxuiUi, key: Str, out: *mut u8) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(key) = (unsafe { checked(key) }) else { return unsafe { fail(ui, "invalid hover key") }; };
    if out.is_null() { return unsafe { fail(ui, "null hover output") }; }
    unsafe { *out = e.hover_delay(key) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_tooltip(ui: *mut PxuiUi, key: Str, text: Str) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let (Some(key), Some(text)) = (unsafe { checked(key) }, unsafe { checked(text) }) else { return unsafe { fail(ui, "invalid tooltip text") }; };
    e.tooltip(key, text);
    0
});

export!(pub unsafe fn pxui_ui_cap_width(ui: *mut PxuiUi, text: Str, out: *mut f32) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(text) = (unsafe { checked(text) }) else { return unsafe { fail(ui, "invalid label") }; };
    if out.is_null() { return unsafe { fail(ui, "null width output") }; }
    unsafe { *out = e.cap_width(text); }
    0
});

export!(pub unsafe fn pxui_ui_text_top(ui: *mut PxuiUi, has_size: u8, size: i64, y: f32, h: f32) -> f32 {
    if has_size > 1 || !finite(&[y, h]) { return 0.0; }
    unsafe { engine(ui) }.map_or(0.0, |e| e.text_top((has_size != 0).then_some(size), y, h))
});
