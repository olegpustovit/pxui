use crate::{PxuiBox, Ui, check};

library!(scrub_lib {
    fn pxui_ui_scrub(ui: *mut PxuiUi, label: pxui_types::Str, value: f32, min: f32, max: f32, step: f32, out: *mut pxui_types::ffi::PxuiScrubOut) -> i32;
});

#[derive(Clone, Copy, Debug)]
pub struct Scrub {
    pub node: PxuiBox,
    pub preview: Option<f32>,
    pub commit: Option<f32>,
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
    pub fn pxui_ui_segmented(ui: *mut PxuiUi, label: Str, options: Slice<Str>, selected: usize, out: *mut usize) -> i32;
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
fn as_str(value: &str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }

impl Ui {
    /// Behavior for a caller-painted scrubber. Pointer capture previews until release; routed
    /// arrows/Page/Home/End and accessibility adjustments take over capture and commit. A collapsed range is
    /// read-only. Use `set_node_layout` to size the returned node and `draw` to paint its body.
    #[track_caller]
    pub fn scrub(&mut self, label: &str, value: f32, range: core::ops::RangeInclusive<f32>, step: f32) -> Scrub {
        let mut out = pxui_types::ffi::PxuiScrubOut::default();
        check(self, unsafe { scrub_lib::pxui_ui_scrub(self.0.as_ptr(), crate::as_str(label), value, *range.start(), *range.end(), step, &mut out) });
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
    #[track_caller] pub fn segmented(&mut self, label: &str, options: &[&str], selected: usize) -> usize {
        #[cfg(feature = "static")]
        { let mut out = 0; check(self, unsafe { pxui_abi_link::trusted::choice(self.0.as_ptr(), 2, label, options, selected, &mut out) }); out }
        #[cfg(not(feature = "static"))]
        {
            let options = Strs::new(options);
            let mut out = 0;
            check(self, unsafe { sys::pxui_ui_segmented(self.0.as_ptr(), as_str(label), options.span(), selected, &mut out) });
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

    /// A search field: the text it changed to, or none; borrowed from the library, valid until the next call on `ui`.
    ///
    /// Allocates: nothing once warm; the changed text is copied into buffers the library reuses.
    /// Identity: `label` keys the field; fields with the same label in one parent collide.
    /// Panics: when the `Ui` has failed, like every `Ui` call.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new().unwrap();
    /// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
    ///     assert_eq!(ui.search_field("find", "?", "", "Search"), None);
    /// }).unwrap();
    /// ```
    #[inline]
    #[track_caller] pub fn search_field(&mut self, label: &str, icon: &str, value: &str, placeholder: &str) -> Option<&str> {
        #[cfg(feature = "static")]
        { let mut out = None; check(self, unsafe { pxui_abi_link::trusted::search_field(self.0.as_ptr(), label, icon, value, placeholder, &mut out) }); out.map(crate::lent_str) }
        #[cfg(not(feature = "static"))]
        {
            let mut changed = 0;
            let mut out = Str::default();
            check(self, unsafe { sys::pxui_ui_search_field(self.0.as_ptr(), as_str(label), as_str(icon), as_str(value), as_str(placeholder), &mut changed, &mut out) });
            if changed == 0 { return None; }
            Some(crate::lent_str(out))
        }
    }
}
