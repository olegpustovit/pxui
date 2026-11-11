use crate::{composite::defer_callback_panic, Ui};
use core::{ffi::c_void, mem::ManuallyDrop, panic::AssertUnwindSafe, ptr::NonNull};
use pxui_types::{Axis, Curve, Look, PxuiBox, Str, ffi::{PxuiCurveDesc, PxuiFontDesc, PxuiLookDesc, PxuiRingDesc, PxuiShadowDesc, Slice}};

#[cfg(feature = "static")]
use pxui_abi_link::{PxuiFooterDesc as FooterDesc, PxuiGridListDesc as GridListDesc, PxuiHintDesc as HintDesc, PxuiInspectorRowDesc as InspectorRowDesc, PxuiInspectorSectionDesc as InspectorSectionDesc, PxuiInspectorTextDesc as InspectorTextDesc, PxuiInspectorValueDesc as InspectorValueDesc, PxuiKeyframeDesc as KeyframeDesc, PxuiMenuBarDesc as MenuBarDesc, PxuiMenuDesc as MenuDesc, PxuiMenuOut as MenuOut, PxuiSplitDesc as SplitDesc, PxuiTableDesc as TableDesc};

#[cfg(not(feature = "static"))]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MenuDesc { title: Str, items: Slice<Str> }
#[cfg(not(feature = "static"))]
#[repr(C)]
pub struct GridListDesc { label: Str, count: usize, cell_w: f32, cell_h: f32, build: Option<extern "C" fn(*mut c_void, *mut sys::PxuiUi, usize)>, user: *mut c_void }
#[cfg(not(feature = "static"))]
#[repr(C)]
pub struct SplitDesc { label: Str, axis: u32, ratio: f32, min_a: f32, min_b: f32, build_a: Option<extern "C" fn(*mut c_void, *mut sys::PxuiUi)>, user_a: *mut c_void, build_b: Option<extern "C" fn(*mut c_void, *mut sys::PxuiUi)>, user_b: *mut c_void }
#[cfg(not(feature = "static"))]
#[repr(C)]
pub struct MenuBarDesc { label: Str, menus: Slice<MenuDesc> }
#[cfg(not(feature = "static"))]
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MenuOut { pub menu: usize, pub item: usize }
#[cfg(not(feature = "static"))]
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct HintDesc { key: Str, value: Str }
#[cfg(not(feature = "static"))]
#[repr(C)]
pub struct FooterDesc { has_right: u8, reserved: [u8; 7], right: Str, hints: Slice<HintDesc> }
#[cfg(not(feature = "static"))]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KeyframeDesc { time: f32, target: u32, reserved: [u8; 4], values: [f32; 4], look: PxuiLookDesc, curve: PxuiCurveDesc }
#[cfg(not(feature = "static"))]
#[repr(C)]
pub struct InspectorRowDesc { key: Str, label: Str, has_width: u8, has_pin: u8, pin: u8, reserved: [u8; 5], width: f32, row: PxuiBox, control_x: f32, control_y: f32, control_w: f32 }
#[cfg(not(feature = "static"))]
#[repr(C)]
pub struct InspectorSectionDesc { key: Str, label: Str, expanded: u8, has_set: u8, set_expanded: u8, reserved: [u8; 5], build: Option<extern "C" fn(*mut c_void, *mut sys::PxuiUi)>, user: *mut c_void, out_open: u8 }
#[cfg(not(feature = "static"))]
#[repr(C)]
pub struct InspectorValueDesc { key: Str, label: Str, value: Str, has_width: u8, reserved: [u8; 7], width: f32 }
#[cfg(not(feature = "static"))]
#[repr(C)]
pub struct InspectorTextDesc { key: Str, text: Str }
#[cfg(not(feature = "static"))]
#[repr(C)]
pub struct TableDesc { label: Str, at: [f32; 2], width: f32, height: f32, headers: Slice<Str>, rows: i64, cell: Option<extern "C" fn(*mut c_void, i64, i64) -> Str>, user: *mut c_void }

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{PxuiUi, pxui_ui_animate_keys, pxui_ui_footer, pxui_ui_grid_list, pxui_ui_inspector_button, pxui_ui_inspector_message, pxui_ui_inspector_readout, pxui_ui_inspector_row, pxui_ui_inspector_section, pxui_ui_inspector_toggle, pxui_ui_menu_bar, pxui_ui_split, pxui_ui_table, pxui_ui_wants_frame};
}
#[cfg(not(feature = "static"))]
mod sys {
    use super::{FooterDesc, GridListDesc, InspectorRowDesc, InspectorSectionDesc, InspectorTextDesc, InspectorValueDesc, KeyframeDesc, MenuBarDesc, MenuOut, SplitDesc, TableDesc};
    pub use crate::sys::PxuiUi;
    use pxui_types::{PxuiBox, Str, ffi::Slice};
    #[link(name = "pxui")]
    unsafe extern "C" {
        pub fn pxui_ui_grid_list(ui: *mut PxuiUi, desc: *const GridListDesc, out: *mut PxuiBox) -> i32;
        pub fn pxui_ui_split(ui: *mut PxuiUi, desc: *const SplitDesc, changed: *mut u8, ratio: *mut f32) -> i32;
        pub fn pxui_ui_menu_bar(ui: *mut PxuiUi, desc: *const MenuBarDesc, out: *mut MenuOut) -> i32;
        pub fn pxui_ui_inspector_row(ui: *mut PxuiUi, desc: *mut InspectorRowDesc) -> i32;
        pub fn pxui_ui_inspector_section(ui: *mut PxuiUi, desc: *mut InspectorSectionDesc) -> i32;
        pub fn pxui_ui_inspector_toggle(ui: *mut PxuiUi, key: Str, label: Str, value: u8, out: *mut u8) -> i32;
        pub fn pxui_ui_inspector_button(ui: *mut PxuiUi, key: Str, label: Str, out: *mut u8) -> i32;
        pub fn pxui_ui_inspector_readout(ui: *mut PxuiUi, desc: *const InspectorValueDesc) -> i32;
        pub fn pxui_ui_inspector_message(ui: *mut PxuiUi, desc: *const InspectorTextDesc) -> i32;
        pub fn pxui_ui_wants_frame(ui: *mut PxuiUi, events: Slice<pxui_types::Event>, now: f64, out: *mut u8) -> i32;
        pub fn pxui_ui_footer(ui: *mut PxuiUi, desc: *const FooterDesc) -> i32;
        pub fn pxui_ui_animate_keys(ui: *mut PxuiUi, box_: *const PxuiBox, keys: Slice<KeyframeDesc>, out: *mut u8) -> i32;
        pub fn pxui_ui_table(ui: *mut PxuiUi, desc: *const TableDesc, clicked_row: *mut i64) -> i32;
    }
}

