use crate::state::{PxuiUi, Shared, checked, core_box, fail, get};
use pxui_core::ui::{Ui, UiBox};
use core::mem::{align_of, offset_of, size_of};
use std::rc::Rc;
use pxui_types::{a11y::Role, layout::{Align, Anchor, Grid, Justify, MAX_TRACKS, Side, Track}, node::{OptAlign, OptAnchor, OptBar, OptCurve, OptGrid}, theme::{Font, Look, Ring, Shadow, State}, Axis, Bar, Dir, Mark, Node, Size, geom::Rect, ffi::{PxuiFontDesc, PxuiLookDesc, PxuiShadowDesc, PxuiStateDesc}, PxuiBox};

pub(crate) fn size(v:pxui_types::ffi::PxuiSizeDesc)->Option<Size>{if !v.value.is_finite(){return None;} Some(match v.kind {0=>Size::Px(v.value),1=>Size::Pct(v.value),2=>Size::Rel(v.relative?),3=>Size::Grow,4=>Size::Fit,_=>return None})}
fn font(v:PxuiFontDesc)->Font { Font { family:v.family, weight:v.weight, size:v.size, tracking:v.tracking, caps:v.caps!=0 } }
fn shadow(v:PxuiShadowDesc)->Shadow { Shadow { color:v.color, offset:v.offset, blur:v.blur, spread:v.spread, inner:v.inner!=0 } }
pub(crate) fn look(v:PxuiLookDesc)->Look { Look {
    fill:v.fill, gradient:(v.has_gradient!=0).then_some(v.gradient).into(), angle:v.angle,
    border:v.border, border_w:v.border_w, sides:v.sides as u8, radius:v.radius,
    shadows:[shadow(v.shadows[0]),shadow(v.shadows[1])], text:v.text, font:font(v.font), opacity:v.opacity,
    ring:Ring { color:v.ring.color, width:v.ring.width, sides:v.ring.sides as u8 },
    selection:v.selection, caret:v.caret, dashed:v.dashed!=0,
} }
pub(crate) fn state(v:PxuiStateDesc)->State { State { hovered:v.hovered!=0, pressed:v.pressed!=0, focused:v.focused!=0, disabled:v.disabled!=0, checked:v.checked!=0 } }
// ---- a node crosses as the engine's own bytes: checked where they lie, then copied once into the engine

