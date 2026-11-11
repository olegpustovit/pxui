#![allow(clippy::all)]
//! Stable PXUI wrapper. Shape and geometry types are re-exported from the std-only public crate.

#[macro_use]
mod macros;
use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;
pub use pxui_types::{FrameIn, FrameOut, PxuiBox, PxuiPanel, PxuiTargetDesc, PxuiUiDesc};
pub use pxui_types::ffi::PxuiNodeDesc;
pub use pxui_types::{Commands, Gpu, Image, Pass};
pub use pxui_types::TextStyle;
use pxui_types::Str;

pub use pxui_types::{draw, geom};
pub use pxui_types::ffi::{A11yAction, A11yNode, A11yTree, Input, Output, PlatformOut, ThemeSource};

pub use pxui_types::theme;

/// The engine's module paths, so an app moves to the wrapper by swapping its dependency.
pub mod a11y {
    pub use pxui_types::a11y::{state, Role};
    pub use pxui_types::ffi::{A11yAction, A11yNode, A11yTree};
}
pub mod ui_core {
    pub use pxui_types::ui::*;
    pub use crate::{Paint, Ui};
    pub use pxui_types::PxuiBox as UiBox;
    pub use pxui_types::{Flags, Size};
    pub use pxui_types::node::{Axis, Bar, Dir, Mark, Node};
    /// The key of a node handle.
    #[inline] pub fn key(b: &UiBox) -> i64 { b.box_key }
}
pub mod ui_text {
    /// Whether `signal` is a right click: released with the right button within 4 points of its press.
    pub fn context_clicked(signal: &crate::Signal) -> bool {
        let ((px, py), (rx, ry)) = (signal.press_point, signal.release_point);
        signal.released && signal.button == Some(1) && (px - rx) * (px - rx) + (py - ry) * (py - ry) < 16.0
    }
    pub use pxui_types::ui::{fuzzy_match, fuzzy_score, line_end, line_start, CodeContext, Completion, MenuResult, Pick};
    pub use crate::{MenuOpts, PickerOpts, TextAreaOpts, ValueFieldOpts};
}
pub mod ui_kit { pub use pxui_types::ui::PanelOpts; }
pub mod ui_widgets { pub use crate::{DialogOut, TreeNode}; }
/// The node handle `box_`, `node` and the views return.
pub type Handle = pxui_types::PxuiBox;

