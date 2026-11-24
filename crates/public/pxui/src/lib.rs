#![allow(clippy::all)]
#![deny(missing_docs)]
#![cfg_attr(feature = "static", allow(unused_mut, dead_code))] // the static build runs callbacks without a trampoline and calls trusted entries: `mut f` and the C-callback machinery serve the dynamic one
//! Stable PXUI wrapper. Shape and geometry types are re-exported from the std-only public crate.
//! Fallible data and host operations return [Error]. Queries use Option for absence, and build
//! operations may panic on caller errors documented on the operation.

#[macro_use]
mod macros;
mod error;
pub use error::Error;
use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;
pub use pxui_types::{Appearance, Environment, Keyboard, FrameOut, PxuiPanel, PxuiTargetDesc, PxuiUiDesc};
use pxui_types::FrameIn as WireFrame;
// the C record `Handle` names; the wrapper's own signatures say `Handle`
use pxui_types::PxuiBox;
pub use pxui_types::{Capitalize, Commands, Event, Gpu, Image, Pass, ReturnKey, Str, TextTraits, Touch};
pub use pxui_types::ffi::Slice;
pub use pxui_types::theme::{BodyId, BodyKind, Part};
pub use a11y_ffi::A11yBridge;
pub use pxui_types::{Label,TextSpan};
pub use pxui_types::ui::{Dialog, DialogButton};

pub use pxui_types::{draw, geom};
pub use pxui_types::a11y::Role;
pub use pxui_types::geom::{Point, Rect, Rgba};
pub use pxui_types::{Flags, Size};
pub use pxui_types::ffi::{A11yAction, A11yNode, Input, Output, PlatformOut, PxuiDamageList, PxuiFrameReasons, PxuiPromoted};
#[doc(hidden)]
pub use pxui_types::damage::DamageList;

pub use pxui_types::theme;
pub mod images;
#[doc(hidden)]
pub mod doc;
pub mod guide;
pub mod testing;

/// The extension set: the inspector rows, the footer, the line and value fields, the grip and their getters, as free functions taking
/// the `Ui`.
///
/// These are the controls of one application (Rays' inspector), kept out of the prelude (`pxui::ui`) and out of the stable surface:
/// they may change or move to that application with no notice. Their C exports are listed apart in `abi/pxui.ext.txt`. The inspector
/// toggle and button are built through the toggle and button bodies, so a registered body draws them too.
pub mod ext {
    pub use crate::author::fields::{line_field, value_field};
    pub use crate::kit::{footer, inspector_button, inspector_message, inspector_readout, inspector_row, inspector_section, inspector_toggle};
    pub use crate::kit_ext::{field_drag, grip, inspector_bar, inspector_body, inspector_header};
    pub use crate::ui_state::{inspector_label_x, inspector_width};
}

/// The types and operations used to build a UI.
pub mod ui {
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
    pub use crate::{Dialog, DialogButton, DismissOpts, Fill, GestureOpts, GestureState, Handle, ImageStyle, Language, Paint, ScrubPhase, Scroll, Segment, Signal, Stroke, SwipeActions, TextArea, TreeNode, Ui, ValueField, ViewTransform};
    pub use pxui_types::text_edit::{scrubbed, fuzzy_match, fuzzy_score, line_end, line_start};
    pub use pxui_types::ui::{
        Checkbox, Chosen, CodeContext, Completion, Cursor, Dropdown, Head, HeadAction, Key, Keyframe, Keyframes,
        LineStep, ListResponse, Menu, MenuItem, Overlay, PanelOpts, Pick, PickArt, PickRow, PickRows, PickerResponse, Presentation, Rate, Repeat, SectionList,
        SectionPart, Segmented, SelectionList, Slider, Target, TextEdit, Toast, TreeResponse,
        UndoKind, DRAG_THRESHOLD, ROOT_KEY, int_key, key_of,
    };
    pub use pxui_types::{Flags, Image, Size};
    pub use pxui_types::node::{Axis, Bar, Dir, Mark, Node, Transition};
    pub use pxui_types::draw::*;
    pub use pxui_types::geom::*;
    pub use pxui_types::layout::*;
    pub use pxui_types::theme::{Curve, Font, Look, Ring, Shadow, State, States, Tokens};
    pub use pxui_types::a11y::Role;
}
/// A node's identity as returned by a build operation (the C record is `PxuiBox`).
pub type Handle = pxui_types::PxuiBox;
mod text;
pub use text::{pragmasevka, PreparedText, RetiredText, Text, TextStyle, TextWorker, UiText};
pub use pxui_types::text::{Ellipsis, TextAlign, Wrap};
pub use box_ops::{NodeFields, PayloadRef, DropRef, GestureOpts, GestureState, Scroll, SwipeActions, ViewTransform};
mod paint;
pub use paint::{Fill, Paint, Stroke};
mod widgets;
mod composite;
mod overlays;
pub use pxui_types::ui::{Checkbox, DismissOpts, Dropdown, ListResponse, Menu, MenuItem, Chosen, Overlay, Pick, PickArt, PickRow, PickRows, PickerResponse, Presentation, SelectMode, SelectionList, Slider, Toast, TreeResponse};
mod collections;
pub mod bodies;
pub use collections::TreeNode;
mod image;
pub use image::ImageStyle;
mod text_inputs;
pub use text_inputs::ScrubPhase;
mod frame;
pub use frame::FrameIn;
mod kit;
pub use pxui_types::ui::{Keyframes, Rate, Repeat};
#[path = "a11y.rs"]
mod a11y_ffi;
mod a11y_state;
mod style;
mod interaction;
mod ui_state;
mod box_ops;
mod kit_ext;
mod theme_scope;
pub use theme_scope::ThemeHandle;
pub use a11y_state::A11yStates;
pub use ui_state::{Bytes, Ime, Platform};
mod node;
mod author;
pub use author::{hash_of, AreaCallbacks, AreaNote, BracketsFn, Button, ColorizeFn, CompleteFn, ContextAtFn, DescribeFn, Icon, IconButton, IndentFn, Key, Language, LineField, LineFieldResponse, NumberAtFn, Picker, Pressable, Response, Reveal, RewriteFn, ScrubFn, Scrubber, SearchField, SearchFieldResponse, Segmented, SlideFn, Splice, TextArea, TextAreaResponse, TextField, TextFieldResponse, Toggle, ValidFn, ValueCallbacks, ValueField, ValueFieldResponse};
pub use author::{Badge, CellState, Column, GridList, HeadResponse, ListState, OverlayResponse, Sort, Table, TableHead, TableResponse, TableState, TreeState, Disclosure, PickResponse, Progress, Radio, SectionIndex, SliderResponse, Spinner, SplitResponse, Tabs};
pub use kit::{MenuBar, MenuBarMenu, Split};
pub use pxui_types::ui::Segment;
#[cfg(test)]
mod settled;
#[cfg(test)]
mod surface;
#[cfg(test)]
mod panics;
pub use interaction::Signal;
mod keys;
pub use keys::{KeyCode, Mods};
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
    /// The handle's public head (`include/pxui.h`): the theme in effect for it, null until a theme read fills it.
    #[repr(C)] pub struct PxuiUi { pub theme: *const pxui_types::theme::ThemeSnapshot, _private: [u8; 0] }
    pub use pxui_types::{PxuiBox, PxuiUiDesc};
    /// Opaque ABI handle for a native renderer.
    #[repr(C)] pub struct PxuiRenderer { _private:[u8;0] }
    /// Opaque ABI handle for a loaded theme.
    #[repr(C)] pub struct PxuiTheme { _private:[u8;0] }
    /// Opaque ABI handle for a active painter.
    #[repr(C)] pub struct PxuiPaint { _private:[u8;0] }
    /// Opaque ABI handle for a standalone text shaper.
    #[repr(C)] pub struct PxuiShaper { _private:[u8;0] }
    /// Opaque ABI handle for a prepared text buffer.
    #[repr(C)] pub struct PxuiPreparedText { _private:[u8;0] }
    /// Opaque ABI handle for a text preparation worker.
    #[repr(C)] pub struct PxuiTextWorker { _private:[u8;0] }
    #[link(name = "pxui")]
    unsafe extern "C" {
        pub safe fn pxui_abi_version()->u32;
        pub safe fn pxui_supports(id:u32)->u32;
        pub safe fn pxui_counter(which:u32)->u64;
        pub fn pxui_ui_create(desc:*const PxuiUiDesc,fonts:*const Str,count:usize,status:*mut i32) -> *mut PxuiUi;
        pub fn pxui_ui_output(ui:*mut PxuiUi)->*const pxui_types::ffi::Output;
        pub fn pxui_ui_set_promotion(ui:*mut PxuiUi,enabled:u8)->i32;
        pub fn pxui_ui_promoted(ui:*mut PxuiUi,ptr:*mut *const pxui_types::ffi::PxuiPromoted,len:*mut usize)->i32;
        pub fn pxui_ui_promote(ui:*mut PxuiUi,box_:*const PxuiBox)->i32;
        pub fn pxui_ui_damage_list(ui:*mut PxuiUi,out:*mut pxui_types::ffi::PxuiDamageList)->i32;
        pub fn pxui_ui_theme(ui:*mut PxuiUi)->*const pxui_types::theme::ThemeSnapshot;
        pub fn pxui_ui_view_size(ui:*mut PxuiUi,out:*mut pxui_types::geom::Point)->i32;
        pub fn pxui_ui_layer_drawn(ui:*mut PxuiUi,layer:u64)->i32;
        pub fn pxui_ui_system_appearance(ui:*mut PxuiUi,out:*mut u32)->i32;
        pub fn pxui_ui_set_dir(ui:*mut PxuiUi,dir:u32)->i32;
        pub fn pxui_ui_set_a11y(ui:*mut PxuiUi,on:u8)->i32;
        pub fn pxui_ui_inject_a11y(ui:*mut PxuiUi,actions:pxui_types::ffi::A11yActionSpan)->i32;
        pub fn pxui_ui_patch_text(ui:*mut PxuiUi,box_:*const PxuiBox,value:Str,out:*mut u8)->i32;
        pub fn pxui_ui_busy_step(ui:*mut PxuiUi,box_:*const PxuiBox,now:f64,out:*mut u8)->i32;
        pub fn pxui_ui_request_frame(ui:*mut PxuiUi)->i32;
        pub fn pxui_ui_frame_reasons(ui:*mut PxuiUi,out:*mut pxui_types::ffi::PxuiFrameReasons)->i32;
        pub fn pxui_ui_step(ui:*mut PxuiUi,time:f64,out:*mut u8)->i32;
        pub fn pxui_ui_rect(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut pxui_types::geom::Rect)->i32;
        pub fn pxui_ui_box_of_key(ui:*mut PxuiUi,key:i64,out:*mut PxuiBox)->u8;
        pub fn pxui_ui_key_seed(ui:*mut PxuiUi,out:*mut i64)->i32;
        pub fn pxui_ui_box_of_a11y(ui:*mut PxuiUi,id:u64,out:*mut PxuiBox)->u8;
        pub fn pxui_ui_text(ui:*mut PxuiUi,box_:*const PxuiBox,value:Str,style:*const pxui_types::ffi::PxuiLabelStyle,spans:*const pxui_types::TextSpans,line_box:u8)->i32;
        pub fn pxui_ui_destroy(ui: *mut PxuiUi) -> i32;
        pub fn pxui_last_error(ui: *const PxuiUi) -> Str;
        pub fn pxui_ui_add_surface(ui:*mut PxuiUi,surface:u64)->i32;
        pub fn pxui_ui_remove_surface(ui:*mut PxuiUi,surface:u64)->i32;
        pub fn pxui_frame(ui:*mut PxuiUi,input:*const FrameIn,build:pxui_types::ffi::UiBuild,user:*mut c_void,out:*mut FrameOut)->i32;
        pub fn pxui_ui_within(ui:*mut PxuiUi,box_:*const PxuiBox,build:pxui_types::ffi::UiBuild,user:*mut c_void)->i32;
        pub fn pxui_ui_scope(ui:*mut PxuiUi,label:Str,build:pxui_types::ffi::UiBuild,user:*mut c_void)->i32;
        pub fn pxui_renderer_prepare(renderer:*mut PxuiRenderer,commands:*const pxui_types::Commands,frame:u64,completed:u64)->i32;
        pub fn pxui_renderer_resize(renderer:*mut PxuiRenderer,width:u32,height:u32)->i32;
        pub fn pxui_renderer_exclude_promoted(renderer:*mut PxuiRenderer,exclude:u8)->i32;
        pub fn pxui_renderer_draw(renderer:*mut PxuiRenderer,pass:*const pxui_types::Pass)->i32;
        pub fn pxui_renderer_draw_damage(renderer:*mut PxuiRenderer,pass:*const pxui_types::Pass,x:u32,y:u32,width:u32,height:u32)->i32;
        pub fn pxui_renderer_draw_damage_rects(renderer:*mut PxuiRenderer,pass:*const pxui_types::Pass,rects:*const u32,count:u32)->i32;
        pub fn pxui_renderer_forget_image(renderer:*mut PxuiRenderer,image:*const pxui_types::Image,completed:u64)->i32;
        pub fn pxui_renderer_draw_calls(renderer:*mut PxuiRenderer)->u64;
        pub fn pxui_renderer_atlas(renderer:*mut PxuiRenderer,kind:u32,out:*mut pxui_types::Image)->i32;
        pub fn pxui_renderer_destroy(renderer:*mut PxuiRenderer);
    }
}