/// The byte at `at` of the record behind `p`.
unsafe fn byte(p: *const u8, at: usize) -> u8 { unsafe { p.add(at).read() } }
/// Where the value of a C-layout enum (tag then a union of its variants) starts: the tag rounded up to the enum's alignment.
const fn payload(tag: usize, align: usize) -> usize { tag.div_ceil(align) * align }
/// Whether the `Size` at `at` has a known tag, a finite length and, for `Rel`, a function.
#[inline(always)]
unsafe fn size_ok(p: *const u8, at: usize) -> bool {
    let value = at + payload(4, align_of::<Size>());
    match unsafe { p.add(at).cast::<u32>().read_unaligned() } {
        0 | 1 => (unsafe { p.add(value).cast::<f32>().read_unaligned() }).is_finite(),
        2 => (unsafe { p.add(value).cast::<usize>().read_unaligned() }) != 0,
        3 | 4 => true,
        _ => false,
    }
}
/// Whether the `Mark` at `at` is a known one.
#[inline(always)]
pub(crate) unsafe fn mark_ok(p: *const u8, at: usize) -> bool {
    let value = at + payload(1, align_of::<Mark>());
    match unsafe { byte(p, at) } { 0 => true, 1 => (unsafe { byte(p, value) }) <= Dir::Right as u8, 2 => (unsafe { byte(p, value + size_of::<f32>()) }) <= 1, _ => false }
}
/// The always-present tag and flag bytes of a node with the largest value each may hold. The node's own C layout: an enum is a
/// byte, an optional field a byte tag (0 none, 1 some) before its value. The first ten are the byte fields `Node` keeps in its
/// first 16 bytes (after `size`, before `z`), tested as one vector; then the tags of the `OPTIONAL` fields whose values are checked
/// when set, the other flags, and `mark` last.
const NODE_BYTES: [(usize, u8); 22] = [
    (offset_of!(Node, axis), Axis::Stack as u8), (offset_of!(Node, justify), Justify::SpaceEvenly as u8), (offset_of!(Node, align), Align::Baseline as u8),
    (offset_of!(Node, wrap), 1), (offset_of!(Node, role), Role::Separator as u8), (offset_of!(Node, state.hovered), 1), (offset_of!(Node, state.pressed), 1),
    (offset_of!(Node, state.focused), 1), (offset_of!(Node, state.disabled), 1), (offset_of!(Node, state.checked), 1),
    (offset_of!(Node, align_self), 1), (offset_of!(Node, anchor), 1), (offset_of!(Node, bar), 1), (offset_of!(Node, transition), 1),
    (offset_of!(Node, grid), 1), (offset_of!(Node, at), 1), (offset_of!(Node, look.gradient), 1),
    (offset_of!(Node, look.shadows) + offset_of!(Shadow, inner), 1), (offset_of!(Node, look.shadows) + size_of::<Shadow>() + offset_of!(Shadow, inner), 1),
    (offset_of!(Node, look.font.caps), 1), (offset_of!(Node, look.dashed), 1), (offset_of!(Node, mark), 2),
];
/// How many of `NODE_BYTES` lie in the first 16 bytes, which hold nothing else but `size` and `z` (every byte is written).
const HEAD: usize = 10;
/// How many tags after the head are of optional fields whose values `node_ok` checks when set (`mark`'s is the last byte).
const OPTIONAL: usize = 4;
const MARK: usize = NODE_BYTES.len() - 1;
/// Past the head every byte is a flag but `mark`: `node_ok` ors the flags and tests `mark` by itself.
const _: () = { let mut i = HEAD; while i < MARK { assert!(NODE_BYTES[i].1 == 1); i += 1; } };
const _: () = assert!(NODE_BYTES[MARK].0 == offset_of!(Node, mark) && NODE_BYTES[HEAD].0 == offset_of!(Node, align_self) && NODE_BYTES[HEAD + OPTIONAL - 1].0 == offset_of!(Node, transition));
const _: () = assert!(offset_of!(Node, z) == 14 && offset_of!(Node, flags) == 16 && NODE_BYTES[HEAD - 1].0 < 14 && NODE_BYTES[HEAD].0 >= 16);
/// Whether a byte of the first 16 at `p` is over its largest value (`HEAD_MAX`): one vector compare.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn head_over(p: *const u8) -> bool {
    use core::arch::aarch64::{vcgtq_u8, vld1q_u8, vmaxvq_u8};
    unsafe { vmaxvq_u8(vcgtq_u8(vld1q_u8(p), vld1q_u8(HEAD_MAX.as_ptr()))) != 0 }
}
/// `head_over` byte by byte, where there is no vector compare to name.
#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
unsafe fn head_over(p: *const u8) -> bool {
    let head = unsafe { p.cast::<[u8; 16]>().read_unaligned() };
    head.iter().zip(HEAD_MAX.iter()).fold(false, |over, (byte, max)| over | (byte > max))
}
/// The first 16 bytes' largest values: a checked byte's own, 0xff where any value goes (`size`, `z`).
static HEAD_MAX: [u8; 16] = {
    let (mut max, mut i) = ([0xff; 16], 0);
    while i < HEAD { max[NODE_BYTES[i].0] = NODE_BYTES[i].1; i += 1; }
    max
};

