use crate::state::{boxed, MAX_TOTAL_TEXT_BYTES, PxuiUi, append_strings, checked, fail, get};
use crate::style::decode_look;
use crate::ui::translate_event;
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str};
use pxui_types::ffi::PxuiResponse;
use pxui_core::ui::{Axis, Keyframe, Keyframes, Repeat, Target, MAX_KEYS};
use pxui_types::ffi::{PxuiCurveDesc, PxuiGridListDesc, Slice};
pub use pxui_types::ffi::{PxuiFooterDesc, PxuiHintDesc, PxuiInspectorRowDesc, PxuiInspectorSectionDesc, PxuiInspectorTextDesc, PxuiInspectorValueDesc, PxuiKeyframeDesc, PxuiMenuBarDesc, PxuiMenuOut, PxuiSplitDesc};

const MAX_MENUS: usize = 256;

#[cfg(test)]
mod curve_tests {
    use super::*;

    #[test]
    fn repeat_validation_keeps_the_running_animation_and_output_unchanged() {
        extern "C" fn build(user: *mut core::ffi::c_void, ui: *mut core::ffi::c_void) {
            assert_eq!(unsafe { crate::pxui_ui_node_keyed(ui.cast(), 77, &pxui_types::Node::DEFAULT, user.cast()) }, 0);
        }
        let ui = unsafe { crate::ui::create_test_ui() };
        let mut target = PxuiBox::default();
        let mut frame = crate::FrameOut::default();
        assert_eq!(unsafe { crate::pxui_frame(ui, &crate::FrameIn::default(), Some(build), (&mut target as *mut PxuiBox).cast(), &mut frame) }, 0);
        let key = PxuiKeyframeDesc { size: core::mem::size_of::<PxuiKeyframeDesc>() as u32, time: 1.0, target: 1,
            values: [0.5, 0.0, 0.0, 0.0], curve: PxuiCurveDesc { kind: PxuiCurveDesc::STEPS, count: 1, duration: 1.0, ..Default::default() }, ..Default::default() };
        let span = Slice { ptr: &key, len: 1 };
        let mut out = 0;
        assert_eq!(unsafe { pxui_ui_animate_keys(ui, &target, span, 0, 1, 0.25, &mut out) }, 0);
        assert_eq!(out, 1);
        for (count, reverse, offset) in [(65_536, 0, 0.0), (1, 2, 0.0), (1, 0, -1.0), (1, 0, f32::NAN), (1, 0, f32::INFINITY)] {
            out = 77;
            assert_eq!(unsafe { pxui_ui_animate_keys(ui, &target, span, count, reverse, offset, &mut out) }, PXUI_ERR_ARGUMENT);
            assert_eq!(out, 77);
            assert_eq!(unsafe { boxed(ui, &target) }.unwrap().0.transitions(), 1);
        }
        for bad in [PxuiKeyframeDesc { time: 0.0, ..key }, PxuiKeyframeDesc { time: f32::INFINITY, ..key }, PxuiKeyframeDesc { size: 0, ..key }] {
            out = 77;
            assert_eq!(unsafe { pxui_ui_animate_keys(ui, &target, Slice { ptr: &bad, len: 1 }, 2, 0, 0.0, &mut out) }, PXUI_ERR_ARGUMENT);
            assert_eq!(out, 77);
        }
        let bad = [key, PxuiKeyframeDesc { time: 0.5, ..key }];
        assert_eq!(unsafe { pxui_ui_animate_keys(ui, &target, Slice { ptr: bad.as_ptr(), len: 2 }, 1, 0, 0.0, &mut out) }, PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { pxui_ui_animate_keys(ui, &target, Slice { ptr: core::ptr::null(), len: 1 }, 1, 0, 0.0, &mut out) }, PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { pxui_ui_animate_keys(ui, &target, span, 1, 0, 0.0, core::ptr::null_mut()) }, PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { crate::pxui_ui_destroy(ui) }, 0);
    }