library!(renderer_lib {
    fn pxui_renderer_create(ui: *mut PxuiUi, gpu: *const Gpu, target: *const PxuiTargetDesc, out: *mut *mut sys::PxuiRenderer) -> i32;
});

/// Pixel format and sample count for a render target.
#[derive(Clone, Copy, Default)]
pub struct TargetDesc {
    /// Pixel format code.
    pub format:u32,
    /// Render target sample count.
    pub samples:u8,
}

/// A renderer on the app's device, bound to the `Ui` it was made from. Either may drop first: a `Ui` dropped while its renderer lives
/// is destroyed when the renderer drops.
pub struct Renderer { raw:NonNull<sys::PxuiRenderer> }

impl Renderer {
    /// Creates a renderer for the app's device and target, tied to `ui`.
    ///
    /// Use the owning Ui outside its frame callback. A call during a callback returns
    /// `Error::State`. Invalid descriptors return `Error::InvalidInput` and native
    /// GPU failures return `Error::Gpu`. `ui.last_error()` has the library's reason.
    /// The call takes the GPU lease even when creation fails.
    /// Allocates: the renderer and its GPU resources on success.
    /// Panics: never.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert!(matches!(pxui::Renderer::new(&ui, pxui::Gpu::default(), pxui::TargetDesc::default()), Err(pxui::Error::InvalidInput)));
    /// ```
    pub fn new(ui: &Ui, gpu: Gpu, target: TargetDesc) -> Result<Self, Error> {
        let desc = PxuiTargetDesc { format: target.format, samples: target.samples, ..PxuiTargetDesc::default() };
        Self::new_with_target_record(ui, gpu, desc)
    }

