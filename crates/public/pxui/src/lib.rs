#![allow(clippy::all)]
//! Stable PXUI wrapper. Shape and geometry types are re-exported from the std-only public crate.

use core::ffi::c_void;
use core::ptr::NonNull;
pub use pxui_types::{FrameIn, FrameOut, PxuiBox, PxuiPanel, PxuiTargetDesc, PxuiUiDesc};
pub use pxui_types::ffi::PxuiNodeDesc;
pub use pxui_types::{Commands, Gpu, Image, Pass};
pub use pxui_types::TextStyle;
use pxui_types::Str;

pub use pxui_types::{draw, geom};
pub use pxui_types::ffi::{A11yAction, A11yNode, A11yTree, Input, Output, PlatformOut, ThemeSource};
pub use pxui_core::{a11y, ui, ui_core, ui_kit, ui_text, ui_widgets};
pub use pxui_core::Theme;
pub use pxui_core::text as engine_text;
pub use pxui_render as render;

pub use pxui_types::theme;
pub mod text;
pub use text::Text;
pub mod paint;
pub use paint::Paint;
pub mod widgets;
pub mod composite;
pub use composite::DialogOut;
pub mod overlays;
pub use overlays::{MenuOpts, MenuResult, Pick, PickerOpts};
pub mod collections;
pub use collections::TreeNode;
pub mod image;
mod text_inputs;
pub use text_inputs::{TextAreaOpts, ValueFieldOpts};
pub mod kit;
#[path = "a11y.rs"]
mod a11y_ffi;
mod a11y_state;
mod style;
mod text_width;
mod interaction;
mod node;
pub use interaction::{Signal, SignalEvent, UiSaved};
pub use pxui_types::Node;

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::*;
}

