use crate::{Key, PxuiBox, Scrubber, SearchField, SearchFieldResponse, Ui, check};

library!(scrub_lib {
    fn pxui_ui_scrub(ui: *mut PxuiUi, label: pxui_types::Str, value: f32, min: f32, max: f32, step: f32, out: *mut pxui_types::ffi::PxuiScrubOut) -> i32;
});

/// What `Ui::scrub` reports for a caller-painted scrubber.
#[derive(Clone, Copy, Debug)]
pub struct Scrub {
    /// The scrubber's node, valid this frame: size it with `set_node_layout` and paint it with `draw`.
    pub node: PxuiBox,
    /// The provisional value while the pointer drags, or none.
    pub preview: Option<f32>,
    /// The final value: the pointer's release, or a key or accessibility adjustment; none otherwise.
    pub commit: Option<f32>,
    /// The pointer holds the scrubber.
    pub held: bool,
}

#[cfg(not(feature = "static"))]
use crate::Strs;
#[cfg(not(feature = "static"))]
use pxui_types::Str;

#[cfg(not(feature = "static"))]
mod sys {
    use pxui_types::{PxuiBox, Str, ffi::Slice};
    use crate::sys::PxuiUi;
    #[link(name = "pxui")]
    unsafe extern "C" {
    pub fn pxui_ui_checkbox(ui: *mut PxuiUi, label: Str, value: u8, out: *mut u8) -> i32;
    pub fn pxui_ui_radio(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32;
    pub fn pxui_ui_tabs(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32;
    pub fn pxui_ui_dropdown(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32;
    pub fn pxui_ui_slider(ui: *mut PxuiUi, label: Str, value: f32, min: f32, max: f32, out: *mut f32) -> i32;
    pub fn pxui_ui_progress(ui: *mut PxuiUi, label: Str, has_value: u8, value: f32, out: *mut PxuiBox) -> i32;
    pub fn pxui_ui_spinner(ui: *mut PxuiUi, label: Str, out: *mut PxuiBox) -> i32;
    pub fn pxui_ui_badge(ui: *mut PxuiUi, label: Str, value: Str, out: *mut PxuiBox) -> i32;
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

    #[inline]
    #[track_caller] pub fn checkbox(&mut self, label: &str, value: bool) -> bool {
        #[cfg(feature = "static")]
        { let mut out = false; check(self, unsafe { pxui_abi_link::trusted::checkbox(self.0.as_ptr(), label, value, &mut out) }); out }
        #[cfg(not(feature = "static"))]
        { let mut out = 0; check(self, unsafe { sys::pxui_ui_checkbox(self.0.as_ptr(), as_str(label), value as u8, &mut out) }); out != 0 }
    }

    #[inline]
    #[track_caller] pub fn radio(&mut self, label: &str, options: &[&str], selected: usize) -> usize {
        #[cfg(feature = "static")]
        { let mut out = 0; check(self, unsafe { pxui_abi_link::trusted::choice(self.0.as_ptr(), 0, label, options, selected, &mut out) }); out }
        #[cfg(not(feature = "static"))]
        {
            let options = Strs::new(options);
            let mut out = 0;
            check(self, unsafe { sys::pxui_ui_radio(self.0.as_ptr(), as_str(label), options.span(), selected, &mut out) });
            out
        }
    }

    #[inline]
    #[track_caller] pub fn tabs(&mut self, label: &str, tabs: &[&str], selected: usize) -> usize {
        #[cfg(feature = "static")]
        { let mut out = 0; check(self, unsafe { pxui_abi_link::trusted::choice(self.0.as_ptr(), 1, label, tabs, selected, &mut out) }); out }
        #[cfg(not(feature = "static"))]
        {
            let tabs = Strs::new(tabs);
            let mut out = 0;
            check(self, unsafe { sys::pxui_ui_tabs(self.0.as_ptr(), as_str(label), tabs.span(), selected, &mut out) });
            out
        }
    }

    #[inline]
    #[track_caller] pub fn dropdown(&mut self, label: &str, options: &[&str], selected: usize) -> usize {
        #[cfg(feature = "static")]
        { let mut out = 0; check(self, unsafe { pxui_abi_link::trusted::choice(self.0.as_ptr(), 3, label, options, selected, &mut out) }); out }
        #[cfg(not(feature = "static"))]
        {
            let options = Strs::new(options);
            let mut out = 0;
            check(self, unsafe { sys::pxui_ui_dropdown(self.0.as_ptr(), as_str(label), options.span(), selected, &mut out) });
            out
        }
    }

    #[inline]
    #[track_caller] pub fn slider(&mut self, label: &str, value: f32, range: core::ops::RangeInclusive<f32>) -> f32 {
        let mut out = 0.0;
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::slider(self.0.as_ptr(), label, value, *range.start(), *range.end(), &mut out) });
        #[cfg(not(feature = "static"))]
        check(self, unsafe { sys::pxui_ui_slider(self.0.as_ptr(), as_str(label), value, *range.start(), *range.end(), &mut out) });
        out
    }

    #[inline]
    #[track_caller] pub fn progress(&mut self, label: &str, value: Option<f32>) -> PxuiBox {
        let mut out = PxuiBox::default();
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::progress(self.0.as_ptr(), label, value, &mut out) });
        #[cfg(not(feature = "static"))]
        check(self, unsafe { sys::pxui_ui_progress(self.0.as_ptr(), as_str(label), value.is_some() as u8, value.unwrap_or_default(), &mut out) });
        out
    }

    #[inline]
    #[track_caller] pub fn spinner(&mut self, label: &str) -> PxuiBox {
        let mut out = PxuiBox::default();
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::spinner(self.0.as_ptr(), label, &mut out) });
        #[cfg(not(feature = "static"))]
        check(self, unsafe { sys::pxui_ui_spinner(self.0.as_ptr(), as_str(label), &mut out) });
        out
    }


    #[inline]
    #[track_caller] pub fn badge(&mut self, label: &str, value: &str) -> PxuiBox {
        let mut out = PxuiBox::default();
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::badge(self.0.as_ptr(), label, value, &mut out) });
        #[cfg(not(feature = "static"))]
        check(self, unsafe { sys::pxui_ui_badge(self.0.as_ptr(), as_str(label), as_str(value), &mut out) });
        out
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
