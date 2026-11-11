use crate::{composite::{defer_callback_panic, deferred}, status, Strs, Ui};
use core::{ffi::c_void, mem::ManuallyDrop, panic::AssertUnwindSafe, ptr::NonNull};
use pxui_types::{Axis, Curve, Look, PxuiBox, Str, ffi::{PxuiCurveDesc, PxuiFontDesc, PxuiLookDesc, PxuiRingDesc, PxuiShadowDesc, Slice}};

#[cfg(feature = "static")]
pub(crate) use pxui_abi_link::{PxuiFooterDesc as FooterDesc, PxuiGridListDesc as GridListDesc, PxuiHintDesc as HintDesc, PxuiInspectorRowDesc as InspectorRowDesc, PxuiInspectorSectionDesc as InspectorSectionDesc, PxuiInspectorTextDesc as InspectorTextDesc, PxuiInspectorValueDesc as InspectorValueDesc, PxuiKeyframeDesc as KeyframeDesc, PxuiMenuBarDesc as MenuBarDesc, PxuiMenuDesc as MenuDesc, PxuiMenuOut as MenuOut, PxuiSplitDesc as SplitDesc, PxuiTableDesc as TableDesc};

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
pub struct HintDesc { pub key: Str, pub value: Str }
#[cfg(not(feature = "static"))]
#[repr(C)]
pub struct FooterDesc { has_right: u8, reserved: [u8; 7], right: Str, hints: Slice<HintDesc> }
#[cfg(not(feature = "static"))]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KeyframeDesc { pub time: f32, pub target: u32, pub reserved: [u8; 4], pub values: [f32; 4], pub look: PxuiLookDesc, pub curve: PxuiCurveDesc }
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

#[inline]
fn as_str(value: &str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }

type Build = Option<extern "C" fn(*mut c_void, *mut sys::PxuiUi)>;
type GridBuild = Option<extern "C" fn(*mut c_void, *mut sys::PxuiUi, usize)>;
type CellFn = Option<extern "C" fn(*mut c_void, i64, i64) -> Str>;

