//! Kit and text widgets beyond the first set, with the engine's names and shapes (see `ui_state` for failures).
use crate::Handle;
use crate::box_ops::Label;
use crate::composite::{call, defer_callback_panic, deferred};
use crate::interaction::decode_signal;
use crate::{Button, Icon, IconButton, Segment, PickResponse, Segmented, Toggle};
use crate::kit::{HintDesc, KeyframeDesc};
use crate::node::look;
use crate::{as_str, check, Signal, Ui};
use core::ffi::c_void;
use pxui_types::{geom::{Point, Rect, Rgba}, ffi::{PxuiButtonDesc, PxuiDialogDesc, PxuiDialogResult, PxuiIconButtonDesc, PxuiResponse, PxuiHeadActionDesc, PxuiHeadOut, PxuiHeaderDesc, PxuiSignal, Slice}, theme::{Curve, Look, State}, Axis, PxuiBox, Str};
use pxui_types::Size;
use crate::Response;
use pxui_types::ui::{Dialog, DialogButton, Chosen, Head, HeadAction, Target};
use std::{fmt::Arguments, rc::Rc};

library!(lib {
    fn pxui_ui_animate(ui: *mut PxuiUi, box_: *const PxuiBox, key: *const KeyframeDesc, out: *mut u8) -> i32;
    fn pxui_ui_button_plain(ui: *mut PxuiUi, key: Str, text: Str, out: *mut PxuiResponse) -> i32;
    fn pxui_ui_dialog_response(ui: *mut PxuiUi, desc: *const PxuiDialogDesc, build: pxui_types::ffi::UiBuild, user: *mut c_void, out: *mut PxuiDialogResult) -> i32;
    fn pxui_ui_button_content(ui: *mut PxuiUi, desc: *const PxuiButtonDesc, build: pxui_types::ffi::PxuiButtonBuild, user: *mut c_void, out: *mut PxuiResponse) -> i32;
    fn pxui_ui_icon_button_plain(ui: *mut PxuiUi, key: Str, icon: Str, name: Str, out: *mut PxuiResponse) -> i32;
    fn pxui_ui_icon_button_response(ui: *mut PxuiUi, desc: *const PxuiIconButtonDesc, out: *mut PxuiResponse) -> i32;
    fn pxui_ui_icon_word(ui: *mut PxuiUi, word: u32) -> i32;
    fn pxui_ui_toggle_plain(ui: *mut PxuiUi, key: Str, text: Str, on: u8, out: *mut PxuiResponse) -> i32;
    fn pxui_ui_toggle_response(ui: *mut PxuiUi, desc: *const pxui_types::ffi::PxuiToggleDesc, out: *mut PxuiResponse) -> i32;
    fn pxui_ui_segmented_items(ui: *mut PxuiUi, desc: *const pxui_types::ffi::PxuiSegmentedDesc, items: Slice<pxui_types::ffi::PxuiPickItem>, out: *mut PxuiResponse, result: *mut pxui_types::ffi::PxuiSegmentedResult) -> i32;
    fn pxui_ui_disabled(ui: *mut PxuiUi, disabled: u8, build: pxui_types::ffi::UiBuild, user: *mut c_void) -> i32;
    fn pxui_ui_grip(ui: *mut PxuiUi, label: Str, axis: u32, has_rect: u8, rect: Rect, out: *mut PxuiBox, signal: *mut PxuiSignal) -> i32;
    fn pxui_ui_inspector_bar(ui: *mut PxuiUi, hints: Slice<HintDesc>, count: i64) -> i32;
    fn pxui_ui_inspector_body(ui: *mut PxuiUi, build: pxui_types::ffi::UiBuild, user: *mut c_void) -> i32;
    fn pxui_ui_inspector_header(ui: *mut PxuiUi, desc: *const PxuiHeaderDesc, rename: Option<extern "C" fn(*mut c_void, Str) -> u8>, user: *mut c_void, out: *mut PxuiHeadOut) -> i32;
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

/// A button's content callback (`pxui_ui_button_content`): `user` is the wrapper's `Option<&mut dyn FnMut(&mut Ui, State)>`, called
/// with the button's state. A panic is deferred to the end of the call, as `composite::call` defers it.
#[cfg(not(feature = "static"))]
extern "C" fn button_build(user: *mut c_void, raw: *mut c_void, state: *const pxui_types::ffi::PxuiStateDesc) {
    if user.is_null() || raw.is_null() || state.is_null() || deferred() { return; }
    let s = unsafe { *state };
    let state = State { hovered: s.hovered != 0, pressed: s.pressed != 0, focused: s.focused != 0, disabled: s.disabled != 0, checked: s.checked != 0 };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ui = core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new(raw.cast()).unwrap()));
        if let Some(f) = unsafe { &mut *user.cast::<Option<&mut dyn FnMut(&mut Ui, State)>>() } { f(&mut ui, state) }
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}

