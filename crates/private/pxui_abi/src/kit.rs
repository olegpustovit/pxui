use crate::state::{boxed, MAX_TEXT_BYTES, MAX_TOTAL_TEXT_BYTES, PxuiUi, append_strings, checked, fail, finite, get, text};
use crate::style::decode_look;
use crate::ui::translate_event;
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str};
use pxui_core::ui::{Axis, Target, MAX_KEYS};
use pxui_types::ffi::{PxuiCurveDesc, PxuiGridListDesc, Slice};
pub use pxui_types::ffi::{PxuiFooterDesc, PxuiHintDesc, PxuiInspectorRowDesc, PxuiInspectorSectionDesc, PxuiInspectorTextDesc, PxuiInspectorValueDesc, PxuiKeyframeDesc, PxuiMenuBarDesc, PxuiMenuOut, PxuiSplitDesc, PxuiTableDesc};

const MAX_MENUS: usize = 256;

export!(trusted pub unsafe fn pxui_ui_list_measured(ui:*mut PxuiUi,label:Str,count:usize,estimate_h:f32,build:pxui_types::ffi::PxuiGridBuild,user:*mut core::ffi::c_void,out:*mut PxuiBox)->i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui,"null measured list output") }; }
    let Some(label)=(unsafe { checked(label) }) else { return unsafe { fail(ui,"invalid measured list label") }; };
    unsafe { crate::trusted::list_measured(ui,label,count,estimate_h,&mut |ui,index| {
        if let Some(build)=build { build(user,ui.cast(),index); }
    },&mut *out) }
});

export!(trusted pub unsafe fn pxui_ui_section_list(ui:*mut PxuiUi,desc:*const pxui_types::ffi::PxuiSectionListDesc,out:*mut PxuiBox)->i32 {
    if unsafe {get(ui)}.is_none() {return PXUI_ERR_ARGUMENT;}
    if desc.is_null() || out.is_null() {return unsafe {fail(ui,"null section list argument")};}
    let Some(desc)=sized!(desc) else {return unsafe {fail(ui,"invalid section list descriptor")};};
    if desc.overlay_scrollbar>1 || desc.reserved!=[0;3] {return unsafe {fail(ui,"invalid section list flags")};}
    let Some(label)=(unsafe {checked(desc.label)}) else {return unsafe {fail(ui,"invalid section list label")};};
    let section=pxui_types::ui::SectionList {count:desc.count,row_height:desc.row_height,leading_height:desc.leading_height,header_height:desc.header_height,trailing_height:desc.trailing_height,overlay_scrollbar:desc.overlay_scrollbar!=0};
    unsafe {crate::trusted::section_list(ui,label,&section,&mut |ui,index| {if let Some(build)=desc.build {build(desc.user,ui.cast(),index);}},&mut *out)}
});

#[cfg(test)]
mod curve_tests {
    use super::*;