    /// Creates a renderer with the complete target descriptor and its optional host guarantees.
    ///
    /// Takes the GPU lease and has the same ownership and error rules as [`Renderer::new`].
    /// Allocates: the renderer and its GPU resources on success.
    /// Panics: never.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert!(matches!(pxui::Renderer::new_with_target_record(&ui, pxui::Gpu::default(), pxui::PxuiTargetDesc::default()), Err(pxui::Error::InvalidInput)));
    /// ```
    pub fn new_with_target_record(ui: &Ui, mut gpu: Gpu, target: PxuiTargetDesc) -> Result<Self, Error> {
        // Rust supplied the full value. C cannot read the lease fields of a short descriptor.
        if gpu.size < core::mem::size_of::<Gpu>() as u32 {
            if let Some(release) = gpu.release_owner.take() { release(gpu.owner_token); }
        }
        let mut raw = core::ptr::null_mut();
        status(unsafe { renderer_lib::pxui_renderer_create(ui.0.as_ptr(), &gpu, &target, &mut raw) })?;
        NonNull::new(raw).map(|raw| Self { raw }).ok_or(Error::Engine)
    }
    /// Prepares a new renderer frame before the host begins its draw pass.
    ///
    /// Errors: InvalidInput for a frame serial that repeats, regresses or runs more than the in-flight frames past `completed`; Gpu when recording fails.
    /// Allocates: when preparing newly needed glyph pages, uploads, or renderer storage.
    /// Panics: if deferred host callback code panicked.
    ///
    /// ```
    /// fn example(renderer: &mut pxui::Renderer, commands: &pxui_types::Commands, frame: u64, completed: u64) -> Result<(), pxui::Error> { renderer.prepare(commands, frame, completed) }
    /// let _ = example;
    /// ```
    pub fn prepare(&mut self,commands:&pxui_types::Commands,frame:u64,completed:u64)->Result<(),Error> {
        let rc=unsafe { sys::pxui_renderer_prepare(self.raw.as_ptr(),commands,frame,completed) };
        composite::resume_deferred();
        if rc==0 { Ok(()) } else { Err(Error::from_code(rc)) }
    }
    /// Prepare blur targets at startup/resize, before recording the next frame. Dimensions are drawable pixels.
    /// Each accepted resize invalidates retained backdrop pixels, including a call with the same dimensions.
    ///
    /// Errors: InvalidInput for a zero dimension; State inside a frame or draw callback.
    /// Allocates: when the renderer needs new blur targets.
    /// Panics: never.
    ///
    /// ```
    /// fn example(renderer: &mut pxui::Renderer, width: u32, height: u32) -> Result<(), pxui::Error> { renderer.resize(width, height) }
    /// let _ = example;
    /// ```
    pub fn resize(&mut self,width:u32,height:u32)->Result<(),Error> {
        let rc=unsafe { sys::pxui_renderer_resize(self.raw.as_ptr(),width,height) };
        if rc==0 { Ok(()) } else { Err(Error::from_code(rc)) }
    }
    /// Forget before destroying/reusing a native image, on every renderer that sampled it.
    /// `completed` is the host's completed GPU serial on the prepare() timeline; it must not go back.
    /// Refuses while any draw of the image remains incomplete. No additional frame is needed.
    ///
    /// Errors: InvalidInput for an unknown image or a `completed` that goes back; State while a draw of the image is incomplete.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// fn example(renderer: &mut pxui::Renderer, image: &pxui::Image, completed: u64) -> Result<(), pxui::Error> { renderer.forget_image(image, completed) }
    /// let _ = example;
    /// ```
    pub fn forget_image(&mut self,image:&Image,completed:u64)->Result<(),Error> {
        let rc=unsafe { sys::pxui_renderer_forget_image(self.raw.as_ptr(),image,completed) };
        if rc==0 { Ok(()) } else { Err(Error::from_code(rc)) }
    }
    /// Omits promoted runs from subsequent draws when the host presents those runs separately.
    ///
    /// Pass `false` to render the entire scene into the target, including for offscreen capture.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// fn example(renderer: &mut pxui::Renderer) -> Result<(), pxui::Error> { renderer.exclude_promoted(true) }
    /// let _ = example;
    /// ```
    pub fn exclude_promoted(&mut self, exclude: bool) -> Result<(), Error> {
        status(unsafe { sys::pxui_renderer_exclude_promoted(self.raw.as_ptr(), exclude.into()) })
    }
    /// Records the prepared UI layers into the host’s open render pass.
    ///
    /// Errors: Gpu when recording fails; State when a draw is already open.
    /// Allocates: when render storage first grows or the host callback allocates.
    /// Panics: if the host callback panics.
    ///
    /// ```
    /// fn example(renderer: &mut pxui::Renderer, pass: &pxui_types::Pass) -> Result<(), pxui::Error> { renderer.draw(pass) }
    /// let _ = example;
    /// ```
    pub fn draw(&mut self,pass:&pxui_types::Pass)->Result<(),Error> {
        let rc=unsafe { sys::pxui_renderer_draw(self.raw.as_ptr(),pass) };
        composite::resume_deferred();
        if rc==0 { Ok(()) } else { Err(Error::from_code(rc)) }
    }
}

impl Renderer {
    /// The draw calls the renderer has issued so far.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// fn example(renderer: &mut pxui::Renderer) -> u64 { renderer.draw_calls() }
    /// let _ = example;
    /// ```
    pub fn draw_calls(&self)->u64 { unsafe { sys::pxui_renderer_draw_calls(self.raw.as_ptr()) } }
}

/// A draw layer borrowed from the last frame.
#[derive(Clone, Copy, Debug)]
pub struct Layer<'a> {
    /// The layer's root node key.
    pub key: i64,
    /// The retained layer's identity.
    pub id: u32,
    /// The number of texture breaks in the instance stream.
    pub breaks: usize,
    /// The complete draw instances in order.
    pub instances: &'a [pxui_types::draw::Instance],
    /// Instance ranges changed since the layer was last drawn.
    pub dirty: &'a [pxui_types::ffi::PxuiDirty],
}

/// The last frame's draw layers in order, borrowed from their Ui.
pub struct Layers<'a> {
    raw: pxui_types::ffi::Strided<pxui_types::ffi::PxuiLayerInfo>,
    next: usize,
    _ui: PhantomData<&'a Ui>,
}

impl<'a> Layers<'a> {
    fn get(&self, index: usize) -> Option<Layer<'a>> {
        // Acknowledgment may update metadata, but the span stays in place until a frame or patch.
        if index >= self.raw.len { return None; }
        let raw = unsafe { *self.raw.at(index) };
        let instances = if raw.instances.len == 0 { &[] } else {
            unsafe { core::slice::from_raw_parts(raw.instances.ptr, raw.instances.len) }
        };
        let dirty = if raw.dirty.len == 0 { &[] } else {
            unsafe { core::slice::from_raw_parts(raw.dirty.ptr, raw.dirty.len) }
        };
        Some(Layer { key: raw.key, id: raw.id, breaks: raw.breaks as usize, instances, dirty })
    }
}

impl<'a> Iterator for Layers<'a> {
    type Item = Layer<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let layer = self.get(self.next)?;
        self.next += 1;
        Some(layer)
    }
}

/// What a pass split hands the app's recorder: the colour target of the pass the app just ended.
pub type Recorder<'a> = &'a mut dyn FnMut(&pxui_types::Image) -> Result<(), Error>;

struct PassSplitContext<'a> {
    f: &'a mut dyn FnMut(Recorder) -> Result<pxui_types::Pass, Error>,
    error: Option<Error>,
}

extern "C" fn split_pass(user: *mut c_void, record: pxui_types::PassRecord, record_user: *mut c_void, out: *mut pxui_types::Pass) -> i32 {
    if deferred_panic() { return -1; }
    let split = unsafe { &mut *user.cast::<PassSplitContext>() };
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
    /// Draws one layer of the last frame (`Ui::layer(layer)`) into the app's open pass, so the app can draw its own content
    /// between layers. A layer with a backdrop blur calls `split`: end the open pass, hand `record` the colour target it holds so far
    /// (the library records the blur into the prepared command buffer), reopen a pass over the same target (load, not clear; the same
    /// viewport and scissor) and return it; the rest of the layer draws into that pass.
    ///
    /// Errors: Gpu when recording fails; the `Error` the `split` closure returned, when it failed.
    /// Allocates: when render storage first grows or the host callback allocates.
    /// Panics: if the host callback panics.
    ///
    /// ```
    /// fn example(renderer: &mut pxui::Renderer, pass: &pxui_types::Pass) -> Result<(), pxui::Error> { renderer.draw_layer(pass, 0, &mut |_| Err(pxui::Error::InvalidInput)) }
    /// let _ = example;
    /// ```
    pub fn draw_layer(&mut self, pass: &pxui_types::Pass, layer: u32, split: &mut dyn FnMut(Recorder) -> Result<pxui_types::Pass, Error>) -> Result<(), Error> {
        self.draw_with_split(&pxui_types::Pass { first_layer: layer, layer_count: 1, ..*pass }, split)
    }

    /// Draws the layer range in `pass`, splitting around backdrop blurs.
    ///
    /// The callback ends the current pass, records blur using its color target, and reopens the pass
    /// with the current pixels loaded. See [`Renderer::draw_layer`] for the split contract.
    /// Allocates: when render storage first grows or the host callback allocates.
    /// Panics: if the host callback panics.
    ///
    /// ```
    /// fn example(renderer: &mut pxui::Renderer, pass: &pxui::Pass) -> Result<(), pxui::Error> {
    ///     renderer.draw_with_split(pass, &mut |_| Err(pxui::Error::InvalidInput))
    /// }
    /// let _ = example;
    /// ```
    pub fn draw_with_split(&mut self, pass: &pxui_types::Pass, split: &mut dyn FnMut(Recorder) -> Result<pxui_types::Pass, Error>) -> Result<(), Error> {
        self.draw_split(pass, &[], split)
    }

    /// Records the prepared UI layers for the open pass's damage scissor.
    ///
    /// Set the pass scissor to the same nonempty `[x, y, width, height]` rectangle in target
    /// pixels before calling. PXUI may skip geometry outside it. See [`Renderer::draw_with_split`]
    /// for the backdrop contract. An empty or out-of-target rectangle returns InvalidInput.
    /// Allocates: when render storage first grows or the host callback allocates.
    /// Panics: if the host callback panics.
    ///
    /// ```
    /// fn example(renderer: &mut pxui::Renderer, pass: &pxui::Pass) -> Result<(), pxui::Error> {
    ///     renderer.draw_damage(pass, [0, 0, 32, 16], &mut |_| Err(pxui::Error::InvalidInput))
    /// }
    /// let _ = example;
    /// ```
    pub fn draw_damage(&mut self, pass: &pxui_types::Pass, scissor: [u32; 4], split: &mut dyn FnMut(Recorder) -> Result<pxui_types::Pass, Error>) -> Result<(), Error> {
        self.draw_split(pass, &[scissor], split)
    }

    /// Draws the prepared layers into the union of two to eight rectangles in target pixels.
    ///
    /// Each `[x, y, width, height]` must be nonempty and inside the target. The host clears these
    /// pixels before calling. PXUI scissors to each rectangle and clips overlapping portions so
    /// each damaged pixel is painted once. See [`Renderer::draw_with_split`] for the backdrop contract.
    /// Allocates: when render storage first grows or the host callback allocates.
    /// Panics: if the host callback panics.
    ///
    /// ```
    /// fn redraw(renderer: &mut pxui::Renderer, pass: &pxui::Pass) -> Result<(), pxui::Error> {
    ///     renderer.draw_damage_rects(pass, &[[0, 0, 10, 10], [10, 10, 10, 10]],
    ///                                &mut |_| Err(pxui::Error::InvalidInput))
    /// }
    /// let _ = redraw;
    /// ```
    pub fn draw_damage_rects(&mut self, pass: &pxui_types::Pass, rects: &[[u32; 4]], split: &mut dyn FnMut(Recorder) -> Result<pxui_types::Pass, Error>) -> Result<(), Error> {
        if !(2..=8).contains(&rects.len()) { return Err(Error::InvalidInput); }
        self.draw_split(pass, rects, split)
    }

    fn draw_split(&mut self, pass: &pxui_types::Pass, scissors: &[[u32; 4]], split: &mut dyn FnMut(Recorder) -> Result<pxui_types::Pass, Error>) -> Result<(), Error> {
        let mut ctx = PassSplitContext { f: split, error: None };
        let pass = pxui_types::Pass { split: Some(split_pass), split_user: (&mut ctx as *mut PassSplitContext).cast(), ..*pass };
        let rc = unsafe {
            match scissors {
                [] => sys::pxui_renderer_draw(self.raw.as_ptr(), &pass),
                &[[x,y,w,h]] => sys::pxui_renderer_draw_damage(self.raw.as_ptr(), &pass, x, y, w, h),
                rects => sys::pxui_renderer_draw_damage_rects(self.raw.as_ptr(), &pass, rects.as_ptr().cast(), rects.len() as u32),
            }
        };
        composite::resume_deferred();
        if rc == 0 { Ok(()) } else { Err(ctx.error.unwrap_or(Error::from_code(rc))) }
    }
}

