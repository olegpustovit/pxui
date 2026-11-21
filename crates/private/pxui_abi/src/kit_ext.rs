//! Kit and text widgets beyond the first set: styled buttons, grips, popups, inspector pieces and the line field.
use crate::interaction::write_signal;
use crate::kit::PxuiHintDesc;
use crate::state::{Build, MAX_TOTAL_TEXT_BYTES, PxuiUi, boxed, call_build, checked, fail, get, to_abi_box};
use crate::text_inputs::TextValid;
use crate::trusted;
use pxui_types::theme::BodyKind;
use crate::{PXUI_ERR_ARGUMENT, PxuiBox, PxuiSignal, Str};
use core::{cell::Cell, ffi::c_void};
use pxui_core::ui::{Along, Axis, Edges, HeadAction, Justify, LineStep, Rect, Size, State, States, Ui};
use pxui_types::{geom::Point, ffi::{PxuiButtonBuild, PxuiButtonDesc, PxuiIconButtonDesc, PxuiStateDesc, PxuiResponse, PxuiPickItem, PxuiSegmentedDesc, PxuiSegmentedResult, PxuiToggleDesc, PxuiCheckboxDesc, PxuiHeadActionDesc, PxuiHeadOut, PxuiHeaderDesc, PxuiPopupDesc, Slice}};
use std::rc::Rc;

const MAX_ACTIONS: usize = 256;