    #[test]
    fn scale_keyframes_keep_axes_and_anchor_and_reject_nonfinite_values() {
        let ui = unsafe { crate::ui::create_test_ui() };
        let key = PxuiKeyframeDesc { size: core::mem::size_of::<PxuiKeyframeDesc>() as u32, target: 3,
            values: [2.0, 0.5, 1.0, 0.0], curve: PxuiCurveDesc { kind: PxuiCurveDesc::STEPS, duration: 0.14, count: 2, ..PxuiCurveDesc::default() }, ..PxuiKeyframeDesc::default() };
        assert_eq!(unsafe { decode_key(ui, &key, -1.0) }, Ok(Keyframe::scale(0.0, 2.0, 0.5).anchor(1.0, 0.0).curve(pxui_types::Curve::Steps { duration: 0.14, count: 2 })));
        for at in 0..4 { for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut bad = key; bad.values[at] = value;
            assert_eq!(unsafe { decode_key(ui, &bad, -1.0) }, Err(PXUI_ERR_ARGUMENT));
        } }
        assert_eq!(unsafe { decode_key(ui, &PxuiKeyframeDesc { target: 4, ..key }, -1.0) }, Err(PXUI_ERR_ARGUMENT));
        assert_eq!(unsafe { crate::pxui_ui_destroy(ui) }, 0);
    }

    #[test]
    fn curve_descriptors_validate_samples_counts_and_finite_parameters() {
        let table = PxuiCurveDesc { kind: PxuiCurveDesc::TABLE, duration: 0.24, count: 3,
            positions: [0.0, 0.25, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            values: [0.0, 1.043, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], ..PxuiCurveDesc::default() };
        assert!(curve(table).is_some());
        assert!(curve(PxuiCurveDesc { kind: PxuiCurveDesc::STEPS, count: 12, ..table }).is_some());
        assert!(curve(PxuiCurveDesc { kind: PxuiCurveDesc::BEZIER, ..table }).is_some());
        assert!(curve(PxuiCurveDesc { kind: PxuiCurveDesc::SPRING, omega: 20.0, ..table }).is_some());
        for count in [0, 1, 17, u32::MAX] { assert!(curve(PxuiCurveDesc { count, ..table }).is_none()); }
        for duration in [-1.0, f32::NAN, f32::INFINITY] { assert!(curve(PxuiCurveDesc { duration, ..table }).is_none()); }
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut bad = table;
            bad.positions[1] = value; assert!(curve(bad).is_none());
            bad = table; bad.values[1] = value; assert!(curve(bad).is_none());
        }
        for value in [0.0, -0.1, 1.0, 1.1] {
            let mut bad = table; bad.positions[1] = value; assert!(curve(bad).is_none());
        }
        for at in [0, 2] {
            let mut bad = table; bad.positions[at] = 0.5; assert!(curve(bad).is_none());
            bad = table; bad.values[at] = 0.5; assert!(curve(bad).is_none());
        }
        assert!(curve(PxuiCurveDesc { kind: PxuiCurveDesc::STEPS, count: 0, ..table }).is_none());
        assert!(curve(PxuiCurveDesc { kind: 5, ..table }).is_none());
        assert!(curve(PxuiCurveDesc { kind: PxuiCurveDesc::SPRING, omega: 0.0, ..table }).is_none());
        let mut full = table;
        full.count = 16;
        for i in 0..16 { full.positions[i] = i as f32 / 15.0; full.values[i] = i as f32 / 15.0; }
        assert!(curve(full).is_some());
    }
}

pub(crate) fn curve(value: PxuiCurveDesc) -> Option<pxui_types::Curve> {
    if (value.size as usize) < core::mem::size_of::<PxuiCurveDesc>() { return None; }
    if !value.duration.is_finite() || !value.omega.is_finite() || !value.points.iter().all(|v| v.is_finite()) { return None; }
    let decoded = match value.kind {
        PxuiCurveDesc::BEZIER => pxui_types::Curve::Bezier { duration: value.duration, points: value.points },
        PxuiCurveDesc::SPRING => pxui_types::Curve::Spring { omega: value.omega },
        PxuiCurveDesc::TABLE => pxui_types::Curve::Table { duration: value.duration, count: value.count, positions: value.positions, values: value.values },
        PxuiCurveDesc::STEPS => pxui_types::Curve::Steps { duration: value.duration, count: value.count },
        _ => return None,
    };
    decoded.valid().then_some(decoded)
}
export!(trusted pub unsafe fn pxui_ui_grid_list(ui: *mut PxuiUi, desc: *const PxuiGridListDesc, out: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if desc.is_null() || out.is_null() { return unsafe { fail(ui, "null grid list argument") }; }
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid grid list descriptor") }; };
    let Some(label) = (unsafe { crate::state::checked_key(desc.label) }) else { return unsafe { fail(ui, "invalid grid list label") }; };
    let (shared, build, user) = (unsafe { (*ui).shared }, desc.build, desc.user);
    unsafe { crate::trusted::grid_list(ui, label, desc.count, desc.cell_w, desc.cell_h, &mut |engine: &mut pxui_core::ui::Ui, index| {
        let mut borrowed = PxuiUi::borrowed(engine, shared);
        let _current = crate::state::Current::set(shared, &mut borrowed);
        if let Some(build) = build { build(user, (&mut borrowed as *mut PxuiUi).cast(), index) }
    }, &mut *out) }
});

