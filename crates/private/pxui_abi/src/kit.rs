use crate::state::{boxed, Build, MAX_TEXT_BYTES, MAX_TOTAL_TEXT_BYTES, PxuiUi, append_strings, call_build, checked, fail, finite, get, to_abi_box};
use crate::style::decode_look;
use crate::ui::translate_event;
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, Str};
use core::ffi::c_void;
use pxui_core::ui::{Axis, Ui, Target, MAX_KEYS};
use pxui_types::ffi::{PxuiCurveDesc, PxuiLookDesc, Slice};

const MAX_ITEMS: usize = 1_000_000;
const MAX_MENUS: usize = 256;

pub type PxuiBuild = Build;
pub type PxuiGridBuild = Option<extern "C" fn(*mut c_void, *mut PxuiUi, usize)>;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiMenuDesc { pub title: Str, pub items: Slice<Str> }
#[repr(C)]
pub struct PxuiGridListDesc { pub label: Str, pub count: usize, pub cell_w: f32, pub cell_h: f32, pub build: PxuiGridBuild, pub user: *mut c_void }
#[repr(C)]
pub struct PxuiSplitDesc { pub label: Str, pub axis: u32, pub ratio: f32, pub min_a: f32, pub min_b: f32, pub build_a: PxuiBuild, pub user_a: *mut c_void, pub build_b: PxuiBuild, pub user_b: *mut c_void }
#[repr(C)]
pub struct PxuiMenuBarDesc { pub label: Str, pub menus: Slice<PxuiMenuDesc> }
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiMenuOut { pub menu: usize, pub item: usize }
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiHintDesc { pub key: Str, pub value: Str }
#[repr(C)]
pub struct PxuiFooterDesc { pub has_right: u8, pub reserved: [u8; 7], pub right: Str, pub hints: Slice<PxuiHintDesc> }
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiKeyframeDesc { pub time: f32, pub target: u32, pub reserved: [u8; 4], pub values: [f32; 4], pub look: PxuiLookDesc, pub curve: PxuiCurveDesc }

#[repr(C)]
pub struct PxuiInspectorRowDesc { pub key: Str, pub label: Str, pub has_width: u8, pub has_pin: u8, pub pin: u8, pub reserved: [u8; 5], pub width: f32, pub row: PxuiBox, pub control_x: f32, pub control_y: f32, pub control_w: f32 }
#[repr(C)]
pub struct PxuiInspectorSectionDesc { pub key: Str, pub label: Str, pub expanded: u8, pub has_set: u8, pub set_expanded: u8, pub reserved: [u8; 5], pub build: PxuiBuild, pub user: *mut c_void, pub out_open: u8 }
#[repr(C)]
pub struct PxuiInspectorValueDesc { pub key: Str, pub label: Str, pub value: Str, pub has_width: u8, pub reserved: [u8; 7], pub width: f32 }
#[repr(C)]
pub struct PxuiInspectorTextDesc { pub key: Str, pub text: Str }
pub type PxuiTableCell = Option<extern "C" fn(*mut c_void, i64, i64) -> Str>;
#[repr(C)]
pub struct PxuiTableDesc { pub label: Str, pub at: [f32; 2], pub width: f32, pub height: f32, pub headers: Slice<Str>, pub rows: i64, pub cell: PxuiTableCell, pub user: *mut c_void }