fn as_str(value: &str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }
fn status(rc: i32) -> Result<(), i32> { if rc == 0 { Ok(()) } else { Err(rc) } }

struct Callback<F> { f: F, panicked: bool }
extern "C" fn call<F: FnMut(&mut Ui)>(user: *mut c_void, raw: *mut sys::PxuiUi) {
    if raw.is_null() { return; }
    let state = unsafe { &mut *user.cast::<Callback<F>>() };
    if state.panicked { return; }
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut ui = ManuallyDrop::new(Ui(unsafe { NonNull::new_unchecked(raw) }));
        (state.f)(&mut ui);
    }));
    if let Err(panic) = result { state.panicked = true; defer_callback_panic(panic); }
}
extern "C" fn call_grid<F: FnMut(&mut Ui, usize)>(user: *mut c_void, raw: *mut sys::PxuiUi, index: usize) {
    if raw.is_null() { return; }
    let state = unsafe { &mut *user.cast::<Callback<F>>() };
    if state.panicked { return; }
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut ui = ManuallyDrop::new(Ui(unsafe { NonNull::new_unchecked(raw) }));
        (state.f)(&mut ui, index);
    }));
    if let Err(panic) = result { state.panicked = true; defer_callback_panic(panic); }
}
struct CellText<F> { f: F, value: String, panicked: bool }
extern "C" fn call_cell<F: FnMut(i64, i64) -> String>(user: *mut c_void, row: i64, column: i64) -> Str {
    let state = unsafe { &mut *user.cast::<CellText<F>>() };
    if state.panicked { return Str::default(); }
    match std::panic::catch_unwind(AssertUnwindSafe(|| (state.f)(row, column))) {
        Ok(value) => { state.value = value; Str { ptr: state.value.as_ptr(), len: state.value.len() } }
        Err(panic) => { state.panicked = true; defer_callback_panic(panic); Str::default() }
    }
}

