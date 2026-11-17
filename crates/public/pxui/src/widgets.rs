use crate::Handle;
use crate::{Key, PxuiBox, Response, Scrubber, SearchField, SearchFieldResponse, Ui, check};
use pxui_types::{ffi::{PxuiKeyedDesc, PxuiResponse, Slice}, theme::BodyKind, ui::Checkbox, Str};

library!(keyed_lib {
    fn pxui_ui_checkbox_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, on: u8, out: *mut PxuiResponse) -> i32;
    fn pxui_ui_choice_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, kind: u32, options: Slice<Str>, selected: usize, out: *mut PxuiResponse, chosen: *mut usize) -> i32;
    fn pxui_ui_slider_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, value: f32, min: f32, max: f32, out: *mut PxuiResponse, value_out: *mut f32) -> i32;
    fn pxui_ui_progress_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, has_value: u8, value: f32, out: *mut PxuiBox) -> i32;
    fn pxui_ui_spinner_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, out: *mut PxuiBox) -> i32;
    fn pxui_ui_badge_keyed(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, value: Str, out: *mut PxuiBox) -> i32;
});

library!(scrub_lib {
    fn pxui_ui_scrub(ui: *mut PxuiUi, label: pxui_types::Str, value: f32, min: f32, max: f32, step: f32, out: *mut pxui_types::ffi::PxuiScrubOut) -> i32;
});

/// What `Ui::scrub` reports for a caller-painted scrubber.
#[derive(Clone, Copy, Debug)]
pub struct Scrub {
    /// The scrubber's node, valid this frame: size it with `set_node_layout` and paint it with `draw`.
    pub node: Handle,
    /// The provisional value while the pointer drags, or none.
    pub preview: Option<f32>,
    /// The final value: the pointer's release, or a key or accessibility adjustment; none otherwise.
    pub commit: Option<f32>,
    /// The pointer holds the scrubber.
    pub held: bool,
}


#[cfg(not(feature = "static"))]
mod sys {
    use pxui_types::Str;
    use crate::sys::PxuiUi;
    #[link(name = "pxui")]
    unsafe extern "C" {
    pub fn pxui_ui_search_field(ui: *mut PxuiUi, label: Str, icon: Str, value: Str, placeholder: Str, changed: *mut u8, out: *mut Str) -> i32;
    }
}


#[cfg(not(feature = "static"))]
#[inline]
fn as_str(value: &str) -> Str { crate::as_str(value) }

impl Ui {

    /// The one body of `scrub`.
    #[track_caller]
    pub(crate) fn scrub_key(&mut self, key: Key, value: f32, range: core::ops::RangeInclusive<f32>, scrubber: &Scrubber<'_>) -> Scrub {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let mut out = pxui_types::ffi::PxuiScrubOut::default();
        check(self, unsafe { scrub_lib::pxui_ui_scrub(self.0.as_ptr(), crate::as_str(label), value, *range.start(), *range.end(), scrubber.step, &mut out) });
        self.a11y_name(&out.node, scrubber.name);
        Scrub { node: out.node, preview: (out.flags & 1 != 0).then_some(out.preview), commit: (out.flags & 2 != 0).then_some(out.commit), held: out.flags & 4 != 0 }
    }

    /// The one body of `checkbox`: the key's label and the text apart in a `PxuiKeyedDesc`, the variant around the call.
    #[track_caller]
    pub(crate) fn checkbox_key(&mut self, key: Key, text: &str, on: bool, checkbox: &Checkbox) -> Response {
        let mut id_label = [0u8; 17];
        let keyed = PxuiKeyedDesc::new(key.label(&mut id_label), crate::clip(text));
        let mut out = PxuiResponse::EMPTY;
        self.with_variant(BodyKind::checkbox, checkbox.variant, &mut |ui| check(ui, unsafe { keyed_lib::pxui_ui_checkbox_keyed(ui.0.as_ptr(), &keyed, on as u8, &mut out) }));
        let mut r = crate::kit_ext::response(&out);
        r.changed = r.clicked;
        r
    }