/// A glyph atlas of the renderer, for an app pass that draws text quads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtlasKind {
    /// Primary grayscale glyph mask page.
    Mask,
    /// Primary colour glyph page.
    Color,
    /// Secondary grayscale glyph mask page.
    Mask2,
    /// Secondary colour glyph page.
    Color2,
}

impl Renderer {
    /// The renderer's atlas: R8 for `Mask`/`Mask2`, RGBA8 for `Color`/`Color2`.
    /// Second pages return `PXUI_ERR_STATE` until a prepare uploads their first glyphs.
    ///
    /// Errors: State for a second page before a prepare has uploaded its first glyphs.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// fn example(renderer: &mut pxui::Renderer, kind: pxui::AtlasKind) -> Result<pxui::Image, pxui::Error> { renderer.atlas(kind) }
    /// let _ = example;
    /// ```
    pub fn atlas(&self, kind: AtlasKind) -> Result<pxui_types::Image, Error> {
        let mut out = pxui_types::Image::default();
        let rc = unsafe { sys::pxui_renderer_atlas(self.raw.as_ptr(), kind as u32, &mut out) };
        if rc == 0 { Ok(out) } else { Err(Error::from_code(rc)) }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe { sys::pxui_renderer_destroy(self.raw.as_ptr()) }
    }
}

/// The UTF-8 text of an event (empty when it has none).
/// Allocates: never.
/// Panics: never.
///
/// ```
/// assert_eq!(pxui::event_text(&pxui::Event::new(pxui::Event::FOCUS_LOST)), "");
/// ```
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

/// The logical extent of the UI surface.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewSize {
    /// Width in logical points.
    pub width: f32,
    /// Height in logical points.
    pub height: f32,
}

/// Text and top-level ordering from the last published accessibility tree.
#[derive(Clone, Copy, Debug)]
pub struct A11yTree<'a> {
    /// Bytes referenced by published nodes' name, description and value ranges.
    pub strings: &'a [u8],
    /// Revision of the published accessibility data.
    pub revision: u64,
    /// Index of the first top-level node, or u32::MAX when the tree is empty.
    pub first_child: u32,
}

/// The UI for one surface.
#[repr(transparent)]
pub struct Ui(NonNull<sys::PxuiUi>);

/// Font bytes borrowed for Ui creation.
#[derive(Clone, Copy)]
pub struct FontData<'a>(
    /// The contents of a font file.
    pub &'a [u8],
);

/// The platform conventions, kit font size and font data for a new Ui.
#[derive(Clone, Copy)]
pub struct UiDesc<'a> {
    platform: Platform,
    font_size: Option<f32>,
    fonts: &'a [FontData<'a>],
}

impl<'a> UiDesc<'a> {
    /// The host platform with the library's default font and kit size.
    pub const DEFAULT: UiDesc<'a> = UiDesc::new(if cfg!(any(target_os = "macos", target_os = "ios")) {
        Platform::Apple
    } else {
        Platform::Other
    });

    /// Sets the platform's text editing and keyboard conventions.
    ///
    /// The default kit size and installed font are used until overridden.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{Platform, Ui, UiDesc};
    /// let ui = Ui::new(&UiDesc::new(Platform::Other)).unwrap();
    /// assert_eq!(ui.platform(), Platform::Other);
    /// ```
    pub const fn new(platform: Platform) -> UiDesc<'a> {
        UiDesc { platform, font_size: None, fonts: &[] }
    }

    /// Sets the kit's logical font size in points.
    ///
    /// Any positive finite size is kept exactly, including fractional points. Creation sends it in the size-prefixed fractional
    /// creation record; the 1.x `PxuiUiDesc::font_size` integer field keeps its meaning for C callers.
    /// Allocates: never.
    /// Panics: never. Creation reports a nonpositive or non-finite size as `Error::InvalidInput`.
    ///
    /// ```
    /// use pxui::{Ui, UiDesc};
    /// let ui = Ui::new(&UiDesc::DEFAULT.font_size(14.5)).unwrap();
    /// assert_eq!(ui.font_size(), 14.5);
    /// ```
    pub const fn font_size(mut self, points: f32) -> UiDesc<'a> {
        self.font_size = Some(points);
        self
    }

    /// Selects the font files in family order, with the kit face first.
    ///
    /// An empty slice uses the library's bundled font. Each supplied file is read during
    /// creation and its borrow can end when `Ui::new` returns.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{FontData, Ui, UiDesc};
    /// let fonts = [FontData(pxui::pragmasevka())];
    /// assert!(Ui::new(&UiDesc::DEFAULT.fonts(&fonts)).is_ok());
    /// ```
    pub const fn fonts(mut self, fonts: &'a [FontData<'a>]) -> UiDesc<'a> {
        self.fonts = fonts;
        self
    }
}

impl Ui {
    /// Creates a UI with the descriptor's platform conventions, kit size and fonts.
    ///
    /// `Error::Version` means the library's ABI major differs or its minor is older than the one this crate was built for (the
    /// minimum-library rule, docs/abi.md). `Error::InvalidInput` means the size
    /// or supplied fonts were refused. At most 64 fonts, 32 MiB per font and 256 MiB total
    /// are accepted. Built with `debug_assertions`, the new UI verifies its retained views (`verify_views(true)`: every clean view is
    /// built again each frame and a forgotten revision panics); turn it off on a `Ui` whose test counts rebuilds or frame cost.
    /// Allocates: the Ui and its owned font storage at creation.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{Error, FontData, Ui, UiDesc};
    /// assert!(matches!(Ui::new(&UiDesc::DEFAULT.fonts(&[FontData(b"invalid font")])), Err(Error::InvalidInput)));
    /// ```
    pub fn new(desc: &UiDesc) -> Result<Self, Error> {
        if !serves(abi_version(), pxui_types::ABI_BUILT) { return Err(Error::Version); }
        // a fractional size is created at its whole points, then set exactly (the 1.x creation record holds an integer)
        let (font_size, exact) = match desc.font_size {
            None => (0, None),
            Some(points) if (1.0..i32::MAX as f32).contains(&points) && points.fract() == 0.0 => (points as i32, None),
            Some(points) if points.is_finite() && (0.5..=1024.0).contains(&points) => (points.round().max(1.0) as i32, Some(points)),
            Some(_) => return Err(Error::InvalidInput),
        };
        let raw = sys::PxuiUiDesc {
            platform: match desc.platform { Platform::Apple => 1, Platform::Other => 2 },
            font_size,
            ..sys::PxuiUiDesc::default()
        };
        // fonts as the library takes them (empty: its default face); at most 64
        let mut fonts = [Str::default(); 64];
        let Some(fonts) = fonts.get_mut(..desc.fonts.len()) else { return Err(Error::InvalidInput) };
        for (out, FontData(bytes)) in fonts.iter_mut().zip(desc.fonts) {
            *out = Str { ptr: bytes.as_ptr(), len: bytes.len() };
        }
        let mut code = 0;
        let ui = unsafe { sys::pxui_ui_create(&raw, fonts.as_ptr(), fonts.len(), &mut code) };
        let ui = NonNull::new(ui).map(Self).ok_or_else(|| Error::from_code(code))?;
        if let Some(points) = exact { ui.set_font_points(points)?; }
        status(unsafe { sys::pxui_ui_add_surface(ui.0.as_ptr(), pxui_types::FrameIn::MAIN) })?;
        #[cfg(debug_assertions)]
        let mut ui = ui;
        #[cfg(debug_assertions)]
        ui.verify_views(true);
        Ok(ui)
    }

