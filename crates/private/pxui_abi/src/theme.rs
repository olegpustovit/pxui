use crate::state::{Build, PxuiUi, call_build, checked, fail, get};
use crate::{PXUI_ERR_ARGUMENT, Str};
use core::ffi::c_void;
use pxui_core::{Theme, ui::Ui};
use pxui_types::{ffi::ThemeTokens, geom::Rgba, theme::{Bodies, BodyId, ColorToken, ThemeSnapshot}};

// The engine's theme is the snapshot's bytes: both are generated from `theme_widgets!` with C layout.
const _: () = assert!(core::mem::size_of::<Theme>() == core::mem::size_of::<ThemeSnapshot>() && core::mem::align_of::<Theme>() == core::mem::align_of::<ThemeSnapshot>()
    && core::mem::offset_of!(Theme, tokens) == core::mem::offset_of!(ThemeSnapshot, tokens) && core::mem::offset_of!(Theme, ring) == core::mem::offset_of!(ThemeSnapshot, ring)
    && core::mem::offset_of!(Theme, image) == core::mem::offset_of!(ThemeSnapshot, image));

// The theme in effect, whole, lent in the 1.x layout (`include/pxui.h`: integer token text sizes): valid until the `Ui` next
// changes it (a theme swap, a `themed` scope) or is freed. The engine's own snapshot keeps fractional sizes (`pxui_ui_theme_exact`);
// this projection of it is copied for C callers only, here, into storage the `Ui` holds inline (no allocation, one address for the
// `Ui`'s life), never on a frame's path or by the Rust wrapper.
export!(pub unsafe fn pxui_ui_theme(ui:*mut PxuiUi) -> *const ThemeSnapshot {
    let Some(state) = (unsafe { get(ui) }) else { return core::ptr::null(); };
    let exact = unsafe { crate::trusted::pxui_ui_theme(ui) };
    if exact.is_null() { return core::ptr::null(); }
    let legacy = state.shared().legacy_theme.insert(unsafe { *exact });
    legacy_tokens(&mut legacy.tokens);
    legacy
});

// ABI 1.78: the theme in effect as the engine keeps it, lent: `Tokens` text sizes are `float` in each 8-byte slot of the 1.x layout
// (the slot's other half 0), fonts carry `size_points`. Valid as `pxui_ui_theme`'s.
export!(trusted pub unsafe fn pxui_ui_theme_exact(ui:*mut PxuiUi) -> *const ThemeSnapshot {
    if unsafe{get(ui)}.is_none() { return core::ptr::null(); }
    unsafe{crate::trusted::pxui_ui_theme(ui)}
});

/// Writes each token text size as the 1.x C snapshot has it, a whole-point `int64_t` rounded half away from zero, over its 8-byte slot.
fn legacy_tokens(tokens: &mut pxui_types::theme::Tokens) {
    let sizes = [tokens.display_size, tokens.title_size, tokens.font_size, tokens.label_size];
    let base = (tokens as *mut pxui_types::theme::Tokens).cast::<u8>();
    for (offset, size) in [core::mem::offset_of!(pxui_types::theme::Tokens, display_size), core::mem::offset_of!(pxui_types::theme::Tokens, title_size),
        core::mem::offset_of!(pxui_types::theme::Tokens, font_size), core::mem::offset_of!(pxui_types::theme::Tokens, label_size)].into_iter().zip(sizes) {
        // the slots are 8-byte aligned (`Tokens` is `align(8)`; checked by the layout rows) and lie inside `tokens`
        unsafe { base.add(offset).cast::<i64>().write(size.round() as i64) };
    }
}