fn curve(value: PxuiCurveDesc) -> Option<pxui_types::Curve> {
    if !value.duration.is_finite() || !value.omega.is_finite() || !value.points.iter().all(|v| v.is_finite()) { return None; }
    Some(match value.kind {
        PxuiCurveDesc::BEZIER if value.duration >= 0.0 && (0.0..=1.0).contains(&value.points[0]) && (0.0..=1.0).contains(&value.points[2]) => pxui_types::Curve::Bezier { duration: value.duration, points: value.points },
        PxuiCurveDesc::SPRING if value.omega > 0.0 => pxui_types::Curve::Spring { omega: value.omega },
        _ => return None,
    })
}
export!(pub unsafe fn pxui_ui_grid_list(ui: *mut PxuiUi, desc: *const PxuiGridListDesc, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() || out.is_null() { return unsafe { fail(ui, "null grid list argument") }; }
    let desc = unsafe { &*desc };
    if desc.count > MAX_ITEMS || !desc.cell_w.is_finite() || !desc.cell_h.is_finite() || desc.cell_w <= 0.0 || desc.cell_h <= 0.0 {
        return unsafe { fail(ui, "invalid grid list descriptor") };
    }
    let Some(label) = (unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid grid list label") }; };
    let shared = state.shared;
    let mut build = |engine: &mut Ui, index| if let Some(callback) = desc.build {
        let mut borrowed = PxuiUi::borrowed(engine, shared);
        callback(desc.user, &mut borrowed, index);
    };
    let value = unsafe { (&mut *state.engine).grid_list(label, desc.count, desc.cell_w, desc.cell_h, &mut build) };
    unsafe { *out = to_abi_box(value); }
    0
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

export!(pub unsafe fn pxui_ui_footer(ui: *mut PxuiUi, desc: *const PxuiFooterDesc) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() { return unsafe { fail(ui, "null footer descriptor") }; }
    let desc = unsafe { &*desc };
    if desc.has_right > 1 || desc.reserved != [0; 7] || desc.hints.len > 4096 || (desc.hints.len != 0 && desc.hints.ptr.is_null()) {
        return unsafe { fail(ui, "invalid footer descriptor") };
    }
    let right = if desc.has_right != 0 { let Some(s) = (unsafe { checked(desc.right) }) else { return unsafe { fail(ui, "invalid footer text") }; }; Some(s) } else { None };
    let raw = if desc.hints.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(desc.hints.ptr, desc.hints.len) } };
    // the engine takes owned rows; the strings are reused between frames, so a settled footer allocates nothing
    let mut hints = core::mem::take(&mut state.shared().hints);
    hints.truncate(raw.len());
    let mut total = right.map_or(0, str::len);
    for (at, hint) in raw.iter().enumerate() {
        let (Some(key), Some(value)) = (unsafe { checked(hint.key) }, unsafe { checked(hint.value) }) else { state.shared().hints = hints; return unsafe { fail(ui, "invalid footer hint") }; };
        total = match total.checked_add(key.len()).and_then(|v| v.checked_add(value.len())) { Some(v) if v <= MAX_TOTAL_TEXT_BYTES => v, _ => { state.shared().hints = hints; return unsafe { fail(ui, "footer text limit exceeded") } } };
        match hints.get_mut(at) {
            Some(row) => { row.0.clear(); row.0.push_str(key); row.1.clear(); row.1.push_str(value); }
            None => hints.push((key.to_owned(), value.to_owned())),
        }
    }
    unsafe { (&mut *state.engine).footer(right, &hints); }
    state.shared().hints = hints;
    0
});

// One keyframe checked and decoded: its time, target (an offset, an opacity in 0..=1 or a look) and curve.
unsafe fn decode_key(ui: *mut PxuiUi, key: &PxuiKeyframeDesc, previous: f32) -> Result<(f32, Target, pxui_types::Curve), i32> {
    if !key.time.is_finite() || key.time < 0.0 || key.time < previous || key.reserved != [0; 4] || !key.values.iter().all(|v| v.is_finite()) {
        return Err(unsafe { fail(ui, "invalid animation keyframe") });
    }
    let target = match key.target {
        0 => Target::Offset(key.values[0], key.values[1]),
        1 if (0.0..=1.0).contains(&key.values[0]) => Target::Opacity(key.values[0]),
        2 => { let Some(look) = decode_look(key.look) else { return Err(unsafe { fail(ui, "invalid animation look") }); }; Target::Look(look) },
        _ => return Err(unsafe { fail(ui, "invalid animation target") }),
    };
    let Some(curve) = curve(key.curve) else { return Err(unsafe { fail(ui, "invalid animation curve") }); };
    Ok((key.time, target, curve))
}

