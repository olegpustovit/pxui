//! A theme built from tokens and key/value pairs (the keys of a theme file), scoped over a subtree with `Ui::themed`.
use crate::composite::call;
use crate::sys::PxuiTheme;
use crate::{as_text, check, Ui};
use core::{ffi::c_void, ptr::NonNull};
use pxui_types::{ffi::ThemeTokens, geom::Rgba, theme::{ColorToken, Tokens}, Str};

library!(lib {
    fn pxui_theme_create(tokens: *const ThemeTokens) -> *mut PxuiTheme;
    fn pxui_theme_destroy(theme: *mut PxuiTheme);
    fn pxui_theme_set(theme: *mut PxuiTheme, key: Str, value: Str) -> i32;
    fn pxui_theme_load(theme: *mut PxuiTheme, path: Str) -> i32;
    fn pxui_theme_last_error(theme: *const PxuiTheme) -> Str;
    fn pxui_ui_theme_apply(ui: *mut PxuiUi, theme: *mut PxuiTheme, text: Str) -> i32;
    fn pxui_ui_themed(ui: *mut PxuiUi, theme: *const PxuiTheme, build: pxui_types::ffi::UiBuild, user: *mut c_void) -> i32;
    fn pxui_ui_use_theme(ui:*mut PxuiUi,theme:*const PxuiTheme)->i32;
    fn pxui_ui_set_colors(ui:*mut PxuiUi,light:*const Rgba,dark:*const Rgba,count:usize)->i32;
    fn pxui_ui_color(ui:*mut PxuiUi,token:u16)->Rgba;
});

/// A theme owned by the library. `set` and `load` apply over what the theme holds, as a theme file applies over its base: a
/// `tokens.*` key re-derives every look from the new tokens, so set tokens before the keys that restyle single looks.
pub struct Theme(NonNull<PxuiTheme>);

impl Theme {
    fn make(tokens: *const ThemeTokens) -> Theme {
        Theme(NonNull::new(unsafe { lib::pxui_theme_create(tokens) }).expect("pxui: the library refused the theme tokens (not finite, a size or density at or below zero, a flag that is no bool)"))
    }
    pub fn from_tokens(tokens: Tokens) -> Theme { Theme::make(&ThemeTokens::from(tokens)) }
    fn result(&self, rc: i32) -> Result<(), String> {
        if rc == 0 { return Ok(()); }
        let message = unsafe { lib::pxui_theme_last_error(self.0.as_ptr()) };
        Err(if message.len == 0 { format!("theme call failed ({rc})") } else { String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(message.ptr, message.len) }).into_owned() })
    }
    /// Sets one theme file key (`button.hover.fill`, `tokens.accent`, ...); the error says why it was refused.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> { self.result(unsafe { lib::pxui_theme_set(self.0.as_ptr(), as_text(key), as_text(value)) }) }
    /// Applies theme file text over the theme (any number of lines), with `KIND.body = NAME` naming the bodies `ui` registered
    /// (`Ui::register_toggle_body` and its siblings); an unregistered name is an error and nothing changes.
    pub fn apply(&mut self, ui: &Ui, text: &str) -> Result<(), String> { self.result(unsafe { lib::pxui_ui_theme_apply(ui.0.as_ptr(), self.0.as_ptr(), as_text(text)) }) }
    /// Applies the theme file at `path`; nothing changes unless all of it is valid.
    pub fn load(&mut self, path: &str) -> Result<(), String> { self.result(unsafe { lib::pxui_theme_load(self.0.as_ptr(), as_text(path)) }) }
}

impl Default for Theme { fn default() -> Theme { Theme::make(core::ptr::null()) } }
impl Drop for Theme { fn drop(&mut self) { unsafe { lib::pxui_theme_destroy(self.0.as_ptr()) } } }

impl Ui {
    /// Select a parsed root theme between frames. Host reduced motion still overrides it.
    pub fn use_theme(&mut self,theme:&Theme) {
        check(self,unsafe { lib::pxui_ui_use_theme(self.0.as_ptr(),theme.0.as_ptr()) });
    }

    /// Installs the app's colour tokens: `colors[i]` is the (light, dark) pair of `ColorToken(i)`, every later token reads transparent.
    ///
    /// Call between frames, like `use_theme` (install the table once, beside the themes). Reads (`color`, `Paint::color`)
    /// pick the dark entry while the theme in effect is dark (`pxui::theme::dark` of its tokens), so swapping a light theme
    /// for a dark one changes what they return without the app rebuilding its state. A table that differs from the
    /// installed one rebuilds every view once; an equal one changes nothing.
    /// Allocates: never.
    /// Panics: when `colors` holds more than `ColorToken::LIMIT` (64) entries, inside a frame, or when the `Ui` has failed.
    ///
    /// ```
    /// use pxui::{geom::Rgba, theme::ColorToken};
    /// let (paper, night) = (Rgba { r: 250, g: 250, b: 250, a: 255 }, Rgba { r: 16, g: 16, b: 16, a: 255 });
    /// let mut ui = pxui::Ui::new().unwrap();
    /// ui.set_colors(&[(paper, night)]);
    /// let light = !pxui::theme::dark(&ui.tokens());
    /// assert_eq!(ui.color(ColorToken(0)), if light { paper } else { night });
    /// assert_eq!(ui.color(ColorToken(1)), Rgba::default());
    /// ```
    #[track_caller]
    pub fn set_colors(&mut self, colors: &[(Rgba, Rgba)]) {
        assert!(colors.len() <= ColorToken::LIMIT, "pxui: set_colors takes a table of at most 64 (light, dark) pairs");
        // the library takes two arrays: split the pairs on the stack
        let (mut light, mut dark) = ([Rgba::default(); ColorToken::LIMIT], [Rgba::default(); ColorToken::LIMIT]);
        for (at, &(l, d)) in colors.iter().enumerate() { (light[at], dark[at]) = (l, d); }
        check(self, unsafe { lib::pxui_ui_set_colors(self.0.as_ptr(), light.as_ptr(), dark.as_ptr(), colors.len()) });
    }

    /// Colour token `token` for the theme in effect: its dark entry while that theme is dark, else its light one.
    ///
    /// Inside `themed` the scope's theme decides. A token past the installed table (or past `ColorToken::LIMIT`) reads
    /// transparent, as does every token before `set_colors`. Read it while building, or between frames.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{geom::Rgba, theme::ColorToken};
    /// let ink = Rgba { r: 20, g: 20, b: 20, a: 255 };
    /// let mut ui = pxui::Ui::new().unwrap();
    /// ui.set_colors(&[(ink, ink)]);
    /// ui.frame(&pxui::FrameIn::default(), &mut |ui| assert_eq!(ui.color(ColorToken(0)), ink)).unwrap();
    /// ```
    #[inline]
    pub fn color(&self, token: ColorToken) -> Rgba { unsafe { lib::pxui_ui_color(self.0.as_ptr(), token.0) } }

    /// Builds `f`'s subtree under `theme`; the `Ui`'s own theme is back afterwards.
    #[track_caller]
    pub fn themed(&mut self, theme: &Theme, mut f: &mut dyn FnMut(&mut Ui)) {
        check(self, unsafe { lib::pxui_ui_themed(self.0.as_ptr(), theme.0.as_ptr(), Some(call::<&mut dyn FnMut(&mut Ui)>), &mut f as *mut _ as *mut c_void) })
    }
}