export!(pub unsafe fn pxui_ui_wants_frame(ui: *mut PxuiUi, events: pxui_types::ffi::Strided<pxui_types::Event>, now: f64, out: *mut u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() || !now.is_finite() || events.len > 4096 || !events.is_valid() { return unsafe { fail(ui, "invalid wants frame arguments") }; }
    // as `frame_body`: a Ui with no registered surface refuses before any event is read
    if state.shared().surface == 0 { return unsafe { fail(ui, "unknown surface") }; }
    // the events decode into a buffer the `Ui` keeps (an idle host asks on every wake), checked as a frame checks them
    let mut translated = core::mem::take(&mut state.shared().wants);
    let mut budget = MAX_TOTAL_TEXT_BYTES;
    for i in 0..events.len {
        let event = unsafe { events.at(i) };
        let Some(event) = translate_event(event, state.shared().surface, &mut budget) else { translated.clear(); state.shared().wants = translated; return unsafe { fail(ui, "invalid wants frame event") }; };
        translated.push(event);
    }
    unsafe { *out = (&*state.engine).wants_frame(&translated, now) as u8; }
    translated.clear();
    state.shared().wants = translated;
    0
});

export!(trusted pub unsafe fn pxui_ui_footer(ui: *mut PxuiUi, desc: *const PxuiFooterDesc) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() { return unsafe { fail(ui, "null footer descriptor") }; }
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid footer descriptor") }; };
    if desc.has_right > 1 || desc.reserved != [0; 3] || desc.hints.len > 4096 || (desc.hints.len != 0 && desc.hints.ptr.is_null()) {
        return unsafe { fail(ui, "invalid footer descriptor") };
    }
    let right = if desc.has_right != 0 { let Some(s) = (unsafe { checked(desc.right) }) else { return unsafe { fail(ui, "invalid footer text") }; }; Some(s) } else { None };
    let raw = if desc.hints.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(desc.hints.ptr, desc.hints.len) } };
    if !raw.iter().all(|hint| hint.size as usize == core::mem::size_of_val(hint)) { return unsafe { fail(ui, "invalid footer hint") }; }
    // the rows borrow the caller's strings for this call, in a vector reused between frames: a settled footer allocates nothing
    let mut hints = core::mem::take(&mut state.shared().hints);
    for hint in raw {
        let (Some(key), Some(value)) = (unsafe { checked(hint.key) }, unsafe { checked(hint.value) }) else { hints.clear(); state.shared().hints = hints; return unsafe { fail(ui, "invalid footer hint") }; };
        hints.push((key, value));
    }
    let rc = unsafe { crate::trusted::footer(ui, right, &hints) };
    hints.clear();
    state.shared().hints = hints;
    rc
});

// One keyframe checked and decoded: its time, target (an offset, an opacity in 0..=1 or a look) and curve.
unsafe fn decode_key(ui: *mut PxuiUi, key: &PxuiKeyframeDesc, previous: f32) -> Result<Keyframe, i32> {
    if !key.time.is_finite() || key.time < 0.0 || key.time < previous || key.reserved != 0 || !key.values.iter().all(|v| v.is_finite()) {
        return Err(unsafe { fail(ui, "invalid animation keyframe") });
    }
    let target = match key.target {
        0 => Target::Offset(key.values[0], key.values[1]),
        1 if (0.0..=1.0).contains(&key.values[0]) => Target::Opacity(key.values[0]),
        2 => { let Some(look) = decode_look(key.look) else { return Err(unsafe { fail(ui, "invalid animation look") }); }; Target::Look(look) },
        3 => Target::Scale { x: key.values[0], y: key.values[1], anchor: (key.values[2], key.values[3]) },
        _ => return Err(unsafe { fail(ui, "invalid animation target") }),
    };
    let Some(curve) = curve(key.curve) else { return Err(unsafe { fail(ui, "invalid animation curve") }); };
    Ok(match target {
        Target::Offset(x, y) => Keyframe::offset(key.time, x, y),
        Target::Opacity(v) => Keyframe::opacity(key.time, v),
        Target::Look(look) => Keyframe::look(key.time, &look),
        Target::Scale { x, y, anchor } => Keyframe::scale(key.time, x, y).anchor(anchor.0, anchor.1),
    }.curve(curve))
}

