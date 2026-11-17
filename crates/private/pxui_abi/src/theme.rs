use crate::state::{Build, PxuiUi, call_build, checked, fail, get};
use crate::{PXUI_ERR_ARGUMENT, Str};
use core::ffi::c_void;
use pxui_core::{Theme, ui::Ui};
use pxui_types::{ffi::ThemeTokens, geom::Rgba, theme::{ColorToken, ThemeSnapshot}};

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
/// look from the new tokens, so set tokens before the keys that restyle single looks. Body ids are per `Ui`, so the theme also keeps
/// the name each `KIND.body` line selected (`names`, one per kind at most) and resolves them in the `Ui` it is used in (`in_ui`).
pub struct PxuiTheme { theme: Theme, names: Vec<(&'static str, String)>, error: String, pub(crate) poisoned: bool }

// A parsed theme can be selected repeatedly without parsing or allocating on an OS appearance change.
export!(pub unsafe fn pxui_ui_use_theme(ui:*mut PxuiUi,theme:*const PxuiTheme)->i32 {
    let Some(state)=(unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT };
    if !state.owner || state.shared().in_frame { return crate::PXUI_ERR_STATE; }
    let Some(theme)=(unsafe { theme.as_ref() }) else { return unsafe { fail(ui,"null theme") }; };
    let engine = unsafe { &mut *state.engine };
    engine.set_theme(theme.in_ui(engine));
    0
});

// Installs the app's colour tokens between frames: `light[i]`/`dark[i]` are token `i`, `count` at most 64 (0 clears the table).
// A changed table rebuilds every view once; reads pick the dark side while the theme in effect is dark.
export!(pub unsafe fn pxui_ui_set_colors(ui:*mut PxuiUi,light:*const Rgba,dark:*const Rgba,count:usize)->i32 {
    let Some(state)=(unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT };
    if !state.owner || state.shared().in_frame { return crate::PXUI_ERR_STATE; }
    if count>ColorToken::LIMIT || (count!=0 && (light.is_null() || dark.is_null())) { return unsafe { fail(ui,"invalid colour token table") }; }
    let (light,dark)=if count==0 { (&[][..],&[][..]) } else { unsafe { (core::slice::from_raw_parts(light,count),core::slice::from_raw_parts(dark,count)) } };
    unsafe { &mut *state.engine }.set_colors(light,dark);
    0
});

// Colour token `token` for the theme in effect (a `themed` scope's inside one); transparent past the table or on a dead handle.
export!(pub unsafe fn pxui_ui_color(ui:*mut PxuiUi,token:u16)->Rgba { unsafe { crate::state::engine(ui) }.map_or(Rgba::default(),|e| e.color(token)) });

impl PxuiTheme {
    /// Applies `text` (a theme file) over the theme, or records why it cannot be and changes nothing.
    /// With `ui`, `KIND.body = NAME` names the bodies that `Ui` registered; without, only `default`.
    fn apply(&mut self, text: &str, ui: Option<&Ui>) -> i32 {
        let seen = core::cell::RefCell::new(Vec::new());
        let named = |kind: &str, name: &str| {
            seen.borrow_mut().push((kind.to_owned(), name.to_owned()));
            ui.map_or((name == "default").then_some(pxui_types::theme::BodyId::DEFAULT), |ui| ui.named_body(kind, name))
        };
        match Theme::parse_bodies(text, &self.theme, &named) {
            Ok(theme) => {
                self.theme = theme;
                self.error.clear();
                for (kind, name) in seen.into_inner() {
                    let Some(&kind) = pxui_types::theme::Bodies::KINDS.iter().find(|k| **k == kind) else { continue };
                    self.names.retain(|(k, _)| *k != kind);
                    if name != "default" { self.names.push((kind, name)); }
                }
                0
            }
            Err(errors) => {
                self.error = errors.iter().map(|e| format!("line {}: {}", e.line, e.message)).collect::<Vec<_>>().join("\n");
                PXUI_ERR_ARGUMENT
            }
        }
    }
}

impl PxuiTheme {
    /// The theme with each body it names by name resolved in `ui` (the built-in body where `ui` registered no such name): ids are per
    /// `Ui`, so one applied against another `Ui` never selects a body of `ui` by that other `Ui`'s number. Allocates nothing.
    pub(crate) fn in_ui(&self, ui: &Ui) -> Theme {
        let mut theme = self.theme;
        for (kind, name) in &self.names {
            theme.body.set(kind, ui.named_body(kind, name).unwrap_or_default());
        }
        theme
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
    Box::into_raw(Box::new(PxuiTheme { theme, names: Vec::new(), error: String::new(), poisoned: false }))
});

export!(bare pub unsafe fn pxui_theme_destroy(theme: *mut PxuiTheme) { if !theme.is_null() { drop(unsafe { Box::from_raw(theme) }); } });

// Sets one theme file key (`button.hover.fill`, `tokens.accent`, ...) to its value text.
export!(object pub unsafe fn pxui_theme_set(theme: *mut PxuiTheme, key: Str, value: Str) -> i32 {
    let Some(theme) = (unsafe { theme.as_mut() }) else { return PXUI_ERR_ARGUMENT; };
    let (Some(key), Some(value)) = (unsafe { crate::state::checked_key(key) }, unsafe { checked(value) }) else { theme.error = "invalid theme text".into(); return PXUI_ERR_ARGUMENT; };
    if key.contains(['\n', '=']) || value.contains('\n') { theme.error = "a key holds no `=` or line break and a value no line break".into(); return PXUI_ERR_ARGUMENT; }
    theme.apply(&format!("{key} = {value}"), None)
});

// Applies the theme file at `path`.
export!(object pub unsafe fn pxui_theme_load(theme: *mut PxuiTheme, path: Str) -> i32 {
    let Some(theme) = (unsafe { theme.as_mut() }) else { return PXUI_ERR_ARGUMENT; };
    let Some(path) = (unsafe { checked(path) }) else { theme.error = "invalid theme path".into(); return PXUI_ERR_ARGUMENT; };
    match std::fs::read_to_string(path) {
        Ok(text) => theme.apply(&text, None),
        Err(error) => { theme.error = format!("{path}: {error}"); PXUI_ERR_ARGUMENT }
    }
});

// Why the last `set` or `load` was refused, borrowed from the theme until its next call.
export!(bare pub unsafe fn pxui_theme_last_error(theme: *const PxuiTheme) -> Str {
    match unsafe { theme.as_ref() } { Some(theme) => Str { ptr: theme.error.as_ptr(), len: theme.error.len() }, None => Str::default() }
});

// Builds a subtree under `theme`; the Ui's own theme is back when the call returns.
export!(build pub unsafe fn pxui_ui_themed(ui: *mut PxuiUi, theme: *const PxuiTheme, build: Build, user: *mut c_void) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(theme) = (unsafe { theme.as_ref() }) else { return unsafe { fail(ui, "null theme") }; };
    let shared = state.shared;
    let engine = unsafe { &mut *state.engine };
    let resolved = theme.in_ui(engine);
    engine.themed(&resolved, &mut |engine: &mut Ui| call_build(shared, engine, build, user));
    0
});

// Applies `text` (a theme file, any number of lines) over the theme as `pxui_theme_set` does, with `KIND.body = NAME` naming the bodies
// `ui` registered (`pxui_body_register`); an unregistered name is refused like any bad key, with the line in `pxui_theme_last_error`.
export!(pub unsafe fn pxui_ui_theme_apply(ui: *mut PxuiUi, theme: *mut PxuiTheme, text: Str) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(theme) = (unsafe { theme.as_mut() }) else { return unsafe { fail(ui, "null theme") }; };
    let Some(text) = (unsafe { checked(text) }) else { theme.error = "invalid theme text".into(); return PXUI_ERR_ARGUMENT; };
    theme.apply(text, Some(unsafe { &*state.engine }))
});