#[cfg(not(feature = "static"))]
mod sys {
    use core::ffi::c_void;
    use pxui_types::{FrameIn, FrameOut, Str};
    use pxui_types::ffi::{A11yAction, ThemeSource};
    #[repr(C)] pub struct PxuiUi { _private: [u8; 0] }
    pub use pxui_types::{PxuiBox, PxuiPanel, PxuiTargetDesc, PxuiUiDesc};
    use pxui_types::ffi::PxuiNodeDesc;
    #[repr(C)] pub struct PxuiRenderer { _private:[u8;0] }
    #[link(name = "pxui")]
    unsafe extern "C" {
        pub safe fn pxui_abi_version()->u32;
        pub fn pxui_ui_create(desc:*const PxuiUiDesc) -> *mut PxuiUi;
        pub fn pxui_ui_create_with_fonts(desc:*const PxuiUiDesc,fonts:*const Str,count:usize)->*mut PxuiUi;
        pub fn pxui_ui_output(ui:*mut PxuiUi)->pxui_types::ffi::Output;
        pub fn pxui_ui_tokens(ui:*mut PxuiUi)->pxui_types::ffi::ThemeTokens;
        pub fn pxui_ui_theme(ui:*mut PxuiUi)->pxui_types::ffi::PxuiThemeSnapshot;
        pub fn pxui_ui_view_size(ui:*mut PxuiUi,out:*mut pxui_types::geom::Point)->i32;
        pub fn pxui_ui_frame(ui:*mut PxuiUi,input:*const pxui_types::ffi::Input,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void,out:*mut FrameOut)->i32;
        pub fn pxui_ui_set_theme(ui:*mut PxuiUi,source:*const ThemeSource)->i32;
        pub fn pxui_ui_set_dir(ui:*mut PxuiUi,dir:u32)->i32;
        pub fn pxui_ui_set_a11y(ui:*mut PxuiUi,on:u8)->i32;
        pub fn pxui_ui_inject_a11y(ui:*mut PxuiUi,actions:*const A11yAction,count:usize)->i32;
        pub fn pxui_ui_patch_text(ui:*mut PxuiUi,box_:*const PxuiBox,value:Str)->u8;
        pub fn pxui_ui_busy_step(ui:*mut PxuiUi,box_:*const PxuiBox,now:f64)->u8;
        pub fn pxui_ui_request_frame(ui:*mut PxuiUi)->i32;
        pub fn pxui_ui_node(ui:*mut PxuiUi,label:Str,node:*const PxuiNodeDesc,out:*mut PxuiBox)->i32;
        pub fn pxui_ui_rect(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut pxui_types::geom::Rect)->i32;
        pub fn pxui_ui_box_of_key(ui:*mut PxuiUi,key:i64,out:*mut PxuiBox)->u8;
        pub fn pxui_key_of(seed:i64,label:Str)->i64;
        pub fn pxui_ui_text(ui:*mut PxuiUi,box_:*const PxuiBox,value:Str)->i32;
        pub fn pxui_ui_modal(ui:*mut PxuiUi,label:Str,has_width:u8,width:f32,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void,out:*mut u8)->i32;
        pub fn pxui_ui_scroll_view(ui:*mut PxuiUi,label:Str,width:pxui_types::ffi::PxuiSizeDesc,height:pxui_types::ffi::PxuiSizeDesc,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void,out:*mut PxuiBox)->i32;
        pub fn pxui_ui_view(ui:*mut PxuiUi,label:Str,revision:u64,width:pxui_types::ffi::PxuiSizeDesc,height:pxui_types::ffi::PxuiSizeDesc,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void,out:*mut PxuiBox)->i32;
        pub fn pxui_ui_anchored(ui:*mut PxuiUi,label:Str,anchor:*const pxui_types::ffi::PxuiAnchorDesc,width:pxui_types::ffi::PxuiSizeDesc,height:pxui_types::ffi::PxuiSizeDesc,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void,out:*mut PxuiBox)->i32;
        pub fn pxui_ui_dialog(ui:*mut PxuiUi,title:Str,buttons:pxui_types::ffi::Slice<Str>,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void,out:*mut pxui_types::ffi::PxuiDialogOut)->i32;
        pub fn pxui_ui_destroy(ui: *mut PxuiUi);
        pub fn pxui_last_error(ui: *const PxuiUi) -> Str;
        pub fn pxui_frame(ui:*mut PxuiUi,input:*const FrameIn,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void,out:*mut FrameOut)->i32;
        pub fn pxui_within(ui:*mut PxuiUi,box_:*const PxuiBox,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void);
        pub fn pxui_scope(ui:*mut PxuiUi,label:Str,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void);
        pub fn pxui_col(ui:*mut PxuiUi,padding:f32,label:Str,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void);
        pub fn pxui_panel(ui:*mut PxuiUi,panel:*const PxuiPanel,label:Str,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void);
        pub fn pxui_label(ui:*mut PxuiUi,label:Str);
        pub fn pxui_button(ui:*mut PxuiUi,label:Str)->u8;
        pub fn pxui_toggle(ui:*mut PxuiUi,label:Str,on:u8)->u8;
        pub fn pxui_text_field(ui:*mut PxuiUi,label:Str,value:Str,changed:*mut Str)->u8;
        pub fn pxui_renderer_create(ui:*mut PxuiUi,gpu:*const pxui_types::Gpu,target:*const PxuiTargetDesc)->*mut PxuiRenderer;
        pub fn pxui_renderer_prepare(renderer:*mut PxuiRenderer,commands:*const pxui_types::Commands,frame:u64,completed:u64)->i32;
        pub fn pxui_renderer_draw(renderer:*mut PxuiRenderer,pass:*const pxui_types::Pass)->i32;
        pub fn pxui_renderer_destroy(renderer:*mut PxuiRenderer);
    }
}

#[derive(Clone, Copy, Default)]
pub struct TargetDesc { pub format:u32, pub samples:u8 }

pub struct Renderer<'ui> { raw:NonNull<sys::PxuiRenderer>, ui:&'ui mut Ui }