// ABI 1.56: the kit button of `desc`, reporting into one `PxuiResponse`; its content callback is given the button's interaction state this frame.
export!(trusted pub unsafe fn pxui_ui_button(ui: *mut PxuiUi, desc: *const PxuiButtonDesc, build: PxuiButtonBuild, user: *mut c_void, out: *mut PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if desc.is_null() { return unsafe { fail(ui, "null button argument") }; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short button response") } };
    let mut content = |engine: &mut pxui_core::ui::Ui, s: State| if let Some(build) = build {
        let state = PxuiStateDesc { size: core::mem::size_of::<PxuiStateDesc>() as u32, hovered: s.hovered as u8, pressed: s.pressed as u8, focused: s.focused as u8, disabled: s.disabled as u8, checked: s.checked as u8, reserved: [0; 3] };
        { let mut b = unsafe { crate::state::Borrowed::new(engine, ui) }; let h = unsafe { b.enter() }; build(user, h.cast(), &state) }
    };
    unsafe { button_call(ui, desc, if build.is_some() { Some(&mut content) } else { None }, out, None) }
});

// ABI 1.48: the plain kit button (`Button::DEFAULT`: the theme's look, the kit's size, at the start of its line) keyed by `key`.
export!(trusted pub unsafe fn pxui_ui_button_plain(ui: *mut PxuiUi, key: Str, text: Str, out: *mut PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short button response") } };
    let (Some(key), Some(text)) = (unsafe { crate::state::checked_key(key) }, unsafe { checked(text) }) else { return unsafe { fail(ui, "invalid button text") } };
    let mut r = trusted::EngineResponse::default();
    let rc = unsafe { trusted::button_plain(ui, key, text, &mut r) };
    if rc == 0 { trusted::respond(&r, out); }
    rc
});

/// The checked `PxuiButtonDesc` behind `desc` (not null) through the trusted entry: what happened to `out`, the whole signal to
/// `signal` when given.
unsafe fn button_call(ui: *mut PxuiUi, desc: *const PxuiButtonDesc, mut inner: Option<&mut dyn FnMut(&mut pxui_core::ui::Ui, State)>, out: &mut PxuiResponse, signal: Option<&mut PxuiSignal>) -> i32 {
    // a word set for this call goes to the leading icon's drawing; the next call reads 0
    let word = core::mem::take(&mut unsafe { &*ui }.shared().icon_word);
    // the whole record (its size, or more): no earlier size is read
    let Some(d) = read_desc!(desc, PxuiButtonDesc) else { return unsafe { fail(ui, "invalid button descriptor") }; };
    let justify = match d.justify { 0 => Some(Justify::Start), 1 => Some(Justify::Center), 2 => Some(Justify::End), _ => None };
    let (Some(w), Some(h), Some(justify)) = (size(d.w_kind, d.w), size(d.h_kind, d.h), justify) else { return unsafe { fail(ui, "invalid button descriptor") }; };
    if d.enabled > 1 || d.selected > 1 || d.danger > 1 || d.has_name > 1 || d.reserved != 0 || d.centered > 1 || d.reserved2 != 0 || d.reserved5 != [0; 4] { return unsafe { fail(ui, "invalid button descriptor") }; }
    let (Some(key), Some(text), Some(name)) = (unsafe { crate::state::checked_key(d.key) }, unsafe { checked(d.text) }, unsafe { checked(d.name) }) else { return unsafe { fail(ui, "invalid button text") }; };
    let Ok(looks) = (unsafe { looks(d.states) }) else { return unsafe { fail(ui, "invalid button looks") } };
    let axis = match d.axis { 0 => Some(None), 1 => Some(Some(Axis::Row)), 2 => Some(Some(Axis::Column)), 3 => Some(Some(Axis::Stack)), _ => None };
    let content_align = match d.content_align { 0 => Some(None), 1 => Some(Some(Along::Start)), 2 => Some(Some(Along::Center)), 3 => Some(Some(Along::End)), _ => None };
    let (Some(axis), Some(content_align)) = (axis, content_align) else { return unsafe { fail(ui, "invalid button descriptor") }; };
    if d.has_label > 1 || d.focus_on_click > 1 || d.has_detail_label > 1 || d.has_accessory_label > 1 || d.reserved3 != [0; 6] { return unsafe { fail(ui, "invalid button descriptor") }; }
    let (Some(label), Some(detail_label), Some(accessory_label)) = (label_of(d.has_label, &d.label, &d.label_box), label_of(d.has_detail_label, &d.detail_label, &d.detail_box),
        label_of(d.has_accessory_label, &d.accessory_label, &d.accessory_box)) else { return unsafe { fail(ui, "invalid button label") } };
    let (Some(lead), Some(detail), Some(accessory)) = (unsafe { checked(d.lead) }, unsafe { checked(d.detail) }, unsafe { checked(d.accessory) }) else { return unsafe { fail(ui, "invalid button text") }; };
    let pad = Edges { l: d.padding[0], t: d.padding[1], r: d.padding[2], b: d.padding[3] };
    // the variant reaches the button body only: the caller's contents are built with none
    let has_content = inner.is_some();
    let mut clear = |engine: &mut pxui_core::ui::Ui, s: State| { engine.set_variant(0, 0); if let Some(f) = inner.as_mut() { f(engine, s) } };
    with_variant!(ui, BodyKind::button, d.variant, unsafe { trusted::button_with(ui, key, text, (d.has_name != 0).then_some(name), looks.as_ref(), w, h, justify, pad, d.enabled != 0, d.selected != 0, d.danger != 0, d.centered != 0,
        label.as_ref(), axis, d.gap, content_align, d.focus_on_click != 0,
        lead, d.lead_draw, d.lead_user, word, d.icon_size, detail, accessory, detail_label.as_ref(), accessory_label.as_ref(), if has_content { Some(&mut clear) } else { None }, out, signal) })
}

/// The label style a button record carries in `style` and `label_box` when `has` is 1: `Some(None)` for none, `None` for one that is invalid.
fn label_of(has: u8, style: &pxui_types::ffi::PxuiLabelStyle, label_box: &pxui_types::ffi::PxuiLabelBox) -> Option<Option<pxui_types::text::LabelFields>> {
    if has != 1 { return Some(None); }
    let Some((mut fields, false)) = crate::ui::label_style(style) else { return None };
    crate::ui::read_label_box(label_box, &mut fields).then_some(Some(fields))
}

// ABI 1.48: the kit icon button of `desc`, reporting into one `PxuiResponse`.
export!(trusted pub unsafe fn pxui_ui_icon_button(ui: *mut PxuiUi, desc: *const PxuiIconButtonDesc, out: *mut PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if desc.is_null() { return unsafe { fail(ui, "null icon button argument") }; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short icon button response") } };
    unsafe { icon_button_call(ui, desc, out, None) }
});

// ABI 1.48: the plain kit icon button (`IconButton::DEFAULT`: the theme's look, the kit's square) showing the glyph `icon`, named `name`.
export!(trusted pub unsafe fn pxui_ui_icon_button_plain(ui: *mut PxuiUi, key: Str, icon: Str, name: Str, out: *mut PxuiResponse) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    // a word set for this call has no drawing to go to: the next call reads 0
    state.shared().icon_word = 0;
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short icon button response") } };
    let (Some(key), Some(icon), Some(name)) = (unsafe { crate::state::checked_key(key) }, unsafe { checked(icon) }, unsafe { checked(name) }) else { return unsafe { fail(ui, "invalid icon button text") } };
    let mut r = trusted::EngineResponse::default();
    let rc = unsafe { trusted::icon_button_plain(ui, key, icon, name, &mut r) };
    if rc == 0 { trusted::respond(&r, out); }
    rc
});

/// The checked `PxuiIconButtonDesc` behind `desc` (not null) through the trusted entry, as `button_call`.
unsafe fn icon_button_call(ui: *mut PxuiUi, desc: *const PxuiIconButtonDesc, out: &mut PxuiResponse, signal: Option<&mut PxuiSignal>) -> i32 {
    let word = core::mem::take(&mut unsafe { &*ui }.shared().icon_word);
    let Some(d) = read_desc!(desc, PxuiIconButtonDesc) else { return unsafe { fail(ui, "invalid icon button descriptor") }; };
    // null is the kit's springs
    let Some(press) = (unsafe { press_curves(d.press) }) else { return unsafe { fail(ui, "invalid icon button press curves") }; };
    let (Some(w), Some(h)) = (size(d.w_kind, d.w), size(d.h_kind, d.h)) else { return unsafe { fail(ui, "invalid icon button descriptor") }; };
    if d.enabled > 1 || d.reserved != 0 || d.checkable > 1 || d.checked > d.checkable || d.reserved2 != [0; 4] { return unsafe { fail(ui, "invalid icon button descriptor") }; }
    let (Some(key), Some(name), Some(icon)) = (unsafe { crate::state::checked_key(d.key) }, unsafe { checked(d.name) }, unsafe { checked(d.icon) }) else { return unsafe { fail(ui, "invalid icon button text") }; };
    let Ok(looks) = (unsafe { looks(d.states) }) else { return unsafe { fail(ui, "invalid icon button looks") } };
    with_variant!(ui, BodyKind::icon_button, d.variant, unsafe { trusted::icon_button_with(ui, key, icon, d.draw, d.draw_user, word, name, looks.as_ref(), w, h, d.enabled != 0, d.press_scale, press, (d.checkable == 1).then_some(d.checked == 1), out, signal) })
}

/// The two press curves behind `press` (null: `Some(None)`, the kit's), checked before they are read as `Curve`s: `None` for a pointer
/// that is not 4-aligned or a curve whose kind byte is past `Steps`. Their parameters are the trusted entry's to check (`Curve::valid`).
/// The reference is `'static` to the engine's types but the caller's record is only borrowed for the call: nothing keeps it after.
unsafe fn press_curves(press: *const pxui_types::Curve) -> Option<Option<&'static [pxui_types::Curve; 2]>> {
    if press.is_null() { return Some(None); }
    let stride = core::mem::size_of::<pxui_types::Curve>();
    if press as usize % core::mem::align_of::<pxui_types::Curve>() != 0 { return None; }
    // the tag is the first byte of a `repr(C, u8)` enum: 0 Bezier, 1 Spring, 2 Table, 3 Steps
    let tags = unsafe { [press.cast::<u8>().read(), press.cast::<u8>().add(stride).read()] };
    if tags.iter().any(|t| *t > 3) { return None; }
    Some(Some(unsafe { &*press.cast::<[pxui_types::Curve; 2]>() }))
}