/// Whether the record behind `p` is a `Node` as C may write one: long enough, every enum tag known, every flag 0 or 1, sizes
/// finite. Floats are otherwise taken as they are (the engine tolerates any), and a grid's tracks are checked where they are copied.
/// No branch per byte: the first 16 bytes are compared with their largest values as one vector, the scattered flags are or-ed and
/// compared once; a value behind a tag is read only when the tag says it is there.
#[inline(always)]
unsafe fn node_ok(p: *const Node) -> bool {
    let p = p.cast::<u8>();
    // the caller's size before anything past it is read: a shorter record is refused, never read beyond
    // (volatile: else the compiler rebuilds the head's first four bytes from this load instead of loading the 16 at once)
    if (unsafe { core::ptr::read_volatile(p.cast::<u32>()) } as usize) < size_of::<Node>() { return false; }
    let over = unsafe { head_over(p) };
    let tag = |at: usize| unsafe { byte(p, at) } as u32;
    let (mut optional, mut flags) = (0, 0);
    for &(at, _) in &NODE_BYTES[HEAD..HEAD + OPTIONAL] { optional |= tag(at); }
    for &(at, _) in &NODE_BYTES[HEAD + OPTIONAL..MARK] { flags |= tag(at); }
    let mark = tag(offset_of!(Node, mark));
    if over || optional | flags > 1 || mark > 2 || !unsafe { size_ok(p, offset_of!(Node, w)) && size_ok(p, offset_of!(Node, h)) } { return false; }
    // the common node sets none of the optional values whose contents are checked: one branch
    if optional | mark == 0 { return true; }
    let value = |at: usize, align: usize| at + payload(1, align);
    let set = |at: usize| tag(at) == 1;
    let anchor = value(offset_of!(Node, anchor), align_of::<OptAnchor>());
    !(set(offset_of!(Node, align_self)) && unsafe { byte(p, value(offset_of!(Node, align_self), align_of::<OptAlign>())) } > Align::Baseline as u8)
        && !(set(offset_of!(Node, anchor)) && unsafe { byte(p, anchor + offset_of!(Anchor, placement.side)) > Side::Left as u8
            || byte(p, anchor + offset_of!(Anchor, placement.align)) > Align::Baseline as u8 || byte(p, anchor + offset_of!(Anchor, placement.flip)) > 1 })
        && unsafe { mark_ok(p, offset_of!(Node, mark)) }
        && !(set(offset_of!(Node, bar)) && unsafe { byte(p, value(offset_of!(Node, bar), align_of::<OptBar>()) + offset_of!(Bar, whole)) } > 1)
        && !(set(offset_of!(Node, transition)) && unsafe { byte(p, value(offset_of!(Node, transition), align_of::<OptCurve>())) } > 1)
}

/// The checked tracks a C grid points at, into `out`: None when a list is longer than `MAX_TRACKS`, null with a length, or holds
/// an unknown kind or a length that is not finite.
unsafe fn tracks(ptr: *const Track, len: usize, out: &mut [Track]) -> Option<usize> {
    if len > MAX_TRACKS || (len != 0 && ptr.is_null()) { return None; }
    for (i, slot) in out.iter_mut().enumerate().take(len) {
        let at = unsafe { ptr.add(i) }.cast::<u8>();
        let value = unsafe { at.add(payload(1, align_of::<Track>())).cast::<f32>().read_unaligned() };
        *slot = match unsafe { byte(at, 0) } { 0 if value.is_finite() => Track::Px(value), 1 if value.is_finite() => Track::Fr(value), 2 => Track::Fit, _ => return None };
    }
    Some(len)
}