fn keyframe(target: Target, curve: Curve) -> KeyframeDesc {
    let (kind, values, shape) = match target {
        Target::Offset(x, y) => (0, [x, y, 0.0, 0.0], look(Look::NONE)),
        Target::Opacity(value) => (1, [value, 0.0, 0.0, 0.0], look(Look::NONE)),
        Target::Look(value) => (2, [0.0; 4], look(value)),
        Target::Scale { x, y, anchor } => (3, [x, y, anchor.0, anchor.1], look(Look::NONE)),
    };
    KeyframeDesc { size: core::mem::size_of::<KeyframeDesc>() as u32, time: 0.0, target: kind, reserved: 0, values, look: shape, curve: pxui_types::ffi::PxuiCurveDesc::from(curve) }
}

/// The six looks of `states` (idle, hover, pressed, focus, disabled, checked) as the descriptors carry them.
pub(crate) fn six(s: &pxui_types::theme::States) -> [pxui_types::ffi::PxuiLookDesc; 6] { [look(s.idle), look(s.hover), look(s.pressed), look(s.focus), look(s.disabled), look(s.checked)] }

/// The wrapper's `Response` of a button call's record.
// Inline: out of line, the button bodies cost the abi_bench Overlays page 0.9% more retired instructions (static release, U1c.1: 1.0189 inline, 1.0277 not).
#[inline]
pub(crate) fn response(out: &PxuiResponse) -> Response {
    let on = |bit: u32| out.flags & bit != 0;
    Response { clicked: on(PxuiResponse::CLICKED), hovered: on(PxuiResponse::HOVERED), held: on(PxuiResponse::HELD), focused: on(PxuiResponse::FOCUSED), activated: on(PxuiResponse::ACTIVATED), changed: false,
        secondary: on(PxuiResponse::SECONDARY), handle: out.node }
}

