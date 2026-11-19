use crate::Handle;
use crate::{Key, PickResponse, PxuiBox, Response, Scrubber, Segment, SliderResponse, SearchField, SearchFieldResponse, Ui, check};
use pxui_types::{ffi::{PxuiKeyedDesc, PxuiResponse, Slice}, theme::BodyKind, ui::Checkbox, Str};

library!(keyed_lib {
    fn pxui_ui_checkbox(ui: *mut PxuiUi, desc: *const pxui_types::ffi::PxuiCheckboxDesc, out: *mut PxuiResponse) -> i32;
    fn pxui_ui_section_index_result(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, letters: Slice<Str>, out: *mut PxuiResponse, result: *mut pxui_types::ffi::PxuiSegmentedResult) -> i32;
    fn pxui_ui_disclosure(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, open: u8, build: pxui_types::ffi::UiBuild, user: *mut core::ffi::c_void, out: *mut PxuiResponse) -> i32;
    fn pxui_ui_slider(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, value: f32, min: f32, max: f32, out: *mut PxuiResponse, result: *mut pxui_types::ffi::PxuiScrubOut) -> i32;
    fn pxui_ui_pick(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, kind: u32, items: Slice<pxui_types::ffi::PxuiPickItem>, has_selected: u8, selected: usize, out: *mut PxuiResponse, result: *mut pxui_types::ffi::PxuiSegmentedResult) -> i32;
    fn pxui_ui_tabs(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, items: Slice<pxui_types::ffi::PxuiPickItem>, has_selected: u8, selected: usize, focus_on_click: u8, out: *mut PxuiResponse, result: *mut pxui_types::ffi::PxuiSegmentedResult) -> i32;
    fn pxui_ui_progress(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, has_value: u8, value: f32, out: *mut PxuiBox) -> i32;
    fn pxui_ui_spinner(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, out: *mut PxuiBox) -> i32;
    fn pxui_ui_badge(ui: *mut PxuiUi, keyed: *const PxuiKeyedDesc, value: Str, out: *mut PxuiBox) -> i32;
});

library!(scrub_lib {
    fn pxui_ui_scrub(ui: *mut PxuiUi, label: pxui_types::Str, value: f32, min: f32, max: f32, step: f32, response: *mut PxuiResponse, out: *mut pxui_types::ffi::PxuiScrubOut) -> i32;
});

#[cfg(not(feature = "static"))]
mod sys {
    use pxui_types::Str;
    use crate::sys::PxuiUi;
    #[link(name = "pxui")]
    unsafe extern "C" {
    pub fn pxui_ui_search_field_response(ui: *mut PxuiUi, label: Str, icon: Str, value: Str, placeholder: Str, name: Str, action: u8, changed: *mut u8, out: *mut Str, response: *mut pxui_types::ffi::PxuiResponse) -> i32;
    }
}


#[cfg(not(feature = "static"))]
#[inline]
fn as_str(value: &str) -> Str { crate::as_str(value) }

/// A slider or scrubber's result from its response and phase record (flags 1 preview, 2 commit); `value` is shown when neither.
fn slider_response(out: &PxuiResponse, phases: &pxui_types::ffi::PxuiScrubOut, value: f32) -> SliderResponse {
    let preview = (phases.flags & 1 != 0).then_some(phases.preview);
    let commit = (phases.flags & 2 != 0).then_some(phases.commit);
    let mut response = crate::kit_ext::response(out);
    response.changed = commit.is_some();
    SliderResponse { response, value: preview.unwrap_or(value), preview, commit }
}

impl Ui {