pub mod ui {
    pub use crate::ui_core::*;
    pub use crate::Handle;
    pub use pxui_types::Image;
    pub use crate::ui_text::*;
    pub use crate::ui_widgets::*;
    pub use crate::{Paint, Signal, Ui};
    pub use pxui_types::draw::*;
    pub use pxui_types::geom::*;
    pub use pxui_types::layout::*;
    pub use pxui_types::theme::{Curve, Font, Look, Ring, Shadow, State, States, Tokens};
    pub use pxui_types::a11y::Role;
}
pub mod text;
pub use text::{Text, UiText};
pub mod paint;
pub use paint::Paint;
pub mod widgets;
pub mod composite;
pub use composite::DialogOut;
pub mod overlays;
pub use overlays::{MenuOpts, PickerOpts};
pub use pxui_types::ui::{MenuResult, Pick};
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
mod ui_state;
mod box_ops;
mod kit_ext;
mod theme_scope;
pub use theme_scope::Theme;
pub use ui_state::{Bytes, Platform};
mod node;
#[cfg(all(test, feature = "static"))]
mod settled;
#[cfg(all(test, feature = "static"))]
mod surface;
#[cfg(all(test, feature = "static"))]
mod panics;
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
    #[repr(C)] pub struct PxuiTheme { _private:[u8;0] }
    #[repr(C)] pub struct PxuiPaint { _private:[u8;0] }
    #[repr(C)] pub struct PxuiShaper { _private:[u8;0] }
    #[link(name = "pxui")]
    unsafe extern "C" {
        pub safe fn pxui_abi_version()->u32;
        pub fn pxui_ui_create(desc:*const PxuiUiDesc) -> *mut PxuiUi;
        pub fn pxui_ui_create_with_fonts(desc:*const PxuiUiDesc,fonts:*const Str,count:usize)->*mut PxuiUi;
        pub fn pxui_ui_output(ui:*mut PxuiUi)->*const pxui_types::ffi::Output;
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
        pub fn pxui_ui_text(ui:*mut PxuiUi,box_:*const PxuiBox,value:Str)->i32;
        pub fn pxui_ui_modal(ui:*mut PxuiUi,label:Str,has_width:u8,width:f32,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void,out:*mut u8)->i32;
        pub fn pxui_ui_scroll_view(ui:*mut PxuiUi,label:Str,width:pxui_types::ffi::PxuiSizeDesc,height:pxui_types::ffi::PxuiSizeDesc,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void,out:*mut PxuiBox)->i32;
        pub fn pxui_ui_view(ui:*mut PxuiUi,label:Str,revision:u64,width:pxui_types::ffi::PxuiSizeDesc,height:pxui_types::ffi::PxuiSizeDesc,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void,out:*mut PxuiBox)->i32;
        pub fn pxui_ui_anchored(ui:*mut PxuiUi,label:Str,anchor:*const pxui_types::ffi::PxuiAnchorDesc,width:pxui_types::ffi::PxuiSizeDesc,height:pxui_types::ffi::PxuiSizeDesc,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void,out:*mut PxuiBox)->i32;
        pub fn pxui_ui_dialog(ui:*mut PxuiUi,title:Str,buttons:pxui_types::ffi::Slice<Str>,build:Option<extern "C" fn(*mut c_void,*mut PxuiUi)>,user:*mut c_void,out:*mut pxui_types::ffi::PxuiDialogOut)->i32;
        pub fn pxui_ui_destroy(ui: *mut PxuiUi) -> i32;
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
        pub fn pxui_renderer_atlas(renderer:*mut PxuiRenderer,kind:u32,out:*mut pxui_types::Image)->i32;
        pub fn pxui_renderer_destroy(renderer:*mut PxuiRenderer);
    }
}

#[derive(Clone, Copy, Default)]
pub struct TargetDesc { pub format:u32, pub samples:u8 }

pub struct Renderer<'ui> { raw:NonNull<sys::PxuiRenderer>, ui:&'ui mut Ui }

impl<'ui> Renderer<'ui> {
    /// A renderer on the app's device; the error is the library's reason (the device, the target or the `Ui` was refused).
    pub fn new(ui:&'ui mut Ui,gpu:pxui_types::Gpu,target:TargetDesc)->Result<Self,String> {
        let desc=sys::PxuiTargetDesc { format:target.format,samples:target.samples,..sys::PxuiTargetDesc::default() };
        let raw=unsafe { sys::pxui_renderer_create(ui.0.as_ptr(),&gpu,&desc) };
        match NonNull::new(raw) { Some(raw)=>Ok(Self { raw, ui }), None=>Err(ui.last_error().to_owned()) }
    }
    #[inline]
    pub fn ui(&mut self)->&mut Ui { self.ui }
    pub fn prepare(&mut self,commands:&pxui_types::Commands,frame:u64,completed:u64)->Result<(),i32> {
        let rc=unsafe { sys::pxui_renderer_prepare(self.raw.as_ptr(),commands,frame,completed) };
        composite::resume_deferred();
        if rc==0 { Ok(()) } else { Err(rc) }
    }
    pub fn draw(&mut self,pass:&pxui_types::Pass)->Result<(),i32> {
        let rc=unsafe { sys::pxui_renderer_draw(self.raw.as_ptr(),pass) };
        composite::resume_deferred();
        if rc==0 { Ok(()) } else { Err(rc) }
    }
}

/// A glyph atlas of the renderer, for an app pass that draws text quads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtlasKind { Mask, Color }

impl Renderer<'_> {
    /// The native handle of an atlas texture: one byte per texel for `Mask`, RGBA8 for `Color`. It stays the renderer's.
    pub fn atlas(&self, kind: AtlasKind) -> Result<pxui_types::Image, i32> {
        let mut out = pxui_types::Image::default();
        let rc = unsafe { sys::pxui_renderer_atlas(self.raw.as_ptr(), matches!(kind, AtlasKind::Color) as u32, &mut out) };
        if rc == 0 { Ok(out) } else { Err(rc) }
    }
}

