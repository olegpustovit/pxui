use crate::{PxuiBox, Str, Ui, check};

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::pxui_ui_a11y_name;
}

#[cfg(not(feature = "static"))]
mod sys {
    use crate::PxuiBox;
    use crate::sys::PxuiUi;
    use pxui_types::Str;

    #[link(name = "pxui")]
    unsafe extern "C" {
        pub fn pxui_ui_a11y_name(ui: *mut PxuiUi, box_: *const PxuiBox, name: Str) -> i32;
    }
}

impl Ui {
    /// Sets the accessible name for a box declared in the current UI frame.
    #[inline]
    #[track_caller] pub fn a11y_name(&mut self, box_: &PxuiBox, name: &str)  {
        let name = Str { ptr: name.as_ptr(), len: name.len() };
        let rc = unsafe { sys::pxui_ui_a11y_name(self.0.as_ptr(), box_, name) };
        check(self, rc)
    }
}
