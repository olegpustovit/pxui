//! Queries and switches on the `Ui` as a whole, with the engine's names and shapes. A failed call is a
//! caller bug the engine would panic on (a stale handle, a non-finite number, oversized text): it panics
//! here with the library's message, inside the frame callback or at the app's call.
use crate::{as_str, check, Ui};
use pxui_types::ui::{text_top_for, Cursor, UndoKind};

library!(lib {
    fn pxui_ui_font_size(ui: *mut PxuiUi) -> i64;
    fn pxui_ui_row_height(ui: *mut PxuiUi) -> i64;
    fn pxui_ui_environment(ui: *mut PxuiUi, out: *mut pxui_types::Environment) -> i32;
    fn pxui_ui_keyboard(ui: *mut PxuiUi, out: *mut pxui_types::Keyboard) -> i32;
    fn pxui_ui_content_rect(ui: *mut PxuiUi, out: *mut pxui_types::geom::Rect) -> i32;
    fn pxui_ui_text_input_options(ui: *mut PxuiUi, out: *mut pxui_types::PxuiTextInputOptions) -> i32;
    fn pxui_ui_now(ui: *mut PxuiUi) -> f64;
    fn pxui_ui_density(ui: *mut PxuiUi) -> i64;
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
    fn pxui_ui_key_pressed_with(ui: *mut PxuiUi, code: u32, modifier: u32, out: *mut u8) -> i32;
    fn pxui_ui_variant(ui: *mut PxuiUi, kind: u32, variant: u16) -> i32;
    fn pxui_ui_set_font_size(ui: *mut PxuiUi, size: i64) -> i32;
    fn pxui_ui_unfocus(ui: *mut PxuiUi) -> i32;
    fn pxui_ui_dismiss_popup(ui: *mut PxuiUi) -> i32;
    fn pxui_ui_request_cursor(ui: *mut PxuiUi, shape: u32) -> i32;
    fn pxui_ui_repaint_after(ui: *mut PxuiUi, seconds: f64) -> i32;
    fn pxui_verify_new_views(on: u8) -> i32;
    fn pxui_ui_verify_views(ui: *mut PxuiUi, on: u8) -> i32;
    fn pxui_ui_set_timing(ui: *mut PxuiUi, on: u8) -> i32;
    fn pxui_ui_phase_times(ui: *mut PxuiUi, out: *mut u64, len: usize) -> i32;
    fn pxui_ui_phase_cpu_times(ui: *mut PxuiUi, out: *mut u64, len: usize) -> i32;
    fn pxui_ui_bytes(ui: *mut PxuiUi, out: *mut i64, len: usize) -> i32;
    fn pxui_ui_platform(ui: *mut PxuiUi) -> u32;
    fn pxui_ui_rebuilt_named(ui: *mut PxuiUi, prefix: pxui_types::Str, out: *mut u64) -> i32;
    fn pxui_ui_expanded(ui: *mut PxuiUi, key: pxui_types::Str, out: *mut u8) -> i32;
    fn pxui_ui_hover_delay(ui: *mut PxuiUi, key: pxui_types::Str, out: *mut u8) -> i32;
    fn pxui_ui_cap_width(ui: *mut PxuiUi, text: pxui_types::Str, out: *mut f32) -> i32;
});

/// The platform whose editing conventions the `Ui` follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Platform {
    Apple,
    #[default] Other,
}

/// The heap a `Ui` holds, for the host to account for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bytes {
    pub atlas: i64,
    pub glyph_lists: i64,
    pub text_undo: i64,
}

