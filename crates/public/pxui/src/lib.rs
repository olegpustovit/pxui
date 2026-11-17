#![allow(clippy::all)]
#![cfg_attr(feature = "static", allow(unused_mut, dead_code))] // the static build runs callbacks without a trampoline and calls trusted entries: `mut f` and the C-callback machinery serve the dynamic one
//! Stable PXUI wrapper. Shape and geometry types are re-exported from the std-only public crate.

#[macro_use]
mod macros;
mod error;
pub use error::Error;
use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;
pub use pxui_types::{Environment, Keyboard, FrameIn, FrameOut, PxuiBox, PxuiPanel, PxuiTargetDesc, PxuiUiDesc};
pub use pxui_types::{Capitalize, Commands, Event, Gpu, Image, Pass, ReturnKey, Str, TextTraits, Touch};
pub use pxui_types::ffi::Slice;
pub use pxui_types::theme::{BodyId, BodyKind, Part};
pub use a11y_ffi::A11yBridge;
pub use pxui_types::{Label,TextSpan};
pub use pxui_types::ui::{Dialog, DialogButton, DialogResponse};

pub use pxui_types::{draw, geom};
pub use pxui_types::ffi::{A11yAction, A11yNode, A11yTree, Input, Output, PlatformOut, ThemeSource};

pub use pxui_types::theme;
pub mod images;
pub mod doc;
#[cfg(doctest)] mod guide;
pub mod testing;

/// The engine's module paths, so an app moves to the wrapper by swapping its dependency.
pub mod a11y {
    pub use pxui_types::a11y::{state, Role};
    pub use pxui_types::ffi::{A11yAction, A11yNode, A11yTree};
}
pub mod ui_core {
    pub use pxui_types::ui::*;
    pub use crate::opts::{BoxOpts, HitFn};
    pub use crate::{GestureOpts, GestureState, Scroll, SwipeActions, DismissOpts, ViewTransform, Paint, Ui};
    pub use pxui_types::PxuiBox as UiBox;
    pub use pxui_types::{Flags, Size};
    pub use pxui_types::node::{Axis, Bar, Dir, Mark, Node};
    /// The key of a node handle.
    #[inline] pub fn key(b: &UiBox) -> i64 { b.box_key }
}
pub mod ui_text {
    /// Whether `signal` is a right click: released with the right button within 4 points of its press.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::context_clicked, Signal};
    /// let click = Signal { released: true, button: Some(1), press_point: (10.0, 10.0), release_point: (11.0, 10.0), ..Signal::default() };
    /// assert!(context_clicked(&click));
    /// assert!(!context_clicked(&Signal { release_point: (40.0, 10.0), ..click }));
    /// ```
    pub fn context_clicked(signal: &crate::Signal) -> bool {
        let ((px, py), (rx, ry)) = (signal.press_point, signal.release_point);
        signal.released && signal.button == Some(1) && (px - rx) * (px - rx) + (py - ry) * (py - ry) < 16.0
    }
    pub use pxui_types::text_edit::{scrubbed, fuzzy_match, fuzzy_score, line_end, line_start};
    pub use pxui_types::ui::{CodeContext, Completion, Menu, MenuItem, MenuResponse, Pick};
    pub use crate::{Language, ScrubPhase, TextArea, ValueField};
}
pub mod ui_widgets { pub use crate::{Dialog, DialogButton, DialogResponse, Scrub, Segment, TreeNode}; }
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
pub use text::{PreparedText, RetiredText, Text, TextStyle, TextWorker, UiText};
pub use box_ops::{NodeFields, PayloadRef, DropRef, GestureOpts, GestureState, Scroll, SwipeActions, DismissOpts, ViewTransform};
pub mod paint;
pub use paint::Paint;
pub mod widgets;
pub use widgets::Scrub;
pub mod composite;
pub mod overlays;
pub use pxui_types::ui::{Checkbox, Choice, Dropdown, ListResponse, Menu, MenuItem, MenuResponse, Overlay, Pick, PickRows, PickerResponse, SelectionList, Slider, TableResponse, TableView, Toast, TreeResponse};
pub mod collections;
pub mod bodies;
pub use collections::TreeNode;
pub mod image;
mod text_inputs;
pub use text_inputs::ScrubPhase;
pub mod opts;
pub mod kit;
pub use pxui_types::ui::{Keyframes, Repeat};
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
mod author;
pub use author::{hash_of, AreaCallbacks, AreaNote, BracketsFn, Button, ColorizeFn, CompleteFn, ContextAtFn, DescribeFn, Icon, IconButton, IndentFn, Key, Language, LineField, LineFieldResponse, NumberAtFn, PickerOpts, Pressable, Response, Reveal, RewriteFn, ScrubFn, Scrubber, SearchField, SearchFieldResponse, Segmented, SegmentedResponse, SlideFn, Splice, TextArea, TextAreaResponse, TextField, TextFieldResponse, Toggle, ValidFn, ValueCallbacks, ValueField, ValueFieldResponse};
pub use pxui_types::ui::Segment;
#[cfg(test)]
mod settled;
#[cfg(test)]
mod surface;
#[cfg(test)]
mod panics;
pub use interaction::{Signal, UiSaved};
pub use pxui_types::Node;

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::*;
    // what the library's trusted entries replace: this wrapper's arguments are valid Rust values (see `pxui_abi::trusted`)
    pub use pxui_abi_link::trusted::{pxui_ui_text, pxui_ui_theme};
}