fn encode_look(value: Look) -> PxuiLookDesc {
    let shadow = |s: pxui_types::Shadow| PxuiShadowDesc { color: s.color, offset: [s.offset.0, s.offset.1], blur: s.blur, spread: s.spread, inner: s.inner as u8, reserved: [0; 3] };
    PxuiLookDesc {
        fill: value.fill, gradient: value.gradient.unwrap_or_default(), has_gradient: value.gradient.is_some() as u8, reserved: [0; 3], angle: value.angle,
        border: value.border, border_w: value.border_w, sides: value.sides as u32, radius: value.radius,
        shadows: [shadow(value.shadows[0]), shadow(value.shadows[1])], text: value.text,
        font: PxuiFontDesc { family: value.font.family, weight: value.font.weight, size: value.font.size, tracking: value.font.tracking, caps: value.font.caps as u8, reserved: [0; 3] },
        opacity: value.opacity, ring: PxuiRingDesc { color: value.ring.color, width: value.ring.width, sides: value.ring.sides as u32 },
        selection: value.selection, caret: value.caret, dashed: value.dashed as u8, reserved_tail: [0; 3],
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimationTarget { Offset(f32, f32), Opacity(f32), Look(Look) }
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe { pub time: f32, pub target: AnimationTarget, pub curve: Curve }
fn encode_curve(value: Curve) -> PxuiCurveDesc {
    match value {
        Curve::Bezier { duration, points } => PxuiCurveDesc { kind: PxuiCurveDesc::BEZIER, duration, points, omega: 0.0 },
        Curve::Spring { omega } => PxuiCurveDesc { kind: PxuiCurveDesc::SPRING, duration: 0.0, points: [0.0; 4], omega },
    }
}

impl Ui {
    pub fn grid_list<F: FnMut(&mut Ui, usize)>(&mut self, label: &str, count: usize, cell_w: f32, cell_h: f32, mut build: F) -> Result<PxuiBox, i32> {
        let mut callback = Callback { f: &mut build, panicked: false };
        let desc = GridListDesc { label: as_str(label), count, cell_w, cell_h, build: Some(call_grid::<F>), user: &mut callback as *mut _ as *mut c_void };
        let mut out = PxuiBox::default();
        status(unsafe { sys::pxui_ui_grid_list(self.0.as_ptr(), &desc, &mut out) })?;
        Ok(out)
    }

    pub fn split<F: FnMut(&mut Ui), G: FnMut(&mut Ui)>(&mut self, label: &str, axis: Axis, ratio: f32, min_a: f32, min_b: f32, mut a: F, mut b: G) -> Result<Option<f32>, i32> {
        let (mut a, mut b) = (Callback { f: &mut a, panicked: false }, Callback { f: &mut b, panicked: false });
        let desc = SplitDesc { label: as_str(label), axis: match axis { Axis::Row => 0, Axis::Column => 1, Axis::Stack => return Err(-1) }, ratio, min_a, min_b, build_a: Some(call::<F>), user_a: &mut a as *mut _ as *mut c_void, build_b: Some(call::<G>), user_b: &mut b as *mut _ as *mut c_void };
        let (mut changed, mut out) = (0u8, 0.0f32);
        status(unsafe { sys::pxui_ui_split(self.0.as_ptr(), &desc, &mut changed, &mut out) })?;
        Ok((changed != 0).then_some(out))
    }

    pub fn menu_bar(&mut self, label: &str, menus: &[(&str, &[&str])]) -> Result<Option<(usize, usize)>, i32> {
        let mut item_strings: Vec<Vec<Str>> = menus.iter().map(|(_, items)| items.iter().map(|s| as_str(s)).collect()).collect();
        let descriptors: Vec<MenuDesc> = menus.iter().zip(item_strings.iter_mut()).map(|((title, _), items)| MenuDesc { title: as_str(title), items: Slice { ptr: items.as_mut_ptr(), len: items.len() } }).collect();
        let desc = MenuBarDesc { label: as_str(label), menus: Slice { ptr: descriptors.as_ptr(), len: descriptors.len() } };
        let mut out = MenuOut { menu: usize::MAX, item: usize::MAX };
        status(unsafe { sys::pxui_ui_menu_bar(self.0.as_ptr(), &desc, &mut out) })?;
        Ok((out.menu != usize::MAX && out.item != usize::MAX).then_some((out.menu, out.item)))
    }

    pub fn inspector_row(&mut self, width: Option<f32>, pin: Option<bool>, key: &str, label: &str) -> Result<(PxuiBox, f32, f32, f32), i32> {
        let mut desc = InspectorRowDesc { key: as_str(key), label: as_str(label), has_width: width.is_some() as u8, has_pin: pin.is_some() as u8, pin: pin.unwrap_or(false) as u8, reserved: [0; 5], width: width.unwrap_or_default(), row: PxuiBox::default(), control_x: 0.0, control_y: 0.0, control_w: 0.0 };
        status(unsafe { sys::pxui_ui_inspector_row(self.0.as_ptr(), &mut desc) })?;
        Ok((desc.row, desc.control_x, desc.control_y, desc.control_w))
    }

    pub fn inspector_section<F: FnMut(&mut Ui)>(&mut self, key: &str, expanded: bool, set_expanded: Option<bool>, label: &str, mut build: F) -> Result<bool, i32> {
        let mut callback = Callback { f: &mut build, panicked: false };
        let mut desc = InspectorSectionDesc { key: as_str(key), label: as_str(label), expanded: expanded as u8, has_set: set_expanded.is_some() as u8, set_expanded: set_expanded.unwrap_or(false) as u8, reserved: [0; 5], build: Some(call::<F>), user: &mut callback as *mut _ as *mut c_void, out_open: 0 };
        status(unsafe { sys::pxui_ui_inspector_section(self.0.as_ptr(), &mut desc) })?;
        Ok(desc.out_open != 0)
    }

    pub fn inspector_toggle(&mut self, key: &str, label: &str, value: bool) -> Result<bool, i32> {
        let mut out = 0;
        status(unsafe { sys::pxui_ui_inspector_toggle(self.0.as_ptr(), as_str(key), as_str(label), value as u8, &mut out) })?;
        Ok(out != 0)
    }
    pub fn inspector_button(&mut self, key: &str, label: &str) -> Result<bool, i32> {
        let mut out = 0;
        status(unsafe { sys::pxui_ui_inspector_button(self.0.as_ptr(), as_str(key), as_str(label), &mut out) })?;
        Ok(out != 0)
    }
    pub fn inspector_readout(&mut self, width: Option<f32>, key: &str, label: &str, value: &str) -> Result<(), i32> {
        let desc = InspectorValueDesc { key: as_str(key), label: as_str(label), value: as_str(value), has_width: width.is_some() as u8, reserved: [0; 7], width: width.unwrap_or_default() };
        status(unsafe { sys::pxui_ui_inspector_readout(self.0.as_ptr(), &desc) })
    }
    pub fn inspector_message(&mut self, key: &str, message: &str) -> Result<(), i32> {
        let desc = InspectorTextDesc { key: as_str(key), text: as_str(message) };
        status(unsafe { sys::pxui_ui_inspector_message(self.0.as_ptr(), &desc) })
    }

    pub fn wants_frame(&self, events: &[pxui_types::Event], now: f64) -> Result<bool, i32> {
        let mut out = 0;
        status(unsafe { sys::pxui_ui_wants_frame(self.0.as_ptr(), Slice { ptr: events.as_ptr(), len: events.len() }, now, &mut out) })?;
        Ok(out != 0)
    }
    pub fn footer(&mut self, right: Option<&str>, hints: &[(String, String)]) -> Result<(), i32> {
        let values: Vec<HintDesc> = hints.iter().map(|(key, value)| HintDesc { key: as_str(key), value: as_str(value) }).collect();
        let desc = FooterDesc { has_right: right.is_some() as u8, reserved: [0; 7], right: right.map_or_else(Str::default, as_str), hints: Slice { ptr: values.as_ptr(), len: values.len() } };
        status(unsafe { sys::pxui_ui_footer(self.0.as_ptr(), &desc) })
    }
    pub fn animate_keys(&mut self, box_: &PxuiBox, keys: &[Keyframe]) -> Result<bool, i32> {
        let descriptors: Vec<_> = keys.iter().map(|key| {
            let (target, values, look) = match key.target {
                AnimationTarget::Offset(x, y) => (0, [x, y, 0.0, 0.0], encode_look(Look::NONE)),
                AnimationTarget::Opacity(value) => (1, [value, 0.0, 0.0, 0.0], encode_look(Look::NONE)),
                AnimationTarget::Look(value) => (2, [0.0; 4], encode_look(value)),
            };
            KeyframeDesc { time: key.time, target, reserved: [0; 4], values, look, curve: encode_curve(key.curve) }
        }).collect();
        let mut out = 0;
        status(unsafe { sys::pxui_ui_animate_keys(self.0.as_ptr(), box_, Slice { ptr: descriptors.as_ptr(), len: descriptors.len() }, &mut out) })?;
        Ok(out != 0)
    }

    pub fn table<F: FnMut(i64, i64) -> String>(&mut self, at: (f32, f32), width: f32, height: f32, headers: &[String], rows: i64, mut cell: F, label: &str) -> Result<Option<i64>, i32> {
        let header_values: Vec<Str> = headers.iter().map(|s| as_str(s)).collect();
        let mut callback = CellText { f: &mut cell, value: String::new(), panicked: false };
        let desc = TableDesc { label: as_str(label), at: [at.0, at.1], width, height, headers: Slice { ptr: header_values.as_ptr(), len: header_values.len() }, rows, cell: Some(call_cell::<F>), user: &mut callback as *mut _ as *mut c_void };
        let mut out = -1;
        status(unsafe { sys::pxui_ui_table(self.0.as_ptr(), &desc, &mut out) })?;
        Ok((out >= 0).then_some(out))
    }
}
