use crate::PxuiThemeSnapshot;

use crate::state::{Build, PxuiUi, call_build, checked, fail, get};
use crate::{PXUI_ERR_ARGUMENT, Str};
use core::ffi::c_void;
use pxui_core::{Theme, ui::Ui};
use pxui_types::ffi::{PxuiLookDesc, PxuiRingDesc, ThemeTokens};

macro_rules! fill_parts {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {
        /// The kit parts of `theme` in list order, six looks each.
        fn parts(theme: &Theme) -> Vec<PxuiLookDesc> {
            let mut out = Vec::with_capacity(pxui_types::theme::WIDGET_COUNT * 6);
            for s in [$(&theme.$name),*] { out.extend([s.idle, s.hover, s.pressed, s.focus, s.disabled, s.checked].map(PxuiLookDesc::from)); }
            out
        }
    };
}
pxui_types::theme_widgets!(fill_parts);

// The theme in effect, whole: its scalars in `out` and every kit part's six looks in `looks` (`capacity` looks; the count a
// snapshot lists is `out.part_count` * 6, set also when `capacity` is too small, which is an error).
export!(pub unsafe fn pxui_ui_theme(ui:*mut PxuiUi, out:*mut PxuiThemeSnapshot, looks:*mut PxuiLookDesc, capacity:usize) -> i32 {
    let Some(state)=(unsafe{get(ui)}) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() || (looks.is_null() && capacity != 0) { return unsafe{fail(ui,"null theme output")}; }
    let theme=unsafe{&*state.engine}.theme();
    let list=parts(theme);
    let snapshot=PxuiThemeSnapshot {
        size:core::mem::size_of::<PxuiThemeSnapshot>() as u32, part_count:pxui_types::theme::WIDGET_COUNT as u32, tokens:theme.tokens.into(),
        row_h:theme.row_h, control_h:theme.control_h, scrollbar_w:theme.scrollbar_w, scrollbar_gap:theme.scrollbar_gap,
        ring:PxuiRingDesc { color:theme.ring.color, width:theme.ring.width, sides:theme.ring.sides as u32 },
    };
    unsafe { *out=snapshot; }
    if capacity < list.len() { return unsafe{fail(ui,"the looks buffer is too small for the theme")}; }
    unsafe { core::ptr::copy_nonoverlapping(list.as_ptr(), looks, list.len()); }
    0
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