#[cfg(not(feature = "static"))]
mod sys {
    use core::ffi::c_void;
    use pxui_types::{FrameIn, FrameOut, Str};
    use pxui_types::ffi::{A11yAction, ThemeSource};
    #[repr(C)] pub struct PxuiUi { _private: [u8; 0] }
    pub use pxui_types::{PxuiBox, PxuiTargetDesc, PxuiUiDesc};
    #[repr(C)] pub struct PxuiRenderer { _private:[u8;0] }
    #[repr(C)] pub struct PxuiTheme { _private:[u8;0] }
    #[repr(C)] pub struct PxuiPaint { _private:[u8;0] }
    #[repr(C)] pub struct PxuiShaper { _private:[u8;0] }
    #[repr(C)] pub struct PxuiPreparedText { _private:[u8;0] }
    #[repr(C)] pub struct PxuiTextWorker { _private:[u8;0] }
    #[link(name = "pxui")]
    unsafe extern "C" {
        pub safe fn pxui_abi_version()->u32;
        pub safe fn pxui_supports(id:u32)->u32;
        pub safe fn pxui_counter(which:u32)->u64;
        pub fn pxui_ui_create(desc:*const PxuiUiDesc) -> *mut PxuiUi;
        pub fn pxui_ui_create_with_fonts(desc:*const PxuiUiDesc,fonts:*const Str,count:usize)->*mut PxuiUi;
        pub fn pxui_ui_output(ui:*mut PxuiUi)->*const pxui_types::ffi::Output;
        pub fn pxui_ui_theme(ui:*mut PxuiUi)->*const pxui_types::theme::ThemeSnapshot;
        pub fn pxui_ui_view_size(ui:*mut PxuiUi,out:*mut pxui_types::geom::Point)->i32;
        pub fn pxui_ui_layer_drawn(ui:*mut PxuiUi,layer:u64)->i32;
        pub fn pxui_ui_input(ui:*mut PxuiUi,has_owner:u8,owner:i64,out:*mut pxui_types::ffi::Input)->i32;
        pub fn pxui_ui_frame(ui:*mut PxuiUi,input:*const pxui_types::ffi::Input,build:pxui_types::ffi::UiBuild,user:*mut c_void,out:*mut FrameOut)->i32;
        pub fn pxui_ui_system_appearance(ui:*mut PxuiUi,out:*mut u32)->i32;
        pub fn pxui_ui_set_theme(ui:*mut PxuiUi,source:*const ThemeSource)->i32;
        pub fn pxui_ui_set_dir(ui:*mut PxuiUi,dir:u32)->i32;
        pub fn pxui_ui_set_a11y(ui:*mut PxuiUi,on:u8)->i32;
        pub fn pxui_ui_inject_a11y(ui:*mut PxuiUi,actions:*const A11yAction,count:usize)->i32;
        pub fn pxui_ui_patch_text(ui:*mut PxuiUi,box_:*const PxuiBox,value:Str,out:*mut u8)->i32;
        pub fn pxui_ui_busy_step(ui:*mut PxuiUi,box_:*const PxuiBox,now:f64,out:*mut u8)->i32;
        pub fn pxui_ui_request_frame(ui:*mut PxuiUi)->i32;
        pub fn pxui_ui_rect(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut pxui_types::geom::Rect)->i32;
        pub fn pxui_ui_box_of_key(ui:*mut PxuiUi,key:i64,out:*mut PxuiBox)->u8;
        pub fn pxui_ui_box_of_a11y(ui:*mut PxuiUi,id:u64,out:*mut PxuiBox)->u8;
        pub fn pxui_ui_text(ui:*mut PxuiUi,box_:*const PxuiBox,value:Str)->i32;
        pub fn pxui_ui_view(ui:*mut PxuiUi,label:Str,revision:u64,width:pxui_types::ffi::PxuiSizeDesc,height:pxui_types::ffi::PxuiSizeDesc,build:pxui_types::ffi::UiBuild,user:*mut c_void,out:*mut PxuiBox)->i32;
        pub fn pxui_ui_destroy(ui: *mut PxuiUi) -> i32;
        pub fn pxui_last_error(ui: *const PxuiUi) -> Str;
        pub fn pxui_frame(ui:*mut PxuiUi,input:*const FrameIn,build:pxui_types::ffi::UiBuild,user:*mut c_void,out:*mut FrameOut)->i32;
        pub fn pxui_within(ui:*mut PxuiUi,box_:*const PxuiBox,build:pxui_types::ffi::UiBuild,user:*mut c_void)->i32;
        pub fn pxui_scope(ui:*mut PxuiUi,label:Str,build:pxui_types::ffi::UiBuild,user:*mut c_void)->i32;
        pub fn pxui_col(ui:*mut PxuiUi,padding:f32,label:Str,build:pxui_types::ffi::UiBuild,user:*mut c_void)->i32;
        pub fn pxui_renderer_create(ui:*mut PxuiUi,gpu:*const pxui_types::Gpu,target:*const PxuiTargetDesc)->*mut PxuiRenderer;
        pub fn pxui_renderer_prepare(renderer:*mut PxuiRenderer,commands:*const pxui_types::Commands,frame:u64,completed:u64)->i32;
        pub fn pxui_renderer_resize(renderer:*mut PxuiRenderer,width:u32,height:u32)->i32;
        pub fn pxui_renderer_draw(renderer:*mut PxuiRenderer,pass:*const pxui_types::Pass)->i32;
        pub fn pxui_renderer_forget_image(renderer:*mut PxuiRenderer,image:*const pxui_types::Image,completed:u64)->i32;
        pub fn pxui_renderer_draw_calls(renderer:*mut PxuiRenderer)->u64;
        pub fn pxui_renderer_atlas(renderer:*mut PxuiRenderer,kind:u32,out:*mut pxui_types::Image)->i32;
        pub fn pxui_renderer_destroy(renderer:*mut PxuiRenderer);
    }
}

#[derive(Clone, Copy, Default)]
pub struct TargetDesc {
    pub format:u32,
    pub samples:u8,
}

/// A renderer on the app's device, bound to the `Ui` it was made from. Either may drop first: a `Ui` dropped while its renderer lives
/// is destroyed when the renderer drops.
pub struct Renderer { raw:NonNull<sys::PxuiRenderer> }