/// A button size as a descriptor writes it (kind 0 the kit's, 1 points, 2 fill); the points are the trusted entry's to check.
fn size(kind: u8, value: f32) -> Option<Size> { match kind { 0 => Some(Size::Fit), 1 => Some(Size::Px(value)), 2 => Some(Size::Grow), _ => None } }

/// The six looks behind `states` (none when null); `Err` when one does not decode.
unsafe fn looks(states: *const pxui_types::ffi::PxuiLookDesc) -> Result<Option<States>, ()> {
    if states.is_null() { return Ok(None); }
    // the six are spaced by the first one's size (a caller's record may be longer than ours): every one carries that same size, and
    // none is read before the first one's size covers the record
    let first = unsafe { states.cast::<u32>().read_unaligned() } as usize;
    if first < core::mem::size_of::<pxui_types::ffi::PxuiLookDesc>() { return Err(()); }
    let at = |i: usize| -> Option<pxui_core::ui::Look> {
        let p = unsafe { states.cast::<u8>().add(i * first) }.cast::<pxui_types::ffi::PxuiLookDesc>();
        if unsafe { p.cast::<u32>().read_unaligned() } as usize != first { return None; }
        read_desc!(p, pxui_types::ffi::PxuiLookDesc).and_then(crate::style::decode_look)
    };
    let mut it = 0..6usize;
    let mut next = || it.next().and_then(at);
    match (next(), next(), next(), next(), next(), next()) {
        (Some(idle), Some(hover), Some(pressed), Some(focus), Some(disabled), Some(checked)) => Ok(Some(States { idle, hover, pressed, focus, disabled, checked })),
        _ => Err(()),
    }
}

// ABI 1.51: the plain kit toggle (`Toggle::DEFAULT`) keyed by `key`; a click is the flip of `on`, which the caller keeps.
export!(trusted pub unsafe fn pxui_ui_toggle_plain(ui: *mut PxuiUi, key: Str, text: Str, on: u8, out: *mut PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short toggle response") } };
    let (Some(key), Some(text)) = (unsafe { crate::state::checked_key(key) }, unsafe { checked(text) }) else { return unsafe { fail(ui, "invalid toggle text") } };
    if on > 1 { return unsafe { fail(ui, "invalid toggle value") }; }
    let mut r = trusted::EngineResponse::default();
    let rc = unsafe { trusted::toggle_plain(ui, key, text, on != 0, &mut r) };
    if rc == 0 { trusted::respond(&r, out); }
    rc
});

