use crate::state::{Build, PxuiUi, call_build, checked, fail, get};
use crate::{PXUI_ERR_ARGUMENT, Str};
use core::ffi::c_void;
use pxui_core::{Theme, ui::Ui};
use pxui_types::{ffi::ThemeTokens, theme::ThemeSnapshot};

// The engine's theme is the snapshot's bytes: both are generated from `theme_widgets!` with C layout.
const _: () = assert!(core::mem::size_of::<Theme>() == core::mem::size_of::<ThemeSnapshot>() && core::mem::align_of::<Theme>() == core::mem::align_of::<ThemeSnapshot>()
    && core::mem::offset_of!(Theme, tokens) == core::mem::offset_of!(ThemeSnapshot, tokens) && core::mem::offset_of!(Theme, ring) == core::mem::offset_of!(ThemeSnapshot, ring)
    && core::mem::offset_of!(Theme, image) == core::mem::offset_of!(ThemeSnapshot, image));

// The theme in effect, whole, lent: valid until the `Ui` next changes it (a theme swap, a `themed` scope) or is freed.
export!(trusted pub unsafe fn pxui_ui_theme(ui:*mut PxuiUi) -> *const ThemeSnapshot {
    if unsafe{get(ui)}.is_none() { return core::ptr::null(); }
    unsafe{crate::trusted::pxui_ui_theme(ui)}
});

/// A theme a caller builds from tokens and key/value pairs, then scopes over a subtree with `pxui_ui_themed`. Each `set`
/// or `load` applies over what is there already, as `Theme::parse` does over its base: a `tokens.*` key re-derives every
/// look from the new tokens, so set tokens before the keys that restyle single looks.
pub struct PxuiTheme { theme: Theme, error: String }

impl PxuiTheme {
    /// Applies `text` (a theme file) over the theme, or records why it cannot be and changes nothing.
    fn apply(&mut self, text: &str) -> i32 {
        match Theme::parse(text, &self.theme) {
            Ok(theme) => { self.theme = theme; self.error.clear(); 0 }
            Err(errors) => {
                self.error = errors.iter().map(|e| format!("line {}: {}", e.line, e.message)).collect::<Vec<_>>().join("\n");
                PXUI_ERR_ARGUMENT
            }
        }
    }
}

fn tokens_valid(t: &ThemeTokens) -> bool {
    let tail = [t.motion.fast, t.motion.normal, t.motion.slow, t.density];
    t.space.iter().chain(&t.radius).chain(&tail).all(|v| v.is_finite() && *v >= 0.0) && t.motion.ease.iter().all(|v| v.is_finite()) && t.density > 0.0
        && [t.display_size, t.title_size, t.font_size, t.label_size].iter().all(|s| (1..=4096).contains(s))
        && t.has_ink_ground <= 1 && t.reduced_motion <= 1 && t.reserved == [0; 3] && t.reserved_tail == [0; 3]
}

// The defaults, or `Theme::from_tokens(tokens)` when `tokens` is set; null when the tokens are invalid.
export!(bare pub unsafe fn pxui_theme_create(tokens: *const ThemeTokens) -> *mut PxuiTheme {
    // a null `tokens` is the defaults; tokens that cannot be a theme (not finite, a flag that is no bool, a size or density at or
    // below zero) make no theme
    let theme = match unsafe { tokens.as_ref() } {
        None => Theme::default(),
        Some(_) => match read_desc!(tokens, ThemeTokens) { Some(t) if tokens_valid(&t) => Theme::from_tokens(t.into()), _ => return core::ptr::null_mut() },
    };
    Box::into_raw(Box::new(PxuiTheme { theme, error: String::new() }))
});

export!(bare pub unsafe fn pxui_theme_destroy(theme: *mut PxuiTheme) { if !theme.is_null() { drop(unsafe { Box::from_raw(theme) }); } });

// Sets one theme file key (`button.hover.fill`, `tokens.accent`, ...) to its value text.
export!(bare pub unsafe fn pxui_theme_set(theme: *mut PxuiTheme, key: Str, value: Str) -> i32 {
    let Some(theme) = (unsafe { theme.as_mut() }) else { return PXUI_ERR_ARGUMENT; };
    let (Some(key), Some(value)) = (unsafe { checked(key) }, unsafe { checked(value) }) else { theme.error = "invalid theme text".into(); return PXUI_ERR_ARGUMENT; };
    if key.contains(['\n', '=']) || value.contains('\n') { theme.error = "a key holds no `=` or line break and a value no line break".into(); return PXUI_ERR_ARGUMENT; }
    theme.apply(&format!("{key} = {value}"))
});

// Applies the theme file at `path`.
export!(bare pub unsafe fn pxui_theme_load(theme: *mut PxuiTheme, path: Str) -> i32 {
    let Some(theme) = (unsafe { theme.as_mut() }) else { return PXUI_ERR_ARGUMENT; };
    let Some(path) = (unsafe { checked(path) }) else { theme.error = "invalid theme path".into(); return PXUI_ERR_ARGUMENT; };
    match std::fs::read_to_string(path) {
        Ok(text) => theme.apply(&text),
        Err(error) => { theme.error = format!("{path}: {error}"); PXUI_ERR_ARGUMENT }
    }
});

// Why the last `set` or `load` was refused, borrowed from the theme until its next call.
export!(bare pub unsafe fn pxui_theme_last_error(theme: *const PxuiTheme) -> Str {
    match unsafe { theme.as_ref() } { Some(theme) => Str { ptr: theme.error.as_ptr(), len: theme.error.len() }, None => Str::default() }
});

// Builds a subtree under `theme`; the Ui's own theme is back when the call returns.
export!(pub unsafe fn pxui_ui_themed(ui: *mut PxuiUi, theme: *const PxuiTheme, build: Build, user: *mut c_void) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(theme) = (unsafe { theme.as_ref() }) else { return unsafe { fail(ui, "null theme") }; };
    let shared = state.shared;
    unsafe { (&mut *state.engine).themed(&theme.theme, &mut |engine: &mut Ui| call_build(shared, engine, build, user)); }
    0
});
