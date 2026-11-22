//! Retained opacity is per instance. Clip coordinates are window points and NULL clears it. Scale
//! accepts finite zero/negative values and resets previous rotation/fixed-pivot transforms. Equal
//! controls preserve transform storage and retained instances.
//!
//! Boxes and what hangs off them: patches, scroll and text state, carries and drops, view transforms.
use crate::state::{PxuiUi, boxed, checked, engine, fail, finite};
use crate::ui::key_code;
use crate::style::decode_look;
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str};
use pxui_core::ui::{Mark, Rect};
use pxui_types::{Dir, ffi::{PxuiLookDesc, PxuiMarkDesc, Slice}};

#[cfg(test)]
mod reveal_tests {
    use super::*;
    use core::ffi::c_void;

    #[test]
    fn reveal_checks_handles_and_callback_ownership() {
        struct State { owner: *mut PxuiUi, list: PxuiBox }
        extern "C" fn build(user: *mut c_void, raw: *mut c_void) {
            let state = unsafe { &mut *user.cast::<State>() };
            let desc = pxui_types::ffi::PxuiGridListDesc {
                size: core::mem::size_of::<pxui_types::ffi::PxuiGridListDesc>() as u32,
                reserved: 0, label: { let mut record = Str::default(); record.ptr = b"rows".as_ptr(); record.len = 4; record }, count: 100,
                cell_w: 100.0, cell_h: 20.0, build: None, user: core::ptr::null_mut(),
            };
            assert_eq!(unsafe { crate::pxui_ui_grid_list(raw.cast(), &desc, &mut state.list) }, 0);
            assert_eq!(unsafe { pxui_ui_reveal(state.owner, &state.list, 99) }, crate::PXUI_ERR_STATE);
            assert_eq!(unsafe { pxui_ui_reveal(raw.cast(), &state.list, 0) }, 0);
        }
        let owner = unsafe { crate::ui::create_test_ui() };
        let mut state = State { owner, list: PxuiBox::default() };
        let mut out = crate::FrameOut::default();
        assert_eq!(unsafe { crate::pxui_frame(owner, &crate::FrameIn::new(100.0, 100.0), Some(build), (&mut state as *mut State).cast(), &mut out) }, 0);
        assert_eq!(unsafe { pxui_ui_reveal(core::ptr::null_mut(), &state.list, 99) }, PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { pxui_ui_reveal(owner, core::ptr::null(), 99) }, PXUI_ERR_ARGUMENT);
        let invalid = PxuiBox { box_slot: u64::MAX, ..state.list };
        assert_eq!(unsafe { pxui_ui_reveal(owner, &invalid, 99) }, crate::PXUI_ERR_STALE);
        assert_eq!(unsafe { pxui_ui_reveal(owner, &state.list, 99) }, 0);
        let mut offset = 0.0;
        assert_eq!(unsafe { pxui_ui_scroll_offset(owner, &state.list, 0, &mut offset) }, 0);
        assert_eq!(offset, 1900.0);
        for index in [100, usize::MAX - 1, usize::MAX] {
            assert_eq!(unsafe { pxui_ui_reveal(owner, &state.list, index) }, 0);
        }
        assert_eq!(unsafe { pxui_ui_scroll_offset(owner, &state.list, 0, &mut offset) }, 0);
        assert_eq!(offset, 1900.0);
        assert_eq!(unsafe { crate::pxui_ui_destroy(owner) }, 0);
    }
}

#[cfg(test)]
mod gesture_tests {
    use super::*;
    use core::ffi::c_void;
    use pxui_types::ffi::{PxuiDismissIn, PxuiDismissOut, PxuiGestureIn, PxuiGestureOut, PxuiSwipeIn};