/// Declares `desc` as it is. A Rust node's grid borrows tracks that live as long as the program (`Grid::new` takes `'static`
/// slices), so the engine keeps its pointers; a C node's grid reaches here only as the library's own copy (`grid_node`). The handle
/// goes to `out` as its bytes (two vector moves) where the engine's handle has the same layout, else field by field.
#[inline(always)]
pub(crate) fn declare(engine: &mut Ui, label: Option<&str>, key: i64, desc: &Node, out: &mut PxuiBox) {
    let b = match label { Some(label) => engine.node_ref(label, desc), None => engine.node_keyed_ref(key, desc) };
    if SAME_BOX { unsafe { core::ptr::copy_nonoverlapping((&b as *const UiBox).cast::<u8>(), (out as *mut PxuiBox).cast::<u8>(), size_of::<PxuiBox>()) } }
    else { (out.index, out.box_key, out.box_slot, out.generation) = (b.index as u64, b.box_key, b.box_slot as u64, b.generation); }
}
/// Whether the engine's handle is `PxuiBox` byte for byte (every 64-bit target): its padding is copied too, as padding.
const SAME_BOX: bool = size_of::<UiBox>() == size_of::<PxuiBox>() && offset_of!(UiBox, index) == offset_of!(PxuiBox, index)
    && offset_of!(UiBox, box_key) == offset_of!(PxuiBox, box_key) && offset_of!(UiBox, box_slot) == offset_of!(PxuiBox, box_slot)
    && offset_of!(UiBox, generation) == offset_of!(PxuiBox, generation) && size_of::<usize>() == size_of::<u64>();

/// A C node with a grid: its tracks, checked, become the library's own for as long as its box lives (a list equal to one stored is
/// shared, so a settled frame allocates nothing), and the node pointing at that copy goes through the trusted entry.
#[cold]
#[inline(never)]
unsafe fn grid_node(ui: *mut PxuiUi, label: Option<&str>, key: i64, desc: &Node, out: *mut PxuiBox) -> i32 {
    let OptGrid::Some(grid) = desc.grid else { return unsafe { fail(ui, "invalid grid tracks") } };
    let (cols, cols_len, rows, rows_len) = grid.raw();
    let mut buffer = [Track::Fit; 2 * MAX_TRACKS];
    let Some(split) = (unsafe { tracks(cols, cols_len, &mut buffer) }) else { return unsafe { fail(ui, "invalid grid tracks") } };
    let Some(len) = (unsafe { tracks(rows, rows_len, &mut buffer[split..]) }).map(|rows| split + rows) else { return unsafe { fail(ui, "invalid grid tracks") } };
    let flat = &buffer[..len];
    let stored = match unsafe { (*ui).shared() }.grid_tracks.values().find(|stored| stored[..] == *flat) { Some(stored) => Rc::clone(stored), None => Rc::from(flat) };
    // the engine's Grid borrows 'static tracks: these live in `shared` until the box is gone (see `finish_frame`)
    let all: &'static [Track] = unsafe { core::mem::transmute::<&[Track], &'static [Track]>(&stored[..]) };
    let node = Node { grid: OptGrid::Some(Grid::new(&all[..split], &all[split..])), ..*desc };
    let rc = match label { Some(label) => unsafe { crate::trusted::node(ui, label, &node, out) }, None => unsafe { crate::trusted::pxui_ui_node_keyed(ui, key, &node, out) } };
    let shared = unsafe { (*ui).shared() };
    let key = unsafe { (*out).box_key };
    if rc == 0 && !shared.grid_tracks.get(&key).is_some_and(|kept| Rc::ptr_eq(kept, &stored)) {
        if let Some(old) = shared.grid_tracks.insert(key, stored) { shared.grid_retired.push(old); }
    }
    rc
}

/// The node behind `desc` when it is one (see `node_ok`), borrowed for the call.
#[inline(always)]
unsafe fn checked_node<'a>(desc: *const Node) -> Option<&'a Node> {
    if desc.is_null() || !unsafe { node_ok(desc) } { None } else { Some(unsafe { &*desc }) }
}

/// The refusals of a node that fails `node_ok`, out of line with their messages: the check's branches then share one target and
/// nothing is set up for it on the way through.
#[cold]
#[inline(never)]
unsafe fn invalid_node(ui: *mut PxuiUi) -> i32 { unsafe { fail(ui, "invalid node") } }
#[cold]
#[inline(never)]
unsafe fn invalid_keyed(ui: *mut PxuiUi) -> i32 { unsafe { fail(ui, "invalid keyed node") } }

