use crate::Handle;
use crate::{Ui, check};
use pxui_types::{Event, ffi::PxuiSignal};

/// What input did to one node this frame: the raw record `Ui::signal` reads, field for field the engine's.
///
/// The widgets' `Response` is the summary of the same frame's signal of `Response::handle` (`clicked`, `hovered`, `held`) plus what only
/// the widget knows (`focused`, `activated`, `changed`, `secondary`); read `ui.signal(&response.handle)` for the rest (drag, wheel,
/// pinch, press points, click count). Points are window points.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Signal {
    /// The node is the topmost one under the pointer, or holds the pointer capture.
    pub hovered: bool,
    /// A press on the node began this frame.
    pub pressed: bool,
    /// The node holds the pointer capture after this frame's events (a press on it is down).
    pub held: bool,
    /// The capture ended this frame (the press was released, anywhere).
    pub released: bool,
    /// A primary press and its release both landed inside the node's hit rectangle this frame.
    pub clicked: bool,
    /// The click this frame was the second of a double click.
    pub double_clicked: bool,
    /// The node holds the capture and the pointer moved this frame.
    pub dragging: bool,
    /// The ordinal of the last press this frame on the node or on any descendant hit under it; `None` with no such press.
    pub subtree_press: Option<i64>,
    /// On the frame of a primary press: 1, or 2 and 3 for the second and third press within 0.35 s and 5 points; 0 otherwise.
    pub clicks: i64,
    /// The captured pointer's motion this frame (x, y).
    pub drag: (f32,f32),
    /// The latest pointer position.
    pub pointer: (f32,f32),
    /// Where the current or last press began.
    pub press_point: (f32,f32),
    /// Where the last press was released.
    pub release_point: (f32,f32),
    /// The mouse button of the press or release this frame, or of the held press: `Signal::BUTTON_PRIMARY`, `Signal::BUTTON_SECONDARY`,
    /// 2 the middle button, 3 and 4 the side buttons; `None` with no press.
    pub button: Option<u8>,
    /// The wheel steps routed to the node this frame (x, y).
    pub scroll: (f32,f32),
    /// The product of the trackpad pinch factors routed to the node this frame (1 when none was).
    pub pinch: f64,
    /// A value an assistive technology set on the node this frame: the widget applies it as one edit.
    pub a11y_value: Option<f64>,
    /// A selection (anchor, caret, in characters) an assistive technology set this frame on a focused text field.
    pub a11y_select: Option<[u32;2]>,
}

impl Signal {
    /// `button` of the primary (left) button, the one that clicks.
    pub const BUTTON_PRIMARY: u8 = 0;
    /// `button` of the secondary (right) button, or the two-finger or control click the platform reports as it: a context-menu request.
    pub const BUTTON_SECONDARY: u8 = 1;
}

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{pxui_ui_focus_box, pxui_ui_focus_default, pxui_ui_focused, pxui_ui_set_state, pxui_ui_signal, pxui_ui_state};
}
#[cfg(not(feature = "static"))]
mod sys {
    use crate::{PxuiBox, sys::PxuiUi};
    use pxui_types::ffi::PxuiSignal;
    #[link(name = "pxui")]
    unsafe extern "C" {
        // Gives keyboard focus to a live node.
        pub(super) fn pxui_ui_focus_box(ui:*mut PxuiUi,box_:*const PxuiBox)->i32;
        pub(super) fn pxui_ui_focus_default(ui:*mut PxuiUi,box_:*const PxuiBox)->i32;
        // Writes whether a live node holds keyboard focus.
        pub(super) fn pxui_ui_focused(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut u8)->i32;
        /// The C entry `Ui::state` calls: writes `box_`'s state slot to `out`, or `default` when never set; the stale code for a node that has gone.
        /// Allocates: never.
        /// Panics: never.
        ///
        /// ```
        /// pxui::doc::frames(1, &mut |ui| {
        ///     let b = ui.node("n", pxui::Node::DEFAULT);
        ///     assert_eq!(ui.state(&b, 3), 3);
        /// });
        /// ```
        pub fn pxui_ui_state(ui:*mut PxuiUi,box_:*const PxuiBox,default:i64,out:*mut i64)->i32;
        /// The C entry `Ui::set_state` calls: stores `value` in `box_`'s state slot, where it persists while the node lives; the stale code for a node that has gone.
        /// Allocates: never.
        /// Panics: never.
        ///
        /// ```
        /// pxui::doc::frames(2, &mut |ui| {
        ///     let b = ui.node("n", pxui::Node::DEFAULT);
        ///     let n = ui.state(&b, 0);
        ///     ui.set_state(&b, n + 1);
        /// });
        /// ```
        pub fn pxui_ui_set_state(ui:*mut PxuiUi,box_:*const PxuiBox,value:i64)->i32;
        /// The C entry `Ui::signal` and `Ui::signal_keys` call: writes what input did to `box_` this frame (hover, press, click, drag, scroll, pinch, the keys routed to it) into `out`; the stale code for a node that has gone.
        /// The key events in `out` are borrowed from the frame buffer until the next call on the `Ui`.
        /// Allocates: never.
        /// Panics: never.
        ///
        /// ```
        /// pxui::doc::frames(1, &mut |ui| {
        ///     let b = ui.node("n", pxui::Node::DEFAULT);
        ///     let signal = ui.signal(&b);
        ///     assert!(!signal.clicked && !signal.hovered);
        /// });
        /// ```
        pub fn pxui_ui_signal(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut PxuiSignal)->i32;
    }
}