    #[test]
    fn gesture_validates_every_argument_before_changing_the_node() {
        extern "C" fn build(user: *mut c_void, raw: *mut c_void) {
            let node = unsafe { &mut *user.cast::<PxuiBox>() };
            let desc = pxui_types::ffi::PxuiGridListDesc {
                size: core::mem::size_of::<pxui_types::ffi::PxuiGridListDesc>() as u32, reserved: 0,
                label: { let mut record = Str::default(); record.ptr = b"rows".as_ptr(); record.len = 4; record }, count: 10, cell_w: 100.0, cell_h: 20.0, build: None, user: core::ptr::null_mut(),
            };
            assert_eq!(unsafe { crate::pxui_ui_grid_list(raw.cast(), &desc, node) }, 0);
            let sentinel = || PxuiGestureOut { flags: 99, swipe: 1.0, back: 2.0, pull: 3.0, ..Default::default() };
            let mut out = sentinel();
            let good = PxuiGestureIn { kinds: 2, reveal: 80.0, ..Default::default() };
            let bad = [PxuiGestureIn { kinds: 16, ..good }, PxuiGestureIn { kinds: 64, ..good }, PxuiGestureIn { reveal: f32::NAN, ..good }, PxuiGestureIn { threshold: f32::INFINITY, ..good }, PxuiGestureIn { size: 8, ..good }];
            for request in &bad {
                assert_eq!(unsafe { pxui_ui_gesture(raw.cast(), node, request, 0, &mut out) }, PXUI_ERR_ARGUMENT);
            }
            assert_eq!(unsafe { pxui_ui_gesture(raw.cast(), node, &good, 4, &mut out) }, PXUI_ERR_ARGUMENT);
            // ABI 1.40 swipe actions: refused requests change nothing, a good one takes effect
            let swipe = PxuiSwipeIn { leading: 60.0, trailing: 80.0, full: 150.0, ..Default::default() };
            for request in [PxuiSwipeIn { leading: f32::NAN, ..swipe }, PxuiSwipeIn { full: f32::INFINITY, ..swipe }, PxuiSwipeIn { size: 8, ..swipe }] {
                assert_eq!(unsafe { pxui_ui_swipe_actions(raw.cast(), node, &request) }, PXUI_ERR_ARGUMENT);
            }
            assert_eq!(unsafe { pxui_ui_swipe_actions(raw.cast(), node, core::ptr::null()) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { pxui_ui_swipe_actions(raw.cast(), core::ptr::null(), &swipe) }, PXUI_ERR_ARGUMENT);
            let stale_swipe = PxuiBox { box_slot: u64::MAX, ..*node };
            assert_eq!(unsafe { pxui_ui_swipe_actions(raw.cast(), &stale_swipe, &swipe) }, crate::PXUI_ERR_STALE);
            assert_eq!(unsafe { pxui_ui_swipe_actions(raw.cast(), node, &swipe) }, 0);
            // ABI 1.41 dismiss: refused requests change nothing, a good one reads rest
            let ask = PxuiDismissIn { fraction: 0.3, velocity: 700.0, ..Default::default() };
            let mut read = PxuiDismissOut::default();
            for request in [PxuiDismissIn { fraction: f32::NAN, ..ask }, PxuiDismissIn { velocity: f32::INFINITY, ..ask }, PxuiDismissIn { size: 8, ..ask }] {
                assert_eq!(unsafe { pxui_ui_dismiss(raw.cast(), node, &request, &mut read) }, PXUI_ERR_ARGUMENT);
            }
            assert_eq!(unsafe { pxui_ui_dismiss(raw.cast(), node, core::ptr::null(), &mut read) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { pxui_ui_dismiss(raw.cast(), node, &ask, core::ptr::null_mut()) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { pxui_ui_dismiss(raw.cast(), core::ptr::null(), &ask, &mut read) }, PXUI_ERR_ARGUMENT);
            let stale_dismiss = PxuiBox { box_slot: u64::MAX, ..*node };
            assert_eq!(unsafe { pxui_ui_dismiss(raw.cast(), &stale_dismiss, &ask, &mut read) }, crate::PXUI_ERR_STALE);
            assert_eq!(unsafe { pxui_ui_dismiss(raw.cast(), node, &ask, &mut read) }, 0);
            assert_eq!((read.flags, read.offset), (0, 0.0));
            let mut short = 4u32;
            assert_eq!(unsafe { pxui_ui_gesture(raw.cast(), node, &good, 0, (&mut short as *mut u32).cast()) }, PXUI_ERR_ARGUMENT);
            assert_eq!(unsafe { pxui_ui_gesture(raw.cast(), node, &good, 0, core::ptr::null_mut()) }, PXUI_ERR_ARGUMENT);
            assert_eq!((out.flags, out.swipe, out.back, out.pull, short), (99, 1.0, 2.0, 3.0, 4), "refusals write nothing");
            assert_eq!(unsafe { pxui_ui_gesture(raw.cast(), node, core::ptr::null(), 0, &mut out) }, 0);
            assert_eq!(out.flags, 0);
            assert_eq!(unsafe { pxui_ui_gesture(raw.cast(), node, &good, 3, &mut out) }, 0);
            let stale = PxuiBox { box_slot: u64::MAX, ..*node };
            assert_eq!(unsafe { pxui_ui_gesture(raw.cast(), &stale, &good, 0, &mut out) }, crate::PXUI_ERR_STALE);
            assert_eq!(unsafe { pxui_ui_gesture(raw.cast(), core::ptr::null(), &good, 0, &mut out) }, PXUI_ERR_ARGUMENT);
        }
        let ui = unsafe { crate::ui::create_test_ui() };
        let mut node = PxuiBox::default();
        let mut out = crate::FrameOut::default();
        assert_eq!(unsafe { crate::pxui_frame(ui, &crate::FrameIn::new(100.0, 100.0), Some(build), (&mut node as *mut PxuiBox).cast(), &mut out) }, 0);
        assert_eq!(unsafe { pxui_ui_gesture(core::ptr::null_mut(), &node, core::ptr::null(), 0, &mut PxuiGestureOut::default()) }, PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { crate::pxui_ui_destroy(ui) }, 0);
    }
}

export!(build pub unsafe fn pxui_ui_set_at(ui: *mut PxuiUi, box_: *const PxuiBox, x: f32, y: f32) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if !finite(&[x, y]) { return unsafe { fail(ui, "position is not finite") }; }
    e.set_at(&b, (x, y));
    0
});

export!(pub unsafe fn pxui_ui_patch_look(ui: *mut PxuiUi, box_: *const PxuiBox, value: *const PxuiLookDesc, out: *mut u8) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if value.is_null() || out.is_null() { return unsafe { fail(ui, "null patch argument") }; }
    let Some(value) = read_desc!(value, PxuiLookDesc).and_then(decode_look) else { return unsafe { fail(ui, "invalid look") }; };
    unsafe { *out = e.patch_look(&b, &value) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_patch_rect(ui: *mut PxuiUi, box_: *const PxuiBox, value: Rect, out: *mut u8) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if out.is_null() || !finite(&[value.x, value.y, value.w, value.h]) { return unsafe { fail(ui, "invalid patch rectangle") }; }
    unsafe { *out = e.patch_rect(&b, value) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_patch_mark(ui: *mut PxuiUi, box_: *const PxuiBox, value: *const PxuiMarkDesc, out: *mut u8) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if value.is_null() || out.is_null() { return unsafe { fail(ui, "null patch argument") }; }
    let value = unsafe { *value };
    let mark = match value.kind {
        0 => Mark::None,
        1 => Mark::Chevron(match value.direction { 0 => Dir::Down, 1 => Dir::Up, 2 => Dir::Left, 3 => Dir::Right, _ => return unsafe { fail(ui, "unknown mark direction") } }),
        2 if value.width.is_finite() && value.rising <= 1 => Mark::Line { width: value.width, rising: value.rising != 0 },
        _ => return unsafe { fail(ui, "invalid mark") },
    };
    unsafe { *out = e.patch_mark(&b, mark) as u8; }
    0
});

// The look, mark and inset of a node declared this frame, to read (`pxui_ui_node_fields`) and to write back whole
// (`pxui_ui_set_node_fields`), as the engine's `node_mut` lets a widget finish a node once its signal is known.
export!(pub unsafe fn pxui_ui_node_fields(ui: *mut PxuiUi, box_: *const PxuiBox, out: *mut pxui_types::ffi::PxuiNodeFields) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    // the caller's size first: a shorter record is refused, never written past
    if out.is_null() || unsafe { out.cast::<u32>().read_unaligned() } < core::mem::size_of::<pxui_types::ffi::PxuiNodeFields>() as u32 { return unsafe { fail(ui, "invalid node output") }; }
    if !e.live(&b) { return unsafe { crate::state::fail_code(ui, "stale box", crate::PXUI_ERR_STALE) }; }
    let n = e.node_mut(&b);
    let (kind, direction, width, rising) = match n.mark { Mark::None => (0, 0, 0.0, 0), Mark::Chevron(d) => (1, match d { Dir::Down => 0, Dir::Up => 1, Dir::Left => 2, Dir::Right => 3 }, 0.0, 0), Mark::Line { width, rising } => (2, 0, width, rising as u8) };
    unsafe { *out = pxui_types::ffi::PxuiNodeFields { size: core::mem::size_of::<pxui_types::ffi::PxuiNodeFields>() as u32, reserved: 0, look: n.look.into(), mark: PxuiMarkDesc { kind, direction, width, rising, reserved: [0; 3] }, inset: n.inset }; }
    0
});

export!(pub unsafe fn pxui_ui_set_node_fields(ui: *mut PxuiUi, box_: *const PxuiBox, fields: *const pxui_types::ffi::PxuiNodeFields) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    let Some(f) = read_desc!(fields, pxui_types::ffi::PxuiNodeFields) else { return unsafe { fail(ui, "invalid node fields") }; };
    let mark = match f.mark.kind {
        0 => Mark::None,
        1 => Mark::Chevron(match f.mark.direction { 0 => Dir::Down, 1 => Dir::Up, 2 => Dir::Left, 3 => Dir::Right, _ => return unsafe { fail(ui, "unknown mark direction") } }),
        2 if f.mark.width.is_finite() && f.mark.rising <= 1 => Mark::Line { width: f.mark.width, rising: f.mark.rising != 0 },
        _ => return unsafe { fail(ui, "invalid mark") },
    };
    let Some(look) = decode_look(f.look) else { return unsafe { fail(ui, "invalid look") }; };
    if !finite(&[f.inset.l, f.inset.t, f.inset.r, f.inset.b]) { return unsafe { fail(ui, "invalid inset") }; }
    if !e.live(&b) { return unsafe { crate::state::fail_code(ui, "stale box", crate::PXUI_ERR_STALE) } }
    let look = e.resolve_look(&look).unwrap_or(look);
    let n = e.node_mut(&b);
    (n.look, n.mark, n.inset) = (look, mark, f.inset);
    0
});

export!(pub unsafe fn pxui_ui_local_rect(ui: *mut PxuiUi, box_: *const PxuiBox, out: *mut Rect) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if out.is_null() { return unsafe { fail(ui, "null rectangle output") }; }
    unsafe { *out = e.local_rect(&b); }
    0
});