#[cfg(test)]
#[test]
fn set_colors_validates_the_whole_table_before_installing() {
    let c = |v: u8| Rgba { r: v, g: v, b: v, a: 255 };
    let (light, dark) = ([c(1); 65], [c(2); 65]);
    assert_eq!(unsafe { pxui_ui_set_colors(core::ptr::null_mut(), light.as_ptr(), dark.as_ptr(), 1) }, PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe { pxui_ui_color(core::ptr::null_mut(), 0) }, Rgba::default());
    let ui = unsafe { crate::pxui_ui_create(&crate::PxuiUiDesc::default()) };
    assert_eq!(unsafe { pxui_ui_set_colors(ui, light.as_ptr(), dark.as_ptr(), 64) }, 0);
    let side = |token| unsafe { pxui_ui_color(ui, token) };
    let expected = if pxui_types::theme::dark(&unsafe { &*get(ui).unwrap().engine }.theme().tokens) { c(2) } else { c(1) };
    assert_eq!((side(0), side(63), side(64), side(u16::MAX)), (expected, expected, Rgba::default(), Rgba::default()));
    for (l, d, n) in [(light.as_ptr(), dark.as_ptr(), 65), (core::ptr::null(), dark.as_ptr(), 1), (light.as_ptr(), core::ptr::null(), 1)] {
        assert_ne!(unsafe { pxui_ui_set_colors(ui, l, d, n) }, 0);
        assert_eq!(side(0), expected, "a refused table leaves the installed one");
    }
    assert_eq!(unsafe { pxui_ui_set_colors(ui, core::ptr::null(), core::ptr::null(), 0) }, 0);
    assert_eq!(side(0), Rgba::default(), "an empty table clears every token");
    assert_eq!(unsafe { crate::pxui_ui_destroy(ui) }, 0);
}
