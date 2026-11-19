//! Frame descriptor extensions preserve frozen prefixes and validate partial tails/larger bounds
//! before mutation. Match static/dynamic behavior. Root theme replacement happens between frames
//! with owner/callback guards. Output dirty-span copying reserves from engine dirty capacities so
//! new fragmentation of warmed instances does not allocate.
//!
//! The `Ui` lifecycle, the two frame entry points, theme and platform calls, and the first widgets.
use crate::trusted;
use crate::state::{Build, Inner, PxuiUi, Shared, boxed, checked, fail, fail_code, get, pair, text};
use crate::{PXUI_ERR_ARGUMENT, PXUI_ERR_PANIC, PXUI_ERR_STATE, FrameIn, FrameOut, PxuiBox, PxuiPanel, PxuiUiDesc, Str};
use core::ffi::c_void;
use input::{Event, Key, MouseButton, Platform, frame};
use pxui_core::ui::Ui;
use pxui_types::ffi::{A11yNode, A11yTree, Damage, Input, Output, PlatformOut, Slice, ThemeSource};

fn platform() -> Platform {
    #[cfg(any(target_os = "macos", target_os = "ios"))] { Platform::Apple }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))] { Platform::Other }
}

pub(crate) fn key(code: u32) -> Option<Key> { Key::from_code(code) }
pub(crate) fn key_code(key: Key) -> u32 { key.code() }
pub(crate) fn button(code: u32) -> Option<MouseButton> { MouseButton::from_code(code) }
pub(crate) fn button_code(button: MouseButton) -> u32 { button.code() }

export!(bare pub unsafe fn pxui_abi_version() -> u32 { crate::ABI_VERSION });

// Process counters: 0 = calls into the library (this one included); 1.. = the library's own allocator counters, numbered as
// `mem_sys::COUNT_*` (allocations, bytes, live bytes, this thread's allocations and peak, reset of this thread's, 7 = 1 when it counts).
// They count only in a test/performance libpxui (`pxui_dylib/counting`); a production library leaves the process allocator alone
// and reads 0. The app registers this export with `mem_sys::set_peer`, so its budgets and ledgers count the whole process.
// Anything else reads 0, and so does 0 since 1.66: calls are counted per `Ui` (`pxui_ui_calls`), never across the process.
export!(bare pub unsafe fn pxui_counter(which: u32) -> u64 {
    match which { 0 => 0, _ => mem_sys::own(which) }
});

// Calls into the library made on `ui` (its owner and every handle a callback borrowed from it, its renderers and paint handles)
// since it was created; this call is not one of them. A plain field of that `Ui`: no counter is shared between `Ui`s or threads.
// 0 for null.
export!(bare pub unsafe fn pxui_ui_calls(ui: *const PxuiUi) -> u64 {
    if ui.is_null() { return 0; }
    unsafe { (*(*ui).shared).calls }
});

export!(pub unsafe fn pxui_ui_view_size(ui: *mut PxuiUi, out: *mut pxui_types::geom::Point) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null view size output") }; }
    let (x, y) = unsafe { (&*state.engine).view_size() };
    unsafe { *out = pxui_types::geom::Point { x, y }; }
    0
});

export!(bare pub unsafe fn pxui_ui_create(desc: *const PxuiUiDesc) -> *mut PxuiUi {
    if desc.is_null() { return core::ptr::null_mut(); }
    let Some((desc, _)) = read_desc!(desc, PxuiUiDesc, [12]) else { return core::ptr::null_mut(); };
    if !valid_desc(&desc) { return core::ptr::null_mut(); }
    unsafe { create_ui(desc, &[pxui_core::text::PRAGMASEVKA]) }
});

export!(bare pub unsafe fn pxui_ui_create_with_fonts(desc: *const PxuiUiDesc, fonts: *const Str, count: usize) -> *mut PxuiUi {
    if desc.is_null() || count == 0 || count > pxui_core::text::MAX_FONTS || fonts.is_null() { return core::ptr::null_mut(); }
    let Some((desc, _)) = read_desc!(desc, PxuiUiDesc, [12]) else { return core::ptr::null_mut(); };
    if !valid_desc(&desc) { return core::ptr::null_mut(); }
    let mut bytes = Vec::with_capacity(count);
    let mut total = 0usize;
    for font in unsafe { std::slice::from_raw_parts(fonts, count) } {
        total = total.saturating_add(font.len);
        if font.len == 0 || font.len > pxui_core::text::MAX_FONT_BYTES || total > pxui_core::text::MAX_FONT_TOTAL_BYTES || font.ptr.is_null() { return core::ptr::null_mut(); }
        bytes.push(unsafe { std::slice::from_raw_parts(font.ptr, font.len) });
    }
    unsafe { create_ui(desc, &bytes) }
});

/// A `PxuiUiDesc` is read at its 12 bytes (before `abi_built`) or whole. The library's own major is the only one it serves: `abi_built` 0 is unstated, any
/// other value must carry that major and a minor no newer than the library's own (the minimum-library rule: an app built for a newer minor may fill or
/// read records grown past what this library knows, so it is refused at create, never misread later).
fn valid_desc(desc: &PxuiUiDesc) -> bool {
    desc.font_size >= 0 && desc.platform <= 2 && (desc.abi_built == 0 || (desc.abi_built >> 16 == crate::ABI_VERSION >> 16 && desc.abi_built & 0xFFFF <= crate::ABI_VERSION & 0xFFFF))
}

// 1 when the library, as built, has the feature `id` (`PXUI_FEATURE_*`: ids are never reused); 0 for an id it does not know.
export!(bare pub unsafe fn pxui_supports(id: u32) -> u32 {
    use pxui_types::feature as f;
    (match id {
        f::A11Y_BRIDGE => cfg!(target_os = "linux"),
        f::METAL => cfg!(any(target_os = "macos", target_os = "ios")),
        f::VULKAN => cfg!(any(windows, target_os = "linux")),
        f::EXTENSION_SET | f::BUTTON_CONTENT | f::ICON_BUTTON_CURVES | f::TOGGLE_RESPONSE | f::SEGMENTED_RESPONSE | f::DIALOG_RESPONSE | f::WIDGET_VARIANTS
            | f::LABEL_NODE | f::MENU_ITEM_IDS | f::FRAME_SURFACE | f::ITEM_TEXT | f::COLOR_TOKENS | f::SWIPE_ACTIONS | f::DISMISS
            | f::LOOK_TOKENS | f::ICON_WORD | f::TEXT_WORKER | f::TEXT_FIELD_KEYED | f::LABEL_TEXT | f::MENU_MAX_H | f::SEGMENTED_RESULT | f::WIDGETS_KEYED | f::SEGMENTED_STORED_FOCUS | f::IMAGE_CHECKED | f::BUTTON_PARTS | f::VIEW_KEYED => true,
        _ => false,
    }) as u32
});

unsafe fn create_ui(desc: PxuiUiDesc, fonts: &[&[u8]]) -> *mut PxuiUi {
    #[allow(unused_mut)]
    let mut text = pxui_core::text::Text::new(fonts);
    // an unparseable font refuses the Ui
    if text.font_count() != fonts.len() { return core::ptr::null_mut(); }
    #[cfg(debug_assertions)]
    { text.reporter = Some(mem_sys::report); }
    let platform = match desc.platform { 0 => platform(), 1 => Platform::Apple, _ => Platform::Other };
    let font_size = (desc.font_size != 0).then_some(desc.font_size as f32);
    let ui = Ui::create(platform, font_size, text);
    let engine = Box::into_raw(Box::new(ui));
    let shared = Box::into_raw(Box::new(Shared::new()));
    let owner = Box::into_raw(Box::new(PxuiUi { theme: core::ptr::null(), engine, shared, owner: true }));
    unsafe { (*shared).current = owner; }
    owner
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

/// Fills `shared.output` from the engine. `nodes` rebuilds the accessibility records too (the first read after a frame; later
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
    let revision = engine.draw_revision() as u64;
    let acknowledged = core::mem::take(&mut shared.draw_acknowledged);
    if nodes || revision != shared.output.draw_revision {
        // Publish all dirty bytes before lending pointers into the retained buffer.
        shared.dirty.clear();
        shared.dirty.reserve(output.draw.iter().map(|layer| layer.dirty.capacity()).sum());
        shared.dirty.extend(output.draw.iter().flat_map(|l| l.dirty.iter().map(|&(start, end)| pxui_types::ffi::PxuiDirty { start, end })));
        shared.layers.clear();
        let mut at = 0;
        for l in output.draw.iter() {
            let dirty = Slice { ptr: shared.dirty.as_ptr().wrapping_add(at), len: l.dirty.len() };
            at += l.dirty.len();
            shared.layers.push(pxui_types::ffi::PxuiLayerInfo { key: l.key, id: l.id, breaks: l.breaks.len() as u32, instances: Slice { ptr: l.instances.as_ptr(), len: l.instances.len() }, dirty });
        }
    } else if acknowledged {
        // The renderer can acknowledge a draw while Rust readers still hold the published slices.
        let published = shared.layers.as_mut_ptr();
        for (index, drawn) in output.draw.iter().take(shared.layers.len()).enumerate() {
            unsafe { core::ptr::addr_of_mut!((*published.add(index)).dirty.len).write(drawn.dirty.len()); }
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
        size: core::mem::size_of::<Output>() as u32, reserved: 0, draw_revision: revision,
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
        layers: Slice { ptr: shared.layers.as_ptr(), len: shared.layers.len() },
    };
}

// The output of the last frame (all zero before the first). Borrowed from the `Ui` and valid until its next frame;
// the counters and the platform record are refreshed on each call, the accessibility records when their tree changed.
export!(bare pub unsafe fn pxui_ui_output(ui: *mut PxuiUi) -> *const Output {
    let Some(state) = (unsafe { get(ui) }) else { return core::ptr::null(); };
    let shared = state.shared();
    if !shared.in_frame { let nodes = core::mem::take(&mut shared.frame_ended); build_output(shared, unsafe { &*state.engine }, nodes); }
    &shared.output
});

export!(pub unsafe fn pxui_ui_set_theme(ui: *mut PxuiUi, source: *const ThemeSource) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if !state.owner || state.shared().in_frame { return PXUI_ERR_STATE; }
    if source.is_null() { return unsafe { fail(ui, "null theme source") }; }
    let Some(source) = read_desc!(source, ThemeSource) else { return unsafe { fail(ui, "invalid theme descriptor") }; };
    if source.flags & !3 != 0 { return unsafe { fail(ui, "invalid theme descriptor") }; }
    let parsed: Result<pxui_core::Theme, Vec<pxui_core::theme::ThemeError>> = if source.source.len == 0 { Ok(pxui_core::Theme::default()) } else {
        let Some(text) = (unsafe { crate::state::checked_text(source.source) }) else { return unsafe { fail(ui, "invalid theme UTF-8 or length") }; };
        unsafe { &*state.engine }.parse_theme(text, &pxui_core::Theme::default())
    };
    let mut theme = match parsed { Ok(theme) => theme, Err(errors) => return unsafe { fail(ui, &errors.iter().map(|e| format!("line {}: {}", e.line, e.message)).collect::<Vec<_>>().join("\n")) } };
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
    let actions = if count == 0 { &[] } else { unsafe { std::slice::from_raw_parts(actions, count) } };
    // Reject the whole batch before feeding any chunk to the retained engine queue.
    if actions.iter().any(|a| !matches!(a.action_code,1|2|4|8|16|32|64|128|256)) {
        return unsafe { fail(ui, "invalid accessibility action code") };
    }
    use pxui_core::a11y::{Action, A11yAction};
    const KINDS: [Action;9] = [Action::Press,Action::Focus,Action::SetValue,Action::Scroll,Action::Expand,Action::Collapse,Action::Select,Action::MoveUp,Action::MoveDown];
    let mut translated = [A11yAction { id:0,action:Action::Press,payload:0.0,end:0.0 };32];
    for chunk in actions.chunks(translated.len()) {
        for (to,from) in translated.iter_mut().zip(chunk) {
            *to=A11yAction { id:from.id,action:KINDS[from.action_code.trailing_zeros() as usize],payload:from.payload,end:from.end as f64 };
        }
        unsafe { (&mut *state.engine).inject_a11y(&translated[..chunk.len()]); }
    }
    0
});

