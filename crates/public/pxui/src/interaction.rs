use crate::{PxuiBox, Ui, check};
use pxui_types::{Event as FfiEvent, ffi::{PxuiSaved, PxuiSignal, Slice}};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignalEvent { pub kind: u32, pub code: u32, pub x: f64, pub y: f64, pub value: f64, pub text: String }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Signal {
    pub hovered: bool, pub pressed: bool, pub held: bool, pub released: bool, pub clicked: bool,
    pub double_clicked: bool, pub dragging: bool, pub subtree_press: Option<i64>, pub clicks: i64,
    pub drag: (f32,f32), pub pointer: (f32,f32), pub press_point: (f32,f32), pub release_point: (f32,f32),
    pub button: Option<u8>, pub scroll: (f32,f32), pub pinch: f64, pub keys: Vec<SignalEvent>,
    pub a11y_value: Option<f64>, pub a11y_select: Option<[u32;2]>,
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
        pub fn pxui_ui_enter(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut PxuiSaved)->i32;
        pub fn pxui_ui_leave(ui:*mut PxuiUi,saved:*const PxuiSaved)->i32;
        pub fn pxui_ui_focus_box(ui:*mut PxuiUi,box_:*const PxuiBox)->i32;
        pub fn pxui_ui_focused(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut u8)->i32;
        pub fn pxui_ui_state(ui:*mut PxuiUi,box_:*const PxuiBox,default:i64,out:*mut i64)->i32;
        pub fn pxui_ui_set_state(ui:*mut PxuiUi,box_:*const PxuiBox,value:i64)->i32;
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
    #[inline] #[track_caller] pub fn state(&self,box_:&PxuiBox,default:i64)-> i64 {let mut out=0;check(self, unsafe{sys::pxui_ui_state(self.0.as_ptr(),box_,default,&mut out)});out}
    #[inline] #[track_caller] pub fn set_state(&mut self,box_:&PxuiBox,value:i64) {check(self, unsafe{sys::pxui_ui_set_state(self.0.as_ptr(),box_,value)})}

    /// Numeric/pointer/activation state only; `keys` is empty. Warm calls do not allocate.
    #[inline]
    #[track_caller] pub fn signal_state(&self, box_: &PxuiBox) -> Signal {
        let mut raw = PxuiSignal::default();
        let rc = unsafe { sys::pxui_ui_signal(self.0.as_ptr(), box_, &mut raw) };
        if !crate::live(self, rc) { return decode_signal(&PxuiSignal::default()); }
        raw.keys = pxui_types::ffi::Slice::default();
        decode_signal(&raw)
    }

    /// What happened to `box_` this frame: one library call that fills scalars on the stack. A box with no key events
    /// allocates nothing; key events are copied out of the library's frame buffer.
    #[inline]
    #[track_caller] pub fn signal(&self,box_:&PxuiBox)-> Signal {
        let mut raw=PxuiSignal::default();
        let rc=unsafe{sys::pxui_ui_signal(self.0.as_ptr(),box_,&mut raw)};
        // a node that has gone has no signal: the engine's `signal` answers with an all-quiet one
        if !crate::live(self, rc) { return decode_signal(&PxuiSignal::default()); }
        decode_signal(&raw)
    }
}

/// The wrapper's `Signal` from what the library wrote; key events are copied out of the frame buffer.
pub(crate) fn decode_signal(raw:&PxuiSignal)->Signal {
    let keys=if raw.keys.len==0 { Vec::new() } else { decode_keys(&raw.keys) };
    let f=raw.flags;
    Signal{hovered:f&PxuiSignal::HOVERED!=0,pressed:f&PxuiSignal::PRESSED!=0,held:f&PxuiSignal::HELD!=0,released:f&PxuiSignal::RELEASED!=0,clicked:f&PxuiSignal::CLICKED!=0,double_clicked:f&PxuiSignal::DOUBLE_CLICKED!=0,dragging:f&PxuiSignal::DRAGGING!=0,
        subtree_press:(raw.has_subtree_press!=0).then_some(raw.subtree_press),clicks:raw.clicks,drag:(raw.drag[0],raw.drag[1]),pointer:(raw.pointer[0],raw.pointer[1]),
        press_point:(raw.press_point[0],raw.press_point[1]),release_point:(raw.release_point[0],raw.release_point[1]),button:(raw.button!=u8::MAX).then_some(raw.button),scroll:(raw.scroll[0],raw.scroll[1]),
        pinch:raw.pinch,keys,a11y_value:(raw.has_a11y_value!=0).then_some(raw.a11y_value),a11y_select:(raw.has_a11y_select!=0).then_some(raw.a11y_select)}
}

#[cold]
fn decode_keys(keys:&Slice<FfiEvent>)->Vec<SignalEvent>{
    unsafe{core::slice::from_raw_parts(keys.ptr,keys.len)}.iter().map(|e|{
        let text=if e.text.len==0||e.text.ptr.is_null(){String::new()}else{String::from_utf8_lossy(unsafe{core::slice::from_raw_parts(e.text.ptr,e.text.len)}).into_owned()};
        SignalEvent{kind:e.kind,code:e.code,x:e.x,y:e.y,value:e.value,text}
    }).collect()
}
