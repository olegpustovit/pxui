use crate::Handle;
use crate::{Str, Ui};

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{pxui_a11y_fd, pxui_a11y_pump, pxui_a11y_start, pxui_a11y_stop, pxui_a11y_update, pxui_ui_a11y_name, pxui_ui_a11y_value, PxuiA11y};
}

#[cfg(not(feature = "static"))]
mod sys {
    use crate::PxuiBox;
    use crate::sys::PxuiUi;
    use pxui_types::{ffi::A11yTree, Str};

    #[repr(C)] pub struct PxuiA11y { _private: [u8; 0] }
    #[link(name = "pxui")]
    unsafe extern "C" {
        /// The C entry `A11yBridge::connect` calls: connects to the accessibility bus as `name`; null when there is none (always on platforms
        /// whose bridge belongs to the window host).
        ///
        /// The pointer is owned by the caller until `pxui_a11y_stop`.
        /// Allocates: the bridge (Linux); nothing elsewhere.
        /// Panics: never.
        ///
        /// ```
        /// // None without an accessibility bus or off Linux
        /// let bridge = pxui::A11yBridge::connect("doc");
        /// drop(bridge);
        /// ```
        pub fn pxui_a11y_start(name: Str) -> *mut PxuiA11y;
        /// The C entry `A11yBridge::fd` calls: the descriptor the bridge's traffic wakes, or -1 for a null bridge (and off Linux).
        ///
        /// Allocates: never.
        /// Panics: never.
        ///
        /// ```
        /// if let Some(bridge) = pxui::A11yBridge::connect("doc") { assert!(bridge.fd() >= 0); }
        /// ```
        pub fn pxui_a11y_fd(a11y: *mut PxuiA11y) -> i32;
        /// The C entry `A11yBridge::update` calls: publishes `tree` (borrowed for the call); 0 when published, an error code otherwise
        /// (always an argument error off Linux).
        ///
        /// Allocates: what the bridge keeps for the tree.
        /// Panics: never.
        ///
        /// ```
        /// let ui = pxui::doc::frames(1, &mut |ui| { ui.label("hi", pxui::Label::DEFAULT); });
        /// if let Some(mut bridge) = pxui::A11yBridge::connect("doc") { assert!(bridge.update(&ui.output().a11y)); }
        /// ```
        pub fn pxui_a11y_update(a11y: *mut PxuiA11y, surface: u64, tree: *const A11yTree) -> i32;
        /// The C entry `A11yBridge::pump` calls: handles pending bus traffic and writes the requested actions to `out` (borrowed until the
        /// next pump); 1 while the connection is alive, 0 when it is gone, an error code for a bad argument (always off Linux).
        ///
        /// Allocates: the bridge's action buffer, reused between pumps.
        /// Panics: never.
        ///
        /// ```
        /// if let Some(mut bridge) = pxui::A11yBridge::connect("doc") { assert!(bridge.pump(&mut |_action| {})); }
        /// ```
        pub fn pxui_a11y_pump(a11y: *mut PxuiA11y, out: *mut pxui_types::ffi::A11yActionSpan) -> i32;
        /// The C entry `A11yBridge`'s `Drop` calls: disconnects and frees the bridge; null is ignored.
        ///
        /// Allocates: never.
        /// Panics: never.
        ///
        /// ```
        /// drop(pxui::A11yBridge::connect("doc")); // stops the bridge when there is one
        /// ```
        pub fn pxui_a11y_stop(a11y: *mut PxuiA11y);
        /// The C entry `Ui::a11y_name` calls: sets the accessible name of a box (not stale) of the current frame; an error code for a null
        /// or invalid box or name.
        ///
        /// Allocates: grows the node's name string when the name is longer than the one it held.
        /// Panics: never.
        ///
        /// ```
        /// pxui::doc::frames(1, &mut |ui| { let b = ui.node("play", pxui::Node::DEFAULT.role(pxui::Role::Button)); ui.a11y_name(&b, "Play"); });
        /// ```
        pub fn pxui_ui_a11y_name(ui: *mut PxuiUi, box_: *const PxuiBox, name: Str) -> i32;
        /// The C entry `Ui::a11y_value` calls: sets a box's accessible text `value` and selection (`anchor`, `caret` as UTF-8 byte offsets,
        /// stored as character offsets); a stale box returns the stale code, a split character or invalid text an argument error.
        ///
        /// Allocates: grows the node's value string when `value` is longer than the one it held.
        /// Panics: never.
        ///
        /// ```
        /// pxui::doc::frames(1, &mut |ui| { let b = ui.node("field", pxui::Node::DEFAULT); assert!(ui.a11y_value(&b, "héllo", (0, 3)).unwrap()); });
        /// ```
        pub fn pxui_ui_a11y_value(ui:*mut PxuiUi,box_:*const PxuiBox,value:Str,anchor:u32,caret:u32)->i32;
    }
}

