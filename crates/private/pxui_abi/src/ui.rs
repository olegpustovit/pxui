//! The `Ui` lifecycle, the two frame entry points, theme and platform calls, and the first widgets.
use crate::trusted;
use crate::state::{Build, Inner, PxuiUi, Shared, boxed, call_inner, checked, fail, fail_code, get, pair, text};
use crate::{PXUI_ERR_ARGUMENT, PXUI_ERR_PANIC, PXUI_ERR_STATE, FrameIn, FrameOut, PxuiBox, PxuiPanel, PxuiUiDesc, Str};
use core::ffi::c_void;
use input::{Event, Key, MouseButton, Platform, frame};
use pxui_core::ui::{PanelOpts, Ui};
use pxui_types::ffi::{A11yNode, A11yTree, Damage, Input, Output, PlatformOut, Slice, ThemeSource};

fn platform() -> Platform {
    #[cfg(target_os = "macos")] { Platform::Apple }
    #[cfg(not(target_os = "macos"))] { Platform::Other }
}

pub(crate) fn key(code: u32) -> Option<Key> { Key::from_code(code) }
pub(crate) fn key_code(key: Key) -> u32 { key.code() }
pub(crate) fn button(code: u32) -> Option<MouseButton> { MouseButton::from_code(code) }
pub(crate) fn button_code(button: MouseButton) -> u32 { button.code() }

export!(bare pub unsafe fn pxui_abi_version() -> u32 { crate::ABI_VERSION });

// Process counters: 0 = calls into the library (this one included); 1.. = the library's own allocator counters, numbered as
// `mem_sys::COUNT_*` (allocations, bytes, live bytes, this thread's allocations and peak, reset of this thread's). The app registers
// this export with `mem_sys::set_peer`, so its budgets and ledgers count the whole process. Anything else reads 0.
export!(bare pub unsafe fn pxui_counter(which: u32) -> u64 {
    match which { 0 => crate::state::calls(), _ => mem_sys::own(which) }
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
    let Some(desc) = read_desc!(desc, PxuiUiDesc) else { return core::ptr::null_mut(); };
    if !valid_desc(&desc) { return core::ptr::null_mut(); }
    unsafe { create_ui(desc, &[pxui_core::text::PRAGMASEVKA]) }
});

