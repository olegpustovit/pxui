use crate::Handle;
use crate::{Ui};

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{pxui_ui_a11y_number, pxui_ui_set_role};
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
        pub fn pxui_ui_a11y_number(ui: *mut PxuiUi, box_: *const PxuiBox, now: f64, min: f64, max: f64, step: f64) -> i32;
        /// The C entry `Ui::set_role` calls: sets a box's accessibility role and state bits; an error code for a null box or a role past
        /// the last `Role`, and the stale code for a dead box.
        ///
        /// Allocates: never.
        /// Panics: never.
        ///
        /// ```
        /// pxui::doc::frames(1, &mut |ui| { let b = ui.node("go", pxui::Node::DEFAULT); ui.set_role(&b, pxui::Role::Button, pxui::A11yStates::NONE); });
        /// ```
        pub fn pxui_ui_set_role(ui: *mut PxuiUi, box_: *const PxuiBox, role: u32, states: u32) -> i32;
    }
}

/// Accessibility state flags authored for a node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct A11yStates(u32);

impl A11yStates {
    /// No explicit accessibility state.
    pub const NONE: A11yStates = A11yStates(0);
    /// Can take keyboard focus.
    pub const FOCUSABLE: A11yStates = A11yStates(pxui_types::a11y::state::FOCUSABLE);
    /// Holds keyboard focus.
    pub const FOCUSED: A11yStates = A11yStates(pxui_types::a11y::state::FOCUSED);
    /// Accepts input (not disabled).
    pub const ENABLED: A11yStates = A11yStates(pxui_types::a11y::state::ENABLED);
    /// Has a checked state (a check box, toggle, radio, checkable menu item).
    pub const CHECKABLE: A11yStates = A11yStates(pxui_types::a11y::state::CHECKABLE);
    /// Is checked (on).
    pub const CHECKED: A11yStates = A11yStates(pxui_types::a11y::state::CHECKED);
    /// Neither checked nor unchecked (a mixed check box).
    pub const MIXED: A11yStates = A11yStates(pxui_types::a11y::state::MIXED);
    /// Can be selected (a list or tree item, a tab).
    pub const SELECTABLE: A11yStates = A11yStates(pxui_types::a11y::state::SELECTABLE);
    /// Is selected.
    pub const SELECTED: A11yStates = A11yStates(pxui_types::a11y::state::SELECTED);
    /// Can expand to show children (a tree item, a dropdown).
    pub const EXPANDABLE: A11yStates = A11yStates(pxui_types::a11y::state::EXPANDABLE);
    /// Is expanded.
    pub const EXPANDED: A11yStates = A11yStates(pxui_types::a11y::state::EXPANDED);
    /// Its text can be edited.
    pub const EDITABLE: A11yStates = A11yStates(pxui_types::a11y::state::EDITABLE);
    /// Its text spans several lines.
    pub const MULTILINE: A11yStates = A11yStates(pxui_types::a11y::state::MULTILINE);
    /// Its value can be read but not changed.
    pub const READ_ONLY: A11yStates = A11yStates(pxui_types::a11y::state::READ_ONLY);
    /// Is on screen.
    pub const VISIBLE: A11yStates = A11yStates(pxui_types::a11y::state::VISIBLE);
    /// Is held down.
    pub const PRESSED: A11yStates = A11yStates(pxui_types::a11y::state::PRESSED);
    /// Blocks input to everything behind it.
    pub const MODAL: A11yStates = A11yStates(pxui_types::a11y::state::MODAL);
    /// The window holding keyboard focus.
    pub const ACTIVE: A11yStates = A11yStates(pxui_types::a11y::state::ACTIVE);
    /// Lays out or moves horizontally (a slider, a scroll bar, a splitter).
    pub const HORIZONTAL: A11yStates = A11yStates(pxui_types::a11y::state::HORIZONTAL);
    /// Lays out or moves vertically.
    pub const VERTICAL: A11yStates = A11yStates(pxui_types::a11y::state::VERTICAL);
    /// Is loading or working; its content may change.
    pub const BUSY: A11yStates = A11yStates(pxui_types::a11y::state::BUSY);
    /// Is the default action (the button Return presses).
    pub const DEFAULT: A11yStates = A11yStates(pxui_types::a11y::state::DEFAULT);

    /// Combines two sets of accessibility state flags.
    ///
    /// The engine still supplies states derived from the live node.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::A11yStates;
    /// const CHECKED: A11yStates = A11yStates::CHECKABLE.with(A11yStates::CHECKED);
    /// ```
    pub const fn with(self, other: A11yStates) -> A11yStates { A11yStates(self.0 | other.0) }

    /// Returns the state mask used by the accessibility wire records.
    ///
    /// Compare it with the mask on a published accessibility node.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui::A11yStates::NONE.bits(), 0);
    /// ```
    pub const fn bits(self) -> u32 { self.0 }
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
        let rc = unsafe { sys::pxui_ui_a11y_number(self.0.as_ptr(), box_, now, min, max, step) };
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
    ///     ui.set_role(&b, Role::Button, pxui::A11yStates::NONE);
    /// });
    /// assert!(ui.a11y_nodes().iter().any(|n| n.role == Role::Button as u32));
    /// ```
    #[inline]
    #[track_caller] pub fn set_role(&mut self, box_: &Handle, role: pxui_types::a11y::Role, states: A11yStates)  {
        let rc = unsafe { sys::pxui_ui_set_role(self.0.as_ptr(), box_, role as u32, states.bits()) };
        crate::live(self, rc);
    }
}