export!(pub unsafe fn pxui_ui_animate_keys(ui: *mut PxuiUi, box_: *const PxuiBox, keys: Slice<PxuiKeyframeDesc>, out: *mut u8) -> i32 {
    let (engine, target_box) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if out.is_null() || keys.len == 0 || keys.len > MAX_KEYS || keys.ptr.is_null() { return unsafe { fail(ui, "invalid animation keyframe span") }; }
    let raw = unsafe { core::slice::from_raw_parts(keys.ptr, keys.len) };
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
    let (_, target, curve) = match unsafe { decode_key(ui, &*key, -1.0) } { Ok(key) => key, Err(code) => return code };
    unsafe { *out = engine.animate(&target_box, target, curve) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_split(ui: *mut PxuiUi, desc: *const PxuiSplitDesc, changed: *mut u8, ratio: *mut f32) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() || changed.is_null() || ratio.is_null() { return unsafe { fail(ui, "null split argument") }; }
    let desc = unsafe { &*desc };
    if desc.axis > 1 || ![desc.ratio, desc.min_a, desc.min_b].iter().all(|v| v.is_finite()) || desc.min_a < 0.0 || desc.min_b < 0.0 {
        return unsafe { fail(ui, "invalid split descriptor") };
    }
    let Some(label) = (unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid split label") }; };
    let shared = state.shared;
    let mut a = |engine: &mut Ui| call_build(shared, engine, desc.build_a, desc.user_a);
    let mut b = |engine: &mut Ui| call_build(shared, engine, desc.build_b, desc.user_b);
    let axis = if desc.axis == 0 { Axis::Row } else { Axis::Column };
    let out = unsafe { (&mut *state.engine).split(label, axis, desc.ratio, desc.min_a, desc.min_b, &mut a, &mut b) };
    unsafe { *changed = out.is_some() as u8; *ratio = out.unwrap_or_default(); }
    0
});

export!(pub unsafe fn pxui_ui_menu_bar(ui: *mut PxuiUi, desc: *const PxuiMenuBarDesc, out: *mut PxuiMenuOut) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() || out.is_null() { return unsafe { fail(ui, "null menu bar argument") }; }
    let desc = unsafe { &*desc };
    if desc.menus.len > MAX_MENUS || (desc.menus.len != 0 && desc.menus.ptr.is_null()) { return unsafe { fail(ui, "invalid menu bar span") }; }
    let Some(label) = (unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid menu bar label") }; };
    let raw = if desc.menus.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(desc.menus.ptr, desc.menus.len) } };
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
    unsafe { *out = picked.map_or(PxuiMenuOut { menu: usize::MAX, item: usize::MAX }, |(menu, item)| PxuiMenuOut { menu, item }); }
    0
});

export!(pub unsafe fn pxui_ui_inspector_row(ui: *mut PxuiUi, desc: *mut PxuiInspectorRowDesc) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() { return unsafe { fail(ui, "null inspector row descriptor") }; }
    let desc = unsafe { &mut *desc };
    if desc.has_width > 1 || desc.has_pin > 1 || desc.pin > 1 || desc.reserved != [0; 5] || (desc.has_width != 0 && (!desc.width.is_finite() || desc.width < 0.0)) {
        return unsafe { fail(ui, "invalid inspector row descriptor") };
    }
    let (Some(key), Some(label)) = (unsafe { checked(desc.key) }, unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid inspector row text") }; };
    let (row, x, y, w) = unsafe { (&mut *state.engine).inspector_row((desc.has_width != 0).then_some(desc.width), (desc.has_pin != 0).then_some(desc.pin != 0), key, label) };
    (desc.row, desc.control_x, desc.control_y, desc.control_w) = (to_abi_box(row), x, y, w);
    0
});

export!(pub unsafe fn pxui_ui_inspector_section(ui: *mut PxuiUi, desc: *mut PxuiInspectorSectionDesc) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() { return unsafe { fail(ui, "null inspector section descriptor") }; }
    let desc = unsafe { &mut *desc };
    if desc.expanded > 1 || desc.has_set > 1 || desc.set_expanded > 1 || desc.reserved != [0; 5] { return unsafe { fail(ui, "invalid inspector section descriptor") }; }
    let (Some(key), Some(label)) = (unsafe { checked(desc.key) }, unsafe { checked(desc.label) }) else { return unsafe { fail(ui, "invalid inspector section text") }; };
    let shared = state.shared;
    let mut build = |engine: &mut Ui| call_build(shared, engine, desc.build, desc.user);
    let open = unsafe { (&mut *state.engine).inspector_section(key, desc.expanded != 0, (desc.has_set != 0).then_some(desc.set_expanded != 0), label, &mut build) };
    desc.out_open = open as u8;
    0
});

