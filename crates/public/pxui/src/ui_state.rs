//! Queries and switches on the `Ui` as a whole, with the engine's names and shapes. A failed call is a
//! caller bug the engine would panic on (a stale handle, a non-finite number, oversized text): it panics
//! here with the library's message, inside the frame callback or at the app's call.
use crate::{as_str, check, Ui};
use pxui_types::ui::{Cursor, UndoKind};
use crate::{KeyCode, Mods};

library!(lib {
    fn pxui_ui_font_points(ui: *mut PxuiUi, out: *mut f32) -> i32;
    fn pxui_ui_row_points(ui: *mut PxuiUi, out: *mut f32) -> i32;
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
    fn pxui_ui_key_down(ui: *mut PxuiUi, code: u32, out: *mut u8) -> i32;
    fn pxui_ui_key_pressed_mods(ui: *mut PxuiUi, code: u32, modifiers: u32, out: *mut u8) -> i32;
    fn pxui_ui_variant(ui: *mut PxuiUi, kind: u32, variant: u16) -> i32;
    fn pxui_ui_set_font_points(ui: *mut PxuiUi, points: f32) -> i32;
    fn pxui_ui_unfocus(ui: *mut PxuiUi) -> i32;
    fn pxui_ui_dismiss_popup(ui: *mut PxuiUi) -> i32;
    fn pxui_ui_request_cursor(ui: *mut PxuiUi, shape: u32) -> i32;
    fn pxui_ui_repaint_after(ui: *mut PxuiUi, seconds: f64) -> i32;
    fn pxui_ui_verify_views(ui: *mut PxuiUi, on: u8) -> i32;
    fn pxui_ui_set_timing(ui: *mut PxuiUi, on: u8) -> i32;
    fn pxui_ui_phase_times(ui: *mut PxuiUi, out: *mut u64, len: usize) -> i32;
    fn pxui_ui_calls(ui: *const PxuiUi) -> u64;
    fn pxui_ui_phase_cpu_times(ui: *mut PxuiUi, out: *mut u64, len: usize) -> i32;
    fn pxui_ui_bytes(ui: *mut PxuiUi, out: *mut i64, len: usize) -> i32;
    fn pxui_ui_platform(ui: *mut PxuiUi) -> u32;
    fn pxui_ui_rebuilt_named(ui: *mut PxuiUi, prefix: pxui_types::Str, out: *mut u64) -> i32;
    fn pxui_ui_expanded(ui: *mut PxuiUi, key: pxui_types::Str, out: *mut u8) -> i32;
    fn pxui_ui_hover_delay(ui: *mut PxuiUi, key: pxui_types::Str, out: *mut u8) -> i32;
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

/// The focused input method's area and caret position in logical surface points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ime {
    /// The area for native input-method placement.
    pub rect: crate::Rect,
    /// The horizontal caret offset in the area, in logical points.
    pub caret: f32,
}

impl Ui {
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
    /// Reports the active input-method rectangle and caret, if any.
    ///
    /// The geometry is in logical surface points.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert!(ui.ime().is_none());
    /// ```
    #[inline]
    pub fn ime(&self) -> Option<Ime> {
        let p = self.output().platform;
        let [x, y, w, h] = p.ime_rect.map(|value| value as f32);
        (p.has_ime != 0).then_some(Ime { rect: crate::Rect::new(x, y, w, h), caret: p.ime_caret as f32 })
    }
    /// The surface's body font size, in logical points.
    ///
    /// Read the current surface's value exactly as created or set, including fractional points. It is read through the checked
    /// fractional query; the 1.x C integer getter keeps reporting its integer form.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// assert!(ui.font_size() > 0.0);
    /// ```
    #[inline] pub fn font_size(&self) -> f32 { let mut out = 0.0; unsafe { lib::pxui_ui_font_points(self.0.as_ptr(), &mut out) }; out }
    /// The kit's row height, in logical points.
    ///
    /// Read the current surface's value, derived from the fractional font size without truncation. The 1.x C integer getter keeps
    /// its integer form.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// assert!(ui.row_height() > 0.0);
    /// ```
    #[inline] pub fn row_height(&self) -> f32 { let mut out = 0.0; unsafe { lib::pxui_ui_row_points(self.0.as_ptr(), &mut out) }; out }
    /// Hands the registered bodies of kind `kind` that the widget call in progress builds `variant` (0 clears it); called only for a non-zero style variant, around that call.
    #[inline(never)]
    pub(crate) fn set_variant(&mut self, kind: pxui_types::theme::BodyKind, variant: u16) { unsafe { lib::pxui_ui_variant(self.0.as_ptr(), kind as u32, variant) }; }
    /// The rounded rendering density used by authoring and paint.
    ///
    /// Read the current surface's value.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// assert!(ui.density() > 0.0);
    /// ```
    #[inline] pub fn density(&self) -> f32 { (unsafe { lib::pxui_ui_density(self.0.as_ptr()) }) as f32 }
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
    /// assert_eq!(ui.text_traits(), pxui::TextTraits::DEFAULT);
    /// ```
    pub fn text_traits(&self) -> pxui_types::TextTraits {
        let mut value=pxui_types::PxuiTextInputOptions::default();
        check(self,unsafe {lib::pxui_ui_text_input_options(self.0.as_ptr(),&mut value)});
        pxui_types::TextTraits::from_numbers(value.autocorrect, value.capitalization, value.action).unwrap_or_default()
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
    /// Reports whether `key` is held at the end of this frame's input.
    ///
    /// This query includes modifiers while a text field or popup owns the keyboard.
    /// Allocates: never.
    /// Identity: `key` names a physical or character key from the input.
    /// Panics: when the Ui has failed or the character is NUL.
    ///
    /// ```
    /// use pxui::{Event, KeyCode};
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// let press = [Event { kind: Event::KEY_PRESSED, code: 'a' as u32, ..Default::default() }];
    /// pxui::testing::frame(&mut ui, &press, &mut |ui| assert!(ui.key_down(KeyCode::Char('a'))));
    /// pxui::testing::frame(&mut ui, &[], &mut |ui| assert!(ui.key_down(KeyCode::Char('a'))));
    /// ```
    #[inline]
    #[track_caller]
    pub fn key_down(&self, key: KeyCode) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_key_down(self.0.as_ptr(), key.wire(), &mut out) });
        out != 0
    }
    /// Reports whether `key` was pressed this frame with the requested modifiers.
    ///
    /// The check uses the state at each press, before a later release or focus loss.
    /// `Mods::ANY` accepts any state, `Mods::NONE` requires no modifiers, and all
    /// modifiers in a nonempty mask must be held during one press. Additional modifiers
    /// are allowed. The query observes raw input without consuming it.
    /// Allocates: never.
    /// Identity: `key` names a physical or character key from the input.
    /// Panics: when the Ui has failed or the character is NUL.
    ///
    /// ```
    /// use pxui::{KeyCode, Mods};
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// let key = KeyCode::Char('k');
    /// pxui::testing::key(&mut ui, key, &mut |ui| {
    ///     assert!(ui.key_pressed(key, Mods::ANY));
    ///     assert!(ui.key_pressed(key, Mods::NONE));
    ///     assert!(!ui.key_pressed(key, Mods::SHIFT));
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn key_pressed(&self, key: KeyCode, mods: Mods) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_key_pressed_mods(self.0.as_ptr(), key.wire(), mods.0, &mut out) });
        out != 0
    }
    /// Sets the surface's body font size in logical points.
    ///
    /// Set a positive finite size between frames. The kit recomputes the row height and uses the chosen fonts.
    /// Allocates: never at this call.
    /// Panics: with the library's message for a size that is not finite or lies outside 0.5..=1024.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.set_font_size(14.5);
    /// assert_eq!(ui.font_size(), 14.5);
    /// ```
    #[inline]
    #[track_caller]
    pub fn set_font_size(&mut self, size: f32) {
        check(self, unsafe { lib::pxui_ui_set_font_points(self.0.as_ptr(), size) })
    }

    /// The checked fractional setter `set_font_size` and creation share.
    pub(crate) fn set_font_points(&self, points: f32) -> Result<(), crate::Error> {
        let rc = unsafe { lib::pxui_ui_set_font_points(self.0.as_ptr(), points) };
        if rc == 0 { Ok(()) } else { Err(crate::Error::from_code(rc)) }
    }
    #[inline] #[track_caller] pub fn unfocus(&mut self) { check(self, unsafe { lib::pxui_ui_unfocus(self.0.as_ptr()) }) }
    #[inline] #[track_caller] pub fn dismiss_popup(&mut self) { check(self, unsafe { lib::pxui_ui_dismiss_popup(self.0.as_ptr()) }) }
    /// Asks the window host for a pointer cursor `shape` for this frame; the request lapses unless it is repeated every build.
    /// Allocates: never.
    /// Panics: when the library refuses the call (outside a frame).
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| ui.request_cursor(pxui::ui::Cursor::Text));
    /// ```
    #[inline]
    #[track_caller]
    pub fn request_cursor(&mut self, shape: Cursor) {
        let code = match shape { Cursor::HorizontalResize => 1, Cursor::VerticalResize => 2, Cursor::Text => 3 };
        check(self, unsafe { lib::pxui_ui_request_cursor(self.0.as_ptr(), code) })
    }
    #[inline] #[track_caller] pub fn repaint_after(&mut self, seconds: f64) { check(self, unsafe { lib::pxui_ui_repaint_after(self.0.as_ptr(), seconds) }) }
    /// Turns view verification on or off for this `Ui`: while on, every clean retained view is built again each frame and a view whose
    /// output differs from the kept one (a revision the app forgot to bump) panics, naming the view.
    /// On by default when built with `debug_assertions` (`Ui::new`); a test that counts rebuilds or measures a frame turns it off.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.verify_views(false);
    /// ```
    #[inline]
    #[track_caller]
    pub fn verify_views(&mut self, on: bool) { check(self, unsafe { lib::pxui_ui_verify_views(self.0.as_ptr(), on as u8) }) }
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
    /// Calls this `Ui` has made into the library since it was created: its own, its frame callbacks', its renderers' and paint
    /// handles' (this one is not counted). The performance gates' count of library calls; per `Ui`, so another `Ui` or thread never
    /// moves it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// let before = ui.library_calls();
    /// pxui::testing::frame(&mut ui, &[], &mut |_| {});
    /// assert!(ui.library_calls() > before);
    /// ```
    pub fn library_calls(&self) -> u64 { unsafe { lib::pxui_ui_calls(self.0.as_ptr()) } }
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
    /// let mut build = |ui: &mut pxui::Ui| { ui.view_of("list", &1, |_| {}); };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// assert_eq!(ui.rebuilt_named("list"), 1);
    /// ui.verify_views(false); // a debug build otherwise rebuilds every kept view to verify it
    /// pxui::testing::frame(&mut ui, &[], &mut build);
    /// assert_eq!(ui.rebuilt_named("list"), 0); // same revision: kept
    /// ```
    #[track_caller]
    pub fn rebuilt_named(&self, prefix: &str) -> usize {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_rebuilt_named(self.0.as_ptr(), as_str(prefix), &mut out) });
        out as usize
    }
    pub(crate) fn expanded_key(&self, key: crate::Key) -> Option<bool> {
        if let crate::Key::Label(label) = key { crate::authored(label); }
        let mut label = [0; 17];
        let key = key.label(&mut label);
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_expanded(self.0.as_ptr(), as_str(key), &mut out) });
        match out { 0 => None, state => Some(state == 2) }
    }
    pub(crate) fn hover_delay_key(&mut self, key: crate::Key) -> bool {
        // the resolved form crosses the 1.x text export, which waits on the key itself (no text copied)
        let mut label = [0; 17];
        let key = crate::Key::Resolved(self.resolve_lookup_key(key));
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_hover_delay(self.0.as_ptr(), as_str(key.label(&mut label)), &mut out) });
        out != 0
    }

}