// ABI 1.51: the kit toggle with options, reporting into one `PxuiResponse`.
export!(trusted pub unsafe fn pxui_ui_toggle(ui: *mut PxuiUi, desc: *const PxuiToggleDesc, out: *mut PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if desc.is_null() { return unsafe { fail(ui, "null toggle argument") }; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short toggle response") } };
    let Some(d) = read_desc!(desc, PxuiToggleDesc) else { return unsafe { fail(ui, "invalid toggle descriptor") }; };
    if d.enabled > 1 || d.on > 1 || d.press_row > 1 || d.reserved != 0 || d.has_style > 1 || d.reserved2 != 0 || d.reserved3 != [0; 4] { return unsafe { fail(ui, "invalid toggle descriptor") }; }
    let (Some(key), Some(text), Some(detail)) = (unsafe { crate::state::checked_key(d.key) }, unsafe { checked(d.text) }, unsafe { checked(d.detail) }) else { return unsafe { fail(ui, "invalid toggle text") } };
    let (Ok(track), Ok(knob)) = (unsafe { looks(d.track) }, unsafe { looks(d.knob) }) else { return unsafe { fail(ui, "invalid toggle looks") } };
    let label = match d.has_style {
        0 => None,
        _ => match crate::ui::label_style(&d.style) { Some((label, false)) => Some(label), _ => return unsafe { fail(ui, "invalid toggle label style") } },
    };
    with_variant!(ui, BodyKind::toggle, d.variant, unsafe { trusted::toggle_with(ui, key, text, d.on != 0, track.as_ref(), knob.as_ref(), d.row_h, d.switch_w, d.switch_h, d.inset, d.press_row != 0, detail, label.as_ref(), d.enabled != 0, out) })
});

// ABI 1.85: the kit checkbox with its style (`PxuiCheckboxDesc`: size, looks, disabled, passive), reporting into one `PxuiResponse`; a
// click flips `on`, which the caller keeps. Every field is checked before the build.
export!(trusted pub unsafe fn pxui_ui_checkbox(ui: *mut PxuiUi, desc: *const PxuiCheckboxDesc, out: *mut PxuiResponse) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if desc.is_null() { return unsafe { fail(ui, "null checkbox argument") }; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short checkbox response") } };
    let Some(d) = read_desc!(desc, PxuiCheckboxDesc) else { return unsafe { fail(ui, "invalid checkbox descriptor") }; };
    if d.disabled > 1 || d.on > 1 || d.passive > 1 || d.reserved != 0 || d.reserved_variant != [0; 6] || !(0.0..=f32::MAX).contains(&d.box_size) { return unsafe { fail(ui, "invalid checkbox descriptor") }; }
    let (Some(key), Some(text)) = (unsafe { crate::state::checked_key(d.key) }, unsafe { checked(d.text) }) else { return unsafe { fail(ui, "invalid checkbox text") } };
    let Ok(look) = (unsafe { looks(d.look) }) else { return unsafe { fail(ui, "invalid checkbox looks") } };
    let style = pxui_types::ui::Checkbox { variant: d.variant, size: (d.box_size > 0.0).then_some(d.box_size), look: look.as_ref(), disabled: d.disabled != 0, passive: d.passive != 0 };
    let mut r = trusted::EngineResponse::default();
    let rc = unsafe { trusted::checkbox(ui, key, text, d.on != 0, &style, &mut r) };
    if rc == 0 { trusted::respond(&r, out); }
    rc
});

// The kit segmented control: its options are pick-one `items` (each with its id, name and detail; `desc.names` empty), an item with an id
// keyed by it; reports one `PxuiResponse` and a `PxuiSegmentedResult` (the choice made this frame, the option a secondary press began on).
export!(trusted pub unsafe fn pxui_ui_segmented(ui: *mut PxuiUi, desc: *const PxuiSegmentedDesc, items: Slice<PxuiPickItem>, out: *mut PxuiResponse, result: *mut PxuiSegmentedResult) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(result) = sized_mut!(result) else { return unsafe { fail(ui, "null or short segmented result") } };
    let (mut at, mut secondary) = (None, None);
    let rc = unsafe { segmented_desc(ui, desc, items, out, &mut at, &mut secondary) };
    if rc == 0 {
        (result.has_chosen, result.chosen, result.has_secondary, result.secondary) = (at.is_some() as u8, at.unwrap_or(0) as u64, secondary.is_some() as u8, secondary.unwrap_or(0) as u64);
        result.reserved = [0; 2];
    }
    rc
});