    /// The body of `radio`: `pick_one_key` of kind 0, one copy per kind so the engine's pick reads the items in line.
    #[track_caller]
    #[inline]
    pub(crate) fn radio_key(&mut self, key: Key, name: &str, items: &[Segment<'_>], selected: Option<usize>, variant: u16) -> PickResponse {
        self.pick_one_key(0, key, name, items, selected, variant, false)
    }

    /// The body of `tabs`: `pick_one_key` of kind 1 (as `radio_key`).
    #[track_caller]
    #[inline]
    pub(crate) fn tabs_key(&mut self, key: Key, name: &str, items: &[Segment<'_>], selected: Option<usize>, variant: u16, focus: bool) -> PickResponse {
        self.pick_one_key(1, key, name, items, selected, variant, focus)
    }

    /// The body of `dropdown`: `pick_one_key` of kind 3 (as `radio_key`).
    #[track_caller]
    #[inline]
    pub(crate) fn dropdown_key(&mut self, key: Key, text: &str, items: &[Segment<'_>], selected: Option<usize>, variant: u16) -> PickResponse {
        self.pick_one_key(3, key, text, items, selected, variant, false)
    }

    /// The one body of `radio` (`kind` 0), `tabs` (1) and `dropdown` (3): one library call with the items as the export's items column
    /// (on the stack up to 32), the variant around it; `focus` (tabs only) makes a press on a tab focus the bar.
    #[allow(clippy::too_many_arguments)]
    #[track_caller]
    #[inline(always)]
    pub(crate) fn pick_one_key(&mut self, kind: u32, key: Key, name: &str, items: &[Segment<'_>], selected: Option<usize>, variant: u16, focus: bool) -> PickResponse {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let body = match kind { 0 => BodyKind::radio, 1 => BodyKind::tabs, _ => BodyKind::dropdown };
        // linked statically, the library's trusted entry with the items as they are, reporting the engine's response; else the keyed export
        #[cfg(feature = "static")]
        let (mut response, chosen, secondary) = {
            let (mut out, mut chosen, mut secondary) = (pxui_abi_link::trusted::EngineResponse::default(), None, None);
            self.variant_on(body, variant);
            check(self, unsafe { pxui_abi_link::trusted::pick(self.0.as_ptr(), kind, label, name, items, selected, focus, &mut out, &mut chosen, &mut secondary) });
            self.variant_off(body, variant);
            (crate::kit_ext::engine_response(&out), chosen, secondary)
        };
        #[cfg(not(feature = "static"))]
        let (mut response, chosen, secondary) = {
            let mut out = PxuiResponse::EMPTY;
            let keyed = PxuiKeyedDesc::new(label, crate::clip(name));
            let mut list = crate::PickItems::new();
            list.fill(items);
            let mut result = pxui_types::ffi::PxuiSegmentedResult::EMPTY;
            let (has, at) = (selected.is_some() as u8, selected.unwrap_or(0));
            self.variant_on(body, variant);
            check(self, unsafe {
                if kind == 1 { keyed_lib::pxui_ui_tabs(self.0.as_ptr(), &keyed, list.span(), has, at, focus as u8, &mut out, &mut result) }
                else { keyed_lib::pxui_ui_pick(self.0.as_ptr(), &keyed, kind, list.span(), has, at, &mut out, &mut result) }
            });
            self.variant_off(body, variant);
            (crate::kit_ext::response(&out), (result.has_chosen != 0).then_some(result.chosen as usize), (result.has_secondary != 0).then_some(result.secondary as usize))
        };
        response.changed = chosen != selected.filter(|_| !items.is_empty()).map(|i| i.min(items.len() - 1));
        PickResponse { response, chosen, chosen_id: chosen.and_then(|i| items.get(i)?.id), secondary }
    }

    /// The one body of `scrub`.
    #[track_caller]
    pub(crate) fn scrub_key(&mut self, key: Key, value: f32, range: core::ops::RangeInclusive<f32>, scrubber: &Scrubber<'_>) -> SliderResponse {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let (mut response, mut out) = (PxuiResponse::EMPTY, pxui_types::ffi::PxuiScrubOut::default());
        check(self, unsafe { scrub_lib::pxui_ui_scrub(self.0.as_ptr(), crate::as_str(label), value, *range.start(), *range.end(), scrubber.step, &mut response, &mut out) });
        self.a11y_name(&out.node, scrubber.name);
        slider_response(&response, &out, value.clamp(*range.start(), *range.end()))
    }

    /// The one body of `checkbox`: linked statically, the library's trusted entry with the style as it is; else the style, key and text
    /// in one `PxuiCheckboxDesc` (the looks as six look records on the stack). The variant around the call.
    #[track_caller]
    pub(crate) fn checkbox_key(&mut self, key: Key, text: &str, on: bool, checkbox: &Checkbox<'_>) -> Response {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        #[cfg(feature = "static")]
        let mut r = {
            let mut out = pxui_abi_link::trusted::EngineResponse::default();
            self.variant_on(BodyKind::checkbox, checkbox.variant);
            check(self, unsafe { pxui_abi_link::trusted::checkbox(self.0.as_ptr(), label, text, on, checkbox, &mut out) });
            self.variant_off(BodyKind::checkbox, checkbox.variant);
            crate::kit_ext::engine_response(&out)
        };
        #[cfg(not(feature = "static"))]
        let mut r = {
            let mut out = PxuiResponse::EMPTY;
            let look = checkbox.look.map(crate::kit_ext::six);
            let desc = pxui_types::ffi::PxuiCheckboxDesc {
                size: core::mem::size_of::<pxui_types::ffi::PxuiCheckboxDesc>() as u32, disabled: checkbox.disabled as u8, on: on as u8, passive: checkbox.passive as u8,
                reserved: 0, box_size: checkbox.size.unwrap_or(0.0), look: look.as_ref().map_or(core::ptr::null(), |l| l.as_ptr()), key: crate::as_str(label), text: crate::as_str(crate::clip(text)),
            };
            self.variant_on(BodyKind::checkbox, checkbox.variant);
            check(self, unsafe { keyed_lib::pxui_ui_checkbox(self.0.as_ptr(), &desc, &mut out) });
            self.variant_off(BodyKind::checkbox, checkbox.variant);
            crate::kit_ext::response(&out)
        };
        r.changed = r.clicked;
        r
    }

    /// The one body of `section_index`: one library call with the letters as a span (on the stack up to 32) and the name in the keyed
    /// descriptor.
    #[track_caller]
    pub(crate) fn section_index_key(&mut self, key: Key, letters: &[&str], index: &crate::SectionIndex<'_>) -> PickResponse {
        let (name, variant) = (index.name, index.variant);
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let mut out = PxuiResponse::EMPTY;
        #[cfg(feature = "static")]
        let chosen = {
            let mut chosen = None;
            self.variant_on(BodyKind::section_index, variant);
            check(self, unsafe { pxui_abi_link::trusted::section_index(self.0.as_ptr(), label, name, letters, &mut out, &mut chosen) });
            self.variant_off(BodyKind::section_index, variant);
            chosen
        };
        #[cfg(not(feature = "static"))]
        let chosen = {
            let keyed = PxuiKeyedDesc::new(label, crate::clip(name));
            let letters = crate::Strs::new(letters);
            let mut result = pxui_types::ffi::PxuiSegmentedResult::EMPTY;
            self.variant_on(BodyKind::section_index, variant);
            check(self, unsafe { keyed_lib::pxui_ui_section_index_result(self.0.as_ptr(), &keyed, letters.span(), &mut out, &mut result) });
            self.variant_off(BodyKind::section_index, variant);
            (result.has_chosen != 0).then_some(result.chosen as usize)
        };
        let mut response = crate::kit_ext::response(&out);
        response.changed = chosen.is_some();
        PickResponse { response, chosen, chosen_id: None, secondary: None }
    }

    /// The one body of `disclosure`: the section engine with the caller's `open`, the variant given to the `inspector_section` body.
    #[track_caller]
    pub(crate) fn disclosure_key(&mut self, key: Key, text: &str, open: bool, variant: u16, mut f: &mut dyn FnMut(&mut Ui)) -> Response {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let mut out = PxuiResponse::EMPTY;
        #[cfg(feature = "static")]
        {
            self.variant_on(BodyKind::inspector_section, variant);
            check(self, unsafe { let outer = self.0.as_ptr(); pxui_abi_link::trusted::disclosure(outer, label, text, open, inner!(outer, |ui: &mut Ui| f(ui)), &mut out) });
            self.variant_off(BodyKind::inspector_section, variant);
        }
        #[cfg(not(feature = "static"))]
        {
            let keyed = PxuiKeyedDesc::new(label, crate::clip(text));
            self.variant_on(BodyKind::inspector_section, variant);
            check(self, unsafe { keyed_lib::pxui_ui_disclosure(self.0.as_ptr(), &keyed, open as u8, Some(crate::composite::call), &mut f as *mut _ as *mut core::ffi::c_void, &mut out) });
            self.variant_off(BodyKind::inspector_section, variant);
        }
        let mut r = crate::kit_ext::response(&out);
        r.changed = r.clicked;
        r
    }

    /// The one body of `slider`.
    #[track_caller]
    pub(crate) fn slider_key(&mut self, key: Key, text: &str, value: f32, range: core::ops::RangeInclusive<f32>, slider: &crate::Slider) -> SliderResponse {
        let (min, max, variant) = (*range.start(), *range.end(), slider.variant);
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        // linked statically, the engine's own record as it is
        #[cfg(feature = "static")]
        let mut r = {
            let mut out = pxui_abi_link::trusted::SliderResult::default();
            self.variant_on(BodyKind::slider, variant);
            check(self, unsafe { pxui_abi_link::trusted::slider_engine(self.0.as_ptr(), label, text, value, min, max, &pxui_types::ui::Slider::DEFAULT, &mut out) });
            self.variant_off(BodyKind::slider, variant);
            let (r, v, preview, commit) = out;
            SliderResponse { response: crate::kit_ext::engine_response(&r), value: preview.unwrap_or(v), preview, commit }
        };
        #[cfg(not(feature = "static"))]
        let mut r = {
            let (mut out, mut phases) = (PxuiResponse::EMPTY, pxui_types::ffi::PxuiScrubOut::default());
            let keyed = PxuiKeyedDesc::new(label, crate::clip(text));
            self.variant_on(BodyKind::slider, variant);
            check(self, unsafe { keyed_lib::pxui_ui_slider(self.0.as_ptr(), &keyed, value, min, max, &mut out, &mut phases) });
            self.variant_off(BodyKind::slider, variant);
            slider_response(&out, &phases, phases.preview)
        };
        r.response.changed = r.value != value;
        r
    }

    /// The one body of `progress`.
    #[track_caller]
    pub(crate) fn progress_key(&mut self, key: Key, text: &str, value: Option<f32>, variant: u16) -> Handle {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let mut out = Handle::default();
        #[cfg(feature = "static")]
        {
            self.variant_on(BodyKind::progress, variant);
            check(self, unsafe { pxui_abi_link::trusted::progress(self.0.as_ptr(), label, text, value, &mut out) });
            self.variant_off(BodyKind::progress, variant);
        }
        #[cfg(not(feature = "static"))]
        {
            let keyed = PxuiKeyedDesc::new(label, crate::clip(text));
            self.variant_on(BodyKind::progress, variant);
            check(self, unsafe { keyed_lib::pxui_ui_progress(self.0.as_ptr(), &keyed, value.is_some() as u8, value.unwrap_or_default(), &mut out) });
            self.variant_off(BodyKind::progress, variant);
        }
        out
    }

    /// The one body of `spinner`.
    #[track_caller]
    pub(crate) fn spinner_key(&mut self, key: Key, name: &str, variant: u16) -> Handle {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let mut out = Handle::default();
        #[cfg(feature = "static")]
        {
            self.variant_on(BodyKind::spinner, variant);
            check(self, unsafe { pxui_abi_link::trusted::spinner(self.0.as_ptr(), label, name, &mut out) });
            self.variant_off(BodyKind::spinner, variant);
        }
        #[cfg(not(feature = "static"))]
        {
            let keyed = PxuiKeyedDesc::new(label, crate::clip(name));
            self.variant_on(BodyKind::spinner, variant);
            check(self, unsafe { keyed_lib::pxui_ui_spinner(self.0.as_ptr(), &keyed, &mut out) });
            self.variant_off(BodyKind::spinner, variant);
        }
        out
    }

    /// The one body of `badge`.
    #[track_caller]
    pub(crate) fn badge_key(&mut self, key: Key, name: &str, text: &str, variant: u16) -> Handle {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let mut out = Handle::default();
        #[cfg(feature = "static")]
        {
            self.variant_on(BodyKind::badge, variant);
            check(self, unsafe { pxui_abi_link::trusted::badge(self.0.as_ptr(), label, name, text, &mut out) });
            self.variant_off(BodyKind::badge, variant);
        }
        #[cfg(not(feature = "static"))]
        {
            let keyed = PxuiKeyedDesc::new(label, crate::clip(name));
            self.variant_on(BodyKind::badge, variant);
            check(self, unsafe { keyed_lib::pxui_ui_badge(self.0.as_ptr(), &keyed, crate::as_str(text), &mut out) });
            self.variant_off(BodyKind::badge, variant);
        }
        out
    }

    /// Sets `variant` for the bodies of `kind` around a call (`variant_off` clears it after); a variant of 0 sets nothing, in line.
    #[inline]
    pub(crate) fn variant_on(&mut self, kind: BodyKind, variant: u16) { if variant != 0 { self.set_variant(kind, variant); } }
    /// Clears the variant `variant_on` set.
    #[inline]
    pub(crate) fn variant_off(&mut self, kind: BodyKind, variant: u16) { if variant != 0 { self.set_variant(kind, 0); } }


    /// The one body of `search_field`.
    #[track_caller]
    pub(crate) fn search_field_key(&mut self, key: Key, value: &str, field: &SearchField<'_>) -> SearchFieldResponse<'_> {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let mut response = pxui_types::ffi::PxuiResponse::EMPTY;
        self.variant_on(BodyKind::search_field, field.variant);
        #[cfg(feature = "static")]
        let changed = { let mut out = None; check(self, unsafe { pxui_abi_link::trusted::search_field(self.0.as_ptr(), label, field.icon, value, field.placeholder, field.traits, &mut out, Some((field.name, &mut response))) }); out };
        #[cfg(not(feature = "static"))]
        let changed = {
            let mut changed = 0;
            let mut out = Str::default();
            check(self, unsafe { sys::pxui_ui_search_field_response(self.0.as_ptr(), as_str(label), as_str(field.icon), as_str(value), as_str(field.placeholder), as_str(field.name), field.traits.return_key() as u8, &mut changed, &mut out, &mut response) });
            (changed != 0).then_some(out)
        };
        self.variant_off(BodyKind::search_field, field.variant);
        SearchFieldResponse { response: crate::text_inputs::input_response(&response, changed.is_some()), changed: changed.map(crate::lent_str) }
    }
}
