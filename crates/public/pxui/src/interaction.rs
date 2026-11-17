use crate::{PxuiBox, Ui, check};
use pxui_types::{Event, ffi::{PxuiSaved, PxuiSignal}};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Signal {
    pub hovered: bool,
    pub pressed: bool,
    pub held: bool,
    pub released: bool,
    pub clicked: bool,
    pub double_clicked: bool,
    pub dragging: bool,
    pub subtree_press: Option<i64>,
    pub clicks: i64,
    pub drag: (f32,f32),
    pub pointer: (f32,f32),
    pub press_point: (f32,f32),
    pub release_point: (f32,f32),
    pub button: Option<u8>,
    pub scroll: (f32,f32),
    pub pinch: f64,
    pub a11y_value: Option<f64>,
    pub a11y_select: Option<[u32;2]>,
}

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{pxui_ui_enter, pxui_ui_focus_box, pxui_ui_focused, pxui_ui_leave, pxui_ui_set_state, pxui_ui_signal, pxui_ui_state};
}
#[cfg(not(feature = "static"))]
mod sys {
    use crate::{PxuiBox, sys::PxuiUi};
    use pxui_types::ffi::{PxuiSaved, PxuiSignal};
    #[link(name = "pxui")]
    unsafe extern "C" {
        /// The C entry `Ui::enter` calls: makes `box_` (declared this frame) the parent of the nodes declared next and writes the depths to restore into `out`; an error code for a stale box or one not in the current build.
        /// Pair it with `pxui_ui_leave`.
        /// Allocates: never.
        /// Panics: never.
        ///
        /// ```
        /// pxui::doc::frames(1, &mut |ui| {
        ///     let parent = ui.node("p".into(), pxui::Node::DEFAULT);
        ///     let saved = ui.enter(&parent);
        ///     ui.node("child".into(), pxui::Node::DEFAULT); // a child of `parent`
        ///     ui.leave(saved);
        /// });
        /// ```
        pub fn pxui_ui_enter(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut PxuiSaved)->i32;
        /// The C entry `Ui::leave` calls: restores the parent and key seed depths `saved` (what `pxui_ui_enter` wrote) recorded; an error code for a null or oversize record.
        /// Allocates: never.
        /// Panics: never.
        ///
        /// ```
        /// pxui::doc::frames(1, &mut |ui| {
        ///     let parent = ui.node("p".into(), pxui::Node::DEFAULT);
        ///     let saved = ui.enter(&parent);
        ///     ui.leave(saved);
        /// });
        /// ```
        pub fn pxui_ui_leave(ui:*mut PxuiUi,saved:*const PxuiSaved)->i32;
        /// The C entry `Ui::focus_box` calls: gives keyboard focus to `box_`, ending any text composition; an error code for a stale box.
        /// Allocates: never.
        /// Panics: never.
        ///
        /// ```
        /// pxui::doc::frames(1, &mut |ui| {
        ///     let b = ui.node("n".into(), pxui::Node { flags: pxui::ui::Flags::FOCUSABLE, ..pxui::Node::DEFAULT });
        ///     ui.focus_box(&b);
        ///     assert!(ui.focused(&b));
        /// });
        /// ```
        pub fn pxui_ui_focus_box(ui:*mut PxuiUi,box_:*const PxuiBox)->i32;
        /// The C entry `Ui::focused` calls: writes 1 to `out` when `box_` holds keyboard focus, else 0; an error code for a stale box.
        /// Allocates: never.
        /// Panics: never.
        ///
        /// ```
        /// pxui::doc::frames(1, &mut |ui| {
        ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
        ///     assert!(!ui.focused(&b));
        /// });
        /// ```
        pub fn pxui_ui_focused(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut u8)->i32;
        /// The C entry `Ui::state` calls: writes `box_`'s state slot to `out`, or `default` when never set; the stale code for a node that has gone.
        /// Allocates: never.
        /// Panics: never.
        ///
        /// ```
        /// pxui::doc::frames(1, &mut |ui| {
        ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
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
        ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
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
        ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
        ///     let signal = ui.signal(&b);
        ///     assert!(!signal.clicked && !signal.hovered);
        /// });
        /// ```
        pub fn pxui_ui_signal(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut PxuiSignal)->i32;
    }
}