export!(pub unsafe fn pxui_ui_patch_text(ui: *mut PxuiUi, box_: *const PxuiBox, value: Str, out: *mut u8) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    let (Some(value), false) = (unsafe { checked(value) }, out.is_null()) else { return unsafe { fail(ui, "invalid patch text arguments") }; };
    unsafe { *out = e.patch_text(&b, value) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_busy_step(ui: *mut PxuiUi, box_: *const PxuiBox, now: f64, out: *mut u8) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if out.is_null() || !now.is_finite() { return unsafe { fail(ui, "invalid busy step arguments") }; }
    unsafe { *out = e.busy_step(&b, now) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_request_frame(ui: *mut PxuiUi) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    unsafe { (&mut *state.engine).request_frame(); }
    0
});

fn touch_event(e:&pxui_types::Event)->Option<input::Touch> {
    if e.text.len!=core::mem::size_of::<pxui_types::Touch>() || e.text.ptr.is_null() {return None;}
    let t=unsafe {e.text.ptr.cast::<pxui_types::Touch>().read_unaligned()};
    let value=input::Touch {device:t.device,finger:t.finger,point:(t.x,t.y),time:t.time,pressure:t.pressure,phase:input::TouchPhase::from_code(t.phase)?};
    value.valid().then_some(value)
}

/// The largest width or height of a surface a frame takes, in points (`FrameIn.width`/`height`, `Input.logical_size`) and of a resize
/// event, in points or pixels: a larger or negative one is refused.
pub(crate) const MAX_SURFACE: f64 = (1 << 16) as f64;

/// One wire event checked and decoded: the one validator of both frame entries and `pxui_ui_wants_frame`. Its `x`, `y` and `value` are
/// finite whatever its kind; its text (any kind's, read only past these checks) is at most `MAX_TEXT_BYTES` and is taken from `budget`
/// (`MAX_TOTAL_TEXT_BYTES` a frame, or a `wants_frame` call); a touch's payload is a whole valid `Touch`; a text editing range is not
/// negative; a resize is within `0..=MAX_SURFACE`; the kind and its code are known. Anything else is refused (`None`), never cut.
pub(crate) fn translate_event(e: &pxui_types::Event, budget: &mut usize) -> Option<Event> {
    if !(e.x.is_finite() && e.y.is_finite() && e.value.is_finite()) { return None; }
    if e.kind==pxui_types::Event::TOUCH {return touch_event(e).map(Event::Touch);}
    // the commonest event of a frame carries no text or code: no wire record to build and decode
    if e.kind==pxui_types::Event::POINTER_MOVED {return Some(Event::PointerMoved((e.x,e.y)));}
    if e.text.len > crate::state::MAX_TEXT_BYTES || e.text.len > *budget { return None; }
    *budget -= e.text.len;
    let event = Event::from_raw(&input::wire::Raw { kind: e.kind, code: e.code, x: e.x, y: e.y, value: e.value, text: unsafe { text(e.text) },touch:None })?;
    let size = |(w, h): (i64, i64)| (0..=MAX_SURFACE as i64).contains(&w) && (0..=MAX_SURFACE as i64).contains(&h);
    match &event {
        Event::TextEditing { start, length, .. } if *start < 0 || *length < 0 => None,
        Event::Resized(wh) | Event::PixelSizeChanged(wh) if !size(*wh) => None,
        _ => Some(event),
    }
}

fn valid_touches(events:&[pxui_types::Event])->bool {
    events.iter().all(|e|e.kind!=pxui_types::Event::TOUCH || touch_event(e).is_some())
}

/// Decode the whole batch before any edge changes retained input.
fn decode_events(out: &mut Vec<Event>, events: &[pxui_types::Event]) -> bool {
    let mut budget = crate::state::MAX_TOTAL_TEXT_BYTES;
    for raw in events {
        let Some(event) = translate_event(raw, &mut budget) else { return false };
        out.push(event);
    }
    true
}

fn fold_input_edges(base: &mut input::Frame) {
    for event in &base.events {
        match event {
            Event::KeyPressed(k) => if !base.keys.contains(k) { base.keys.push(*k) },
            Event::KeyReleased(k) => base.keys.retain(|held| held != k),
            Event::FocusLost => { base.keys.clear(); base.buttons.clear(); }
            Event::PointerMoved(p) => base.pointer = *p,
            Event::PointerPressed(b, p) => { base.pointer = *p; if !base.buttons.contains(b) { base.buttons.push(*b) } }
            Event::PointerReleased(b, p) => { base.pointer = *p; base.buttons.retain(|held| held != b) }
            Event::PointerCancelled(b) => base.buttons.retain(|held| held != b),
            _ => {}
        }
    }
}

/// Reuse input buffers. Retained keys and buttons change only after the batch has been validated.
unsafe fn take_input(ui: *mut PxuiUi, time: f64) -> Box<frame::Frame> {
    let mut base = unsafe { (*(*ui).shared).spare_input.take() }.unwrap_or_else(|| {
        let mut input=input::at_time(time);
        input.events.reserve(input::EVENT_RESERVE);
        Box::new(frame::Frame { input, pixel_scale: (1.0, 1.0), fps: 60.0, mouse_delta: (0.0, 0.0) })
    });
    base.input.events.clear();
    base
}

unsafe fn restore_input(ui: *mut PxuiUi, base: Box<frame::Frame>) {
    unsafe { (*(*ui).shared).spare_input = Some(base); }
}

const NO_ENVIRONMENT: pxui_types::Environment = pxui_types::Environment { safe_area: [0.0; 4], keyboard_frame: [0.0; 4], keyboard_from: [0.0; 4], keyboard_started: 0.0, keyboard_duration: 0.0, keyboard_curve: 0, keyboard_visible: 0 };
const NO_KEYBOARD: pxui_types::Keyboard = pxui_types::Keyboard { frame: [0.0; 4], flags: 0 };

/// Whether `len` bytes (a whole number of 4-byte words) at `a` and `b` are equal: a few wide compares where a float-by-float `==` is a
/// branch each. Used on values with no padding, one of which the engine holds: it was validated when it was set, so identical bytes
/// are valid (NaN included: it never was).
#[inline(always)]
unsafe fn same_bytes(a: *const u8, b: *const u8, len: usize) -> bool {
    debug_assert!(len % 4 == 0);
    let (mut diff, mut at) = (0u64, 0);
    while at + 8 <= len { diff |= unsafe { a.add(at).cast::<u64>().read_unaligned() ^ b.add(at).cast::<u64>().read_unaligned() }; at += 8; }
    if at < len { diff |= unsafe { u64::from(a.add(at).cast::<u32>().read_unaligned() ^ b.add(at).cast::<u32>().read_unaligned()) }; }
    diff == 0
}

/// `same_bytes` of the host environment on two records.
#[inline(always)]
fn same_environment(a: &pxui_types::Environment, b: &pxui_types::Environment) -> bool {
    unsafe { same_bytes((a as *const pxui_types::Environment).cast(), (b as *const pxui_types::Environment).cast(), core::mem::size_of::<pxui_types::Environment>()) }
}

/// `same_bytes` of the keyboard geometry on two records.
#[inline(always)]
fn same_keyboard(a: &pxui_types::Keyboard, b: &pxui_types::Keyboard) -> bool {
    unsafe { same_bytes((a as *const pxui_types::Keyboard).cast(), (b as *const pxui_types::Keyboard).cast(), core::mem::size_of::<pxui_types::Keyboard>()) }
}

fn valid_environment(value:&pxui_types::Environment)->bool {
    let rect=|r:&[f32;4]| r.iter().all(|v|v.is_finite()) && r[2]>=0.0 && r[3]>=0.0 && (r[0]+r[2]).is_finite() && (r[1]+r[3]).is_finite();
    value.safe_area.iter().all(|v|v.is_finite() && *v>=0.0)
        && (value.safe_area[0]+value.safe_area[2]).is_finite() && (value.safe_area[1]+value.safe_area[3]).is_finite()
        && rect(&value.keyboard_frame) && rect(&value.keyboard_from)
        && value.keyboard_started.is_finite() && value.keyboard_duration.is_finite() && value.keyboard_duration>=0.0
        && (value.keyboard_started+value.keyboard_duration).is_finite() && value.keyboard_visible<=1
}

/// Runs one frame of `ui` over `frame`. Both frame entry points call it after decoding their input.
unsafe fn run_frame(ui: *mut PxuiUi, frame: &frame::Frame, build: Inner, out: &mut FrameOut) -> i32 {
    // The callbacks reach the shared state through `shared`, so no `&mut Shared` is held across the build.
    let (engine, shared) = unsafe { ((*ui).engine, (*ui).shared) };
    unsafe {
        let s = &mut *shared;
        s.error.clear();
        s.paint_error = false;
        s.returned.clear();
        s.lent_used = 0;
        s.returned_bytes = 0;
        s.chunks_used = 0;
        s.in_frame = true;
        // no handle is usable until the build callback runs: the engine is mid-frame (its own phases run between callbacks)
        s.current = core::ptr::null_mut();
        s.building = core::ptr::null_mut();
    }
    // a panic of the build unwinds past this (the static wrapper's callbacks run without a catch): the `Ui` leaves the frame
    struct Leave(*mut Shared, *mut PxuiUi);
    impl Drop for Leave { fn drop(&mut self) { unsafe { (*self.0).in_frame = false; (*self.0).current = self.1; (*self.0).building = core::ptr::null_mut(); } } }
    let _leave = Leave(shared, ui);
    // a panic of the build (the static wrapper's callbacks run without a catch) leaves the engine mid-frame: it is reset, then the panic goes on
    let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { (&mut *engine).frame(frame, build) }));
    let output = match built { Ok(output) => output, Err(panic) => {
        unsafe { (&mut *engine).abandon_frame(); (*shared).frame_ended = true; }
        std::panic::resume_unwind(panic)
    } };
    let (damage, layers, layer_count) = (output.damage, output.draw.as_ptr(), output.draw.len());
    let s = unsafe { &mut *shared };
    s.in_frame = false;
    crate::node::finish_frame(s, unsafe { &*engine });
    let damage_rect = damage.map_or([0.0; 4], |r| [r.x, r.y, r.w, r.h]);
    *out = FrameOut { size: core::mem::size_of::<FrameOut>() as u32, damage: damage_rect, has_damage: damage.is_some() as u8, reserved: [0; 3], layer_count: layer_count as u32, layers: layers.cast() };
    if core::mem::take(&mut s.poisoned) { PXUI_ERR_PANIC } else if core::mem::take(&mut s.paint_error) { PXUI_ERR_STATE } else { 0 }
}

/// The checks every frame entry point makes before it decodes anything: a live owner handle, not re-entered from a
/// callback; a `Ui` poisoned since its last frame skips this frame (returning the error) and starts clean.
unsafe fn enter_frame(ui: *mut PxuiUi) -> Result<(), i32> {
    let Some(state) = (unsafe { get(ui) }) else { return Err(PXUI_ERR_ARGUMENT) };
    let shared = state.shared();
    if !state.owner || shared.in_frame { return Err(unsafe { fail_code(ui, "a frame cannot start from a frame callback", PXUI_ERR_STATE) }); }
    if core::mem::take(&mut shared.poisoned) { shared.current = ui; return Err(PXUI_ERR_PANIC); }
    Ok(())
}