export!(bare pub unsafe fn pxui_ui_create_with_fonts(desc: *const PxuiUiDesc, fonts: *const Str, count: usize) -> *mut PxuiUi {
    if desc.is_null() || count == 0 || count > 128 || fonts.is_null() { return core::ptr::null_mut(); }
    let Some(desc) = read_desc!(desc, PxuiUiDesc) else { return core::ptr::null_mut(); };
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
    let owner = Box::into_raw(Box::new(PxuiUi { engine, shared, owner: true }));
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
    // the dirty ranges first, all of them, so no slice below points into a vector that grows
    shared.dirty.clear();
    // A new partition of warmed instances must not grow the ABI copy after drawing starts.
    shared.dirty.reserve(output.draw.iter().map(|layer| layer.dirty.capacity()).sum());
    shared.dirty.extend(output.draw.iter().flat_map(|l| l.dirty.iter().map(|&(start, end)| pxui_types::ffi::PxuiDirty { start, end })));
    shared.layers.clear();
    let mut at = 0;
    for l in output.draw.iter() {
        let dirty = Slice { ptr: shared.dirty.as_ptr().wrapping_add(at), len: l.dirty.len() };
        at += l.dirty.len();
        shared.layers.push(pxui_types::ffi::PxuiLayerInfo { key: l.key, id: l.id, breaks: l.breaks.len() as u32, instances: Slice { ptr: l.instances.as_ptr(), len: l.instances.len() }, dirty });
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
        let Some(text) = (unsafe { text(source.source) }) else { return unsafe { fail(ui, "invalid theme UTF-8") }; };
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

pub(crate) fn translate_event(e: &pxui_types::Event) -> Option<Event> {
    Event::from_raw(&input::wire::Raw { kind: e.kind, code: e.code, x: e.x, y: e.y, value: e.value, text: unsafe { text(e.text) } })
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

/// The engine's input record for a frame at `time`: the last frame's, emptied, so that its vectors keep their capacity.
unsafe fn take_input(ui: *mut PxuiUi, time: f64) -> input::Frame {
    let Some(mut base) = (unsafe { (*(*ui).shared).spare_input.take() }) else { return input::at_time(time) };
    (base.t, base.dt, base.frame, base.tick, base.size, base.pointer) = (time, 0.0, 0, 0, (0, 0), (0.0, 0.0));
    base.buttons.clear();
    base.keys.clear();
    base.events.clear();
    base
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
        s.chunks_used = 0;
        s.in_frame = true;
        // no handle is usable until the build callback runs: the engine is mid-frame (its own phases run between callbacks)
        s.current = core::ptr::null_mut();
    }
    // a panic of the build unwinds past this (the static wrapper's callbacks run without a catch): the `Ui` leaves the frame
    struct Leave(*mut Shared, *mut PxuiUi);
    impl Drop for Leave { fn drop(&mut self) { unsafe { (*self.0).in_frame = false; (*self.0).current = self.1; } } }
    let _leave = Leave(shared, ui);
    // a panic of the build (the static wrapper's callbacks run without a catch) leaves the engine mid-frame: it is reset, then the panic goes on
    let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { (&mut *engine).frame(frame, &mut |engine: &mut Ui| call_inner(shared, engine, build)) }));
    let output = match built { Ok(output) => output, Err(panic) => { unsafe { (&mut *engine).abandon_frame(); } std::panic::resume_unwind(panic) } };
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
    crate::state::note_call();
    if let Err(code) = unsafe { enter_frame(ui) } { return code; }
    if !input.time.is_finite() || !input.scale.is_finite() || input.scale <= 0.0 || !input.width.is_finite() || !input.height.is_finite() { return unsafe { fail(ui, "invalid frame descriptor") }; }
    if input.event_count > 4096 || (input.event_count != 0 && input.events.is_null()) { return unsafe { fail(ui, "invalid frame event slice") }; }
    let legacy=core::mem::offset_of!(FrameIn,appearance) as u32;
    let (appearance,reduced)=if input.size==legacy { (0,0) } else if input.size>=core::mem::size_of::<FrameIn>() as u32 {
        (input.appearance,input.reduced_motion)
    } else { return unsafe { fail(ui,"invalid frame descriptor size") }; };
    if appearance>FrameIn::APPEARANCE_DARK || reduced>1 { return unsafe { fail(ui,"invalid system preferences") }; }
    let mut base = unsafe { take_input(ui, input.time) };
    base.size = (input.width as i64, input.height as i64);
    base.frame = input.time.to_bits() as i64;
    if !unsafe { push_events(&mut base, Slice { ptr: input.events, len: input.event_count }) } { return unsafe { fail(ui, "invalid event kind, code, or UTF-8 payload") }; }
    unsafe { &mut *(*ui).engine }.set_system_preferences(appearance,reduced!=0);
    let frame = frame::Frame { input: base, pixel_scale: (input.scale as f64, input.scale as f64), fps: 60.0, mouse_delta: (0.0, 0.0) };
    let rc = unsafe { run_frame(ui, &frame, build, out) };
    unsafe { (*(*ui).shared).spare_input = Some(frame.input); }
    rc
}

// Read only fields the caller supplied: old 48-byte inputs carry no OS preferences.
unsafe fn read_frame(input:*const FrameIn)->Option<FrameIn> {
    if input.is_null() { return None; }
    let size=unsafe { input.cast::<u32>().read_unaligned() } as usize;
    let legacy=core::mem::offset_of!(FrameIn,appearance);
    if size!=legacy && size<core::mem::size_of::<FrameIn>() { return None; }
    let mut value=FrameIn::default();
    unsafe { core::ptr::copy_nonoverlapping(input.cast::<u8>(),(&mut value as *mut FrameIn).cast::<u8>(),legacy); }
    if size>=core::mem::size_of::<FrameIn>() {
        value.appearance=unsafe { core::ptr::addr_of!((*input).appearance).read_unaligned() };
        value.reduced_motion=unsafe { core::ptr::addr_of!((*input).reduced_motion).read_unaligned() };
    }
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
    let Some(input) = (unsafe { read_frame(input) }) else { return unsafe { fail(ui, "invalid frame descriptor") }; };
    unsafe { frame_body(ui, &input, &mut |ui| if let Some(build) = build { build(user, ui.cast()) }, &mut *out) }
});

pub(crate) unsafe fn input_frame_body(ui: *mut PxuiUi, input: &Input, build: Inner, out: &mut FrameOut) -> i32 {
    crate::state::note_call();
    if let Err(code) = unsafe { enter_frame(ui) } { return code; }
    if !input.time.is_finite() || !input.dt.is_finite() || input.dt < 0.0 || !input.fps.is_finite() || input.fps < 0.0 || !input.pointer.iter().all(|v| v.is_finite()) || !input.pixel_scale.iter().all(|v| v.is_finite() && *v > 0.0) || !input.mouse_delta.iter().all(|v| v.is_finite()) { return unsafe { fail(ui, "invalid input frame") }; }
    if input.events.len > 4096 || input.keys.len > 512 || input.buttons.len > 32 || (input.events.len != 0 && input.events.ptr.is_null()) || (input.keys.len != 0 && input.keys.ptr.is_null()) || (input.buttons.len != 0 && input.buttons.ptr.is_null()) { return unsafe { fail(ui, "invalid input spans") }; }
    let mut base = unsafe { take_input(ui, input.time) };
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
    let rc = unsafe { run_frame(ui, &frame, build, out) };
    unsafe { (*(*ui).shared).spare_input = Some(frame.input); }
    rc
}

export!(trusted frame pub unsafe fn pxui_ui_frame(ui: *mut PxuiUi, input: *const Input, build: Build, user: *mut c_void, out: *mut FrameOut) -> i32 {
    if input.is_null() || out.is_null() { return unsafe { fail(ui, "null frame argument") }; }
    let Some(input) = read_desc!(input, Input) else { return unsafe { fail(ui, "invalid input frame") }; };
    unsafe { input_frame_body(ui, &input, &mut |ui| if let Some(build) = build { build(user, ui.cast()) }, &mut *out) }
});

