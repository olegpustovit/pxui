//! The `Ui` lifecycle, the two frame entry points, theme and platform calls, and the first widgets.
use crate::state::{Build, PxuiUi, Shared, call_build, checked, core_box, fail, fail_code, get, text};
use crate::{PXUI_ERR_ARGUMENT, PXUI_ERR_PANIC, PXUI_ERR_STATE, FrameIn, FrameOut, PxuiBox, PxuiPanel, PxuiUiDesc, Str};
use core::ffi::c_void;
use input::{Event, Key, MouseButton, Platform, ScrollPhase, frame};
use pxui_core::ui::{PanelOpts, Ui};
use pxui_types::{KEY_SPECIAL, KEY_UNKNOWN};
use pxui_types::ffi::{A11yNode, A11yTree, Damage, Input, Output, PlatformOut, Slice, ThemeSource, ThemeTokens};

fn platform() -> Platform {
    #[cfg(target_os = "macos")] { Platform::Apple }
    #[cfg(not(target_os = "macos"))] { Platform::Other }
}

/// The special keys in `pxui.h` order: key code `KEY_SPECIAL + n` is `SPECIAL_KEYS[n]`.
const SPECIAL_KEYS: [Key; 31] = [
    Key::ArrowUp, Key::ArrowDown, Key::ArrowLeft, Key::ArrowRight, Key::Space, Key::Enter, Key::Escape, Key::Backspace,
    Key::Tab, Key::Shift, Key::Ctrl, Key::Alt, Key::Meta,
    Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6, Key::F7, Key::F8, Key::F9, Key::F10, Key::F11, Key::F12,
    Key::Home, Key::End, Key::PageUp, Key::PageDown, Key::Insert, Key::Delete,
];

pub(crate) fn key(code: u32) -> Option<Key> {
    if code >= KEY_UNKNOWN { return Some(Key::Unknown(i64::from(code - KEY_UNKNOWN))); }
    if code >= KEY_SPECIAL { return SPECIAL_KEYS.get((code - KEY_SPECIAL) as usize).copied(); }
    char::from_u32(code).filter(|c| *c != '\0').map(Key::KeyChar)
}

pub(crate) fn key_code(key: Key) -> u32 {
    match key {
        Key::KeyChar(c) => c as u32,
        // ponytail: platform codes outside 0..2^31 are truncated; widen the event if one ever matters
        Key::Unknown(code) => KEY_UNKNOWN | (code as u32 & !KEY_UNKNOWN),
        special => SPECIAL_KEYS.iter().position(|k| *k == special).map_or(KEY_UNKNOWN, |n| KEY_SPECIAL + n as u32),
    }
}

pub(crate) fn button(code: u32) -> Option<MouseButton> {
    Some(match code { 0 => MouseButton::LeftButton, 1 => MouseButton::RightButton, 2 => MouseButton::MiddleButton, 3 => MouseButton::MouseX1, 4 => MouseButton::MouseX2, _ => return None })
}

pub(crate) fn button_code(button: MouseButton) -> u32 {
    match button { MouseButton::LeftButton => 0, MouseButton::RightButton => 1, MouseButton::MiddleButton => 2, MouseButton::MouseX1 => 3, MouseButton::MouseX2 => 4 }
}

export!(bare pub unsafe fn pxui_abi_version() -> u32 { crate::ABI_VERSION });

export!(pub unsafe fn pxui_ui_view_size(ui: *mut PxuiUi, out: *mut pxui_types::geom::Point) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null view size output") }; }
    let (x, y) = unsafe { (&*state.engine).view_size() };
    unsafe { *out = pxui_types::geom::Point { x, y }; }
    0
});

export!(bare pub unsafe fn pxui_ui_create(desc: *const PxuiUiDesc) -> *mut PxuiUi {
    if desc.is_null() { return core::ptr::null_mut(); }
    let desc = unsafe { *desc };
    if !valid_desc(&desc) { return core::ptr::null_mut(); }
    unsafe { create_ui(desc, &[pxui_core::text::PRAGMASEVKA]) }
});