// The body of `pxui_ui_segmented`: the whole record checked, then the items checked once and read where they lie.
unsafe fn segmented_desc(ui: *mut PxuiUi, desc: *const PxuiSegmentedDesc, items: Slice<PxuiPickItem>, out: *mut PxuiResponse, chosen: &mut Option<usize>, secondary: &mut Option<usize>) -> i32 {
    if desc.is_null() { return unsafe { fail(ui, "null segmented argument") }; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short segmented response") } };
    let Some(d) = read_desc!(desc, PxuiSegmentedDesc) else { return unsafe { fail(ui, "invalid segmented descriptor") }; };
    if d.enabled > 1 || d.grow > 1 || d.reserved != [0; 2] || d.reserved2 != 0 || d.names.len != 0 || d.has_selected > 1 || d.reserved3 != [0; 7] || d.focus_on_click > 1 || d.reserved4 != [0; 7] { return unsafe { fail(ui, "invalid segmented descriptor") }; }
    let Ok(states) = (unsafe { looks(d.states) }) else { return unsafe { fail(ui, "invalid segmented looks") } };
    let selected = (d.has_selected == 1).then_some(d.selected);
    let (Some(key), Some(name)) = (unsafe { crate::state::checked_key(d.key) }, unsafe { checked(d.name) }) else { return unsafe { fail(ui, "invalid segmented strings") } };
    let Some(items) = (unsafe { checked_items(items) }) else { return unsafe { fail(ui, "invalid segmented items") } };
    with_variant!(ui, BodyKind::segmented, d.variant, unsafe { trusted::segmented_checked(ui, key, name, items.len(), &|i| segment_of(&items[i]), selected, d.enabled != 0, d.h, d.grow != 0, states.as_ref(), d.w, d.padding, d.min_segment, d.focus_on_click != 0, out, chosen, secondary) })
}

/// `items` checked once, in one pass before the call (at most `MAX_OPTIONS`, `has_id` 0 or 1 with `reserved` zero, every string UTF-8
/// and all of them within `MAX_TOTAL_TEXT_BYTES`), as a slice the engine then reads in place: None refuses them.
unsafe fn checked_items<'a>(items: Slice<PxuiPickItem>) -> Option<&'a [PxuiPickItem]> {
    if items.len > trusted::MAX_OPTIONS || (items.len != 0 && items.ptr.is_null()) { return None; }
    let items = if items.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(items.ptr, items.len) } };
    let mut total = 0usize;
    for item in items {
        total = total.saturating_add(item.text.len).saturating_add(item.name.len).saturating_add(item.detail.len);
        let strings = unsafe { checked(item.text).is_some() && checked(item.name).is_some() && checked(item.detail).is_some() };
        if !strings || item.has_id > 1 || item.reserved != [0; 7] || total > MAX_TOTAL_TEXT_BYTES { return None; }
    }
    Some(items)
}

/// Item `i` of items `checked_items` passed, as the engine's segment (its strings were checked: read as they are).
fn segment_of(item: &PxuiPickItem) -> pxui_types::ui::Segment<'_> {
    let text = |s: Str| if s.len == 0 { "" } else { unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(s.ptr, s.len)) } };
    let (name, detail) = (text(item.name), text(item.detail));
    pxui_types::ui::Segment { text: text(item.text), id: (item.has_id == 1).then_some(item.id), detail, name: (!name.is_empty()).then_some(name) }
}

/// The body of `pxui_ui_pick` and `pxui_ui_tabs`: every argument checked once (`kind` is the caller's to check), then one engine
/// call that reads the items where they lie.
#[allow(clippy::too_many_arguments)]
unsafe fn pick_export(ui: *mut PxuiUi, keyed: *const pxui_types::ffi::PxuiKeyedDesc, kind: u32, items: Slice<PxuiPickItem>, has_selected: u8, selected: usize, focus: u8, out: *mut PxuiResponse, result: *mut PxuiSegmentedResult) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "null or short pick response") } };
    let Some(result) = sized_mut!(result) else { return unsafe { fail(ui, "null or short pick result") } };
    let Some((key, name)) = (unsafe { crate::widgets::read_keyed(keyed) }).filter(|(key, _)| !crate::state::unusable(key)) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    if has_selected > 1 || focus > 1 { return unsafe { fail(ui, "invalid pick arguments") }; }
    let Some(items) = (unsafe { checked_items(items) }) else { return unsafe { fail(ui, "invalid pick items") } };
    let (mut at, mut secondary) = (None, None);
    let body = match kind { 0 => BodyKind::radio, 1 => BodyKind::tabs, _ => BodyKind::dropdown };
    let rc = with_variant!(ui, body, unsafe { (*keyed).variant }, unsafe { trusted::pick_checked(ui, kind, key, name, items.len(), &|i| segment_of(&items[i]), (has_selected == 1).then_some(selected), focus == 1, out, &mut at, &mut secondary) });
    if rc == 0 {
        (result.has_chosen, result.chosen, result.has_secondary, result.secondary) = (at.is_some() as u8, at.unwrap_or(0) as u64, secondary.is_some() as u8, secondary.unwrap_or(0) as u64);
        result.reserved = [0; 2];
    }
    rc
}

