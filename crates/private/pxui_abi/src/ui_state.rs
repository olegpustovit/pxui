//! Queries and switches on the `Ui` as a whole: font and row size, focus, carries, timing and the frame counters.
use crate::state::{PxuiUi, checked, engine, fail, get};
use crate::ui::key;
use crate::{PXUI_ERR_ARGUMENT, Str};
use pxui_core::ui::{Cursor, UndoKind};

// The body size and kit row in points exactly as created or set, fractions kept.
export!(pub unsafe fn pxui_ui_font_points(ui: *mut PxuiUi, out: *mut f32) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null font size output") }; }
    unsafe { *out = e.font_size(); }
    0
});
export!(pub unsafe fn pxui_ui_row_points(ui: *mut PxuiUi, out: *mut f32) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null row height output") }; }
    unsafe { *out = e.row_height(); }
    0
});
export!(pub unsafe fn pxui_ui_density(ui: *mut PxuiUi) -> i64 { unsafe { engine(ui) }.map_or(0, |e| e.density()) });
export!(pub unsafe fn pxui_ui_environment(ui:*mut PxuiUi,out:*mut pxui_types::Environment)->i32 {
    let Some(e)=(unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    // an out record: the caller's `size` covers the library's, else it is refused; the library's own is written
    let Some(out)=sized_mut!(out) else { return unsafe { fail(ui,"null or short environment output") }; };
    *out=pxui_types::Environment { size: core::mem::size_of::<pxui_types::Environment>() as u32, ..e.environment() };
    0
});
export!(pub unsafe fn pxui_ui_keyboard(ui:*mut PxuiUi,out:*mut pxui_types::Keyboard)->i32 {
    let Some(e)=(unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(out)=sized_mut!(out) else { return unsafe { fail(ui,"null or short keyboard output") }; };
    *out=pxui_types::Keyboard { size: core::mem::size_of::<pxui_types::Keyboard>() as u32, ..e.keyboard() };
    0
});
export!(pub unsafe fn pxui_ui_text_input_options(ui:*mut PxuiUi,out:*mut pxui_types::PxuiTextInputOptions)->i32 {
    let Some(e)=(unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui,"null text input options output") }; }
    unsafe { out.write(e.text_input_options().into()); }
    0
});
export!(pub unsafe fn pxui_ui_content_rect(ui:*mut PxuiUi,out:*mut pxui_types::geom::Rect)->i32 {
    let Some(e)=(unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui,"null content rectangle output") }; }
    unsafe { out.write(e.content_rect()); }
    0
});
export!(pub unsafe fn pxui_ui_now(ui: *mut PxuiUi) -> f64 { unsafe { engine(ui) }.map_or(0.0, |e| e.now()) });
export!(pub unsafe fn pxui_ui_frame_index(ui: *mut PxuiUi) -> u64 { unsafe { engine(ui) }.map_or(0, |e| e.frame_index()) });
export!(pub unsafe fn pxui_ui_captured(ui: *mut PxuiUi) -> u8 { unsafe { engine(ui) }.is_some_and(|e| e.captured()) as u8 });
export!(pub unsafe fn pxui_ui_reordering(ui: *mut PxuiUi) -> u8 { unsafe { engine(ui) }.is_some_and(|e| e.reordering()) as u8 });
export!(pub unsafe fn pxui_ui_text_input_focused(ui: *mut PxuiUi) -> u8 { unsafe { engine(ui) }.is_some_and(|e| e.text_input_focused()) as u8 });
export!(pub unsafe fn pxui_ui_verifying(ui: *mut PxuiUi) -> u8 { unsafe { engine(ui) }.is_some_and(|e| e.verifying()) as u8 });
export!(pub unsafe fn pxui_ui_text_undo_bytes(ui: *mut PxuiUi) -> u64 { unsafe { engine(ui) }.map_or(0, |e| e.text_undo_bytes() as u64) });
export!(pub unsafe fn pxui_ui_inspector_width(ui: *mut PxuiUi) -> f32 { unsafe { engine(ui) }.map_or(0.0, |e| e.inspector_width()) });
export!(pub unsafe fn pxui_ui_inspector_label_x(ui: *mut PxuiUi) -> f32 { unsafe { engine(ui) }.map_or(0.0, |e| e.inspector_label_x()) });

// 0 for none, 1 for undo, 2 for redo.
export!(pub unsafe fn pxui_ui_passed_undo(ui: *mut PxuiUi) -> u32 {
    match unsafe { engine(ui) }.and_then(|e| e.passed_undo()) { None => 0, Some(UndoKind::Undo) => 1, Some(UndoKind::Redo) => 2 }
});

export!(pub unsafe fn pxui_ui_key_down(ui: *mut PxuiUi, code: u32, out: *mut u8) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(k) = key(code) else { return unsafe { fail(ui, "unknown key code") }; };
    if out.is_null() { return unsafe { fail(ui, "null key output") }; }
    unsafe { *out = e.key_down(k) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_key_pressed(ui: *mut PxuiUi, code: u32, modifiers: u32, out: *mut u8) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(k) = key(code) else { return unsafe { fail(ui, "unknown key code") }; };
    if modifiers > 16 { return unsafe { fail(ui, "invalid modifier mask") }; }
    if out.is_null() { return unsafe { fail(ui, "null key output") }; }
    unsafe { out.write(e.key_pressed_mods(k, modifiers) as u8); }
    0
});