impl Ui {
    /// Moves the box `b` towards `target` along `curve`; true while it is still moving.
    /// Call it every build with the node's goal: a new goal starts a new segment from where the node is, an unchanged one keeps the running one. A stale handle returns false.
    /// Allocates: never on a warm frame; the first animation of a node takes its animation record.
    /// Panics: when the library refuses the call (an invalid curve, a non-finite target, or a target the node cannot take).
    ///
    /// ```
    /// use pxui::ui::{Curve, Target};
    /// pxui::doc::frames(2, &mut |ui| {
    ///     let b = ui.node("n", pxui::Node::DEFAULT);
    ///     ui.animate(&b, Target::Opacity(0.5), Curve::Spring { omega: 12.0 });
    /// });
    /// ```
    #[track_caller]
    pub fn animate(&mut self, b: &Handle, target: Target, curve: Curve) -> bool {
        let mut out = 0;
        let rc = unsafe { lib::pxui_ui_animate(self.0.as_ptr(), b, &keyframe(target, curve), &mut out) };
        crate::live(self, rc) && out != 0
    }
    /// The one body of `icon_button`: linked statically, the library's trusted entry with the options as they are; else the plain call
    /// for `IconButton::DEFAULT` with a glyph, or the options as a descriptor. One `PxuiResponse` comes back either way.
    #[track_caller]
    // Inline: out of line, the button bodies cost the abi_bench Overlays page 0.9% more retired instructions (static release, U1c.1: 1.0189 inline, 1.0277 not).
    #[inline]
    pub(crate) fn icon_button_keyed(&mut self, key: &str, icon: Icon, name: &str, button: &IconButton<'_>) -> Response {
        let mut buf = [0u8; 4];
        let (glyph, draw, user, word) = match icon {
            Icon::Glyph(ch) => (ch.encode_utf8(&mut buf) as &str, None, core::ptr::null_mut(), 0),
            // the app's function itself is the user pointer (`paint::icon_draw` casts it back), its word the icon's word
            Icon::Draw(f, word) => ("", Some(crate::paint::icon_draw as extern "C" fn(*mut c_void, *mut c_void, Rect, Rgba, *const pxui_types::ffi::PxuiPaintInfo)), f as *mut c_void, word),
        };
        let mut out = PxuiResponse::EMPTY;
        let plain = button.look.is_none() && matches!((button.w, button.h), (Size::Fit, Size::Fit)) && button.enabled && button.press_scale == 1.0 && draw.is_none()
            && button.press.is_none() && button.selected.is_none();
        #[cfg(feature = "static")]
        let rc = if plain {
            unsafe { pxui_abi_link::trusted::icon_button_plain(self.0.as_ptr(), key, glyph, name, &mut out) }
        } else {
            unsafe { pxui_abi_link::trusted::icon_button_with(self.0.as_ptr(), key, glyph, draw, user, word, name, button.look, button.w, button.h, button.enabled, button.press_scale, button.press, button.selected, &mut out, None) }
        };
        #[cfg(not(feature = "static"))]
        let rc = if plain {
            unsafe { lib::pxui_ui_icon_button_plain(self.0.as_ptr(), as_str(key), as_str(glyph), as_str(name), &mut out) }
        } else {
            let dim = |size: Size| match size { Size::Fit => (0, 0.0), Size::Px(v) => (1, v), Size::Grow => (2, 0.0), _ => (3, 0.0) };
            let ((w_kind, w), (h_kind, h)) = (dim(button.w), dim(button.h));
            let looks = button.look.map(|s| [look(s.idle), look(s.hover), look(s.pressed), look(s.focus), look(s.disabled), look(s.checked)]);
            let desc = PxuiIconButtonDesc {
                size: core::mem::size_of::<PxuiIconButtonDesc>() as u32, enabled: button.enabled as u8, w_kind, h_kind, reserved: 0, w, h, press_scale: button.press_scale,
                states: looks.as_ref().map_or(core::ptr::null(), |l| l.as_ptr()), key: as_str(key), name: as_str(name), icon: as_str(glyph), draw, draw_user: user,
                press: button.press.map_or(core::ptr::null(), |p| p.as_ptr()),
                checkable: button.selected.is_some() as u8, checked: (button.selected == Some(true)) as u8, reserved2: [0; 6],
            };
            if draw.is_some() { unsafe { lib::pxui_ui_icon_word(self.0.as_ptr(), word) }; }
            unsafe { lib::pxui_ui_icon_button_response(self.0.as_ptr(), &desc, &mut out) }
        };
        check(self, rc);
        response(&out)
    }
    /// The one body of `button` and `button_content`: linked statically, the library's trusted entry with the options as
    /// they are; else the plain call for `Button::DEFAULT` without contents, or the options as a descriptor and the contents as a callback
    /// given the button's state. One `PxuiResponse` comes back either way.
    #[track_caller]
    // Inline: out of line, the button bodies cost the abi_bench Overlays page 0.9% more retired instructions (static release, U1c.1: 1.0189 inline, 1.0277 not).
    #[inline]
    pub(crate) fn button_keyed(&mut self, key: &str, text: &str, button: &Button<'_>, content: Option<&mut dyn FnMut(&mut Ui, State)>) -> Response {
        let mut out = PxuiResponse::EMPTY;
        let plain = content.is_none() && button.plain;
        // the leading icon as the icon button passes it: a glyph's text, or the app's function as the drawing's user pointer and its word
        let mut glyph = [0u8; 4];
        let (lead, lead_draw, lead_user, lead_word) = match button.icon {
            None => ("", None, core::ptr::null_mut(), 0),
            Some(Icon::Glyph(ch)) => (ch.encode_utf8(&mut glyph) as &str, None, core::ptr::null_mut(), 0),
            Some(Icon::Draw(f, word)) => ("", Some(crate::paint::icon_draw as extern "C" fn(*mut c_void, *mut c_void, Rect, Rgba, *const pxui_types::ffi::PxuiPaintInfo)), f as *mut c_void, word),
        };
        #[cfg(feature = "static")]
        let rc = if plain {
            unsafe { pxui_abi_link::trusted::button_plain(self.0.as_ptr(), key, text, &mut out) }
        } else {
            let has_content = content.is_some();
            let mut inner = content;
            let outer = self.0.as_ptr();
            let mut run = |engine: &mut pxui_abi_link::trusted::Engine, state: State| if let Some(f) = inner.as_mut() {
                let mut borrowed = unsafe { pxui_abi_link::trusted::Borrowed::new(engine, outer) };
                let mut ui = core::mem::ManuallyDrop::new(Ui(unsafe { core::ptr::NonNull::new_unchecked(borrowed.enter()) }));
                f(&mut ui, state)
            };
            unsafe { pxui_abi_link::trusted::button_with(self.0.as_ptr(), key, text, button.name, button.look, button.w, button.h, button.align, button.pad,
                button.enabled, button.selected, button.danger, button.centered, button.label.as_ref().map(|l| l.fields()), button.axis, button.gap, button.content_align, button.focus_on_click,
                lead, lead_draw, lead_user, lead_word, button.icon_size, button.detail, button.accessory, button.detail_label.as_ref().map(|l| l.fields()), button.accessory_label.as_ref().map(|l| l.fields()),
                if has_content { Some(&mut run) } else { None }, &mut out, None) }
        };
        #[cfg(not(feature = "static"))]
        let rc = if plain {
            unsafe { lib::pxui_ui_button_plain(self.0.as_ptr(), as_str(key), as_str(text), &mut out) }
        } else {
            let dim = |size: Size| match size { Size::Fit => (0, 0.0), Size::Px(v) => (1, v), Size::Grow => (2, 0.0), _ => (3, 0.0) };
            let ((w_kind, w), (h_kind, h)) = (dim(button.w), dim(button.h));
            let looks = button.look.map(|s| [look(s.idle), look(s.hover), look(s.pressed), look(s.focus), look(s.disabled), look(s.checked)]);
            let p = button.pad;
            let desc = PxuiButtonDesc {
                size: core::mem::size_of::<PxuiButtonDesc>() as u32, enabled: button.enabled as u8, selected: button.selected as u8, danger: button.danger as u8,
                justify: match button.align { pxui_types::layout::Justify::Center => 1, pxui_types::layout::Justify::End => 2, _ => 0 }, w_kind, h_kind, has_name: button.name.is_some() as u8, reserved: 0,
                w, h, padding: [p.l, p.t, p.r, p.b], states: looks.as_ref().map_or(core::ptr::null(), |l| l.as_ptr()),
                key: as_str(key), text: as_str(text), name: as_str(button.name.unwrap_or("")), centered: button.centered as u8, reserved2: [0; 7],
                has_label: button.label.is_some() as u8, axis: button.axis.map_or(0, |a| a as u8 + 1),
                content_align: match button.content_align { None => 0, Some(pxui_types::layout::Along::Start) => 1, Some(pxui_types::layout::Along::Center) => 2, Some(pxui_types::layout::Along::End) => 3 }, focus_on_click: button.focus_on_click as u8,
                gap: button.gap, label: button.label.as_ref().map(pxui_types::ffi::PxuiLabelStyle::from).unwrap_or_default(), label_box: button.label.as_ref().map(pxui_types::ffi::PxuiLabelBox::from).unwrap_or_default(),
                lead: as_str(lead), lead_draw, lead_user, detail: as_str(button.detail), accessory: as_str(button.accessory),
                has_detail_label: button.detail_label.is_some() as u8, has_accessory_label: button.accessory_label.is_some() as u8, reserved3: [0; 6],
                detail_label: button.detail_label.as_ref().map(pxui_types::ffi::PxuiLabelStyle::from).unwrap_or_default(), detail_box: button.detail_label.as_ref().map(pxui_types::ffi::PxuiLabelBox::from).unwrap_or_default(),
                accessory_label: button.accessory_label.as_ref().map(pxui_types::ffi::PxuiLabelStyle::from).unwrap_or_default(), accessory_box: button.accessory_label.as_ref().map(pxui_types::ffi::PxuiLabelBox::from).unwrap_or_default(),
                icon_size: button.icon_size,
            };
            if lead_draw.is_some() { unsafe { lib::pxui_ui_icon_word(self.0.as_ptr(), lead_word) }; }
            let build = if content.is_some() { Some(button_build as extern "C" fn(*mut c_void, *mut c_void, *const pxui_types::ffi::PxuiStateDesc)) } else { None };
            let mut inner = content;
            unsafe { lib::pxui_ui_button_content(self.0.as_ptr(), &desc, build, &mut inner as *mut _ as *mut c_void, &mut out) }
        };
        check(self, rc);
        response(&out)
    }
    /// The one body of `toggle`; a click is the flip of `on`. Linked statically, the library's trusted entry with the options
    /// as they are; else the plain call for `Toggle::DEFAULT`, or the options as a descriptor. One `PxuiResponse` comes back either way.
    #[track_caller]
    pub(crate) fn toggle_keyed(&mut self, key: &str, text: &str, on: bool, toggle: &Toggle) -> Response {
        let mut out = PxuiResponse::EMPTY;
        let plain = toggle.row_h.is_none() && toggle.switch.is_none() && toggle.track.is_none() && toggle.knob.is_none() && toggle.enabled && toggle.detail.is_empty()
            && toggle.inset.is_none() && !toggle.press_row && toggle.label.is_none();
        let (switch_w, switch_h) = toggle.switch.unwrap_or((0.0, 0.0));
        #[cfg(feature = "static")]
        let rc = if plain {
            unsafe { pxui_abi_link::trusted::toggle_plain(self.0.as_ptr(), key, text, on, &mut out) }
        } else {
            unsafe { pxui_abi_link::trusted::toggle_with(self.0.as_ptr(), key, text, on, toggle.track, toggle.knob, toggle.row_h.unwrap_or(0.0), switch_w, switch_h,
                toggle.inset.unwrap_or(0.0), toggle.press_row, toggle.detail, toggle.label.as_ref().map(|l| l.fields()), toggle.enabled, &mut out) }
        };
        #[cfg(not(feature = "static"))]
        let rc = if plain {
            unsafe { lib::pxui_ui_toggle_plain(self.0.as_ptr(), as_str(key), as_str(text), on as u8, &mut out) }
        } else {
            let (track, knob) = (toggle.track.map(six), toggle.knob.map(six));
            let desc = pxui_types::ffi::PxuiToggleDesc {
                size: core::mem::size_of::<pxui_types::ffi::PxuiToggleDesc>() as u32, enabled: toggle.enabled as u8, on: on as u8, press_row: toggle.press_row as u8, reserved: 0,
                row_h: toggle.row_h.unwrap_or(0.0), switch_w, switch_h, inset: toggle.inset.unwrap_or(0.0),
                track: track.as_ref().map_or(core::ptr::null(), |l| l.as_ptr()), knob: knob.as_ref().map_or(core::ptr::null(), |l| l.as_ptr()),
                key: as_str(key), text: as_str(text), detail: as_str(toggle.detail),
                has_style: toggle.label.is_some() as u8, reserved2: [0; 7], style: toggle.label.as_ref().map_or_else(Default::default, pxui_types::ffi::PxuiLabelStyle::from),
            };
            unsafe { lib::pxui_ui_toggle_response(self.0.as_ptr(), &desc, &mut out) }
        };
        check(self, rc);
        let mut r = response(&out);
        r.changed = r.clicked;
        r
    }
    /// The one body of `segmented`, as `toggle_keyed`: the strip's response, the choice and the option a secondary press began on.
    #[track_caller]
    pub(crate) fn segmented_keyed(&mut self, key: &str, name: &str, options: &[Segment], selected: Option<usize>, segmented: &Segmented) -> PickResponse {
        let mut out = PxuiResponse::EMPTY;
        #[cfg(feature = "static")]
        let (chosen, secondary, rc) = {
            let (mut chosen, mut secondary) = (None, None);
            let rc = unsafe { pxui_abi_link::trusted::segmented(self.0.as_ptr(), key, name, options, selected, segmented.enabled, segmented.h.unwrap_or(0.0), segmented.grow,
                segmented.look, segmented.w.unwrap_or(0.0), segmented.padding, segmented.min_segment, segmented.focus_on_click, &mut out, &mut chosen, &mut secondary) };
            (chosen, secondary, rc)
        };
        #[cfg(not(feature = "static"))]
        let (chosen, secondary, rc) = {
            let items = crate::PickItems::new(options);
            let states = segmented.look.map(six);
            let desc = pxui_types::ffi::PxuiSegmentedDesc {
                size: core::mem::size_of::<pxui_types::ffi::PxuiSegmentedDesc>() as u32, enabled: segmented.enabled as u8, grow: segmented.grow as u8, reserved: [0; 2], h: segmented.h.unwrap_or(0.0),
                states: states.as_ref().map_or(core::ptr::null(), |l| l.as_ptr()), key: as_str(key), name: as_str(name), selected: selected.unwrap_or(0),
                w: segmented.w.unwrap_or(0.0), padding: segmented.padding, min_segment: segmented.min_segment, reserved2: 0,
                names: Slice { ptr: core::ptr::null(), len: 0 }, has_selected: selected.is_some() as u8, reserved3: [0; 7], focus_on_click: segmented.focus_on_click as u8, reserved4: [0; 7],
            };
            let mut result = pxui_types::ffi::PxuiSegmentedResult::EMPTY;
            let rc = unsafe { lib::pxui_ui_segmented_items(self.0.as_ptr(), &desc, items.span(), &mut out, &mut result) };
            ((result.has_chosen != 0).then_some(result.chosen as usize), (result.has_secondary != 0).then_some(result.secondary as usize), rc)
        };
        check(self, rc);
        let mut response = response(&out);
        response.changed = chosen != selected.filter(|_| !options.is_empty()).map(|i| i.min(options.len() - 1));
        PickResponse { response, chosen, chosen_id: chosen.and_then(|i| options.get(i)?.id), secondary }
    }
    /// The one body of `disabled`: `f` runs under the library's scope.
    #[track_caller]
    pub(crate) fn disabled_scope(&mut self, disabled: bool, mut f: &mut dyn FnMut(&mut Ui)) {
        check(self, unsafe { lib::pxui_ui_disabled(self.0.as_ptr(), disabled as u8, Some(call), &mut f as *mut _ as *mut c_void) })
    }
    /// The one body of `dialog`: linked statically, the library's trusted entry with the buttons as they are; else the buttons as a
    /// descriptor (their labels and flags) and the body as a callback.
    #[track_caller]
    pub(crate) fn dialog_keyed(&mut self, key: &str, title: &str, buttons: &[DialogButton], style: &Dialog, mut body: &mut dyn FnMut(&mut Ui)) -> Chosen {
        #[cfg(feature = "static")]
        {
            let mut out = Chosen::default();
            check(self, unsafe { let outer = self.0.as_ptr(); pxui_abi_link::trusted::dialog(outer, key, title, buttons, style, inner!(outer, |ui: &mut Ui| body(ui)), &mut out) });
            out
        }
        #[cfg(not(feature = "static"))]
        {
            let (mut texts, mut details) = ([Str::default(); 64], [Str::default(); 64]);
            let at = |pick: fn(&DialogButton) -> bool| buttons.iter().position(pick).map_or(-1, |i| i as i64);
            let mask = |pick: fn(&DialogButton) -> bool| buttons.iter().take(64).enumerate().fold(0u64, |m, (i, b)| m | (pick(b) as u64) << i);
            for ((text, detail), b) in texts.iter_mut().zip(details.iter_mut()).zip(buttons) { (*text, *detail) = (as_str(b.text), as_str(b.detail)); }
            let desc = PxuiDialogDesc {
                size: core::mem::size_of::<PxuiDialogDesc>() as u32, width: style.w, key: as_str(key), title: as_str(title),
                buttons: Slice { ptr: texts.as_ptr(), len: buttons.len() }, default_at: at(|b| b.primary), cancel_at: at(|b| b.cancel),
                danger: mask(|b| b.danger), disabled: mask(|b| !b.enabled),
                details: Slice { ptr: details.as_ptr(), len: buttons.len() },
            };
            let mut result = PxuiDialogResult::EMPTY;
            check(self, unsafe { lib::pxui_ui_dialog_response(self.0.as_ptr(), &desc, Some(call), &mut body as *mut _ as *mut c_void, &mut result) });
            Chosen { chosen: (result.has_chosen != 0).then_some(result.chosen as usize), chosen_id: None, dismissed: result.dismissed != 0 }
        }
    }
}