// `which`: 0 the scroll offset, 1 the painted position (with the elastic edge).
export!(pub unsafe fn pxui_ui_scroll_offset(ui: *mut PxuiUi, box_: *const PxuiBox, which: u32, out: *mut f32) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if out.is_null() || which > 1 { return unsafe { fail(ui, "invalid scroll query") }; }
    unsafe { *out = if which == 0 { e.scroll_offset(&b) } else { e.scroll_position(&b) }; }
    0
});

export!(pub unsafe fn pxui_ui_set_scroll_offset(ui: *mut PxuiUi, box_: *const PxuiBox, value: f32) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if !value.is_finite() { return unsafe { fail(ui, "scroll offset is not finite") }; }
    e.set_scroll_offset(&b, value);
    0
});

export!(pub unsafe fn pxui_ui_reveal(ui: *mut PxuiUi, list: *const PxuiBox, index: usize) -> i32 {
    let (engine, list) = match unsafe { boxed(ui, list) } { Ok(v) => v, Err(code) => return code };
    engine.reveal(&list, index);
    0
});

// ABI 1.38: read, and with `in_` set, request a node's touch gestures; `commands` bit 0 closes a swiped
// row, bit 1 ends a refresh. Every argument is checked before the node changes.
export!(pub unsafe fn pxui_ui_gesture(ui: *mut PxuiUi, box_: *const PxuiBox, in_: *const pxui_types::ffi::PxuiGestureIn, commands: u32, out: *mut pxui_types::ffi::PxuiGestureOut) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "invalid gesture output") }; };
    let cfg = if in_.is_null() { None } else {
        let Some(request) = read_desc!(in_, pxui_types::ffi::PxuiGestureIn) else { return unsafe { fail(ui, "invalid gesture request") }; };
        if request.kinds & !47 != 0 || !request.reveal.is_finite() || !request.threshold.is_finite() {
            return unsafe { fail(ui, "invalid gesture request") };
        }
        Some(pxui_core::ui::gesture::GestureCfg { kinds: request.kinds, reveal: request.reveal, threshold: request.threshold, ..Default::default() })
    };
    if commands & !3 != 0 { return unsafe { fail(ui, "invalid gesture command") }; }
    let state = e.gesture(&b, cfg, commands);
    *out = pxui_types::ffi::PxuiGestureOut { flags: state.flags, swipe: state.swipe, back: state.back, pull: state.pull, ..Default::default() };
    0
});