// Plays the 1 to 8 keyframes `keys` on `box_` `repeat` times (1: once; 0: forever; a count includes the reverse leg), reversing every
// other pass when `autoreverse` is 1, starting `start_offset` seconds in; writes 1 to `out` while the run is going.
export!(pub unsafe fn pxui_ui_animate_keys(ui: *mut PxuiUi, box_: *const PxuiBox, keys: Slice<PxuiKeyframeDesc>, repeat: u32, autoreverse: u8, start_offset: f32, out: *mut u8) -> i32 {
    unsafe { animate_keys(ui, box_, keys, (repeat, autoreverse, start_offset), if repeat == 0 { 30 } else { 0 }, out) }
});

// As pxui_ui_animate_keys, with an explicit idle cadence in Hz (0: display rate).
export!(pub unsafe fn pxui_ui_animate_keys_at_rate(ui: *mut PxuiUi, box_: *const PxuiBox, keys: Slice<PxuiKeyframeDesc>, repeat: u32, autoreverse: u8, start_offset: f32, rate_hz: u8, out: *mut u8) -> i32 {
    unsafe { animate_keys(ui, box_, keys, (repeat, autoreverse, start_offset), rate_hz, out) }
});

unsafe fn animate_keys(ui: *mut PxuiUi, box_: *const PxuiBox, keys: Slice<PxuiKeyframeDesc>, repeat: (u32, u8, f32), rate_hz: u8, out: *mut u8) -> i32 {
    let (engine, target_box) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if out.is_null() || keys.len == 0 || keys.len > MAX_KEYS || keys.ptr.is_null() { return unsafe { fail(ui, "invalid animation keyframe span") }; }
    if repeat.0 > u16::MAX as u32 || repeat.1 > 1 || !repeat.2.is_finite() || repeat.2 < 0.0 { return unsafe { fail(ui, "invalid animation repeat") }; }
    let raw = unsafe { core::slice::from_raw_parts(keys.ptr, keys.len) };
    if !raw.iter().all(|key| key.size as usize == core::mem::size_of_val(key)) { return unsafe { fail(ui, "invalid animation keyframe span") }; }
    // Decode the complete bounded packet before changing the running animation.
    let mut decoded = [Keyframe::opacity(0.0, 1.0); MAX_KEYS];
    let mut previous = -1.0f32;
    for (slot, key) in decoded.iter_mut().zip(raw) {
        match unsafe { decode_key(ui, key, previous) } { Ok(key) => { previous = key.time(); *slot = key; } Err(code) => return code }
    }
    let (count, reverse, offset) = repeat;
    if (count, reverse, offset) != (1, 0, 0.0) && previous <= 0.0 {
        return unsafe { fail(ui, "repeating animation needs positive duration") };
    }
    let cycles = if count == 0 { Repeat::Forever } else { Repeat::Count(count as u16) };
    let rate = if rate_hz == 0 { pxui_types::ui::Rate::Display } else { pxui_types::ui::Rate::Ambient(rate_hz) };
    let play = Keyframes::DEFAULT.repeat(cycles).offset(offset).rate(rate);
    let running = engine.animate_keys(&target_box, &decoded[..raw.len()], &if reverse != 0 { play.autoreverse() } else { play });
    unsafe { *out = running as u8; }
    0
}

// Moves the box towards one target along a curve; the keyframe's time is ignored.
export!(pub unsafe fn pxui_ui_animate(ui: *mut PxuiUi, box_: *const PxuiBox, key: *const PxuiKeyframeDesc, out: *mut u8) -> i32 {
    let (engine, target_box) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if key.is_null() || out.is_null() { return unsafe { fail(ui, "invalid animation argument") }; }
    // a keyframe passed alone is read at its own size (a newer, longer one is fine); `decode_key` then checks it whole
    let Some(key) = sized!(key) else { return unsafe { fail(ui, "invalid animation keyframe") }; };
    let (target, curve) = match unsafe { decode_key(ui, key, -1.0) } { Ok(key) => (key.target(), key.segment()), Err(code) => return code };
    unsafe { *out = engine.animate(&target_box, target, curve) as u8; }
    0
});