export!(bare pub unsafe fn pxui_ui_create_with_fonts(desc: *const PxuiUiDesc, fonts: *const Str, count: usize) -> *mut PxuiUi {
    if desc.is_null() || count == 0 || count > 128 || fonts.is_null() { return core::ptr::null_mut(); }
    let desc = unsafe { *desc };
    if !valid_desc(&desc) { return core::ptr::null_mut(); }
    let mut bytes = Vec::with_capacity(count);
    for font in unsafe { std::slice::from_raw_parts(fonts, count) } {
        if font.len == 0 || font.len > 16 * 1024 * 1024 || font.ptr.is_null() { return core::ptr::null_mut(); }
        bytes.push(unsafe { std::slice::from_raw_parts(font.ptr, font.len) });
    }
    unsafe { create_ui(desc, &bytes) }
});

fn valid_desc(desc: &PxuiUiDesc) -> bool { desc.size >= core::mem::size_of::<PxuiUiDesc>() as u32 && desc.font_size >= 0 && desc.platform <= 2 }

unsafe fn create_ui(desc: PxuiUiDesc, fonts: &[&[u8]]) -> *mut PxuiUi {
    let text = pxui_core::text::Text::new(fonts);
    let platform = match desc.platform { 0 => platform(), 1 => Platform::Apple, _ => Platform::Other };
    let font_size = (desc.font_size != 0).then_some(i64::from(desc.font_size));
    let engine = Box::into_raw(Box::new(Ui::create(platform, font_size, text)));
    let shared = Box::into_raw(Box::new(Shared::new()));
    Box::into_raw(Box::new(PxuiUi { engine, shared, owner: true }))
}

// Frees a `Ui`. Fails (and frees nothing) on a borrowed handle, inside a frame, or while a renderer is attached.
export!(bare pub unsafe fn pxui_ui_destroy(ui: *mut PxuiUi) -> i32 {
    if ui.is_null() { return 0; }
    let state = unsafe { &*ui };
    let shared = state.shared();
    if !state.owner || shared.in_frame { return PXUI_ERR_STATE; }
    if shared.renderers != 0 { return unsafe { fail_code(ui, "a renderer is still attached to this Ui", PXUI_ERR_STATE) }; }
    let ui = unsafe { Box::from_raw(ui) };
    drop(unsafe { Box::from_raw(ui.engine) });
    drop(unsafe { Box::from_raw(ui.shared) });
    0
});

export!(bare pub unsafe fn pxui_last_error(ui: *const PxuiUi) -> Str {
    if ui.is_null() { return Str::default(); }
    let error = unsafe { &*(*ui).shared }.error.as_bytes();
    Str { ptr: error.as_ptr(), len: error.len() }
});