// ABI 1.40: enable a node's row swipe with actions on both sides and a full-swipe commit distance, keeping its
// other gestures; call after `pxui_ui_gesture` each build. Flags 512/1024 (commit) and 2048/4096 (open side) join
// the gesture flags.
export!(pub unsafe fn pxui_ui_swipe_actions(ui: *mut PxuiUi, box_: *const PxuiBox, in_: *const pxui_types::ffi::PxuiSwipeIn) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    let Some(request) = sized!(in_) else { return unsafe { fail(ui, "invalid swipe request") }; };
    if !request.leading.is_finite() || !request.trailing.is_finite() || !request.full.is_finite() { return unsafe { fail(ui, "invalid swipe request") }; }
    e.swipe_actions(&b, request.leading, request.trailing, request.full);
    0
});

// ABI 1.41: take the vertical drag-to-dismiss on a node (after `pxui_ui_gesture` each build) and read it.
export!(pub unsafe fn pxui_ui_dismiss(ui: *mut PxuiUi, box_: *const PxuiBox, in_: *const pxui_types::ffi::PxuiDismissIn, out: *mut pxui_types::ffi::PxuiDismissOut) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    let Some(request) = sized!(in_) else { return unsafe { fail(ui, "invalid dismiss request") }; };
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "invalid dismiss output") }; };
    if !request.fraction.is_finite() || !request.velocity.is_finite() { return unsafe { fail(ui, "invalid dismiss request") }; }
    let state = e.dismiss(&b, request.fraction, request.velocity);
    *out = pxui_types::ffi::PxuiDismissOut { flags: state.flags, offset: state.offset, ..Default::default() };
    0
});