// ABI 1.76: the split of `desc` reporting the grip's response (the empty one when the body builds no grip) into `out`; `has_ratio` is
// 1 with the ratio under the pointer in `ratio` while the grip is held and on its release frame (the release value is final), else 0.
// A refused call writes none of them.
export!(trusted pub unsafe fn pxui_ui_split(ui: *mut PxuiUi, desc: *const PxuiSplitDesc, out: *mut PxuiResponse, has_ratio: *mut u8, ratio: *mut f32) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(response) = sized_mut!(out) else { return unsafe { fail(ui, "null or short split response") } };
    if desc.is_null() || has_ratio.is_null() || ratio.is_null() { return unsafe { fail(ui, "null split argument") }; }
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid split descriptor") }; };
    if desc.axis > 1 { return unsafe { fail(ui, "invalid split descriptor") }; }
    let Some(label) = (unsafe { crate::state::checked_key(desc.label) }) else { return unsafe { fail(ui, "invalid split label") }; };
    let axis = if desc.axis == 0 { Axis::Row } else { Axis::Column };
    let mut at = None;
    let rc = unsafe { crate::trusted::split(ui, label, axis, desc.ratio, desc.min_a, desc.min_b,
        &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, desc.build_a, desc.user_a), &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, desc.build_b, desc.user_b), response, &mut at) };
    if rc == 0 { unsafe { (*has_ratio, *ratio) = (at.is_some() as u8, at.unwrap_or_default()); } }
    rc
});

// ABI 1.76: `pxui_ui_menu_bar` that also sets `dismissed` to 1 when the menu open at the frame's start closed without a pick.
export!(build pub unsafe fn pxui_ui_menu_bar(ui: *mut PxuiUi, desc: *const PxuiMenuBarDesc, out: *mut PxuiMenuOut, dismissed: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if dismissed.is_null() { return unsafe { fail(ui, "null menu bar argument") }; }
    unsafe { menu_bar(ui, desc, out, &mut *dismissed) }
});

/// The body of `pxui_ui_menu_bar`.
unsafe fn menu_bar(ui: *mut PxuiUi, desc: *const PxuiMenuBarDesc, out: *mut PxuiMenuOut, dismissed: &mut u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() || out.is_null() { return unsafe { fail(ui, "null menu bar argument") }; }
    let (Some(desc), Some(out)) = (sized!(desc), sized_mut!(out)) else { return unsafe { fail(ui, "invalid menu bar descriptor") }; };
    if desc.menus.len > MAX_MENUS || (desc.menus.len != 0 && desc.menus.ptr.is_null()) { return unsafe { fail(ui, "invalid menu bar span") }; }
    let Some(label) = (unsafe { crate::state::checked_key(desc.label) }) else { return unsafe { fail(ui, "invalid menu bar label") }; };
    let raw = if desc.menus.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(desc.menus.ptr, desc.menus.len) } };
    if !raw.iter().all(|menu| menu.size as usize == core::mem::size_of_val(menu)) { return unsafe { fail(ui, "invalid menu descriptor") }; }
    // all items go into one buffer, then each menu is a slice of it; both buffers are reused between frames
    let (mut flat, mut menus) = { let shared = state.shared(); (shared.take_strs(), core::mem::take(&mut shared.menus)) };
    flat.clear();
    menus.clear();
    let mut total = 0usize;
    let mut valid = true;
    for menu in raw {
        let before = flat.len();
        valid &= unsafe { checked(menu.title) }.is_some() && unsafe { append_strings(menu.items, MAX_MENUS, &mut flat) };
        let bytes = flat[before..].iter().map(|s| s.len()).sum::<usize>() + menu.title.len;
        total = total.saturating_add(bytes);
        if !valid || total > MAX_TOTAL_TEXT_BYTES { valid = false; break; }
    }
    let picked = if valid {
        let mut at = 0;
        for menu in raw {
            // `flat` is complete: the slices stay valid until it is cleared below
            let items: &'static [&'static str] = unsafe { core::slice::from_raw_parts(flat.as_ptr().add(at), menu.items.len) };
            menus.push((unsafe { checked(menu.title) }.unwrap_or(""), items));
            at += menu.items.len;
        }
        Some(with_variant!(ui, pxui_types::theme::BodyKind::menu_bar, desc.variant, unsafe { (&mut *state.engine).menu_bar_result(label, menus.len(), &|m| (menus[m].0, menus[m].1.len()), &|m, i| menus[m].1[i]) }))
    } else { None };
    menus.clear();
    { let shared = state.shared(); shared.put_strs(flat); shared.menus = menus; }
    let Some((picked, closed)) = picked else { return unsafe { fail(ui, "invalid menu bar strings") }; };
    (out.menu, out.item) = picked.unwrap_or((usize::MAX, usize::MAX));
    *dismissed = closed as u8;
    0
}