impl Ui {
    /// `verify_views(true)` for every `Ui` created from now on: a test harness turns it on once.
    #[track_caller] pub fn verify_new_views(on: bool) { assert_eq!(unsafe { lib::pxui_verify_new_views(on as u8) }, 0, "pxui: verify_new_views refused") }
    /// The last frame asked for the clipboard's text: send it with the next frame as a paste event.
    #[inline] pub fn paste_requested(&self) -> bool { self.output().platform.paste != 0 }
    /// The text the last frame wants on the clipboard (a copy or cut), or `None` when it asked for none.
    ///
    /// Borrowed from the library until the next call on this `Ui`; copy it to the system clipboard before the next frame. An empty string is a request to clear it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |ui| { ui.label("hi", pxui::Label::DEFAULT); });
    /// assert_eq!(ui.clipboard_request(), None);
    /// ```
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
    /// Hands the registered bodies of kind `kind` that the widget call in progress builds `variant` (0 clears it); called only for a non-zero style variant, around that call.
    #[inline(never)]
    pub(crate) fn set_variant(&mut self, kind: pxui_types::theme::BodyKind, variant: u16) { unsafe { lib::pxui_ui_variant(self.0.as_ptr(), kind as u32, variant) }; }
    /// Integer rendering density shared with Paint: host scale rounded to at least one.
    #[inline] pub fn density(&self) -> i64 { unsafe { lib::pxui_ui_density(self.0.as_ptr()) } }
    /// The last frame's host geometry (safe-area insets, keyboard frame and animation), the same value every cached view sees.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert_eq!(ui.environment().keyboard_visible, 0);
    /// ```
    pub fn environment(&self) -> pxui_types::Environment {
        let mut value=pxui_types::Environment::default();
        check(self,unsafe { lib::pxui_ui_environment(self.0.as_ptr(),&mut value) });
        value
    }
    /// The native keyboard's presentation geometry (`frame`, in window points, and `Keyboard::VALID`/`ANIMATING` flags in `flags`).
    ///
    /// Unlike `environment`, it follows the keyboard while it animates and does not change the frame's layout.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert_eq!(ui.keyboard().flags & pxui::Keyboard::VALID, 0); // a headless host reports no keyboard
    /// ```
    pub fn keyboard(&self) -> pxui_types::Keyboard {
        let mut value = pxui_types::Keyboard::default();
        check(self, unsafe { lib::pxui_ui_keyboard(self.0.as_ptr(), &mut value) });
        value
    }
    /// The platform keyboard traits (autocorrect, capitalization, return key) of the text field in focus, or the defaults when none is.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert_eq!(ui.text_input_options(), pxui::TextInputOptions::DEFAULT);
    /// ```
    pub fn text_input_options(&self) -> pxui_types::TextInputOptions {
        let mut value=pxui_types::PxuiTextInputOptions::default();
        check(self,unsafe {lib::pxui_ui_text_input_options(self.0.as_ptr(),&mut value)});
        value.into()
    }
    /// The window area left above a docked keyboard, in window points: the whole window while no keyboard is shown.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// let r = ui.content_rect();
    /// assert_eq!((r.w, r.h), (320.0, 240.0));
    /// ```
    pub fn content_rect(&self) -> pxui_types::geom::Rect {
        let mut value = pxui_types::geom::Rect::new(0.0, 0.0, 0.0, 0.0);
        check(self, unsafe { lib::pxui_ui_content_rect(self.0.as_ptr(), &mut value) });
        value
    }
    /// The undo or redo keystroke the focused text field could not apply because its own history was exhausted, or `None`.
    /// Valid for the frame that received the key: hand the step to the application's own undo when it is `Some`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert_eq!(ui.passed_undo(), None);
    /// ```
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
    /// Whether `key` (a key code, a character or `KEY_SPECIAL` plus an index, modifiers included) is held in this frame's input.
    /// A raw query, unlike `key_pressed` it reports the held state: a key pressed and released in one frame was never held at the frame's end. While a text field or popup owns the keyboard the modifiers are still reported.
    /// Allocates: never.
    /// Identity: `key` is a key code, not a node key.
    /// Panics: when the `Ui` has failed, like every `Ui` call.
    ///
    /// ```
    /// use pxui::Event;
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// let press = [Event { kind: Event::KEY_PRESSED, code: 'a' as u32, ..Default::default() }];
    /// pxui::testing::frame(&mut ui, &press, &mut |ui| assert!(ui.key_down('a' as u32)));
    /// pxui::testing::frame(&mut ui, &[], &mut |ui| assert!(ui.key_down('a' as u32))); // still held
    /// ```
    #[inline]
    #[track_caller]
    pub fn key_down(&self, key: u32) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_key_down(self.0.as_ptr(), key, &mut out) });
        out != 0
    }
    /// Whether `key` (a `KEY_*` code or a character) was pressed in this frame's events; with `modifier` (Shift, Ctrl, Alt or Meta
    /// code), only a press made while that modifier was held, even if it was released in the same frame.
    ///
    /// A raw query: it neither filters modal or widget ownership nor consumes the press. A `modifier` that is not one of the four is
    /// refused (the call panics).
    /// Allocates: never.
    /// Identity: `key` is a key code, not a node key.
    /// Panics: when the `Ui` has failed, like every `Ui` call.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// let mut seen = (false, false);
    /// // no modifier was held for the press, so the query with Shift (special key 12) does not match
    /// let shift = pxui_types::KEY_SPECIAL + 12;
    /// pxui::testing::key(&mut ui, 'k' as u32, &mut |ui| { seen = (ui.key_pressed('k', None), ui.key_pressed('k', Some(shift))); });
    /// assert_eq!(seen, (true, false));
    /// ```
    #[inline]
    #[track_caller]
    pub fn key_pressed(&self, key: impl Into<u32>, modifier: Option<u32>) -> bool {
        let key = key.into();
        let mut out = 0;
        check(self, unsafe { match modifier {
            None => lib::pxui_ui_key_pressed(self.0.as_ptr(), key, &mut out),
            Some(modifier) => lib::pxui_ui_key_pressed_with(self.0.as_ptr(), key, modifier, &mut out),
        } });
        out != 0
    }
    /// Asks the window host for a pointer cursor `shape` for this frame; the request lapses unless it is repeated every build.
    /// Allocates: never.
    /// Panics: when the library refuses the call (outside a frame).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| ui.request_cursor(pxui::ui::Cursor::Text));
    /// ```
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
    /// The last frame's wall time in each phase, in `begin, build, intrinsic, arrange, paint, a11y, finish` order; all zero until `set_timing(true)` and a frame has run.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// ui.set_timing(true);
    /// pxui::testing::frame(&mut ui, &[], &mut |_| {});
    /// assert_eq!(ui.phase_times().len(), 7);
    /// ```
    #[track_caller]
    pub fn phase_times(&self) -> [std::time::Duration; 7] {
        let mut nanos = [0u64; 7];
        check(self, unsafe { lib::pxui_ui_phase_times(self.0.as_ptr(), nanos.as_mut_ptr(), nanos.len()) });
        nanos.map(std::time::Duration::from_nanos)
    }
    /// The last frame's thread CPU time in each phase, in `phase_times` order, summed across settling passes.
    ///
    /// Needs `set_timing(true)` and a platform thread CPU clock (Apple platforms today); there is no wall-clock fallback.
    /// Allocates: never.
    /// Panics: never; an unavailable clock is the `Err` below.
    /// Errors: `Error::State` unless timing is enabled and the platform has a thread CPU clock.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// ui.set_timing(true);
    /// pxui::testing::frame(&mut ui, &[], &mut |_| {});
    /// let _cpu = ui.phase_cpu_times(); // `Err` on a platform without a thread CPU clock
    /// ```
    #[track_caller]
    pub fn phase_cpu_times(&self) -> Result<[std::time::Duration; 7], crate::Error> {
        let mut nanos = [0u64; 7];
        let code = unsafe {
            lib::pxui_ui_phase_cpu_times(self.0.as_ptr(), nanos.as_mut_ptr(), nanos.len())
        };
        if code == 0 {
            Ok(nanos.map(std::time::Duration::from_nanos))
        } else {
            Err(crate::Error::from_code(code))
        }
    }
    /// The heap this `Ui` holds for the host's accounting: glyph atlas, glyph lists and text undo history, in bytes.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui`).
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |ui| { ui.label("hi", pxui::Label::DEFAULT); });
    /// assert_eq!(ui.bytes().text_undo, 0);
    /// ```
    #[track_caller]
    pub fn bytes(&self) -> Bytes {
        let mut out = [0i64; 3];
        check(self, unsafe { lib::pxui_ui_bytes(self.0.as_ptr(), out.as_mut_ptr(), out.len()) });
        Bytes { atlas: out[0], glyph_lists: out[1], text_undo: out[2] }
    }
    /// How many views whose label starts with `prefix` the last frame built instead of keeping (their revision changed, or they were new).
    ///
    /// An empty `prefix` counts every view. Read after the frame, never inside a view's own build.
    /// Allocates: never.
    /// Panics: when the library refuses the call (a null `Ui` or a `prefix` over the text limit).
    ///
    /// ```
    /// let mut build = |ui: &mut pxui::Ui| { ui.view("list", 1, pxui::ui::Size::Grow, pxui::ui::Size::Grow, &mut |_| {}); };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// assert_eq!(ui.rebuilt_named("list"), 1);
    /// pxui::testing::frame(&mut ui, &[], &mut build);
    /// assert_eq!(ui.rebuilt_named("list"), 0); // same revision: kept
    /// ```
    #[track_caller]
    pub fn rebuilt_named(&self, prefix: &str) -> usize {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_rebuilt_named(self.0.as_ptr(), as_str(prefix), &mut out) });
        out as usize
    }
    /// The state of the inspector section built under `key` in the current parent: `Some(true)` expanded, `Some(false)` collapsed, `None` when no section of that key has been built there.
    /// Read it where the section is built (inside the same parent); it is `None` until the section has first been built there, and keeps its state from frame to frame.
    /// Allocates: never.
    /// Identity: `key` is the key given to `inspector_section`, resolved under the current parent's seed as for any label.
    /// Panics: when the library refuses the call (a null `Ui`, or `key` over the text limit).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     assert_eq!(ui.expanded("details"), None); // not built yet
    ///     ui.inspector_section("details", true, None, "Details", &mut |ui| { ui.label("body", pxui::Label::DEFAULT); });
    ///     assert_eq!(ui.expanded("details"), Some(true));
    /// });
    /// ```
    #[track_caller]
    pub fn expanded(&self, key: &str) -> Option<bool> {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_expanded(self.0.as_ptr(), as_str(key), &mut out) });
        match out { 0 => None, state => Some(state == 2) }
    }
    /// Whether the pointer has rested, unmoved, on the target named `key` for the hover delay (the tooltip wait); call it every frame the pointer is over the target.
    /// A different `key`, a moved pointer, a missed frame, a touch, a press in progress, a modal or focus loss restart the wait. The first call for a key is always false.
    /// Allocates: the first call for a key copies it; never afterwards for the same key.
    /// Identity: `key` names the hover target for this one wait, compared as text (not resolved under a parent): two targets with one key share the wait.
    /// Panics: when the library refuses the call (a null `Ui`, or `key` over the text limit).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| assert!(!ui.hover_delay("play")));
    /// ```
    #[track_caller]
    pub fn hover_delay(&mut self, key: &str) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_hover_delay(self.0.as_ptr(), as_str(key), &mut out) });
        out != 0
    }
    /// The width in points a string takes in the small-caps label style at the current theme.
    /// Allocates: on the first measurement of `text` (the text cache); never when it is cached.
    /// Panics: when the library refuses the call (outside a frame, or text over the limit).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| assert!(ui.cap_width("TITLE") > ui.cap_width("T")));
    /// ```
    #[track_caller]
    pub fn cap_width(&mut self, text: &str) -> f32 {
        let mut out = 0.0;
        check(self, unsafe { lib::pxui_ui_cap_width(self.0.as_ptr(), as_str(text), &mut out) });
        out
    }
    /// Where the top of kit text of `size` points sits centred in a row at `y` of height `h`.
    #[inline] pub fn text_top(&self, size: Option<i64>, y: f32, h: f32) -> f32 { text_top_for(self.font_size(), size, y, h) }
}