    /// The engine the library keeps behind this `Ui`, for the checks that compare the wrapper's drawing with the engine's own;
    /// static link only, valid while the `Ui` lives.
    #[cfg(feature = "static")]
    #[doc(hidden)]
    pub fn engine_ptr(&self) -> *const c_void { pxui_abi_link::engine_of(self.instance_id()).cast() }

    /// The library's handle of this `Ui`: stable while it lives, so an app can key its own per-`Ui` state by it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert_ne!(ui.instance_id(), 0);
    /// ```
    #[inline] pub fn instance_id(&self) -> usize { self.0.as_ptr() as usize }

    /// Routes the frame's input and builds its draw and platform output.
    ///
    /// The host supplies logical points and a monotonic time in seconds. A frame can carry
    /// event edges or an authoritative host snapshot. The returned draw output borrows the Ui.
    /// Malformed input returns `Error::InvalidInput`. Calling inside a frame callback returns
    /// `Error::State`. A panic caught inside the library returns `Error::Panic`.
    /// Allocates: when input or retained build storage grows.
    /// Panics: a panic in `build` or one of its callbacks resumes after the library returns.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("a", pxui::Label::DEFAULT); });
    /// ui.frame(&pxui::FrameIn::new(320.0, 240.0), &mut |ui| { ui.label("b", pxui::Label::DEFAULT); }).unwrap();
    /// assert_eq!(ui.frame_index(), 2);
    /// ```
    #[inline]
    pub fn frame(&mut self, input: &FrameIn<'_>, mut build: &mut dyn FnMut(&mut Ui)) -> Result<Frame<'_>, Error> {
        let mut out = FrameOut::default();
        // the wire record points at the borrowed events for this call only, whatever `input.wire.events` held
        let mut wire = input.wire;
        wire.events = pxui_types::ffi::Strided::of(input.events);
        // the records' sizes are the library's own, whatever the caller's `wire` holds
        wire.size = core::mem::size_of::<WireFrame>() as u32;
        wire.environment.size = core::mem::size_of::<Environment>() as u32;
        wire.keyboard.size = core::mem::size_of::<Keyboard>() as u32;
        #[cfg(feature = "static")]
        let rc = unsafe {
            let outer = self.0.as_ptr();
            let mut invoke = |engine: &mut pxui_abi_link::trusted::Engine| {
                let mut borrowed = pxui_abi_link::trusted::Borrowed::new(engine, outer);
                build(&mut core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new_unchecked(borrowed.enter()))));
            };
            pxui_abi_link::trusted::frame(outer, &wire, &mut invoke, &mut out)
        };
        #[cfg(not(feature = "static"))]
        let rc = unsafe {
            let call: pxui_types::ffi::UiBuild = Some(composite::call);
            let user = (&mut build as *mut &mut dyn FnMut(&mut Ui)).cast::<c_void>();
            sys::pxui_frame(self.0.as_ptr(), &wire, call, user, &mut out)
        };
        composite::resume_deferred();
        if rc==0 { Ok(Frame { out, _ui:PhantomData }) } else { Err(Error::from_code(rc)) }
    }

    /// Registers a surface on this `Ui` (`Ui::new` registered `FrameIn::MAIN`).
    ///
    /// A frame names its surface (`FrameIn::surface`) and every event carries it; an unregistered surface is `Error::InvalidInput`.
    /// Focus, the accessibility tree, IME and keyboard geometry belong to a surface. A `Ui` serves one surface for now: adding one while
    /// another is registered is `Error::State`, as is any call inside a frame; remove the first with `remove_surface`.
    /// shortcut: one surface per Ui, upgrade when the engine keeps per-surface focus/a11y/IME state.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.remove_surface(pxui_types::FrameIn::MAIN).unwrap();
    /// ui.add_surface(7).unwrap();
    /// ui.frame(&pxui::FrameIn::new(10.0, 10.0).surface(7), &mut |_| {}).unwrap();
    /// ```
    pub fn add_surface(&mut self, surface: u64) -> Result<(), Error> { status(unsafe { sys::pxui_ui_add_surface(self.0.as_ptr(), surface) }) }

    /// Removes the registered surface (an unregistered id is `Error::InvalidInput`; inside a frame `Error::State`).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.remove_surface(pxui_types::FrameIn::MAIN).unwrap();
    /// ```
    pub fn remove_surface(&mut self, surface: u64) -> Result<(), Error> { status(unsafe { sys::pxui_ui_remove_surface(self.0.as_ptr(), surface) }) }

    /// Borrows the most recent diagnostic for this UI.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// let _ = ui.last_error();
    /// ```
    pub fn last_error(&self) -> &str {
        let s = unsafe { sys::pxui_last_error(self.0.as_ptr()) };
        if s.len == 0 || s.ptr.is_null() { return ""; }
        core::str::from_utf8(unsafe { core::slice::from_raw_parts(s.ptr, s.len) }).unwrap_or("invalid error string")
    }

    /// The accessibility nodes of the last frame (parents before children), when `set_a11y(true)`.
    ///
    /// Allocates: never.
    /// Panics: if reading the frame output fails or its node stride differs from this build’s.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |ui| { ui.label("hi", pxui::Label::DEFAULT); });
    /// assert!(!ui.a11y_nodes().is_empty());
    /// ```
    pub fn a11y_nodes(&self) -> &[A11yNode] {
        // shortcut: the library writes the record it was built with (stride = size_of::<A11yNode>()), so the span is a slice; a wider stride from a newer library needs a stepping reader
        unsafe { self.output().a11y.nodes.as_slice() }.expect("a11y node stride is the record size")
    }

    /// Borrows the text and ordering of the last published accessibility tree.
    ///
    /// The string table is borrowed until the next mutable call. The revision changes when tree content changes.
    /// Allocates: never on a warm query.
    /// Panics: when reading output fails.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert_eq!(ui.a11y_tree().first_child, u32::MAX);
    /// ```
    pub fn a11y_tree(&self) -> A11yTree<'_> {
        let tree = self.output().a11y;
        let strings = if tree.strings.len == 0 { &[][..] } else { unsafe { core::slice::from_raw_parts(tree.strings.ptr, tree.strings.len) } };
        A11yTree { strings, revision: tree.revision, first_child: tree.first_child }
    }

    /// The accessible name of `node`, one of `a11y_nodes()`.
    /// Allocates: never.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |ui| { ui.label("Hello", pxui::Label::DEFAULT); });
    /// assert!(ui.a11y_nodes().iter().any(|node| ui.a11y_name_of(node) == "Hello"));
    /// ```
    pub fn a11y_name_of(&self, node: &A11yNode) -> &str {
        let strings = self.output().a11y.strings;
        if strings.len == 0 { return ""; }
        let text = unsafe { core::slice::from_raw_parts(strings.ptr, strings.len) };
        text.get(node.name_start as usize..node.name_end as usize).and_then(|name| core::str::from_utf8(name).ok()).unwrap_or("")
    }

    /// The text value of a current accessibility node (a number's words, ABI 1.126); None for no value or a number without words.
    /// Allocates: never.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |ui| { ui.label("Hello", pxui::Label::DEFAULT); });
    /// for node in ui.a11y_nodes() { let _ = ui.a11y_value_of(node); }
    /// ```
    pub fn a11y_value_of(&self,node:&A11yNode)->Option<&str> {
        if node.value.tag==0 || (node.value.tag==2 && node.value.text_end<=node.value.text_start) {return None;}
        let strings=self.output().a11y.strings;
        let text=if strings.len==0 {&[][..]} else {unsafe {core::slice::from_raw_parts(strings.ptr,strings.len)}};
        text.get(node.value.text_start as usize..node.value.text_end as usize).and_then(|value|core::str::from_utf8(value).ok())
    }

    /// How many layers the last frame has (a floating root's each, in draw order).
    /// Allocates: never.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert_eq!(ui.layer_count(), ui.layers().count());
    /// ```
    #[inline] pub fn layer_count(&self) -> usize { self.output().layers.len }

    /// Looks up a draw layer of the last frame by zero-based index.
    ///
    /// Returns `None` when that index has no layer. Its instances and dirty ranges are
    /// borrowed from the Ui. Later output reads and renderer acknowledgments preserve
    /// the bytes of a layer already returned.
    /// Allocates: when reading the output grows its retained layer or dirty-range storage.
    /// Panics: when reading the Ui's output fails.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert!(ui.layer(ui.layer_count()).is_none());
    /// ```
    pub fn layer(&self, index: usize) -> Option<Layer<'_>> {
        self.layers().get(index)
    }

    /// Visits the last frame's layers in draw order while borrowing the Ui.
    ///
    /// Allocates: when reading the output grows its retained layer or dirty-range storage.
    /// Panics: when reading the Ui's output fails.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert_eq!(ui.layers().count(), ui.layer_count());
    /// ```
    pub fn layers(&self) -> Layers<'_> {
        Layers { raw: self.output().layers, next: 0, _ui: PhantomData }
    }

    /// Marks layer `index` of the last frame drawn (its dirty ranges clear), as `Renderer::draw_layer` does: for a host that
    /// reads the layers itself.
    /// Allocates: never.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("Layer", pxui::Label::DEFAULT); });
    /// if ui.layer_count() != 0 { ui.drawn(0); }
    /// ```
    #[track_caller] pub fn drawn(&mut self, index: usize) { check(self, unsafe { sys::pxui_ui_layer_drawn(self.0.as_ptr(), index as u64) }) }

    /// Reads the last frame's platform and draw output records.
    ///
    /// The descriptor is copied, so a later query can report damage cleared by a
    /// renderer without changing a descriptor already returned. Its raw span pointers
    /// lend Ui storage. Prefer `layer` or `layers` for borrowed draw data.
    /// Allocates: when publishing a frame or patch grows retained output storage.
    /// Panics: when the library fails to provide the output.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// let output = ui.output();
    /// assert_eq!(output.draw_revision, ui.output().draw_revision);
    /// ```
    #[inline]
    #[track_caller]
    pub fn output(&self) -> Output {
        // null only when building the output panicked inside the library
        let output = unsafe { sys::pxui_ui_output(self.0.as_ptr()) };
        if output.is_null() { refused(self, pxui_types::PXUI_ERR_PANIC) }
        unsafe { output.read() }
    }
    /// Reads the latest damage in logical window points.
    ///
    /// The returned copy includes undrawn patches and steps. A draw acknowledgment
    /// clears the list, and a full redraw reports the viewport with `full` set.
    /// Allocates: never.
    /// Panics: when the library refuses the Ui.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert_eq!(ui.damage_list(), ui.output().damage_list);
    /// ```
    pub fn damage_list(&self) -> PxuiDamageList {
        let mut damage = PxuiDamageList::default();
        check(self, unsafe { sys::pxui_ui_damage_list(self.0.as_ptr(), &mut damage) });
        damage
    }

    /// Enables output promotion for a host that composites the reported views above its canvas.
    ///
    /// Call between frames and draw the reported runs separately on every frame that excludes them.
    /// Allocates: reserves both promotion buffers on first enable.
    /// Panics: when called inside a frame or refused by the library.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// ui.set_promotion(true);
    /// ```
    pub fn set_promotion(&mut self, enabled: bool) {
        check(self, unsafe { sys::pxui_ui_set_promotion(self.0.as_ptr(), enabled.into()) });
    }

    /// Borrows the runs the compositor can put above the latest canvas output.
    ///
    /// Call between frames. Run positions and clips use window points and key times use seconds.
    /// Allocates: never.
    /// Panics: when called inside a frame or refused by the library.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert!(ui.promoted().is_empty());
    /// ```
    pub fn promoted(&self) -> &[pxui_types::ffi::PxuiPromoted] {
        let (mut ptr, mut len) = (core::ptr::null(), 0);
        check(self, unsafe { sys::pxui_ui_promoted(self.0.as_ptr(), &mut ptr, &mut len) });
        if len == 0 { &[] } else { unsafe { core::slice::from_raw_parts(ptr, len) } }
    }

    /// Hints that a retained view's content can move to the host compositor.
    ///
    /// Only eligible content without any later overlapping draws is promoted. The hint stays with
    /// the live view. Returns `Error::InvalidInput` for a live handle that is not a retained view.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let view = ui.view_of("v", &0, |_| {});
    ///     ui.promote(&view).unwrap();
    /// });
    /// ```
    pub fn promote(&mut self, view: &Handle) -> Result<(), Error> {
        let rc = unsafe { sys::pxui_ui_promote(self.0.as_ptr(), view) };
        status(rc)
    }

    /// Reads the host appearance from the last frame, including inside cached views.
    ///
    /// `Appearance::Unknown` is the default before the host reports a value.
    /// Allocates: never.
    /// Panics: when the library refuses the Ui.
    ///
    /// ```
    /// use pxui::{Appearance, FrameIn, Ui, UiDesc};
    /// let mut ui = Ui::new(&UiDesc::DEFAULT).unwrap();
    /// ui.frame(&FrameIn::new(20.0, 20.0).appearance(Appearance::Dark), &mut |_| {}).unwrap();
    /// assert_eq!(ui.system_appearance(), Appearance::Dark);
    /// ```
    pub fn system_appearance(&self) -> Appearance {
        let mut value = 0;
        check(self, unsafe { sys::pxui_ui_system_appearance(self.0.as_ptr(), &mut value) });
        match value {
            WireFrame::APPEARANCE_LIGHT => Appearance::Light,
            WireFrame::APPEARANCE_DARK => Appearance::Dark,
            _ => Appearance::Unknown,
        }
    }

    /// The theme in effect's tokens, by value: what a canvas closure captures.
    /// Allocates: never.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert!(ui.tokens().font_size > 0.0);
    /// ```
    #[inline] pub fn tokens(&self) -> pxui_types::theme::Tokens { self.theme().tokens }
    /// The theme in effect, whole: the library's own fractional snapshot, lent (no copy). Text sizes and fonts keep fractional
    /// points. Anything that could change it takes `&mut self`, so the borrow cannot outlive it.
    /// Panics with the library's message when the first read of a handle is refused (a `Ui` an earlier panic poisoned): the library
    /// returns null. A later read is the handle's own record of it (one load, no call), which stays valid while the handle does.
    /// Allocates: never.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert_eq!(ui.theme().tokens.font_size, ui.tokens().font_size);
    /// ```
    #[inline]
    #[track_caller]
    pub fn theme(&self) -> &pxui_types::theme::Theme {
        let cached = unsafe { (*self.0.as_ptr()).theme };
        let theme = if cached.is_null() { self.theme_read() } else { cached };
        unsafe { &*theme }
    }

    /// The first theme read of a handle: the library lends the theme and records it in the handle.
    #[cold]
    #[inline(never)]
    #[track_caller]
    fn theme_read(&self) -> *const pxui_types::theme::Theme {
        // the 1.x `pxui_ui_theme` lends the legacy integer layout, which this type no longer matches
        let theme = unsafe { sys::pxui_ui_theme(self.0.as_ptr()) };
        if theme.is_null() { refused(self, pxui_types::PXUI_ERR_PANIC) }
        theme
    }

    /// Reports the revision of the last draw list.
    ///
    /// Allocates: never.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// let _ = ui.draw_revision();
    /// ```
    #[inline] pub fn draw_revision(&self) -> u64 { self.output().draw_revision }
    /// Reports the number of nodes built for the last frame.
    ///
    /// Allocates: never.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// let _ = ui.built_nodes();
    /// ```
    #[inline] pub fn built_nodes(&self) -> usize { self.output().built_nodes as usize }
    /// Reports the number of retained views rebuilt for the last frame.
    ///
    /// Allocates: never.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// let _ = ui.rebuilt_views();
    /// ```
    #[inline] pub fn rebuilt_views(&self) -> usize { self.output().rebuilt_views as usize }
    /// Reports the number of nodes arranged for the last frame.
    ///
    /// Allocates: never.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// let _ = ui.arranged_nodes();
    /// ```
    #[inline] pub fn arranged_nodes(&self) -> usize { self.output().arranged_nodes as usize }
    /// Returns the logical extent of this UI surface.
    ///
    /// Dimensions are in logical points.
    /// Allocates: never.
    /// Panics: when reading the surface size fails.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert!(ui.view_size().width > 0.0);
    /// ```
    #[track_caller]
    pub fn view_size(&self) -> ViewSize {
        let mut out = pxui_types::geom::Point::default();
        check(self, unsafe { sys::pxui_ui_view_size(self.0.as_ptr(), &mut out) });
        ViewSize { width: out.x, height: out.y }
    }
    /// Reports the next requested repaint time on the frame clock.
    ///
    /// Allocates: never.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// let _ = ui.repaint_at();
    /// ```
    #[inline]
    pub fn repaint_at(&self) -> Option<f64> {
        let at = self.output().repaint_at;
        at.is_finite().then_some(at)
    }

    /// Sets whether text and directional layout use right-to-left order.
    ///
    /// Allocates: never.
    /// Panics: when the UI is poisoned or the operation is refused in its current state.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.set_dir(true);
    /// ```
    #[track_caller] pub fn set_dir(&mut self,rtl:bool) {check(self, unsafe{sys::pxui_ui_set_dir(self.0.as_ptr(),rtl as u32)})}
    /// Enables or disables publishing an accessibility tree.
    ///
    /// Allocates: never at this call; building the next tree may grow its storage.
    /// Panics: when the UI is poisoned or the operation is refused in its current state.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.set_a11y(true);
    /// ```
    #[track_caller] pub fn set_a11y(&mut self,on:bool) {check(self, unsafe{sys::pxui_ui_set_a11y(self.0.as_ptr(),on as u8)})}
    /// Queues accessibility requests for this Ui's registered surface, applying them at the next frame.
    ///
    /// A batch must name this Ui's surface. At most 4096 actions may wait across calls; refusal queues none of the batch.
    /// Allocates: when the retained pending-action buffer grows; never once warm.
    /// Panics: for invalid requests, another surface, or a full queue before the next frame.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// ui.inject_a11y(&[]);
    /// ```
    #[track_caller] pub fn inject_a11y(&mut self,actions:&[A11yAction]) {check(self, unsafe{sys::pxui_ui_inject_a11y(self.0.as_ptr(),pxui_types::ffi::Strided::of(actions))})}
    /// Replaces a live box’s text between builds and reports whether it was applied.
    ///
    /// Allocates: when cached shaping or draw storage grows.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| { let b = ui.node("sample", pxui::Node::DEFAULT.w_px(20.0).h_px(20.0)); let _ = ui.patch_text(&b, "Updated"); });
    /// ```
    #[inline] #[track_caller] pub fn patch_text(&mut self,box_:&Handle,value:&str)->bool{let mut out=0;let rc=unsafe{sys::pxui_ui_patch_text(self.0.as_ptr(),box_,as_str(value),&mut out)};live(self,rc)&&out!=0}
    /// Advances the busy indicator of a live box to the given frame time in seconds.
    ///
    /// Allocates: if draw storage must grow.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| { let b = ui.node("sample", pxui::Node::DEFAULT.w_px(20.0).h_px(20.0)); let _ = ui.busy_step(&b, 0.0); });
    /// ```
    #[inline] #[track_caller] pub fn busy_step(&mut self,box_:&Handle,now:f64)->bool{let mut out=0;let rc=unsafe{sys::pxui_ui_busy_step(self.0.as_ptr(),box_,now,&mut out)};live(self,rc)&&out!=0}
    /// Advances running transitions (`animate`, `animate_keys`) to `time` without building, laying out or painting:
    /// animated nodes are patched in place and a view's transition moves its view entry, so the cost follows the
    /// animated nodes, not the window. Returns false, changing nothing, when the next frame needs a build: before the
    /// first frame, while input, a gesture, a frame request, a timer, a hover delay or settling layout is due by `time`,
    /// or when time runs backwards. A host with no events and nothing new to show calls it instead of `frame` when
    /// `repaint_at` is due. Read `damage_list()` for the changed regions and `draw_revision` for draw-list changes.
    ///
    /// Allocates: never once warm (the patch scratch grows on first use).
    /// Panics: when the UI is poisoned.
    ///
    /// ```
    /// let mut ui = pxui::doc::frames(1, &mut |_| {});
    /// assert!(ui.step(1.0) || ui.repaint_at().is_none());
    /// ```
    #[inline] #[track_caller] pub fn step(&mut self,time:f64)->bool{let mut out=0;let rc=unsafe{sys::pxui_ui_step(self.0.as_ptr(),time,&mut out)};live(self,rc)&&out!=0}
    /// Reads cumulative frame scheduling diagnostics from this UI.
    ///
    /// Counters start at zero and only advance if `PXUI_FRAME_REASONS=1` was set at creation.
    /// Allocates: never.
    /// Panics: when the UI is poisoned.
    ///
    /// ```
    /// let ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// assert_eq!(ui.frame_reasons().stepped, 0);
    /// ```
    pub fn frame_reasons(&self) -> PxuiFrameReasons {
        let mut out = PxuiFrameReasons::default();
        check(self, unsafe { sys::pxui_ui_frame_reasons(self.0.as_ptr(), &mut out) });
        out
    }
    /// Requests another frame from the host.
    ///
    /// Allocates: never.
    /// Panics: when the UI is poisoned or the operation is refused in its current state.
    ///
    /// ```
    /// let mut ui = pxui::Ui::new(&pxui::UiDesc::DEFAULT).unwrap();
    /// ui.request_frame();
    /// ```
    #[track_caller] pub fn request_frame(&mut self) {check(self, unsafe{sys::pxui_ui_request_frame(self.0.as_ptr())})}
    /// Returns the live box’s rectangle in logical points.
    ///
    /// Allocates: never.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| { let b = ui.node("sample", pxui::Node::DEFAULT.w_px(20.0).h_px(20.0)); let _ = ui.rect(&b); });
    /// ```
    #[inline]
    #[track_caller] pub fn rect(&self,box_:&Handle)-> geom::Rect {
        let mut out=geom::Rect::default();
        #[cfg(feature = "static")]
        check(self, unsafe{pxui_abi_link::trusted::rect(self.0.as_ptr(),box_,&mut out)});
        #[cfg(not(feature = "static"))]
        check(self, unsafe{sys::pxui_ui_rect(self.0.as_ptr(),box_,&mut out)});
        out
    }
    pub(crate) fn box_of_author_key(&self, key: Key) -> Option<Handle> {
        self.box_of_resolved_key(self.resolve_lookup_key(key))
    }

    /// The box key a lookup names: `Key::Resolved` unchanged (already hashed, never hashed again); an authored label or id hashed
    /// as `node` hashes it, under the active scope seed during a build and under `ROOT_KEY` outside one. No allocation.
    #[track_caller]
    pub(crate) fn resolve_lookup_key(&self, key: Key) -> i64 {
        match key {
            Key::Resolved(key) => key,
            key => self.resolve_under(self.lookup_seed(), key),
        }
    }

    /// `key` hashed as `node` hashes it under `seed` (`Key::Resolved` as it is).
    #[track_caller]
    pub(crate) fn resolve_under(&self, seed: i64, key: Key) -> i64 {
        if let Key::Label(label) = key { authored(label); }
        let mut buf = [0u8; 17];
        pxui_types::ui::key_of(seed, key.label(&mut buf))
    }

    /// The seed `resolve_lookup_key` hashes an authored key under.
    #[track_caller]
    pub(crate) fn lookup_seed(&self) -> i64 {
        let mut seed = 0;
        check(self, unsafe { sys::pxui_ui_key_seed(self.0.as_ptr(), &mut seed) });
        seed
    }

    pub(crate) fn box_of_resolved_key(&self,key:i64)->Option<Handle>{
        let mut out=Handle::default();
        #[cfg(feature = "static")]
        let found=unsafe{pxui_abi_link::trusted::box_of_key(self.0.as_ptr(),key,&mut out)};
        #[cfg(not(feature = "static"))]
        let found=unsafe{sys::pxui_ui_box_of_key(self.0.as_ptr(),key,&mut out)}!=0;
        found.then_some(out)
    }
    /// The live node for a published accessibility identity; `None` after it retires.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let ui = pxui::doc::frames(1, &mut |_| {});
    /// assert!(ui.box_of_a11y(u64::MAX).is_none());
    /// ```
    #[inline] pub fn box_of_a11y(&self,id:u64)->Option<Handle> {
        let mut out=Handle::default();
        #[cfg(feature = "static")]
        let found=unsafe {pxui_abi_link::pxui_ui_box_of_a11y(self.0.as_ptr(),id,&mut out)};
        #[cfg(not(feature = "static"))]
        let found=unsafe {sys::pxui_ui_box_of_a11y(self.0.as_ptr(),id,&mut out)};
        (found!=0).then_some(out)
    }
    /// Sets the text of a box being built.
    ///
    /// Allocates: when the text cache or frame storage grows.
    /// Panics: when the library refuses the call.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| { let b = ui.node("sample", pxui::Node::DEFAULT.w_px(20.0).h_px(20.0)); ui.text(&b, "Hello"); });
    /// ```
    #[inline] #[track_caller] pub fn text(&mut self,box_:&Handle,value:&str) {
        #[cfg(feature = "static")]
        let rc = unsafe { sys::pxui_ui_text(self.0.as_ptr(), box_, as_str(value)) };
        #[cfg(not(feature = "static"))]
        let rc = unsafe { sys::pxui_ui_text(self.0.as_ptr(), box_, as_str(value), core::ptr::null(), core::ptr::null(), 0) };
        check(self, rc)
    }

    /// Builds child nodes within a live parent box.
    ///
    /// Allocates: when the frame arena or retained storage first grows; never on a warm frame.
    /// Panics: for a stale parent, a refused build call, or a panic from the callback.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| { let b = ui.node("sample", pxui::Node::DEFAULT.w_px(20.0).h_px(20.0)); ui.within(&b, &mut |ui| { ui.label("Child", pxui::Label::DEFAULT); }); });
    /// ```
    #[inline]
    #[track_caller]
    pub fn within(&mut self, box_: &Handle, mut f: &mut dyn FnMut(&mut Ui)) {
        #[cfg(feature = "static")]
        let rc = unsafe {
            let outer = self.0;
            pxui_abi_link::trusted::within(outer.as_ptr(), box_, &mut |_| f(&mut core::mem::ManuallyDrop::new(Ui(outer))))
        };
        #[cfg(not(feature = "static"))]
        let rc = unsafe { sys::pxui_ui_within(self.0.as_ptr(), box_, Some(composite::call), &mut f as *mut _ as *mut c_void) };
        check(self, rc)
    }

    /// The one body of `id`: `f` under the scope of `key`'s label (a resolved label is the seed as it is), with no node.
    #[track_caller]
    pub(crate) fn id_scope(&mut self, key: Key, mut f: &mut dyn FnMut(&mut Ui)) {
        if let Key::Label(label) = key { authored(label); }
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        #[cfg(feature = "static")]
        let rc = unsafe { let outer = self.0.as_ptr(); pxui_abi_link::trusted::scope(outer, label, inner!(outer, |ui: &mut Ui| f(ui))) };
        #[cfg(not(feature = "static"))]
        let rc = unsafe { sys::pxui_ui_scope(self.0.as_ptr(), as_text(label), Some(composite::call), &mut f as *mut _ as *mut c_void) };
        check(self, rc)
    }


}