impl Ui {
    /// Sets the accessible text value of `box_` and its selection, `(anchor, caret)` as UTF-8 byte offsets into `value`.
    ///
    /// A value set with `a11y_number` on the same box takes precedence in the published tree. The tree reports the selection in Unicode
    /// scalar offsets. An offset past the end of `value` becomes the end, and one inside a character moves back to
    /// its start. Returns `Ok(false)` for a retired node and `Error::InvalidInput` if the value exceeds 64 KiB.
    /// Allocates: grows the node's value string when `value` is longer than the one it held; otherwise never.
    /// Panics: never.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("name", pxui::Node::DEFAULT.role(pxui::Role::TextField));
    ///     assert!(ui.a11y_value(&b, "héllo", (0, 3)).unwrap());
    /// });
    /// assert!(ui.a11y_tree().strings.windows("héllo".len()).any(|w| w == "héllo".as_bytes()));
    /// ```
    pub fn a11y_value(&mut self, box_: &Handle, value: &str, selection: (u32, u32)) -> Result<bool, crate::Error> {
        let boundary=|offset:u32|{let mut at=(offset as usize).min(value.len());while !value.is_char_boundary(at) {at-=1;}at as u32};
        let (anchor,caret)=(boundary(selection.0),boundary(selection.1));
        let value=Str {ptr:value.as_ptr(),len:value.len()};
        let rc = unsafe { sys::pxui_ui_a11y_value(self.0.as_ptr(), box_, value, anchor, caret) };
        if rc == pxui_types::PXUI_ERR_STALE { Ok(false) } else { crate::status(rc).map(|()| true) }
    }
    /// Sets the accessible name (what a screen reader announces) of `box_`, a box declared in the current frame.
    ///
    /// Returns false for a retired node. Names over 64 KiB are clipped at a UTF-8 character boundary.
    /// Allocates: grows the node's name string when `name` is longer than the one it held; otherwise never.
    /// Panics: when the library refuses the Ui or the handle is malformed.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("play", pxui::Node::DEFAULT.role(pxui::Role::Button));
    ///     assert!(ui.a11y_name(&b, "Play"));
    /// });
    /// assert!(ui.a11y_tree().strings.windows(4).any(|w| w == b"Play"));
    /// ```
    #[inline]
    #[track_caller] pub fn a11y_name(&mut self, box_: &Handle, name: &str) -> bool {
        let rc = unsafe { sys::pxui_ui_a11y_name(self.0.as_ptr(), box_, crate::as_str(name)) };
        crate::live(self, rc)
    }
}

/// The accessibility bridge of an app's window (Linux: AT-SPI2 through the library). Elsewhere `connect` is `None`: the platform's
/// bridge belongs to the window host.
pub struct A11yBridge(core::ptr::NonNull<sys::PxuiA11y>);

impl A11yBridge {
    /// Connects to the accessibility bus as application `name`; `None` when there is no bus, and always `None` off Linux.
    ///
    /// Allocates: the bridge (Linux).
    /// Panics: never.
    ///
    /// ```
    /// // None on a machine without an accessibility bus
    /// let _bridge = pxui::A11yBridge::connect("doc-example");
    /// ```
    pub fn connect(name: &str) -> Option<Self> {
        core::ptr::NonNull::new(unsafe { sys::pxui_a11y_start(Str { ptr: name.as_ptr(), len: name.len() }) }).map(Self)
    }
    /// The file descriptor that becomes readable when the bridge has traffic: add it to the event loop, then call `pump`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// if let Some(bridge) = pxui::A11yBridge::connect("doc-example") { assert!(bridge.fd() >= 0); }
    /// ```
    pub fn fd(&self) -> i32 { unsafe { sys::pxui_a11y_fd(self.0.as_ptr()) } }
    /// Publishes a frame's accessibility tree (`Ui::output().a11y`) to assistive technology; false when the bridge refused it.
    ///
    /// The tree is read during the call only.
    /// Allocates: what the bridge keeps for the tree.
    /// Panics: never.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |ui| { ui.label("hi", pxui::Label::DEFAULT); });
    /// if let Some(mut bridge) = pxui::A11yBridge::connect("doc-example") { assert!(bridge.update(&ui.output().a11y)); }
    /// ```
    pub fn update(&mut self, tree: &pxui_types::ffi::A11yTree) -> bool { unsafe { sys::pxui_a11y_update(self.0.as_ptr(), pxui_types::FrameIn::MAIN, tree) == 0 } }
    /// Handles pending traffic and passes each action assistive technology asked for to `out`, in order; false when the connection is gone.
    ///
    /// The actions are meant for `Ui::inject_a11y`. Call it when `fd` is readable.
    /// Allocates: never when warm (the bridge reuses one action buffer).
    /// Panics: never (unless `out` does).
    ///
    /// ```
    /// if let Some(mut bridge) = pxui::A11yBridge::connect("doc-example") {
    ///     let alive = bridge.pump(&mut |_action| {});
    ///     assert!(alive);
    /// }
    /// ```
    pub fn pump(&mut self, out: &mut dyn FnMut(pxui_types::ffi::A11yAction)) -> bool {
        let mut actions = pxui_types::ffi::Strided::default();
        let alive = unsafe { sys::pxui_a11y_pump(self.0.as_ptr(), &mut actions) } == 1;
        for i in 0..actions.len { out(unsafe { *actions.at(i) }); }
        alive
    }
}

impl Drop for A11yBridge { fn drop(&mut self) { unsafe { sys::pxui_a11y_stop(self.0.as_ptr()) } } }