/// Fills `shared.output` from the engine. `nodes` rebuilds the accessibility records too (every frame; between frames
/// only when the tree's revision moved).
pub(crate) fn build_output(shared: &mut Shared, engine: &Ui, nodes: bool) {
    let output = engine.output();
    let tree = output.a11y;
    if nodes || tree.revision != shared.output.a11y.revision || tree.nodes.len() != shared.nodes.len() {
        shared.nodes.clear();
        shared.last_children.clear();
        shared.last_children.resize(tree.nodes.len(), u32::MAX);
        for (index, node) in tree.nodes.iter().enumerate() {
            if node.parent != u32::MAX { shared.last_children[node.parent as usize] = index as u32; }
        }
        for (index, n) in tree.nodes.iter().enumerate() {
            let value = match n.value {
                pxui_core::a11y::Value::None => pxui_types::ffi::A11yValue::default(),
                pxui_core::a11y::Value::Text(r) => pxui_types::ffi::A11yValue { tag: 1, text_start: r.start, text_end: r.end, ..Default::default() },
                pxui_core::a11y::Value::Number { now, min, max, step } => pxui_types::ffi::A11yValue { tag: 2, now, min, max, step, ..Default::default() },
            };
            shared.nodes.push(A11yNode {
                id: n.id, parent: n.parent, role: n.role as u32, name_start: n.name.start, name_end: n.name.end,
                description_start: n.description.start, description_end: n.description.end, value, rect: n.rect, states: n.states,
                actions: n.actions, selection: [n.selection.0, n.selection.1], child_count: n.child_count, first_child: n.first_child,
                next_sibling: n.next_sibling, last_child: shared.last_children[index],
            });
        }
    }
    let platform = output.platform;
    let (cursor, has_cursor) = match platform.cursor {
        Some(pxui_core::ui::Cursor::HorizontalResize) => (1, true), Some(pxui_core::ui::Cursor::VerticalResize) => (2, true),
        Some(pxui_core::ui::Cursor::Text) => (3, true), None => (0, false),
    };
    let (ime_rect, ime_caret, has_ime) = platform.ime.map_or(([0; 4], 0, false), |(r, c)| ([r.0, r.1, r.2, r.3], c, true));
    let (background, has_background) = platform.background.map_or((pxui_types::geom::Rgba::default(), false), |c| (c, true));
    let (clipboard, has_clipboard) = platform.clipboard.as_ref().map_or((Str::default(), false), |s| (Str { ptr: s.as_ptr(), len: s.len() }, true));
    shared.output = Output {
        size: core::mem::size_of::<Output>() as u32, reserved: 0, draw_revision: engine.draw_revision() as u64,
        repaint_at: engine.repaint_at().unwrap_or(f64::INFINITY), built_nodes: engine.built_nodes() as u64,
        rebuilt_views: engine.rebuilt_views() as u64, arranged_nodes: engine.arranged_nodes() as u64,
        platform: PlatformOut {
            size: core::mem::size_of::<PlatformOut>() as u32, cursor, has_cursor: has_cursor as u8, has_ime: has_ime as u8,
            has_background: has_background as u8, has_clipboard: has_clipboard as u8, ime_rect, ime_caret, clipboard, background,
            paste: platform.paste as u8, reserved: [0; 7],
        },
        damage: output.damage.map_or_else(Damage::default, |r| Damage { has_damage: 1, reserved: [0; 3], rect: r }),
        a11y: A11yTree {
            nodes: Slice { ptr: shared.nodes.as_ptr(), len: shared.nodes.len() }, strings: Str { ptr: tree.strings.as_ptr(), len: tree.strings.len() },
            revision: tree.revision, first_child: tree.first_child(u32::MAX), child_count: tree.child_count(u32::MAX), actions: Slice::default(),
        },
    };
}

// The output of the last frame (all zero before the first). Borrowed from the `Ui` and valid until its next frame;
// the counters and the platform record are refreshed on each call, the accessibility records when their tree changed.
export!(bare pub unsafe fn pxui_ui_output(ui: *mut PxuiUi) -> *const Output {
    let Some(state) = (unsafe { get(ui) }) else { return core::ptr::null(); };
    let shared = state.shared();
    if !shared.in_frame { build_output(shared, unsafe { &*state.engine }, false); }
    &shared.output
});

export!(pub unsafe fn pxui_ui_tokens(ui: *mut PxuiUi) -> ThemeTokens {
    let Some(ui) = (unsafe { get(ui) }) else { return unsafe { core::mem::zeroed() }; };
    crate::theme::tokens(unsafe { &*ui.engine }.tokens())
});

export!(pub unsafe fn pxui_ui_set_theme(ui: *mut PxuiUi, source: *const ThemeSource) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if !state.owner || state.shared().in_frame { return PXUI_ERR_STATE; }
    if source.is_null() { return unsafe { fail(ui, "null theme source") }; }
    let source = unsafe { *source };
    if source.size < core::mem::size_of::<ThemeSource>() as u32 || source.flags & !3 != 0 { return unsafe { fail(ui, "invalid theme descriptor") }; }
    let parsed = if source.source.len == 0 { Ok(pxui_core::Theme::default()) } else {
        let Some(text) = (unsafe { text(source.source) }) else { return unsafe { fail(ui, "invalid theme UTF-8") }; };
        pxui_core::Theme::parse(text, &pxui_core::Theme::default()).map_err(|_| ())
    };
    let Ok(mut theme) = parsed else { return unsafe { fail(ui, "theme parse failed") }; };
    theme.tokens.density = if source.flags & ThemeSource::COMPACT != 0 { 0.84 } else { 1.0 };
    theme.tokens.reduced_motion = source.flags & ThemeSource::REDUCED_MOTION != 0;
    unsafe { (&mut *state.engine).set_theme(theme); }
    0
});

export!(pub unsafe fn pxui_ui_set_dir(ui: *mut PxuiUi, dir: u32) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let dir = match dir { 0 => pxui_core::layout::Direction::Ltr, 1 => pxui_core::layout::Direction::Rtl, _ => return unsafe { fail(ui, "invalid direction") } };
    unsafe { (&mut *state.engine).set_dir(dir); }
    0
});