// ABI 1.76: a radio group (`kind` 0) or a dropdown (3) of pick-one `items` keyed by `keyed.key` and named `keyed.name`; `selected` is
// read when `has_selected` is 1. Reports one `PxuiResponse` and a `PxuiSegmentedResult` (the choice, none for none, and the item a
// secondary press began on).
export!(trusted pub unsafe fn pxui_ui_pick(ui: *mut PxuiUi, keyed: *const pxui_types::ffi::PxuiKeyedDesc, kind: u32, items: Slice<PxuiPickItem>, has_selected: u8, selected: usize, out: *mut PxuiResponse, result: *mut PxuiSegmentedResult) -> i32 {
    if !matches!(kind, 0 | 3) { return if unsafe { get(ui) }.is_none() { PXUI_ERR_ARGUMENT } else { unsafe { fail(ui, "invalid pick arguments") } }; }
    unsafe { pick_export(ui, keyed, kind, items, has_selected, selected, 0, out, result) }
});

// ABI 1.84: a tab bar of pick-one `items` (a tab's `detail` beside its text, its id keying it) keyed by `keyed.key` and named
// `keyed.name`; `selected` is the tab in use when `has_selected` is 1 (0: none is); `focus_on_click` 1 makes a press on a tab focus the
// bar. Reports one `PxuiResponse` and a `PxuiSegmentedResult` (the tab in use, none for none, and the tab a secondary press began on).
export!(trusted pub unsafe fn pxui_ui_tabs(ui: *mut PxuiUi, keyed: *const pxui_types::ffi::PxuiKeyedDesc, items: Slice<PxuiPickItem>, has_selected: u8, selected: usize, focus_on_click: u8, out: *mut PxuiResponse, result: *mut PxuiSegmentedResult) -> i32 {
    unsafe { pick_export(ui, keyed, 1, items, has_selected, selected, focus_on_click, out, result) }
});

// Builds a subtree with every interactive node in it disabled when `disabled` is 1 (scopes nest by OR); the Ui is back as it was when the call returns.
export!(build pub unsafe fn pxui_ui_disabled(ui: *mut PxuiUi, disabled: u8, build: Build, user: *mut c_void) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if disabled > 1 { return unsafe { fail(ui, "invalid disabled flag") }; }
    let shared = state.shared;
    unsafe { (&mut *state.engine).disabled(disabled != 0, &mut |engine: &mut Ui| call_build(shared, engine, build, user)); }
    0
});

// `axis` 0 is a row (a vertical grip), 1 a column; `rect` places the grip in its parent when `has_rect` is set.
export!(build pub unsafe fn pxui_ui_grip(ui: *mut PxuiUi, label: Str, axis: u32, has_rect: u8, rect: Rect, out: *mut PxuiBox, signal: *mut PxuiSignal) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let axis = match axis { 0 => Axis::Row, 1 => Axis::Column, _ => return unsafe { fail(ui, "unknown grip axis") } };
    let Some(label) = (unsafe { crate::state::checked_key(label) }).filter(|_| has_rect <= 1 && !out.is_null() && !signal.is_null() && crate::state::finite(&[rect.x, rect.y, rect.w, rect.h])) else { return unsafe { fail(ui, "invalid grip arguments") }; };
    let (handle, result) = unsafe { (&mut *state.engine).grip(format_args!("{label}"), axis, (has_rect != 0).then_some(rect)) };
    unsafe { *out = to_abi_box(handle); write_signal(state, &result, &mut *signal); }
    0
});

/// The popup of `desc` keyed by `key` and named `name`, the body of `pxui_ui_popup`: `out` and `root` are written only when it builds.
unsafe fn popup(ui: *mut PxuiUi, key: &str, name: &str, d: &PxuiPopupDesc, build: Build, user: *mut c_void, out: &mut u8, root: &mut PxuiBox) -> i32 {
    if d.has_stroke > 1 || d.has_max_height > 1 || d.dismiss_initial > 1 || d.attached > 1
        || !crate::state::finite(&[d.x, d.y, d.width, d.height]) || (d.has_max_height != 0 && !d.max_height.is_finite()) || d.width < 0.0 || d.height < 0.0 {
        return unsafe { fail(ui, "invalid popup descriptor") };
    }
    let overlay = pxui_types::ui::Overlay { at: (d.x, d.y), w: Some(d.width), h: Some(d.height), max_h: (d.has_max_height != 0).then_some(d.max_height),
        stroke: (d.has_stroke != 0).then_some(d.stroke), dismiss_initial: d.dismiss_initial != 0, attached: d.attached != 0, name: Some(name), ..pxui_types::ui::Overlay::DEFAULT };
    let mut open = false;
    let mut node = PxuiBox::default();
    let rc = unsafe { crate::trusted::popup(ui, key, &overlay, &mut |engine: &mut pxui_core::ui::Ui| crate::state::call_build((*ui).shared, engine, build, user), &mut open, &mut node) };
    if rc == 0 { (*out, *root) = (open as u8, node); }
    rc
}

