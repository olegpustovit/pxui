//! Parsed themes for a whole UI or one subtree.
use crate::composite::call;
use crate::sys::PxuiTheme;
use crate::{as_text, check, status, Error, Ui};
use core::{ffi::c_void, ptr::NonNull};
use pxui_types::{ffi::ThemeTokens, geom::Rgba, theme::{ColorToken, Tokens}, Str};

library!(lib {
    fn pxui_theme_create(tokens: *const ThemeTokens) -> *mut PxuiTheme;
    fn pxui_theme_destroy(theme: *mut PxuiTheme);
    fn pxui_theme_parse(theme: *mut PxuiTheme, source: Str) -> i32;
    fn pxui_theme_set_color(theme: *mut PxuiTheme, token: u16, color: Rgba) -> i32;
    fn pxui_theme_density(theme: *mut PxuiTheme, density: f32) -> i32;
    fn pxui_theme_reduced_motion(theme: *mut PxuiTheme, on: u8) -> i32;
    fn pxui_ui_themed(ui: *mut PxuiUi, theme: *const PxuiTheme, build: pxui_types::ffi::UiBuild, user: *mut c_void) -> i32;
    fn pxui_ui_use_theme(ui:*mut PxuiUi,theme:*const PxuiTheme)->i32;
    fn pxui_ui_color(ui:*mut PxuiUi,token:u16)->Rgba;
});

/// A retained theme selected by `Ui::set_theme` or scoped by `Ui::themed`.
pub struct ThemeHandle(NonNull<PxuiTheme>);

impl ThemeHandle {
    fn make(tokens: *const ThemeTokens) -> Option<ThemeHandle> {
        NonNull::new(unsafe { lib::pxui_theme_create(tokens) }).map(ThemeHandle)
    }

    /// Derives a theme's looks from the supplied kit tokens.
    ///
    /// Allocates: the theme in the library.
    /// Panics: when the token values cannot make a theme.
    ///
    /// ```
    /// let theme = pxui::ThemeHandle::from_tokens(pxui::theme::Tokens::DEFAULT);
    /// pxui::doc::frames(1, &mut |ui| ui.themed(&theme, &mut |ui| assert_eq!(ui.tokens().density, 1.0)));
    /// ```
    #[track_caller]
    pub fn from_tokens(tokens: Tokens) -> ThemeHandle {
        Self::make(&ThemeTokens::from(tokens)).expect("pxui: invalid theme tokens")
    }

    /// Parses theme-file text over the default theme.
    ///
    /// Invalid keys, values and body-name syntax return `Error::InvalidInput`. A
    /// `KIND.body = NAME` line keeps the body name for resolution in the destination Ui.
    /// Allocates: the parsed theme, body names and parser storage.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{Error, ThemeHandle};
    /// assert!(ThemeHandle::parse("tokens.accent = #ff8000").is_ok());
    /// assert!(matches!(ThemeHandle::parse("tokens.accent = orange"), Err(Error::InvalidInput)));
    /// ```
    pub fn parse(source: &str) -> Result<ThemeHandle, Error> {
        let theme = Self::make(core::ptr::null()).ok_or(Error::Allocation)?;
        status(unsafe { lib::pxui_theme_parse(theme.0.as_ptr(), as_text(source)) })?;
        Ok(theme)
    }

    /// Assigns a color to an app token in this theme.
    ///
    /// Unset entries read transparent. A color that names another token is stored as
    /// transparent so token lookups never recurse.
    /// Allocates: never.
    /// Panics: if `token` is outside `ColorToken::LIMIT`.
    ///
    /// ```
    /// use pxui::{theme::ColorToken, Rgba, ThemeHandle};
    /// let mut theme = ThemeHandle::default();
    /// theme.set_color(ColorToken(0), Rgba::rgb(255, 128, 0));
    /// ```
    #[track_caller]
    pub fn set_color(&mut self, token: ColorToken, color: Rgba) {
        let rc = unsafe { lib::pxui_theme_set_color(self.0.as_ptr(), token.0, color) };
        assert_eq!(rc, 0, "pxui: invalid theme color token");
    }