#[cfg(test)]
mod held_key_tests {
    use super::*;
    #[test]
    fn held_key_queries_validate_handle_key_and_output_without_changing_failed_output() {
        let ui=unsafe { crate::ui::create_test_ui() };
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
        for (code, mask) in [(0, 8), (b'k' as u32, 17), (b'k' as u32, u32::MAX)] {
            assert_eq!(unsafe { pxui_ui_key_pressed(ui,code,mask,&mut out) },PXUI_ERR_ARGUMENT);
            assert_eq!(out,99);
        }
        assert_eq!(unsafe { pxui_ui_key_pressed(ui,b'k' as u32,9,std::ptr::null_mut()) },PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { pxui_ui_key_pressed(ui,b'k' as u32,9,&mut out) },0);
        assert_eq!(out,0);
        assert_eq!(unsafe { crate::pxui_ui_destroy(ui) },0);
    }
}

export!(pub unsafe fn pxui_ui_variant(ui: *mut PxuiUi, kind: u32, variant: u16) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if kind as usize >= pxui_types::theme::Bodies::KINDS.len() { return unsafe { fail(ui, "unknown body kind") }; }
    e.set_variant(kind, variant);
    0
});
// ABI 1.61: the word the next icon button call (`pxui_ui_icon_button`) hands its drawing, read in the drawing with
// `pxui_paint_icon_word`; that call takes it, so the one after reads 0 unless set again.
export!(pub unsafe fn pxui_ui_icon_word(ui: *mut PxuiUi, word: u32) -> i32 {
    let Some(ui) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    ui.shared().icon_word = word;
    0
});

// Sets the body size in points, fractions kept: finite, at least 0.5 (it rounds to a whole point of at least 1) and at most 1024.
export!(pub unsafe fn pxui_ui_set_font_points(ui: *mut PxuiUi, points: f32) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if !(points.is_finite() && (0.5..=1024.0).contains(&points)) { return unsafe { fail(ui, "font size out of range") }; }
    e.set_font_size(points);
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
    e.set_cpu_clock(if on != 0 { Some(thread_cpu_clock) } else { None });
    0
});

fn thread_cpu_clock() -> Option<u64> {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        mem_sys::profile::thread_cpu_time_ns().ok()
    }
    // shortcut: Apple-only CPU clocks, add native clocks when qualifying another host.
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        None
    }
}

// CPU nanoseconds, all settling passes; no output write if timing/clock is unavailable.
export!(pub unsafe fn pxui_ui_phase_cpu_times(ui: *mut PxuiUi, out: *mut u64, len: usize) -> i32 {
        let Some(e) = (unsafe { engine(ui) }) else {
            return PXUI_ERR_ARGUMENT;
        };
        if out.is_null() || len != pxui_core::ui::PHASES {
            return unsafe { fail(ui, "CPU phase times need seven slots") };
        }
        let Some(times) = e.phase_cpu_times() else {
            return crate::PXUI_ERR_STATE;
        };
        unsafe {
            core::ptr::copy_nonoverlapping(times.as_ptr(), out, times.len());
        }
        0
    });

#[test]
fn cpu_phase_output_is_atomic_for_invalid_or_unavailable_requests() {
    let ui = unsafe { crate::ui::create_test_ui() };
    let mut out = [99; 7];
    assert_eq!(
        unsafe { pxui_ui_phase_cpu_times(core::ptr::null_mut(), out.as_mut_ptr(), 7) },
        PXUI_ERR_ARGUMENT
    );
    assert_eq!(
        unsafe { pxui_ui_phase_cpu_times(ui, core::ptr::null_mut(), 7) },
        PXUI_ERR_ARGUMENT
    );
    for len in [0, 6, 8] {
        assert_eq!(
            unsafe { pxui_ui_phase_cpu_times(ui, out.as_mut_ptr(), len) },
            PXUI_ERR_ARGUMENT
        );
    }
    assert_eq!(
        unsafe { pxui_ui_phase_cpu_times(ui, out.as_mut_ptr(), 7) },
        crate::PXUI_ERR_STATE
    );
    assert_eq!(out, [99; 7]);
    unsafe {
        crate::pxui_ui_destroy(ui);
    }
}

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
    let Some(key) = (unsafe { crate::state::checked_key(key) }) else { return unsafe { fail(ui, "invalid section key") }; };
    if out.is_null() { return unsafe { fail(ui, "null section output") }; }
    unsafe { *out = match e.expanded(key) { None => 0, Some(false) => 1, Some(true) => 2 }; }
    0
});

// ABI 1.77: a `key` in the resolved form (a 0x01 byte and 16 lowercase hex digits, `Key::Resolved`) waits on that key itself,
// allocation-free and shared with tooltip owners; any other text keeps the 1.x wait compared as text.
export!(pub unsafe fn pxui_ui_hover_delay(ui: *mut PxuiUi, key: Str, out: *mut u8) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(key) = (unsafe { crate::state::checked_key(key) }) else { return unsafe { fail(ui, "invalid hover key") }; };
    if out.is_null() { return unsafe { fail(ui, "null hover output") }; }
    let resolved = key.len() == 17 && key.as_bytes()[0] == 1;
    unsafe { *out = if resolved { e.hover_delay_key(pxui_types::ui::key_of(0, key)) } else { e.hover_delay(key) } as u8; }
    0
});

// ABI 1.77: the seed an authored key resolves under for a lookup: the active scope's during a build, `ROOT_KEY` outside one, so
// `key_of(seed, label)` names what `pxui_ui_node` keyed by `label` in that scope.
export!(pub unsafe fn pxui_ui_key_seed(ui: *mut PxuiUi, out: *mut i64) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null seed output") }; }
    unsafe { *out = e.lookup_seed(); }
    0
});