impl Drop for Renderer<'_> { fn drop(&mut self) { unsafe { sys::pxui_renderer_destroy(self.raw.as_ptr()) } } }

/// One built frame: the draw output and damage of `pxui_frame`. It borrows the `Ui` it came from, so the
/// layer pointers it holds cannot be read after the next frame invalidates them.
pub struct Frame<'a> { out:FrameOut, _ui:PhantomData<&'a mut Ui> }

impl core::ops::Deref for Frame<'_> { type Target=FrameOut; #[inline] fn deref(&self)->&FrameOut { &self.out } }

#[repr(transparent)]
pub struct Ui(NonNull<sys::PxuiUi>);

impl Ui {
    pub fn new() -> Option<Self> {
        let version=abi_version();
        if version >> 16 != 1 { return None; }
        NonNull::new(unsafe { sys::pxui_ui_create(&sys::PxuiUiDesc::default()) }).map(Self)
    }

    pub fn new_with_fonts(platform:u32,font_size:i32,fonts:&[&[u8]])->Option<Self> {
        let desc=sys::PxuiUiDesc { platform,font_size,..sys::PxuiUiDesc::default() };
        let fonts:Vec<Str>=fonts.iter().map(|bytes|Str { ptr:bytes.as_ptr(),len:bytes.len() }).collect();
        NonNull::new(unsafe { sys::pxui_ui_create_with_fonts(&desc,fonts.as_ptr(),fonts.len()) }).map(Self)
    }