export!(trusted pub unsafe fn pxui_ui_node(ui:*mut PxuiUi,label:pxui_types::Str,desc:*const Node,out:*mut PxuiBox)->i32 {
    if unsafe{get(ui)}.is_none() {return crate::PXUI_ERR_ARGUMENT};
    if out.is_null(){return unsafe{fail(ui,"null node argument")};}
    let Some(label)=(unsafe{checked(label)}) else{return unsafe{fail(ui,"invalid node label")};};
    let Some(desc)=(unsafe{checked_node(desc)}) else{return unsafe{invalid_node(ui)};};
    if desc.grid.is_some() { return unsafe{grid_node(ui,Some(label),0,desc,out)}; }
    unsafe{crate::trusted::node(ui,label,desc,out)}
});

export!(trusted pub unsafe fn pxui_ui_node_keyed(ui:*mut PxuiUi,key:i64,desc:*const Node,out:*mut PxuiBox)->i32 {
    if unsafe{get(ui)}.is_none() {return crate::PXUI_ERR_ARGUMENT};
    if out.is_null(){return unsafe{fail(ui,"null keyed node argument")};}
    let Some(desc)=(unsafe{checked_node(desc)}) else{return unsafe{invalid_keyed(ui)};};
    if desc.grid.is_some() { return unsafe{grid_node(ui,None,key,desc,out)}; }
    unsafe{crate::trusted::pxui_ui_node_keyed(ui,key,desc,out)}
});

/// Ends a frame: frees replaced grid tracks and those of boxes that are gone. The output is built when it is read.
pub(crate) fn finish_frame(shared: &mut Shared, engine: &Ui) {
    shared.grid_retired.clear();
    if !shared.grid_tracks.is_empty() { shared.grid_tracks.retain(|key, _| engine.box_of_key(*key).is_some()); }
    shared.frame_ended = true;
}

export!(trusted pub unsafe fn pxui_ui_rect(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut Rect)->i32 {
    if unsafe{get(ui)}.is_none() {return crate::PXUI_ERR_ARGUMENT};
    if box_.is_null()||out.is_null(){return unsafe{fail(ui,"null rect argument")};}
    if core_box(unsafe{*box_}).is_none(){return unsafe{fail(ui,"invalid rect box")};}
    unsafe{crate::trusted::rect(ui,&*box_,&mut *out)}
});

export!(trusted pub unsafe fn pxui_ui_box_of_key(ui:*mut PxuiUi,key:i64,out:*mut PxuiBox)->u8 {
    if unsafe{get(ui)}.is_none()||out.is_null() {return 0};
    let found=unsafe{crate::trusted::box_of_key(ui,key,&mut *out)};
    found as u8
});