export!(pub unsafe fn pxui_ui_inspector_toggle(ui: *mut PxuiUi, key: Str, label: Str, value: u8, out: *mut u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() || value > 1 { return unsafe { fail(ui, "invalid inspector toggle arguments") }; }
    let (Some(key), Some(label)) = (unsafe { checked(key) }, unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid inspector toggle text") }; };
    unsafe { *out = (&mut *state.engine).inspector_toggle(key, label, value != 0) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_inspector_button(ui: *mut PxuiUi, key: Str, label: Str, out: *mut u8) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if out.is_null() { return unsafe { fail(ui, "null inspector button output") }; }
    let (Some(key), Some(label)) = (unsafe { checked(key) }, unsafe { checked(label) }) else { return unsafe { fail(ui, "invalid inspector button text") }; };
    unsafe { *out = (&mut *state.engine).inspector_button(key, label) as u8; }
    0
});

export!(pub unsafe fn pxui_ui_inspector_readout(ui: *mut PxuiUi, desc: *const PxuiInspectorValueDesc) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() { return unsafe { fail(ui, "null inspector readout descriptor") }; }
    let desc = unsafe { &*desc };
    if desc.has_width > 1 || desc.reserved != [0; 7] || (desc.has_width != 0 && (!desc.width.is_finite() || desc.width < 0.0)) { return unsafe { fail(ui, "invalid inspector readout descriptor") }; }
    let (Some(key), Some(label), Some(value)) = (unsafe { checked(desc.key) }, unsafe { checked(desc.label) }, unsafe { checked(desc.value) }) else { return unsafe { fail(ui, "invalid inspector readout text") }; };
    unsafe { (&mut *state.engine).inspector_readout((desc.has_width != 0).then_some(desc.width), key, label, value); }
    0
});

export!(pub unsafe fn pxui_ui_inspector_message(ui: *mut PxuiUi, desc: *const PxuiInspectorTextDesc) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() { return unsafe { fail(ui, "null inspector message descriptor") }; }
    let desc = unsafe { &*desc };
    let (Some(key), Some(value)) = (unsafe { checked(desc.key) }, unsafe { checked(desc.text) }) else { return unsafe { fail(ui, "invalid inspector message text") }; };
    unsafe { (&mut *state.engine).inspector_message(key, value); }
    0
});

export!(pub unsafe fn pxui_ui_table(ui: *mut PxuiUi, desc: *const PxuiTableDesc, clicked_row: *mut i64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if desc.is_null() || clicked_row.is_null() { return unsafe { fail(ui, "null table argument") }; }
    let desc = unsafe { &*desc };
    if desc.headers.len > 256 || desc.rows < 0 || desc.rows as usize > MAX_ITEMS || (desc.headers.len != 0 && desc.headers.ptr.is_null())
        || !desc.width.is_finite() || !desc.height.is_finite() || desc.width < 0.0 || desc.height < 0.0 || !desc.at.iter().all(|v| v.is_finite()) {
        return unsafe { fail(ui, "invalid table descriptor") };
    }
    let mut list = state.shared().take_strs();
    let Some(label) = (unsafe { checked(desc.label) }).filter(|_| unsafe { crate::state::strings(desc.headers, 256, &mut list) }) else {
        state.shared().put_strs(list);
        return unsafe { fail(ui, "invalid table strings") };
    };
    let callback = desc.cell;
    let user = desc.user;
    let (mut bad_text, mut text_bytes) = (false, 0usize);
    let mut cell = |row: i64, col: i64, emit: &mut dyn FnMut(&str)| {
        let Some(callback) = callback else { return emit(""); };
        let value = callback(user, row, col);
        text_bytes = text_bytes.saturating_add(value.len);
        match unsafe { checked(value) } {
            Some(value) if text_bytes <= MAX_TOTAL_TEXT_BYTES => emit(value),
            _ => { bad_text = true; emit("") }
        }
    };
    let (clicked, _) = unsafe { (&mut *state.engine).table_with((desc.at[0], desc.at[1]), desc.width, desc.height, &list, desc.rows, &mut cell, label) };
    state.shared().put_strs(list);
    if bad_text { return unsafe { fail(ui, "invalid table cell callback text") }; }
    unsafe { *clicked_row = clicked.unwrap_or(-1); }
    0
});