impl Drop for Ui {
    fn drop(&mut self) {
        // with a renderer of this `Ui` alive the library keeps it until the last renderer is destroyed
        unsafe { sys::pxui_ui_destroy(self.0.as_ptr()) };
    }
}

/// A `&[&str]` as a span of `Str` for one call: up to 32 strings live on the stack, more on the heap.
/// Pick-one items as the C items column, on the stack up to 32. Only the slots the call fills are written (and only they cross): zeroing
/// all 32 (2 KiB) cost a radio more than its checks (U1c.5).
pub(crate) struct PickItems {
    stack:[core::mem::MaybeUninit<pxui_types::ffi::PxuiPickItem>;32],
    heap:Vec<pxui_types::ffi::PxuiPickItem>,
    len:usize,
}

impl PickItems {
    /// An empty column: `fill` writes it where it lies (a filled 2 KiB column returned by value was copied, a memmove per pick).
    #[inline(always)]
    pub(crate) fn new()->Self { Self { stack:[const { core::mem::MaybeUninit::uninit() };32], heap:Vec::new(), len:0 } }
    #[inline(always)]
    pub(crate) fn fill(&mut self, items:&[Segment]) {
        let item=|s:&Segment| pxui_types::ffi::PxuiPickItem { has_id:s.id.is_some() as u8, id:s.id.unwrap_or(0), text:as_str(s.text), name:as_str(s.name.unwrap_or("")), detail:as_str(s.detail), ..Default::default() };
        self.len=items.len();
        if items.len()<=32 { for (out,s) in self.stack.iter_mut().zip(items) { out.write(item(s)); } }
        else { self.heap=items.iter().map(item).collect(); }
    }
    #[inline]
    pub(crate) fn span(&self)->pxui_types::ffi::Slice<pxui_types::ffi::PxuiPickItem> {
        // the first `len` stack slots are written when `len` is at most 32
        let ptr=if self.len<=32 { self.stack.as_ptr().cast() } else { self.heap.as_ptr() };
        pxui_types::ffi::Slice { ptr, len:self.len }
    }
}

