//! Retained opacity is per instance. Clip coordinates are window points and NULL clears it. Scale
//! accepts finite zero/negative values and resets previous rotation/fixed-pivot transforms. Equal
//! controls preserve transform storage and retained instances.
//!
//! Boxes and what hangs off them: `BoxOpts` boxes, patches, scroll and text state, carries and drops, view transforms.
use crate::paint::CallbackOwner;
use crate::state::{PxuiUi, boxed, checked, engine, fail, finite, get, to_abi_box};
use crate::node::size;
use crate::ui::key_code;
use crate::style::decode_look;
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str};
use core::ffi::c_void;
use pxui_core::ui::{Axis, BoxOpts, Mark, Rect};
use pxui_types::{Dir, ffi::{PxuiBoxDesc, PxuiLookDesc, PxuiMarkDesc, Slice}};
use std::rc::Rc;

#[cfg(test)]
mod reveal_tests {
    use super::*;

    #[test]
    fn reveal_checks_handles_and_callback_ownership() {
        struct State { owner: *mut PxuiUi, list: PxuiBox }
        extern "C" fn build(user: *mut c_void, raw: *mut c_void) {
            let state = unsafe { &mut *user.cast::<State>() };
            let desc = pxui_types::ffi::PxuiGridListDesc {
                size: core::mem::size_of::<pxui_types::ffi::PxuiGridListDesc>() as u32,
                reserved: 0, label: Str { ptr: b"rows".as_ptr(), len: 4 }, count: 100,
                cell_w: 100.0, cell_h: 20.0, build: None, user: core::ptr::null_mut(),
            };
            assert_eq!(unsafe { crate::pxui_ui_grid_list(raw.cast(), &desc, &mut state.list) }, 0);
            assert_eq!(unsafe { pxui_ui_reveal(state.owner, &state.list, 99) }, crate::PXUI_ERR_STATE);
            assert_eq!(unsafe { pxui_ui_reveal(raw.cast(), &state.list, 0) }, 0);
        }
        let owner = unsafe { crate::pxui_ui_create(&crate::PxuiUiDesc::default()) };
        let mut state = State { owner, list: PxuiBox::default() };
        let mut out = crate::FrameOut::default();
        assert_eq!(unsafe { crate::pxui_frame(owner, &crate::FrameIn { width: 100.0, height: 100.0, ..Default::default() }, Some(build), (&mut state as *mut State).cast(), &mut out) }, 0);
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
    use pxui_types::ffi::{PxuiGestureIn, PxuiGestureOut};

    #[test]
    fn gesture_validates_every_argument_before_changing_the_node() {
        extern "C" fn build(user: *mut c_void, raw: *mut c_void) {
            let node = unsafe { &mut *user.cast::<PxuiBox>() };
            let desc = pxui_types::ffi::PxuiGridListDesc {
                size: core::mem::size_of::<pxui_types::ffi::PxuiGridListDesc>() as u32, reserved: 0,
                label: Str { ptr: b"rows".as_ptr(), len: 4 }, count: 10, cell_w: 100.0, cell_h: 20.0, build: None, user: core::ptr::null_mut(),
            };
            assert_eq!(unsafe { crate::pxui_ui_grid_list(raw.cast(), &desc, node) }, 0);
            let sentinel = || PxuiGestureOut { flags: 99, swipe: 1.0, back: 2.0, pull: 3.0, ..Default::default() };
            let mut out = sentinel();
            let good = PxuiGestureIn { kinds: 2, reveal: 80.0, ..Default::default() };
            let bad = [PxuiGestureIn { kinds: 16, ..good }, PxuiGestureIn { reveal: f32::NAN, ..good }, PxuiGestureIn { threshold: f32::INFINITY, ..good }, PxuiGestureIn { size: 8, ..good }];
            for request in &bad {
                assert_eq!(unsafe { pxui_ui_gesture(raw.cast(), node, request, 0, &mut out) }, PXUI_ERR_ARGUMENT);
            }
            assert_eq!(unsafe { pxui_ui_gesture(raw.cast(), node, &good, 4, &mut out) }, PXUI_ERR_ARGUMENT);
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
        let ui = unsafe { crate::pxui_ui_create(&crate::PxuiUiDesc::default()) };
        let mut node = PxuiBox::default();
        let mut out = crate::FrameOut::default();
        assert_eq!(unsafe { crate::pxui_frame(ui, &crate::FrameIn { width: 100.0, height: 100.0, ..Default::default() }, Some(build), (&mut node as *mut PxuiBox).cast(), &mut out) }, 0);
        assert_eq!(unsafe { pxui_ui_gesture(core::ptr::null_mut(), &node, core::ptr::null(), 0, &mut PxuiGestureOut::default()) }, PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { crate::pxui_ui_destroy(ui) }, 0);
    }
}

/// Keeps `value` alive until the next frame of `ui` and returns it as a `Str`.
fn ui_give(ui: *mut PxuiUi, value: String) -> Str { unsafe { &*ui }.give(value) }

pub type PxuiHit = Option<extern "C" fn(*mut c_void, Rect) -> Rect>;

// Declares a box under the current parent. The descriptor's `hit` callback is owned by the box's hit shape from here on:
// it is released exactly once, also when the call is refused.
/// What a box's hit callback calls: the caller's function and its user pointer, and the owner that releases the user pointer when the
/// slot lets go (or the closure is reused).
pub(crate) struct HitSlot { call: core::cell::Cell<PxuiHit>, user: core::cell::Cell<*mut c_void>, owner: core::cell::RefCell<CallbackOwner> }

/// The engine's closure for a hit callback. The engine keeps it as long as the box lives, and a box is rebuilt every frame, so the
/// closures are kept in a list: one the engine has let go of (only the list holds it) takes the new callback and its owner, which
/// releases the old user pointer, and the allocation is reused. A settled frame allocates none.
// ponytail: the list holds one entry per hit callback ever live at once; trim it if a frame ever builds thousands and then none.
fn hit_closure(shared: &mut crate::state::Shared, call: extern "C" fn(*mut c_void, Rect) -> Rect, user: *mut c_void, owner: CallbackOwner) -> Rc<dyn Fn(Rect) -> Rect> {
    if let Some((closure, slot)) = shared.hits.iter().find(|(closure, _)| Rc::strong_count(closure) == 1) {
        slot.call.set(Some(call));
        slot.user.set(user);
        *slot.owner.borrow_mut() = owner;
        return Rc::clone(closure);
    }
    let slot = Rc::new(HitSlot { call: core::cell::Cell::new(Some(call)), user: core::cell::Cell::new(user), owner: core::cell::RefCell::new(owner) });
    let read = Rc::clone(&slot);
    let closure: Rc<dyn Fn(Rect) -> Rect> = Rc::new(move |rect| match read.call.get() { Some(call) => call(read.user.get(), rect), None => rect });
    shared.hits.push((Rc::clone(&closure), slot));
    closure
}

export!(pub unsafe fn pxui_ui_box(ui: *mut PxuiUi, label: Str, desc: *const PxuiBoxDesc, out: *mut PxuiBox) -> i32 {
    if desc.is_null() { return PXUI_ERR_ARGUMENT; }
    // the box owns the hit callback from here on, so a refused descriptor still releases it: read what it holds first
    let Some(desc) = read_desc!(desc, PxuiBoxDesc) else { return PXUI_ERR_ARGUMENT; };
    let owner = CallbackOwner { user: desc.hit_user, release: desc.hit_release };
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() || desc.size < core::mem::size_of::<PxuiBoxDesc>() as u32 || desc.has_at > 1 || desc.has_hit_rect > 1
        || desc.max_h.is_nan() || !finite(&[desc.padding, desc.scroll_step, desc.at[0], desc.at[1], desc.hit_rect.x, desc.hit_rect.y, desc.hit_rect.w, desc.hit_rect.h]) {
        return unsafe { fail(ui, "invalid box descriptor") };
    }
    let axis = match desc.axis { 0 => Axis::Row, 1 => Axis::Column, 2 => Axis::Stack, _ => return unsafe { fail(ui, "unknown box axis") } };
    let (Some(label), Some(w), Some(h)) = (unsafe { checked(label) }, size(desc.w), size(desc.h)) else { return unsafe { fail(ui, "invalid box arguments") }; };
    let hit = desc.hit.map(|hit| hit_closure(state.shared(), hit, desc.hit_user, owner));
    let opts = BoxOpts {
        flags: desc.flags, w, h, max_h: desc.max_h, axis, padding: desc.padding, at: (desc.has_at != 0).then_some((desc.at[0], desc.at[1])),
        scroll_step: desc.scroll_step, hit, hit_rect: (desc.has_hit_rect != 0).then_some(desc.hit_rect),
    };
    unsafe { *out = to_abi_box((&mut *state.engine).box_(label, opts)); }
    0
});

export!(pub unsafe fn pxui_ui_set_at(ui: *mut PxuiUi, box_: *const PxuiBox, x: f32, y: f32) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if !finite(&[x, y]) { return unsafe { fail(ui, "position is not finite") }; }
    e.set_at(&b, (x, y));
    0
});

export!(pub unsafe fn pxui_ui_patch_look(ui: *mut PxuiUi, box_: *const PxuiBox, value: *const PxuiLookDesc, out: *mut u8) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if value.is_null() || out.is_null() { return unsafe { fail(ui, "null patch argument") }; }
    let Some(value) = decode_look(unsafe { *value }) else { return unsafe { fail(ui, "invalid look") }; };
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

#[cfg(test)]
#[test]
fn text_layout_count_checks_outputs_before_writing() {
    let mut out=77;
    assert_eq!(unsafe {pxui_ui_text_layout_count(core::ptr::null_mut(),&mut out)},PXUI_ERR_ARGUMENT);
    assert_eq!(out,77);
    let ui=unsafe {crate::pxui_ui_create(&crate::PxuiUiDesc::default())};
    assert_eq!(unsafe {pxui_ui_text_layout_count(ui,core::ptr::null_mut())},PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe {pxui_ui_text_layout_count(ui,&mut out)},0);
    assert_eq!(out,0);
    assert_eq!(unsafe {crate::pxui_ui_destroy(ui)},0);
}

export!(pub unsafe fn pxui_ui_text_layout_count(ui: *mut PxuiUi, out: *mut u64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null text layout count output") }; }
    let Some(count) = (unsafe { &*state.engine }).text_layout_count() else { return crate::PXUI_ERR_STATE; };
    unsafe { *out = count; }
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
        let request = unsafe { in_.read_unaligned() };
        if (request.size as usize) < core::mem::size_of::<pxui_types::ffi::PxuiGestureIn>() || request.kinds & !15 != 0 || !request.reveal.is_finite() || !request.threshold.is_finite() {
            return unsafe { fail(ui, "invalid gesture request") };
        }
        Some(pxui_core::ui::gesture::GestureCfg { kinds: request.kinds, reveal: request.reveal, threshold: request.threshold })
    };
    if commands & !3 != 0 { return unsafe { fail(ui, "invalid gesture command") }; }
    let state = e.gesture(&b, cfg, commands);
    *out = pxui_types::ffi::PxuiGestureOut { flags: state.flags, swipe: state.swipe, back: state.back, pull: state.pull, ..Default::default() };
    0
});

export!(pub unsafe fn pxui_ui_text_state(ui: *mut PxuiUi, box_: *const PxuiBox, present: *mut u8, out: *mut Str) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if present.is_null() || out.is_null() { return unsafe { fail(ui, "null text state output") }; }
    let value = e.text_state(&b);
    unsafe {
        *present = value.is_some() as u8;
        *out = value.map_or(Str::default(), |value| ui_give(ui, value));
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
export!(pub unsafe fn pxui_ui_cached_label(ui: *mut PxuiUi, box_: *const PxuiBox, index: u64, text: Str, out: *mut Str) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    let Some(text) = (unsafe { checked(text) }) else { return unsafe { fail(ui, "invalid label") }; };
    if out.is_null() || index > 1 << 20 { return unsafe { fail(ui, "invalid cached label arguments") }; }
    let label = e.cached_label(&b, index as usize, text);
    unsafe { *out = Str { ptr: label.as_ptr(), len: label.len() }; }
    0
});

export!(pub unsafe fn pxui_ui_to_front(ui: *mut PxuiUi, order: i64, box_: *const PxuiBox) -> i32 {
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
    unsafe {
        *present = payload.is_some() as u8;
        (*kind, *value) = payload.map_or((Str::default(), Str::default()), |p| ((&*ui).lend(&p.kind), (&*ui).lend(&p.value)));
    }
    0
}); 

export!(pub unsafe fn pxui_ui_carry(ui: *mut PxuiUi, has_from: u8, from: *const PxuiBox, kind: Str, value: Str) -> i32 {
    let Some(e) = (unsafe { engine(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let (Some(kind), Some(value)) = (unsafe { checked(kind) }, unsafe { checked(value) }) else { return unsafe { fail(ui, "invalid carry text") }; };
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
    unsafe {
        *tag = code;
        (*kind, *value) = payload.map_or((Str::default(), Str::default()), |p| ((&*ui).lend(&p.kind), (&*ui).lend(&p.value)));
    }
    0
});

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
        let ui=unsafe { pxui_ui_create(&PxuiUiDesc::default()) };
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