    /// The one body of `radio`, `tabs` and `dropdown` (`kind` 0, 1, 3 of `pxui_ui_choice_keyed`, `body` their option's kind).
    #[allow(clippy::too_many_arguments)]
    #[track_caller]
    pub(crate) fn choice_key(&mut self, kind: u32, body: BodyKind, key: Key, name: &str, options: &[&str], selected: usize, variant: u16) -> (Response, usize) {
        let mut id_label = [0u8; 17];
        let keyed = PxuiKeyedDesc::new(key.label(&mut id_label), crate::clip(name));
        let (mut out, mut chosen) = (PxuiResponse::EMPTY, 0usize);
        let options = crate::Strs::new(options);
        self.with_variant(body, variant, &mut |ui| check(ui, unsafe { keyed_lib::pxui_ui_choice_keyed(ui.0.as_ptr(), &keyed, kind, options.span(), selected, &mut out, &mut chosen) }));
        let mut r = crate::kit_ext::response(&out);
        r.changed = chosen != selected;
        (r, chosen)
    }

    /// The one body of `slider`.
    #[track_caller]
    pub(crate) fn slider_key(&mut self, key: Key, text: &str, value: f32, min: f32, max: f32, variant: u16) -> (Response, f32) {
        let mut id_label = [0u8; 17];
        let keyed = PxuiKeyedDesc::new(key.label(&mut id_label), crate::clip(text));
        let (mut out, mut v) = (PxuiResponse::EMPTY, 0.0f32);
        self.with_variant(BodyKind::slider, variant, &mut |ui| check(ui, unsafe { keyed_lib::pxui_ui_slider_keyed(ui.0.as_ptr(), &keyed, value, min, max, &mut out, &mut v) }));
        let mut r = crate::kit_ext::response(&out);
        r.changed = v != value;
        (r, v)
    }

    /// The one body of `progress`.
    #[track_caller]
    pub(crate) fn progress_key(&mut self, key: Key, text: &str, value: Option<f32>) -> Handle {
        let mut id_label = [0u8; 17];
        let keyed = PxuiKeyedDesc::new(key.label(&mut id_label), crate::clip(text));
        let mut out = Handle::default();
        check(self, unsafe { keyed_lib::pxui_ui_progress_keyed(self.0.as_ptr(), &keyed, value.is_some() as u8, value.unwrap_or_default(), &mut out) });
        out
    }

    /// The one body of `spinner`.
    #[track_caller]
    pub(crate) fn spinner_key(&mut self, key: Key, name: &str) -> Handle {
        let mut id_label = [0u8; 17];
        let keyed = PxuiKeyedDesc::new(key.label(&mut id_label), crate::clip(name));
        let mut out = Handle::default();
        check(self, unsafe { keyed_lib::pxui_ui_spinner_keyed(self.0.as_ptr(), &keyed, &mut out) });
        out
    }

    /// The one body of `badge`.
    #[track_caller]
    pub(crate) fn badge_key(&mut self, key: Key, name: &str, text: &str) -> Handle {
        let mut id_label = [0u8; 17];
        let keyed = PxuiKeyedDesc::new(key.label(&mut id_label), crate::clip(name));
        let mut out = Handle::default();
        check(self, unsafe { keyed_lib::pxui_ui_badge_keyed(self.0.as_ptr(), &keyed, crate::as_str(text), &mut out) });
        out
    }

    /// Runs `f` with `variant` set for the bodies of `kind` (cleared after); a variant of 0 sets nothing.
    #[track_caller]
    pub(crate) fn with_variant(&mut self, kind: BodyKind, variant: u16, f: &mut dyn FnMut(&mut Ui)) {
        if variant == 0 { return f(self); }
        self.set_variant(kind, variant);
        f(self);
        self.set_variant(kind, 0);
    }


    /// The one body of `search_field`.
    #[track_caller]
    pub(crate) fn search_field_key(&mut self, key: Key, value: &str, field: &SearchField<'_>) -> SearchFieldResponse<'_> {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        #[cfg(feature = "static")]
        let changed = { let mut out = None; check(self, unsafe { pxui_abi_link::trusted::search_field(self.0.as_ptr(), label, field.icon, value, field.placeholder, &mut out) }); out };
        #[cfg(not(feature = "static"))]
        let changed = {
            let mut changed = 0;
            let mut out = Str::default();
            check(self, unsafe { sys::pxui_ui_search_field(self.0.as_ptr(), as_str(label), as_str(field.icon), as_str(value), as_str(field.placeholder), &mut changed, &mut out) });
            (changed != 0).then_some(out)
        };
        let response = self.input_response(changed.is_some(), Some(field.name));
        SearchFieldResponse { response, changed: changed.map(crate::lent_str) }
    }
}