pub(crate) struct Strs {
    stack:[core::mem::MaybeUninit<Str>;32],
    heap:Vec<Str>,
    len:usize,
}

impl Strs {
    #[inline]
    pub(crate) fn new(values:&[&str])->Self {
        let mut strs=Self { stack:[const { core::mem::MaybeUninit::uninit() };32], heap:Vec::new(), len:values.len() };
        if values.len()<=32 { for (out,v) in strs.stack.iter_mut().zip(values) { out.write(as_str(v)); } }
        else { strs.heap=values.iter().map(|v|as_str(v)).collect(); }
        strs
    }
    /// `len` strings, the `i`-th `value(i)`, as `new` keeps them.
    #[inline]
    pub(crate) fn each<'a>(len:usize, value:&dyn Fn(usize)->&'a str)->Self {
        let mut strs=Self { stack:[const { core::mem::MaybeUninit::uninit() };32], heap:Vec::new(), len };
        if len<=32 { for (i,out) in strs.stack[..len].iter_mut().enumerate() { out.write(as_str(value(i))); } }
        else { strs.heap=(0..len).map(|i|as_str(value(i))).collect(); }
        strs
    }
    #[inline]
    pub(crate) fn span(&self)->pxui_types::ffi::Slice<Str> {
        // the first `len` stack slots are written when `len` is at most 32
        let ptr=if self.len<=32 { self.stack.as_ptr().cast() } else { self.heap.as_ptr() };
        pxui_types::ffi::Slice { ptr, len:self.len }
    }
}

