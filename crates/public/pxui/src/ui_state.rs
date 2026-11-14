//! Queries and switches on the `Ui` as a whole, with the engine's names and shapes. A failed call is a
//! caller bug the engine would panic on (a stale handle, a non-finite number, oversized text): it panics
//! here with the library's message, inside the frame callback or at the app's call.
use crate::{as_str, check, Ui};
use pxui_types::ui::{text_top_for, Cursor, UndoKind};

library!(lib {
    fn pxui_ui_font_size(ui: *mut PxuiUi) -> i64;
    fn pxui_ui_row_height(ui: *mut PxuiUi) -> i64;
    fn pxui_ui_now(ui: *mut PxuiUi) -> f64;
    fn pxui_ui_frame_index(ui: *mut PxuiUi) -> u64;
    fn pxui_ui_captured(ui: *mut PxuiUi) -> u8;
    fn pxui_ui_text_input_focused(ui: *mut PxuiUi) -> u8;
    fn pxui_ui_verifying(ui: *mut PxuiUi) -> u8;
    fn pxui_ui_text_undo_bytes(ui: *mut PxuiUi) -> u64;
    fn pxui_ui_inspector_width(ui: *mut PxuiUi) -> f32;
    fn pxui_ui_inspector_label_x(ui: *mut PxuiUi) -> f32;
    fn pxui_ui_passed_undo(ui: *mut PxuiUi) -> u32;
    fn pxui_ui_key_pressed(ui: *mut PxuiUi, code: u32, out: *mut u8) -> i32;
    fn pxui_ui_key_down(ui: *mut PxuiUi, code: u32, out: *mut u8) -> i32;
    fn pxui_ui_set_font_size(ui: *mut PxuiUi, size: i64) -> i32;
    fn pxui_ui_unfocus(ui: *mut PxuiUi) -> i32;
    fn pxui_ui_dismiss_popup(ui: *mut PxuiUi) -> i32;
    fn pxui_ui_request_cursor(ui: *mut PxuiUi, shape: u32) -> i32;
    fn pxui_ui_repaint_after(ui: *mut PxuiUi, seconds: f64) -> i32;
    fn pxui_verify_new_views(on: u8) -> i32;
    fn pxui_ui_verify_views(ui: *mut PxuiUi, on: u8) -> i32;
    fn pxui_ui_set_timing(ui: *mut PxuiUi, on: u8) -> i32;
    fn pxui_ui_phase_times(ui: *mut PxuiUi, out: *mut u64, len: usize) -> i32;
    fn pxui_ui_bytes(ui: *mut PxuiUi, out: *mut i64, len: usize) -> i32;
    fn pxui_ui_platform(ui: *mut PxuiUi) -> u32;
    fn pxui_ui_rebuilt_named(ui: *mut PxuiUi, prefix: pxui_types::Str, out: *mut u64) -> i32;
    fn pxui_ui_expanded(ui: *mut PxuiUi, key: pxui_types::Str, out: *mut u8) -> i32;
    fn pxui_ui_hover_delay(ui: *mut PxuiUi, key: pxui_types::Str, out: *mut u8) -> i32;
    fn pxui_ui_tooltip(ui: *mut PxuiUi, key: pxui_types::Str, text: pxui_types::Str) -> i32;
    fn pxui_ui_cap_width(ui: *mut PxuiUi, text: pxui_types::Str, out: *mut f32) -> i32;
});

/// The platform whose editing conventions the `Ui` follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Platform { Apple, #[default] Other }

/// The heap a `Ui` holds, for the host to account for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bytes { pub atlas: i64, pub glyph_lists: i64, pub text_undo: i64 }