export!(pub unsafe fn pxui_ui_set_a11y(ui: *mut PxuiUi, on: u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if on > 1 { return unsafe { fail(ui, "invalid accessibility flag") }; }
    unsafe { (&mut *state.engine).set_a11y(on != 0); }
    0
});

export!(pub unsafe fn pxui_ui_inject_a11y(ui: *mut PxuiUi, actions: *const pxui_types::ffi::A11yAction, count: usize) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if count > 4096 || (count != 0 && actions.is_null()) { return unsafe { fail(ui, "invalid accessibility action span") }; }
    let mut translated = Vec::with_capacity(count);
    if count != 0 {
        for a in unsafe { std::slice::from_raw_parts(actions, count) } {
            let action = match a.action_code {
                1 => pxui_core::a11y::Action::Press, 2 => pxui_core::a11y::Action::Focus, 4 => pxui_core::a11y::Action::SetValue,
                8 => pxui_core::a11y::Action::Scroll, 16 => pxui_core::a11y::Action::Expand, 32 => pxui_core::a11y::Action::Collapse,
                64 => pxui_core::a11y::Action::Select, _ => return unsafe { fail(ui, "invalid accessibility action code") },
            };
            translated.push(pxui_core::a11y::A11yAction { id: a.id, action, payload: a.payload, end: a.end as f64 });
        }
    }
    unsafe { (&mut *state.engine).inject_a11y(&translated); }
    0
});

export!(pub unsafe fn pxui_ui_patch_text(ui: *mut PxuiUi, box_: *const PxuiBox, value: Str) -> u8 {
    let Some(state) = (unsafe { get(ui) }) else { return 0 };
    if box_.is_null() { return 0; }
    let (Some(value), Some(handle)) = (unsafe { text(value) }, core_box(unsafe { *box_ })) else { return 0 };
    unsafe { (&mut *state.engine).patch_text(&handle, value) as u8 }
});

export!(pub unsafe fn pxui_ui_busy_step(ui: *mut PxuiUi, box_: *const PxuiBox, now: f64) -> u8 {
    let Some(state) = (unsafe { get(ui) }) else { return 0 };
    if box_.is_null() || !now.is_finite() { return 0; }
    let Some(handle) = core_box(unsafe { *box_ }) else { return 0 };
    unsafe { (&mut *state.engine).busy_step(&handle, now) as u8 }
});

export!(pub unsafe fn pxui_ui_request_frame(ui: *mut PxuiUi) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    unsafe { (&mut *state.engine).request_frame(); }
    0
});

pub(crate) fn translate_event(e: &pxui_types::Event) -> Option<Event> {
    use pxui_types::Event as E;
    let s = unsafe { text(e.text) };
    Some(match e.kind {
        E::KEY_PRESSED => Event::KeyPressed(key(e.code)?),
        E::KEY_RELEASED => Event::KeyReleased(key(e.code)?),
        E::POINTER_MOVED => Event::PointerMoved((e.x, e.y)),
        E::POINTER_PRESSED => Event::PointerPressed(button(e.code)?, (e.x, e.y)),
        E::POINTER_RELEASED => Event::PointerReleased(button(e.code)?, (e.x, e.y)),
        E::POINTER_CANCELLED => Event::PointerCancelled(button(e.code)?),
        E::SCROLLED => Event::Scrolled((e.x, e.y)),
        E::TEXT_INPUT => Event::TextInput(s?.to_owned()),
        E::PASTE => Event::Paste(s?.to_owned()),
        E::TEXT_EDITING => Event::TextEditing { text: s?.to_owned(), start: e.x as i64, length: e.y as i64 },
        E::FILE_DROPPED => Event::FileDropped(s?.to_owned()),
        E::FILE_DRAG_MOVED => Event::FileDragMoved((e.x, e.y)),
        E::FILE_DRAG_ENDED => Event::FileDragEnded,
        E::PINCHED => Event::Pinched(e.value),
        E::FILE_DIALOG => match e.code {
            0 => Event::FileDialog { id: e.value as i64, result: Ok(s?.split('\0').map(str::to_owned).collect()) },
            1 => Event::FileDialog { id: e.value as i64, result: Err(s?.to_owned()) },
            _ => return None,
        },
        E::DIALOG_OPENED => Event::DialogOpened(e.code as i64),
        E::RESIZED => Event::Resized((e.x as i64, e.y as i64)),
        E::FOCUS_LOST => Event::FocusLost,
        E::FOCUS_GAINED => Event::FocusGained,
        E::CLOSED => Event::Closed,
        E::PIXEL_SIZE_CHANGED => Event::PixelSizeChanged((e.x as i64, e.y as i64)),
        E::VISIBILITY_CHANGED => Event::VisibilityChanged(e.value != 0.0),
        E::TRACKPAD_SCROLLED => Event::TrackpadScrolled {
            delta: (e.x, e.y), time: e.value,
            phase: match e.code { 0 => ScrollPhase::Touched, 1 => ScrollPhase::Moved, 2 => ScrollPhase::Lifted, 3 => ScrollPhase::Momentum, _ => return None },
        },
        _ => return None,
    })
}