impl Renderer {
    /// A renderer on the app's device; the error is the library's reason (the device, the target or the `Ui` was refused).
    pub fn new(ui:&Ui,gpu:pxui_types::Gpu,target:TargetDesc)->Result<Self,String> {
        let desc=sys::PxuiTargetDesc { format:target.format,samples:target.samples,..sys::PxuiTargetDesc::default() };
        let raw=unsafe { sys::pxui_renderer_create(ui.0.as_ptr(),&gpu,&desc) };
        match NonNull::new(raw) { Some(raw)=>Ok(Self { raw }), None=>Err(ui.last_error().to_owned()) }
    }
    /// Errors: InvalidInput for a frame serial that repeats, regresses or runs more than the in-flight frames past `completed`; Gpu when recording fails.
    pub fn prepare(&mut self,commands:&pxui_types::Commands,frame:u64,completed:u64)->Result<(),Error> {
        let rc=unsafe { sys::pxui_renderer_prepare(self.raw.as_ptr(),commands,frame,completed) };
        composite::resume_deferred();
        if rc==0 { Ok(()) } else { Err(Error::from_code(rc)) }
    }
    /// Prepare blur targets at startup/resize, before recording the next frame. Dimensions are drawable pixels.
    ///
    /// Errors: InvalidInput for a zero dimension; State inside a frame or draw callback.
    pub fn resize(&mut self,width:u32,height:u32)->Result<(),Error> {
        let rc=unsafe { sys::pxui_renderer_resize(self.raw.as_ptr(),width,height) };
        if rc==0 { Ok(()) } else { Err(Error::from_code(rc)) }
    }
    /// Forget before destroying/reusing a native image, on every renderer that sampled it.
    /// `completed` is the host's completed GPU serial on the prepare() timeline; it must not go back.
    /// Refuses while any draw of the image remains incomplete. No additional frame is needed.
    ///
    /// Errors: InvalidInput for an unknown image or a `completed` that goes back; State while a draw of the image is incomplete.
    pub fn forget_image(&mut self,image:&Image,completed:u64)->Result<(),Error> {
        let rc=unsafe { sys::pxui_renderer_forget_image(self.raw.as_ptr(),image,completed) };
        if rc==0 { Ok(()) } else { Err(Error::from_code(rc)) }
    }
    /// Errors: Gpu when recording fails; State when a draw is already open.
    pub fn draw(&mut self,pass:&pxui_types::Pass)->Result<(),Error> {
        let rc=unsafe { sys::pxui_renderer_draw(self.raw.as_ptr(),pass) };
        composite::resume_deferred();
        if rc==0 { Ok(()) } else { Err(Error::from_code(rc)) }
    }
}

impl Renderer {
    /// The draw calls the renderer has issued so far.
    pub fn draw_calls(&self)->u64 { unsafe { sys::pxui_renderer_draw_calls(self.raw.as_ptr()) } }
}

/// A layer of the last frame: the key of its root box, its retained id, its instances, how many texture breaks they hold and the
/// instance ranges changed since it was last drawn.
#[derive(Clone, Copy, Debug)]
pub struct Layer<'a> {
    pub key: i64,
    pub id: u32,
    pub breaks: usize,
    pub instances: &'a [pxui_types::draw::Instance],
    pub dirty: &'a [pxui_types::ffi::PxuiDirty],
}

/// What a pass split hands the app's recorder: the colour target of the pass the app just ended.
pub type Recorder<'a> = &'a mut dyn FnMut(&pxui_types::Image) -> Result<(), Error>;

struct Split<'a> {
    f: &'a mut dyn FnMut(Recorder) -> Result<pxui_types::Pass, Error>,
    error: Option<Error>,
}

extern "C" fn split_pass(user: *mut c_void, record: pxui_types::PassRecord, record_user: *mut c_void, out: *mut pxui_types::Pass) -> i32 {
    if deferred_panic() { return -1; }
    let split = unsafe { &mut *user.cast::<Split>() };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut recorder = |image: &pxui_types::Image| -> Result<(), Error> { let rc = record(record_user, image); if rc == 0 { Ok(()) } else { Err(Error::from_code(rc)) } };
        (split.f)(&mut recorder)
    }));
    match result {
        Ok(Ok(next)) => { unsafe { *out = next; } 0 }
        Ok(Err(error)) => { split.error = Some(error); -1 }
        Err(panic) => { composite::defer_callback_panic(panic); -1 }
    }
}

#[inline]
fn deferred_panic() -> bool { composite::deferred() }

impl Renderer {
    /// Draws one layer of the last frame (`Ui::layers()[layer]`) into the app's open pass, so the app can draw its own content
    /// between layers. A layer with a backdrop blur calls `split`: end the open pass, hand `record` the colour target it holds so far
    /// (the library records the blur into the prepared command buffer), reopen a pass over the same target (load, not clear; the same
    /// viewport and scissor) and return it; the rest of the layer draws into that pass.
    ///
    /// Errors: Gpu when recording fails; the `Error` the `split` closure returned, when it failed.
    pub fn draw_layer(&mut self, pass: &pxui_types::Pass, layer: u32, split: &mut dyn FnMut(Recorder) -> Result<pxui_types::Pass, Error>) -> Result<(), Error> {
        let mut ctx = Split { f: split, error: None };
        let pass = pxui_types::Pass { first_layer: layer, layer_count: 1, split: Some(split_pass), split_user: (&mut ctx as *mut Split).cast(), ..*pass };
        let rc = unsafe { sys::pxui_renderer_draw(self.raw.as_ptr(), &pass) };
        composite::resume_deferred();
        if rc == 0 { Ok(()) } else { Err(ctx.error.unwrap_or(Error::from_code(rc))) }
    }
}

/// A glyph atlas of the renderer, for an app pass that draws text quads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtlasKind {
    Mask,
    Color,
    Mask2,
    Color2,
}

impl Renderer {
    /// The renderer's atlas: R8 for `Mask`/`Mask2`, RGBA8 for `Color`/`Color2`.
    /// Second pages return `PXUI_ERR_STATE` until a prepare uploads their first glyphs.
    ///
    /// Errors: State for a second page before a prepare has uploaded its first glyphs.
    pub fn atlas(&self, kind: AtlasKind) -> Result<pxui_types::Image, Error> {
        let mut out = pxui_types::Image::default();
        let rc = unsafe { sys::pxui_renderer_atlas(self.raw.as_ptr(), kind as u32, &mut out) };
        if rc == 0 { Ok(out) } else { Err(Error::from_code(rc)) }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe { sys::pxui_renderer_destroy(self.raw.as_ptr()) }
        sweep_orphans();
    }
}