// ABI 1.76: a popup keyed by `keyed.key`, named `keyed.name`, placed and dressed by `desc`; writes `out` (1 while open) and the popup's
// root to `root` (the empty box when it is dismissed). A refused call writes neither.
export!(build pub unsafe fn pxui_ui_popup(ui: *mut PxuiUi, keyed: *const pxui_types::ffi::PxuiKeyedDesc, desc: *const PxuiPopupDesc, build: Build, user: *mut c_void, out: *mut u8, root: *mut PxuiBox) -> i32 {
    if unsafe { get(ui) }.is_none() { return PXUI_ERR_ARGUMENT; }
    if desc.is_null() || out.is_null() || root.is_null() { return unsafe { fail(ui, "null popup argument") }; }
    let Some((key, name)) = (unsafe { crate::widgets::read_keyed(keyed) }) else { return unsafe { fail(ui, "invalid keyed descriptor") } };
    let Some(d) = read_desc!(desc, PxuiPopupDesc) else { return unsafe { fail(ui, "invalid popup descriptor") }; };
    if d.label.len != 0 { return unsafe { fail(ui, "a keyed popup takes no label") }; }
    unsafe { popup(ui, key, name, &d, build, user, &mut *out, &mut *root) }
});

export!(build pub unsafe fn pxui_ui_inspector_bar(ui: *mut PxuiUi, hints: Slice<PxuiHintDesc>, count: i64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if hints.len > 4096 || (hints.len != 0 && hints.ptr.is_null()) { return unsafe { fail(ui, "invalid inspector bar hints") }; }
    let raw = if hints.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(hints.ptr, hints.len) } };
    if !raw.iter().all(|hint| hint.size as usize == core::mem::size_of_val(hint)) { return unsafe { fail(ui, "invalid inspector bar hints") }; }
    // the rows borrow the caller's strings for this call, in a vector reused between frames, like the footer's: a settled bar allocates nothing
    let mut rows = core::mem::take(&mut state.shared().hints);
    let mut total = 0usize;
    for hint in raw {
        let (Some(key), Some(value)) = (unsafe { checked(hint.key) }, unsafe { checked(hint.value) }) else { rows.clear(); state.shared().hints = rows; return unsafe { fail(ui, "invalid inspector bar hint") }; };
        total += key.len() + value.len();
        if total > MAX_TOTAL_TEXT_BYTES { rows.clear(); state.shared().hints = rows; return unsafe { fail(ui, "inspector bar text limit exceeded") }; }
        rows.push((key, value));
    }
    unsafe { (&mut *state.engine).inspector_bar(&rows, count); }
    rows.clear();
    state.shared().hints = rows;
    0
});

export!(build pub unsafe fn pxui_ui_inspector_body(ui: *mut PxuiUi, build: Build, user: *mut c_void) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let shared = state.shared;
    unsafe { (&mut *state.engine).inspector_body(&mut |engine: &mut Ui| call_build(shared, engine, build, user)); }
    0
});