/// The frame entries' bodies, shared by the exports (which decode and check the C descriptor first) and the static wrapper.
pub(crate) unsafe fn frame_body(ui: *mut PxuiUi, input: &FrameIn, build: Inner, out: &mut FrameOut) -> i32 {
    if !ui.is_null() { unsafe { crate::state::note_call((*ui).shared) }; }
    if let Err(code) = unsafe { enter_frame(ui) } { return code; }
    // a size within 0..=MAX_SURFACE (NaN is outside every range)
    let surface = |v: f32| (0.0..=MAX_SURFACE as f32).contains(&v);
    if !input.time.is_finite() || !input.scale.is_finite() || input.scale <= 0.0 || !surface(input.width) || !surface(input.height) { return unsafe { fail(ui, "invalid frame descriptor") }; }
    if input.event_count > 4096 || (input.event_count != 0 && input.events.is_null()) { return unsafe { fail(ui, "invalid frame event slice") }; }
    let legacy=core::mem::offset_of!(FrameIn,appearance) as u32;
    let previous=core::mem::offset_of!(FrameIn,environment) as u32;
    let (appearance,reduced)=if input.size==legacy { (0,0) } else if input.size>=previous {
        (input.appearance,input.reduced_motion)
    } else { return unsafe { fail(ui,"invalid frame descriptor size") }; };
    // read where they lie: what the engine holds was validated when it was set (the default is valid), so a frame that repeats it
    // compares bytes and checks nothing, and only a changed value is validated (a NaN never matches)
    let environment=if input.size>=core::mem::offset_of!(FrameIn,keyboard) as u32 { &input.environment } else { &NO_ENVIRONMENT };
    let keyboard=if input.size>=core::mem::offset_of!(FrameIn,surface) as u32 { &input.keyboard } else { &NO_KEYBOARD };
    let engine=unsafe { &mut *(*ui).engine };
    let environment_changed=!same_environment(environment,engine.environment_ref());
    if environment_changed && !valid_environment(environment) { return unsafe { fail(ui,"invalid host environment") }; }
    let keyboard_changed=!same_keyboard(keyboard,engine.keyboard_ref());
    if keyboard_changed && !keyboard.valid() { return unsafe { fail(ui,"invalid keyboard geometry") }; }
    if appearance>FrameIn::APPEARANCE_DARK || reduced>1 { return unsafe { fail(ui,"invalid system preferences") }; }
    let mut base = unsafe { take_input(ui, input.time) };
    let events = if input.event_count == 0 { &[] } else { unsafe { core::slice::from_raw_parts(input.events, input.event_count) } };
    if !decode_events(&mut base.input.events, events) {
        let message=if valid_touches(events) { "invalid event kind, code, or UTF-8 payload" } else { "invalid touch payload" };
        base.input.events.clear();
        unsafe { restore_input(ui, base); }
        return unsafe { fail(ui, message) };
    }
    let frame = &mut base.input;
    (frame.t, frame.dt, frame.frame, frame.tick) = (input.time, 0.0, input.time.to_bits() as i64, 0);
    (frame.size, frame.pointer) = ((input.width as i64, input.height as i64), (0.0, 0.0));
    frame.buttons.clear();
    fold_input_edges(frame);
    unsafe { &mut *(*ui).engine }.set_system_preferences(appearance,reduced!=0);
    if environment_changed { engine.set_environment(*environment); }
    if keyboard_changed { engine.set_keyboard(*keyboard); }
    (base.pixel_scale, base.fps, base.mouse_delta) = ((input.scale as f64, input.scale as f64), 60.0, (0.0, 0.0));
    let rc = unsafe { run_frame(ui, &base, build, out) };
    unsafe { restore_input(ui, base); }
    rc
}

// Read only fields the caller supplied: old 48-byte inputs carry no OS preferences.
unsafe fn read_frame(input:*const FrameIn)->Option<FrameIn> {
    if input.is_null() { return None; }
    let size=unsafe { input.cast::<u32>().read_unaligned() } as usize;
    let legacy=core::mem::offset_of!(FrameIn,appearance);
    let previous=core::mem::offset_of!(FrameIn,environment);
    // the sizes FrameIn has had (48, 56, 128, 152) or the whole record and more; no other
    if !(matches!(size,48|56|128|152) || size>=core::mem::size_of::<FrameIn>()) { return None; }
    let mut value=FrameIn::default();
    let copied=if size>=core::mem::size_of::<FrameIn>() { core::mem::size_of::<FrameIn>() } else if size>=core::mem::offset_of!(FrameIn,surface) { core::mem::offset_of!(FrameIn,surface) } else if size>=core::mem::offset_of!(FrameIn,keyboard) { core::mem::offset_of!(FrameIn,keyboard) } else if size>=previous { previous } else { legacy };
    unsafe { core::ptr::copy_nonoverlapping(input.cast::<u8>(),(&mut value as *mut FrameIn).cast::<u8>(),copied); }
    Some(value)
}

export!(pub unsafe fn pxui_ui_system_appearance(ui:*mut PxuiUi,out:*mut u32)->i32 {
    let Some(engine)=(unsafe { crate::state::engine(ui) }) else { return PXUI_ERR_ARGUMENT };
    if out.is_null() { return unsafe { fail(ui,"null appearance output") }; }
    unsafe { out.write(engine.system_appearance()); }
    0
});

export!(trusted frame pub unsafe fn pxui_frame(ui: *mut PxuiUi, input: *const FrameIn, build: Build, user: *mut c_void, out: *mut FrameOut) -> i32 {
    if input.is_null() || out.is_null() { return unsafe { fail(ui, "null frame argument") }; }
    let shared = <*mut PxuiUi as crate::state::Handle>::shared(ui);
    let mut build = |engine: &mut Ui| crate::state::call_build(shared, engine, build, user);
    // a whole record is read where it lies (`frame_body` reads it before the build runs, so the caller cannot change it under the
    // engine); an older, shorter one is copied over the defaults
    if unsafe { input.cast::<u32>().read_unaligned() } as usize >= core::mem::size_of::<FrameIn>() { return unsafe { frame_body(ui, &*input, &mut build, &mut *out) }; }
    let Some(input) = (unsafe { read_frame(input) }) else { return unsafe { fail(ui, "invalid frame descriptor") }; };
    unsafe { frame_body(ui, &input, &mut build, &mut *out) }
});

pub(crate) unsafe fn input_frame_body(ui: *mut PxuiUi, input: &Input, build: Inner, out: &mut FrameOut) -> i32 {
    if !ui.is_null() { unsafe { crate::state::note_call((*ui).shared) }; }
    if let Err(code) = unsafe { enter_frame(ui) } { return code; }
    if !input.time.is_finite() || !input.dt.is_finite() || input.dt < 0.0 || !input.fps.is_finite() || input.fps < 0.0 || !input.pointer.iter().all(|v| v.is_finite()) || !input.pixel_scale.iter().all(|v| v.is_finite() && *v > 0.0) || !input.mouse_delta.iter().all(|v| v.is_finite())
        || !input.logical_size.iter().all(|v| (0..=MAX_SURFACE as i64).contains(v)) { return unsafe { fail(ui, "invalid input frame") }; }
    if input.events.len > 4096 || input.keys.len > 512 || input.buttons.len > 32 || (input.events.len != 0 && input.events.ptr.is_null()) || (input.keys.len != 0 && input.keys.ptr.is_null()) || (input.buttons.len != 0 && input.buttons.ptr.is_null()) { return unsafe { fail(ui, "invalid input spans") }; }
    let events=if input.events.len==0 {&[]} else {unsafe {core::slice::from_raw_parts(input.events.ptr,input.events.len)}};
    if !valid_touches(events) {return unsafe {fail(ui,"invalid touch payload")};}
    let keys = if input.keys.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(input.keys.ptr, input.keys.len) } };
    let buttons = if input.buttons.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(input.buttons.ptr, input.buttons.len) } };
    if keys.iter().any(|code| key(*code).is_none()) { return unsafe { fail(ui, "invalid held key") }; }
    if buttons.iter().any(|code| button(*code).is_none()) { return unsafe { fail(ui, "invalid held mouse button") }; }
    let mut held = unsafe { take_input(ui, input.time) };
    let base = &mut held.input;
    if !decode_events(&mut base.events, events) {
        base.events.clear();
        unsafe { restore_input(ui, held); }
        return unsafe { fail(ui, "invalid event kind, code, or UTF-8 payload") };
    }
    (base.t, base.dt, base.frame, base.tick) = (input.time, input.dt, input.frame, input.tick);
    base.keys.clear();
    base.keys.extend(keys.iter().filter_map(|code| key(*code)));
    base.buttons.clear();
    base.buttons.extend(buttons.iter().filter_map(|code| button(*code)));
    base.size = (input.logical_size[0], input.logical_size[1]);
    base.pointer = (input.pointer[0], input.pointer[1]);
    fold_input_edges(base);
    unsafe { &mut *(*ui).engine }.set_environment(pxui_types::Environment::default());
    unsafe { &mut *(*ui).engine }.set_keyboard(pxui_types::Keyboard::default());
    (held.pixel_scale, held.fps, held.mouse_delta) = ((input.pixel_scale[0], input.pixel_scale[1]), input.fps, (input.mouse_delta[0], input.mouse_delta[1]));
    let rc = unsafe { run_frame(ui, &held, build, out) };
    unsafe { restore_input(ui, held); }
    rc
}

export!(trusted frame pub unsafe fn pxui_ui_frame(ui: *mut PxuiUi, input: *const Input, build: Build, user: *mut c_void, out: *mut FrameOut) -> i32 {
    if input.is_null() || out.is_null() { return unsafe { fail(ui, "null frame argument") }; }
    let Some(input) = read_desc!(input, Input) else { return unsafe { fail(ui, "invalid input frame") }; };
    unsafe { input_frame_body(ui, &input, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, build, user), &mut *out) }
});

// The frame's input as the engine routed it to `owner` (a viewport's root key; none: the app's), for the app's own content: the
// same record `pxui_ui_frame` takes, borrowed until the next call. Only after a frame has run.
export!(pub unsafe fn pxui_ui_input(ui: *mut PxuiUi, has_owner: u8, owner: i64, out: *mut Input) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() || has_owner > 1 { return unsafe { fail(ui, "invalid input query") }; }
    let engine = unsafe { &*state.engine };
    if engine.frame_index() == 0 { return unsafe { fail_code(ui, "no frame has completed", PXUI_ERR_STATE) }; }
    let shared = state.shared();
    let routed=shared.routed.get_or_insert_with(||frame::Frame {input:input::at_time(0.0),pixel_scale:(1.0,1.0),fps:0.0,mouse_delta:(0.0,0.0)});
    engine.input_into((has_owner!=0).then_some(owner),routed);
    let Some(f) = shared.routed.as_ref() else { return PXUI_ERR_STATE };
    shared.routed_events.clear();
    shared.routed_touches.clear();
    shared.routed_touches.reserve(f.input.events.iter().filter(|e|matches!(e,Event::Touch(_))).count());
    if shared.routed_text.len()<f.input.events.len() {shared.routed_text.resize(f.input.events.len(),String::new());}
    for (event, scratch) in f.input.events.iter().zip(shared.routed_text.iter_mut()) {
        scratch.clear();
        if let Event::Touch(t)=event {
            shared.routed_touches.push(pxui_types::Touch {device:t.device,finger:t.finger,x:t.point.0,y:t.point.1,time:t.time,pressure:t.pressure,phase:t.phase.code()});
            shared.routed_events.push(pxui_types::Event {kind:pxui_types::Event::TOUCH,text:Str {ptr:shared.routed_touches.as_ptr().wrapping_add(shared.routed_touches.len()-1).cast(),len:core::mem::size_of::<pxui_types::Touch>()},..Default::default()});
            continue;
        }
        let raw = event.raw(scratch);
        let text = raw.text.unwrap_or("");
        shared.routed_events.push(pxui_types::Event { kind: raw.kind, code: raw.code, x: raw.x, y: raw.y, value: raw.value, text: Str { ptr: text.as_ptr(), len: text.len() } });
    }
    shared.routed_keys.clear();
    shared.routed_keys.extend(f.input.keys.iter().map(|k| k.code()));
    shared.routed_buttons.clear();
    shared.routed_buttons.extend(f.input.buttons.iter().map(|b| b.code()));
    unsafe {
        *out = Input {
            size: core::mem::size_of::<Input>() as u32, time: f.input.t, dt: f.input.dt, frame: f.input.frame, tick: f.input.tick,
            logical_size: [f.input.size.0, f.input.size.1], pointer: [f.input.pointer.0, f.input.pointer.1],
            keys: Slice { ptr: shared.routed_keys.as_ptr(), len: shared.routed_keys.len() }, buttons: Slice { ptr: shared.routed_buttons.as_ptr(), len: shared.routed_buttons.len() },
            events: Slice { ptr: shared.routed_events.as_ptr(), len: shared.routed_events.len() },
            pixel_scale: [f.pixel_scale.0, f.pixel_scale.1], mouse_delta: [f.mouse_delta.0, f.mouse_delta.1], fps: f.fps,
        };
    }
    0
});