export!(trusted pub unsafe fn pxui_ui_text(ui:*mut PxuiUi,box_:*const PxuiBox,value:pxui_types::Str)->i32 {
    if unsafe{get(ui)}.is_none() {return crate::PXUI_ERR_ARGUMENT};
    if box_.is_null(){return unsafe{fail(ui,"null text box")};}
    let Some(value)=(unsafe{checked(value)}) else{return unsafe{fail(ui,"invalid text")};};
    if core_box(unsafe{*box_}).is_none(){return unsafe{fail(ui,"invalid text box")};}
    unsafe{crate::trusted::text(ui,box_,value)}
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FrameIn, FrameOut, PXUI_ERR_ARGUMENT, PxuiUiDesc, pxui_frame, pxui_ui_create, pxui_ui_destroy};
    use core::ffi::c_void;
    use pxui_types::{Size, layout::Placement, theme::Curve};

    /// A node as raw bytes, as C hands one over.
    #[repr(C, align(8))]
    struct Raw([u8; size_of::<Node>()]);
    fn raw(node: Node) -> Raw {
        let mut out = Raw([0; size_of::<Node>()]);
        unsafe { core::ptr::copy_nonoverlapping((&node as *const Node).cast::<u8>(), out.0.as_mut_ptr(), size_of::<Node>()) };
        out
    }

    /// Every byte `node_ok` reads: a valid node (each optional field set) crosses, and one byte past its largest value at any of
    /// them is refused, as are a short record, an unknown size kind, a `Rel` size with no function and a length that is not finite.
    extern "C" fn build(_: *mut c_void, ui: *mut c_void) {
        let ui = ui.cast::<PxuiUi>();
        let place = |node: &Raw| { let mut out = PxuiBox::default(); unsafe { pxui_ui_node_keyed(ui, 7, (node.0.as_ptr()).cast(), &mut out) } };
        let full = Node {
            align_self: Some(Align::Center).into(), anchor: Some(Anchor { rect: Rect::default(), placement: Placement::BELOW }).into(),
            at: Some((1.0, 2.0)).into(), mark: Mark::Line { width: 1.0, rising: true }, bar: Some(Bar { color: Default::default(), radius: 0.0, width: 2.0, gap: 0.0, margin: 0.0, whole: true }).into(),
            transition: Some(Curve::Spring { omega: 9.0 }).into(), ..Node::DEFAULT
        };
        assert_eq!(place(&raw(full)), 0);
        assert_eq!(place(&raw(Node { mark: Mark::Chevron(Dir::Right), ..full })), 0);
        // the head's unchecked bytes take any value (`z` is all ones here)
        assert_eq!(place(&raw(Node { z: -1, ..full })), 0);
        let mut refused = Vec::new();
        // one past the largest value, and a byte with its top bit set (the first 16 bytes are compared as one vector, see `head_over`)
        for &(at, max) in NODE_BYTES.iter() { for byte in [max + 1, 0x80, 0xff] { let mut bad = raw(full); bad.0[at] = byte; refused.push(bad); } }
        let value = |at: usize, align: usize| at + payload(1, align);
        let anchor = value(offset_of!(Node, anchor), align_of::<OptAnchor>());
        for (at, byte) in [
            (value(offset_of!(Node, align_self), align_of::<OptAlign>()), 5), (anchor + offset_of!(Anchor, placement.side), 4),
            (anchor + offset_of!(Anchor, placement.align), 5), (anchor + offset_of!(Anchor, placement.flip), 2),
            (value(offset_of!(Node, mark), align_of::<Mark>()) + size_of::<f32>(), 2), (value(offset_of!(Node, bar), align_of::<OptBar>()) + offset_of!(Bar, whole), 2),
            (value(offset_of!(Node, transition), align_of::<OptCurve>()), 2), (offset_of!(Node, w), 5), (offset_of!(Node, size), 8),
        ] { let mut bad = raw(full); bad.0[at] = byte; refused.push(bad); }
        let mut chevron = raw(Node { mark: Mark::Chevron(Dir::Right), ..full });
        chevron.0[value(offset_of!(Node, mark), align_of::<Mark>())] = 4;
        refused.push(chevron);
        refused.push(raw(Node { w: Size::Px(f32::NAN), ..full }));
        let mut no_function = raw(full);
        no_function.0[offset_of!(Node, h)..offset_of!(Node, h) + 4].copy_from_slice(&2u32.to_ne_bytes());
        no_function.0[offset_of!(Node, h) + 8..offset_of!(Node, h) + 16].fill(0);
        refused.push(no_function);
        for (i, bad) in refused.iter().enumerate() { assert_eq!(place(bad), PXUI_ERR_ARGUMENT, "refused case {i}"); }
    }

    #[test]
    fn a_node_crosses_as_its_bytes_and_any_unknown_tag_or_flag_is_refused() {
        let ui = unsafe { pxui_ui_create(&PxuiUiDesc::default()) };
        let mut out = FrameOut::default();
        assert_eq!(unsafe { pxui_frame(ui, &FrameIn { width: 100.0, height: 100.0, ..FrameIn::default() }, Some(build), core::ptr::null_mut(), &mut out) }, 0);
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    }
}