/// What `enter` returns and `leave` takes back, unchanged.
pub type UiSaved = PxuiSaved;

impl Ui {
    #[inline]
    #[track_caller] pub fn enter(&mut self,box_:&PxuiBox)-> UiSaved {
        let mut out=PxuiSaved::default();
        check(self, unsafe{sys::pxui_ui_enter(self.0.as_ptr(),box_,&mut out)});out
    }
    #[inline] #[track_caller] pub fn leave(&mut self,saved:UiSaved) {check(self, unsafe{sys::pxui_ui_leave(self.0.as_ptr(),&saved)})}
    #[inline] #[track_caller] pub fn focus_box(&mut self,box_:&PxuiBox) {check(self, unsafe{sys::pxui_ui_focus_box(self.0.as_ptr(),box_)})}
    #[inline] #[track_caller] pub fn focused(&self,box_:&PxuiBox)-> bool {let mut out=0;check(self, unsafe{sys::pxui_ui_focused(self.0.as_ptr(),box_,&mut out)});out!=0}
    /// `box_`'s state slot, or `default` while it was never set.
    ///
    /// A handle of a node that has gone reads `default` (no panic), like every query on a stale handle.
    /// Allocates: never.
    /// Panics: outside a frame, or when the `Ui` has failed.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     assert_eq!(ui.state(&b, 5), 5);
    ///     ui.set_state(&b, 7);
    ///     assert_eq!(ui.state(&b, 5), 7);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn state(&self,box_:&PxuiBox,default:i64)-> i64 {
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
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     ui.set_state(&b, -1);
    ///     assert_eq!(ui.state(&b, 0), -1);
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn set_state(&mut self,box_:&PxuiBox,value:i64) { let _ = crate::live(self, unsafe{sys::pxui_ui_set_state(self.0.as_ptr(),box_,value)}); }

    /// What happened to `box_` this frame: one library call that fills scalars on the stack.
    ///
    /// Allocates: nothing.
    /// Panics: when the `Ui` has failed, like every `Ui` call.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new().unwrap();
    /// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     assert!(!ui.signal(&b).hovered);
    /// }).unwrap();
    /// ```
    #[inline]
    #[track_caller] pub fn signal(&self,box_:&PxuiBox)-> Signal {
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
    /// let mut ui = pxui::Ui::new().unwrap();
    /// ui.frame(&pxui::FrameIn::default(), &mut |ui| {
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     assert!(ui.signal_keys(&b).is_empty());
    /// }).unwrap();
    /// ```
    #[inline]
    #[track_caller] pub fn signal_keys(&self,box_:&PxuiBox)-> &[Event] {
        let mut raw=PxuiSignal::default();
        let rc=unsafe{sys::pxui_ui_signal(self.0.as_ptr(),box_,&mut raw)};
        if !crate::live(self, rc) || raw.keys.len==0 || raw.keys.ptr.is_null() { return &[]; }
        unsafe{core::slice::from_raw_parts(raw.keys.ptr,raw.keys.len)}
    }
}

/// The wrapper's `Signal` from what the library wrote.
pub(crate) fn decode_signal(raw:&PxuiSignal)->Signal {
    let f=raw.flags;
    Signal{hovered:f&PxuiSignal::HOVERED!=0,pressed:f&PxuiSignal::PRESSED!=0,held:f&PxuiSignal::HELD!=0,released:f&PxuiSignal::RELEASED!=0,clicked:f&PxuiSignal::CLICKED!=0,double_clicked:f&PxuiSignal::DOUBLE_CLICKED!=0,dragging:f&PxuiSignal::DRAGGING!=0,
        subtree_press:(raw.has_subtree_press!=0).then_some(raw.subtree_press),clicks:raw.clicks,drag:(raw.drag[0],raw.drag[1]),pointer:(raw.pointer[0],raw.pointer[1]),
        press_point:(raw.press_point[0],raw.press_point[1]),release_point:(raw.release_point[0],raw.release_point[1]),button:(raw.button!=u8::MAX).then_some(raw.button),scroll:(raw.scroll[0],raw.scroll[1]),
        pinch:raw.pinch,a11y_value:(raw.has_a11y_value!=0).then_some(raw.a11y_value),a11y_select:(raw.has_a11y_select!=0).then_some(raw.a11y_select)}
}