impl Ui {
    /// Gives keyboard focus to a live node.
    ///
    /// A registered text-field body can use this on its row to bring the editor into focus
    /// during the field's first build. Changing the focused row ends text composition.
    /// Refocusing the same row preserves it.
    /// Allocates: never.
    /// Panics: if the handle has retired or the library refuses the Ui.
    ///
    /// ```
    /// use pxui::{A11yStates, Flags, Node, Role};
    /// let ui = pxui::doc::frames(1, &mut |ui| {
    ///     let row = ui.node("editor", Node::DEFAULT.flags(Flags::FOCUSABLE).role(Role::TextField));
    ///     ui.focus_box(&row);
    /// });
    /// assert!(ui.a11y_nodes().iter().any(|node| node.is_role(Role::TextField) && node.states & A11yStates::FOCUSED.bits() != 0));
    /// ```
    #[inline] #[track_caller] pub fn focus_box(&mut self,box_:&Handle) {check(self, unsafe{sys::pxui_ui_focus_box(self.0.as_ptr(),box_)})}
    /// Names the box a key press takes as its focus while no node holds it: a window's default key target (an initial first
    /// responder), such as the main list, which then takes the keys it handles and leaves the rest to `key_pressed`. The focus
    /// goes to the box's nearest focusable box (itself or an ancestor: a list's scroll `handle` names the list), unless a modal
    /// leaves it out. Stated each build; a build that names none leaves no default. A click elsewhere or Escape out of a field
    /// gives up the focus, and the next key press comes back here. While another widget (not a text editor) holds the focus, a
    /// press it leaves untaken falls through here (a focused sidebar leaves Command-A and Escape to the main list and keeps
    /// Return); the focused widget sees the press first when it builds before this box.
    /// Allocates: never.
    /// Panics: if the handle has retired or the library refuses the Ui.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| { let b = ui.node("sample", pxui::Node::DEFAULT.flags(pxui::Flags::FOCUSABLE)); ui.focus_default(&b); });
    /// ```
    #[inline] #[track_caller] pub fn focus_default(&mut self,box_:&Handle) {check(self, unsafe{sys::pxui_ui_focus_default(self.0.as_ptr(),box_)})}
    #[inline] #[track_caller] pub(crate) fn focused(&self,box_:&Handle)-> bool {let mut out=0;check(self, unsafe{sys::pxui_ui_focused(self.0.as_ptr(),box_,&mut out)});out!=0}
    /// `box_`'s state slot, or `default` while it was never set: the one per-node value a build keeps across frames.
    ///
    /// The slot is an `i64`; store a bool as `0`/`1` and an `f32`/`f64` as its bits (`value.to_bits() as i64`). Its text counterpart
    /// is `text_state` (a separate slot of the same node). A handle of a node that has gone reads `default` (no panic), like every
    /// query on a stale handle.
    /// Allocates: never.
    /// Panics: outside a frame, or when the `Ui` has failed.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n", pxui::Node::DEFAULT);
    ///     assert_eq!(ui.state(&b, 5), 5);
    ///     ui.set_state(&b, 7);
    ///     assert_eq!(ui.state(&b, 5), 7);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn state(&self,box_:&Handle,default:i64)-> i64 {
        let mut out=default;
        if !crate::live(self, unsafe{sys::pxui_ui_state(self.0.as_ptr(),box_,default,&mut out)}) { return default; }
        out
    }
    /// Stores `value` in `box_`'s state slot; it persists across frames while the node lives.
    ///
    /// A handle of a node that has gone stores nothing (no panic), like every patch on a stale handle.
    /// Allocates: never.
    /// Panics: outside a frame, or when the `Ui` has failed.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n", pxui::Node::DEFAULT);
    ///     ui.set_state(&b, -1);
    ///     assert_eq!(ui.state(&b, 0), -1);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn set_state(&mut self,box_:&Handle,value:i64) { let _ = crate::live(self, unsafe{sys::pxui_ui_set_state(self.0.as_ptr(),box_,value)}); }

    /// What happened to `box_` this frame: one library call that fills scalars on the stack.
    ///
    /// Allocates: nothing.
    /// Panics: when the `Ui` has failed, like every `Ui` call.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
    ///     let b = ui.node("n", pxui::Node::DEFAULT);
    ///     assert!(!ui.signal(&b).hovered);
    /// }).unwrap();
    /// ```
    #[inline]
    #[track_caller] pub fn signal(&self,box_:&Handle)-> Signal {
        let mut raw=PxuiSignal::default();
        let rc=unsafe{sys::pxui_ui_signal(self.0.as_ptr(),box_,&mut raw)};
        // a node that has gone has no signal: the engine's `signal` answers with an all-quiet one
        if !crate::live(self, rc) { return decode_signal(&PxuiSignal::default()); }
        decode_signal(&raw)
    }

    /// The key and text events routed to `box_` this frame, borrowed from the library's frame buffer:
    /// valid until the next call on `ui`.
    ///
    /// Allocates: nothing.
    /// Panics: when the `Ui` has failed, like every `Ui` call.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
    ///     let b = ui.node("n", pxui::Node::DEFAULT);
    ///     assert!(ui.signal_keys(&b).is_empty());
    /// }).unwrap();
    /// ```
    #[inline]
    #[track_caller] pub fn signal_keys(&self,box_:&Handle)-> &[Event] {
        let mut raw=PxuiSignal::default();
        let rc=unsafe{sys::pxui_ui_signal(self.0.as_ptr(),box_,&mut raw)};
        if !crate::live(self, rc) || raw.keys.len==0 || raw.keys.ptr.is_null() { return &[]; }
        unsafe{core::slice::from_raw_parts(raw.keys.ptr,raw.keys.len)}
    }
}