/// Appends `events` to `base`, folding the key and pointer ones into its held state. False when one is invalid.
unsafe fn push_events(base: &mut input::Frame, events: Slice<pxui_types::Event>) -> bool {
    if events.len == 0 { return true; }
    for raw in unsafe { std::slice::from_raw_parts(events.ptr, events.len) } {
        let Some(event) = translate_event(raw) else { return false };
        match &event {
            Event::KeyPressed(k) => if !base.keys.contains(k) { base.keys.push(*k) },
            Event::KeyReleased(k) => base.keys.retain(|held| held != k),
            Event::PointerMoved(p) => base.pointer = *p,
            Event::PointerPressed(b, p) => { base.pointer = *p; if !base.buttons.contains(b) { base.buttons.push(*b) } }
            Event::PointerReleased(b, p) => { base.pointer = *p; base.buttons.retain(|held| held != b) }
            Event::PointerCancelled(b) => base.buttons.retain(|held| held != b),
            _ => {}
        }
        base.events.push(event);
    }
    true
}

/// Runs one frame of `ui` over `frame`. Both frame entry points call it after decoding their input.
unsafe fn run_frame(ui: *mut PxuiUi, frame: &frame::Frame, build: Build, user: *mut c_void, out: *mut FrameOut) -> i32 {
    // The callbacks reach the shared state through `shared`, so no `&mut Shared` is held across the build.
    let (engine, shared) = unsafe { ((*ui).engine, (*ui).shared) };
    unsafe {
        let s = &mut *shared;
        s.error.clear();
        s.returned.clear();
        s.chunks_used = 0;
        s.in_frame = true;
    }
    let output = unsafe { (&mut *engine).frame(frame, &mut |engine: &mut Ui| call_build(shared, engine, build, user)) };
    let (damage, layers, layer_count) = (output.damage, output.draw.as_ptr(), output.draw.len());
    let s = unsafe { &mut *shared };
    s.in_frame = false;
    crate::node::finish_frame(s, unsafe { &*engine });
    let damage_rect = damage.map_or([0.0; 4], |r| [r.x, r.y, r.w, r.h]);
    unsafe { *out = FrameOut { size: core::mem::size_of::<FrameOut>() as u32, damage: damage_rect, has_damage: damage.is_some() as u8, reserved: [0; 3], layer_count: layer_count as u32, layers: layers.cast() }; }
    if core::mem::take(&mut s.poisoned) { PXUI_ERR_PANIC } else { 0 }
}

/// The checks every frame entry point makes before it decodes anything: a live owner handle, not re-entered from a
/// callback; a `Ui` poisoned since its last frame skips this frame (returning the error) and starts clean.
unsafe fn enter_frame(ui: *mut PxuiUi) -> Result<(), i32> {
    let Some(state) = (unsafe { get(ui) }) else { return Err(PXUI_ERR_ARGUMENT) };
    let shared = state.shared();
    if !state.owner || shared.in_frame { return Err(unsafe { fail_code(ui, "a frame cannot start from a frame callback", PXUI_ERR_STATE) }); }
    if core::mem::take(&mut shared.poisoned) { return Err(PXUI_ERR_PANIC); }
    Ok(())
}