// Marks layer `layer` of the last frame drawn (its dirty ranges clear), as `pxui_renderer_draw` does for a layer it draws: for a
// host that reads the layers itself.
export!(pub unsafe fn pxui_ui_layer_drawn(ui: *mut PxuiUi, layer: u64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let engine = unsafe { &mut *state.engine };
    if layer as usize >= engine.output().draw.len() { return unsafe { fail(ui, "no such layer") }; }
    engine.drawn(layer as usize);
    state.shared().draw_acknowledged = true;
    0
});

export!(trusted pub unsafe fn pxui_within(ui: *mut PxuiUi, box_: *const PxuiBox, build: Build, user: *mut c_void) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    unsafe { trusted::within(ui, box_, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, build, user)) }
});

export!(trusted pub unsafe fn pxui_scope(ui: *mut PxuiUi, label: Str, build: Build, user: *mut c_void) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(label) = (unsafe { crate::state::checked_key(label) }) else { return unsafe { fail(ui, "invalid scope label") }; };
    unsafe { trusted::scope(ui, label, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, build, user)) }
});

export!(trusted pub unsafe fn pxui_col(ui: *mut PxuiUi, padding: f32, label: Str, build: Build, user: *mut c_void) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(label) = (unsafe { crate::state::checked_key(label) }) else { return unsafe { fail(ui, "invalid column arguments") }; };
    unsafe { trusted::col(ui, padding, label, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, build, user)) }
});

/// The panel of `panel`'s record keyed by `key` and named `name`, the body of `pxui_panel` and `pxui_ui_panel_keyed`.
unsafe fn panel(ui: *mut PxuiUi, key: &str, name: &str, panel: *const PxuiPanel, build: Build, user: *mut c_void, root: &mut PxuiBox) -> i32 {
    // read where it lies (a record at least as long as ours), the look only when it has one
    let Some(panel) = sized!(panel) else { return unsafe { fail(ui, "invalid panel descriptor") }; };
    if panel.padding < 0 { return unsafe { fail(ui, "invalid panel padding") }; }
    // the fractional padding counts only where it agrees with the integer one (a 1.x record leaves the hole unset)
    let padding = if (panel.padding_points - panel.padding as f32).abs() <= 0.5 && panel.padding_points >= 0.0 { panel.padding_points } else { panel.padding as f32 };
    let mut overlay = pxui_types::ui::Overlay { at: (panel.x, panel.y), w: Some(panel.width), h: (panel.has_height != 0).then_some(panel.height),
        max_h: (panel.has_max_height != 0).then_some(panel.max_height), window: panel.window != 0, padding, name: Some(name), ..pxui_types::ui::Overlay::DEFAULT };
    if panel.has_stroke != 0 { overlay.stroke = Some(panel.stroke); }
    if panel.has_look != 0 { let Some(look) = crate::style::decode_look(panel.look) else { return unsafe { fail(ui, "invalid panel look") }; }; overlay.look = Some(look); }
    unsafe { trusted::panel(ui, key, &overlay, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, build, user), root) }
}

// Legacy (cut at 2.0): keyed by its label, named up to a `##`; `pxui_ui_panel_keyed` keys apart.
export!(trusted pub unsafe fn pxui_panel(ui: *mut PxuiUi, panel: *const PxuiPanel, label: Str, build: Build, user: *mut c_void) -> i32 {
    if unsafe { get(ui) }.is_none() || panel.is_null() { return PXUI_ERR_ARGUMENT; }
    let Some(label) = (unsafe { crate::state::checked_label(label) }) else { return unsafe { fail(ui, "invalid panel label") }; };
    unsafe { self::panel(ui, label, pxui_types::ui::shown(label), panel, build, user, &mut PxuiBox::default()) }
});