/// The wrapper's `Signal` from what the library wrote.
#[inline]
pub(crate) fn decode_signal(raw:&PxuiSignal)->Signal {
    let f=raw.flags;
    Signal{hovered:f&PxuiSignal::HOVERED!=0,pressed:f&PxuiSignal::PRESSED!=0,held:f&PxuiSignal::HELD!=0,released:f&PxuiSignal::RELEASED!=0,clicked:f&PxuiSignal::CLICKED!=0,double_clicked:f&PxuiSignal::DOUBLE_CLICKED!=0,dragging:f&PxuiSignal::DRAGGING!=0,
        subtree_press:(raw.has_subtree_press!=0).then_some(raw.subtree_press),clicks:raw.clicks,drag:(raw.drag[0],raw.drag[1]),pointer:(raw.pointer[0],raw.pointer[1]),
        press_point:(raw.press_point[0],raw.press_point[1]),release_point:(raw.release_point[0],raw.release_point[1]),button:(raw.button!=u8::MAX).then_some(raw.button),scroll:(raw.scroll[0],raw.scroll[1]),
        pinch:raw.pinch,a11y_value:(raw.has_a11y_value!=0).then_some(raw.a11y_value),a11y_select:(raw.has_a11y_select!=0).then_some(raw.a11y_select)}
}