// `rename` (when set) says whether a typed name is valid; it is called only during this call.
export!(build pub unsafe fn pxui_ui_inspector_header(ui: *mut PxuiUi, desc: *const PxuiHeaderDesc, rename: TextValid, user: *mut c_void, out: *mut PxuiHeadOut) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(out) = sized_mut!(out) else { return unsafe { fail(ui, "invalid inspector header output") }; };
    if desc.is_null() { return unsafe { fail(ui, "null inspector header argument") }; }
    let Some(d) = read_desc!(desc, PxuiHeaderDesc) else { return unsafe { fail(ui, "invalid inspector header descriptor") }; };
    if [d.has_kind, d.has_badge, d.has_index, d.has_reset, d.reset_enabled].iter().any(|v| *v > 1)
        || d.actions.len > MAX_ACTIONS || (d.actions.len != 0 && d.actions.ptr.is_null()) {
        return unsafe { fail(ui, "invalid inspector header descriptor") };
    }
    let optional = |flag: u8, value: Str| -> Result<Option<&'static str>, ()> { if flag == 0 { Ok(None) } else { unsafe { checked(value) }.map(Some).ok_or(()) } };
    let (Some(key), Some(title), Some(detail), Ok(kind), Ok(badge), Ok(index), Ok(reset)) = (
        unsafe { crate::state::checked_key(d.key) }, unsafe { checked(d.title) }, unsafe { checked(d.detail) },
        optional(d.has_kind, d.kind), optional(d.has_badge, d.badge), optional(d.has_index, d.index), optional(d.has_reset, d.reset),
    ) else { return unsafe { fail(ui, "invalid inspector header text") }; };
    let raw: &[PxuiHeadActionDesc] = if d.actions.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(d.actions.ptr, d.actions.len) } };
    if !raw.iter().all(|a| a.size as usize == core::mem::size_of_val(a)) { return unsafe { fail(ui, "invalid inspector header action") }; }
    // the strings are reused between frames, like the hints': a settled head allocates nothing
    let mut actions = core::mem::take(&mut state.shared().actions);
    actions.truncate(raw.len());
    for (at, a) in raw.iter().enumerate() {
        let (Some(caption), Some(keycap)) = (unsafe { checked(a.caption) }, unsafe { checked(a.keycap) }) else { state.shared().actions = actions; return unsafe { fail(ui, "invalid inspector header action") }; };
        if a.active > 1 || a.usable > 1 { state.shared().actions = actions; return unsafe { fail(ui, "invalid inspector header action flag") }; }
        match actions.get_mut(at) {
            Some(row) => { row.caption.clear(); row.caption.push_str(caption); row.keycap.clear(); row.keycap.push_str(keycap); row.active = a.active != 0; row.usable = a.usable != 0; }
            None => actions.push(HeadAction { caption: caption.to_owned(), keycap: keycap.to_owned(), active: a.active != 0, usable: a.usable != 0 }),
        }
    }
    let valid = rename.map(|rename| {
        let shared = state.shared();
        let (cell, valid) = shared.rename.get_or_insert_with(|| {
            let cell = Rc::new(Cell::new((None, 0usize)));
            let read = Rc::clone(&cell);
            (cell, Rc::new(move |name: &str| { let (rename, user) = read.get(); rename.is_some_and(|rename| rename(user as *mut c_void, Str { ptr: name.as_ptr(), len: name.len() }) != 0) }))
        });
        cell.set((Some(rename), user as usize));
        Rc::clone(valid)
    });
    let head = unsafe { (&mut *state.engine).inspector_header(key, kind, badge, index, valid, &actions, reset, d.reset_enabled != 0, title, detail) };
    state.shared().actions = actions;
    let Ok(renamed) = state.give(head.renamed) else { return crate::PXUI_ERR_STATE };
    (out.renamed, out.chosen, out.has_chosen, out.reset_pressed) = (renamed, head.chosen.unwrap_or(0), head.chosen.is_some() as u8, head.reset_pressed as u8);
    0
});

// ABI 1.82: a line field at `x`, `y` (`w` by `h`) named `name` to a screen reader: `text` takes its text, `step` where a key moved the
// focus (0 none, 1 next, 2 previous, 3 close) and `response` what happened to its box. A refused call writes none of them.
export!(build pub unsafe fn pxui_ui_line_field(ui: *mut PxuiUi, x: f32, y: f32, w: f32, h: f32, grab: u8, placeholder: Str, label: Str, value: Str, name: Str, text: *mut Str, step: *mut u32, response: *mut PxuiResponse) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    let Some(response) = sized_mut!(response) else { return unsafe { fail(ui, "null or short line field response") } };
    let Some(name) = (unsafe { checked(name) }) else { return unsafe { fail(ui, "invalid line field text") }; };
    let (Some(placeholder), Some(label), Some(value)) = (unsafe { checked(placeholder) }, unsafe { crate::state::checked_label(label) }, unsafe { crate::state::checked_text(value) }) else { return unsafe { fail(ui, "invalid line field text") }; };
    if text.is_null() || step.is_null() || grab > 1 || !crate::state::finite(&[x, y, w, h]) { return unsafe { fail(ui, "invalid line field arguments") }; }
    let (edited, next) = unsafe { (&mut *state.engine).line_field((x, y), w, h, grab != 0, placeholder, label, value) };
    let Ok(edited) = state.give(edited) else { return crate::PXUI_ERR_STATE };
    let rc = unsafe { trusted::input_response(ui, Some(name), response) };
    if rc == 0 {
        unsafe {
            *text = edited;
            *step = match next { LineStep::None => 0, LineStep::Next => 1, LineStep::Previous => 2, LineStep::Close => 3 };
        }
    }
    rc
});

export!(pub unsafe fn pxui_ui_field_drag(ui: *mut PxuiUi, box_: *const PxuiBox, present: *mut u8, out: *mut Point) -> i32 {
    let (e, b) = match unsafe { boxed(ui, box_) } { Ok(v) => v, Err(code) => return code };
    if present.is_null() || out.is_null() { return unsafe { fail(ui, "null field drag output") }; }
    let drag = e.field_drag(&b);
    unsafe { *present = drag.is_some() as u8; *out = drag.map_or(Point::default(), |(x, y)| Point { x, y }); }
    0
});
