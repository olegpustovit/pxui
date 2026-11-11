use crate::PxuiThemeSnapshot;

use crate::state::{Build, PxuiUi, call_build, checked, fail, get};
use crate::{PXUI_ERR_ARGUMENT, Str};
use core::ffi::c_void;
use pxui_core::{Theme, ui::Ui};
use pxui_types::theme::{Font, Look, Shadow};
use pxui_types::ffi::{PxuiFontDesc, PxuiLookDesc, PxuiRingDesc, PxuiShadowDesc, ThemeTokens};

fn font(v: Font) -> PxuiFontDesc {
    PxuiFontDesc { family:v.family, weight:v.weight, size:v.size, tracking:v.tracking, caps:v.caps as u8, reserved:[0;3] }
}

fn shadow(v: Shadow) -> PxuiShadowDesc {
    PxuiShadowDesc { color:v.color, offset:[v.offset.0,v.offset.1], blur:v.blur, spread:v.spread, inner:v.inner as u8, reserved:[0;3] }
}

fn look(v: Look) -> PxuiLookDesc {
    PxuiLookDesc {
        fill:v.fill, gradient:v.gradient.unwrap_or_default(), has_gradient:v.gradient.is_some() as u8, reserved:[0;3], angle:v.angle,
        border:v.border, border_w:v.border_w, sides:v.sides as u32, radius:v.radius,
        shadows:[shadow(v.shadows[0]),shadow(v.shadows[1])], text:v.text, font:font(v.font), opacity:v.opacity,
        ring:PxuiRingDesc { color:v.ring.color, width:v.ring.width, sides:v.ring.sides as u32 },
        selection:v.selection, caret:v.caret, dashed:v.dashed as u8, reserved_tail:[0;3],
    }
}

export!(pub unsafe fn pxui_ui_theme(ui:*mut PxuiUi) -> PxuiThemeSnapshot {
    let Some(ui)=(unsafe{get(ui)}) else { return unsafe{core::mem::zeroed()}; };
    let engine=unsafe{&*ui.engine};
    let theme=engine.theme();
    PxuiThemeSnapshot {
        size:core::mem::size_of::<PxuiThemeSnapshot>() as u32, reserved:0,
        tokens:theme.tokens.into(), value:look(theme.value.idle), hint:look(theme.hint.idle),
        field:look(theme.field.idle), footer:look(theme.footer.idle),
    }
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

// The defaults, or `Theme::from_tokens(tokens)` when `tokens` is set.
export!(bare pub unsafe fn pxui_theme_create(tokens: *const ThemeTokens) -> *mut PxuiTheme {
    let theme = if tokens.is_null() { Theme::default() } else { Theme::from_tokens(unsafe { *tokens }.into()) };
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