/// A theme a caller builds from tokens and key/value pairs, then scopes over a subtree with `pxui_ui_themed`. Each `set`
/// or `load` applies over what is there already, as `Theme::parse` does over its base: a `tokens.*` key re-derives every
/// look from the new tokens, so set tokens before the keys that restyle single looks. Body ids are per `Ui`, so the theme also keeps
/// the name each `KIND.body` line selected (`names`, one per kind at most) and resolves them in the `Ui` it is used in (`in_ui`).
pub struct PxuiTheme {
    theme: Theme,
    names: Vec<(&'static str, String)>,
    error: String,
    names_tokens: bool,
    strict_bodies: bool,
    palette: Option<[Rgba; ColorToken::LIMIT]>,
    pub(crate) poisoned: bool,
}

// A parsed theme can be selected repeatedly without parsing or allocating on an OS appearance change.
export!(pub unsafe fn pxui_ui_use_theme(ui:*mut PxuiUi,theme:*const PxuiTheme)->i32 {
    let Some(state)=(unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT };
    if !state.owner || state.shared().in_frame { return crate::PXUI_ERR_STATE; }
    let Some(theme)=(unsafe { theme.as_ref() }) else { return unsafe { fail(ui,"null theme") }; };
    let engine = unsafe { &mut *state.engine };
    let bodies = match theme.bodies_in_ui(engine, theme.strict_bodies) {
        Ok(bodies) => bodies,
        Err((kind, name)) => return unsafe { fail(ui, &format!("no {kind} body is registered as `{name}`")) },
    };
    if bodies == theme.theme.body {
        engine.set_theme_with_palette(&theme.theme, theme.names_tokens, theme.palette.as_ref());
    } else {
        use_remapped_theme(engine, theme, bodies);
    }
    0
});

/// Materialise a theme only when the destination UI uses other body IDs.
#[inline(never)]
fn use_remapped_theme(engine: &mut Ui, theme: &PxuiTheme, bodies: Bodies) {
    engine.set_theme_with_palette(&Theme { body: bodies, ..theme.theme }, theme.names_tokens, theme.palette.as_ref());
}

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
    fn apply(&mut self, text: &str, body: &dyn Fn(&str, &str) -> Option<BodyId>) -> i32 {
        let seen = core::cell::RefCell::new(Vec::new());
        let named = |kind: &str, name: &str| {
            seen.borrow_mut().push((kind.to_owned(), name.to_owned()));
            body(kind, name)
        };
        match Theme::parse_bodies(text, &self.theme, &named) {
            Ok(theme) => {
                self.names_tokens = theme.names_color_tokens();
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
    /// Body ids belong to the destination Ui. Scopes and legacy root installs allow a built-in fallback.
    fn bodies_in_ui(&self, ui: &Ui, strict: bool) -> Result<Bodies, (&'static str, &str)> {
        let mut bodies = self.theme.body;
        for (kind, name) in &self.names {
            let id = match ui.named_body(kind, name) {
                Some(id) => id,
                None if strict => return Err((kind, name)),
                None => BodyId::DEFAULT,
            };
            bodies.set(kind, id);
        }
        Ok(bodies)
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
    Box::into_raw(Box::new(PxuiTheme {
        names_tokens: theme.names_color_tokens(), theme, names: Vec::new(), error: String::new(),
        strict_bodies: false, palette: None, poisoned: false,
    }))
});

export!(bare pub unsafe fn pxui_theme_destroy(theme: *mut PxuiTheme) { if !theme.is_null() { drop(unsafe { Box::from_raw(theme) }); } });

// Sets one theme file key (`button.hover.fill`, `tokens.accent`, ...) to its value text.
export!(object pub unsafe fn pxui_theme_set(theme: *mut PxuiTheme, key: Str, value: Str) -> i32 {
    let Some(theme) = (unsafe { theme.as_mut() }) else { return PXUI_ERR_ARGUMENT; };
    let (Some(key), Some(value)) = (unsafe { crate::state::checked_key(key) }, unsafe { checked(value) }) else { theme.error = "invalid theme text".into(); return PXUI_ERR_ARGUMENT; };
    if key.contains(['\n', '=']) || value.contains('\n') { theme.error = "a key holds no `=` or line break and a value no line break".into(); return PXUI_ERR_ARGUMENT; }
    theme.apply(&format!("{key} = {value}"), &|_, _| None)
});

// Applies the theme file at `path`.
export!(object pub unsafe fn pxui_theme_load(theme: *mut PxuiTheme, path: Str) -> i32 {
    let Some(theme) = (unsafe { theme.as_mut() }) else { return PXUI_ERR_ARGUMENT; };
    let Some(path) = (unsafe { checked(path) }) else { theme.error = "invalid theme path".into(); return PXUI_ERR_ARGUMENT; };
    match std::fs::read_to_string(path) {
        Ok(text) => theme.apply(&text, &|_, _| None),
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
    let body = theme.bodies_in_ui(engine, false).expect("a scope falls back to the built-in body");
    let resolved = Theme { body, ..theme.theme };
    engine.themed_with_palette(&resolved, theme.palette.as_ref(), &mut |engine: &mut Ui| call_build(shared, engine, build, user));
    0
});

// Applies `text` (a theme file, any number of lines) over the theme as `pxui_theme_set` does, with `KIND.body = NAME` naming the bodies
// `ui` registered (`pxui_body_register`); an unregistered name is refused like any bad key, with the line in `pxui_theme_last_error`.
export!(pub unsafe fn pxui_ui_theme_apply(ui: *mut PxuiUi, theme: *mut PxuiTheme, text: Str) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(theme) = (unsafe { theme.as_mut() }) else { return unsafe { fail(ui, "null theme") }; };
    let Some(text) = (unsafe { checked(text) }) else { theme.error = "invalid theme text".into(); return PXUI_ERR_ARGUMENT; };
    let engine = unsafe { &*state.engine };
    theme.apply(text, &|kind, name| engine.named_body(kind, name))
});

// Parses a detached theme source. Named bodies are validated when applying the root theme to a Ui.
export!(object pub unsafe fn pxui_theme_parse(theme: *mut PxuiTheme, source: Str) -> i32 {
    let Some(theme) = (unsafe { theme.as_mut() }) else { return PXUI_ERR_ARGUMENT; };
    let Some(source) = (unsafe { crate::state::checked_text(source) }) else { theme.error = "invalid theme text".into(); return PXUI_ERR_ARGUMENT; };
    let status = theme.apply(source, &|_, name| crate::bodies::name_ok(name).then_some(BodyId::DEFAULT));
    if status == 0 { theme.strict_bodies = true; }
    status
});

export!(object pub unsafe fn pxui_theme_set_color(theme: *mut PxuiTheme, token: u16, color: Rgba) -> i32 {
    let Some(theme) = (unsafe { theme.as_mut() }) else { return PXUI_ERR_ARGUMENT; };
    if token as usize >= ColorToken::LIMIT { return PXUI_ERR_ARGUMENT; }
    let palette = theme.palette.get_or_insert([Rgba::default(); ColorToken::LIMIT]);
    palette[token as usize] = if color.as_token().is_none() { color } else { Rgba::default() };
    0
});

export!(object pub unsafe fn pxui_theme_density(theme: *mut PxuiTheme, density: f32) -> i32 {
    let Some(theme) = (unsafe { theme.as_mut() }) else { return PXUI_ERR_ARGUMENT; };
    if !density.is_finite() || density <= 0.0 { return PXUI_ERR_ARGUMENT; }
    theme.theme.tokens.density = density;
    0
});

export!(object pub unsafe fn pxui_theme_reduced_motion(theme: *mut PxuiTheme, on: u8) -> i32 {
    let Some(theme) = (unsafe { theme.as_mut() }) else { return PXUI_ERR_ARGUMENT; };
    if on > 1 { return PXUI_ERR_ARGUMENT; }
    theme.theme.tokens.reduced_motion = on != 0;
    0
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
    let expected = if (unsafe { &*get(ui).unwrap().engine }.theme().tokens).is_dark() { c(2) } else { c(1) };
    let accent = unsafe { &*get(ui).unwrap().engine }.theme().tokens.accent;
    assert_eq!((side(0), side(63), side(64), side(65), side(u16::MAX)), (expected, expected, accent, Rgba::default(), Rgba::default()), "64 is the accent (U2b.7)");
    for (l, d, n) in [(light.as_ptr(), dark.as_ptr(), 65), (core::ptr::null(), dark.as_ptr(), 1), (light.as_ptr(), core::ptr::null(), 1)] {
        assert_ne!(unsafe { pxui_ui_set_colors(ui, l, d, n) }, 0);
        assert_eq!(side(0), expected, "a refused table leaves the installed one");
    }
    assert_eq!(unsafe { pxui_ui_set_colors(ui, core::ptr::null(), core::ptr::null(), 0) }, 0);
    assert_eq!(side(0), Rgba::default(), "an empty table clears every token");
    assert_eq!(unsafe { crate::pxui_ui_destroy(ui) }, 0);
}

#[cfg(test)]
#[test]
fn editing_a_parsed_theme_updates_its_colour_token_scan() {
    use crate::state::pair;
    let ui = unsafe { crate::pxui_ui_create(&crate::PxuiUiDesc::default()) };
    let theme = unsafe { pxui_theme_create(core::ptr::null()) };
    let color = Rgba::rgb(30, 60, 90);
    assert_eq!(unsafe { pxui_ui_set_colors(ui, &color, &color, 1) }, 0);
    for (setting, expected) in [("#544b0000", color), ("#010203", Rgba::rgb(1, 2, 3))] {
        assert_eq!(unsafe { pxui_theme_set(theme, pair("button.idle.fill"), pair(setting)) }, 0);
        assert_eq!(unsafe { pxui_ui_use_theme(ui, theme) }, 0);
        let engine = unsafe { &*get(ui).unwrap().engine };
        assert_eq!(engine.theme().button.idle.fill, expected);
    }
    unsafe { pxui_theme_destroy(theme) };
    assert_eq!(unsafe { crate::pxui_ui_destroy(ui) }, 0);
}

#[cfg(test)]
#[test]
fn legacy_theme_edit_and_load_keep_their_error_detail() {
    use crate::state::pair;
    let theme = unsafe { pxui_theme_create(core::ptr::null()) };
    let message = || unsafe { crate::state::text(pxui_theme_last_error(theme)).unwrap() };
    assert_eq!(unsafe { pxui_theme_set(theme, pair("button.nonsense"), pair("1")) }, PXUI_ERR_ARGUMENT);
    assert!(message().contains("unknown key"), "{}", message());
    assert_eq!(unsafe { pxui_theme_set(theme, pair("tokens.accent"), pair("red")) }, PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe { pxui_theme_load(theme, pair("/no/such/file.theme")) }, PXUI_ERR_ARGUMENT);
    unsafe { pxui_theme_destroy(theme) };
}

#[cfg(test)]
mod legacy_tests {
    use pxui_types::theme::{ThemeSnapshot, Tokens};

    /// The 1.x C snapshot holds each token text size as a whole-point `int64_t` in the slot where the engine's snapshot keeps an `f32`
    /// (its other half 0); the rest of the snapshot is the same bytes, and the 1.x getters report the body size rounded.
    #[test]
    fn the_c_snapshot_is_the_integer_projection_of_the_exact_one() {
        let ui = unsafe { crate::pxui_ui_create(&crate::PxuiUiDesc::default()) };
        assert_eq!(unsafe { crate::ui_state::pxui_ui_set_font_points(ui, 14.5) }, 0);
        let (exact, legacy): (&ThemeSnapshot, &ThemeSnapshot) = unsafe { (&*super::pxui_ui_theme_exact(ui), &*super::pxui_ui_theme(ui)) };
        let slot = |t: &Tokens, at: usize| unsafe { (t as *const Tokens).cast::<u8>().add(at).cast::<i64>().read() };
        for (at, size) in [(core::mem::offset_of!(Tokens, display_size), exact.tokens.display_size), (core::mem::offset_of!(Tokens, font_size), exact.tokens.font_size),
            (core::mem::offset_of!(Tokens, title_size), exact.tokens.title_size), (core::mem::offset_of!(Tokens, label_size), exact.tokens.label_size)] {
            assert_eq!(slot(&legacy.tokens, at), size.round() as i64);
            assert_eq!(slot(&exact.tokens, at), size.to_bits() as i64, "an exact slot is the f32's bits, its other half 0");
        }
        assert!(exact.button == legacy.button && exact.row_h == legacy.row_h && exact.tokens.accent == legacy.tokens.accent);
        let mut points = 0.0;
        assert_eq!(unsafe { crate::ui_state::pxui_ui_font_points(ui, &mut points) }, 0);
        assert_eq!((points, unsafe { crate::ui_state::pxui_ui_font_size(ui) }), (14.5, 15));
        unsafe { crate::pxui_ui_destroy(ui) };
    }

    /// The C projection: copied into storage the `Ui` holds, so a call allocates nothing; lent at one address that keeps the 1.x
    /// contract (still the theme in effect until the `Ui` changes it); and only `pxui_ui_theme` builds it, never the exact query the
    /// Rust wrapper uses or a frame.
    #[test]
    fn the_c_projection_allocates_nothing_keeps_its_lifetime_and_is_never_built_for_rust() {
        let ui = unsafe { crate::pxui_ui_create(&crate::PxuiUiDesc::default()) };
        let built = || unsafe { crate::state::get(ui) }.unwrap().shared().legacy_theme.is_some();
        assert!(!unsafe { super::pxui_ui_theme_exact(ui) }.is_null());
        let frame = |ui| { let mut out = pxui_types::FrameOut::default(); unsafe { crate::trusted::frame(ui, &pxui_types::FrameIn::default(), &mut |_| {}, &mut out) } };
        assert_eq!(frame(ui), 0);
        assert!(!built(), "the exact query and a frame never build the C projection");
        mem_sys::local_reset();
        let first = unsafe { super::pxui_ui_theme(ui) };
        let font = unsafe { (*first).tokens.font_size };
        let again = unsafe { super::pxui_ui_theme(ui) };
        assert_eq!(mem_sys::local_alloc_count(), 0, "the projection is copied without allocating");
        assert!(built() && core::ptr::eq(first, again), "one address for the Ui's life");
        assert_eq!(frame(ui), 0);
        assert!(!unsafe { super::pxui_ui_theme_exact(ui) }.is_null());
        assert_eq!(unsafe { (*first).tokens.font_size }, font, "still the theme in effect: valid until the Ui changes it");
        unsafe { crate::pxui_ui_destroy(ui) };
    }
}