impl<'ui> Renderer<'ui> {
    pub fn new(ui:&'ui mut Ui,gpu:pxui_types::Gpu,target:TargetDesc)->Option<Self> {
        let desc=sys::PxuiTargetDesc { size:core::mem::size_of::<sys::PxuiTargetDesc>() as u32,format:target.format,samples:target.samples,reserved:[0;3],mask_width:0,mask_height:0,color_width:0,color_height:0,reserved_tail:0 };
        let raw=unsafe { sys::pxui_renderer_create(ui.0.as_ptr(),&gpu,&desc) };
        NonNull::new(raw).map(|raw| Self { raw, ui })
    }
    pub fn ui(&mut self)->&mut Ui { self.ui }
    pub fn prepare(&mut self,commands:&pxui_types::Commands,frame:u64,completed:u64)->Result<(),i32> {
        let rc=unsafe { sys::pxui_renderer_prepare(self.raw.as_ptr(),commands,frame,completed) };
        if rc==0 { Ok(()) } else { Err(rc) }
    }
    pub fn draw(&mut self,pass:&pxui_types::Pass)->Result<(),i32> {
        let rc=unsafe { sys::pxui_renderer_draw(self.raw.as_ptr(),pass) };
        if rc==0 { Ok(()) } else { Err(rc) }
    }
}

impl Drop for Renderer<'_> { fn drop(&mut self) { unsafe { sys::pxui_renderer_destroy(self.raw.as_ptr()) } } }

#[repr(transparent)]
pub struct Ui(NonNull<sys::PxuiUi>);

impl Ui {
    pub fn new() -> Option<Self> {
        let version=sys::pxui_abi_version();
        if version >> 16 != 1 { return None; }
        let desc=sys::PxuiUiDesc { size:core::mem::size_of::<sys::PxuiUiDesc>() as u32,platform:0,font_size:0 };
        NonNull::new(unsafe { sys::pxui_ui_create(&desc) }).map(Self)
    }

    pub fn new_with_fonts(platform:u32,font_size:i32,fonts:&[&[u8]])->Option<Self> {
        let desc=sys::PxuiUiDesc { size:core::mem::size_of::<sys::PxuiUiDesc>() as u32,platform,font_size };
        let fonts:Vec<Str>=fonts.iter().map(|bytes|Str { ptr:bytes.as_ptr(),len:bytes.len() }).collect();
        NonNull::new(unsafe { sys::pxui_ui_create_with_fonts(&desc,fonts.as_ptr(),fonts.len()) }).map(Self)
    }

    pub fn frame<F: FnMut(&mut Ui)>(&mut self, input: &FrameIn, mut build: F) -> Result<FrameOut, i32> {
        let mut out = FrameOut::default();
        let rc = unsafe { sys::pxui_frame(self.0.as_ptr(), input, Some(frame_callback::<F>), &mut build as *mut _ as *mut c_void, &mut out) };
        let result=if rc == 0 { Ok(out) } else { Err(rc) };
        if let Some(panic)=paint::take_callback_panic(){std::panic::resume_unwind(panic)}
        if let Some(panic)=composite::take_callback_panic(){std::panic::resume_unwind(panic)}
        result
    }

    pub fn last_error(&self) -> &str {
        let s = unsafe { sys::pxui_last_error(self.0.as_ptr()) };
        if s.len == 0 || s.ptr.is_null() { return ""; }
        core::str::from_utf8(unsafe { core::slice::from_raw_parts(s.ptr, s.len) }).unwrap_or("invalid error string")
    }

    pub fn output(&mut self) -> Output { unsafe { sys::pxui_ui_output(self.0.as_ptr()) } }
    pub fn tokens(&mut self) -> pxui_types::theme::Tokens { unsafe { sys::pxui_ui_tokens(self.0.as_ptr()) }.into() }
    pub fn theme(&mut self) -> pxui_types::theme::ThemeSnapshot { unsafe { sys::pxui_ui_theme(self.0.as_ptr()) }.into() }