impl Ui {
    /// `verify_views(true)` for every `Ui` created from now on: a test harness turns it on once.
    #[track_caller] pub fn verify_new_views(on: bool) { assert_eq!(unsafe { lib::pxui_verify_new_views(on as u8) }, 0, "pxui: verify_new_views refused") }
    /// The last frame asked for the clipboard's text: send it with the next frame as a paste event.
    #[inline] pub fn paste_requested(&self) -> bool { self.output().platform.paste != 0 }
    /// The text the last frame wants on the clipboard, borrowed until the next frame.
    pub fn clipboard_request(&self) -> Option<&str> {
        let p = &self.output().platform;
        if p.has_clipboard == 0 { return None; }
        if p.clipboard.len == 0 { return Some(""); }
        core::str::from_utf8(unsafe { core::slice::from_raw_parts(p.clipboard.ptr, p.clipboard.len) }).ok()
    }
    /// The window ground colour the last frame asked for.
    #[inline] pub fn background_request(&self) -> Option<pxui_types::geom::Rgba> { let p = &self.output().platform; (p.has_background != 0).then_some(p.background) }
    /// The input-method area of the focused text and its caret offset, in points: `((x, y, w, h), caret)`.
    #[inline] pub fn ime(&self) -> Option<((i64, i64, i64, i64), i64)> { let p = &self.output().platform; (p.has_ime != 0).then(|| ((p.ime_rect[0], p.ime_rect[1], p.ime_rect[2], p.ime_rect[3]), p.ime_caret)) }
    #[inline] pub fn font_size(&self) -> i64 { unsafe { lib::pxui_ui_font_size(self.0.as_ptr()) } }
    #[inline] pub fn row_height(&self) -> i64 { unsafe { lib::pxui_ui_row_height(self.0.as_ptr()) } }
    /// Host-supplied monotonic frame time; cached views share the same animation clock.
    #[inline] pub fn now(&self) -> f64 { unsafe { lib::pxui_ui_now(self.0.as_ptr()) } }
    #[inline] pub fn frame_index(&self) -> u64 { unsafe { lib::pxui_ui_frame_index(self.0.as_ptr()) } }
    #[inline] pub fn captured(&self) -> bool { unsafe { lib::pxui_ui_captured(self.0.as_ptr()) != 0 } }
    #[inline] pub fn text_input_focused(&self) -> bool { unsafe { lib::pxui_ui_text_input_focused(self.0.as_ptr()) != 0 } }
    #[inline] pub fn verifying(&self) -> bool { unsafe { lib::pxui_ui_verifying(self.0.as_ptr()) != 0 } }
    #[inline] pub fn text_undo_bytes(&self) -> usize { unsafe { lib::pxui_ui_text_undo_bytes(self.0.as_ptr()) as usize } }
    #[inline] pub fn inspector_width(&self) -> f32 { unsafe { lib::pxui_ui_inspector_width(self.0.as_ptr()) } }
    #[inline] pub fn inspector_label_x(&self) -> f32 { unsafe { lib::pxui_ui_inspector_label_x(self.0.as_ptr()) } }
    #[inline] pub fn platform(&self) -> Platform { if unsafe { lib::pxui_ui_platform(self.0.as_ptr()) } == 1 { Platform::Apple } else { Platform::Other } }
    #[inline]
    pub fn passed_undo(&self) -> Option<UndoKind> {
        match unsafe { lib::pxui_ui_passed_undo(self.0.as_ptr()) } { 1 => Some(UndoKind::Undo), 2 => Some(UndoKind::Redo), _ => None }
    }
    /// Whether the key (a `KEY_*` code or a character) was pressed in this frame's events.
    #[inline]
    #[track_caller]
    pub fn key_pressed(&self, key: impl Into<u32>) -> bool {
        let key = key.into();
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_key_pressed(self.0.as_ptr(), key, &mut out) });
        out != 0
    }
    /// Raw held state, including modifiers while a field or popup owns keyboard focus.
    #[inline]
    #[track_caller]
    pub fn key_down(&self, key: impl Into<u32>) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_key_down(self.0.as_ptr(), key.into(), &mut out) });
        out != 0
    }
    #[inline] #[track_caller] pub fn set_font_size(&mut self, size: i64) { check(self, unsafe { lib::pxui_ui_set_font_size(self.0.as_ptr(), size) }) }
    #[inline] #[track_caller] pub fn unfocus(&mut self) { check(self, unsafe { lib::pxui_ui_unfocus(self.0.as_ptr()) }) }
    #[inline] #[track_caller] pub fn dismiss_popup(&mut self) { check(self, unsafe { lib::pxui_ui_dismiss_popup(self.0.as_ptr()) }) }
    #[inline]
    #[track_caller]
    pub fn request_cursor(&mut self, shape: Cursor) {
        let code = match shape { Cursor::HorizontalResize => 1, Cursor::VerticalResize => 2, Cursor::Text => 3 };
        check(self, unsafe { lib::pxui_ui_request_cursor(self.0.as_ptr(), code) })
    }
    #[inline] #[track_caller] pub fn repaint_after(&mut self, seconds: f64) { check(self, unsafe { lib::pxui_ui_repaint_after(self.0.as_ptr(), seconds) }) }
    #[inline] #[track_caller] pub fn verify_views(&mut self, on: bool) { check(self, unsafe { lib::pxui_ui_verify_views(self.0.as_ptr(), on as u8) }) }
    #[inline] #[track_caller] pub fn set_timing(&mut self, on: bool) { check(self, unsafe { lib::pxui_ui_set_timing(self.0.as_ptr(), on as u8) }) }
    /// The last frame's time in each phase, in `begin, build, intrinsic, arrange, paint, a11y, finish` order.
    #[track_caller]
    pub fn phase_times(&self) -> [std::time::Duration; 7] {
        let mut nanos = [0u64; 7];
        check(self, unsafe { lib::pxui_ui_phase_times(self.0.as_ptr(), nanos.as_mut_ptr(), nanos.len()) });
        nanos.map(std::time::Duration::from_nanos)
    }
    #[track_caller]
    pub fn bytes(&self) -> Bytes {
        let mut out = [0i64; 3];
        check(self, unsafe { lib::pxui_ui_bytes(self.0.as_ptr(), out.as_mut_ptr(), out.len()) });
        Bytes { atlas: out[0], glyph_lists: out[1], text_undo: out[2] }
    }
    #[track_caller]
    pub fn rebuilt_named(&self, prefix: &str) -> usize {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_rebuilt_named(self.0.as_ptr(), as_str(prefix), &mut out) });
        out as usize
    }
    /// The state of the inspector section built under `key` in the current parent, if there is one.
    #[track_caller]
    pub fn expanded(&self, key: &str) -> Option<bool> {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_expanded(self.0.as_ptr(), as_str(key), &mut out) });
        match out { 0 => None, state => Some(state == 2) }
    }
    #[track_caller]
    pub fn hover_delay(&mut self, key: &str) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_hover_delay(self.0.as_ptr(), as_str(key), &mut out) });
        out != 0
    }
    #[inline] #[track_caller] pub fn tooltip(&mut self, key: &str, text: &str) { check(self, unsafe { lib::pxui_ui_tooltip(self.0.as_ptr(), as_str(key), as_str(text)) }) }
    #[track_caller]
    pub fn cap_width(&mut self, text: &str) -> f32 {
        let mut out = 0.0;
        check(self, unsafe { lib::pxui_ui_cap_width(self.0.as_ptr(), as_str(text), &mut out) });
        out
    }
    /// Where the top of kit text of `size` points sits centred in a row at `y` of height `h`.
    #[inline] pub fn text_top(&self, size: Option<i64>, y: f32, h: f32) -> f32 { text_top_for(self.font_size(), size, y, h) }
}