    #[test]
    fn scale_keyframes_keep_axes_and_anchor_and_reject_nonfinite_values() {
        let ui = unsafe { crate::pxui_ui_create(&crate::PxuiUiDesc::default()) };
        let key = PxuiKeyframeDesc { size: core::mem::size_of::<PxuiKeyframeDesc>() as u32, target: 3,
            values: [2.0, 0.5, 1.0, 0.0], curve: PxuiCurveDesc { kind: PxuiCurveDesc::STEPS, duration: 0.14, count: 2, ..PxuiCurveDesc::default() }, ..PxuiKeyframeDesc::default() };
        assert_eq!(unsafe { decode_key(ui, &key, -1.0) }, Ok((0.0, Target::Scale { x: 2.0, y: 0.5, anchor: (1.0, 0.0) }, pxui_types::Curve::Steps { duration: 0.14, count: 2 })));
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

fn curve(value: PxuiCurveDesc) -> Option<pxui_types::Curve> {
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
    let Some(label) = (unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid grid list label") }; };
    let (shared, build, user) = (unsafe { (*ui).shared }, desc.build, desc.user);
    unsafe { crate::trusted::grid_cells(ui, label, desc.count, desc.cell_w, desc.cell_h, &mut |engine: &mut pxui_core::ui::Ui, index| {
        let mut borrowed = PxuiUi::borrowed(engine, shared);
        let _current = crate::state::Current::set(shared, &mut borrowed);
        if let Some(build) = build { build(user, (&mut borrowed as *mut PxuiUi).cast(), index) }
    }, &mut *out) }
});

export!(pub unsafe fn pxui_ui_wants_frame(ui: *mut PxuiUi, events: Slice<pxui_types::Event>, now: f64, out: *mut u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() || !now.is_finite() || events.len > 4096 || (events.len != 0 && events.ptr.is_null()) { return unsafe { fail(ui, "invalid wants frame arguments") }; }
    let mut translated = Vec::with_capacity(events.len);
    if events.len != 0 {
        for event in unsafe { core::slice::from_raw_parts(events.ptr, events.len) } {
            if event.text.len > MAX_TEXT_BYTES || !finite(&[event.x as f32, event.y as f32, event.value as f32]) || ![event.x, event.y, event.value].iter().all(|v| v.is_finite()) { return unsafe { fail(ui, "invalid wants frame event") }; }
            let Some(event) = translate_event(event) else { return unsafe { fail(ui, "invalid wants frame event") }; };
            translated.push(event);
        }
    }
    unsafe { *out = (&*state.engine).wants_frame(&translated, now) as u8; }
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
    // the engine takes owned rows; the strings are reused between frames, so a settled footer allocates nothing
    let mut hints = core::mem::take(&mut state.shared().hints);
    hints.truncate(raw.len());
    for (at, hint) in raw.iter().enumerate() {
        let (Some(key), Some(value)) = (unsafe { checked(hint.key) }, unsafe { checked(hint.value) }) else { state.shared().hints = hints; return unsafe { fail(ui, "invalid footer hint") }; };
        match hints.get_mut(at) {
            Some(row) => { row.0.clear(); row.0.push_str(key); row.1.clear(); row.1.push_str(value); }
            None => hints.push((key.to_owned(), value.to_owned())),
        }
    }
    let rc = unsafe { crate::trusted::footer(ui, right, &hints) };
    state.shared().hints = hints;
    rc
});

// One keyframe checked and decoded: its time, target (an offset, an opacity in 0..=1 or a look) and curve.
unsafe fn decode_key(ui: *mut PxuiUi, key: &PxuiKeyframeDesc, previous: f32) -> Result<(f32, Target, pxui_types::Curve), i32> {
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
    Ok((key.time, target, curve))
}

export!(pub unsafe fn pxui_ui_animate_keys(ui: *mut PxuiUi, box_: *const PxuiBox, keys: Slice<PxuiKeyframeDesc>, out: *mut u8) -> i32 {
    let (engine, target_box) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if out.is_null() || keys.len == 0 || keys.len > MAX_KEYS || keys.ptr.is_null() { return unsafe { fail(ui, "invalid animation keyframe span") }; }
    let raw = unsafe { core::slice::from_raw_parts(keys.ptr, keys.len) };
    if !raw.iter().all(|key| key.size as usize == core::mem::size_of_val(key)) { return unsafe { fail(ui, "invalid animation keyframe span") }; }
    // at most MAX_KEYS keyframes: decoded on the stack
    let mut decoded = [(0.0, Target::Opacity(1.0), pxui_types::Curve::Spring { omega: 1.0 }); MAX_KEYS];
    let mut previous = -1.0f32;
    for (slot, key) in decoded.iter_mut().zip(raw) {
        match unsafe { decode_key(ui, key, previous) } { Ok(key) => { previous = key.0; *slot = key; } Err(code) => return code }
    }
    unsafe { *out = engine.animate_keys(&target_box, &decoded[..raw.len()]) as u8; }
    0
});

// Moves the box towards one target along a curve; the keyframe's time is ignored.
export!(pub unsafe fn pxui_ui_animate(ui: *mut PxuiUi, box_: *const PxuiBox, key: *const PxuiKeyframeDesc, out: *mut u8) -> i32 {
    let (engine, target_box) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if key.is_null() || out.is_null() { return unsafe { fail(ui, "invalid animation argument") }; }
    // a keyframe passed alone is read at its own size (a newer, longer one is fine); `decode_key` then checks it whole
    let Some(key) = sized!(key) else { return unsafe { fail(ui, "invalid animation keyframe") }; };
    let (_, target, curve) = match unsafe { decode_key(ui, key, -1.0) } { Ok(key) => key, Err(code) => return code };
    unsafe { *out = engine.animate(&target_box, target, curve) as u8; }
    0
});

export!(trusted pub unsafe fn pxui_ui_split(ui: *mut PxuiUi, desc: *const PxuiSplitDesc, changed: *mut u8, ratio: *mut f32) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if desc.is_null() || changed.is_null() || ratio.is_null() { return unsafe { fail(ui, "null split argument") }; }
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid split descriptor") }; };
    if desc.axis > 1 { return unsafe { fail(ui, "invalid split descriptor") }; }
    let Some(label) = (unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid split label") }; };
    let axis = if desc.axis == 0 { Axis::Row } else { Axis::Column };
    let mut moved = None;
    let rc = unsafe { crate::trusted::split(ui, label, axis, desc.ratio, desc.min_a, desc.min_b,
        &mut |ui| if let Some(build) = desc.build_a { build(desc.user_a, ui.cast()) }, &mut |ui| if let Some(build) = desc.build_b { build(desc.user_b, ui.cast()) }, &mut moved) };
    unsafe { *changed = moved.is_some() as u8; *ratio = moved.unwrap_or_default(); }
    rc
});

export!(pub unsafe fn pxui_ui_menu_bar(ui: *mut PxuiUi, desc: *const PxuiMenuBarDesc, out: *mut PxuiMenuOut) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() || out.is_null() { return unsafe { fail(ui, "null menu bar argument") }; }
    let (Some(desc), Some(out)) = (sized!(desc), sized_mut!(out)) else { return unsafe { fail(ui, "invalid menu bar descriptor") }; };
    if desc.menus.len > MAX_MENUS || (desc.menus.len != 0 && desc.menus.ptr.is_null()) { return unsafe { fail(ui, "invalid menu bar span") }; }
    let Some(label) = (unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid menu bar label") }; };
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
        Some(unsafe { (&mut *state.engine).menu_bar(label, &menus) })
    } else { None };
    menus.clear();
    { let shared = state.shared(); shared.put_strs(flat); shared.menus = menus; }
    let Some(picked) = picked else { return unsafe { fail(ui, "invalid menu bar strings") }; };
    (out.menu, out.item) = picked.unwrap_or((usize::MAX, usize::MAX));
    0
});

export!(trusted pub unsafe fn pxui_ui_inspector_row(ui: *mut PxuiUi, desc: *mut PxuiInspectorRowDesc) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(desc) = sized_mut!(desc)else { return unsafe { fail(ui, "invalid inspector row descriptor") }; };
    if desc.has_width > 1 || desc.has_pin > 1 || desc.pin > 1 || desc.reserved != 0 || desc.reserved_tail != 0 { return unsafe { fail(ui, "invalid inspector row descriptor") }; }
    let (Some(key), Some(label)) = (unsafe { checked(desc.key) }, unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid inspector row text") }; };
    let mut row = (PxuiBox::default(), 0.0, 0.0, 0.0);
    let rc = unsafe { crate::trusted::inspector_row(ui, (desc.has_width != 0).then_some(desc.width), (desc.has_pin != 0).then_some(desc.pin != 0), key, label, &mut row) };
    if rc == 0 { (desc.row, desc.control_x, desc.control_y, desc.control_w) = row; }
    rc
});

export!(trusted pub unsafe fn pxui_ui_inspector_section(ui: *mut PxuiUi, desc: *mut PxuiInspectorSectionDesc) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(desc) = sized_mut!(desc)else { return unsafe { fail(ui, "invalid inspector section descriptor") }; };
    if desc.expanded > 1 || desc.has_set > 1 || desc.set_expanded > 1 || desc.reserved != 0 { return unsafe { fail(ui, "invalid inspector section descriptor") }; }
    let (Some(key), Some(label)) = (unsafe { checked(desc.key) }, unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid inspector section text") }; };
    let (build, user) = (desc.build, desc.user);
    let mut open = false;
    let rc = unsafe { crate::trusted::inspector_section(ui, key, desc.expanded != 0, (desc.has_set != 0).then_some(desc.set_expanded != 0), label, &mut |ui| if let Some(build) = build { build(user, ui.cast()) }, &mut open) };
    desc.out_open = open as u8;
    rc
});

export!(trusted pub unsafe fn pxui_ui_inspector_toggle(ui: *mut PxuiUi, key: Str, label: Str, value: u8, out: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() || value > 1 { return unsafe { fail(ui, "invalid inspector toggle arguments") }; }
    let (Some(key), Some(label)) = (unsafe { checked(key) }, unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid inspector toggle text") }; };
    let mut on = false;
    let rc = unsafe { crate::trusted::inspector_toggle(ui, key, label, value != 0, &mut on) };
    unsafe { *out = on as u8; }
    rc
});

export!(trusted pub unsafe fn pxui_ui_inspector_button(ui: *mut PxuiUi, key: Str, label: Str, out: *mut u8) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if out.is_null() { return unsafe { fail(ui, "null inspector button output") }; }
    let (Some(key), Some(label)) = (unsafe { checked(key) }, unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid inspector button text") }; };
    let mut pressed = false;
    let rc = unsafe { crate::trusted::inspector_button(ui, key, label, &mut pressed) };
    unsafe { *out = pressed as u8; }
    rc
});

export!(trusted pub unsafe fn pxui_ui_inspector_readout(ui: *mut PxuiUi, desc: *const PxuiInspectorValueDesc) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(desc) = sized!(desc)else { return unsafe { fail(ui, "invalid inspector readout descriptor") }; };
    if desc.has_width > 1 || desc.reserved != [0; 3] { return unsafe { fail(ui, "invalid inspector readout descriptor") }; }
    let (Some(key), Some(label), Some(value)) = (unsafe { checked(desc.key) }, unsafe { checked(desc.label) }, unsafe { checked(desc.value) }) else { return unsafe { fail(ui, "invalid inspector readout text") }; };
    unsafe { crate::trusted::inspector_readout(ui, (desc.has_width != 0).then_some(desc.width), key, label, value) }
});

export!(trusted pub unsafe fn pxui_ui_inspector_message(ui: *mut PxuiUi, desc: *const PxuiInspectorTextDesc) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(desc)=sized!(desc) else { return unsafe { fail(ui, "invalid inspector message descriptor") }; };
    let (Some(key), Some(value)) = (unsafe { checked(desc.key) }, unsafe { checked(desc.text) }) else { return unsafe { fail(ui, "invalid inspector message text") }; };
    unsafe { crate::trusted::inspector_message(ui, key, value) }
});