export!(pub unsafe fn pxui_ui_text_state(ui: *mut PxuiUi, box_: *const PxuiBox, present: *mut u8, out: *mut Str) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if present.is_null() || out.is_null() { return unsafe { fail(ui, "null text state output") }; }
    let value = e.text_state(&b);
    let given = match value { Some(value) => match unsafe { &*ui }.give(value) { Ok(value) => Some(value), Err(code) => return code }, None => None };
    unsafe {
        *present = given.is_some() as u8;
        *out = given.unwrap_or_default();
    }
    0
});

export!(pub unsafe fn pxui_ui_set_text_state(ui: *mut PxuiUi, box_: *const PxuiBox, present: u8, value: Str) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    let Some(value) = (unsafe { checked(value) }).filter(|_| present <= 1) else { return unsafe { fail(ui, "invalid text state") }; };
    e.set_text_state(&b, (present != 0).then(|| value.to_owned()));
    0
});

// The text kept in the box's slot under `index`: borrowed from the slot, valid until the next `cached_label` of the
// same box or the next frame.
export!(build pub unsafe fn pxui_ui_cached_label(ui: *mut PxuiUi, box_: *const PxuiBox, index: u64, text: Str, out: *mut Str) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    let Some(text) = (unsafe { checked(text) }) else { return unsafe { fail(ui, "invalid label") }; };
    if out.is_null() || index > 1 << 20 { return unsafe { fail(ui, "invalid cached label arguments") }; }
    let label = e.cached_label(&b, index as usize, text);
    unsafe { *out = Str { ptr: label.as_ptr(), len: label.len() }; }
    0
});

export!(build pub unsafe fn pxui_ui_to_front(ui: *mut PxuiUi, order: i64, box_: *const PxuiBox) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if !e.is_current_build_box(&b) { return unsafe { fail(ui, "to_front needs a box of this frame's build") }; }
    e.to_front(order, &b);
    0
});

// ------------------------------------------------------------------ carry and drop

export!(pub unsafe fn pxui_ui_carrying(ui: *mut PxuiUi, present: *mut u8, kind: *mut Str, value: *mut Str) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if present.is_null() || kind.is_null() || value.is_null() { return unsafe { fail(ui, "null carry output") }; }
    let payload = e.carrying_ref();
    let lent = match payload { Some(p) => match unsafe { lend_payload(ui, &p.kind, &p.value) } { Ok(pair) => pair, Err(code) => return code }, None => (Str::default(), Str::default()) };
    unsafe {
        *present = payload.is_some() as u8;
        (*kind, *value) = lent;
    }
    0
}); 

