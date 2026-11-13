//! Kit and text widgets beyond the first set, with the engine's names and shapes (see `ui_state` for failures).
use crate::box_ops::Label;
use crate::composite::{call, defer_callback_panic, deferred};
use crate::interaction::decode_signal;
use crate::kit::{encode_curve, HintDesc, KeyframeDesc};
use crate::node::look;
use crate::{as_str, check, Signal, Ui};
use core::ffi::c_void;
use pxui_types::{geom::{Point, Rect, Rgba}, ffi::{PxuiHeadActionDesc, PxuiHeadOut, PxuiHeaderDesc, PxuiPopupDesc, PxuiSignal, Slice}, theme::{Curve, Look}, Axis, PxuiBox, Str};
use pxui_types::ui::{Head, HeadAction, LineStep, Target};
use std::{fmt::Arguments, rc::Rc};

library!(lib {
    fn pxui_ui_animate(ui: *mut PxuiUi, box_: *const PxuiBox, key: *const KeyframeDesc, out: *mut u8) -> i32;
    fn pxui_ui_button_styled(ui: *mut PxuiUi, text: Str, bare: u8, icon: u8, at_end: u8, has_ink: u8, ink: Rgba, out: *mut u8) -> i32;
    fn pxui_ui_grip(ui: *mut PxuiUi, label: Str, axis: u32, has_rect: u8, rect: Rect, out: *mut PxuiBox, signal: *mut PxuiSignal) -> i32;
    fn pxui_ui_popup(ui: *mut PxuiUi, desc: *const PxuiPopupDesc, build: pxui_types::ffi::UiBuild, user: *mut c_void, out: *mut u8) -> i32;
    fn pxui_ui_inspector_bar(ui: *mut PxuiUi, hints: Slice<HintDesc>, count: i64) -> i32;
    fn pxui_ui_inspector_body(ui: *mut PxuiUi, build: pxui_types::ffi::UiBuild, user: *mut c_void) -> i32;
    fn pxui_ui_inspector_header(ui: *mut PxuiUi, desc: *const PxuiHeaderDesc, rename: Option<extern "C" fn(*mut c_void, Str) -> u8>, user: *mut c_void, out: *mut PxuiHeadOut) -> i32;
    fn pxui_ui_inspector_section_view(ui: *mut PxuiUi, key: Str, revision: u64, expanded: u8, has_set: u8, set_expanded: u8, label: Str, build: pxui_types::ffi::UiBuild, user: *mut c_void, out: *mut u8) -> i32;
    fn pxui_ui_inspector_toggle_value(ui: *mut PxuiUi, key: Str, x: f32, y: f32, value: u8, out: *mut u8) -> i32;
    fn pxui_ui_line_field(ui: *mut PxuiUi, x: f32, y: f32, w: f32, h: f32, grab: u8, placeholder: Str, label: Str, value: Str, text: *mut Str, step: *mut u32) -> i32;
    fn pxui_ui_field_drag(ui: *mut PxuiUi, box_: *const PxuiBox, present: *mut u8, out: *mut Point) -> i32;
});

/// The validator of an inspector head's name: called by the library during the head's call.
extern "C" fn rename_call(user: *mut c_void, name: Str) -> u8 {
    if user.is_null() || deferred() { return 0; }
    let valid = unsafe { &*user.cast::<Rc<dyn Fn(&str) -> bool>>() };
    let name = if name.len == 0 { "" } else { core::str::from_utf8(unsafe { core::slice::from_raw_parts(name.ptr, name.len) }).unwrap_or("") };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| valid(name))) {
        Ok(ok) => ok as u8,
        Err(panic) => { defer_callback_panic(panic); 0 }
    }
}

fn keyframe(target: Target, curve: Curve) -> KeyframeDesc {
    let (kind, values, shape) = match target {
        Target::Offset(x, y) => (0, [x, y, 0.0, 0.0], look(Look::NONE)),
        Target::Opacity(value) => (1, [value, 0.0, 0.0, 0.0], look(Look::NONE)),
        Target::Look(value) => (2, [0.0; 4], look(value)),
        Target::Scale { x, y, anchor } => (3, [x, y, anchor.0, anchor.1], look(Look::NONE)),
    };
    KeyframeDesc { size: core::mem::size_of::<KeyframeDesc>() as u32, time: 0.0, target: kind, reserved: 0, values, look: shape, curve: encode_curve(curve) }
}