    pub fn draw_revision(&mut self) -> u64 { self.output().draw_revision }
    pub fn built_nodes(&mut self) -> u64 { self.output().built_nodes }
    pub fn rebuilt_views(&mut self) -> u64 { self.output().rebuilt_views }
    pub fn arranged_nodes(&mut self) -> u64 { self.output().arranged_nodes }
    pub fn view_size(&self) -> (f32, f32) {
        let mut out = pxui_types::geom::Point::default();
        let _ = unsafe { sys::pxui_ui_view_size(self.0.as_ptr(), &mut out) };
        (out.x, out.y)
    }
    pub fn repaint_at(&mut self) -> Option<f64> {
        let at = self.output().repaint_at;
        at.is_finite().then_some(at)
    }

    pub fn input_frame<F: FnMut(&mut Ui)>(&mut self,input:&Input,mut build:F)->Result<FrameOut,i32> {
        let mut out=FrameOut::default();
        let rc=unsafe { sys::pxui_ui_frame(self.0.as_ptr(),input,Some(frame_callback::<F>),&mut build as *mut _ as *mut c_void,&mut out) };
        let result=if rc==0 {Ok(out)} else {Err(rc)};
        if let Some(panic)=paint::take_callback_panic(){std::panic::resume_unwind(panic)}
        if let Some(panic)=composite::take_callback_panic(){std::panic::resume_unwind(panic)}
        result
    }

    pub fn set_theme_source(&mut self,source:&ThemeSource)->Result<(),i32> {
        let rc=unsafe{sys::pxui_ui_set_theme(self.0.as_ptr(),source)};if rc==0{Ok(())}else{Err(rc)}
    }
    pub fn set_theme(&mut self,source:&ThemeSource)->Result<(),i32>{self.set_theme_source(source)}
    pub fn set_dir(&mut self,rtl:bool)->Result<(),i32>{let rc=unsafe{sys::pxui_ui_set_dir(self.0.as_ptr(),rtl as u32)};if rc==0{Ok(())}else{Err(rc)}}
    pub fn set_a11y(&mut self,on:bool)->Result<(),i32>{let rc=unsafe{sys::pxui_ui_set_a11y(self.0.as_ptr(),on as u8)};if rc==0{Ok(())}else{Err(rc)}}
    pub fn inject_a11y(&mut self,actions:&[A11yAction])->Result<(),i32>{let rc=unsafe{sys::pxui_ui_inject_a11y(self.0.as_ptr(),actions.as_ptr(),actions.len())};if rc==0{Ok(())}else{Err(rc)}}
    pub fn patch_text(&mut self,box_:&PxuiBox,value:&str)->bool{unsafe{sys::pxui_ui_patch_text(self.0.as_ptr(),box_,as_str(value))!=0}}
    pub fn busy_step(&mut self,box_:&PxuiBox,now:f64)->bool{unsafe{sys::pxui_ui_busy_step(self.0.as_ptr(),box_,now)!=0}}
    pub fn request_frame(&mut self)->Result<(),i32>{let rc=unsafe{sys::pxui_ui_request_frame(self.0.as_ptr())};if rc==0{Ok(())}else{Err(rc)}}
    pub fn node_desc(&mut self,label:&str,node:&PxuiNodeDesc)->Result<PxuiBox,i32>{
        let mut out=PxuiBox::default();let rc=unsafe{sys::pxui_ui_node(self.0.as_ptr(),as_str(label),node,&mut out)};
        if rc==0{Ok(out)}else{Err(rc)}
    }
    pub fn node(&mut self,label:&str,node:&Node)->Result<PxuiBox,i32>{let encoded=node::EncodedNode::new(node);self.node_desc(label,&encoded.desc)}
    pub fn rect(&mut self,box_:&PxuiBox)->Result<geom::Rect,i32>{
        let mut out=geom::Rect::default();let rc=unsafe{sys::pxui_ui_rect(self.0.as_ptr(),box_,&mut out)};
        if rc==0{Ok(out)}else{Err(rc)}
    }
    pub fn box_of_key(&mut self,key:i64)->Option<PxuiBox>{let mut out=PxuiBox::default();(unsafe{sys::pxui_ui_box_of_key(self.0.as_ptr(),key,&mut out)}!=0).then_some(out)}
    pub fn key_of(seed:i64,label:&str)->i64{unsafe{sys::pxui_key_of(seed,as_str(label))}}
    pub fn text(&mut self,box_:&PxuiBox,value:&str)->Result<(),i32>{let rc=unsafe{sys::pxui_ui_text(self.0.as_ptr(),box_,as_str(value))};if rc==0{Ok(())}else{Err(rc)}}