/// Panics when an authored label begins with a byte below 0x20: those labels are reserved for the id and resolved key forms and the
/// engine's own positional keys.
#[inline]
#[track_caller]
pub(crate) fn authored(label: &str) {
    assert!(label.as_bytes().first().is_none_or(|b| *b >= 0x20), "pxui: a key label beginning with a control byte is reserved");
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
/// Reads a counter from the library's allocator for the `mem_sys::set_peer` protocol.
///
/// The selectors 1 through 5 read allocations, total bytes, live bytes, this thread's
/// allocations and this thread's peak delta in bytes. Selector 6 resets this thread's
/// counters and returns zero. Selector 7 reports whether the allocator is counting.
/// Other selectors return zero. A dynamic counting library can be installed as the
/// allocator peer. A static link shares the application's allocator.
/// Allocates: never.
/// Panics: never.
///
/// ```
/// assert_eq!(pxui::library_counter(0), 0);
/// ```
pub fn library_counter(which: u32) -> u64 {
    #[cfg(feature = "static")]
    { unsafe { sys::pxui_counter(which) } }
    #[cfg(not(feature = "static"))]
    { sys::pxui_counter(which) }
}
/// Counter id for allocations made by the pxui library.
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
}

/// Whether the library has `feature`, as it was built: backends and the accessibility bridge depend on the platform. Everything not named in `Feature` is in every build.
///
/// ```
/// let _ = pxui::supports(pxui::Feature::ExtensionSet);
/// ```
///
/// Allocates: never.
/// Panics: never.
#[cfg(feature="static")]
pub fn supports(feature:Feature)->bool { unsafe { sys::pxui_supports(feature as u32) != 0 } }
/// Whether the library has `feature`, as it was built: backends and the accessibility bridge depend on the platform. Everything not named in `Feature` is in every build.
///
/// ```
/// let _ = pxui::supports(pxui::Feature::ExtensionSet);
/// ```
///
/// Allocates: never.
/// Panics: never.
#[cfg(not(feature="static"))]
pub fn supports(feature:Feature)->bool { sys::pxui_supports(feature as u32) != 0 }
#[cfg(feature="static")]
#[inline]
fn abi_version()->u32 { unsafe { sys::pxui_abi_version() } }
#[cfg(not(feature="static"))]
#[inline]
fn abi_version()->u32 { sys::pxui_abi_version() }

/// Whether a library of ABI `library` serves an app built for `built` (both `major << 16 | minor`): the same major and a minor no older
/// than the app's (the minimum-library rule, docs/abi.md).
fn serves(library: u32, built: u32) -> bool { library >> 16 == built >> 16 && library & 0xFFFF >= built & 0xFFFF }

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

#[cfg(test)]
mod snapshot_tests {
    use super::*;

    /// An older library (an earlier minor) or another major is refused before any record is exchanged; an equal or newer library serves.
    #[test]
    fn ui_new_refuses_a_library_older_than_the_app() {
        let built = pxui_types::ABI_BUILT;
        assert!(serves(built, built) && serves(built + 1, built) && serves(built | 0xFFFF, built), "the same or a newer minor");
        assert!(!serves(built, built + 1) && !serves(built - 1, built), "an older library: the app is built for a newer minor");
        assert!(!serves(built + (1 << 16), built) && !serves(built - (1 << 16), built), "another major");
        assert!(Ui::new(&UiDesc::DEFAULT).is_ok(), "this library serves this crate");
    }
}