// The library refuses to destroy a `Ui` while a renderer of it lives (the renderer reads its engine). A `Ui` dropped first waits here
// and goes when the last renderer does, so no drop order leaks one.
thread_local! { static ORPHANS: std::cell::RefCell<Vec<NonNull<sys::PxuiUi>>> = const { std::cell::RefCell::new(Vec::new()) }; }

fn sweep_orphans() {
    let _ = ORPHANS.try_with(|orphans| orphans.borrow_mut().retain(|ui| unsafe { sys::pxui_ui_destroy(ui.as_ptr()) } != 0));
}

/// The input of a frame as routed to one owner: its scalars and the wire form of its keys, buttons and events (the numbering of
/// `input::wire`), borrowed from the `Ui`.
pub struct Routed<'a> {
    raw: Input,
    _ui: PhantomData<&'a mut Ui>,
}

impl Routed<'_> {
    /// Numeric touch payload borrowed from this routed frame. Non-touch events return None.
    pub fn touch(&self,index:usize)->Option<Touch> {
        let event=self.events().get(index)?;
        if event.kind!=Event::TOUCH || event.text.len!=core::mem::size_of::<Touch>() || event.text.ptr.is_null() {return None;}
        Some(unsafe {event.text.ptr.cast::<Touch>().read_unaligned()})
    }
    #[inline] pub fn input(&self) -> &Input { &self.raw }
    pub fn keys(&self) -> &[u32] { if self.raw.keys.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(self.raw.keys.ptr, self.raw.keys.len) } } }
    pub fn buttons(&self) -> &[u32] { if self.raw.buttons.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(self.raw.buttons.ptr, self.raw.buttons.len) } } }
    pub fn events(&self) -> &[Event] { if self.raw.events.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(self.raw.events.ptr, self.raw.events.len) } } }
}

/// The UTF-8 text of an event (empty when it has none).
pub fn event_text(event: &Event) -> &str {
    if event.kind==Event::TOUCH || event.text.len == 0 || event.text.ptr.is_null() { return ""; }
    core::str::from_utf8(unsafe { core::slice::from_raw_parts(event.text.ptr, event.text.len) }).unwrap_or("")
}

/// One built frame: the draw output and damage of `pxui_frame`. It borrows the `Ui` it came from, so the
/// layer pointers it holds cannot be read after the next frame invalidates them.
pub struct Frame<'a> {
    out:FrameOut,
    _ui:PhantomData<&'a mut Ui>,
}

impl core::ops::Deref for Frame<'_> { type Target=FrameOut; #[inline] fn deref(&self)->&FrameOut { &self.out } }

#[repr(transparent)]
pub struct Ui(NonNull<sys::PxuiUi>);

impl Ui {
    /// A `Ui` with the library's default font; `None` when the library's ABI is not the one this wrapper was built for.
    pub fn new() -> Option<Self> {
        Self::try_create(if cfg!(any(target_os = "macos", target_os = "ios")) { Platform::Apple } else { Platform::Other }, None, &[])
    }

    /// A `Ui` for `platform` with the kit font size `font_size` (the theme's when `None`) and `fonts` (the library's default
    /// font when empty). A caller bug (an unusable font, a library of another ABI major) panics with the library's message.
    #[track_caller]
    pub fn create(platform: Platform, font_size: Option<i64>, fonts: &[&[u8]]) -> Self {
        Self::try_create(platform, font_size, fonts).expect("pxui: the library refused to create a Ui (a font it cannot read, or a library of another ABI major version than this wrapper's)")
    }

    fn try_create(platform: Platform, font_size: Option<i64>, fonts: &[&[u8]]) -> Option<Self> {
        if abi_version() >> 16 != pxui_types::ABI_BUILT >> 16 { return None; }
        let desc = sys::PxuiUiDesc { platform: match platform { Platform::Apple => 1, Platform::Other => 2 }, font_size: font_size.map_or(0, |size| size as i32), ..sys::PxuiUiDesc::default() };
        if fonts.is_empty() { return NonNull::new(unsafe { sys::pxui_ui_create(&desc) }).map(Self); }
        let fonts: Vec<Str> = fonts.iter().map(|bytes| Str { ptr: bytes.as_ptr(), len: bytes.len() }).collect();
        NonNull::new(unsafe { sys::pxui_ui_create_with_fonts(&desc, fonts.as_ptr(), fonts.len()) }).map(Self)
    }

    /// The engine the library keeps behind this `Ui`, for the checks that compare the wrapper's drawing with the engine's own;
    /// static link only, valid while the `Ui` lives.
    #[cfg(feature = "static")]
    #[doc(hidden)]
    pub fn engine_ptr(&self) -> *const c_void { pxui_abi_link::engine_of(self.id()).cast() }

    /// The library's handle of this `Ui`: stable while it lives, so an app can key its own per-`Ui` state by it.
    #[inline] pub fn id(&self) -> usize { self.0.as_ptr() as usize }

