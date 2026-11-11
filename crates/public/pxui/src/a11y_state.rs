use crate::{PxuiBox, Ui};

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{pxui_ui_a11y_number, pxui_ui_set_role};
}
#[cfg(not(feature = "static"))]
mod sys {
    use crate::{PxuiBox, sys::PxuiUi};
    #[link(name = "pxui")]
    unsafe extern "C" {
        pub fn pxui_ui_a11y_number(ui: *mut PxuiUi, box_: *const PxuiBox, now: f64, min: f64, max: f64, step: f64) -> i32;
        pub fn pxui_ui_set_role(ui: *mut PxuiUi, box_: *const PxuiBox, role: u32, states: u32) -> i32;
    }
}

impl Ui {
    #[inline]
    #[track_caller] pub fn a11y_number(&mut self, box_: &PxuiBox, now: f64, min: f64, max: f64, step: f64)  {
        let rc = unsafe { sys::pxui_ui_a11y_number(self.0.as_ptr(), box_, now, min, max, step) };
        crate::live(self, rc);
    }
    #[inline]
    #[track_caller] pub fn set_role(&mut self, box_: &PxuiBox, role: pxui_types::a11y::Role, states: u32)  {
        let rc = unsafe { sys::pxui_ui_set_role(self.0.as_ptr(), box_, role as u32, states) };
        crate::live(self, rc);
    }
}