    /// Builds one frame. A panic in `build` (or in any callback of a widget it called) is carried out of the
    /// library and resumed here, after the call returns.
    pub fn frame<F: FnMut(&mut Ui)>(&mut self, input: &FrameIn, mut build: F) -> Result<Frame<'_>, i32> {
        let mut out = FrameOut::default();
        let rc = unsafe { sys::pxui_frame(self.0.as_ptr(), input, Some(composite::call::<F>), &mut build as *mut _ as *mut c_void, &mut out) };
        self.framed(rc, out)
    }

    pub fn input_frame<F: FnMut(&mut Ui)>(&mut self,input:&Input,mut build:F)->Result<Frame<'_>,i32> {
        let mut out=FrameOut::default();
        let rc=unsafe { sys::pxui_ui_frame(self.0.as_ptr(),input,Some(composite::call::<F>),&mut build as *mut _ as *mut c_void,&mut out) };
        self.framed(rc, out)
    }

    fn framed(&mut self, rc:i32, out:FrameOut)->Result<Frame<'_>,i32> {
        composite::resume_deferred();
        if rc==0 { Ok(Frame { out, _ui:PhantomData }) } else { Err(rc) }
    }

    pub fn last_error(&self) -> &str {
        let s = unsafe { sys::pxui_last_error(self.0.as_ptr()) };
        if s.len == 0 || s.ptr.is_null() { return ""; }
        core::str::from_utf8(unsafe { core::slice::from_raw_parts(s.ptr, s.len) }).unwrap_or("invalid error string")
    }

    /// The output of the last frame, borrowed from the `Ui`: a new frame needs `&mut self`, so a stale read does not compile.
    #[inline]
    pub fn output(&self) -> &Output { unsafe { &*sys::pxui_ui_output(self.0.as_ptr()) } }
    pub fn tokens(&mut self) -> pxui_types::theme::Tokens { unsafe { sys::pxui_ui_tokens(self.0.as_ptr()) }.into() }
    pub fn theme(&mut self) -> pxui_types::theme::ThemeSnapshot { unsafe { sys::pxui_ui_theme(self.0.as_ptr()) }.into() }

    #[inline] pub fn draw_revision(&self) -> u64 { self.output().draw_revision }
    #[inline] pub fn built_nodes(&self) -> u64 { self.output().built_nodes }
    #[inline] pub fn rebuilt_views(&self) -> u64 { self.output().rebuilt_views }
    #[inline] pub fn arranged_nodes(&self) -> u64 { self.output().arranged_nodes }
    pub fn view_size(&self) -> (f32, f32) {
        let mut out = pxui_types::geom::Point::default();
        let _ = unsafe { sys::pxui_ui_view_size(self.0.as_ptr(), &mut out) };
        (out.x, out.y)
    }
    #[inline]
    pub fn repaint_at(&self) -> Option<f64> {
        let at = self.output().repaint_at;
        at.is_finite().then_some(at)
    }

    /// Replaces the theme from a theme file's text; the library refuses a file that does not parse (see `last_error`).
    pub fn set_theme_source(&mut self,source:&ThemeSource)->Result<(),i32> { status(unsafe{sys::pxui_ui_set_theme(self.0.as_ptr(),source)}) }
    pub fn set_theme(&mut self,source:&ThemeSource)->Result<(),i32> {self.set_theme_source(source)}
    #[track_caller] pub fn set_dir(&mut self,rtl:bool) {check(self, unsafe{sys::pxui_ui_set_dir(self.0.as_ptr(),rtl as u32)})}
    #[track_caller] pub fn set_a11y(&mut self,on:bool) {check(self, unsafe{sys::pxui_ui_set_a11y(self.0.as_ptr(),on as u8)})}
    #[track_caller] pub fn inject_a11y(&mut self,actions:&[A11yAction]) {check(self, unsafe{sys::pxui_ui_inject_a11y(self.0.as_ptr(),actions.as_ptr(),actions.len())})}
    #[inline] pub fn patch_text(&mut self,box_:&PxuiBox,value:&str)->bool{unsafe{sys::pxui_ui_patch_text(self.0.as_ptr(),box_,as_str(value))!=0}}
    #[inline] pub fn busy_step(&mut self,box_:&PxuiBox,now:f64)->bool{unsafe{sys::pxui_ui_busy_step(self.0.as_ptr(),box_,now)!=0}}
    #[track_caller] pub fn request_frame(&mut self) {check(self, unsafe{sys::pxui_ui_request_frame(self.0.as_ptr())})}
    #[inline]
    #[track_caller] pub fn node_desc(&mut self,label:&str,node:&PxuiNodeDesc)-> PxuiBox {
        let mut out=PxuiBox::default();check(self, unsafe{sys::pxui_ui_node(self.0.as_ptr(),as_str(label),node,&mut out)});out
    }
    #[inline]
    #[track_caller] pub fn rect(&mut self,box_:&PxuiBox)-> geom::Rect {
        let mut out=geom::Rect::default();check(self, unsafe{sys::pxui_ui_rect(self.0.as_ptr(),box_,&mut out)});out
    }
    #[inline] pub fn box_of_key(&mut self,key:i64)->Option<PxuiBox>{let mut out=PxuiBox::default();(unsafe{sys::pxui_ui_box_of_key(self.0.as_ptr(),key,&mut out)}!=0).then_some(out)}
    #[inline] pub fn key_of(seed:i64,label:&str)->i64{pxui_types::ui::key_of(seed,label)}
    #[inline] #[track_caller] pub fn text(&mut self,box_:&PxuiBox,value:&str) {check(self, unsafe{sys::pxui_ui_text(self.0.as_ptr(),box_,as_str(value))})}

    #[inline]
    pub fn within<F: FnMut(&mut Ui)>(&mut self, box_: &PxuiBox, mut f: F) {
        unsafe { sys::pxui_within(self.0.as_ptr(), box_, Some(composite::call::<F>), &mut f as *mut _ as *mut c_void) }
    }

    #[inline]
    pub fn scope<F: FnMut(&mut Ui)>(&mut self, label: &str, mut f: F) {
        unsafe { sys::pxui_scope(self.0.as_ptr(), as_str(label), Some(composite::call::<F>), &mut f as *mut _ as *mut c_void) }
    }
    #[inline]
    pub fn col<F: FnMut(&mut Ui)>(&mut self, padding:f32, label:&str, mut f:F) {
        unsafe { sys::pxui_col(self.0.as_ptr(), padding, as_str(label), Some(composite::call::<F>), &mut f as *mut _ as *mut c_void) }
    }
    pub fn panel<F: FnMut(&mut Ui)>(&mut self, label:&str, f:F) {
        let panel = sys::PxuiPanel { x:12.0,y:12.0,width:280.0,..sys::PxuiPanel::default() };
        self.panel_desc(&panel, label, f)
    }
    #[inline]
    pub fn panel_desc<F: FnMut(&mut Ui)>(&mut self, panel:&PxuiPanel, label:&str, mut f:F) {
        unsafe { sys::pxui_panel(self.0.as_ptr(), panel, as_str(label), Some(composite::call::<F>), &mut f as *mut _ as *mut c_void) }
    }
    #[inline] pub fn label(&mut self, text:&str) { unsafe { sys::pxui_label(self.0.as_ptr(), as_str(text)) } }
    #[inline] pub fn button(&mut self, text:&str) -> bool { unsafe { sys::pxui_button(self.0.as_ptr(), as_str(text)) != 0 } }
    #[inline] pub fn toggle(&mut self, text:&str, on:bool) -> bool { unsafe { sys::pxui_toggle(self.0.as_ptr(), as_str(text), on as u8) != 0 } }
    pub fn text_field(&mut self, label:&str, value:&str) -> Option<&str> {
        let mut changed = Str::default();
        let did_change = unsafe { sys::pxui_text_field(self.0.as_ptr(), as_str(label), as_str(value), &mut changed) };
        if did_change == 0 { None } else { Some(core::str::from_utf8(unsafe { core::slice::from_raw_parts(changed.ptr, changed.len) }).ok()?) }
    }
}