    /// Builds one frame. A panic in `build` (or in any callback of a widget it called) is carried out of the
    /// library and resumed here, after the call returns.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("a", pxui::Label::DEFAULT); });
    /// ui.frame(&pxui::FrameIn { width: 320.0, height: 240.0, ..Default::default() }, &mut |ui| { ui.label("b", pxui::Label::DEFAULT); }).unwrap();
    /// assert_eq!(ui.frame_index(), 2);
    /// ```
    ///
    /// Errors: InvalidInput for malformed input (events, environment); State when called inside a frame callback; Panic when the build closure panicked in the library.
    pub fn frame(&mut self, input: &FrameIn, mut build: &mut dyn FnMut(&mut Ui)) -> Result<Frame<'_>, Error> {
        let mut out = FrameOut::default();
        #[cfg(feature = "static")]
        let rc = unsafe { pxui_abi_link::trusted::frame(self.0.as_ptr(), input, inner!(|ui: &mut Ui| build(ui)), &mut out) };
        #[cfg(not(feature = "static"))]
        let rc = unsafe { sys::pxui_frame(self.0.as_ptr(), input, Some(composite::call::<&mut dyn FnMut(&mut Ui)>), &mut build as *mut _ as *mut c_void, &mut out) };
        self.framed(rc, out)
    }

    /// Errors: As `frame`.
    pub fn input_frame(&mut self,input:&Input,mut build: &mut dyn FnMut(&mut Ui))->Result<Frame<'_>,Error> {
        let mut out=FrameOut::default();
        #[cfg(feature = "static")]
        let rc=unsafe { pxui_abi_link::trusted::input_frame(self.0.as_ptr(),input,inner!(|ui: &mut Ui| build(ui)),&mut out) };
        #[cfg(not(feature = "static"))]
        let rc=unsafe { sys::pxui_ui_frame(self.0.as_ptr(),input,Some(composite::call::<&mut dyn FnMut(&mut Ui)>),&mut build as *mut _ as *mut c_void,&mut out) };
        self.framed(rc, out)
    }

    /// The last frame's input as the engine routed it to `owner` (a viewport's root key; `None`: the app's), borrowed until the
    /// next frame or call. Panics before the first frame, like the engine.
    #[track_caller]
    pub fn routed_input(&mut self, owner: Option<i64>) -> Routed<'_> {
        let mut raw = Input::default();
        check(self, unsafe { sys::pxui_ui_input(self.0.as_ptr(), owner.is_some() as u8, owner.unwrap_or_default(), &mut raw) });
        Routed { raw, _ui: PhantomData }
    }

    fn framed(&mut self, rc:i32, out:FrameOut)->Result<Frame<'_>,Error> {
        composite::resume_deferred();
        if rc==0 { Ok(Frame { out, _ui:PhantomData }) } else { Err(Error::from_code(rc)) }
    }

    pub fn last_error(&self) -> &str {
        let s = unsafe { sys::pxui_last_error(self.0.as_ptr()) };
        if s.len == 0 || s.ptr.is_null() { return ""; }
        core::str::from_utf8(unsafe { core::slice::from_raw_parts(s.ptr, s.len) }).unwrap_or("invalid error string")
    }

    /// The accessibility nodes of the last frame (parents before children), when `set_a11y(true)`.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |ui| { ui.label("hi", pxui::Label::DEFAULT); });
    /// assert!(!ui.a11y_nodes().is_empty());
    /// ```
    pub fn a11y_nodes(&self) -> &[A11yNode] {
        let nodes = self.output().a11y.nodes;
        if nodes.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(nodes.ptr, nodes.len) } }
    }

    /// The text every node's name, description and text value ranges point into, and the revision of the tree
    /// (it changes only when the tree's content did) with its first top-level node (`u32::MAX` when empty).
    pub fn a11y_tree(&self) -> (&[u8], u64, u32) {
        let tree = self.output().a11y;
        let strings = if tree.strings.len == 0 { &[][..] } else { unsafe { core::slice::from_raw_parts(tree.strings.ptr, tree.strings.len) } };
        (strings, tree.revision, tree.first_child)
    }

    /// The accessible name of `node`, one of `a11y_nodes()`.
    pub fn a11y_name_of(&self, node: &A11yNode) -> &str {
        let strings = self.output().a11y.strings;
        if strings.len == 0 { return ""; }
        let text = unsafe { core::slice::from_raw_parts(strings.ptr, strings.len) };
        text.get(node.name_start as usize..node.name_end as usize).and_then(|name| core::str::from_utf8(name).ok()).unwrap_or("")
    }

    /// The text value of a current accessibility node; None for absent/numeric values.
    pub fn a11y_value_of(&self,node:&A11yNode)->Option<&str> {
        if node.value.tag!=1 {return None;}
        let strings=self.output().a11y.strings;
        let text=if strings.len==0 {&[][..]} else {unsafe {core::slice::from_raw_parts(strings.ptr,strings.len)}};
        text.get(node.value.text_start as usize..node.value.text_end as usize).and_then(|value|core::str::from_utf8(value).ok())
    }

    /// How many layers the last frame has (a floating root's each, in draw order).
    #[inline] pub fn layer_count(&self) -> usize { self.output().layers.len }

    /// Layer `index` of the last frame.
    pub fn layer(&self, index: usize) -> Layer<'_> {
        let layers = self.output().layers;
        assert!(index < layers.len, "pxui: layer {index} of {}", layers.len);
        let raw = unsafe { *layers.ptr.add(index) };
        let span = |ptr, len| if len == 0 { &[][..] } else { unsafe { core::slice::from_raw_parts(ptr, len) } };
        Layer { key: raw.key, id: raw.id, breaks: raw.breaks as usize, instances: span(raw.instances.ptr, raw.instances.len), dirty: { let d = raw.dirty; if d.len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(d.ptr, d.len) } } } }
    }

    /// The last frame's layers in draw order.
    pub fn layers(&self) -> impl Iterator<Item = Layer<'_>> + '_ { (0..self.layer_count()).map(|index| self.layer(index)) }

    /// Marks layer `index` of the last frame drawn (its dirty ranges clear), as `Renderer::draw_layer` does: for a host that
    /// reads the layers itself.
    #[track_caller] pub fn drawn(&mut self, index: usize) { check(self, unsafe { sys::pxui_ui_layer_drawn(self.0.as_ptr(), index as u64) }) }

    /// The output of the last frame, borrowed from the `Ui`: a new frame needs `&mut self`, so a stale read does not compile.
    #[inline]
    #[track_caller]
    pub fn output(&self) -> &Output {
        // null only when building the output panicked inside the library
        let output = unsafe { sys::pxui_ui_output(self.0.as_ptr()) };
        if output.is_null() { refused(self, pxui_types::PXUI_ERR_PANIC) }
        unsafe { &*output }
    }
    /// The last FrameIn's host appearance (UNKNOWN, LIGHT or DARK); available inside cached views.
    pub fn system_appearance(&self)->u32 {
        let mut value=0;
        check(self,unsafe { sys::pxui_ui_system_appearance(self.0.as_ptr(),&mut value) });
        value
    }

    /// The theme in effect's tokens, by value: what a canvas closure captures.
    #[inline] pub fn tokens(&self) -> pxui_types::theme::Tokens { self.theme().tokens }
    /// The theme in effect, whole: the library's own, lent (no copy). Anything that could change it takes `&mut self`, so the
    /// borrow cannot outlive it.
    /// Panics with the library's message when the call is refused (a `Ui` an earlier panic poisoned): the library returns null.
    #[inline]
    #[track_caller]
    pub fn theme(&self) -> &pxui_types::theme::Theme {
        let theme = unsafe { sys::pxui_ui_theme(self.0.as_ptr()) };
        if theme.is_null() { refused(self, pxui_types::PXUI_ERR_PANIC) }
        unsafe { &*theme }
    }

    #[inline] pub fn draw_revision(&self) -> u64 { self.output().draw_revision }
    #[inline] pub fn built_nodes(&self) -> usize { self.output().built_nodes as usize }
    #[inline] pub fn rebuilt_views(&self) -> usize { self.output().rebuilt_views as usize }
    #[inline] pub fn arranged_nodes(&self) -> usize { self.output().arranged_nodes as usize }
    #[track_caller]
    pub fn view_size(&self) -> (f32, f32) {
        let mut out = pxui_types::geom::Point::default();
        check(self, unsafe { sys::pxui_ui_view_size(self.0.as_ptr(), &mut out) });
        (out.x, out.y)
    }
    #[inline]
    pub fn repaint_at(&self) -> Option<f64> {
        let at = self.output().repaint_at;
        at.is_finite().then_some(at)
    }

    /// Replaces the theme from a theme file's text; the library refuses a file that does not parse (see `last_error`).
    ///
    /// Errors: InvalidInput for theme text the engine cannot parse; State inside a frame callback.
    pub fn set_theme_source(&mut self,source:&ThemeSource)->Result<(),Error> { status(unsafe{sys::pxui_ui_set_theme(self.0.as_ptr(),source)}) }
    /// Errors: As `set_theme_source`.
    pub fn set_theme(&mut self,source:&ThemeSource)->Result<(),Error> {self.set_theme_source(source)}
    #[track_caller] pub fn set_dir(&mut self,rtl:bool) {check(self, unsafe{sys::pxui_ui_set_dir(self.0.as_ptr(),rtl as u32)})}
    #[track_caller] pub fn set_a11y(&mut self,on:bool) {check(self, unsafe{sys::pxui_ui_set_a11y(self.0.as_ptr(),on as u8)})}
    #[track_caller] pub fn inject_a11y(&mut self,actions:&[A11yAction]) {check(self, unsafe{sys::pxui_ui_inject_a11y(self.0.as_ptr(),actions.as_ptr(),actions.len())})}
    #[inline] #[track_caller] pub fn patch_text(&mut self,box_:&PxuiBox,value:&str)->bool{let mut out=0;let rc=unsafe{sys::pxui_ui_patch_text(self.0.as_ptr(),box_,as_str(value),&mut out)};live(self,rc)&&out!=0}
    #[inline] #[track_caller] pub fn busy_step(&mut self,box_:&PxuiBox,now:f64)->bool{let mut out=0;let rc=unsafe{sys::pxui_ui_busy_step(self.0.as_ptr(),box_,now,&mut out)};live(self,rc)&&out!=0}
    #[track_caller] pub fn request_frame(&mut self) {check(self, unsafe{sys::pxui_ui_request_frame(self.0.as_ptr())})}
    #[inline]
    #[track_caller] pub fn rect(&self,box_:&PxuiBox)-> geom::Rect {
        let mut out=geom::Rect::default();
        #[cfg(feature = "static")]
        check(self, unsafe{pxui_abi_link::trusted::rect(self.0.as_ptr(),box_,&mut out)});
        #[cfg(not(feature = "static"))]
        check(self, unsafe{sys::pxui_ui_rect(self.0.as_ptr(),box_,&mut out)});
        out
    }
    #[inline] pub fn box_of_key(&self,key:i64)->Option<PxuiBox>{
        let mut out=PxuiBox::default();
        #[cfg(feature = "static")]
        let found=unsafe{pxui_abi_link::trusted::box_of_key(self.0.as_ptr(),key,&mut out)};
        #[cfg(not(feature = "static"))]
        let found=unsafe{sys::pxui_ui_box_of_key(self.0.as_ptr(),key,&mut out)}!=0;
        found.then_some(out)
    }
    /// The live node for a published accessibility identity; `None` after it retires.
    #[inline] pub fn box_of_a11y(&self,id:u64)->Option<PxuiBox> {
        let mut out=PxuiBox::default();
        #[cfg(feature = "static")]
        let found=unsafe {pxui_abi_link::pxui_ui_box_of_a11y(self.0.as_ptr(),id,&mut out)};
        #[cfg(not(feature = "static"))]
        let found=unsafe {sys::pxui_ui_box_of_a11y(self.0.as_ptr(),id,&mut out)};
        (found!=0).then_some(out)
    }
    #[inline] pub fn key_of(seed:i64,label:&str)->i64{pxui_types::ui::key_of(seed,label)}
    #[inline] #[track_caller] pub fn text(&mut self,box_:&PxuiBox,value:&str) {check(self, unsafe{sys::pxui_ui_text(self.0.as_ptr(),box_,as_str(value))})}

    #[inline]
    #[track_caller]
    pub fn within(&mut self, box_: &PxuiBox, mut f: &mut dyn FnMut(&mut Ui)) {
        #[cfg(feature = "static")]
        let rc = unsafe { pxui_abi_link::trusted::within(self.0.as_ptr(), box_, inner!(|ui: &mut Ui| f(ui))) };
        #[cfg(not(feature = "static"))]
        let rc = unsafe { sys::pxui_within(self.0.as_ptr(), box_, Some(composite::call::<&mut dyn FnMut(&mut Ui)>), &mut f as *mut _ as *mut c_void) };
        check(self, rc)
    }

    #[inline]
    #[track_caller]
    pub fn scope(&mut self, label: &str, mut f: &mut dyn FnMut(&mut Ui)) {
        #[cfg(feature = "static")]
        let rc = unsafe { pxui_abi_link::trusted::scope(self.0.as_ptr(), label, inner!(|ui: &mut Ui| f(ui))) };
        #[cfg(not(feature = "static"))]
        let rc = unsafe { sys::pxui_scope(self.0.as_ptr(), as_str(label), Some(composite::call::<&mut dyn FnMut(&mut Ui)>), &mut f as *mut _ as *mut c_void) };
        check(self, rc)
    }
    #[inline]
    #[track_caller]
    pub fn col(&mut self, padding:f32, label:&str, mut f: &mut dyn FnMut(&mut Ui)) {
        #[cfg(feature = "static")]
        let rc = unsafe { pxui_abi_link::trusted::col(self.0.as_ptr(), padding, label, inner!(|ui: &mut Ui| f(ui))) };
        #[cfg(not(feature = "static"))]
        let rc = unsafe { sys::pxui_col(self.0.as_ptr(), padding, as_str(label), Some(composite::call::<&mut dyn FnMut(&mut Ui)>), &mut f as *mut _ as *mut c_void) };
        check(self, rc)
    }
    /// A kit button with the theme's look: `true` on the frame a press and release both land inside it.
    ///
    /// It is `button_with("button", text, &Button::DEFAULT).clicked`; use `button_with` for any option or a fixed key.
    /// Allocates: never on a warm frame.
    /// Identity: position among siblings (every `button(..)` shares one key and takes order-stable duplicate keys); use `button_with(key, ..)` when the order can change.
    /// Panics: when the library refuses the call (outside a frame). Display text past 64 KiB is clipped at a character boundary, never refused.
    ///
    /// ```
    /// let mut clicked = false;
    /// let mut build = |ui: &mut pxui::Ui| { clicked |= ui.button("Save"); };
    /// let mut ui = pxui::doc::frames(1, &mut build);
    /// pxui::testing::click(&mut ui, 20.0, 12.0, &mut build);
    /// assert!(clicked);
    /// ```
    #[inline] #[track_caller] pub fn button(&mut self, text:&str) -> bool { self.button_with("button", text, &Button::DEFAULT).clicked }
}


