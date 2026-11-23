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
use pxui_types::ffi::{A11yNode, A11yTree, Damage, Input, Output, PlatformOut, PxuiDamageList, Slice, Strided};

fn platform() -> Platform {
    #[cfg(any(target_os = "macos", target_os = "ios"))] { Platform::Apple }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))] { Platform::Other }
}

pub(crate) fn key(code: u32) -> Option<Key> { Key::from_code(code) }
pub(crate) fn key_code(key: Key) -> u32 { key.code() }
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

export!(pub unsafe fn pxui_ui_view_size(ui: *mut PxuiUi, out: *mut pxui_types::geom::Point) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null view size output") }; }
    let (x, y) = unsafe { (&*state.engine).view_size() };
    unsafe { *out = pxui_types::geom::Point { x, y }; }
    0
});

// Creates a `Ui` from `desc` with the `count` fonts of `fonts` (0, and `fonts` may be null: the library's embedded default face; the
// first font is the theme's). Null, with `*status` (when `status` is not null) set to why: PXUI_ERR_ARGUMENT for a record, font or count that
// is invalid, PXUI_ERR_VERSION for an app built for another major or a newer minor than this library; 0 on success.
export!(bare pub unsafe fn pxui_ui_create(desc: *const PxuiUiDesc, fonts: *const Str, count: usize, status: *mut i32) -> *mut PxuiUi {
    let refuse = |code: i32| { if !status.is_null() { unsafe { *status = code; } } core::ptr::null_mut() };
    if desc.is_null() || count > pxui_core::text::MAX_FONTS || (count != 0 && fonts.is_null()) { return refuse(PXUI_ERR_ARGUMENT); }
    let Some(desc) = read_desc!(desc, PxuiUiDesc) else { return refuse(PXUI_ERR_ARGUMENT); };
    if !valid_desc(&desc) { return refuse(PXUI_ERR_ARGUMENT); }
    if !served_version(&desc) { return refuse(crate::PXUI_ERR_VERSION); }
    let mut bytes = Vec::with_capacity(count.max(1));
    let mut total = 0usize;
    if count == 0 { bytes.push(pxui_core::text::PRAGMASEVKA); }
    for font in if count == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(fonts, count) } } {
        total = total.saturating_add(font.len);
        if font.len == 0 || font.len > pxui_core::text::MAX_FONT_BYTES || total > pxui_core::text::MAX_FONT_TOTAL_BYTES || font.ptr.is_null() { return refuse(PXUI_ERR_ARGUMENT); }
        bytes.push(unsafe { std::slice::from_raw_parts(font.ptr, font.len) });
    }
    let ui = unsafe { create_ui(desc, &bytes) };
    if ui.is_null() { return refuse(PXUI_ERR_ARGUMENT); }
    if !status.is_null() { unsafe { *status = 0; } }
    ui
});

/// A `PxuiUiDesc` is read whole (its size covers it, else `PXUI_ERR_ARGUMENT`); its numbers must be in range.
fn valid_desc(desc: &PxuiUiDesc) -> bool { desc.font_size >= 0 && desc.platform <= 2 }

/// The library's own major is the only one it serves, and `abi_built` must state it: 0 (unstated) and any other major (a 1.x app) are refused,
/// and so is a minor newer than the library's own (the minimum-library rule: an app built for a newer minor may fill or read records grown
/// past what this library knows, so it is refused at create with `PXUI_ERR_VERSION`, never misread later).
fn served_version(desc: &PxuiUiDesc) -> bool {
    desc.abi_built >> 16 == crate::ABI_VERSION >> 16 && desc.abi_built & 0xFFFF <= crate::ABI_VERSION & 0xFFFF
}