impl Drop for Ui { fn drop(&mut self) { let _ = unsafe { sys::pxui_ui_destroy(self.0.as_ptr()) }; } }

/// A `&[&str]` as a span of `Str` for one call: up to 32 strings live on the stack, more on the heap.
pub(crate) struct Strs { stack:[Str;32], heap:Vec<Str>, len:usize }

impl Strs {
    #[inline]
    pub(crate) fn new(values:&[&str])->Self {
        let mut strs=Self { stack:[Str::default();32], heap:Vec::new(), len:values.len() };
        if values.len()<=32 { for (out,v) in strs.stack.iter_mut().zip(values) { *out=as_str(v); } }
        else { strs.heap=values.iter().map(|v|as_str(v)).collect(); }
        strs
    }
    #[inline]
    pub(crate) fn owned(values:&[String])->Self {
        let mut strs=Self { stack:[Str::default();32], heap:Vec::new(), len:values.len() };
        if values.len()<=32 { for (out,v) in strs.stack.iter_mut().zip(values) { *out=as_str(v); } }
        else { strs.heap=values.iter().map(|v|as_str(v)).collect(); }
        strs
    }
    #[inline]
    pub(crate) fn span(&self)->pxui_types::ffi::Slice<Str> {
        let ptr=if self.len<=32 { self.stack.as_ptr() } else { self.heap.as_ptr() };
        pxui_types::ffi::Slice { ptr, len:self.len }
    }
}

#[inline]
fn as_str(s: &str) -> Str { Str { ptr: s.as_ptr(), len: s.len() } }

/// A failed call is a caller bug the engine would panic on (stale handle, non-finite number, oversized text):
/// it panics with the library's message, inside the frame callback (carried to the app's `frame` call) or at the call.
#[track_caller]
fn check(ui: &Ui, rc: i32) { if rc != 0 { panic!("pxui: {} ({rc})", ui.last_error()); } }
#[cfg(feature="static")]
#[inline]
fn abi_version()->u32 { unsafe { sys::pxui_abi_version() } }
#[cfg(not(feature="static"))]
#[inline]
fn abi_version()->u32 { sys::pxui_abi_version() }

/// A string the library lent, copied: the library only returns UTF-8 within its limits, so anything else is a library bug.
#[track_caller]
fn lent_string(value: Str) -> String {
    if value.len == 0 { return String::new(); }
    String::from_utf8(unsafe { core::slice::from_raw_parts(value.ptr, value.len) }.to_vec()).expect("pxui: the library returned text that is not UTF-8")
}

#[inline]
fn status(rc:i32)->Result<(),i32>{ if rc==0 { Ok(()) } else { Err(rc) } }