impl Drop for Ui {
    fn drop(&mut self) {
        // refused while a renderer of this `Ui` lives: it goes when the renderer drops (see `ORPHANS`)
        if unsafe { sys::pxui_ui_destroy(self.0.as_ptr()) } != 0 { let _ = ORPHANS.try_with(|orphans| orphans.borrow_mut().push(self.0)); }
    }
}

/// A `&[&str]` as a span of `Str` for one call: up to 32 strings live on the stack, more on the heap.
pub(crate) struct Strs {
    stack:[Str;32],
    heap:Vec<Str>,
    len:usize,
}

impl Strs {
    #[inline]
    pub(crate) fn new(values:&[&str])->Self {
        let mut strs=Self { stack:[Str::default();32], heap:Vec::new(), len:values.len() };
        if values.len()<=32 { for (out,v) in strs.stack.iter_mut().zip(values) { *out=as_str(v); } }
        else { strs.heap=values.iter().map(|v|as_str(v)).collect(); }
        strs
    }
    /// `len` strings, the `i`-th `value(i)`, as `new` keeps them.
    #[inline]
    pub(crate) fn each<'a>(len:usize, value:&dyn Fn(usize)->&'a str)->Self {
        let mut strs=Self { stack:[Str::default();32], heap:Vec::new(), len };
        if len<=32 { for (i,out) in strs.stack[..len].iter_mut().enumerate() { *out=as_str(value(i)); } }
        else { strs.heap=(0..len).map(|i|as_str(value(i))).collect(); }
        strs
    }
    #[inline]
    pub(crate) fn span(&self)->pxui_types::ffi::Slice<Str> {
        let ptr=if self.len<=32 { self.stack.as_ptr() } else { self.heap.as_ptr() };
        pxui_types::ffi::Slice { ptr, len:self.len }
    }
}

