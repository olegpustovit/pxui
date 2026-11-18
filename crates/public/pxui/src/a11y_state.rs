use crate::Handle;
use crate::{Ui};

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{pxui_ui_a11y_number_checked, pxui_ui_set_role};
}
#[cfg(not(feature = "static"))]
mod sys {
    use crate::{PxuiBox, sys::PxuiUi};
    #[link(name = "pxui")]
    unsafe extern "C" {
        /// The C entry `Ui::a11y_number` calls: sets a box's accessible number; an error code for a null box, a non-finite value,
        /// `now` outside `min..=max` or a negative `step`, and the stale code for a dead box.
        ///
        /// Allocates: never.
        /// Panics: never.
        ///
        /// ```
        /// pxui::doc::frames(1, &mut |ui| { let b = ui.node("vol", pxui::Node::DEFAULT); assert!(ui.a11y_number(&b, 5.0, 0.0, 10.0, 1.0).unwrap()); });
        /// ```
        pub fn pxui_ui_a11y_number_checked(ui: *mut PxuiUi, box_: *const PxuiBox, now: f64, min: f64, max: f64, step: f64) -> i32;
        /// The C entry `Ui::set_role` calls: sets a box's accessibility role and state bits; an error code for a null box or a role past
        /// the last `Role`, and the stale code for a dead box.
        ///
        /// Allocates: never.
        /// Panics: never.
        ///
        /// ```
        /// pxui::doc::frames(1, &mut |ui| { let b = ui.node("go", pxui::Node::DEFAULT); ui.set_role(&b, pxui::Role::Button, 0); });
        /// ```
        pub fn pxui_ui_set_role(ui: *mut PxuiUi, box_: *const PxuiBox, role: u32, states: u32) -> i32;
    }
}

impl Ui {
    /// Sets the accessible number of `box_`: the current value `now` within `min..=max`, moved by `step` (a slider, spin button or progress).
    ///
    /// A number takes precedence over a text value on the same box. Returns `Ok(false)` for
    /// a retired node and `Error::InvalidInput` for nonfinite values, a current value outside
    /// its bounds, or a negative step.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(2, &mut |ui| {
    ///     let b = ui.node("volume", pxui::Node::DEFAULT);
    ///     assert!(ui.a11y_number(&b, 3.0, 0.0, 10.0, 1.0).unwrap());
    /// });
    /// ```
    #[inline]
    pub fn a11y_number(&mut self, box_: &Handle, now: f64, min: f64, max: f64, step: f64) -> Result<bool, crate::Error> {
        let rc = unsafe { sys::pxui_ui_a11y_number_checked(self.0.as_ptr(), box_, now, min, max, step) };
        if rc == pxui_types::PXUI_ERR_STALE { Ok(false) } else { crate::status(rc).map(|()| true) }
    }
    /// Sets the node's role and state flags in the accessibility tree.
    ///
    /// A dead `box_` (from an earlier frame) is ignored.
    /// Allocates: never.
    /// Panics: outside a frame, or when the library refuses the call (a null box).
    ///
    /// ```
    /// use pxui::Role;
    /// let ui = pxui::doc::frames(2, &mut |ui| {
    ///     let b = ui.node("go", pxui::Node::DEFAULT);
    ///     ui.set_role(&b, Role::Button, 0);
    /// });
    /// assert!(ui.a11y_nodes().iter().any(|n| n.role == Role::Button as u32));
    /// ```
    #[inline]
    #[track_caller] pub fn set_role(&mut self, box_: &Handle, role: pxui_types::a11y::Role, states: u32)  {
        let rc = unsafe { sys::pxui_ui_set_role(self.0.as_ptr(), box_, role as u32, states) };
        crate::live(self, rc);
    }
}
