use crate::{PxuiBox, Ui};
use pxui_types::Event as FfiEvent;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct UiSaved { size: u32, reserved: u32, parents: usize, seeds: usize }

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignalEvent { pub kind: u32, pub code: u32, pub x: f64, pub y: f64, pub value: f64, pub text: String }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Signal {
    pub hovered: bool, pub pressed: bool, pub held: bool, pub released: bool, pub clicked: bool,
    pub double_clicked: bool, pub dragging: bool, pub subtree_press: Option<i64>, pub clicks: i64,
    pub drag: [f32;2], pub pointer: [f32;2], pub press_point: [f32;2], pub release_point: [f32;2],
    pub button: Option<u8>, pub scroll: [f32;2], pub pinch: f64, pub keys: Vec<SignalEvent>,
    pub a11y_value: Option<f64>, pub a11y_select: Option<[u32;2]>,
}

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{PxuiSaved, PxuiSignal, pxui_ui_enter, pxui_ui_focus_box, pxui_ui_focused, pxui_ui_leave, pxui_ui_set_state, pxui_ui_signal, pxui_ui_state};
}
#[cfg(not(feature = "static"))]
mod sys {
    use crate::{PxuiBox, sys::PxuiUi};
    #[repr(C)] pub struct PxuiSaved { pub size:u32, pub reserved:u32, pub parents:usize, pub seeds:usize }
    #[repr(C)]
    pub struct PxuiSignal {
        pub size:u32, pub flags:u32, pub has_subtree_press:u8, pub button:u8, pub has_a11y_value:u8, pub has_a11y_select:u8,
        pub reserved:[u8;4], pub subtree_press:i64, pub clicks:i64, pub drag:[f32;2], pub pointer:[f32;2],
        pub press_point:[f32;2], pub release_point:[f32;2], pub scroll:[f32;2], pub pinch:f64, pub a11y_value:f64,
        pub a11y_select:[u32;2], pub keys:pxui_types::ffi::Slice<pxui_types::Event>, pub text_len:usize,
    }
    impl Default for PxuiSignal { fn default()->Self { Self { size:0,flags:0,has_subtree_press:0,button:u8::MAX,has_a11y_value:0,has_a11y_select:0,reserved:[0;4],subtree_press:0,clicks:0,drag:[0.0;2],pointer:[0.0;2],press_point:[0.0;2],release_point:[0.0;2],scroll:[0.0;2],pinch:1.0,a11y_value:0.0,a11y_select:[0;2],keys:pxui_types::ffi::Slice{ptr:core::ptr::null(),len:0},text_len:0 } } }
    use pxui_types::Event;
    #[link(name = "pxui")]
    unsafe extern "C" {
        pub fn pxui_ui_enter(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut PxuiSaved)->i32;
        pub fn pxui_ui_leave(ui:*mut PxuiUi,saved:*const PxuiSaved)->i32;
        pub fn pxui_ui_focus_box(ui:*mut PxuiUi,box_:*const PxuiBox)->i32;
        pub fn pxui_ui_focused(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut u8)->i32;
        pub fn pxui_ui_state(ui:*mut PxuiUi,box_:*const PxuiBox,default:i64,out:*mut i64)->i32;
        pub fn pxui_ui_set_state(ui:*mut PxuiUi,box_:*const PxuiBox,value:i64)->i32;
        pub fn pxui_ui_signal(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut PxuiSignal,events:*mut Event,event_capacity:usize,text:*mut u8,text_capacity:usize)->i32;
    }
}

fn result(rc:i32)->Result<(),i32>{if rc==0{Ok(())}else{Err(rc)}}
impl Ui {
    pub fn enter(&mut self,box_:&PxuiBox)->Result<UiSaved,i32>{
        let mut out=sys::PxuiSaved{size:0,reserved:0,parents:0,seeds:0};
        let rc=unsafe{sys::pxui_ui_enter(self.0.as_ptr(),box_,&mut out)};result(rc)?;Ok(UiSaved{size:out.size,reserved:out.reserved,parents:out.parents,seeds:out.seeds})
    }
    pub fn leave(&mut self,saved:UiSaved)->Result<(),i32>{let saved=sys::PxuiSaved{size:saved.size,reserved:saved.reserved,parents:saved.parents,seeds:saved.seeds};result(unsafe{sys::pxui_ui_leave(self.0.as_ptr(),&saved)})}
    pub fn focus_box(&mut self,box_:&PxuiBox)->Result<(),i32>{result(unsafe{sys::pxui_ui_focus_box(self.0.as_ptr(),box_)})}
    pub fn focused(&mut self,box_:&PxuiBox)->Result<bool,i32>{let mut out=0;result(unsafe{sys::pxui_ui_focused(self.0.as_ptr(),box_,&mut out)})?;Ok(out!=0)}
    pub fn state(&mut self,box_:&PxuiBox,default:i64)->Result<i64,i32>{let mut out=0;result(unsafe{sys::pxui_ui_state(self.0.as_ptr(),box_,default,&mut out)})?;Ok(out)}
    pub fn set_state(&mut self,box_:&PxuiBox,value:i64)->Result<(),i32>{result(unsafe{sys::pxui_ui_set_state(self.0.as_ptr(),box_,value)})}
    pub fn signal(&mut self,box_:&PxuiBox)->Result<Signal,i32>{
        let mut raw=sys::PxuiSignal::default();
        let rc=unsafe{sys::pxui_ui_signal(self.0.as_ptr(),box_,&mut raw,core::ptr::null_mut(),0,core::ptr::null_mut(),0)};
        if rc!=1&&rc!=0{return Err(rc)}
        let mut events=vec![FfiEvent::default();raw.keys.len];let mut text=vec![0u8;raw.text_len];
        let rc=unsafe{sys::pxui_ui_signal(self.0.as_ptr(),box_,&mut raw,events.as_mut_ptr(),events.len(),text.as_mut_ptr(),text.len())};result(rc)?;
        let keys=events.into_iter().map(|e|{
            let text_value=if e.text.len==0{String::new()}else{
                let start=text.as_ptr() as usize;let end=start+text.len();let ptr=e.text.ptr as usize;
                if ptr<start||ptr.checked_add(e.text.len).map_or(true,|last|last>end){return SignalEvent{kind:e.kind,code:e.code,x:e.x,y:e.y,value:e.value,text:String::new()}}
                String::from_utf8_lossy(&text[ptr-start..ptr-start+e.text.len]).into_owned()
            };
            SignalEvent{kind:e.kind,code:e.code,x:e.x,y:e.y,value:e.value,text:text_value}
        }).collect();
        let f=raw.flags;
        Ok(Signal{hovered:f&1!=0,pressed:f&2!=0,held:f&4!=0,released:f&8!=0,clicked:f&16!=0,double_clicked:f&32!=0,dragging:f&64!=0,
            subtree_press:(raw.has_subtree_press!=0).then_some(raw.subtree_press),clicks:raw.clicks,drag:raw.drag,pointer:raw.pointer,
            press_point:raw.press_point,release_point:raw.release_point,button:(raw.button!=u8::MAX).then_some(raw.button),scroll:raw.scroll,
            pinch:raw.pinch,keys,a11y_value:(raw.has_a11y_value!=0).then_some(raw.a11y_value),a11y_select:(raw.has_a11y_select!=0).then_some(raw.a11y_select)})
    }
}