/// Display text as the library takes it, clipped to the display-text cap (`MAX_LABEL_BYTES`, 64 KiB) at a character boundary: text past
/// 64 KiB from user data is clipped, never refused, so it cannot panic the app. Identifiers and editor text go through `as_text`.
#[inline]
fn as_str(s: &str) -> Str { as_text(clip(s)) }

/// A string as the library takes it, unchanged: identifiers (refused past 256 bytes: a programmer error), editor text (16 MiB) and
/// text the text service measures or shapes.
#[inline]
fn as_text(s: &str) -> Str { Str { ptr: s.as_ptr(), len: s.len() } }

/// The library's cap on one display text (`docs/abi.md`, Limits).
pub(crate) const MAX_LABEL_BYTES: usize = 64 << 10;

/// `s` cut to `MAX_LABEL_BYTES` at a character boundary (itself when shorter). Allocates nothing.
#[inline]
pub(crate) fn clip(s: &str) -> &str {
    if s.len() <= MAX_LABEL_BYTES { return s; }
    let mut end = MAX_LABEL_BYTES;
    while !s.is_char_boundary(end) { end -= 1; }
    &s[..end]
}

/// `rc` of a call the engine tolerates a stale handle for: true when it ran, false when the handle had gone; anything else is the
/// caller bug `check` reports.
#[track_caller]
fn live(ui: &Ui, rc: i32) -> bool {
    if rc == pxui_types::PXUI_ERR_STALE { return false; }
    check(ui, rc);
    true
}

/// A failed call is a caller bug the engine would panic on (stale handle, non-finite number, oversized text):
/// it panics with the library's message, inside the frame callback (carried to the app's `frame` call) or at the call.
#[inline]
#[track_caller]
fn check(ui: &Ui, rc: i32) { if rc != 0 { refused(ui, rc) } }

