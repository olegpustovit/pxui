use crate::{PxuiBox, Str, Ui, check};

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{pxui_a11y_fd, pxui_a11y_pump, pxui_a11y_start, pxui_a11y_stop, pxui_a11y_update, pxui_ui_a11y_name, PxuiA11y};
}

#[cfg(not(feature = "static"))]
mod sys {
    use crate::PxuiBox;
    use crate::sys::PxuiUi;
    use pxui_types::{ffi::A11yTree, Str};

    #[repr(C)] pub struct PxuiA11y { _private: [u8; 0] }
    #[link(name = "pxui")]
    unsafe extern "C" {
        pub fn pxui_a11y_start(name: Str) -> *mut PxuiA11y;
        pub fn pxui_a11y_fd(a11y: *mut PxuiA11y) -> i32;
        pub fn pxui_a11y_update(a11y: *mut PxuiA11y, tree: *const A11yTree) -> i32;
        pub fn pxui_a11y_pump(a11y: *mut PxuiA11y, out: *mut pxui_types::ffi::A11yActionSpan) -> i32;
        pub fn pxui_a11y_stop(a11y: *mut PxuiA11y);
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

/// The accessibility bridge of an app's window (Linux: AT-SPI2 through the library). Elsewhere `connect` is `None`: the platform's
/// bridge belongs to the window host.
pub struct A11yBridge(core::ptr::NonNull<sys::PxuiA11y>);

impl A11yBridge {
    /// Connects to the accessibility bus as `name`; `None` when there is none.
    pub fn connect(name: &str) -> Option<Self> {
        core::ptr::NonNull::new(unsafe { sys::pxui_a11y_start(Str { ptr: name.as_ptr(), len: name.len() }) }).map(Self)
    }
    /// The descriptor the bridge's traffic wakes: add it to the event loop.
    pub fn fd(&self) -> i32 { unsafe { sys::pxui_a11y_fd(self.0.as_ptr()) } }
    /// Publishes a frame's accessibility tree (`Ui::output().a11y`).
    pub fn update(&mut self, tree: &pxui_types::ffi::A11yTree) -> bool { unsafe { sys::pxui_a11y_update(self.0.as_ptr(), tree) == 0 } }
    /// Handles pending traffic and appends the actions assistive technology asked for to `out`; false when the connection is gone.
    pub fn pump(&mut self, out: &mut Vec<pxui_types::ffi::A11yAction>) -> bool {
        let mut actions = pxui_types::ffi::Slice::default();
        let alive = unsafe { sys::pxui_a11y_pump(self.0.as_ptr(), &mut actions) } == 1;
        if actions.len != 0 { out.extend_from_slice(unsafe { core::slice::from_raw_parts(actions.ptr, actions.len) }); }
        alive
    }
}

impl Drop for A11yBridge { fn drop(&mut self) { unsafe { sys::pxui_a11y_stop(self.0.as_ptr()) } } }