export!(frame pub unsafe fn pxui_frame(ui: *mut PxuiUi, input: *const FrameIn, build: Build, user: *mut c_void, out: *mut FrameOut) -> i32 {
    if let Err(code) = unsafe { enter_frame(ui) } { return code; }
    if input.is_null() || out.is_null() { return unsafe { fail(ui, "null frame argument") }; }
    let input = unsafe { &*input };
    if input.size < core::mem::size_of::<FrameIn>() as u32 || !input.time.is_finite() || !input.scale.is_finite() || input.scale <= 0.0 || !input.width.is_finite() || !input.height.is_finite() { return unsafe { fail(ui, "invalid frame descriptor") }; }
    if input.event_count > 4096 || (input.event_count != 0 && input.events.is_null()) { return unsafe { fail(ui, "invalid frame event slice") }; }
    let mut base = input::at_time(input.time);
    base.size = (input.width as i64, input.height as i64);
    base.frame = input.time.to_bits() as i64;
    if !unsafe { push_events(&mut base, Slice { ptr: input.events, len: input.event_count }) } { return unsafe { fail(ui, "invalid event kind, code, or UTF-8 payload") }; }
    let frame = frame::Frame { input: base, pixel_scale: (input.scale as f64, input.scale as f64), fps: 60.0, mouse_delta: (0.0, 0.0) };
    unsafe { run_frame(ui, &frame, build, user, out) }
});

export!(frame pub unsafe fn pxui_ui_frame(ui: *mut PxuiUi, input: *const Input, build: Build, user: *mut c_void, out: *mut FrameOut) -> i32 {
    if let Err(code) = unsafe { enter_frame(ui) } { return code; }
    if input.is_null() || out.is_null() { return unsafe { fail(ui, "null frame argument") }; }
    let input = unsafe { *input };
    if input.size < core::mem::size_of::<Input>() as u32 || !input.time.is_finite() || !input.dt.is_finite() || input.dt < 0.0 || !input.fps.is_finite() || input.fps < 0.0 || !input.pointer.iter().all(|v| v.is_finite()) || !input.pixel_scale.iter().all(|v| v.is_finite() && *v > 0.0) || !input.mouse_delta.iter().all(|v| v.is_finite()) { return unsafe { fail(ui, "invalid input frame") }; }
    if input.events.len > 4096 || input.keys.len > 512 || input.buttons.len > 32 || (input.events.len != 0 && input.events.ptr.is_null()) || (input.keys.len != 0 && input.keys.ptr.is_null()) || (input.buttons.len != 0 && input.buttons.ptr.is_null()) { return unsafe { fail(ui, "invalid input spans") }; }
    let mut base = input::at_time(input.time);
    base.dt = input.dt; base.frame = input.frame; base.tick = input.tick;
    base.size = (input.logical_size[0], input.logical_size[1]);
    base.pointer = (input.pointer[0], input.pointer[1]);
    if input.keys.len != 0 { for code in unsafe { std::slice::from_raw_parts(input.keys.ptr, input.keys.len) } { base.keys.push(key(*code).unwrap_or(Key::Unknown(*code as i64))); } }
    if input.buttons.len != 0 {
        for code in unsafe { std::slice::from_raw_parts(input.buttons.ptr, input.buttons.len) } {
            let Some(value) = button(*code) else { return unsafe { fail(ui, "invalid held mouse button") }; };
            base.buttons.push(value);
        }
    }
    if !unsafe { push_events(&mut base, input.events) } { return unsafe { fail(ui, "invalid event kind, code, or UTF-8 payload") }; }
    let frame = frame::Frame { input: base, pixel_scale: (input.pixel_scale[0], input.pixel_scale[1]), fps: input.fps, mouse_delta: (input.mouse_delta[0], input.mouse_delta[1]) };
    unsafe { run_frame(ui, &frame, build, user, out) }
});

export!(pub unsafe fn pxui_within(ui: *mut PxuiUi, box_: *const PxuiBox, build: Build, user: *mut c_void) {
    let (Some(state), false) = (unsafe { get(ui) }, box_.is_null()) else { return; };
    let Some(b) = core_box(unsafe { *box_ }) else { return; };
    let shared = state.shared;
    unsafe { (&mut *state.engine).within(&b, &mut |engine: &mut Ui| call_build(shared, engine, build, user)); }
});