impl Ui {
    /// Moves the box towards `target` along `curve`; whether it is still moving.
    #[track_caller]
    pub fn animate(&mut self, b: &PxuiBox, target: Target, curve: Curve) -> bool {
        let mut out = 0;
        let rc = unsafe { lib::pxui_ui_animate(self.0.as_ptr(), b, &keyframe(target, curve), &mut out) };
        crate::live(self, rc) && out != 0
    }
    #[track_caller]
    pub fn button_styled(&mut self, text: &str, bare: bool, icon: bool, at_end: bool, ink: Option<Rgba>) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_button_styled(self.0.as_ptr(), as_str(text), bare as u8, icon as u8, at_end as u8, ink.is_some() as u8, ink.unwrap_or_default(), &mut out) });
        out != 0
    }
    /// A 1-point splitter line (see the engine): its box and what happened to it.
    #[track_caller]
    pub fn grip(&mut self, label: Arguments, axis: Axis, rect: Option<Rect>) -> (PxuiBox, Signal) {
        let (mut out, mut raw) = (PxuiBox::default(), PxuiSignal::default());
        let label = Label::new(label);
        let code = match axis { Axis::Row => 0, _ => 1 };
        check(self, unsafe { lib::pxui_ui_grip(self.0.as_ptr(), as_str(label.as_str()), code, rect.is_some() as u8, rect.unwrap_or_default(), &mut out, &mut raw) });
        (out, decode_signal(&raw))
    }
    #[track_caller]
    pub fn popup(&mut self, stroke: Option<Rgba>, max_height: Option<f32>, dismiss_initial: bool, attached: bool, at: (f32, f32), width: f32, height: f32, label: &str, mut f: &mut dyn FnMut(&mut Ui)) -> bool {
        let desc = PxuiPopupDesc {
            size: core::mem::size_of::<PxuiPopupDesc>() as u32, has_stroke: stroke.is_some() as u8, has_max_height: max_height.is_some() as u8,
            dismiss_initial: dismiss_initial as u8, attached: attached as u8, stroke: stroke.unwrap_or_default(), max_height: max_height.unwrap_or_default(),
            x: at.0, y: at.1, width, height, label: as_str(label),
        };
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_popup(self.0.as_ptr(), &desc, Some(call::<&mut dyn FnMut(&mut Ui)>), &mut f as *mut _ as *mut c_void, &mut out) });
        out != 0
    }
    #[track_caller]
    pub fn inspector_bar(&mut self, hints: &[(String, String)], count: i64) {
        let hints = crate::kit::Hints::new(hints);
        check(self, unsafe { lib::pxui_ui_inspector_bar(self.0.as_ptr(), hints.span(), count) })
    }
    #[track_caller]
    pub fn inspector_body(&mut self, mut f: &mut dyn FnMut(&mut Ui)) {
        check(self, unsafe { lib::pxui_ui_inspector_body(self.0.as_ptr(), Some(call::<&mut dyn FnMut(&mut Ui)>), &mut f as *mut _ as *mut c_void) })
    }
    #[allow(clippy::too_many_arguments)]
    #[track_caller]
    pub fn inspector_header(&mut self, key: &str, kind: Option<&str>, badge: Option<&str>, index: Option<&str>, rename: Option<Rc<dyn Fn(&str) -> bool>>,
        actions: &[HeadAction], reset: Option<&str>, reset_enabled: bool, title: &str, detail: &str) -> Head {
        let text = |value: Option<&str>| value.map_or_else(Str::default, as_str);
        // up to 16 actions live on the stack, more on the heap
        let one = |a: &HeadAction| PxuiHeadActionDesc { size: core::mem::size_of::<PxuiHeadActionDesc>() as u32, active: a.active as u8, usable: a.usable as u8, reserved: [0; 2], caption: as_str(&a.caption), keycap: as_str(&a.keycap) };
        let mut stack = [one(&HeadAction { caption: String::new(), keycap: String::new(), active: false, usable: false }); 16];
        let heap: Vec<PxuiHeadActionDesc>;
        let descs: &[PxuiHeadActionDesc] = if actions.len() <= 16 {
            for (slot, a) in stack.iter_mut().zip(actions) { *slot = one(a); }
            &stack[..actions.len()]
        } else { heap = actions.iter().map(one).collect(); &heap };
        let desc = PxuiHeaderDesc {
            size: core::mem::size_of::<PxuiHeaderDesc>() as u32, has_kind: kind.is_some() as u8, has_badge: badge.is_some() as u8, has_index: index.is_some() as u8,
            has_reset: reset.is_some() as u8, reset_enabled: reset_enabled as u8, reserved: [0; 3], key: as_str(key), kind: text(kind), badge: text(badge), index: text(index),
            reset: text(reset), title: as_str(title), detail: as_str(detail), actions: Slice { ptr: descs.as_ptr(), len: descs.len() },
        };
        let mut out = PxuiHeadOut { size: core::mem::size_of::<PxuiHeadOut>() as u32, has_chosen: 0, reset_pressed: 0, reserved: [0; 2], renamed: Str::default(), chosen: 0 };
        let user = rename.as_ref().map_or(core::ptr::null_mut(), |rename| rename as *const Rc<dyn Fn(&str) -> bool> as *mut c_void);
        check(self, unsafe { lib::pxui_ui_inspector_header(self.0.as_ptr(), &desc, rename.is_some().then_some(rename_call as extern "C" fn(*mut c_void, Str) -> u8), user, &mut out) });
        let renamed = if out.renamed.len == 0 { String::new() } else { String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(out.renamed.ptr, out.renamed.len) }).into_owned() };
        Head { renamed, chosen: (out.has_chosen != 0).then_some(out.chosen), reset_pressed: out.reset_pressed != 0 }
    }
    #[track_caller]
    pub fn inspector_section_view(&mut self, key: &str, revision: u64, expanded: bool, set_expanded: Option<bool>, label: &str, mut f: &mut dyn FnMut(&mut Ui)) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_inspector_section_view(self.0.as_ptr(), as_str(key), revision, expanded as u8, set_expanded.is_some() as u8, set_expanded.unwrap_or(false) as u8, as_str(label), Some(call::<&mut dyn FnMut(&mut Ui)>), &mut f as *mut _ as *mut c_void, &mut out) });
        out != 0
    }
    #[track_caller]
    pub fn inspector_toggle_value(&mut self, key: &str, at: (f32, f32), value: bool) -> bool {
        let mut out = 0;
        check(self, unsafe { lib::pxui_ui_inspector_toggle_value(self.0.as_ptr(), as_str(key), at.0, at.1, value as u8, &mut out) });
        out != 0
    }
    /// A one-line search field at `at`: the new value and what its keys asked.
    #[allow(clippy::too_many_arguments)]
    #[track_caller]
    pub fn line_field(&mut self, at: (f32, f32), w: f32, h: f32, grab: bool, placeholder: &str, label: &str, value: &str) -> (String, LineStep) {
        let (mut text, mut step) = (Str::default(), 0);
        check(self, unsafe { lib::pxui_ui_line_field(self.0.as_ptr(), at.0, at.1, w, h, grab as u8, as_str(placeholder), as_str(label), as_str(value), &mut text, &mut step) });
        let edited = if text.len == 0 { String::new() } else { String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(text.ptr, text.len) }).into_owned() };
        (edited, match step { 1 => LineStep::Next, 2 => LineStep::Previous, 3 => LineStep::Close, _ => LineStep::None })
    }
    /// The fraction of a dragged `kept_drag` field's track under the pointer, and the pointer's points from the press.
    #[track_caller]
    pub fn field_drag(&self, field: &PxuiBox) -> Option<(f32, f32)> {
        let (mut present, mut out) = (0, Point::default());
        let rc = unsafe { lib::pxui_ui_field_drag(self.0.as_ptr(), field, &mut present, &mut out) };
        (crate::live(self, rc) && present != 0).then_some((out.x, out.y))
    }
}