// 1 when the library, as built, has the feature `id` (`PXUI_FEATURE_*`: ids are never reused); 0 for an id it does not know.
export!(bare pub unsafe fn pxui_supports(id: u32) -> u32 {
    use pxui_types::feature as f;
    (match id {
        f::A11Y_BRIDGE => cfg!(target_os = "linux"),
        f::METAL => cfg!(any(target_os = "macos", target_os = "ios")),
        f::VULKAN => cfg!(any(windows, target_os = "linux")),
        f::EXTENSION_SET => true,
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
    unsafe { (*shared).current = owner; (*shared).owner = owner; }
    owner
}

// Frees a `Ui`. Fails (and frees nothing) on a borrowed handle or inside a frame. With a renderer still attached the `Ui` goes when the
// last renderer is destroyed (each reads its engine): the handle is invalid for the caller from this call on either way.
export!(bare pub unsafe fn pxui_ui_destroy(ui: *mut PxuiUi) -> i32 {
    if ui.is_null() { return 0; }
    let state = unsafe { &*ui };
    let shared = state.shared();
    if !state.owner || shared.in_frame { return PXUI_ERR_STATE; }
    if shared.renderers != 0 { shared.destroy_pending = true; return 0; }
    unsafe { free_ui(ui) };
    0
});

/// Frees the owner handle `ui`, its engine and its shared state.
unsafe fn free_ui(ui: *mut PxuiUi) {
    let ui = unsafe { Box::from_raw(ui) };
    drop(unsafe { Box::from_raw(ui.engine) });
    drop(unsafe { Box::from_raw(ui.shared) });
}

/// A renderer of the `Ui` behind `shared` has gone: the `Ui` goes with the last one when its destroy came first.
pub(crate) unsafe fn renderer_released(shared: *mut crate::state::Shared) {
    unsafe {
        (*shared).renderers -= 1;
        if (*shared).renderers == 0 && (*shared).destroy_pending { free_ui((*shared).owner); }
    }
}

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
                pxui_core::a11y::Value::Number { now, min, max, step, text } => pxui_types::ffi::A11yValue { tag: 2, text_start: text.start, text_end: text.end, now, min, max, step, ..Default::default() },
            };
            shared.nodes.push(A11yNode {
                id: n.id, parent: n.parent, role: n.role as u32, name_start: n.name.start, name_end: n.name.end,
                description_start: n.description.start, description_end: n.description.end, value, rect: n.rect, states: n.states,
                actions: n.actions, selection: [n.selection.0, n.selection.1], child_count: n.child_count, first_child: n.first_child,
                next_sibling: n.next_sibling, last_child: shared.last_children[index], ..A11yNode::default()
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
            shared.layers.push(pxui_types::ffi::PxuiLayerInfo { size: core::mem::size_of::<pxui_types::ffi::PxuiLayerInfo>() as u32, role: l.role as u32, key: l.key, id: l.id, breaks: l.breaks.len() as u32, instances: Slice { ptr: l.instances.as_ptr(), len: l.instances.len() }, dirty, anchor: l.anchor });
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
        size: core::mem::size_of::<Output>() as u32, repaint_hz: engine.repaint_hz(), draw_revision: revision,
        repaint_at: engine.repaint_at().unwrap_or(f64::INFINITY), built_nodes: engine.built_nodes() as u64,
        rebuilt_views: engine.rebuilt_views() as u64, arranged_nodes: engine.arranged_nodes() as u64,
        platform: PlatformOut {
            size: core::mem::size_of::<PlatformOut>() as u32, cursor, has_cursor: has_cursor as u8, has_ime: has_ime as u8,
            has_background: has_background as u8, has_clipboard: has_clipboard as u8, ime_rect, ime_caret, clipboard, background,
            paste: platform.paste as u8, reserved: [0; 7],
        },
        damage: output.damage.bounds().map_or_else(Damage::default, |r| Damage { has_damage: 1, reserved: [0; 3], rect: r }),
        a11y: A11yTree {
            nodes: Strided::of(&shared.nodes), strings: Str { ptr: tree.strings.as_ptr(), len: tree.strings.len() },
            revision: tree.revision, first_child: tree.first_child(u32::MAX), child_count: tree.child_count(u32::MAX), ..A11yTree::default()
        },
        layers: Strided::of(&shared.layers),
        damage_list: damage_list(output.damage),
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

fn damage_list(damage: &pxui_types::damage::DamageList) -> PxuiDamageList {
    let mut out = PxuiDamageList {
        count: damage.rects().len() as u8,
        full: damage.is_full() as u8,
        ..PxuiDamageList::default()
    };
    for (dest, r) in out.rects.iter_mut().zip(damage.rects()) {
        *dest = [r.x, r.y, r.w, r.h];
    }
    out
}

export!(pub unsafe fn pxui_ui_damage_list(ui: *mut PxuiUi, out: *mut PxuiDamageList) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null damage list output") }; }
    let shared = state.shared();
    let list = if shared.in_frame {
        shared.output.damage_list
    } else {
        damage_list(unsafe { &*state.engine }.output().damage)
    };
    unsafe { out.write(list); }
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

export!(pub unsafe fn pxui_ui_inject_a11y(ui: *mut PxuiUi, actions: pxui_types::ffi::A11yActionSpan) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if actions.len > 4096 || !actions.is_valid() { return unsafe { fail(ui, "invalid accessibility action span (a stride below the action size, or no pointer)") }; }
    if unsafe { (&*state.engine).pending_a11y_actions() } > 4096 - actions.len {
        return unsafe { fail_code(ui, "accessibility action queue is full until the next frame", PXUI_ERR_STATE) };
    }
    // Reject the whole batch before feeding any chunk to the retained engine queue, including actions addressed to another surface.
    if (0..actions.len).any(|i| { let a = unsafe { actions.at(i) }; (a.size as usize) < core::mem::size_of::<pxui_types::ffi::A11yAction>() || a.surface == 0 || a.surface != state.shared().surface || a.reserved != 0 || !a.payload.is_finite() || !matches!(a.action_code,1|2|4|8|16|32|64|128|256) }) {
        return unsafe { fail(ui, "invalid accessibility action record or code") };
    }
    use pxui_core::a11y::{Action, A11yAction};
    const KINDS: [Action;9] = [Action::Press,Action::Focus,Action::SetValue,Action::Scroll,Action::Expand,Action::Collapse,Action::Select,Action::MoveUp,Action::MoveDown];
    let mut translated = [A11yAction { id:0,action:Action::Press,payload:0.0,end:0.0 };32];
    for start in (0..actions.len).step_by(translated.len()) {
        let n = translated.len().min(actions.len - start);
        for (k, to) in translated[..n].iter_mut().enumerate() {
            let from = unsafe { actions.at(start + k) };
            *to = A11yAction { id:from.id,action:KINDS[from.action_code.trailing_zeros() as usize],payload:from.payload,end:from.end as f64 };
        }
        unsafe { (&mut *state.engine).inject_a11y(&translated[..n]); }
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

export!(pub unsafe fn pxui_ui_step(ui: *mut PxuiUi, time: f64, out: *mut u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() || !time.is_finite() { return unsafe { fail(ui, "invalid step arguments") }; }
    unsafe { *out = (&mut *state.engine).step(time) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_frame_reasons(ui: *mut PxuiUi, out: *mut pxui_types::ffi::PxuiFrameReasons) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null frame reasons output") }; }
    unsafe { *out = (&*state.engine).frame_reasons(); }
    0
});

export!(pub unsafe fn pxui_ui_request_frame(ui: *mut PxuiUi) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    unsafe { (&mut *state.engine).request_frame(); }
    0
});

/// A `Ui` with `FrameIn::MAIN` registered, for the tests that run frames.
#[cfg(test)]
pub(crate) unsafe fn create_test_ui() -> *mut PxuiUi {
    let ui = unsafe { pxui_ui_create(&crate::PxuiUiDesc::default(), core::ptr::null(), 0, core::ptr::null_mut()) };
    if !ui.is_null() { assert_eq!(unsafe { pxui_ui_add_surface(ui, pxui_types::FrameIn::MAIN) }, 0); }
    ui
}

export!(pub unsafe fn pxui_ui_add_surface(ui: *mut PxuiUi, surface: u64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if surface == 0 { return unsafe { fail(ui, "surface id 0 is reserved") }; }
    // an id twice, or a second surface (one per Ui for now), is a state error; a frame callback cannot register
    if !state.owner || state.shared().in_frame || state.shared().surface != 0 { return unsafe { fail_code(ui, "a surface is registered already, or a frame is running", PXUI_ERR_STATE) }; }
    state.shared().surface = surface;
    0
});

export!(pub unsafe fn pxui_ui_remove_surface(ui: *mut PxuiUi, surface: u64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if !state.owner || state.shared().in_frame { return unsafe { fail_code(ui, "a frame is running", PXUI_ERR_STATE) }; }
    if surface == 0 || state.shared().surface != surface { return unsafe { fail(ui, "unknown surface") }; }
    unsafe { (&mut *state.engine).clear_a11y_actions(); }
    state.shared().surface = 0;
    0
});

fn touch_event(e:&pxui_types::Event)->Option<input::Touch> {
    // the record is the payload's length, whole: its own `size` covers `Touch` and does not run past the payload
    if e.text.len<core::mem::size_of::<pxui_types::Touch>() || e.text.ptr.is_null() {return None;}
    let t=unsafe {e.text.ptr.cast::<pxui_types::Touch>().read_unaligned()};
    if (t.size as usize)<core::mem::size_of::<pxui_types::Touch>() || t.size as usize>e.text.len {return None;}
    let value=input::Touch {device:t.device,finger:t.finger,point:(t.x,t.y),time:t.time,pressure:t.pressure,phase:input::TouchPhase::from_code(t.phase)?};
    value.valid().then_some(value)
}

/// The largest width or height of a surface a frame takes, in points (`FrameIn.width`/`height`, `Input.logical_size`) and of a resize
/// event, in points or pixels: a larger or negative one is refused.
pub(crate) const MAX_SURFACE: f64 = (1 << 16) as f64;

/// One wire event checked and decoded: the one validator of both frame entries and `pxui_ui_wants_frame`. Its `x`, `y` and `value` are
/// finite whatever its kind; its text (any kind's, read only past these checks) is at most `MAX_TEXT_BYTES` and is taken from `budget`
/// (`MAX_TOTAL_TEXT_BYTES` a frame, or a `wants_frame` call); a touch's payload is a whole valid `Touch`; a text editing range is not
/// negative; a resize is within `0..=MAX_SURFACE`; the kind and its code are known; the record is whole (`size`) and names `surface`. Anything else is refused (`None`), never cut.
pub(crate) fn translate_event(e: &pxui_types::Event, surface: u64, budget: &mut usize) -> Option<Event> {
    // a whole record, for the surface of the call that carries it (the frame's, or the Ui's for `wants_frame`)
    if (e.size as usize) < core::mem::size_of::<pxui_types::Event>() || e.surface != surface { return None; }
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

fn valid_touches(events:&Strided<pxui_types::Event>)->bool {
    (0..events.len).all(|i| { let e = unsafe { events.at(i) }; e.kind!=pxui_types::Event::TOUCH || touch_event(e).is_some() })
}

/// Decode the whole batch before any edge changes retained input.
#[inline(always)]
fn decode_events(out: &mut Vec<Event>, events: &Strided<pxui_types::Event>, surface: u64) -> bool {
    let mut budget = crate::state::MAX_TOTAL_TEXT_BYTES;
    // the span was checked by the caller (`is_valid`: a stride that holds an event, a pointer); each event's own size is checked by `translate_event`
    for i in 0..events.len {
        let raw = unsafe { events.at(i) };
        // the commonest event of a frame, decoded in line (`translate_event` decodes it the same)
        if raw.kind == pxui_types::Event::POINTER_MOVED && raw.size as usize >= core::mem::size_of::<pxui_types::Event>() && raw.surface == surface && raw.x.is_finite() && raw.y.is_finite() && raw.value.is_finite() { out.push(Event::PointerMoved((raw.x, raw.y))); continue; }
        let Some(event) = translate_event(raw, surface, &mut budget) else { return false };
        out.push(event);
    }
    true
}

#[inline(always)]
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
#[inline(always)]
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
    if (value.size as usize)<core::mem::size_of::<pxui_types::Environment>() { return false; }
    let rect=|r:&[f32;4]| r.iter().all(|v|v.is_finite()) && r[2]>=0.0 && r[3]>=0.0 && (r[0]+r[2]).is_finite() && (r[1]+r[3]).is_finite();
    value.safe_area.iter().all(|v|v.is_finite() && *v>=0.0)
        && (value.safe_area[0]+value.safe_area[2]).is_finite() && (value.safe_area[1]+value.safe_area[3]).is_finite()
        && rect(&value.keyboard_frame) && rect(&value.keyboard_from)
        && value.keyboard_started.is_finite() && value.keyboard_duration.is_finite() && value.keyboard_duration>=0.0
        && (value.keyboard_started+value.keyboard_duration).is_finite() && value.keyboard_visible<=1
        && value.scrollers<=pxui_types::Environment::OVERLAY_SCROLLERS
}

/// A changed host environment, given to the engine out of line: a frame that repeats it (the most) keeps none of it in registers.
#[cold]
#[inline(never)]
fn set_environment(engine: &mut Ui, environment: &pxui_types::Environment) { engine.set_environment(pxui_types::Environment { size: core::mem::size_of::<pxui_types::Environment>() as u32, ..*environment }); }

/// A changed keyboard geometry, given to the engine out of line (as `set_environment`).
#[cold]
#[inline(never)]
fn set_keyboard(engine: &mut Ui, keyboard: &pxui_types::Keyboard) { engine.set_keyboard(pxui_types::Keyboard { size: core::mem::size_of::<pxui_types::Keyboard>() as u32, ..*keyboard }); }

/// Runs one frame of `ui` over `frame`. Both frame entry points call it after decoding their input (inline: one frame call fewer).
#[inline(always)]
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
    let (damage, layers, layer_count) = (output.damage.bounds(), output.draw.as_ptr(), output.draw.len());
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
    // the frame's surface is one registered on this Ui (the one compare); every event must carry it too (`decode_events`)
    let registered = unsafe { (*(*ui).shared).surface };
    if registered == 0 || input.surface != registered { return unsafe { fail(ui, "unknown surface") }; }
    // a size within 0..=MAX_SURFACE, a scale in (0, inf) and a finite time: NaN fails every compare, so each is a compare or two
    let surface = |v: f32| (0.0..=MAX_SURFACE as f32).contains(&v);
    if !(input.time.abs() < f64::INFINITY) || !(input.scale > 0.0 && input.scale < f32::INFINITY) || !surface(input.width) || !surface(input.height) { return unsafe { fail(ui, "invalid frame descriptor") }; }
    if input.events.len > 4096 || !input.events.is_valid() { return unsafe { fail(ui, "invalid frame event span (a stride below the event size, or no pointer)") }; }
    let (appearance, reduced) = (input.appearance, input.reduced_motion);
    // read where they lie: what the engine holds was validated when it was set (the default is valid), so a frame that repeats it
    // compares bytes and checks nothing, and only a changed value is validated (a NaN never matches)
    let (environment, keyboard) = (&input.environment, &input.keyboard);
    let engine=unsafe { &mut *(*ui).engine };
    let environment_changed=!same_environment(environment,engine.environment_ref());
    if environment_changed && !valid_environment(environment) { return unsafe { fail(ui,"invalid host environment") }; }
    let keyboard_changed=!same_keyboard(keyboard,engine.keyboard_ref());
    if keyboard_changed && !keyboard.valid() { return unsafe { fail(ui,"invalid keyboard geometry") }; }
    if appearance>FrameIn::APPEARANCE_DARK || reduced>1 { return unsafe { fail(ui,"invalid system preferences") }; }
    let mut base = unsafe { take_input(ui, input.time) };
    let events = &input.events;
    if !decode_events(&mut base.input.events, events, registered) {
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
    if environment_changed { set_environment(engine, environment); }
    if keyboard_changed { set_keyboard(engine, keyboard); }
    (base.pixel_scale, base.fps, base.mouse_delta) = ((input.scale as f64, input.scale as f64), 60.0, (0.0, 0.0));
    let rc = unsafe { run_frame(ui, &base, build, out) };
    unsafe { restore_input(ui, base); }
    rc
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
    // the frame's callback sees the owner's theme (the `Ui`'s own: no scope is open)
    let theme = if ui.is_null() { core::ptr::null() } else { unsafe { (*ui).theme } };
    let mut build = |engine: &mut Ui| unsafe { crate::state::call_build_seeing(shared, engine, theme, build, user) };
    // the whole record is read where it lies (`frame_body` reads it before the build runs, so the caller cannot change it under the
    // engine): its size covers `FrameIn`, else it is refused (there are no older sizes)
    if (unsafe { input.cast::<u32>().read_unaligned() } as usize) < core::mem::size_of::<FrameIn>() { return unsafe { fail(ui, "invalid frame descriptor") }; }
    unsafe { frame_body(ui, &*input, &mut build, &mut *out) }
});

// The frame's input as the engine routed it to `owner` (a viewport's root key; none: the app's), for the app's own content: the
// `Input` record (an output record now: `pxui_frame` takes a `PxuiFrameIn`), borrowed until the next call. Only after a frame has run.
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
            shared.routed_touches.push(pxui_types::Touch::new(t.device,t.finger,[t.point.0,t.point.1],t.time,t.pressure,t.phase.code()));
            shared.routed_events.push(pxui_types::Event {kind:pxui_types::Event::TOUCH,surface:shared.surface,text:Str {ptr:shared.routed_touches.as_ptr().wrapping_add(shared.routed_touches.len()-1).cast(),len:core::mem::size_of::<pxui_types::Touch>()},..Default::default()});
            continue;
        }
        let raw = event.raw(scratch);
        let text = raw.text.unwrap_or("");
        shared.routed_events.push(pxui_types::Event { surface: shared.surface, kind: raw.kind, code: raw.code, x: raw.x, y: raw.y, value: raw.value, text: Str { ptr: text.as_ptr(), len: text.len() }, ..Default::default() });
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
            events: Strided::of(&shared.routed_events),
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

export!(trusted pub unsafe fn pxui_ui_within(ui: *mut PxuiUi, box_: *const PxuiBox, build: Build, user: *mut c_void) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    unsafe { trusted::within(ui, box_, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build_seeing((*ui).shared, engine, (*ui).theme, build, user)) }
});

export!(trusted pub unsafe fn pxui_ui_scope(ui: *mut PxuiUi, label: Str, build: Build, user: *mut c_void) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(label) = (unsafe { crate::state::checked_key(label) }) else { return unsafe { fail(ui, "invalid scope label") }; };
    unsafe { trusted::scope(ui, label, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build_seeing((*ui).shared, engine, (*ui).theme, build, user)) }
});

/// The panel of `panel`'s record keyed by `key` and named `name`, the body of `pxui_ui_panel`.
unsafe fn panel(ui: *mut PxuiUi, key: &str, name: &str, panel: *const PxuiPanel, build: Build, user: *mut c_void, root: &mut PxuiBox) -> i32 {
    // read where it lies (a record at least as long as ours), the look only when it has one
    let Some(panel) = sized!(panel) else { return unsafe { fail(ui, "invalid panel descriptor") }; };
    if panel.padding < 0 { return unsafe { fail(ui, "invalid panel padding") }; }
    // the fractional padding counts only where it agrees with the integer one (a 1.x record leaves the hole unset)
    let padding = if (panel.padding_points - panel.padding as f32).abs() <= 0.5 && panel.padding_points >= 0.0 { panel.padding_points } else { panel.padding as f32 };
    // the look decoded first, so the overlay is written once
    let look = if panel.look.is_null() { None } else { let Some(look) = read_desc!(panel.look, pxui_types::ffi::PxuiLookDesc).and_then(crate::style::decode_look) else { return unsafe { fail(ui, "invalid panel look") }; }; Some(look) };
    let overlay = pxui_types::ui::Overlay { at: (panel.x, panel.y), w: Some(panel.width), h: (panel.has_height != 0).then_some(panel.height),
        max_h: (panel.has_max_height != 0).then_some(panel.max_height), window: panel.window != 0, padding, name: Some(name),
        stroke: (panel.has_stroke != 0).then_some(panel.stroke), look: look.as_ref(), ..pxui_types::ui::Overlay::DEFAULT };
    unsafe { trusted::panel_keyed(ui, key, crate::state::unusable(key), &overlay, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, build, user), root) }
}

// ABI 1.76: a panel keyed by `keyed.key`, named `keyed.name`, placed and dressed by `panel`, writing its root to `root`.
export!(trusted pub unsafe fn pxui_ui_panel(ui: *mut PxuiUi, keyed: *const pxui_types::ffi::PxuiKeyedDesc, panel: *const PxuiPanel, build: Build, user: *mut c_void, root: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() || panel.is_null() { return PXUI_ERR_ARGUMENT; }
    if root.is_null() { return unsafe { fail(ui, "null panel root") }; }
    let Some((key, name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    unsafe { self::panel(ui, key, name, panel, build, user, &mut *root) }
});


export!(build pub unsafe fn pxui_ui_label_at(ui: *mut PxuiUi, key: Str, rect: pxui_types::geom::Rect, text: Str, style: *const pxui_types::ffi::PxuiLabelStyle, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if !state.shared().in_frame || out.is_null() { return unsafe { fail(ui, "placed label needs a frame and output") }; }
    let (Some(key), Some(text), Some((style, _))) = (unsafe { crate::state::checked_key(key) }, unsafe { checked(text) }, label_style(style)) else {
        return unsafe { fail(ui, "invalid placed label key, text or style") };
    };
    if ![rect.x, rect.y, rect.w, rect.h].iter().all(|v| v.is_finite()) || rect.w < 0.0 || rect.h < 0.0 {
        return unsafe { fail(ui, "invalid placed label rectangle") };
    }
    let Some(label) = (unsafe { &mut *state.engine }).label_at(key, rect, text, &style) else {
        return unsafe { fail(ui, "placed label needs a loaded font and a positive line height") };
    };
    unsafe { *out = crate::state::to_abi_box(label); }
    0
});

// ABI 1.54: a label of `text` in `style` with the label's box (`label_box`, null for none), writing the label's node to `out`; a null
// `style` is the theme's label.
export!(trusted pub unsafe fn pxui_ui_label(ui: *mut PxuiUi, text: Str, style: *const pxui_types::ffi::PxuiLabelStyle, label_box: *const pxui_types::ffi::PxuiLabelBox, out: *mut PxuiBox) -> i32 {
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
    if b.set & !B::SET_ALL != 0 || !b.height.is_finite() || b.role > pxui_types::a11y::Role::Separator as u8 || b.whole_pixels > 1 { return false; }
    if b.set & B::SET_HEIGHT != 0 { label.height = Some(b.height); }
    // the role was checked against the last one above
    if b.set & B::SET_ROLE != 0 { label.role = unsafe { core::mem::transmute::<u8, pxui_types::a11y::Role>(b.role) }; }
    label.whole_pixels |= b.whole_pixels != 0;
    label.variant = b.variant;
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

// ABI 1.82: a text field of `desc` that reports what happened to the field's box in `response` (box, hover, press, focus and click). The
// trusted entry checks the handle (once: the export does not count the call itself).
export!(trusted pub unsafe fn pxui_ui_text_field(ui: *mut PxuiUi, desc: *const pxui_types::ffi::PxuiTextFieldDesc, changed: *mut Str, out_changed: *mut u8, response: *mut pxui_types::ffi::PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if desc.is_null() || changed.is_null() || out_changed.is_null() { return unsafe { fail(ui, "null text field descriptor or output") }; }
    let Some(response) = sized_mut!(response) else { return unsafe { fail(ui, "null or short text field response") } };
    if unsafe { (*desc).size } < core::mem::size_of::<pxui_types::ffi::PxuiTextFieldDesc>() as u32 { return unsafe { fail(ui, "invalid text field descriptor") }; }
    let d = unsafe { *desc };
    let (Some(key), Some(name), Some(value)) = (unsafe { crate::state::checked_key(d.key) }, unsafe { checked(d.name) }, unsafe { crate::state::checked_text(d.value) }) else { return unsafe { fail(ui, "invalid text field text") }; };
    let Some(traits) = pxui_types::TextTraits::from_numbers(d.autocorrect as u32, d.capitalization as u32, d.action as u32).filter(|_| d.reserved == 0 && d.reserved_variant == [0; 6]) else { return unsafe { fail(ui, "invalid text input options") }; };
    let mut text = None;
    let rc = with_variant!(ui, pxui_types::theme::BodyKind::text_field, d.variant, unsafe { trusted::text_field(ui, key, name, value, traits, &mut text, response) });
    // a refused call (outside a frame among them) leaves the outputs alone
    if rc == 0 { unsafe { (*changed, *out_changed) = text.map_or((Str::default(), 0), |text| (text, 1)); } }
    rc
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
        let input = crate::FrameIn::new(320.0, 240.0);
        assert_eq!(unsafe { super::pxui_frame(ui, &input, Some(run), (&mut f as *mut &mut dyn FnMut(*mut super::PxuiUi)).cast(), &mut out) }, 0);
    }

    #[test]
    fn the_keyed_text_field_refuses_invalid_descriptors_before_output_or_widget_mutation() {
        use super::*;
        use pxui_types::ffi::PxuiTextFieldDesc;
        let ui=unsafe {crate::ui::create_test_ui()};
        let text=|s:&'static str| { let mut record = Str::default(); record.ptr = s.as_ptr(); record.len = s.len(); record };
        let good=PxuiTextFieldDesc {size:core::mem::size_of::<PxuiTextFieldDesc>() as u32,autocorrect:1,capitalization:1,action:0,reserved:0,key:text("k"),name:text("n"),value:text("v"),variant:0,reserved_variant:[0;6]};
        let bad_utf8={ let mut record = Str::default(); record.ptr = [0xff_u8].as_ptr(); record.len = 1; record };
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
            PxuiTextFieldDesc {key:{ let mut span = Str::default(); span.len = 1; span },..good},
        ];
        let mut changed={ let mut record = Str::default(); record.ptr = b"sentinel".as_ptr(); record.len = 8; record };
        let mut flag=77;
        let mut r=pxui_types::ffi::PxuiResponse::EMPTY;
        // outside a frame the builder is refused before its descriptor is read
        assert_eq!(unsafe {pxui_ui_text_field(ui,&good,&mut changed,&mut flag,&mut r)},PXUI_ERR_STATE);
        in_frame(ui,&mut |ui| {
            let built=unsafe {&*(*ui).engine}.built_nodes();
            for (i,desc) in cases.iter().enumerate() {
                // a refused call must not leave the Ui poisoned or with a node
                assert_eq!(unsafe {pxui_ui_text_field(ui,desc,&mut changed,&mut flag,&mut r)},PXUI_ERR_ARGUMENT,"case {i}");
                assert_eq!((changed.len,flag),(8,77),"case {i} left its output alone");
                assert_eq!(unsafe {&*(*ui).engine}.built_nodes(),built,"case {i} built nothing");
            }
            assert_eq!(unsafe {pxui_ui_text_field(ui,core::ptr::null(),&mut changed,&mut flag,&mut r)},PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe {pxui_ui_text_field(ui,&good,core::ptr::null_mut(),&mut flag,&mut r)},PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe {pxui_ui_text_field(ui,&good,&mut changed,core::ptr::null_mut(),&mut r)},PXUI_ERR_ARGUMENT);
        });
        assert_eq!(unsafe {pxui_ui_text_field(core::ptr::null_mut(),&good,&mut changed,&mut flag,&mut r)},PXUI_ERR_ARGUMENT);
        assert_eq!((changed.len,flag),(8,77));
        assert_eq!(unsafe {pxui_ui_destroy(ui)},0);
    }

    /// Each text input export reports what `pxui_ui_signal` and `pxui_ui_focused` read of the box it built, in the one call; a short
    /// response record is refused before the widget runs.
    #[test]
    fn text_input_response_exports_report_their_box_in_one_call() {
        use super::*;
        use crate::kit_ext::pxui_ui_line_field;
        use crate::text_inputs::{pxui_ui_text_area, pxui_ui_value_field};
        use crate::widgets::pxui_ui_search_field;
        use pxui_types::ffi::{PxuiResponse, PxuiTextAreaDesc, PxuiTextFieldDesc, PxuiValueFieldDesc};
        let s=|t: &'static str| { let mut record = Str::default(); record.ptr = t.as_ptr(); record.len = t.len(); record };
        let ui=unsafe {crate::ui::create_test_ui()};
        let field=PxuiTextFieldDesc {size:core::mem::size_of::<PxuiTextFieldDesc>() as u32,autocorrect:0,capitalization:0,action:0,reserved:0,key:s("f"),name:s("Field"),value:s("abc"),variant:0,reserved_variant:[0;6]};
        let value=PxuiValueFieldDesc {label:s("v"),value:s("1"),width:40.0,height:20.0,size:core::mem::size_of::<PxuiValueFieldDesc>() as u32,..unsafe {core::mem::zeroed()}};
        let area=PxuiTextAreaDesc {label:s("a"),value:s("x"),size:core::mem::size_of::<PxuiTextAreaDesc>() as u32,..unsafe {core::mem::zeroed()}};
        for frame in 0..2 {
            in_frame(ui,&mut |ui| {
                let calls: [&dyn Fn(*mut PxuiResponse)->i32; 5]=[
                    &|r| unsafe {pxui_ui_text_field(ui,&field,&mut Str::default(),&mut 0,r)},
                    &|r| unsafe {pxui_ui_search_field(ui,s("s"),s("/"),s("q"),s(""),s("Find"),2,0,&mut 0,&mut Str::default(),r)},
                    &|r| unsafe {pxui_ui_line_field(ui,0.0,30.0,80.0,20.0,0,s(""),s("l"),s("x"),s("Line"),&mut Str::default(),&mut 0,r)},
                    &|r| unsafe {pxui_ui_value_field(ui,&value,None,None,None,core::ptr::null_mut(),s("Scale"),&mut Str::default(),&mut 0,&mut 0,r)},
                    &|r| unsafe {pxui_ui_text_area(ui,&area,&pxui_types::Node::DEFAULT.w_px(80.0).h_px(40.0),core::ptr::null(),s("Notes"),&mut Str::default(),&mut 0,&mut 0,r)},
                ];
                for (i,call) in calls.iter().enumerate() {
                    let built=unsafe {&*(*ui).engine}.built_nodes();
                    let mut short=PxuiResponse {size:4,..PxuiResponse::EMPTY};
                    assert_eq!(call(&mut short),PXUI_ERR_ARGUMENT,"input {i}: a short record is refused");
                    assert_eq!(unsafe {&*(*ui).engine}.built_nodes(),built,"input {i}: refused before the widget runs");
                    let mut out=PxuiResponse::EMPTY;
                    assert_eq!(call(&mut out),0,"input {i}");
                    let (mut signal,mut focused)=(crate::PxuiSignal::default(),7u8);
                    assert_eq!(unsafe {crate::interaction::pxui_ui_signal(ui,&out.node,&mut signal)},0,"input {i}, frame {frame}: its own box is live");
                    assert_eq!(unsafe {crate::interaction::pxui_ui_focused(ui,&out.node,&mut focused)},0);
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
        let ui=unsafe {crate::ui::create_test_ui()};
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
        let (mut out,mut changed,mut submitted)=({ let mut record = Str::default(); record.ptr = b"sentinel".as_ptr(); record.len = 8; record },7u8,9u8);
        in_frame(ui,&mut |ui| {
            let built=unsafe {&*(*ui).engine}.built_nodes();
            for (i,desc) in cases.iter().enumerate() {
                assert_eq!(unsafe {crate::text_inputs::pxui_ui_text_area(ui,desc,&pxui_types::Node::DEFAULT,core::ptr::null(),Str::default(),&mut out,&mut changed,&mut submitted,&mut { pxui_types::ffi::PxuiResponse::EMPTY })},PXUI_ERR_ARGUMENT,"case {i}");
                assert_eq!(unsafe {&*(*ui).engine}.built_nodes(),built,"case {i} built nothing");
            }
        });
        assert_eq!(unsafe {pxui_ui_destroy(ui)},0);
    }

    #[test]
    fn text_input_options_query_refuses_null_arguments_without_writing() {
        use super::*;
        let ui=unsafe {crate::ui::create_test_ui()};
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
        assert!(super::same_environment(&env, &env.clone()));
        for change in 0..4 {
            let mut other = env;
            match change { 0 => other.safe_area[0] = 48.0, 1 => other.keyboard_started = 1.5, 2 => other.keyboard_visible = 0, _ => other.keyboard_from[3] = f32::NAN }
            assert!(!super::same_environment(&env, &other), "change {change}");
        }
        let keyboard = Keyboard::new(pxui_types::geom::Rect::new(0.0, 500.0, 390.0, 344.0), Keyboard::VALID);
        assert!(super::same_keyboard(&keyboard, &keyboard.clone()) && !super::same_keyboard(&keyboard, &Keyboard { flags: 0, ..keyboard }));
        // a pointer that is not finite is refused before either path
        for (x, y) in [(1.5, -2.0), (f64::NAN, 0.0), (f64::INFINITY, -0.0)] {
            let event = C::pointer(C::POINTER_MOVED, 0, x, y);
            let general = input::Event::from_raw(&input::wire::Raw { kind: event.kind, code: event.code, x, y, value: event.value, text: unsafe { super::text(event.text) }, touch: None })
                .filter(|_| x.is_finite() && y.is_finite());
            let got = super::translate_event(&event, event.surface, &mut 0);
            assert_eq!(format!("{got:?}"), format!("{general:?}"));
        }
    }

    #[test]
    fn completed_file_drop_events_preserve_paths_order_and_warm_storage() {
        use super::*;
        use pxui_types::Event as Wire;
        extern "C" fn mark_built(user: *mut c_void, _: *mut c_void) {
            unsafe { user.cast::<bool>().write(true); }
        }
        let ui = unsafe { crate::ui::create_test_ui() };
        let mut out = FrameOut::default();
        for (code, payload) in [(0, "/Music/名前.flac\0/Music/Björk.m4a"), (1, "transport limit exceeded"), (0, "")] {
            let backing = payload.to_owned();
            let event = { let mut record = Wire::default(); record.kind = Wire::FILES_DROPPED; record.code = code; record.x = -12.5; record.y = 31.25; record.text = pair(&backing); record };
            let frame = { let mut record = FrameIn::new(320.0, 240.0); record.events = pxui_types::ffi::Strided { ptr: &event, len: 1, stride: core::mem::size_of::<pxui_types::Event>() }; record };
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
        let empty = Wire::new(Wire::FILES_DROPPED);
        for invalid in [
            { let mut record = empty; record.code = 2; record },
            { let mut record = empty; record.x = f64::NAN; record },
            { let mut record = empty; record.y = f64::INFINITY; record },
            { let mut record = empty; record.text = { let mut record = Str::default(); record.ptr = core::ptr::null(); record.len = 1; record }; record },
            { let mut record = empty; record.text = { let mut record = Str::default(); record.ptr = bad_utf8.as_ptr(); record.len = 1; record }; record },
        ] {
            let frame = { let mut record = FrameIn::new(320.0, 240.0); record.events = pxui_types::ffi::Strided { ptr: &invalid, len: 1, stride: core::mem::size_of::<pxui_types::Event>() }; record };
            let mut built = false;
            out.size = 777;
            assert_eq!(unsafe {
                pxui_frame(ui, &frame, Some(mark_built), core::ptr::from_mut(&mut built).cast(), &mut out)
            }, PXUI_ERR_ARGUMENT);
            assert!(!built);
            assert_eq!(out.size, 777);
        }
        let frame = { let mut record = FrameIn::new(320.0, 240.0); record.events = pxui_types::ffi::Strided { ptr: &empty, len: 1, stride: core::mem::size_of::<pxui_types::Event>() }; record };
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
        let ui = unsafe { crate::ui::create_test_ui() };
        let mut packets = [Touch::default(); 32];
        let mut events = [Wire::default(); 32];
        let mut out = FrameOut::default();
        for tick in 0..182 {
            let count = [32, 1, 0, 3][tick % 4];
            for (index, packet) in packets.iter_mut().enumerate() {
                *packet = Touch::new(u64::MAX, u64::MAX - index as u64, [-1.25, 99.5], tick as f64 / 120.0, 0.375, (tick % 4) as u32);
                events[index] = { let mut record = Wire::default(); record.kind = Wire::TOUCH; record.text = { let mut record = Str::default(); record.ptr = (packet as *const Touch).cast(); record.len = core::mem::size_of::<Touch>(); record }; record };
            }
            let expected = packets;
            let frame = { let mut record = FrameIn::new(320.0, 240.0); record.time = tick as f64 / 120.0; record.events = pxui_types::ffi::Strided { ptr: events.as_ptr(), len: count, stride: core::mem::size_of::<pxui_types::Event>() }; record };
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
        let ui=unsafe {crate::ui::create_test_ui()};
        let mut packet=Touch::new(u64::MAX, 0xfedcba9876543210, [-1.25, 99.5], 10.0, 0.375, 0);
        let mut out=crate::FrameOut::default();
        for frame in 0..182 {
            packet.phase=(frame%4) as u32;
            let event={ let mut event = C::new(C::TOUCH); event.text.ptr = (&packet as *const Touch).cast(); event.text.len = core::mem::size_of::<Touch>(); event };
            mem_sys::local_reset();
            assert_eq!(unsafe {super::pxui_frame(ui,&{ let mut record = FrameIn::default(); record.events = pxui_types::ffi::Strided { ptr: &event, len: 1, stride: core::mem::size_of::<pxui_types::Event>() }; record.width = 320.0; record.height = 240.0; record.time = frame as f64; record },None,core::ptr::null_mut(),&mut out)},0);
            let mut input=pxui_types::ffi::Input::default();
            assert_eq!(unsafe {super::pxui_ui_input(ui,0,0,&mut input)},0);
            assert_eq!(input.events.len,1);
            let encoded=unsafe {&*input.events.ptr};
            assert_eq!(encoded.kind,C::TOUCH);
            assert_eq!(encoded.text.len,core::mem::size_of::<Touch>());
            assert_eq!(unsafe {encoded.text.ptr.cast::<Touch>().read_unaligned()},packet);
            if frame>=82 {assert_eq!(mem_sys::local_alloc_count(),0,"warm touch frame and routed read allocated");}
        }
        let input=FrameIn::new(320.0, 240.0);
        for field in 0..7 {
            let mut bad=packet;
            match field {0=>bad.phase=4,1=>bad.x=f64::NAN,2=>bad.y=f64::INFINITY,3=>bad.time=f64::NAN,4=>bad.pressure=-1.0,5=>bad.pressure=1.01,_=>bad.pressure=f32::NAN}
            let events=[C::key(C::KEY_PRESSED, input::Key::Meta.code()),{ let mut event = C::new(C::TOUCH); event.text.ptr = (&bad as *const Touch).cast(); event.text.len = core::mem::size_of::<Touch>(); event }];
            out.size=777;
            assert_eq!(unsafe {super::pxui_frame(ui,&{ let mut record = input; record.events = pxui_types::ffi::Strided { ptr: events.as_ptr(), len: 2, stride: core::mem::size_of::<pxui_types::Event>() }; record },None,core::ptr::null_mut(),&mut out)},crate::PXUI_ERR_ARGUMENT);
            assert_eq!(out.size,777);
            let engine=unsafe {&*(*ui).engine};
            assert!(!engine.key_down(input::Key::Meta));
            assert_eq!(unsafe {super::pxui_frame(ui,&input,None,core::ptr::null_mut(),&mut out)},0);
            assert!(!unsafe {&*(*ui).engine}.key_down(input::Key::Meta), "the rejected batch must leave the next frame's held keys intact");
        }
        for text in [Str::default(),{ let mut record = Str::default(); record.ptr = (&packet as *const Touch).cast(); record.len = 47; record }] {
            let event={ let mut event = C::new(C::TOUCH); event.text = text; event };
            assert_eq!(unsafe {super::pxui_frame(ui,&{ let mut record = input; record.events = pxui_types::ffi::Strided { ptr: &event, len: 1, stride: core::mem::size_of::<pxui_types::Event>() }; record },None,core::ptr::null_mut(),&mut out)},crate::PXUI_ERR_ARGUMENT);
        }
        // the record's own size: short, or past the payload, is refused; a longer payload than the record is read as its first bytes
        let mut frame=|touch:&Touch,len:usize| {
            let event={ let mut event = C::new(C::TOUCH); event.text.ptr = (touch as *const Touch).cast(); event.text.len = len; event };
            unsafe {super::pxui_frame(ui,&{ let mut record = input; record.events = pxui_types::ffi::Strided { ptr: &event, len: 1, stride: core::mem::size_of::<pxui_types::Event>() }; record },None,core::ptr::null_mut(),&mut out)}
        };
        let whole=Touch::new(1,2,[10.0,20.0],1.0,1.0,0);
        for size in [0,4,55] { assert_eq!(frame(&Touch {size,..whole},core::mem::size_of::<Touch>()),crate::PXUI_ERR_ARGUMENT,"touch size {size}"); }
        assert_eq!(frame(&Touch {size:57,..whole},core::mem::size_of::<Touch>()),crate::PXUI_ERR_ARGUMENT,"a record claiming more than its payload");
        let padded=[whole,whole];
        assert_eq!(frame(&padded[0],core::mem::size_of::<Touch>()),0);
        assert_eq!(frame(&Touch {size:64,..padded[0]},64),0,"a longer record");
        assert_eq!(unsafe {super::pxui_ui_destroy(ui)},0);
    }

    #[test]
    fn host_environment_rejects_invalid_geometry_before_mutating_a_frame() {
        use pxui_types::{Environment,FrameIn};
        let ui=unsafe { crate::ui::create_test_ui() };
        let value=Environment::new([47.0,0.0,34.0,0.0]).keyboard_animation([0.0,500.0,390.0,344.0],[0.0,844.0,390.0,344.0],1.0,0.25,7,true);
        let input={ let mut record = FrameIn::default(); record.width = 390.0; record.height = 844.0; record.environment = value; record };
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
            assert_eq!(unsafe {super::pxui_frame(ui,&{ let mut record = input; record.environment = bad; record },None,core::ptr::null_mut(),&mut out)},crate::PXUI_ERR_ARGUMENT);
            assert_eq!(out.size,123);
            assert_eq!(unsafe {crate::ui_state::pxui_ui_environment(ui,&mut got)},0);
            assert_eq!(got,value);
        }
        assert_eq!(unsafe {super::pxui_ui_destroy(ui)},0);
    }

    /// The frame record has one size: it is read whole, a longer one as its first `size_of::<FrameIn>()` bytes, a shorter one (an older
    /// layout included) or a null one refused; the system preferences are read from it.
    #[test]
    fn the_frame_record_has_one_size() {
        use pxui_types::FrameIn;
        let ui=unsafe { crate::ui::create_test_ui() };
        let mut out=crate::FrameOut::default();
        let full=core::mem::size_of::<FrameIn>() as u32;
        for size in [0,4,48,56,128,152,full-1] {
            out.size=777;
            let input={ let mut record = FrameIn::new(320.0,240.0); record.size = size; record };
            assert_eq!(unsafe { super::pxui_frame(ui,&input,None,core::ptr::null_mut(),&mut out) },crate::PXUI_ERR_ARGUMENT,"size {size}");
            assert_eq!(out.size,777);
        }
        assert_eq!(unsafe { super::pxui_frame(ui,core::ptr::null(),None,core::ptr::null_mut(),&mut out) },crate::PXUI_ERR_ARGUMENT);
        for size in [full,full+8,u32::MAX] {
            let input={ let mut record = FrameIn::new(320.0,240.0); record.size = size; record.appearance = 2; record.reduced_motion = 1; record };
            assert_eq!(unsafe { super::pxui_frame(ui,&input,None,core::ptr::null_mut(),&mut out) },0,"size {size}");
            let mut appearance=99;
            assert_eq!(unsafe { super::pxui_ui_system_appearance(ui,&mut appearance) },0);
            assert_eq!(appearance,2);
        }
        assert_eq!(unsafe { super::pxui_ui_system_appearance(ui,core::ptr::null_mut()) },crate::PXUI_ERR_ARGUMENT);
        // the embedded environment and keyboard records carry their own sizes: a short one is refused before anything changes
        let good=FrameIn::new(320.0,240.0);
        for size in [0,4,79] {
            out.size=777;
            let bad={ let mut record = good; record.environment = pxui_types::Environment { size,..good.environment }; record };
            assert_eq!(unsafe { super::pxui_frame(ui,&bad,None,core::ptr::null_mut(),&mut out) },crate::PXUI_ERR_ARGUMENT,"environment size {size}");
            assert_eq!(out.size,777);
        }
        for size in [0,4,23] {
            let bad={ let mut record = good; record.keyboard = pxui_types::Keyboard { size,..good.keyboard }; record };
            assert_eq!(unsafe { super::pxui_frame(ui,&bad,None,core::ptr::null_mut(),&mut out) },crate::PXUI_ERR_ARGUMENT,"keyboard size {size}");
        }
        // a longer one is read as its first bytes and stored with the library's own size
        let longer={ let mut record = good; record.environment = pxui_types::Environment { size:200,..pxui_types::Environment::new([7.0,0.0,0.0,0.0]) }; record };
        assert_eq!(unsafe { super::pxui_frame(ui,&longer,None,core::ptr::null_mut(),&mut out) },0);
        // the out records: the caller's size must cover the library's
        let mut environment=pxui_types::Environment::default();
        let mut keyboard=pxui_types::Keyboard::default();
        assert_eq!(unsafe { crate::ui_state::pxui_ui_environment(ui,&mut environment) },0);
        assert_eq!((environment.size as usize,environment.safe_area[0]),(core::mem::size_of::<pxui_types::Environment>(),7.0));
        for size in [0,4,79] {
            let mut short=pxui_types::Environment { size,..environment };
            assert_eq!(unsafe { crate::ui_state::pxui_ui_environment(ui,&mut short) },crate::PXUI_ERR_ARGUMENT);
            assert_eq!(short.size,size);
        }
        assert_eq!(unsafe { crate::ui_state::pxui_ui_keyboard(ui,&mut keyboard) },0);
        let mut short=pxui_types::Keyboard { size:20,..keyboard };
        assert_eq!(unsafe { crate::ui_state::pxui_ui_keyboard(ui,&mut short) },crate::PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { super::pxui_ui_destroy(ui) },0);
    }

    /// The numbering `input` encodes with is the one the C header and `pxui_types` document.
    #[test]
    fn wire_numbers_are_the_abi_numbers() {
        use input::wire as w;
        assert_eq!((w::KEY_SPECIAL, w::KEY_UNKNOWN), (pxui_types::KEY_SPECIAL, pxui_types::KEY_UNKNOWN));
        let pairs = [(w::KEY_PRESSED, C::KEY_PRESSED), (w::KEY_RELEASED, C::KEY_RELEASED), (w::POINTER_MOVED, C::POINTER_MOVED), (w::POINTER_PRESSED, C::POINTER_PRESSED),
            (w::POINTER_RELEASED, C::POINTER_RELEASED), (w::POINTER_CANCELLED, C::POINTER_CANCELLED), (w::SCROLLED, C::SCROLLED), (w::TEXT_INPUT, C::TEXT_INPUT),
            (w::PASTE, C::PASTE), (w::TEXT_EDITING, C::TEXT_EDITING), (w::FILE_DRAG_MOVED, C::FILE_DRAG_MOVED),
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
    fn a_frame_names_a_registered_surface_and_every_event_carries_it() {
        use pxui_types::{Event, Keyboard};
        let ui = unsafe { pxui_ui_create(&crate::PxuiUiDesc::default(), core::ptr::null(), 0, core::ptr::null_mut()) };
        let mut out = FrameOut::default();
        let frame = |ui, input: &FrameIn, out: &mut FrameOut| unsafe { super::pxui_frame(ui, input, None, core::ptr::null_mut(), out) };
        let input = { let mut record = FrameIn::default(); record.width = 390.0; record.height = 844.0; record.keyboard = Keyboard::new(pxui_types::geom::Rect::new(0.0, 508.0, 390.0, 336.0), Keyboard::VALID); record };
        // no surface registered yet: the frame is refused, whatever it names
        assert_eq!(frame(ui, &input, &mut out), PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { super::pxui_ui_add_surface(ui, 0) }, PXUI_ERR_ARGUMENT, "0 is no surface");
        assert_eq!(unsafe { super::pxui_ui_add_surface(core::ptr::null_mut(), 1) }, PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { super::pxui_ui_add_surface(ui, 7) }, 0);
        // a second surface, or the same twice: a state error (one surface per Ui for now)
        assert_eq!(unsafe { super::pxui_ui_add_surface(ui, 8) }, crate::PXUI_ERR_STATE);
        assert_eq!(unsafe { super::pxui_ui_add_surface(ui, 7) }, crate::PXUI_ERR_STATE);
        // an unknown surface: an argument error, the full record or a legacy one (a 152-byte record names the main surface)
        for surface in [0, 1, 8, u64::MAX] { assert_eq!(frame(ui, &{ let mut record = input; record.surface = surface; record }, &mut out), PXUI_ERR_ARGUMENT, "surface {surface}"); }
        assert_eq!(frame(ui, &{ let mut record = input; record.size = 152; record.surface = 7; record }, &mut out), PXUI_ERR_ARGUMENT, "a 152-byte record is for the main surface");
        assert_eq!(frame(ui, &{ let mut record = input; record.surface = 7; record }, &mut out), 0);
        let mut got = Keyboard::default();
        assert_eq!(unsafe { crate::ui_state::pxui_ui_keyboard(ui, &mut got) }, 0);
        assert_eq!(got, input.keyboard);
        // events: the frame's surface, a whole record, at most 4096 of them
        let on = |surface, size| [{ let mut record = Event::new(Event::FOCUS_GAINED); record.surface = surface; record.size = size; record }];
        let with = |events: &[Event]| { let mut record = input; record.surface = 7; record.events = pxui_types::ffi::Strided { ptr: events.as_ptr(), len: events.len(), stride: core::mem::size_of::<pxui_types::Event>() }; record };
        assert_eq!(frame(ui, &with(&on(7, 64)), &mut out), 0);
        assert_eq!(frame(ui, &with(&on(7, 100)), &mut out), 0, "a longer record is read as its first 64 bytes");
        assert_eq!(frame(ui, &with(&on(1, 64)), &mut out), PXUI_ERR_ARGUMENT, "an event for another surface");
        assert_eq!(frame(ui, &with(&on(0, 64)), &mut out), PXUI_ERR_ARGUMENT);
        assert_eq!(frame(ui, &with(&on(7, 48)), &mut out), PXUI_ERR_ARGUMENT, "a short record");
        assert_eq!(frame(ui, &with(&on(7, 0)), &mut out), PXUI_ERR_ARGUMENT);
        let many = vec![{ let mut record = Event::new(Event::FOCUS_GAINED); record.surface = 7; record }; 4097];
        assert_eq!(frame(ui, &with(&many[..4096]), &mut out), 0);
        assert_eq!(frame(ui, &with(&many), &mut out), PXUI_ERR_ARGUMENT, "more than 4096 events");
        // `pxui_ui_wants_frame` holds its events to the Ui's surface too
        let mut wants = 0u8;
        let slice = |events: &[Event]| pxui_types::ffi::Strided::of(events);
        assert_eq!(unsafe { crate::kit::pxui_ui_wants_frame(ui, slice(&on(7, 64)), 1.0, &mut wants) }, 0);
        assert_eq!(unsafe { crate::kit::pxui_ui_wants_frame(ui, slice(&on(1, 64)), 1.0, &mut wants) }, PXUI_ERR_ARGUMENT);
        // removing: an unknown id is an argument error; then the surface is free again
        assert_eq!(unsafe { super::pxui_ui_remove_surface(ui, 8) }, PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { super::pxui_ui_remove_surface(ui, 0) }, PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { super::pxui_ui_remove_surface(ui, 7) }, 0);
        assert_eq!(frame(ui, &{ let mut record = input; record.surface = 7; record }, &mut out), PXUI_ERR_ARGUMENT);
        for surface in [0, 7] { assert_eq!(unsafe { crate::kit::pxui_ui_wants_frame(ui, slice(&on(surface, 64)), 1.0, &mut wants) }, PXUI_ERR_ARGUMENT, "wants_frame with no registered surface, event surface {surface}"); }
        assert_eq!(unsafe { super::pxui_ui_add_surface(ui, 8) }, 0);
        assert_eq!(frame(ui, &{ let mut record = input; record.surface = 8; record }, &mut out), 0);
        assert_eq!(unsafe { super::pxui_ui_destroy(ui) }, 0);
    }

    #[test]
    fn a_handle_of_another_ui_or_a_zeroed_one_is_stale_on_every_path() {
        use pxui_types::{PxuiBox, ffi::PxuiSignal};
        extern "C" fn declare(user: *mut core::ffi::c_void, ui: *mut core::ffi::c_void) {
            assert_eq!(unsafe { crate::pxui_ui_node_keyed(ui.cast(), 4242, &pxui_types::Node::DEFAULT, &mut *user.cast::<PxuiBox>()) }, 0);
        }
        let (a, b) = (unsafe { crate::ui::create_test_ui() }, unsafe { crate::ui::create_test_ui() });
        let (mut mine, mut theirs) = (PxuiBox::default(), PxuiBox::default());
        let mut out = FrameOut::default();
        let input = FrameIn::new(100.0, 100.0);
        assert_eq!(unsafe { super::pxui_frame(a, &input, Some(declare), (&mut mine as *mut PxuiBox).cast(), &mut out) }, 0);
        assert_eq!(unsafe { super::pxui_frame(b, &input, Some(declare), (&mut theirs as *mut PxuiBox).cast(), &mut out) }, 0);
        // the two handles name the same slot and key and differ in the serial only
        assert_ne!(mine.ui, 0);
        assert_ne!(mine.ui, theirs.ui);
        assert_eq!((mine.index, mine.box_key, mine.box_slot, mine.generation), (theirs.index, theirs.box_key, theirs.box_slot, theirs.generation));
        // refused: a stale-box status where the export has one, an argument error where it never had
        let refused = |rc: i32| rc == crate::PXUI_ERR_STALE || rc == PXUI_ERR_ARGUMENT;
        let zeroed = PxuiBox::default();
        let wrong = [theirs, zeroed, PxuiBox { ui: u32::MAX, ..mine }, PxuiBox { ui: 0, ..mine }];
        let mut rect = pxui_types::geom::Rect::new(0.0, 0.0, 0.0, 0.0);
        let (mut value, mut focused, mut signal) = (0i64, 0u8, PxuiSignal::default());
        let text = { let mut record = pxui_types::Str::default(); record.ptr = b"x".as_ptr(); record.len = 1; record };
        // the trusted calls (rect, text), the checked ones (state, focused, signal) and the core-box ones (patch_text, a11y name)
        assert_eq!(unsafe { crate::pxui_ui_rect(a, &mine, &mut rect) }, 0);
        assert_eq!(unsafe { crate::pxui_ui_state(a, &mine, 7, &mut value) }, 0);
        assert_eq!(unsafe { crate::pxui_ui_focused(a, &mine, &mut focused) }, 0);
        assert_eq!(unsafe { crate::pxui_ui_signal(a, &mine, &mut signal) }, 0);
        for bad in &wrong {
            assert!(refused(unsafe { crate::pxui_ui_rect(a, bad, &mut rect) }), "rect {bad:?}");
            assert!(refused(unsafe { crate::pxui_ui_state(a, bad, 7, &mut value) }), "state {bad:?}");
            assert!(refused(unsafe { crate::pxui_ui_focused(a, bad, &mut focused) }), "focused {bad:?}");
            assert!(refused(unsafe { crate::pxui_ui_signal(a, bad, &mut signal) }), "signal {bad:?}");
            assert!(refused(unsafe { crate::pxui_ui_a11y_name(a, bad, text) }), "a11y name {bad:?}");
            assert!(refused(unsafe { crate::pxui_ui_set_state(a, bad, 1) }), "set_state {bad:?}");
        }
        // the other Ui's own handle works there, and refuses ours
        assert_eq!(unsafe { crate::pxui_ui_rect(b, &theirs, &mut rect) }, 0);
        assert!(refused(unsafe { crate::pxui_ui_rect(b, &mine, &mut rect) }));
        assert_eq!(unsafe { super::pxui_ui_destroy(a) } | unsafe { super::pxui_ui_destroy(b) }, 0);
    }

    /// The builder calls that take a box (`within`, `text`, `tip`) refuse another `Ui`'s handle and a zeroed one with the stale status,
    /// inside a frame, and do the same call with the `Ui`'s own handle.
    #[test]
    fn builder_calls_refuse_another_uis_handle_as_stale() {
        use pxui_types::PxuiBox;
        struct Case { mine: PxuiBox, wrong: [PxuiBox; 3], checked: bool }
        extern "C" fn declare(user: *mut core::ffi::c_void, ui: *mut core::ffi::c_void) {
            let case = unsafe { &mut *user.cast::<Case>() };
            let ui = ui.cast();
            assert_eq!(unsafe { crate::pxui_ui_node_keyed(ui, 4242, &pxui_types::Node::DEFAULT, &mut case.mine) }, 0);
            let text = { let mut record = pxui_types::Str::default(); record.ptr = b"x".as_ptr(); record.len = 1; record };
            extern "C" fn nothing(_: *mut core::ffi::c_void, _: *mut core::ffi::c_void) {}
            assert_eq!(unsafe { crate::pxui_ui_within(ui, &case.mine, Some(nothing), core::ptr::null_mut()) }, 0);
            assert_eq!(unsafe { crate::pxui_ui_text(ui, &case.mine, text, core::ptr::null(), core::ptr::null(), 0) }, 0);
            assert_eq!(unsafe { crate::pxui_ui_tip(ui, &case.mine, text) }, 0);
            for bad in &case.wrong {
                assert_eq!(unsafe { crate::pxui_ui_within(ui, bad, Some(nothing), core::ptr::null_mut()) }, crate::PXUI_ERR_STALE, "within {bad:?}");
                assert_eq!(unsafe { crate::pxui_ui_text(ui, bad, text, core::ptr::null(), core::ptr::null(), 0) }, crate::PXUI_ERR_STALE, "text {bad:?}");
                assert_eq!(unsafe { crate::pxui_ui_tip(ui, bad, text) }, crate::PXUI_ERR_STALE, "tip {bad:?}");
            }
            case.checked = true;
        }
        let (a, b) = (unsafe { crate::ui::create_test_ui() }, unsafe { crate::ui::create_test_ui() });
        let mut out = FrameOut::default();
        let input = FrameIn::new(100.0, 100.0);
        let mut theirs = Case { mine: PxuiBox::default(), wrong: [PxuiBox::default(); 3], checked: false };
        // the other Ui's box lives in `b`; a zeroed one and one with the serial cleared come from it too
        assert_eq!(unsafe { super::pxui_frame(b, &input, Some(declare), (&mut theirs as *mut Case).cast(), &mut out) }, 0);
        let other = theirs.mine;
        let mut case = Case { mine: PxuiBox::default(), wrong: [other, PxuiBox::default(), PxuiBox { ui: 0, ..other }], checked: false };
        assert_eq!(unsafe { super::pxui_frame(a, &input, Some(declare), (&mut case as *mut Case).cast(), &mut out) }, 0);
        assert!(case.checked);
        assert_eq!(unsafe { super::pxui_ui_destroy(a) } | unsafe { super::pxui_ui_destroy(b) }, 0);
    }

    /// `within` compares the Ui serial only. A handle of this Ui that is dead (made in an earlier frame, a forged index, slot or generation)
    /// must still be harmless: the builder inside it runs, the frame completes and no other box's children change.
    #[test]
    fn within_a_dead_or_forged_handle_of_this_ui_is_harmless() {
        use pxui_types::PxuiBox;
        struct Case { saved: PxuiBox, tries: [PxuiBox; 4], codes: [i32; 4], within: [i32; 4], asked: usize, frame: u32, built: u32 }
        extern "C" fn child(user: *mut core::ffi::c_void, ui: *mut core::ffi::c_void) {
            let case = unsafe { &mut *user.cast::<Case>() };
            let mut out = PxuiBox::default();
            // a forged parent may make the declare refuse; it must not be anything else
            let code = unsafe { crate::pxui_ui_node_keyed(ui.cast(), 9001, &pxui_types::Node::DEFAULT, &mut out) };
            case.built += 1;
            case.codes[(case.built as usize - 1) % 4] = code;
        }
        extern "C" fn declare(user: *mut core::ffi::c_void, ui: *mut core::ffi::c_void) {
            let case = unsafe { &mut *user.cast::<Case>() };
            let ui = ui.cast();
            if case.frame == 0 {
                assert_eq!(unsafe { crate::pxui_ui_node_keyed(ui, 4242, &pxui_types::Node::DEFAULT, &mut case.saved) }, 0);
            } else if case.frame == 2 {
                // the same box declared in this frame is live: its builder runs
                let mut mine = PxuiBox::default();
                assert_eq!(unsafe { crate::pxui_ui_node_keyed(ui, 4242, &pxui_types::Node::DEFAULT, &mut mine) }, 0);
                assert_eq!(unsafe { crate::pxui_ui_within(ui, &mine, Some(child), (case as *mut Case).cast()) }, 0);
            } else {
                for t in case.tries {
                    let code = unsafe { crate::pxui_ui_within(ui, &t, Some(child), (case as *mut Case).cast()) };
                    case.within[case.asked] = code;
                    case.asked += 1;
                }
            }
        }
        let ui = unsafe { crate::ui::create_test_ui() };
        let mut out = FrameOut::default();
        let input = FrameIn::new(100.0, 100.0);
        let mut case = Case { saved: PxuiBox::default(), tries: [PxuiBox::default(); 4], codes: [0; 4], within: [0; 4], asked: 0, frame: 0, built: 0 };
        assert_eq!(unsafe { super::pxui_frame(ui, &input, Some(declare), (&mut case as *mut Case).cast(), &mut out) }, 0);
        let s = case.saved;
        case.tries = [s, PxuiBox { index: u64::MAX >> 1, ..s }, PxuiBox { box_slot: u64::MAX >> 1, generation: s.generation.wrapping_add(7), ..s }, PxuiBox { index: 1 << 40, box_key: 77, ..s }];
        case.frame = 1;
        assert_eq!(unsafe { super::pxui_frame(ui, &input, Some(declare), (&mut case as *mut Case).cast(), &mut out) }, 0);
        // a dead handle (its box is not in this frame's build) and the forged ones are stale and run no builder
        assert_eq!(case.within, [crate::PXUI_ERR_STALE; 4], "{:?}", case.within);
        assert_eq!(case.built, 0);
        case.frame = 2;
        assert_eq!(unsafe { super::pxui_frame(ui, &input, Some(declare), (&mut case as *mut Case).cast(), &mut out) }, 0);
        assert_eq!((case.built, case.codes[0]), (1, 0));
        assert_eq!(unsafe { super::pxui_ui_destroy(ui) }, 0);
    }

    #[test]
    fn keyboard_tail_validates_before_mutation() {
        use pxui_types::{Environment, Keyboard};
        let ui=unsafe {crate::ui::create_test_ui()};
        let keyboard=Keyboard::new(pxui_types::geom::Rect::new(0.0, 508.0, 390.0, 336.0), Keyboard::VALID);
        let environment=Environment::new([47.0,0.0,34.0,0.0]);
        let input={ let mut record = FrameIn::default(); record.width = 390.0; record.height = 844.0; record.environment = environment; record.keyboard = keyboard; record };
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
            assert_eq!(unsafe {super::pxui_frame(ui,&{ let mut record = input; record.keyboard = bad; record },None,core::ptr::null_mut(),&mut out)},PXUI_ERR_ARGUMENT);
            assert_eq!(out.size,123);
            assert_eq!(unsafe {crate::ui_state::pxui_ui_keyboard(ui,&mut got)},0);
            assert_eq!(got,keyboard);
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
            B { whole_pixels: 2, ..full }] {
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

#[cfg(test)]
mod renderer_lifetime_tests {
    /// OS-12: a `Ui` destroyed while renderers are attached goes with the last renderer, in any drop order; nothing waits in the wrapper.
    #[test]
    fn a_ui_destroyed_first_goes_with_the_last_renderer() {
        let ui = unsafe { super::create_test_ui() };
        let shared = unsafe { (*ui).shared };
        unsafe { (*shared).renderers = 2; }
        assert_eq!(unsafe { crate::pxui_ui_destroy(ui) }, 0, "the destroy is accepted");
        assert!(unsafe { (*shared).destroy_pending && (*shared).renderers == 2 }, "and held for the renderers");
        unsafe { super::renderer_released(shared); }
        assert_eq!(unsafe { ((*shared).renderers, (*shared).destroy_pending) }, (1, true), "one renderer left: the Ui is still there");
        unsafe { super::renderer_released(shared); } // the last one frees it
    }
}
