//! A theme built from tokens and key/value pairs (the keys of a theme file), scoped over a subtree with `Ui::themed`.
use crate::composite::call;
use crate::sys::PxuiTheme;
use crate::{as_str, check, Ui};
use core::{ffi::c_void, ptr::NonNull};
use pxui_types::{ffi::ThemeTokens, theme::Tokens, Str};

library!(lib {
    fn pxui_theme_create(tokens: *const ThemeTokens) -> *mut PxuiTheme;
    fn pxui_theme_destroy(theme: *mut PxuiTheme);
    fn pxui_theme_set(theme: *mut PxuiTheme, key: Str, value: Str) -> i32;
    fn pxui_theme_load(theme: *mut PxuiTheme, path: Str) -> i32;
    fn pxui_theme_last_error(theme: *const PxuiTheme) -> Str;
    fn pxui_ui_themed(ui: *mut PxuiUi, theme: *const PxuiTheme, build: Option<extern "C" fn(*mut c_void, *mut PxuiUi)>, user: *mut c_void) -> i32;
});

/// A theme owned by the library. `set` and `load` apply over what the theme holds, as a theme file applies over its base: a
/// `tokens.*` key re-derives every look from the new tokens, so set tokens before the keys that restyle single looks.
pub struct Theme(NonNull<PxuiTheme>);

impl Theme {
    fn make(tokens: *const ThemeTokens) -> Theme {
        Theme(NonNull::new(unsafe { lib::pxui_theme_create(tokens) }).expect("pxui: the library could not make a theme"))
    }
    pub fn from_tokens(tokens: Tokens) -> Theme { Theme::make(&ThemeTokens::from(tokens)) }
    fn result(&self, rc: i32) -> Result<(), String> {
        if rc == 0 { return Ok(()); }
        let message = unsafe { lib::pxui_theme_last_error(self.0.as_ptr()) };
        Err(if message.len == 0 { format!("theme call failed ({rc})") } else { String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(message.ptr, message.len) }).into_owned() })
    }
    /// Sets one theme file key (`button.hover.fill`, `tokens.accent`, ...); the error says why it was refused.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> { self.result(unsafe { lib::pxui_theme_set(self.0.as_ptr(), as_str(key), as_str(value)) }) }
    /// Applies the theme file at `path`; nothing changes unless all of it is valid.
    pub fn load(&mut self, path: &str) -> Result<(), String> { self.result(unsafe { lib::pxui_theme_load(self.0.as_ptr(), as_str(path)) }) }
}

impl Default for Theme { fn default() -> Theme { Theme::make(core::ptr::null()) } }
impl Drop for Theme { fn drop(&mut self) { unsafe { lib::pxui_theme_destroy(self.0.as_ptr()) } } }

impl Ui {
    /// Builds `f`'s subtree under `theme`; the `Ui`'s own theme is back afterwards.
    #[track_caller]
    pub fn themed<F: FnMut(&mut Ui)>(&mut self, theme: &Theme, mut f: F) {
        check(self, unsafe { lib::pxui_ui_themed(self.0.as_ptr(), theme.0.as_ptr(), Some(call::<F>), &mut f as *mut F as *mut c_void) })
    }
}