// ABI 1.68: a panel keyed by `keyed.key`, named `keyed.name` (empty: none).
export!(trusted pub unsafe fn pxui_ui_panel_keyed(ui: *mut PxuiUi, keyed: *const pxui_types::ffi::PxuiKeyedDesc, panel: *const PxuiPanel, build: Build, user: *mut c_void) -> i32 {
    if unsafe { get(ui) }.is_none() || panel.is_null() { return PXUI_ERR_ARGUMENT; }
    let Some((key, name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { self::panel(ui, key, name, panel, build, user, &mut PxuiBox::default()) }
});

// ABI 1.76: `pxui_ui_panel_keyed` that also writes the panel's root to `root`.
export!(trusted pub unsafe fn pxui_ui_panel_root(ui: *mut PxuiUi, keyed: *const pxui_types::ffi::PxuiKeyedDesc, panel: *const PxuiPanel, build: Build, user: *mut c_void, root: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() || panel.is_null() { return PXUI_ERR_ARGUMENT; }
    if root.is_null() { return unsafe { fail(ui, "null panel root") }; }
    let Some((key, name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { self::panel(ui, key, name, panel, build, user, &mut *root) }
});

export!(trusted pub unsafe fn pxui_label(ui: *mut PxuiUi, label: Str) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid label text") } };
    unsafe { trusted::pxui_label(ui, pair(label)) }
});

export!(trusted pub unsafe fn pxui_ui_label(ui: *mut PxuiUi, text: Str, style: *const pxui_types::ffi::PxuiLabelStyle) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let (Some(text), false) = (unsafe { checked(text) }, style.is_null()) else { return unsafe { fail(ui, "invalid label arguments") } };
    let Some((style, digits)) = label_style(style) else { return unsafe { fail(ui, "invalid label style") } };
    let mut out = PxuiBox::default();
    unsafe { trusted::pxui_ui_label(ui, pair(text), &style, digits, &mut out) }
});

// ABI 1.54: `pxui_ui_label` with the label's box (`label_box`, null for none) writing the label's node to `out`; a null `style` is
// the theme's label (`pxui_label`).
export!(trusted pub unsafe fn pxui_ui_label_node(ui: *mut PxuiUi, text: Str, style: *const pxui_types::ffi::PxuiLabelStyle, label_box: *const pxui_types::ffi::PxuiLabelBox, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let (Some(text), false) = (unsafe { checked(text) }, out.is_null()) else { return unsafe { fail(ui, "invalid label arguments") } };
    let (mut style, digits) = if style.is_null() { (pxui_types::text::LabelFields::UNSET, false) } else {
        let Some(read) = label_style(style) else { return unsafe { fail(ui, "invalid label style") } };
        read
    };
    if !label_box.is_null() && !read_label_box(label_box, &mut style) { return unsafe { fail(ui, "invalid label box") } }
    unsafe { trusted::pxui_ui_label(ui, pair(text), &style, digits, &mut *out) }
});

/// Applies the caller's `PxuiLabelBox` (not null) to `label`: false, leaving `label` as it was, for a short record, unknown `set` bits,
/// a non-finite height, a role past the last one, `whole_pixels` past 1 or nonzero reserved bytes. What the record leaves unset stays
/// as `label` has it.
pub(crate) fn read_label_box(label_box: *const pxui_types::ffi::PxuiLabelBox, label: &mut pxui_types::text::LabelFields) -> bool {
    use pxui_types::ffi::PxuiLabelBox as B;
    let Some(b) = read_desc!(label_box, B) else { return false };
    if b.set & !B::SET_ALL != 0 || !b.height.is_finite() || b.role > pxui_types::a11y::Role::Separator as u8 || b.whole_pixels > 1 || b.reserved != [0; 2] { return false; }
    if b.set & B::SET_HEIGHT != 0 { label.height = Some(b.height); }
    // the role was checked against the last one above
    if b.set & B::SET_ROLE != 0 { label.role = unsafe { core::mem::transmute::<u8, pxui_types::a11y::Role>(b.role) }; }
    label.whole_pixels |= b.whole_pixels != 0;
    true
}

/// The `Wrap` a C record's number names (`None`: no wrap has it).
pub(crate) fn wrap_of(value: u32) -> Option<pxui_types::text::Wrap> {
    use pxui_types::text::Wrap as W;
    [W::None, W::Word, W::Glyph, W::WordSpaces].into_iter().find(|w| *w as u32 == value)
}
/// The `Ellipsis` a C record's number names.
pub(crate) fn ellipsis_of(value: u8) -> Option<pxui_types::text::Ellipsis> {
    use pxui_types::text::Ellipsis as E;
    [E::None, E::End, E::Middle, E::Head].into_iter().find(|e| *e as u8 == value)
}
/// The `TextAlign` a C record's number names.
pub(crate) fn align_of(value: u32) -> Option<pxui_types::text::TextAlign> {
    use pxui_types::text::TextAlign as A;
    [A::Start, A::Center, A::End].into_iter().find(|a| *a as u32 == value)
}

// ABI 1.49: the text style `style` shapes with, resolved against the Ui's theme label (what `pxui_ui_label` would shape), written to
// `out` (its `size` set). `digits` is ignored. Refused for a null or invalid style or one that does not resolve.
export!(pub unsafe fn pxui_ui_resolve_label(ui: *mut PxuiUi, style: *const pxui_types::ffi::PxuiLabelStyle, out: *mut pxui_types::ffi::PxuiTextStyle) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null resolve output") }; }
    let Some((style, _)) = label_style(style) else { return unsafe { fail(ui, "invalid label style") } };
    let Some(resolved) = unsafe { &*state.engine }.resolve(&style) else { return unsafe { fail(ui, "label style does not resolve") } };
    unsafe { out.write_unaligned(resolved); }
    0
});

/// The `Label` and `digits` flag a caller's record holds; `None` for a null or short record, unknown `set` bits, a flag past 1, a
/// non-finite set value, an invalid set width, a wrap, align or ellipsis number no style has or a feature bit past tabular numbers.
/// Sizes are the engine's to check (`resolve_label`).
pub(crate) fn label_style(style: *const pxui_types::ffi::PxuiLabelStyle) -> Option<(pxui_types::text::LabelFields, bool)> {
    use pxui_types::{ffi::{PxuiLabelStyle as S, PxuiTextStyle as TextStyle}, text::LabelFields};
    if style.is_null() { return None; }
    let r = read_desc!(style, S)?;
    if r.set & !S::SET_ALL != 0 || r.caps > 1 || r.digits > 1 || ![r.size_points, r.tracking, r.line_height, r.pad].iter().all(|v| v.is_finite())
        || r.features & !TextStyle::FEATURE_TABULAR_NUMBERS != 0 { return None; }
    let on = |bit: u32| r.set & bit != 0;
    // decoded straight into the fields the engine reads: one pass, no builder chain
    let mut l = LabelFields::UNSET;
    l.font = on(S::SET_FONT).then_some(r.font);
    l.size = on(S::SET_SIZE).then_some(r.size_points);
    l.weight = on(S::SET_WEIGHT).then_some(r.weight);
    l.tracking = on(S::SET_TRACKING).then_some(r.tracking);
    l.caps = on(S::SET_CAPS).then_some(r.caps != 0);
    l.color = on(S::SET_COLOR).then_some(pxui_types::theme::ColorToken(r.color));
    l.width = if on(S::SET_WIDTH) { Some(crate::node::size(r.width)?) } else { None };
    l.line_height = on(S::SET_LINE_HEIGHT).then_some(r.line_height);
    l.pad = on(S::SET_PAD).then_some(r.pad);
    l.wrap = wrap_of(r.wrap)?;
    l.ellipsis = ellipsis_of(r.ellipsis)?;
    l.align = align_of(r.align)?;
    l.features = r.features;
    l.max_lines = r.max_lines;
    Some((l, r.digits != 0))
}

export!(trusted pub unsafe fn pxui_button(ui: *mut PxuiUi, label: Str, out: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let (Some(label), false) = (unsafe { crate::state::checked_label(label) }, out.is_null()) else { return unsafe { fail(ui, "invalid button arguments") } };
    unsafe { trusted::pxui_button(ui, pair(label), out) }
});

export!(trusted pub unsafe fn pxui_toggle(ui: *mut PxuiUi, label: Str, on: u8, out: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let (Some(label), false) = (unsafe { crate::state::checked_label(label) }, out.is_null() || on > 1) else { return unsafe { fail(ui, "invalid toggle arguments") } };
    unsafe { trusted::pxui_toggle(ui, pair(label), on, out) }
});

export!(trusted pub unsafe fn pxui_text_field(ui: *mut PxuiUi, label: Str, value: Str, changed: *mut Str, out_changed: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if changed.is_null() || out_changed.is_null() { return unsafe { fail(ui, "null text field output") }; }
    let (Some(label), Some(value)) = (unsafe { crate::state::checked_label(label) }, unsafe { crate::state::checked_text(value) }) else { return unsafe { fail(ui, "invalid text field arguments") } };
    unsafe { trusted::pxui_text_field(ui, pair(label), pair(value), changed, out_changed) }
});

export!(build pub unsafe fn pxui_text_field_with_options(ui: *mut PxuiUi, label: Str, value: Str, options: *const pxui_types::PxuiTextInputOptions, changed: *mut Str, out_changed: *mut u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if options.is_null() || changed.is_null() || out_changed.is_null() { return unsafe { fail(ui,"null text field options or output") }; }
    let (Some(label),Some(value))=(unsafe { crate::state::checked_label(label) },unsafe { crate::state::checked_text(value) }) else { return unsafe { fail(ui,"invalid text field text") }; };
    let o=unsafe { *options };
    let Some(options)=pxui_types::TextTraits::from_numbers(o.autocorrect,o.capitalization,o.action) else { return unsafe { fail(ui,"invalid text input options") }; };
    unsafe { *out_changed=0; *changed=Str::default(); }
    if let Some(value) = unsafe { &mut *state.engine }.text_field_keyed(label,label,value,options) {
        let Ok(value)=state.give(value) else { return PXUI_ERR_STATE };
        unsafe { *changed=value; *out_changed=1; }
    }
    0
});

// ABI 1.64: a text field keyed by `desc.key` and named `desc.name` to a screen reader (unnamed when empty); `changed` takes the new text when
// `out_changed` is 1. The record is size-prefixed: a shorter one, an out-of-range trait or text that is not UTF-8 is refused before the widget runs.
export!(build pub unsafe fn pxui_ui_text_field_keyed(ui: *mut PxuiUi, desc: *const pxui_types::ffi::PxuiTextFieldDesc, changed: *mut Str, out_changed: *mut u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() || changed.is_null() || out_changed.is_null() { return unsafe { fail(ui, "null text field descriptor or output") }; }
    if unsafe { (*desc).size } < core::mem::size_of::<pxui_types::ffi::PxuiTextFieldDesc>() as u32 { return unsafe { fail(ui, "invalid text field descriptor") }; }
    let d = unsafe { *desc };
    let (Some(key), Some(name), Some(value)) = (unsafe { crate::state::checked_key(d.key) }, unsafe { checked(d.name) }, unsafe { crate::state::checked_text(d.value) }) else { return unsafe { fail(ui, "invalid text field text") }; };
    let Some(traits) = pxui_types::TextTraits::from_numbers(d.autocorrect as u32, d.capitalization as u32, d.action as u32).filter(|_| d.reserved == 0) else { return unsafe { fail(ui, "invalid text input options") }; };
    unsafe { *out_changed = 0; *changed = Str::default(); }
    if let Some(text) = unsafe { &mut *state.engine }.text_field_keyed(key, name, value, traits) {
        let Ok(text) = state.give(text) else { return PXUI_ERR_STATE };
        unsafe { *changed = text; *out_changed = 1; }
    }
    0
});

// ABI 1.82: `pxui_ui_text_field_keyed` that also reports what happened to the field's box in `response`, the result a caller would otherwise
// read with `pxui_ui_input_box`, `pxui_ui_signal` and `pxui_ui_focused`. The descriptor and outputs are checked as that export checks them; the
// trusted entry checks the handle (once: the export does not count the call itself).
export!(trusted pub unsafe fn pxui_ui_text_field_response(ui: *mut PxuiUi, desc: *const pxui_types::ffi::PxuiTextFieldDesc, changed: *mut Str, out_changed: *mut u8, response: *mut pxui_types::ffi::PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if desc.is_null() || changed.is_null() || out_changed.is_null() { return unsafe { fail(ui, "null text field descriptor or output") }; }
    let Some(response) = sized_mut!(response) else { return unsafe { fail(ui, "null or short text field response") } };
    if unsafe { (*desc).size } < core::mem::size_of::<pxui_types::ffi::PxuiTextFieldDesc>() as u32 { return unsafe { fail(ui, "invalid text field descriptor") }; }
    let d = unsafe { *desc };
    let (Some(key), Some(name), Some(value)) = (unsafe { crate::state::checked_key(d.key) }, unsafe { checked(d.name) }, unsafe { crate::state::checked_text(d.value) }) else { return unsafe { fail(ui, "invalid text field text") }; };
    let Some(traits) = pxui_types::TextTraits::from_numbers(d.autocorrect as u32, d.capitalization as u32, d.action as u32).filter(|_| d.reserved == 0) else { return unsafe { fail(ui, "invalid text input options") }; };
    unsafe { *out_changed = 0; *changed = Str::default(); }
    let mut text = None;
    let rc = unsafe { trusted::text_field(ui, key, name, value, traits, &mut text, response) };
    if let Some(text) = text { unsafe { *changed = text; *out_changed = 1; } }
    rc
});

// ABI 1.64: the box of the text input the last text field, line field, search field, text area or value field call built.
export!(pub unsafe fn pxui_ui_input_box(ui: *mut PxuiUi, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null text input box output") }; }
    match unsafe { &*state.engine }.input_box() {
        Some(b) if unsafe { &*state.engine }.live(&b) => { unsafe { *out = crate::state::to_abi_box(b); } 0 }
        _ => PXUI_ERR_STATE,
    }
});

#[cfg(test)]
mod tests {
    use pxui_types::Event as C;

    /// Runs `f` on the borrowed handle of one frame of `ui`: builders are refused outside a frame (`PXUI_ERR_STATE`), so argument checks
    /// are tested inside one.
    fn in_frame(ui: *mut super::PxuiUi, f: &mut dyn FnMut(*mut super::PxuiUi)) {
        extern "C" fn run(user: *mut core::ffi::c_void, ui: *mut core::ffi::c_void) { unsafe { (*user.cast::<&mut dyn FnMut(*mut super::PxuiUi)>())(ui.cast()) } }
        let mut f = f;
        let mut out = crate::FrameOut::default();
        let input = crate::FrameIn { width: 320.0, height: 240.0, ..Default::default() };
        assert_eq!(unsafe { super::pxui_frame(ui, &input, Some(run), (&mut f as *mut &mut dyn FnMut(*mut super::PxuiUi)).cast(), &mut out) }, 0);
    }

    #[test]
    fn the_keyed_text_field_refuses_invalid_descriptors_before_output_or_widget_mutation() {
        use super::*;
        use pxui_types::ffi::PxuiTextFieldDesc;
        let ui=unsafe {pxui_ui_create(&PxuiUiDesc::default())};
        let text=|s:&'static str| Str {ptr:s.as_ptr(),len:s.len()};
        let good=PxuiTextFieldDesc {size:core::mem::size_of::<PxuiTextFieldDesc>() as u32,autocorrect:1,capitalization:1,action:0,reserved:0,key:text("k"),name:text("n"),value:text("v")};
        let bad_utf8=Str {ptr:[0xff_u8].as_ptr(),len:1};
        let cases=[
            PxuiTextFieldDesc {size:core::mem::size_of::<PxuiTextFieldDesc>() as u32-1,..good},
            PxuiTextFieldDesc {size:0,..good},
            PxuiTextFieldDesc {autocorrect:2,..good},
            PxuiTextFieldDesc {capitalization:4,..good},
            PxuiTextFieldDesc {action:3,..good},
            PxuiTextFieldDesc {reserved:1,..good},
            PxuiTextFieldDesc {key:bad_utf8,..good},
            PxuiTextFieldDesc {name:bad_utf8,..good},
            PxuiTextFieldDesc {value:bad_utf8,..good},
            PxuiTextFieldDesc {key:Str {ptr:core::ptr::null(),len:1},..good},
        ];
        let mut changed=Str {ptr:b"sentinel".as_ptr(),len:8};
        let mut flag=77;
        // outside a frame the builder is refused before its descriptor is read
        assert_eq!(unsafe {pxui_ui_text_field_keyed(ui,&good,&mut changed,&mut flag)},PXUI_ERR_STATE);
        in_frame(ui,&mut |ui| {
            let built=unsafe {&*(*ui).engine}.built_nodes();
            for (i,desc) in cases.iter().enumerate() {
                // a refused call must not leave the Ui poisoned or with a node
                assert_eq!(unsafe {pxui_ui_text_field_keyed(ui,desc,&mut changed,&mut flag)},PXUI_ERR_ARGUMENT,"case {i}");
                assert_eq!((changed.len,flag),(8,77),"case {i} left its output alone");
                assert_eq!(unsafe {&*(*ui).engine}.built_nodes(),built,"case {i} built nothing");
            }
            assert_eq!(unsafe {pxui_ui_text_field_keyed(ui,core::ptr::null(),&mut changed,&mut flag)},PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe {pxui_ui_text_field_keyed(ui,&good,core::ptr::null_mut(),&mut flag)},PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe {pxui_ui_text_field_keyed(ui,&good,&mut changed,core::ptr::null_mut())},PXUI_ERR_ARGUMENT);
        });
        assert_eq!(unsafe {pxui_ui_text_field_keyed(core::ptr::null_mut(),&good,&mut changed,&mut flag)},PXUI_ERR_ARGUMENT);
        assert_eq!((changed.len,flag),(8,77));
        // no input has been built yet, and a null output is refused
        let mut raw=PxuiBox::default();
        assert_eq!(unsafe {pxui_ui_input_box(ui,&mut raw)},PXUI_ERR_STATE);
        assert_eq!(unsafe {pxui_ui_input_box(ui,core::ptr::null_mut())},PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe {pxui_ui_input_box(core::ptr::null_mut(),&mut raw)},PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe {pxui_ui_destroy(ui)},0);
    }

    /// Each text input's `_response` export reports what `pxui_ui_input_box`, `pxui_ui_signal` and `pxui_ui_focused` read of the box it
    /// built, in the one call; a short response record is refused before the widget runs.
    #[test]
    fn text_input_response_exports_report_their_box_in_one_call() {
        use super::*;
        use crate::kit_ext::pxui_ui_line_field_response;
        use crate::text_inputs::{pxui_ui_text_area_response, pxui_ui_value_field_response};
        use crate::widgets::pxui_ui_search_field_response;
        use pxui_types::ffi::{PxuiResponse, PxuiTextAreaDesc, PxuiTextFieldDesc, PxuiValueFieldDesc};
        let s=|t: &'static str| Str {ptr:t.as_ptr(),len:t.len()};
        let ui=unsafe {pxui_ui_create(&PxuiUiDesc::default())};
        let field=PxuiTextFieldDesc {size:core::mem::size_of::<PxuiTextFieldDesc>() as u32,autocorrect:0,capitalization:0,action:0,reserved:0,key:s("f"),name:s("Field"),value:s("abc")};
        let value=PxuiValueFieldDesc {label:s("v"),value:s("1"),width:40.0,height:20.0,size:core::mem::size_of::<PxuiValueFieldDesc>() as u32,..unsafe {core::mem::zeroed()}};
        let area=PxuiTextAreaDesc {label:s("a"),value:s("x"),size:core::mem::size_of::<PxuiTextAreaDesc>() as u32,..unsafe {core::mem::zeroed()}};
        for frame in 0..2 {
            in_frame(ui,&mut |ui| {
                let calls: [&dyn Fn(*mut PxuiResponse)->i32; 5]=[
                    &|r| unsafe {pxui_ui_text_field_response(ui,&field,&mut Str::default(),&mut 0,r)},
                    &|r| unsafe {pxui_ui_search_field_response(ui,s("s"),s("/"),s("q"),s(""),s("Find"),2,&mut 0,&mut Str::default(),r)},
                    &|r| unsafe {pxui_ui_line_field_response(ui,0.0,30.0,80.0,20.0,0,s(""),s("l"),s("x"),s("Line"),&mut Str::default(),&mut 0,r)},
                    &|r| unsafe {pxui_ui_value_field_response(ui,&value,None,None,None,core::ptr::null_mut(),s("Scale"),&mut Str::default(),&mut 0,&mut 0,r)},
                    &|r| unsafe {pxui_ui_text_area_response(ui,&area,&pxui_types::Node::DEFAULT.w_px(80.0).h_px(40.0),core::ptr::null(),s("Notes"),&mut Str::default(),&mut 0,&mut 0,r)},
                ];
                for (i,call) in calls.iter().enumerate() {
                    let built=unsafe {&*(*ui).engine}.built_nodes();
                    let mut short=PxuiResponse {size:4,..PxuiResponse::EMPTY};
                    assert_eq!(call(&mut short),PXUI_ERR_ARGUMENT,"input {i}: a short record is refused");
                    assert_eq!(unsafe {&*(*ui).engine}.built_nodes(),built,"input {i}: refused before the widget runs");
                    let mut out=PxuiResponse::EMPTY;
                    assert_eq!(call(&mut out),0,"input {i}");
                    let (mut raw,mut signal,mut focused)=(PxuiBox::default(),crate::PxuiSignal::default(),7u8);
                    assert_eq!(unsafe {pxui_ui_input_box(ui,&mut raw)},0);
                    assert_eq!(unsafe {crate::interaction::pxui_ui_signal(ui,&raw,&mut signal)},0);
                    assert_eq!(unsafe {crate::interaction::pxui_ui_focused(ui,&raw,&mut focused)},0);
                    assert_eq!((out.node.box_key,out.node.generation),(raw.box_key,raw.generation),"input {i}, frame {frame}: its own box");
                    let hovered=signal.flags&crate::PxuiSignal::HOVERED!=0;
                    assert_eq!((out.flags&PxuiResponse::HOVERED!=0,out.flags&PxuiResponse::FOCUSED!=0),(hovered,focused!=0),"input {i}");
                }
            });
        }
        assert_eq!(unsafe {pxui_ui_destroy(ui)},0);
    }

    #[test]
    fn a_text_area_refuses_negative_offsets_before_output_or_widget_mutation() {
        use super::*;
        use pxui_types::ffi::{PxuiColorSpan, PxuiNote, PxuiRange, PxuiSplice, PxuiTextAreaDesc};
        let ui=unsafe {pxui_ui_create(&PxuiUiDesc::default())};
        let base=|| -> PxuiTextAreaDesc { let mut d: PxuiTextAreaDesc = unsafe { core::mem::zeroed() }; d.size=core::mem::size_of::<PxuiTextAreaDesc>() as u32; d.width=10.0; d.height=10.0; d };
        let negative=[-1_i64];
        let ranges=[PxuiRange {start:-1,end:2}];
        let notes=[PxuiNote {line:0,start:-1,end:1,has_range:1,reserved:[0;7],text:Str::default()}];
        let splices=[PxuiSplice {start:0,end:-3,text:Str::default()}];
        let chips=[PxuiColorSpan {start:-2,end:1,color:Default::default(),reserved:0}];
        let mut cases=Vec::new();
        let mut d=base(); d.flags=PxuiTextAreaDesc::HAS_REVEAL; d.reveal=-1; cases.push(d);
        let mut d=base(); d.flags=PxuiTextAreaDesc::HAS_SELECT; d.select=PxuiRange {start:0,end:-1}; cases.push(d);
        let mut d=base(); d.errors=pxui_types::ffi::Slice {ptr:negative.as_ptr(),len:1}; cases.push(d);
        let mut d=base(); d.spans=pxui_types::ffi::Slice {ptr:ranges.as_ptr(),len:1}; cases.push(d);
        let mut d=base(); d.messages=pxui_types::ffi::Slice {ptr:notes.as_ptr(),len:1}; cases.push(d);
        let mut d=base(); d.splice=pxui_types::ffi::Slice {ptr:splices.as_ptr(),len:1}; cases.push(d);
        let mut d=base(); d.chips=pxui_types::ffi::Slice {ptr:chips.as_ptr(),len:1}; cases.push(d);
        let (mut out,mut changed,mut submitted)=(Str {ptr:b"sentinel".as_ptr(),len:8},7u8,9u8);
        in_frame(ui,&mut |ui| {
            let built=unsafe {&*(*ui).engine}.built_nodes();
            for (i,desc) in cases.iter().enumerate() {
                assert_eq!(unsafe {crate::text_inputs::pxui_ui_text_area_submit(ui,desc,core::ptr::null(),&mut out,&mut changed,&mut submitted)},PXUI_ERR_ARGUMENT,"case {i}");
                assert_eq!(unsafe {&*(*ui).engine}.built_nodes(),built,"case {i} built nothing");
            }
        });
        assert_eq!(unsafe {pxui_ui_destroy(ui)},0);
    }

    #[test]
    fn text_input_options_reject_invalid_calls_before_output_or_widget_mutation() {
        use super::*;
        extern "C" fn invalid(_: *mut c_void, ui: *mut c_void) {
            let ui=ui.cast::<PxuiUi>();
            let mut changed=Str {ptr:b"sentinel".as_ptr(),len:8};
            let mut flag=77;
            let built=unsafe {&*(*ui).engine}.built_nodes();
            for options in [
                pxui_types::PxuiTextInputOptions {autocorrect:2,..Default::default()},
                pxui_types::PxuiTextInputOptions {capitalization:4,..Default::default()},
                pxui_types::PxuiTextInputOptions {action:3,..Default::default()},
            ] {
                assert_eq!(unsafe {pxui_text_field_with_options(ui,Str::default(),Str::default(),&options,&mut changed,&mut flag)},PXUI_ERR_ARGUMENT);
                assert_eq!(changed.len,8); assert_eq!(flag,77);
                assert_eq!(unsafe {&*(*ui).engine}.built_nodes(),built);
            }
            assert_eq!(unsafe {pxui_text_field_with_options(ui,Str::default(),Str::default(),core::ptr::null(),&mut changed,&mut flag)},PXUI_ERR_ARGUMENT);
            assert_eq!(changed.len,8); assert_eq!(flag,77);
        }
        let ui=unsafe {pxui_ui_create(&PxuiUiDesc::default())};
        // outside a frame the builder is refused before its arguments are read
        let (mut changed,mut flag)=(Str {ptr:b"sentinel".as_ptr(),len:8},77);
        let options=pxui_types::PxuiTextInputOptions::default();
        assert_eq!(unsafe {pxui_text_field_with_options(ui,Str::default(),Str::default(),&options,&mut changed,&mut flag)},PXUI_ERR_STATE);
        assert_eq!(changed.len,8); assert_eq!(flag,77);
        let mut out=crate::FrameOut::default();
        assert_eq!(unsafe {pxui_frame(ui,&FrameIn {width:320.0,height:240.0,..Default::default()},Some(invalid),core::ptr::null_mut(),&mut out)},0);
        assert_eq!(unsafe {crate::ui_state::pxui_ui_text_input_options(ui,core::ptr::null_mut())},PXUI_ERR_ARGUMENT);
        let mut options=pxui_types::PxuiTextInputOptions::from(pxui_types::TextTraits::SEARCH);
        assert_eq!(unsafe {crate::ui_state::pxui_ui_text_input_options(core::ptr::null_mut(),&mut options)},PXUI_ERR_ARGUMENT);
        assert_eq!(options,pxui_types::PxuiTextInputOptions::from(pxui_types::TextTraits::SEARCH));
        assert_eq!(unsafe {pxui_ui_destroy(ui)},0);
    }

    /// The frame entry's fast paths answer what the general ones do: a repeated environment or keyboard is recognised bytewise (and a
    /// changed one, a NaN one included, is not), and a moved pointer decodes as the wire record does.
    #[test]
    fn frame_fast_paths_match_the_general_decoding_and_comparison() {
        use pxui_types::{Environment, Keyboard};
        let env = Environment { safe_area: [47.0, 0.0, 34.0, 0.0], keyboard_frame: [0.0, 500.0, 390.0, 344.0], keyboard_started: 1.0, keyboard_duration: 0.25, keyboard_curve: 7, keyboard_visible: 1, ..Environment::default() };
        assert!(super::same_environment(&env, &env.clone()) && super::same_environment(&Environment::default(), &super::NO_ENVIRONMENT));
        for change in 0..4 {
            let mut other = env;
            match change { 0 => other.safe_area[0] = 48.0, 1 => other.keyboard_started = 1.5, 2 => other.keyboard_visible = 0, _ => other.keyboard_from[3] = f32::NAN }
            assert!(!super::same_environment(&env, &other), "change {change}");
        }
        let keyboard = Keyboard { frame: [0.0, 500.0, 390.0, 344.0], flags: Keyboard::VALID };
        assert!(super::same_keyboard(&keyboard, &keyboard.clone()) && !super::same_keyboard(&keyboard, &Keyboard { flags: 0, ..keyboard }));
        // a pointer that is not finite is refused before either path
        for (x, y) in [(1.5, -2.0), (f64::NAN, 0.0), (f64::INFINITY, -0.0)] {
            let event = C { kind: C::POINTER_MOVED, x, y, ..Default::default() };
            let general = input::Event::from_raw(&input::wire::Raw { kind: event.kind, code: event.code, x, y, value: event.value, text: unsafe { super::text(event.text) }, touch: None })
                .filter(|_| x.is_finite() && y.is_finite());
            let got = super::translate_event(&event, &mut 0);
            assert_eq!(format!("{got:?}"), format!("{general:?}"));
        }
    }

    #[test]
    fn snapshot_retains_geometry_timing_and_held_input_after_rejected_spans() {
        use super::*;
        let ui = unsafe { pxui_ui_create(&PxuiUiDesc::default()) };
        let keys = [Key::Meta.code()];
        let buttons = [MouseButton::LeftButton.code()];
        let snapshot = Input {
            logical_size: [320, 240], pixel_scale: [2.0, 3.0],
            pointer: [42.0, 35.0], mouse_delta: [8.0, 9.0],
            time: 0.25, dt: 0.125, frame: 7, tick: 11, fps: 120.0,
            keys: Slice { ptr: keys.as_ptr(), len: keys.len() },
            buttons: Slice { ptr: buttons.as_ptr(), len: buttons.len() },
            ..Default::default()
        };
        let mut out = FrameOut::default();
        assert_eq!(unsafe { pxui_ui_frame(ui, &snapshot, None, core::ptr::null_mut(), &mut out) }, 0);
        let invalid = [Key::Alt.code(), 0];
        let bad = Input { keys: Slice { ptr: invalid.as_ptr(), len: invalid.len() }, ..snapshot };
        out.size = 777;
        assert_eq!(unsafe { pxui_ui_frame(ui, &bad, None, core::ptr::null_mut(), &mut out) }, PXUI_ERR_ARGUMENT);
        assert_eq!(out.size, 777);
        let frame = unsafe { &*(*ui).shared }.spare_input.as_ref().unwrap();
        let input = &frame.input;
        assert_eq!((input.t, input.dt, input.frame, input.tick), (0.25, 0.125, 7, 11));
        assert_eq!((input.size, input.pointer), ((320, 240), (42.0, 35.0)));
        assert_eq!((frame.pixel_scale, frame.fps, frame.mouse_delta), ((2.0, 3.0), 120.0, (8.0, 9.0)));
        assert_eq!(input.keys, [Key::Meta]);
        assert_eq!(input.buttons, [MouseButton::LeftButton]);
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    }

    #[test]
    fn completed_file_drop_events_preserve_paths_order_and_warm_storage() {
        use super::*;
        use pxui_types::Event as Wire;
        extern "C" fn mark_built(user: *mut c_void, _: *mut c_void) {
            unsafe { user.cast::<bool>().write(true); }
        }
        let ui = unsafe { pxui_ui_create(&PxuiUiDesc::default()) };
        let mut out = FrameOut::default();
        for (code, payload) in [(0, "/Music/名前.flac\0/Music/Björk.m4a"), (1, "transport limit exceeded"), (0, "")] {
            let backing = payload.to_owned();
            let event = Wire { kind: Wire::FILES_DROPPED, code, x: -12.5, y: 31.25, text: pair(&backing), ..Default::default() };
            let frame = FrameIn { events: &event, event_count: 1, ..FrameIn::new(320.0, 240.0) };
            assert_eq!(unsafe { pxui_frame(ui, &frame, None, core::ptr::null_mut(), &mut out) }, 0);
            drop(backing);
            for (has_owner, owner) in [(0, 0), (1, 123)] {
                let mut routed = Input::default();
                assert_eq!(unsafe { pxui_ui_input(ui, has_owner, owner, &mut routed) }, 0);
                assert_eq!(routed.events.len, 1);
                let event = unsafe { routed.events.ptr.read() };
                assert_eq!((event.kind, event.code, event.x, event.y), (Wire::FILES_DROPPED, code, -12.5, 31.25));
                assert_eq!(unsafe { text(event.text) }, Some(payload));
            }
        }
        let bad_utf8 = [255u8];
        let empty = Wire { kind: Wire::FILES_DROPPED, ..Default::default() };
        for invalid in [
            Wire { code: 2, ..empty },
            Wire { x: f64::NAN, ..empty },
            Wire { y: f64::INFINITY, ..empty },
            Wire { text: Str { ptr: core::ptr::null(), len: 1 }, ..empty },
            Wire { text: Str { ptr: bad_utf8.as_ptr(), len: 1 }, ..empty },
        ] {
            let frame = FrameIn { events: &invalid, event_count: 1, ..FrameIn::new(320.0, 240.0) };
            let mut built = false;
            out.size = 777;
            assert_eq!(unsafe {
                pxui_frame(ui, &frame, Some(mark_built), core::ptr::from_mut(&mut built).cast(), &mut out)
            }, PXUI_ERR_ARGUMENT);
            assert!(!built);
            assert_eq!(out.size, 777);
        }
        let frame = FrameIn { events: &empty, event_count: 1, ..FrameIn::new(320.0, 240.0) };
        for round in 0..20 {
            mem_sys::local_reset();
            assert_eq!(unsafe { pxui_frame(ui, &frame, None, core::ptr::null_mut(), &mut out) }, 0);
            let mut routed = Input::default();
            assert_eq!(unsafe { pxui_ui_input(ui, 0, 0, &mut routed) }, 0);
            assert_eq!(routed.events.len, 1);
            if round > 2 { assert_eq!(mem_sys::local_alloc_count(), 0); }
        }
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    }

    #[test]
    fn typed_touch_batches_keep_ids_phases_lifetimes_and_zero_warm_allocations() {
        use super::*;
        use pxui_types::{Event as Wire, Touch};
        let ui = unsafe { pxui_ui_create(&PxuiUiDesc::default()) };
        let mut packets = [Touch::default(); 32];
        let mut events = [Wire::default(); 32];
        let mut out = FrameOut::default();
        for tick in 0..182 {
            let count = [32, 1, 0, 3][tick % 4];
            for (index, packet) in packets.iter_mut().enumerate() {
                *packet = Touch { device: u64::MAX, finger: u64::MAX - index as u64, x: -1.25, y: 99.5,
                    time: tick as f64 / 120.0, pressure: 0.375, phase: (tick % 4) as u32 };
                events[index] = Wire { kind: Wire::TOUCH,
                    text: Str { ptr: (packet as *const Touch).cast(), len: core::mem::size_of::<Touch>() },
                    ..Default::default() };
            }
            let expected = packets;
            let frame = FrameIn { time: tick as f64 / 120.0, events: events.as_ptr(), event_count: count, ..FrameIn::new(320.0, 240.0) };
            mem_sys::local_reset();
            assert_eq!(unsafe { pxui_frame(ui, &frame, None, core::ptr::null_mut(), &mut out) }, 0);
            packets.fill(Touch::default());
            let mut routed = Input::default();
            assert_eq!(unsafe { pxui_ui_input(ui, 0, 0, &mut routed) }, 0);
            assert_eq!(routed.events.len, count);
            for (index, packet) in expected[..count].iter().enumerate() {
                let event = unsafe { routed.events.ptr.add(index).read() };
                assert_eq!(event.kind, Wire::TOUCH);
                assert_eq!(touch_event(&event).map(|touch| touch.finger), Some(packet.finger));
                assert_eq!(unsafe { event.text.ptr.cast::<Touch>().read_unaligned() }, *packet);
            }
            if tick >= 82 { assert_eq!(mem_sys::local_alloc_count(), 0); }
        }
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    }

    #[test]
    fn typed_touch_payloads_round_trip_full_ids_and_reject_invalid_tails_atomically() {
        use pxui_types::{Touch,Event as C,FrameIn,Str};
        let ui=unsafe {super::pxui_ui_create(&crate::PxuiUiDesc::default())};
        let mut packet=Touch {device:u64::MAX,finger:0xfedcba9876543210,x:-1.25,y:99.5,time:10.0,pressure:0.375,phase:0};
        let mut out=crate::FrameOut::default();
        for frame in 0..182 {
            packet.phase=(frame%4) as u32;
            let event=C {kind:C::TOUCH,text:Str {ptr:(&packet as *const Touch).cast(),len:core::mem::size_of::<Touch>()},..Default::default()};
            mem_sys::local_reset();
            assert_eq!(unsafe {super::pxui_frame(ui,&FrameIn {events:&event,event_count:1,width:320.0,height:240.0,time:frame as f64,..Default::default()},None,core::ptr::null_mut(),&mut out)},0);
            let mut input=pxui_types::ffi::Input::default();
            assert_eq!(unsafe {super::pxui_ui_input(ui,0,0,&mut input)},0);
            assert_eq!(input.events.len,1);
            let encoded=unsafe {&*input.events.ptr};
            assert_eq!(encoded.kind,C::TOUCH);
            assert_eq!(encoded.text.len,core::mem::size_of::<Touch>());
            assert_eq!(unsafe {encoded.text.ptr.cast::<Touch>().read_unaligned()},packet);
            if frame>=82 {assert_eq!(mem_sys::local_alloc_count(),0,"warm touch frame and routed read allocated");}
        }
        let input=FrameIn {width:320.0,height:240.0,..Default::default()};
        for field in 0..7 {
            let mut bad=packet;
            match field {0=>bad.phase=4,1=>bad.x=f64::NAN,2=>bad.y=f64::INFINITY,3=>bad.time=f64::NAN,4=>bad.pressure=-1.0,5=>bad.pressure=1.01,_=>bad.pressure=f32::NAN}
            let events=[C {kind:C::KEY_PRESSED,code:input::Key::Meta.code(),..Default::default()},C {kind:C::TOUCH,text:Str {ptr:(&bad as *const Touch).cast(),len:core::mem::size_of::<Touch>()},..Default::default()}];
            out.size=777;
            assert_eq!(unsafe {super::pxui_frame(ui,&FrameIn {events:events.as_ptr(),event_count:2,..input},None,core::ptr::null_mut(),&mut out)},crate::PXUI_ERR_ARGUMENT);
            assert_eq!(out.size,777);
            let engine=unsafe {&*(*ui).engine};
            assert!(!engine.key_down(input::Key::Meta));
            let snapshot=pxui_types::ffi::Input {events:pxui_types::ffi::Slice {ptr:events.as_ptr(),len:events.len()},..Default::default()};
            assert_eq!(unsafe {super::pxui_ui_frame(ui,&snapshot,None,core::ptr::null_mut(),&mut out)},crate::PXUI_ERR_ARGUMENT);
            assert_eq!(out.size,777);
            assert!(!unsafe {&*(*ui).engine}.key_down(input::Key::Meta));
            assert_eq!(unsafe {super::pxui_frame(ui,&input,None,core::ptr::null_mut(),&mut out)},0);
            assert!(!unsafe {&*(*ui).engine}.key_down(input::Key::Meta), "the rejected batch must leave the next frame's held keys intact");
        }
        for text in [Str::default(),Str {ptr:(&packet as *const Touch).cast(),len:47}] {
            let event=C {kind:C::TOUCH,text,..Default::default()};
            assert_eq!(unsafe {super::pxui_frame(ui,&FrameIn {events:&event,event_count:1,..input},None,core::ptr::null_mut(),&mut out)},crate::PXUI_ERR_ARGUMENT);
        }
        assert_eq!(unsafe {super::pxui_ui_destroy(ui)},0);
    }

    #[test]
    fn host_environment_rejects_invalid_geometry_before_mutating_a_frame() {
        use pxui_types::{Environment,FrameIn};
        let ui=unsafe { super::pxui_ui_create(&crate::PxuiUiDesc::default()) };
        let value=Environment { safe_area:[47.0,0.0,34.0,0.0],keyboard_frame:[0.0,500.0,390.0,344.0],keyboard_from:[0.0,844.0,390.0,344.0],
            keyboard_started:1.0,keyboard_duration:0.25,keyboard_curve:7,keyboard_visible:1 };
        let input=FrameIn { width:390.0,height:844.0,environment:value,..Default::default() };
        let mut out=crate::FrameOut::default();
        assert_eq!(unsafe {super::pxui_frame(ui,&input,None,core::ptr::null_mut(),&mut out)},0);
        let mut got=Environment::default();
        assert_eq!(unsafe {crate::ui_state::pxui_ui_environment(ui,&mut got)},0);
        assert_eq!(got,value);
        assert_eq!(unsafe {crate::ui_state::pxui_ui_environment(core::ptr::null_mut(),&mut got)},crate::PXUI_ERR_ARGUMENT);
        assert_eq!(got,value);
        assert_eq!(unsafe {crate::ui_state::pxui_ui_environment(ui,core::ptr::null_mut())},crate::PXUI_ERR_ARGUMENT);
        for field in 0..12 {
            let mut bad=value;
            match field {
                0=>bad.safe_area[0]=f32::NAN, 1=>bad.safe_area[2]=-1.0, 2=>bad.safe_area=[f32::MAX;4],
                3=>bad.keyboard_frame[0]=f32::INFINITY, 4=>bad.keyboard_frame[2]=-1.0, 5=>bad.keyboard_from[3]=-1.0,
                6=>bad.keyboard_from[1]=f32::NAN, 7=>bad.keyboard_frame=[f32::MAX;4], 8=>bad.keyboard_started=f64::NAN,
                9=>bad.keyboard_duration=-0.1, 10=>bad.keyboard_duration=f64::INFINITY, _=>bad.keyboard_visible=2,
            }
            out.size=123;
            assert_eq!(unsafe {super::pxui_frame(ui,&FrameIn {environment:bad,..input},None,core::ptr::null_mut(),&mut out)},crate::PXUI_ERR_ARGUMENT);
            assert_eq!(out.size,123);
            assert_eq!(unsafe {crate::ui_state::pxui_ui_environment(ui,&mut got)},0);
            assert_eq!(got,value);
        }
        for size in [56] {
            let raw=FrameIn { size,environment:Environment {keyboard_visible:2,..value},..input };
            let decoded=unsafe {super::read_frame(&raw)}.unwrap();
            assert_eq!(decoded.environment,Environment::default());
        }
        assert_eq!(unsafe {super::pxui_ui_destroy(ui)},0);
    }

    #[test]
    fn system_frame_tail_accepts_legacy_without_reading_past_it() {
        use pxui_types::FrameIn;
        #[repr(C)]
        struct Legacy { size:u32,events:*const pxui_types::Event,count:usize,width:f32,height:f32,scale:f32,time:f64 }
        let legacy=Legacy { size:48,events:core::ptr::null(),count:0,width:320.0,height:240.0,scale:1.0,time:9.0 };
        assert_eq!(core::mem::size_of::<Legacy>(),48);
        let decoded=unsafe { super::read_frame((&legacy as *const Legacy).cast()) }.unwrap();
        assert_eq!((decoded.size,decoded.width,decoded.time,decoded.appearance,decoded.reduced_motion),(48,320.0,9.0,0,0));
        let ui=unsafe { super::pxui_ui_create(&crate::PxuiUiDesc::default()) };
        assert!(!ui.is_null());
        let mut out=crate::FrameOut::default();
        assert_eq!(unsafe { super::pxui_frame(ui,(&legacy as *const Legacy).cast(),None,core::ptr::null_mut(),&mut out) },0);
        let mut appearance=99;
        assert_eq!(unsafe { super::pxui_ui_system_appearance(ui,&mut appearance) },0);
        assert_eq!(appearance,0);
        assert_eq!(unsafe { super::pxui_ui_system_appearance(ui,core::ptr::null_mut()) },crate::PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { super::pxui_ui_destroy(ui) },0);
        let short=[4u32];
        assert!(unsafe { super::read_frame(short.as_ptr().cast()) }.is_none());
        assert!(unsafe { super::read_frame(core::ptr::null()) }.is_none());
        for size in 0..56 {
            let input=FrameIn { size,..Default::default() };
            assert_eq!(unsafe { super::read_frame(&input) }.is_some(),size==48);
        }
        for size in [56,128,152,160,u32::MAX] {
            let input=FrameIn { size,appearance:2,reduced_motion:1,..Default::default() };
            let decoded=unsafe { super::read_frame(&input) }.unwrap();
            assert_eq!((decoded.appearance,decoded.reduced_motion),(2,1));
        }
    }

    /// The numbering `input` encodes with is the one the C header and `pxui_types` document.
    #[test]
    fn wire_numbers_are_the_abi_numbers() {
        use input::wire as w;
        assert_eq!((w::KEY_SPECIAL, w::KEY_UNKNOWN), (pxui_types::KEY_SPECIAL, pxui_types::KEY_UNKNOWN));
        let pairs = [(w::KEY_PRESSED, C::KEY_PRESSED), (w::KEY_RELEASED, C::KEY_RELEASED), (w::POINTER_MOVED, C::POINTER_MOVED), (w::POINTER_PRESSED, C::POINTER_PRESSED),
            (w::POINTER_RELEASED, C::POINTER_RELEASED), (w::POINTER_CANCELLED, C::POINTER_CANCELLED), (w::SCROLLED, C::SCROLLED), (w::TEXT_INPUT, C::TEXT_INPUT),
            (w::PASTE, C::PASTE), (w::TEXT_EDITING, C::TEXT_EDITING), (w::FILE_DROPPED, C::FILE_DROPPED), (w::FILE_DRAG_MOVED, C::FILE_DRAG_MOVED),
            (w::FILE_DRAG_ENDED, C::FILE_DRAG_ENDED), (w::PINCHED, C::PINCHED), (w::FILE_DIALOG, C::FILE_DIALOG), (w::DIALOG_OPENED, C::DIALOG_OPENED),
            (w::RESIZED, C::RESIZED), (w::FOCUS_LOST, C::FOCUS_LOST), (w::FOCUS_GAINED, C::FOCUS_GAINED), (w::CLOSED, C::CLOSED),
            (w::PIXEL_SIZE_CHANGED, C::PIXEL_SIZE_CHANGED), (w::VISIBILITY_CHANGED, C::VISIBILITY_CHANGED), (w::TRACKPAD_SCROLLED, C::TRACKPAD_SCROLLED), (w::ENVIRONMENT_CHANGED, C::ENVIRONMENT_CHANGED), (w::TOUCH, C::TOUCH), (w::FILES_DROPPED,C::FILES_DROPPED)];
        for (wire, abi) in pairs { assert_eq!(wire, abi); }
    }
}

#[cfg(test)]
mod keyboard_tests {
    use super::*;
    #[test]
    fn surface_is_reserved_and_the_152_byte_record_still_carries_its_keyboard() {
        use pxui_types::Keyboard;
        let ui=unsafe {super::pxui_ui_create(&crate::PxuiUiDesc::default())};
        let keyboard=Keyboard {frame:[0.0,508.0,390.0,336.0],flags:Keyboard::VALID};
        let input=FrameIn {width:390.0,height:844.0,keyboard,..FrameIn::default()};
        assert_eq!(core::mem::offset_of!(FrameIn,surface),152);
        let mut out=FrameOut::default();
        let mut got=Keyboard::default();
        // a 152-byte caller (built before `surface`): its keyboard is read, the surface is absent
        let old=FrameIn {size:152,surface:9,..input};
        assert_eq!(unsafe {super::pxui_frame(ui,&old,None,core::ptr::null_mut(),&mut out)},0);
        assert_eq!(unsafe {crate::ui_state::pxui_ui_keyboard(ui,&mut got)},0);
        assert_eq!(got,keyboard);
        // a full record with any surface behaves as surface 0
        for surface in [0,1,u64::MAX] {
            assert_eq!(unsafe {super::pxui_frame(ui,&FrameIn {surface,..input},None,core::ptr::null_mut(),&mut out)},0);
            assert_eq!(unsafe {crate::ui_state::pxui_ui_keyboard(ui,&mut got)},0);
            assert_eq!(got,keyboard);
        }
    }

    #[test]
    fn keyboard_tail_validates_before_mutation_and_preserves_all_legacy_prefixes() {
        use pxui_types::{Environment, Keyboard};
        let ui=unsafe {super::pxui_ui_create(&crate::PxuiUiDesc::default())};
        let keyboard=Keyboard {frame:[0.0,508.0,390.0,336.0],flags:Keyboard::VALID};
        let environment=Environment {safe_area:[47.0,0.0,34.0,0.0],..Environment::default()};
        let input=FrameIn {width:390.0,height:844.0,environment,keyboard,..FrameIn::default()};
        let mut out=FrameOut::default();
        assert_eq!(unsafe {super::pxui_frame(ui,&input,None,core::ptr::null_mut(),&mut out)},0);
        let mut got=Keyboard::default();
        let mut rect=pxui_types::geom::Rect::new(0.0,0.0,0.0,0.0);
        assert_eq!(unsafe {crate::ui_state::pxui_ui_keyboard(ui,&mut got)},0);
        assert_eq!(got,keyboard);
        assert_eq!(unsafe {crate::ui_state::pxui_ui_content_rect(ui,&mut rect)},0);
        assert_eq!((rect.y,rect.h),(47.0,461.0));
        for field in 0..7 {
            let mut bad=keyboard;
            match field {0=>bad.frame[0]=f32::NAN,1=>bad.frame[1]=f32::INFINITY,2=>bad.frame[2]=-1.0,3=>bad.frame[3]=-1.0,4=>bad.frame=[f32::MAX;4],5=>bad.flags=2,_=>bad.flags=4}
            out.size=123;
            assert_eq!(unsafe {super::pxui_frame(ui,&FrameIn {keyboard:bad,..input},None,core::ptr::null_mut(),&mut out)},PXUI_ERR_ARGUMENT);
            assert_eq!(out.size,123);
            assert_eq!(unsafe {crate::ui_state::pxui_ui_keyboard(ui,&mut got)},0);
            assert_eq!(got,keyboard);
        }
        for size in [128] {
            let current=FrameIn {size,keyboard:Keyboard {flags:2,..keyboard},..input};
            let mut prefix=[0u8;128];
            unsafe {core::ptr::copy_nonoverlapping((&current as *const FrameIn).cast::<u8>(),prefix.as_mut_ptr(),128)};
            let decoded=unsafe {super::read_frame(prefix.as_ptr().cast())}.unwrap();
            assert_eq!(decoded.environment,environment);
            assert_eq!(decoded.keyboard,Keyboard::default());
            assert_eq!(unsafe {super::pxui_frame(ui,prefix.as_ptr().cast(),None,core::ptr::null_mut(),&mut out)},0);
        }
        assert_eq!(unsafe {crate::ui_state::pxui_ui_keyboard(ui,core::ptr::null_mut())},PXUI_ERR_ARGUMENT);
        let prior=got;
        assert_eq!(unsafe {crate::ui_state::pxui_ui_keyboard(core::ptr::null_mut(),&mut got)},PXUI_ERR_ARGUMENT);
        assert_eq!(got,prior);
        assert_eq!(unsafe {super::pxui_ui_destroy(ui)},0);
    }
}

#[cfg(test)]
mod label_records {
    use super::{label_style, read_label_box};
    use pxui_types::{a11y::Role, ffi::PxuiLabelBox as B, Label};

    /// A label's box record carries its height, role and whole pixels; nothing set leaves the style's; a short record, unknown bits,
    /// an out-of-range role, a non-finite height, `whole_pixels` past 1 or a nonzero reserved byte is refused and changes nothing.
    #[test]
    fn label_box_record_applies_or_is_refused_whole() {
        let full = B::from(&Label::BODY.h_px(30.0).whole_pixels().role(Role::Paragraph));
        assert_eq!(full.set, B::SET_HEIGHT | B::SET_ROLE);
        let mut read = *Label::BODY.size(12.0).fields();
        assert!(read_label_box(&full, &mut read));
        assert!(read.height == Some(30.0) && read.whole_pixels && read.role == Role::Paragraph && read.size == Some(12.0));
        assert_eq!(B::from(&Label::BODY), B { size: core::mem::size_of::<B>() as u32, ..B::default() }, "a plain label sets nothing");
        for bad in [B { size: 12, ..full }, B { set: 4, ..full }, B { role: Role::Separator as u8 + 1, ..full }, B { height: f32::NAN, ..full },
            B { whole_pixels: 2, ..full }, B { reserved: [0, 1], ..full }] {
            let mut read = *Label::BODY.fields();
            assert!(!read_label_box(&bad, &mut read), "{bad:?}");
            assert!(read.height.is_none() && !read.whole_pixels && read.role == Role::Label);
        }
    }

    /// A C label record reads back as the `Label` it came from (its wrap, ellipsis, alignment and tabular numbers as enumerations), and
    /// a number no `Wrap`, `Ellipsis` or `TextAlign` has, or a feature bit past tabular numbers, is refused where the record is read.
    #[test]
    fn label_records_round_trip_and_refuse_unknown_numbers() {
        use pxui_types::{ffi::PxuiLabelStyle as S, text::{Ellipsis, TextAlign, Wrap}};
        let label = Label::BODY.size(13.0).wrap_glyph().lines(2).ellipsis_head().align(TextAlign::End).tabular().caps(true).pad(3.0);
        let record = S::from(&label);
        let (read, digits) = label_style(&record).expect("a label's own record");
        assert!(!digits);
        let (a, b) = (read, *label.fields());
        assert_eq!((a.wrap, a.ellipsis, a.align, a.features, a.max_lines, a.caps, a.pad), (Wrap::Glyph, Ellipsis::Head, TextAlign::End, 1, 2, Some(true), Some(3.0)));
        assert_eq!((a.wrap, a.ellipsis, a.align, a.features, a.size), (b.wrap, b.ellipsis, b.align, b.features, b.size));
        for bad in [S { wrap: 4, ..record }, S { ellipsis: 4, ..record }, S { align: 3, ..record }, S { features: 2, ..record }] {
            assert!(label_style(&bad).is_none(), "{bad:?}");
        }
    }
}