export!(pub unsafe fn pxui_ui_carry(ui: *mut PxuiUi, has_from: u8, from: *const PxuiBox, kind: Str, value: Str) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let (Some(kind), Some(value)) = (unsafe { crate::state::checked_key(kind) }, unsafe { checked(value) }) else { return unsafe { fail(ui, "invalid carry text") }; };
    if has_from > 1 { return unsafe { fail(ui, "invalid carry flag") }; }
    if has_from == 0 { e.carry(None, kind, value); return 0; }
    let (e, b) = match unsafe { boxed(ui, from) } { Ok(v) => v, Err(code) => return code };
    e.carry(Some(&b), kind, value);
    0
});

export!(pub unsafe fn pxui_ui_transfer_carry(ui: *mut PxuiUi, to: *const PxuiBox, out: *mut u8) -> i32 {
    let (e, b) = match unsafe { boxed(ui, to) } { Ok(v) => v, Err(code) => return code };
    if out.is_null() { return unsafe { fail(ui, "null carry transfer output") }; }
    unsafe { *out = e.transfer_carry(&b) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_cancel_carry(ui: *mut PxuiUi) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    e.cancel_carry();
    0
});

// `tag`: 0 nothing, 1 the pointer hovers the box while a payload is held, 2 the payload was dropped on it this frame.
export!(pub unsafe fn pxui_ui_drop_target(ui: *mut PxuiUi, box_: *const PxuiBox, tag: *mut u32, kind: *mut Str, value: *mut Str) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if tag.is_null() || kind.is_null() || value.is_null() { return unsafe { fail(ui, "null drop output") }; }
    let (code, payload) = match e.drop_target_ref(&b) { None => (0, None), Some((false, p)) => (1, Some(p)), Some((true, p)) => (2, Some(p)) };
    let lent = match payload { Some(p) => match unsafe { lend_payload(ui, &p.kind, &p.value) } { Ok(pair) => pair, Err(code) => return code }, None => (Str::default(), Str::default()) };
    unsafe {
        *tag = code;
        (*kind, *value) = lent;
    }
    0
});

/// A payload's kind and value lent to the caller until the next frame (`PxuiUi::lend`, refused past its caps).
unsafe fn lend_payload(ui: *mut PxuiUi, kind: &str, value: &str) -> Result<(Str, Str), i32> {
    let ui = unsafe { &*ui };
    Ok((ui.lend(kind)?, ui.lend(value)?))
}

export!(pub unsafe fn pxui_ui_context_at(ui: *mut PxuiUi, box_: *const PxuiBox, present: *mut u8, out: *mut pxui_types::geom::Point) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if present.is_null() || out.is_null() { return unsafe { fail(ui, "null context output") }; }
    let at = e.context_at(&b);
    unsafe { *present = at.is_some() as u8; *out = at.map_or(Default::default(), |(x, y)| pxui_types::geom::Point { x, y }); }
    0
});

// `within`: 0 the box itself is the topmost hit, 1 the box or a descendant.
export!(pub unsafe fn pxui_ui_hovered(ui: *mut PxuiUi, box_: *const PxuiBox, within: u8, out: *mut u8) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if out.is_null() || within > 1 { return unsafe { fail(ui, "invalid hover query") }; }
    unsafe { *out = if within == 0 { e.hovered_exactly(&b) } else { e.hovered_within(&b) } as u8; }
    0
});

// The keys held at the box's captured press, as key codes borrowed from the `Ui` until the next frame.
export!(pub unsafe fn pxui_ui_press_keys(ui: *mut PxuiUi, box_: *const PxuiBox, ptr: *mut *const u32, len: *mut usize) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if ptr.is_null() || len.is_null() { return unsafe { fail(ui, "null keys output") }; }
    let codes = &mut unsafe { &*ui }.shared().key_codes;
    codes.clear();
    codes.extend(e.press_keys(&b).into_iter().map(key_code));
    unsafe { *ptr = codes.as_ptr(); *len = codes.len(); }
    0
});

export!(pub unsafe fn pxui_ui_root_at(ui: *mut PxuiUi, x: f32, y: f32, roots: Slice<i64>, present: *mut u8, out: *mut i64) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if present.is_null() || out.is_null() || !finite(&[x, y]) || roots.len > 1 << 16 || (roots.len != 0 && roots.ptr.is_null()) { return unsafe { fail(ui, "invalid root query") }; }
    let roots = if roots.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(roots.ptr, roots.len) } };
    let hit = e.root_at((x, y), roots);
    unsafe { *present = hit.is_some() as u8; *out = hit.unwrap_or_default(); }
    0
});