    /// Sets the kit's density multiplier while preserving the theme's authored looks.
    ///
    /// Allocates: never.
    /// Panics: if `density` is nonfinite or nonpositive.
    ///
    /// ```
    /// let mut compact = pxui::ThemeHandle::default();
    /// compact.density(0.84);
    /// pxui::doc::frames(1, &mut |ui| ui.themed(&compact, &mut |ui| assert_eq!(ui.tokens().density, 0.84)));
    /// ```
    #[track_caller]
    pub fn density(&mut self, density: f32) {
        let rc = unsafe { lib::pxui_theme_density(self.0.as_ptr(), density) };
        assert_eq!(rc, 0, "pxui: invalid theme density");
    }

    /// Sets this theme's reduced-motion preference.
    ///
    /// The host's reduced-motion preference also applies, in every theme scope.
    /// Allocates: never.
    /// Panics: if the library refuses the theme handle.
    ///
    /// ```
    /// let mut quiet = pxui::ThemeHandle::default();
    /// quiet.reduced_motion(true);
    /// pxui::doc::frames(1, &mut |ui| ui.themed(&quiet, &mut |ui| assert!(ui.tokens().reduced_motion)));
    /// ```
    #[track_caller]
    pub fn reduced_motion(&mut self, on: bool) {
        let rc = unsafe { lib::pxui_theme_reduced_motion(self.0.as_ptr(), on as u8) };
        assert_eq!(rc, 0, "pxui: invalid theme handle");
    }
}

impl Default for ThemeHandle {
    fn default() -> Self { Self::make(core::ptr::null()).expect("pxui: could not create default theme") }
}

impl Drop for ThemeHandle {
    fn drop(&mut self) { unsafe { lib::pxui_theme_destroy(self.0.as_ptr()) } }
}

impl Ui {
    /// Installs this theme for the whole Ui between frames.
    ///
    /// Body names resolve in this Ui before installation. An absent name returns
    /// `Error::InvalidInput` with the detail in `Ui::last_error`. A changed theme
    /// invalidates retained views. The Ui owns it, and the host can require reduced motion.
    /// Calling during a frame returns `Error::State`.
    /// Allocates: never when selecting a prepared theme with its own palette.
    /// Panics: never.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// let theme = pxui::ThemeHandle::parse("tokens.accent = #ff8000").unwrap();
    /// ui.set_theme(&theme).unwrap();
    /// assert_eq!(ui.tokens().accent, pxui::Rgba::rgb(255, 128, 0));
    /// ```
    pub fn set_theme(&mut self, theme: &ThemeHandle) -> Result<(), Error> {
        status(unsafe { lib::pxui_ui_use_theme(self.0.as_ptr(), theme.0.as_ptr()) })
    }

    /// Reads a color token from the theme in effect.
    ///
    /// Inside `themed` reads the active scope's palette. An unset token reads
    /// transparent. Read while building or between frames.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{theme::ColorToken, Rgba, ThemeHandle};
    /// let ink = Rgba::rgb(20, 20, 20);
    /// let mut theme = ThemeHandle::default();
    /// theme.set_color(ColorToken(0), ink);
    /// pxui::doc::frames(1, &mut |ui| ui.themed(&theme, &mut |ui| assert_eq!(ui.color(ColorToken(0)), ink)));
    /// ```
    #[inline]
    pub fn color(&self, token: ColorToken) -> Rgba { unsafe { lib::pxui_ui_color(self.0.as_ptr(), token.0) } }

    /// Builds `f`'s subtree using the supplied theme.
    ///
    /// The prior theme resumes when the call returns. Body names resolve in this Ui
    /// and fall back to the built-in body if absent. The host can require reduced motion.
    /// Allocates: when the retained theme stack or scope cache grows.
    /// Panics: outside a build, on a refused Ui, or when the callback panics.
    ///
    /// ```
    /// let theme = pxui::ThemeHandle::parse("tokens.accent = #ff8000").unwrap();
    /// pxui::doc::frames(1, &mut |ui| ui.themed(&theme, &mut |ui| assert_eq!(ui.tokens().accent.g, 128)));
    /// ```
    #[track_caller]
    pub fn themed(&mut self, theme: &ThemeHandle, mut f: &mut dyn FnMut(&mut Ui)) {
        check(self, unsafe { lib::pxui_ui_themed(self.0.as_ptr(), theme.0.as_ptr(), Some(call::<&mut dyn FnMut(&mut Ui)>), &mut f as *mut _ as *mut c_void) })
    }
}