// The extension set (`pxui::ext`): free functions on a `Ui`, out of the prelude, with no stability promise.
/// A 1-point splitter line (see the engine): its box and what happened to it.
#[track_caller]
pub fn grip(ui: &mut Ui, label: Arguments, axis: Axis, rect: Option<Rect>) -> (Handle, Signal) {
    let (mut out, mut raw) = (Handle::default(), PxuiSignal::default());
    let label = Label::new(label);
    let code = match axis { Axis::Row => 0, _ => 1 };
    check(ui, unsafe { lib::pxui_ui_grip(ui.0.as_ptr(), as_str(label.as_str()), code, rect.is_some() as u8, rect.unwrap_or_default(), &mut out, &mut raw) });
    (out, decode_signal(&raw))
}
#[track_caller]
pub fn inspector_bar(ui: &mut Ui, hints: &[(&str, &str)], count: i64) {
    let hints = crate::kit::Hints::new(hints);
    check(ui, unsafe { lib::pxui_ui_inspector_bar(ui.0.as_ptr(), hints.span(), count) })
}
#[track_caller]
pub fn inspector_body(ui: &mut Ui, mut f: &mut dyn FnMut(&mut Ui)) {
    check(ui, unsafe { lib::pxui_ui_inspector_body(ui.0.as_ptr(), Some(call), &mut f as *mut _ as *mut c_void) })
}
#[allow(clippy::too_many_arguments)]
#[track_caller]
pub fn inspector_header(ui: &mut Ui, key: &str, kind: Option<&str>, badge: Option<&str>, index: Option<&str>, rename: Option<Rc<dyn Fn(&str) -> bool>>,
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
    check(ui, unsafe { lib::pxui_ui_inspector_header(ui.0.as_ptr(), &desc, rename.is_some().then_some(rename_call as extern "C" fn(*mut c_void, Str) -> u8), user, &mut out) });
    let renamed = if out.renamed.len == 0 { String::new() } else { String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(out.renamed.ptr, out.renamed.len) }).into_owned() };
    Head { renamed, chosen: (out.has_chosen != 0).then_some(out.chosen), reset_pressed: out.reset_pressed != 0 }
}
/// The fraction of a dragged `kept_drag` field's track under the pointer, and the pointer's points from the press.
#[track_caller]
pub fn field_drag(ui: &Ui, field: &Handle) -> Option<(f32, f32)> {
    let (mut present, mut out) = (0, Point::default());
    let rc = unsafe { lib::pxui_ui_field_drag(ui.0.as_ptr(), field, &mut present, &mut out) };
    (crate::live(ui, rc) && present != 0).then_some((out.x, out.y))
}