// The frame's input as the engine routed it to `owner` (a viewport's root key; none: the app's), for the app's own content: the
// same record `pxui_ui_frame` takes, borrowed until the next call. Only after a frame has run.
export!(pub unsafe fn pxui_ui_input(ui: *mut PxuiUi, has_owner: u8, owner: i64, out: *mut Input) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() || has_owner > 1 { return unsafe { fail(ui, "invalid input query") }; }
    let engine = unsafe { &*state.engine };
    if engine.frame_index() == 0 { return unsafe { fail_code(ui, "no frame has completed", PXUI_ERR_STATE) }; }
    let shared = state.shared();
    shared.routed = Some(engine.input((has_owner != 0).then_some(owner)));
    let Some(f) = shared.routed.as_ref() else { return PXUI_ERR_STATE };
    shared.routed_events.clear();
    shared.routed_text.clear();
    shared.routed_text.resize(f.input.events.len(), String::new());
    for (event, scratch) in f.input.events.iter().zip(shared.routed_text.iter_mut()) {
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
    0
});

export!(trusted pub unsafe fn pxui_within(ui: *mut PxuiUi, box_: *const PxuiBox, build: Build, user: *mut c_void) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    unsafe { trusted::within(ui, box_, &mut |ui| if let Some(build) = build { build(user, ui.cast()) }) }
});

export!(trusted pub unsafe fn pxui_scope(ui: *mut PxuiUi, label: Str, build: Build, user: *mut c_void) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid scope label") }; };
    unsafe { trusted::scope(ui, label, &mut |ui| if let Some(build) = build { build(user, ui.cast()) }) }
});

export!(trusted pub unsafe fn pxui_col(ui: *mut PxuiUi, padding: f32, label: Str, build: Build, user: *mut c_void) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid column arguments") }; };
    unsafe { trusted::col(ui, padding, label, &mut |ui| if let Some(build) = build { build(user, ui.cast()) }) }
});

export!(trusted pub unsafe fn pxui_panel(ui: *mut PxuiUi, panel: *const PxuiPanel, label: Str, build: Build, user: *mut c_void) -> i32 {
    if unsafe { get(ui) }.is_none() || panel.is_null() { return PXUI_ERR_ARGUMENT; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid panel label") }; };
    let Some(panel) = read_desc!(panel, PxuiPanel) else { return unsafe { fail(ui, "invalid panel descriptor") }; };
    let mut opts = PanelOpts::default().at(panel.x, panel.y).width(panel.width).padding(panel.padding).window(panel.window != 0);
    if panel.has_height != 0 { opts = opts.height(panel.height); }
    if panel.has_max_height != 0 { opts = opts.max_height(panel.max_height); }
    if panel.has_stroke != 0 { opts.stroke = Some(panel.stroke); }
    if panel.has_look != 0 { let Some(look) = crate::style::decode_look(panel.look) else { return unsafe { fail(ui, "invalid panel look") }; }; opts.look = Some(look); }
    unsafe { trusted::panel(ui, opts, label, &mut |ui| if let Some(build) = build { build(user, ui.cast()) }) }
});

export!(trusted pub unsafe fn pxui_label(ui: *mut PxuiUi, label: Str) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(label) = (unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid label text") } };
    unsafe { trusted::pxui_label(ui, pair(label)) }
});

export!(trusted pub unsafe fn pxui_button(ui: *mut PxuiUi, label: Str, out: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let (Some(label), false) = (unsafe { checked(label) }, out.is_null()) else { return unsafe { fail(ui, "invalid button arguments") } };
    unsafe { trusted::pxui_button(ui, pair(label), out) }
});

export!(trusted pub unsafe fn pxui_toggle(ui: *mut PxuiUi, label: Str, on: u8, out: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let (Some(label), false) = (unsafe { checked(label) }, out.is_null() || on > 1) else { return unsafe { fail(ui, "invalid toggle arguments") } };
    unsafe { trusted::pxui_toggle(ui, pair(label), on, out) }
});

export!(trusted pub unsafe fn pxui_text_field(ui: *mut PxuiUi, label: Str, value: Str, changed: *mut Str, out_changed: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if changed.is_null() || out_changed.is_null() { return unsafe { fail(ui, "null text field output") }; }
    let (Some(label), Some(value)) = (unsafe { checked(label) }, unsafe { checked(value) }) else { return unsafe { fail(ui, "invalid text field arguments") } };
    unsafe { trusted::pxui_text_field(ui, pair(label), pair(value), changed, out_changed) }
});

#[cfg(test)]
mod tests {
    use pxui_types::Event as C;

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
        for size in [56,64,u32::MAX] {
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
            (w::PIXEL_SIZE_CHANGED, C::PIXEL_SIZE_CHANGED), (w::VISIBILITY_CHANGED, C::VISIBILITY_CHANGED), (w::TRACKPAD_SCROLLED, C::TRACKPAD_SCROLLED)];
        for (wire, abi) in pairs { assert_eq!(wire, abi); }
    }
}