export!(trusted pub unsafe fn pxui_ui_inspector_row(ui: *mut PxuiUi, desc: *mut PxuiInspectorRowDesc) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(desc) = sized_mut!(desc)else { return unsafe { fail(ui, "invalid inspector row descriptor") }; };
    if desc.has_width > 1 || desc.has_pin > 1 || desc.pin > 1 || desc.reserved != 0 || desc.reserved_tail != 0 { return unsafe { fail(ui, "invalid inspector row descriptor") }; }
    let (Some(key), Some(label)) = (unsafe { crate::state::checked_key(desc.key) }, unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid inspector row text") }; };
    let mut row = (PxuiBox::default(), 0.0, 0.0, 0.0);
    let rc = unsafe { crate::trusted::inspector_row(ui, (desc.has_width != 0).then_some(desc.width), (desc.has_pin != 0).then_some(desc.pin != 0), key, label, &mut row) };
    if rc == 0 { (desc.row, desc.control_x, desc.control_y, desc.control_w) = row; }
    rc
});

export!(trusted pub unsafe fn pxui_ui_inspector_section(ui: *mut PxuiUi, desc: *mut PxuiInspectorSectionDesc) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(desc) = sized_mut!(desc)else { return unsafe { fail(ui, "invalid inspector section descriptor") }; };
    if desc.expanded > 1 || desc.has_set > 1 || desc.set_expanded > 1 || desc.reserved != 0 { return unsafe { fail(ui, "invalid inspector section descriptor") }; }
    let (Some(key), Some(label)) = (unsafe { crate::state::checked_key(desc.key) }, unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid inspector section text") }; };
    let (build, user) = (desc.build, desc.user);
    let mut open = false;
    let rc = unsafe { crate::trusted::inspector_section(ui, key, desc.expanded != 0, (desc.has_set != 0).then_some(desc.set_expanded != 0), label, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, build, user), &mut open) };
    if rc == 0 { desc.out_open = open as u8; }
    rc
});

export!(trusted pub unsafe fn pxui_ui_inspector_toggle(ui: *mut PxuiUi, key: Str, label: Str, value: u8, out: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() || value > 1 { return unsafe { fail(ui, "invalid inspector toggle arguments") }; }
    let (Some(key), Some(label)) = (unsafe { crate::state::checked_key(key) }, unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid inspector toggle text") }; };
    let mut on = false;
    let rc = unsafe { crate::trusted::inspector_toggle(ui, key, label, value != 0, &mut on) };
    if rc == 0 { unsafe { *out = on as u8; } }
    rc
});

export!(trusted pub unsafe fn pxui_ui_inspector_button(ui: *mut PxuiUi, key: Str, label: Str, out: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null inspector button output") }; }
    let (Some(key), Some(label)) = (unsafe { crate::state::checked_key(key) }, unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid inspector button text") }; };
    let mut pressed = false;
    let rc = unsafe { crate::trusted::inspector_button(ui, key, label, &mut pressed) };
    if rc == 0 { unsafe { *out = pressed as u8; } }
    rc
});

export!(trusted pub unsafe fn pxui_ui_inspector_readout(ui: *mut PxuiUi, desc: *const PxuiInspectorValueDesc) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(desc) = sized!(desc)else { return unsafe { fail(ui, "invalid inspector readout descriptor") }; };
    if desc.has_width > 1 || desc.reserved != [0; 3] { return unsafe { fail(ui, "invalid inspector readout descriptor") }; }
    let (Some(key), Some(label), Some(value)) = (unsafe { crate::state::checked_key(desc.key) }, unsafe { checked(desc.label) }, unsafe { checked(desc.value) }) else { return unsafe { fail(ui, "invalid inspector readout text") }; };
    unsafe { crate::trusted::inspector_readout(ui, (desc.has_width != 0).then_some(desc.width), key, label, value) }
});

export!(trusted pub unsafe fn pxui_ui_inspector_message(ui: *mut PxuiUi, desc: *const PxuiInspectorTextDesc) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(desc)=sized!(desc) else { return unsafe { fail(ui, "invalid inspector message descriptor") }; };
    let (Some(key), Some(value)) = (unsafe { crate::state::checked_key(desc.key) }, unsafe { checked(desc.text) }) else { return unsafe { fail(ui, "invalid inspector message text") }; };
    unsafe { crate::trusted::inspector_message(ui, key, value) }
});