export!(pub unsafe fn pxui_scope(ui: *mut PxuiUi, label: Str, build: Build, user: *mut c_void) {
    let Some(state) = (unsafe { get(ui) }) else { return; };
    let Some(label) = (unsafe { text(label) }) else { return; };
    let shared = state.shared;
    unsafe { (&mut *state.engine).scope(label, &mut |engine: &mut Ui| call_build(shared, engine, build, user)); }
});

export!(pub unsafe fn pxui_col(ui: *mut PxuiUi, padding: f32, label: Str, build: Build, user: *mut c_void) {
    let Some(state) = (unsafe { get(ui) }) else { return; };
    let Some(label) = (unsafe { text(label) }) else { return; };
    let shared = state.shared;
    unsafe { (&mut *state.engine).col(padding, label, &mut |engine: &mut Ui| call_build(shared, engine, build, user)); }
});

export!(pub unsafe fn pxui_panel(ui: *mut PxuiUi, panel: *const PxuiPanel, label: Str, build: Build, user: *mut c_void) {
    let (Some(state), false) = (unsafe { get(ui) }, panel.is_null()) else { return; };
    let Some(label) = (unsafe { text(label) }) else { return; };
    let panel = unsafe { *panel };
    if panel.size < core::mem::size_of::<PxuiPanel>() as u32 || !crate::state::finite(&[panel.x, panel.y, panel.width, panel.height]) { return; }
    let mut opts = PanelOpts::default().at(panel.x, panel.y).width(panel.width).padding(panel.padding).window(panel.window != 0);
    if panel.has_height != 0 { opts = opts.height(panel.height); }
    let shared = state.shared;
    unsafe { (&mut *state.engine).panel(opts, label, &mut |engine: &mut Ui| call_build(shared, engine, build, user)); }
});

export!(pub unsafe fn pxui_label(ui: *mut PxuiUi, label: Str) {
    let Some(state) = (unsafe { get(ui) }) else { return; };
    if let Some(label) = unsafe { text(label) } { unsafe { (&mut *state.engine).label(label); } }
});

export!(pub unsafe fn pxui_button(ui: *mut PxuiUi, label: Str) -> u8 {
    let Some(state) = (unsafe { get(ui) }) else { return 0; };
    unsafe { text(label) }.map_or(0, |label| unsafe { (&mut *state.engine).button(label) as u8 })
});

export!(pub unsafe fn pxui_toggle(ui: *mut PxuiUi, label: Str, on: u8) -> u8 {
    let Some(state) = (unsafe { get(ui) }) else { return on; };
    unsafe { text(label) }.map_or(on, |label| unsafe { (&mut *state.engine).toggle(label, on != 0) as u8 })
});

export!(pub unsafe fn pxui_text_field(ui: *mut PxuiUi, label: Str, value: Str, changed: *mut Str) -> u8 {
    let Some(state) = (unsafe { get(ui) }) else { return 0; };
    if changed.is_null() { return 0; }
    let (Some(label), Some(value)) = (unsafe { checked(label) }, unsafe { checked(value) }) else { return 0; };
    let Some(value) = (unsafe { (&mut *state.engine).text_field(label, value) }) else { return 0; };
    unsafe { *changed = state.give(value); }
    1
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_codes_round_trip_and_never_collide() {
        let mut keys = vec![Key::KeyChar('a'), Key::KeyChar('\u{100}'), Key::KeyChar('\u{10ffff}'), Key::Unknown(7), Key::Unknown(0x7fff_ffff)];
        keys.extend(SPECIAL_KEYS);
        let mut codes = std::collections::HashSet::new();
        for k in keys {
            let code = key_code(k);
            assert_eq!(key(code), Some(k), "{k:?} -> {code:#x}");
            assert!(codes.insert(code), "{k:?} collides");
        }
        assert_eq!(key(0), None);
        assert_eq!(key(0xd800), None);
        assert_eq!(key(KEY_SPECIAL + SPECIAL_KEYS.len() as u32), None);
        assert_eq!(key(key_code(Key::KeyChar('\u{100}'))), Some(Key::KeyChar('\u{100}')));
        assert_ne!(key_code(Key::KeyChar('\u{100}')), key_code(Key::ArrowUp));
    }
}