export!(trusted pub unsafe fn pxui_ui_table(ui: *mut PxuiUi, desc: *const PxuiTableDesc, clicked_row: *mut i64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() || clicked_row.is_null() { return unsafe { fail(ui, "null table argument") }; }
    let Some(desc) = sized!(desc) else { return unsafe { fail(ui, "invalid table descriptor") }; };
    if desc.headers.len > 256 || (desc.headers.len != 0 && desc.headers.ptr.is_null()) { return unsafe { fail(ui, "invalid table descriptor") }; }
    let mut list = state.shared().take_strs();
    let Some(label) = (unsafe { checked(desc.label) }).filter(|_| unsafe { crate::state::strings(desc.headers, 256, &mut list) }) else {
        state.shared().put_strs(list);
        return unsafe { fail(ui, "invalid table strings") };
    };
    let (callback, user) = (desc.cell, desc.user);
    let mut bad_text = false;
    let mut clicked = None;
    let rc = unsafe { crate::trusted::table(ui, (desc.at[0], desc.at[1]), desc.width, desc.height, &list, desc.rows, &mut |row, col, put| {
        let Some(callback) = callback else { return put("") };
        match text(callback(user, row, col)) { Some(value) => put(value), None => { bad_text = true; put("") } }
    }, label, &mut clicked) };
    state.shared().put_strs(list);
    if rc != 0 { return rc; }
    if bad_text { return unsafe { fail(ui, "invalid table cell callback text") }; }
    unsafe { *clicked_row = clicked.unwrap_or(-1); }
    0
});