// Each context hands out its trampoline and its user pointer from one method, so the two cannot name different types.
struct Callback<F> { f: F }
impl<F: FnMut(&mut Ui)> Callback<F> {
    fn erased(&mut self) -> (Build, *mut c_void) { (Some(call::<F>), (self as *mut Self).cast()) }
}
struct GridCallback<F> { f: F }
impl<F: FnMut(&mut Ui, usize)> GridCallback<F> {
    fn erased(&mut self) -> (GridBuild, *mut c_void) { (Some(call_grid::<F>), (self as *mut Self).cast()) }
}
extern "C" fn call<F: FnMut(&mut Ui)>(user: *mut c_void, raw: *mut sys::PxuiUi) {
    if raw.is_null() || deferred() { return; }
    let state = unsafe { &mut *user.cast::<Callback<F>>() };
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut ui = ManuallyDrop::new(Ui(unsafe { NonNull::new_unchecked(raw) }));
        (state.f)(&mut ui);
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}
extern "C" fn call_grid<F: FnMut(&mut Ui, usize)>(user: *mut c_void, raw: *mut sys::PxuiUi, index: usize) {
    if raw.is_null() || deferred() { return; }
    let state = unsafe { &mut *user.cast::<GridCallback<F>>() };
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut ui = ManuallyDrop::new(Ui(unsafe { NonNull::new_unchecked(raw) }));
        (state.f)(&mut ui, index);
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}
struct CellText<F> { f: F, value: String }
impl<F: FnMut(i64, i64) -> String> CellText<F> {
    fn erased(&mut self) -> (CellFn, *mut c_void) { (Some(call_cell::<F>), (self as *mut Self).cast()) }
}
extern "C" fn call_cell<F: FnMut(i64, i64) -> String>(user: *mut c_void, row: i64, column: i64) -> Str {
    let state = unsafe { &mut *user.cast::<CellText<F>>() };
    if deferred() { return Str::default(); }
    match std::panic::catch_unwind(AssertUnwindSafe(|| (state.f)(row, column))) {
        Ok(value) => { state.value = value; Str { ptr: state.value.as_ptr(), len: state.value.len() } }
        Err(panic) => { defer_callback_panic(panic); Str::default() }
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
pub(crate) fn encode_curve(value: Curve) -> PxuiCurveDesc {
    match value {
        Curve::Bezier { duration, points } => PxuiCurveDesc { kind: PxuiCurveDesc::BEZIER, duration, points, omega: 0.0 },
        Curve::Spring { omega } => PxuiCurveDesc { kind: PxuiCurveDesc::SPRING, duration: 0.0, points: [0.0; 4], omega },
    }
}

/// Hint rows as descriptors for one call: up to 16 on the stack, more on the heap.
pub(crate) struct Hints { stack: [HintDesc; 16], heap: Vec<HintDesc>, len: usize }
impl Hints {
    pub(crate) fn new(hints: &[(String, String)]) -> Hints {
        let mut out = Hints { stack: [HintDesc::default(); 16], heap: Vec::new(), len: hints.len() };
        let values: &mut [HintDesc] = if hints.len() <= 16 { &mut out.stack[..hints.len()] } else { out.heap = vec![HintDesc::default(); hints.len()]; &mut out.heap[..] };
        for (out, (key, value)) in values.iter_mut().zip(hints) { *out = HintDesc { key: as_str(key), value: as_str(value) }; }
        out
    }
    pub(crate) fn span(&self) -> Slice<HintDesc> { Slice { ptr: if self.len <= 16 { self.stack.as_ptr() } else { self.heap.as_ptr() }, len: self.len } }
}

impl Ui {
    #[inline]
    pub fn grid_list<F: FnMut(&mut Ui, usize)>(&mut self, label: &str, count: usize, cell_w: f32, cell_h: f32, build: F) -> Result<PxuiBox, i32> {
        let mut context = GridCallback { f: build };
        let (build, user) = context.erased();
        let desc = GridListDesc { label: as_str(label), count, cell_w, cell_h, build, user };
        let mut out = PxuiBox::default();
        status(unsafe { sys::pxui_ui_grid_list(self.0.as_ptr(), &desc, &mut out) })?;
        Ok(out)
    }

    #[inline]
    pub fn split<F: FnMut(&mut Ui), G: FnMut(&mut Ui)>(&mut self, label: &str, axis: Axis, ratio: f32, min_a: f32, min_b: f32, a: F, b: G) -> Result<Option<f32>, i32> {
        let (mut context_a, mut context_b) = (Callback { f: a }, Callback { f: b });
        let ((build_a, user_a), (build_b, user_b)) = (context_a.erased(), context_b.erased());
        let desc = SplitDesc { label: as_str(label), axis: match axis { Axis::Row => 0, Axis::Column => 1, Axis::Stack => return Err(-1) }, ratio, min_a, min_b, build_a, user_a, build_b, user_b };
        let (mut changed, mut out) = (0u8, 0.0f32);
        status(unsafe { sys::pxui_ui_split(self.0.as_ptr(), &desc, &mut changed, &mut out) })?;
        Ok((changed != 0).then_some(out))
    }

    #[inline]
    pub fn menu_bar(&mut self, label: &str, menus: &[(&str, &[&str])]) -> Result<Option<(usize, usize)>, i32> {
        // a bar of up to 8 menus and 64 items lives on the stack, a larger one on the heap
        let total: usize = menus.iter().map(|(_, items)| items.len()).sum();
        let none = MenuDesc { title: Str::default(), items: Slice { ptr: core::ptr::null(), len: 0 } };
        let (mut flat_stack, mut menus_stack) = ([Str::default(); 64], [none; 8]);
        let (mut flat_heap, mut menus_heap): (Vec<Str>, Vec<MenuDesc>);
        let (flat, descs): (&mut [Str], &mut [MenuDesc]) = if total <= 64 && menus.len() <= 8 {
            (&mut flat_stack[..total], &mut menus_stack[..menus.len()])
        } else {
            flat_heap = vec![Str::default(); total];
            menus_heap = vec![none; menus.len()];
            (&mut flat_heap[..], &mut menus_heap[..])
        };
        let mut at = 0;
        for ((title, items), desc) in menus.iter().zip(descs.iter_mut()) {
            for (out, item) in flat[at..at + items.len()].iter_mut().zip(*items) { *out = as_str(item); }
            *desc = MenuDesc { title: as_str(title), items: Slice { ptr: flat[at..].as_ptr(), len: items.len() } };
            at += items.len();
        }
        let desc = MenuBarDesc { label: as_str(label), menus: Slice { ptr: descs.as_ptr(), len: descs.len() } };
        let mut out = MenuOut { menu: usize::MAX, item: usize::MAX };
        status(unsafe { sys::pxui_ui_menu_bar(self.0.as_ptr(), &desc, &mut out) })?;
        Ok((out.menu != usize::MAX && out.item != usize::MAX).then_some((out.menu, out.item)))
    }

    #[inline]
    pub fn inspector_row(&mut self, width: Option<f32>, pin: Option<bool>, key: &str, label: &str) -> Result<(PxuiBox, f32, f32, f32), i32> {
        let mut desc = InspectorRowDesc { key: as_str(key), label: as_str(label), has_width: width.is_some() as u8, has_pin: pin.is_some() as u8, pin: pin.unwrap_or(false) as u8, reserved: [0; 5], width: width.unwrap_or_default(), row: PxuiBox::default(), control_x: 0.0, control_y: 0.0, control_w: 0.0 };
        status(unsafe { sys::pxui_ui_inspector_row(self.0.as_ptr(), &mut desc) })?;
        Ok((desc.row, desc.control_x, desc.control_y, desc.control_w))
    }

    #[inline]
    pub fn inspector_section<F: FnMut(&mut Ui)>(&mut self, key: &str, expanded: bool, set_expanded: Option<bool>, label: &str, build: F) -> Result<bool, i32> {
        let mut context = Callback { f: build };
        let (build, user) = context.erased();
        let mut desc = InspectorSectionDesc { key: as_str(key), label: as_str(label), expanded: expanded as u8, has_set: set_expanded.is_some() as u8, set_expanded: set_expanded.unwrap_or(false) as u8, reserved: [0; 5], build, user, out_open: 0 };
        status(unsafe { sys::pxui_ui_inspector_section(self.0.as_ptr(), &mut desc) })?;
        Ok(desc.out_open != 0)
    }

    #[inline]
    pub fn inspector_toggle(&mut self, key: &str, label: &str, value: bool) -> Result<bool, i32> {
        let mut out = 0;
        status(unsafe { sys::pxui_ui_inspector_toggle(self.0.as_ptr(), as_str(key), as_str(label), value as u8, &mut out) })?;
        Ok(out != 0)
    }
    #[inline]
    pub fn inspector_button(&mut self, key: &str, label: &str) -> Result<bool, i32> {
        let mut out = 0;
        status(unsafe { sys::pxui_ui_inspector_button(self.0.as_ptr(), as_str(key), as_str(label), &mut out) })?;
        Ok(out != 0)
    }
    #[inline]
    pub fn inspector_readout(&mut self, width: Option<f32>, key: &str, label: &str, value: &str) -> Result<(), i32> {
        let desc = InspectorValueDesc { key: as_str(key), label: as_str(label), value: as_str(value), has_width: width.is_some() as u8, reserved: [0; 7], width: width.unwrap_or_default() };
        status(unsafe { sys::pxui_ui_inspector_readout(self.0.as_ptr(), &desc) })
    }
    #[inline]
    pub fn inspector_message(&mut self, key: &str, message: &str) -> Result<(), i32> {
        let desc = InspectorTextDesc { key: as_str(key), text: as_str(message) };
        status(unsafe { sys::pxui_ui_inspector_message(self.0.as_ptr(), &desc) })
    }

    #[inline]
    pub fn wants_frame(&self, events: &[pxui_types::Event], now: f64) -> Result<bool, i32> {
        let mut out = 0;
        status(unsafe { sys::pxui_ui_wants_frame(self.0.as_ptr(), Slice { ptr: events.as_ptr(), len: events.len() }, now, &mut out) })?;
        Ok(out != 0)
    }
    #[inline]
    pub fn footer(&mut self, right: Option<&str>, hints: &[(String, String)]) -> Result<(), i32> {
        let hints = Hints::new(hints);
        let desc = FooterDesc { has_right: right.is_some() as u8, reserved: [0; 7], right: right.map_or_else(Str::default, as_str), hints: hints.span() };
        status(unsafe { sys::pxui_ui_footer(self.0.as_ptr(), &desc) })
    }
    #[inline]
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

    #[inline]
    pub fn table<F: FnMut(i64, i64) -> String>(&mut self, at: (f32, f32), width: f32, height: f32, headers: &[String], rows: i64, cell: F, label: &str) -> Result<Option<i64>, i32> {
        let header_values = Strs::owned(headers);
        let mut callback = CellText { f: cell, value: String::new() };
        let (cell, user) = callback.erased();
        let desc = TableDesc { label: as_str(label), at: [at.0, at.1], width, height, headers: header_values.span(), rows, cell, user };
        let mut out = -1;
        status(unsafe { sys::pxui_ui_table(self.0.as_ptr(), &desc, &mut out) })?;
        Ok((out >= 0).then_some(out))
    }
}