/// The panic of a refused call, out of line: the many callers of `check` stay small enough to inline.
#[cold]
#[inline(never)]
#[track_caller]
fn refused(ui: &Ui, rc: i32) -> ! { panic!("pxui: {} ({rc})", ui.last_error()) }
/// The library's process counter, for the performance gate: heap allocations its allocator has made (`COUNTER_ALLOCATIONS`). They
/// count only in a counting build (a `libpxui` built with `pxui_dylib/counting`; statically, the app's `mem_sys/counting`) and read 0
/// otherwise; in a static build they are the app's own allocator. Calls are counted per `Ui` (`Ui::library_calls`), not here.
#[cfg(feature="static")]
pub fn library_counter(which:u32)->u64 { unsafe { sys::pxui_counter(which) } }
#[cfg(not(feature="static"))]
pub fn library_counter(which:u32)->u64 { sys::pxui_counter(which) }
pub const COUNTER_ALLOCATIONS:u32=1;
/// A capability a library may or may not have, as `pxui_supports` numbers them (`PXUI_FEATURE_*`; ids are append-only and mirror `pxui_types::feature`).
#[repr(u32)]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    /// The accessibility bridge of a window the app owns (Linux).
    A11yBridge = pxui_types::feature::A11Y_BRIDGE,
    /// The Metal renderer backend. Answers from the target today (Apple); must switch to `cfg!(feature = ..)` if a backend becomes optional.
    Metal = pxui_types::feature::METAL,
    /// The Vulkan renderer backend. Answers from the target today (Windows, Linux); must switch to `cfg!(feature = ..)` if a backend becomes optional.
    Vulkan = pxui_types::feature::VULKAN,
    /// The extension set (no stability promise).
    ExtensionSet = pxui_types::feature::EXTENSION_SET,
    /// `Button` contents, label, axis, gap and place (1.56).
    ButtonContent = pxui_types::feature::BUTTON_CONTENT,
    /// The icon button's press curves (1.54).
    IconButtonCurves = pxui_types::feature::ICON_BUTTON_CURVES,
    /// `Toggle` options and response (1.51).
    ToggleResponse = pxui_types::feature::TOGGLE_RESPONSE,
    /// `Segmented` options and response (1.51).
    SegmentedResponse = pxui_types::feature::SEGMENTED_RESPONSE,
    /// The dialog response call (1.53).
    DialogResponse = pxui_types::feature::DIALOG_RESPONSE,
    /// Widget body variants (1.52).
    WidgetVariants = pxui_types::feature::WIDGET_VARIANTS,
    /// `ui_label_node` (1.54).
    LabelNode = pxui_types::feature::LABEL_NODE,
    /// Menu rows keyed by id (1.58).
    MenuItemIds = pxui_types::feature::MENU_ITEM_IDS,
    /// `FrameIn.surface` (1.57).
    FrameSurface = pxui_types::feature::FRAME_SURFACE,
    /// `ui_item_text` (1.49).
    ItemText = pxui_types::feature::ITEM_TEXT,
    /// App colour tokens (1.43).
    ColorTokens = pxui_types::feature::COLOR_TOKENS,
    /// Row swipe actions (1.40).
    SwipeActions = pxui_types::feature::SWIPE_ACTIONS,
    /// Drag-to-dismiss (1.41).
    Dismiss = pxui_types::feature::DISMISS,
    /// ABI 1.61: look colours that name colour tokens (`Color::token`).
    LookTokens = pxui_types::feature::LOOK_TOKENS,
    /// ABI 1.61: `Icon::draw(f, word)` passes the word to the drawing.
    IconWord = pxui_types::feature::ICON_WORD,
    /// ABI 1.62: `TextWorker`, `Ui::set_text_worker`, `Ui::prewarm_text` (the prepare queue).
    TextWorker = pxui_types::feature::TEXT_WORKER,
    /// ABI 1.67: `Menu::max_h` bounds the sheet's height.
    MenuMaxH = pxui_types::feature::MENU_MAX_H,
    /// ABI 1.67: a segmented control with no choice, reporting the option a secondary press began on; 1.67 also changes the keyboard
    /// model of every segmented control (one Tab stop, the chosen option; every option focusable to assistive technology).
    SegmentedResult = pxui_types::feature::SEGMENTED_RESULT,
    /// ABI 1.68: the keyed widget and container calls (`PxuiKeyedDesc`): key and name apart for checkbox, radio, tabs, dropdown, slider,
    /// progress, spinner, badge, panel, popup, modal, anchored, scroll_view, toasts, table_view and picker.
    WidgetsKeyed = pxui_types::feature::WIDGETS_KEYED,
    /// ABI 1.69: `Segmented::focus_on_click`, and `PxuiSegmentConfig::stored` for a registered segment body.
    SegmentedStoredFocus = pxui_types::feature::SEGMENTED_STORED_FOCUS,
}

/// Whether the library has `feature`, as it was built: backends and the accessibility bridge depend on the platform, the rest on the library's minor.
///
/// ```
/// let _ = pxui::supports(pxui::Feature::ExtensionSet);
/// ```
///
/// Allocates: nothing. Panics: never.
#[cfg(feature="static")]
pub fn supports(feature:Feature)->bool { unsafe { sys::pxui_supports(feature as u32) != 0 } }
#[cfg(not(feature="static"))]
pub fn supports(feature:Feature)->bool { sys::pxui_supports(feature as u32) != 0 }
#[cfg(feature="static")]
#[inline]
fn abi_version()->u32 { unsafe { sys::pxui_abi_version() } }
#[cfg(not(feature="static"))]
#[inline]
fn abi_version()->u32 { sys::pxui_abi_version() }

/// A string the library lent, borrowed: it only returns UTF-8, so anything else is a library bug.
#[track_caller]
fn lent_str<'a>(value: Str) -> &'a str {
    if value.len == 0 { return ""; }
    core::str::from_utf8(unsafe { core::slice::from_raw_parts(value.ptr, value.len) }).expect("pxui: the library returned text that is not UTF-8")
}

/// A string the library lent, copied.
#[track_caller]
fn lent_string(value: Str) -> String { lent_str(value).to_owned() }

#[inline]
fn status(rc:i32)->Result<(),Error>{ if rc==0 { Ok(()) } else { Err(Error::from_code(rc)) } }