    pub fn within<F: FnMut(&mut Ui)>(&mut self, box_: &PxuiBox, mut f: F) {
        unsafe { sys::pxui_within(self.0.as_ptr(), box_, Some(frame_callback::<F>), &mut f as *mut _ as *mut c_void) }
    }

    pub fn scope<F: FnMut(&mut Ui)>(&mut self, label: &str, mut f: F) {
        unsafe { sys::pxui_scope(self.0.as_ptr(), as_str(label), Some(frame_callback::<F>), &mut f as *mut _ as *mut c_void) }
    }
    pub fn col<F: FnMut(&mut Ui)>(&mut self, padding:f32, label:&str, mut f:F) {
        unsafe { sys::pxui_col(self.0.as_ptr(), padding, as_str(label), Some(frame_callback::<F>), &mut f as *mut _ as *mut c_void) }
    }
    pub fn panel<F: FnMut(&mut Ui)>(&mut self, label:&str, f:F) {
        let panel = sys::PxuiPanel { size:core::mem::size_of::<sys::PxuiPanel>() as u32,x:12.0,y:12.0,width:280.0,height:0.0,padding:0,has_height:0,window:0 };
        self.panel_desc(&panel, label, f)
    }
    pub fn panel_desc<F: FnMut(&mut Ui)>(&mut self, panel:&PxuiPanel, label:&str, mut f:F) {
        unsafe { sys::pxui_panel(self.0.as_ptr(), panel, as_str(label), Some(frame_callback::<F>), &mut f as *mut _ as *mut c_void) }
    }
    pub fn label(&mut self, text:&str) { unsafe { sys::pxui_label(self.0.as_ptr(), as_str(text)) } }
    pub fn button(&mut self, text:&str) -> bool { unsafe { sys::pxui_button(self.0.as_ptr(), as_str(text)) != 0 } }
    pub fn toggle(&mut self, text:&str, on:bool) -> bool { unsafe { sys::pxui_toggle(self.0.as_ptr(), as_str(text), on as u8) != 0 } }
    pub fn text_field(&mut self, label:&str, value:&str) -> Option<&str> {
        let mut changed = Str::default();
        let did_change = unsafe { sys::pxui_text_field(self.0.as_ptr(), as_str(label), as_str(value), &mut changed) };
        if did_change == 0 { None } else { Some(core::str::from_utf8(unsafe { core::slice::from_raw_parts(changed.ptr, changed.len) }).ok()?) }
    }
}

extern "C" fn frame_callback<F:FnMut(&mut Ui)>(user:*mut c_void,raw:*mut sys::PxuiUi) {
    if user.is_null() || raw.is_null() { return; }
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let f=unsafe{&mut *user.cast::<F>()};
        let mut ui=core::mem::ManuallyDrop::new(Ui(unsafe{NonNull::new_unchecked(raw)}));
        f(&mut ui);
    }));
    if let Err(panic)=result { composite::defer_callback_panic(panic); }
}

impl Drop for Ui { fn drop(&mut self) { unsafe { sys::pxui_ui_destroy(self.0.as_ptr()) } } }

fn as_str(s: &str) -> Str { Str { ptr: s.as_ptr(), len: s.len() } }