#[cfg(test)]
mod measured_list_tests {
    use crate::*;
    #[test]
    fn measured_lists_validate_c_arguments_and_keep_row_handles_scoped() {
        struct State { owner:*mut crate::state::PxuiUi,frame:*mut crate::state::PxuiUi,rows:u32 }
        extern "C" fn row(user:*mut core::ffi::c_void,raw:*mut core::ffi::c_void,index:usize) {
            let state=unsafe { &mut *user.cast::<State>() };
            assert_eq!(index,0);
            state.rows+=1;
            let mut size=pxui_types::geom::Point::default();
            assert_eq!(unsafe { crate::pxui_ui_view_size(state.owner,&mut size) },PXUI_ERR_STATE);
            assert_eq!(unsafe { crate::pxui_ui_view_size(state.frame,&mut size) },PXUI_ERR_STATE);
            assert_eq!(unsafe { crate::pxui_ui_view_size(raw.cast(),&mut size) },0);
            let node=pxui_types::Node { h:pxui_types::Size::Px(20.0),..pxui_types::Node::DEFAULT };
            let mut out=PxuiBox::default();
            assert_eq!(unsafe { crate::pxui_ui_node_keyed(raw.cast(),77,&node,&mut out) },0);
        }
        extern "C" fn build(user:*mut core::ffi::c_void,raw:*mut core::ffi::c_void) {
            let state=unsafe { &mut *user.cast::<State>() };
            state.frame=raw.cast();
            let mut out=PxuiBox::default();
            for (count,estimate) in [(1_000_001,20.0),(1,0.0),(1,f32::NAN),(2,f32::MAX)] {
                assert_eq!(unsafe { super::pxui_ui_list_measured(raw.cast(),Str::default(),count,estimate,Some(row),user,&mut out) },PXUI_ERR_ARGUMENT);
            }
            assert_eq!(state.rows,0,"invalid arguments never reach rows or allocate their cache");
            assert_eq!(unsafe { super::pxui_ui_list_measured(raw.cast(),Str::default(),1,20.0,Some(row),user,&mut out) },0);
        }
        let ui=unsafe { crate::pxui_ui_create(&PxuiUiDesc::default()) };
        let mut out=PxuiBox::default();
        assert_eq!(unsafe { super::pxui_ui_list_measured(ui,Str::default(),1,20.0,None,core::ptr::null_mut(),&mut out) },PXUI_ERR_STATE);
        assert_eq!(unsafe { super::pxui_ui_list_measured(ui,Str::default(),1,20.0,None,core::ptr::null_mut(),core::ptr::null_mut()) },PXUI_ERR_ARGUMENT);
        let mut state=State { owner:ui,frame:core::ptr::null_mut(),rows:0 };
        let mut frame=FrameOut::default();
        assert_eq!(unsafe { crate::pxui_frame(ui,&FrameIn { width:320.0,height:240.0,..Default::default() },Some(build),(&mut state as *mut State).cast(),&mut frame) },0);
        assert!(state.rows>0 && state.rows<=3);
        assert_eq!(unsafe { crate::pxui_ui_destroy(ui) },0);
    }
}