// ------------------------------------------------------------------ views

export!(pub unsafe fn pxui_ui_view_opacity(ui:*mut PxuiUi,box_:*const PxuiBox,opacity:f32)->i32 {
    let (e,b)=match unsafe { boxed(ui,box_) } { Ok(v)=>v,Err(code)=>return code };
    if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) { return unsafe { fail(ui,"view opacity out of range") }; }
    e.view_opacity(&b,opacity);
    0
});

export!(pub unsafe fn pxui_ui_view_scale(ui:*mut PxuiUi,box_:*const PxuiBox,scale:f32)->i32 {
    let (e,b)=match unsafe { boxed(ui,box_) } { Ok(v)=>v,Err(code)=>return code };
    if !scale.is_finite() { return unsafe { fail(ui,"view scale is not finite") }; }
    e.view_scale(&b,scale);
    0
});

export!(pub unsafe fn pxui_ui_view_clip(ui:*mut PxuiUi,box_:*const PxuiBox,clip:*const pxui_types::geom::Rect)->i32 {
    let (e,b)=match unsafe { boxed(ui,box_) } { Ok(v)=>v,Err(code)=>return code };
    let clip=unsafe { clip.as_ref() }.copied();
    if let Some(r)=clip {
        if r.w<0.0 || r.h<0.0 || !finite(&[r.x,r.y,r.w,r.h,r.x+r.w,r.y+r.h]) { return unsafe { fail(ui,"invalid view clip") }; }
    }
    e.view_clip(&b,clip);
    0
});

export!(pub unsafe fn pxui_ui_view_offset(ui: *mut PxuiUi, box_: *const PxuiBox, dx: f32, dy: f32) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if !finite(&[dx, dy]) { return unsafe { fail(ui, "view offset is not finite") }; }
    e.view_offset(&b, dx, dy);
    0
});

export!(pub unsafe fn pxui_ui_view_transform_about(ui: *mut PxuiUi, box_: *const PxuiBox, scale: f32, transform: *const f32, px: f32, py: f32) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if transform.is_null() || !finite(&[scale, px, py]) { return unsafe { fail(ui, "invalid view transform") }; }
    let transform = unsafe { [*transform, *transform.add(1), *transform.add(2), *transform.add(3)] };
    if !finite(&transform) { return unsafe { fail(ui, "view transform is not finite") }; }
    e.view_transform_about(&b, scale, transform, (px, py));
    0
});

export!(pub unsafe fn pxui_ui_view_backdrop_blur(ui: *mut PxuiUi, box_: *const PxuiBox, sigma: f32) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if !sigma.is_finite() || sigma < 0.0 { return unsafe { fail(ui, "blur sigma out of range") }; }
    e.view_backdrop_blur(&b, sigma);
    0
});

export!(pub unsafe fn pxui_ui_view_backdrop_saturation(ui: *mut PxuiUi, box_: *const PxuiBox, saturation: f32) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if !saturation.is_finite() || saturation < 0.0 { return unsafe { fail(ui, "backdrop saturation out of range") }; }
    e.view_backdrop_saturation(&b, saturation);
    0
});

#[cfg(test)]
mod view_tests {
    #[test]
    fn public_view_controls_refuse_null_and_invalid_handles_without_poisoning() {
        use crate::*;
        let ui=unsafe { crate::ui::create_test_ui() };
        assert!(!ui.is_null());
        let invalid=PxuiBox { box_slot:u64::MAX,..Default::default() };
        for (handle,expected) in [(core::ptr::null(),PXUI_ERR_ARGUMENT),(&invalid as *const PxuiBox,PXUI_ERR_STALE)] {
            assert_eq!(unsafe { super::pxui_ui_view_opacity(ui,handle,0.5) },expected);
            assert_eq!(unsafe { super::pxui_ui_view_scale(ui,handle,1.0) },expected);
            assert_eq!(unsafe { super::pxui_ui_view_clip(ui,handle,core::ptr::null()) },expected);
        }
        let mut out=FrameOut::default();
        assert_eq!(unsafe { pxui_frame(ui,&FrameIn::default(),None,core::ptr::null_mut(),&mut out) },0);
        assert_eq!(unsafe { pxui_ui_destroy(ui) },0);
    }
}
