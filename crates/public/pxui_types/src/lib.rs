#![allow(clippy::all)]
//! Plain, stable value types shared by PXUI and applications.

#[cfg(test)]
#[macro_use]
mod test_layout_rows;
pub mod a11y;
pub mod draw;
pub mod ffi;
pub mod geom;
pub mod layout;
pub mod node;
pub mod shape;
pub use shape::Span;
pub mod text;
pub mod theme;
pub mod ui;

pub use node::{Axis, Bar, Dir, Mark, Node};
pub use text::{Label, TextGlyph, TextMetrics, TextSpan, TextSpans, TextStyle};
pub use theme::{Curve, Font, Look, Motion, Ring, Shadow, State, States, Tokens};

/// Node behavior bits as a typed value over the raw integer (`bits`), so unknown future bits stay representable.
///
/// Combine with `|` or the `const fn or`; test with `any`. `Flags::FOCUSABLE` is the node flag; the accessibility state of the same name is
/// `a11y::state::FOCUSABLE`.
///
/// ```
/// use pxui_types::{Flags, Node};
/// const N: Node = Node::row().flags(Flags::CLICKABLE.or(Flags::TAB_STOP));
/// assert_eq!(N.flags, Flags::CLICKABLE | Flags::TAB_STOP);
/// assert_eq!(Flags::CLICKABLE | Flags::CLIP, Flags(9));
/// ```
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Flags(pub i64);

impl Flags {
    /// No flags.
    pub const NONE: Flags = Flags(0);
    /// Takes pointer presses and clicks.
    pub const CLICKABLE: Flags = Flags(1);
    /// Takes the keyboard focus.
    pub const FOCUSABLE: Flags = Flags(2);
    /// Scrolls its content vertically.
    pub const SCROLL: Flags = Flags(4);
    /// Clips its children to its bounds.
    pub const CLIP: Flags = Flags(8);
    /// Blocks input to what lies under it.
    pub const BLOCKING: Flags = Flags(16);
    /// Kit controls enter keyboard mode through Tab; pointer editing retains the existing host
    /// shortcut behavior. Text-entry rows also focus on a press.
    pub const TAB_ONLY: Flags = Flags(32);
    /// Keyboard traversal focus for a control whose pointer interaction leaves keyboard input with its
    /// host. Escape relinquishes this keyboard focus.
    pub const TAB_STOP: Flags = Flags(Self::FOCUSABLE.0 | Self::TAB_ONLY.0);
    /// a multiline editor keeps Tab for itself (two spaces) instead of moving the focus
    pub const KEEP_TAB: Flags = Flags(64);
    /// a press on such a box leaves the keyboard focus where it is (a completion row under an editor)
    pub const KEEP_FOCUS: Flags = Flags(128);
    /// a scrolling box whose thumb is drawn over its content (an inspector body): no gutter
    pub const OVER_THUMB: Flags = Flags(4096);
    /// A press whose topmost hit box is not clickable is captured by the nearest ancestor with this
    /// flag.
    pub const CAPTURE_UNCLAIMED: Flags = Flags(8192);
    /// the control marks its own keyboard focus: the engine strokes no ring round it
    pub const FOCUS_MARK: Flags = Flags(256);
    /// `TAB_STOP` with `FOCUS_MARK`.
    pub const TAB_STOP_MARKED: Flags = Flags(Self::TAB_STOP.0 | Self::FOCUS_MARK.0);
    /// The node's text is cut with an ellipsis to the room its look leaves (the placed width less the
    /// text inset) when it does not fit.
    pub const FIT_TEXT: Flags = Flags(16384);
    /// Fit text width, including text insets, rounded up to the frame's physical-pixel density.
    pub const FIT_TEXT_PIXEL: Flags = Flags(2097152);
    /// scrolls sideways (the wheel's and the trackpad's horizontal motion); with `SCROLL` on both axes
    pub const SCROLL_X: Flags = Flags(32768);
    /// a flow child of a scroll box that stops at its top edge instead of scrolling out (CSS sticky);
    /// it paints over its siblings
    pub const STICKY: Flags = Flags(65536);
    /// with `CLIP`: the children's clip takes the corner radius of the node's look (CSS `overflow:
    /// hidden` with `border-radius`); without it a clip is square
    pub const ROUND_CLIP: Flags = Flags(131072);
    /// Nothing the node's view builds follows its hover: the pointer moving onto or off it dirties no
    /// view (the app reads `hovered_within` between frames and patches what shows it).
    pub const HOVER_FREE: Flags = Flags(262144);
    /// The node's square look paints its fill and border instances even when they are clear, so a
    /// patch to a look that shows them rewrites its range in place (it never grows the layer).
    pub const KEEP_ROOM: Flags = Flags(524288);
    /// Nothing the node's view builds follows a drag of it: while the pointer holds it and only moves
    /// (no press, release or key), its view is not dirtied. The owner reads the drag between builds
    /// (`signal` on the kept handle; `field_drag` for a `value_field`) and patches what shows it.
    pub const DRAG_FREE: Flags = Flags(1048576);
    /// The union of two flag sets.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Flags;
    /// assert_eq!(Flags::CLICKABLE.or(Flags::CLIP).bits(), 9);
    /// ```
    #[inline]
    pub const fn or(self, other: Flags) -> Flags {
        Flags(self.0 | other.0)
    }
    /// Whether any bit of `other` is set in `self`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Flags;
    /// assert!(Flags::TAB_STOP.any(Flags::FOCUSABLE | Flags::CLIP));
    /// assert!(!Flags::CLIP.any(Flags::SCROLL));
    /// ```
    #[inline]
    pub const fn any(self, other: Flags) -> bool {
        self.0 & other.0 != 0
    }
    /// The raw bits.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Flags;
    /// assert_eq!(Flags::FOCUSABLE.bits(), 2);
    /// ```
    #[inline]
    pub const fn bits(self) -> i64 {
        self.0
    }
}

impl core::ops::BitOr for Flags {
    type Output = Flags;
    #[inline]
    fn bitor(self, other: Flags) -> Flags {
        self.or(other)
    }
}

impl core::ops::BitOrAssign for Flags {
    #[inline]
    fn bitor_assign(&mut self, other: Flags) {
        self.0 |= other.0;
    }
}

/// Box sizing accepted by the UI.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub enum Size {
    Px(f32),
    Pct(f32),
    Rel(extern "C" fn(f32) -> f32),
    Grow,
    Fit,
}

impl Size {
    pub fn is_grow(&self) -> bool {
        matches!(self, Self::Grow)
    }
}

/// Painter view transform.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub offset: [f32; 2],
    pub opacity: f32,
    pub clip: [f32; 4],
    pub scale: f32,
    pub transform: [f32; 4],
    pub pivot: [f32; 2],
}

impl View {
    pub const IDENTITY: Self = Self {
        offset: [0.0; 2],
        opacity: 1.0,
        clip: [-32768.0, -32768.0, 32767.0, 32767.0],
        scale: 1.0,
        transform: [1.0, 0.0, 0.0, 1.0],
        pivot: [0.0; 2],
    };

    pub fn rotation(radians: f32, pivot: [f32; 2]) -> Self {
        let (s, c) = radians.sin_cos();
        Self {
            transform: [c, -s, s, c],
            pivot,
            ..Self::IDENTITY
        }
    }
}

pub const MAX_LAYERS: usize = 64;

/// Status codes of every `pxui_*` call that returns `i32`: 0 is success, a negative value one of these.
pub const PXUI_ERR_ARGUMENT: i32 = -1;
/// A GPU adoption or recording failure (`pxui_renderer_*`).
pub const PXUI_ERR_GPU: i32 = -2;
/// The call is not allowed now: inside a frame callback, on a borrowed handle, or with a renderer attached.
pub const PXUI_ERR_STATE: i32 = -3;
/// A panic was caught inside the library; the `Ui` is poisoned until its next frame call.
pub const PXUI_ERR_PANIC: i32 = -4;
/// The box handle names a node that has gone. The engine tolerates this for queries and patches (a stale handle patches nothing,
/// has no signal, an empty rectangle), so the wrapper turns it into that result; for every other call it is a caller bug.
pub const PXUI_ERR_STALE: i32 = -5;

/// Key codes of `Event::KEY_PRESSED` and `KEY_RELEASED`: a Unicode scalar value below `KEY_SPECIAL`
/// is that character; `KEY_SPECIAL + n` is the special key `n` (arrows, enter, F-keys...) in the
/// order documented in `pxui.h`; `KEY_UNKNOWN | code` carries a platform code the library has no name for.
pub const KEY_SPECIAL: u32 = 0x11_0000;
pub const KEY_UNKNOWN: u32 = 0x8000_0000;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Str {
    pub ptr: *const u8,
    pub len: usize,
}

/// Single-line native keyboard traits and the per-call body variant of a text field, as a `Copy` value built with `const fn` builders from
/// `TextInputOptions::DEFAULT` or `SEARCH_FIELD`. Capitalization: none, sentences, words, letters (0..=3).
/// Action: Return (0), Done (1, dismisses focus), Search (2, leaves submission to the app).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextInputOptions {
    pub(crate) autocorrect: u32,
    pub(crate) capitalization: u32,
    pub(crate) action: u32,
    /// What a registered text field body reads from its config (`PxuiTextFieldConfig::variant`); 0 for none.
    pub(crate) variant: u16,
}
impl Default for TextInputOptions {
    fn default() -> Self { Self::DEFAULT }
}
/// The C record of the text input traits (`pxui_text_field_with_options`, `pxui_ui_text_input_options`): `TextInputOptions` without its variant,
/// which goes through `pxui_ui_variant` like every widget's. For C callers and the library's own conversions, not for authoring.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PxuiTextInputOptions {
    /// 0 or 1: the platform keyboard corrects words while typing.
    pub autocorrect: u32,
    /// 0 none, 1 sentences, 2 words, 3 letters.
    pub capitalization: u32,
    /// The return key: 0 Return, 1 Done (Enter ends editing), 2 Search (submission is the app's).
    pub action: u32,
}
impl From<TextInputOptions> for PxuiTextInputOptions {
    fn from(o: TextInputOptions) -> Self { Self {autocorrect:o.autocorrect,capitalization:o.capitalization,action:o.action} }
}
impl From<PxuiTextInputOptions> for TextInputOptions {
    fn from(o: PxuiTextInputOptions) -> Self { Self {autocorrect:o.autocorrect,capitalization:o.capitalization,action:o.action,variant:0} }
}
impl TextInputOptions {
    /// The return key keeps its ordinary action (`action`): the kit's default.
    pub const RETURN: u32 = 0;
    /// The return key ends editing: Enter dismisses focus and the keyboard (`action`).
    pub const DONE: u32 = 1;
    /// The return key reads Search and leaves submission to the app (`action`).
    pub const SEARCH: u32 = 2;
    /// The kit's text field: autocorrect on, sentence capitalization, Return.
    pub const DEFAULT: Self = Self {autocorrect:1,capitalization:1,action:Self::RETURN,variant:0};
    /// A search box: no autocorrect, no capitalization, the Search key.
    pub const SEARCH_FIELD: Self = Self {autocorrect:0,capitalization:0,action:Self::SEARCH,variant:0};
    /// Whether every trait is in range: autocorrect 0 or 1, capitalization 0..=3, action `RETURN`, `DONE` or `SEARCH`; the library refuses others.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::TextInputOptions;
    /// assert!(TextInputOptions::SEARCH_FIELD.valid() && !TextInputOptions::DEFAULT.action(7).valid());
    /// ```
    pub fn valid(&self) -> bool {self.autocorrect<=1 && self.capitalization<=3 && self.action<=2}
    /// These options with autocorrect on (1) or off (0).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::TextInputOptions;
    /// assert_eq!(TextInputOptions::DEFAULT.autocorrect(0).autocorrect_mode(), 0);
    /// ```
    pub const fn autocorrect(mut self, autocorrect: u32) -> Self { self.autocorrect = autocorrect; self }
    /// These options with capitalization none, sentences, words or letters (0..=3).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::TextInputOptions;
    /// assert_eq!(TextInputOptions::DEFAULT.capitalization(2).capitalization_mode(), 2);
    /// ```
    pub const fn capitalization(mut self, capitalization: u32) -> Self { self.capitalization = capitalization; self }
    /// These options with the return key's action: `RETURN`, `DONE` or `SEARCH`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::TextInputOptions;
    /// assert_eq!(TextInputOptions::SEARCH_FIELD.action(TextInputOptions::DONE).return_action(), TextInputOptions::DONE);
    /// ```
    pub const fn action(mut self, action: u32) -> Self { self.action = action; self }
    /// These options with the variant a registered text field body reads from its config (`variant`, 0 by default): one body builds
    /// different structures by it, so the app picks per call without comparing text. The built-in body ignores it.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::TextInputOptions;
    /// const FIND: TextInputOptions = TextInputOptions::SEARCH_FIELD.action(TextInputOptions::DONE).variant(1);
    /// assert_eq!(FIND.body_variant(), 1);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Self { self.variant = variant; self }
    /// Whether the platform keyboard corrects words: 1 on, 0 off.
    pub const fn autocorrect_mode(&self) -> u32 { self.autocorrect }
    /// The platform keyboard's capitalization: 0 none, 1 sentences, 2 words, 3 letters.
    pub const fn capitalization_mode(&self) -> u32 { self.capitalization }
    /// The variant a registered text field body reads from its config; 0 for none.
    pub const fn body_variant(&self) -> u16 { self.variant }
    /// The return key's action: `RETURN`, `DONE` or `SEARCH`.
    pub const fn return_action(&self) -> u32 { self.action }
}

/// ABI 1.25 TOUCH payload, borrowed through Event::text (len = size_of::<Touch>()).
/// Phase: 0 began, 1 moved, 2 ended, 3 cancelled. Position: logical points; time: source monotonic seconds.
/// Pressure is finite in 0..=1. Positions may lie outside the window; identities retain all 64 bits.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Touch {
    pub device:u64,
    pub finger:u64,
    pub x:f64,
    pub y:f64,
    pub time:f64,
    pub pressure:f32,
    pub phase:u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Event {
    pub kind: u32,
    pub code: u32,
    pub x: f64,
    pub y: f64,
    pub value: f64,
    pub text: Str,
}

impl Event {
    pub const KEY_PRESSED: u32 = 1;
    pub const KEY_RELEASED: u32 = 2;
    pub const POINTER_MOVED: u32 = 3;
    pub const POINTER_PRESSED: u32 = 4;
    pub const POINTER_RELEASED: u32 = 5;
    pub const POINTER_CANCELLED: u32 = 6;
    pub const SCROLLED: u32 = 7;
    pub const TEXT_INPUT: u32 = 8;
    pub const PASTE: u32 = 9;
    pub const TEXT_EDITING: u32 = 10;
    pub const FILE_DROPPED: u32 = 11;
    pub const FILE_DRAG_MOVED: u32 = 12;
    pub const FILE_DRAG_ENDED: u32 = 13;
    pub const PINCHED: u32 = 14;
    pub const FILE_DIALOG: u32 = 15;
    pub const DIALOG_OPENED: u32 = 16;
    pub const RESIZED: u32 = 17;
    pub const FOCUS_LOST: u32 = 18;
    pub const FOCUS_GAINED: u32 = 19;
    pub const CLOSED: u32 = 20;
    pub const PIXEL_SIZE_CHANGED: u32 = 21;
    pub const VISIBILITY_CHANGED: u32 = 22;
    pub const TRACKPAD_SCROLLED: u32 = 23;
    pub const ENVIRONMENT_CHANGED: u32 = 24;
    pub const TOUCH: u32 = 25;
    pub const FILES_DROPPED: u32 = 26;
}

/// Host geometry in logical window points. Rectangles are [x, y, width, height].
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Environment {
    /// Insets in top, right, bottom, left order; backgrounds may draw behind them.
    pub safe_area: [f32; 4],
    /// Keyboard's animation target and starting frame, including offscreen hide targets.
    pub keyboard_frame: [f32; 4],
    pub keyboard_from: [f32; 4],
    /// Start time uses FrameIn::time's clock. Duration and curve come from the host.
    pub keyboard_started: f64,
    pub keyboard_duration: f64,
    /// UIKit's raw animation curve, including platform values beyond 0..3.
    pub keyboard_curve: u32,
    /// 0 or 1; set after native did-show, cleared after native did-hide.
    pub keyboard_visible: u32,
}

/// Native presentation geometry, in logical window points; does not change Environment's frozen layout.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Keyboard {
    pub frame: [f32; 4],
    pub flags: u32,
}
impl Keyboard {
    pub const VALID: u32 = 1;
    pub const ANIMATING: u32 = 2;
    /// Whether every trait is in range: autocorrect 0 or 1, capitalization 0..=3, action `RETURN`, `DONE` or `SEARCH`; the library refuses others.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::TextInputOptions;
    /// assert!(TextInputOptions::SEARCH_FIELD.valid() && !TextInputOptions::DEFAULT.action(7).valid());
    /// ```
    pub fn valid(&self) -> bool {
        self.flags <= 3 && self.flags != Self::ANIMATING
            && self.frame.iter().all(|v| v.is_finite()) && self.frame[2] >= 0.0 && self.frame[3] >= 0.0
            && (self.frame[0]+self.frame[2]).is_finite() && (self.frame[1]+self.frame[3]).is_finite()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FrameIn {
    pub size: u32,
    pub events: *const Event,
    pub event_count: usize,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub time: f64,
    /// Host appearance: UNKNOWN, LIGHT or DARK. Added after the frozen 48-byte v1 prefix.
    pub appearance: u32,
    /// 0 or 1; the OS preference overrides themes that permit motion.
    pub reduced_motion: u32,
    /// ABI 1.24 tail; absent tails use the default environment.
    pub environment: Environment,
    /// ABI 1.26 tail: current native keyboard geometry, including intermediate animation frames.
    pub keyboard: Keyboard,
}
impl Default for FrameIn {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, events: core::ptr::null(), event_count: 0, width: 0.0, height: 0.0, scale: 1.0, time: 0.0,appearance:0,reduced_motion:0,environment:Environment::default(),keyboard:Keyboard::default() } }
}
impl FrameIn {
    pub const APPEARANCE_UNKNOWN:u32=0;
    pub const APPEARANCE_LIGHT:u32=1;
    pub const APPEARANCE_DARK:u32=2;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameOut {
    pub size: u32,
    pub damage: [f32; 4],
    pub has_damage: u8,
    pub reserved: [u8; 3],
    pub layer_count: u32,
    pub layers: *const core::ffi::c_void,
}

/// The app's GPU, adopted by `pxui_renderer_create`. PXUI records into the app's command buffers and
/// passes; it never submits, waits, presents or destroys what the app created. `owner_token` and
/// `release_owner` are one lease on the app's device (OGPU: `ogpu_core::backend::native_leased`); the
/// renderer takes it, also when creation fails, and releases it when it is destroyed. Vulkan: `features`
/// are the `ogpu_vulkan::VK_*` bits the app enabled; creation fails unless `REQUIRED_FOR_UI` are set and
/// the entry points behind them are loaded.
#[repr(C)]
#[derive(Debug)]
pub struct Gpu {
    pub size: u32,
    pub backend: u32,
    pub instance: *mut core::ffi::c_void,
    pub physical_device: *mut core::ffi::c_void,
    pub device: *mut core::ffi::c_void,
    pub queue: *mut core::ffi::c_void,
    pub queue_family: u32,
    pub api_version: u32,
    pub features: u64,
    pub get_instance_proc_addr: *mut core::ffi::c_void,
    pub pipeline_cache: *mut core::ffi::c_void,
    pub mtl_device: *mut core::ffi::c_void,
    pub wl_display: *mut core::ffi::c_void,
    pub wl_surface: *mut core::ffi::c_void,
    pub metal_layer: *mut core::ffi::c_void,
    pub hwnd: *mut core::ffi::c_void,
    pub owner_token: *mut core::ffi::c_void,
    pub release_owner: Option<extern "C" fn(*mut core::ffi::c_void)>,
}

impl Default for Gpu {
    fn default() -> Self {
        let none = core::ptr::null_mut();
        Self { size: core::mem::size_of::<Self>() as u32, backend: 0, instance: none, physical_device: none, device: none, queue: none, queue_family: 0, api_version: 0,
            features: 0, get_instance_proc_addr: none, pipeline_cache: none, mtl_device: none, wl_display: none, wl_surface: none, metal_layer: none, hwnd: none,
            owner_token: none, release_owner: None }
    }
}

/// The app's command buffer of one frame, outside any pass, for `pxui_renderer_prepare(commands, frame,
/// completed)`. `frame` is the app's serial for this command buffer and grows by at least one per prepare;
/// `completed` is the last serial whose command buffer the app knows has finished, never going back, and
/// at least `frame - 3` (the renderer reuses what frame `frame - 3` used). Otherwise prepare returns
/// `PXUI_ERR_ARGUMENT` and records nothing. Vulkan: prepare records glyph uploads (and image layout
/// transitions of textures it made since the last frame) into this buffer, so the app submits it before or
/// with the frame's pass. Before `pxui_renderer_destroy` the app must have finished every frame it prepared.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Commands {
    pub size: u32,
    pub vk_command_buffer: *mut core::ffi::c_void,
    pub mtl_command_buffer: *mut core::ffi::c_void,
}
impl Default for Commands {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, vk_command_buffer: core::ptr::null_mut(), mtl_command_buffer: core::ptr::null_mut() } }
}

/// What a `Pass::split` calls with the target the app's pass drew into, once the app has ended that pass.
pub type PassRecord = extern "C" fn(*mut core::ffi::c_void, *const Image) -> i32;
/// A backdrop blur needs what the frame holds so far, so the library asks the app to end its open pass: `split(user, record,
/// record_user, out)` ends the pass, calls `record(record_user, &target)` (the colour target, sampleable, holding everything
/// drawn so far; the library records its blur passes into the prepared command buffer), reopens a pass over the same target
/// (load, not clear; the same viewport and scissor), writes it to `out` and returns 0. The library draws the rest of the layer
/// into that pass; the app's last pass is the one it holds afterwards.
pub type PassSplit = extern "C" fn(*mut core::ffi::c_void, PassRecord, *mut core::ffi::c_void, *mut Pass) -> i32;

/// The app's open pass (Vulkan: inside `vkCmdBeginRendering` on the prepared frame's command buffer)
/// for `pxui_renderer_draw`. It draws layers `first_layer..first_layer + layer_count` of the last frame (`layer_count` 0: every
/// layer from `first_layer`), so an app can draw its own content between them. `split` is needed only by a layer with a backdrop blur. `scale` is the app's pixels per point. Bound state is not preserved: after
/// draw the pass holds PXUI's pipeline, descriptors, viewport and scissor, so either the UI is the last
/// thing in the pass or the app rebinds everything (OGPU: `ogpu_core::backend::invalidate_state`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Pass {
    pub size: u32,
    pub vk_command_buffer: *mut core::ffi::c_void,
    pub mtl_render_encoder: *mut core::ffi::c_void,
    pub width_px: u32,
    pub height_px: u32,
    pub scale: f32,
    pub mtl_command_buffer: *mut core::ffi::c_void,
    pub first_layer: u32,
    pub layer_count: u32,
    pub split: Option<PassSplit>,
    pub split_user: *mut core::ffi::c_void,
}
impl Default for Pass {
    fn default() -> Self {
        Self { size: core::mem::size_of::<Self>() as u32, vk_command_buffer: core::ptr::null_mut(), mtl_render_encoder: core::ptr::null_mut(), width_px: 0, height_px: 0, scale: 1.0, mtl_command_buffer: core::ptr::null_mut(),
            first_layer: 0, layer_count: 0, split: None, split_user: core::ptr::null_mut() }
    }
}

/// An app texture PXUI samples at a `Break`, without a copy. The app keeps it alive until the frames
/// drawing it complete. Vulkan: `vk_image_view` is a 2D color view whose image is in
/// `VK_IMAGE_LAYOUT_GENERAL` whenever those frames run (every OGPU image is). PXUI only samples it:
/// it never reads, writes, copies or destroys the image.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Image {
    pub size: u32,
    pub vk_image_view: *mut core::ffi::c_void,
    pub mtl_texture: *mut core::ffi::c_void,
    pub width: u32,
    pub height: u32,
    pub format: u32,
}
impl Default for Image { fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, vk_image_view: core::ptr::null_mut(), mtl_texture: core::ptr::null_mut(), width: 0, height: 0, format: 0 } } }

impl Image {
    pub const FORMAT_R8: u32 = 0;
    pub const FORMAT_RGBA8: u32 = 1;
    pub const FORMAT_BGRA8: u32 = 2;
    pub const FORMAT_BGRA8_SRGB: u32 = 3;
    pub const FORMAT_RGBA16F: u32 = 4;
    pub const FORMAT_RGBA32F: u32 = 5;
    pub const FORMAT_DEPTH32F: u32 = 6;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiBox {
    pub index: u64,
    pub box_key: i64,
    pub box_slot: u64,
    pub generation: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiPanel {
    pub size: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub padding: i64,
    pub has_height: u8,
    pub window: u8,
    pub has_max_height: u8,
    pub has_stroke: u8,
    pub has_look: u8,
    pub reserved: [u8; 3],
    pub max_height: f32,
    pub stroke: geom::Rgba,
    pub look: ffi::PxuiLookDesc,
}
impl Default for PxuiPanel {
    fn default() -> Self {
        Self { size: core::mem::size_of::<Self>() as u32, x: 0.0, y: 0.0, width: 0.0, height: 0.0, padding: 0, has_height: 0, window: 0,
            has_max_height: 0, has_stroke: 0, has_look: 0, reserved: [0; 3], max_height: 0.0, stroke: geom::Rgba::default(), look: ffi::PxuiLookDesc::default() }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiTargetDesc {
    pub size: u32,
    pub format: u32,
    pub samples: u8,
    pub reserved: [u8; 3],
    pub mask_width: u32,
    pub mask_height: u32,
    pub color_width: u32,
    pub color_height: u32,
    pub reserved_tail: u32,
}
impl Default for PxuiTargetDesc { fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, format: 0, samples: 1, reserved: [0; 3], mask_width: 0, mask_height: 0, color_width: 0, color_height: 0, reserved_tail: 0 } } }

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiUiDesc {
    pub size: u32,
    pub platform: u32,
    pub font_size: i32,
}
impl Default for PxuiUiDesc { fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, platform: 0, font_size: 0 } } }

#[cfg(test)]
mod abi_layout {
    use super::*;
    use core::mem::{align_of, size_of};
    use crate::test_layout_rows::row as assert_layout;

    pub(crate) fn abi_descriptors_rows() {
        assert_layout("PxuiBox", size_of::<PxuiBox>(), align_of::<PxuiBox>(), &[
            ("index", core::mem::offset_of!(PxuiBox, index)), ("box_key", core::mem::offset_of!(PxuiBox, box_key)), ("box_slot", core::mem::offset_of!(PxuiBox, box_slot)), ("generation", core::mem::offset_of!(PxuiBox, generation)),
        ]);
        assert_layout("PxuiPanel", size_of::<PxuiPanel>(), align_of::<PxuiPanel>(), &[
            ("size", core::mem::offset_of!(PxuiPanel, size)), ("x", core::mem::offset_of!(PxuiPanel, x)), ("y", core::mem::offset_of!(PxuiPanel, y)), ("width", core::mem::offset_of!(PxuiPanel, width)),
            ("height", core::mem::offset_of!(PxuiPanel, height)), ("padding", core::mem::offset_of!(PxuiPanel, padding)), ("has_height", core::mem::offset_of!(PxuiPanel, has_height)), ("window", core::mem::offset_of!(PxuiPanel, window)),
            ("has_max_height", core::mem::offset_of!(PxuiPanel, has_max_height)), ("has_stroke", core::mem::offset_of!(PxuiPanel, has_stroke)), ("has_look", core::mem::offset_of!(PxuiPanel, has_look)), ("reserved", core::mem::offset_of!(PxuiPanel, reserved)),
            ("max_height", core::mem::offset_of!(PxuiPanel, max_height)), ("stroke", core::mem::offset_of!(PxuiPanel, stroke)), ("look", core::mem::offset_of!(PxuiPanel, look)),
        ]);
        assert_layout("PxuiTargetDesc", size_of::<PxuiTargetDesc>(), align_of::<PxuiTargetDesc>(), &[
            ("size", core::mem::offset_of!(PxuiTargetDesc, size)), ("format", core::mem::offset_of!(PxuiTargetDesc, format)), ("samples", core::mem::offset_of!(PxuiTargetDesc, samples)),
            ("reserved", core::mem::offset_of!(PxuiTargetDesc, reserved)), ("mask_width", core::mem::offset_of!(PxuiTargetDesc, mask_width)), ("mask_height", core::mem::offset_of!(PxuiTargetDesc, mask_height)),
            ("color_width", core::mem::offset_of!(PxuiTargetDesc, color_width)), ("color_height", core::mem::offset_of!(PxuiTargetDesc, color_height)), ("reserved_tail", core::mem::offset_of!(PxuiTargetDesc, reserved_tail)),
        ]);
        assert_layout("PxuiUiDesc", size_of::<PxuiUiDesc>(), align_of::<PxuiUiDesc>(), &[
            ("size", core::mem::offset_of!(PxuiUiDesc, size)), ("platform", core::mem::offset_of!(PxuiUiDesc, platform)), ("font_size", core::mem::offset_of!(PxuiUiDesc, font_size)),
        ]);
        assert_layout("TextStyle", size_of::<TextStyle>(), align_of::<TextStyle>(), &[
            ("size", core::mem::offset_of!(TextStyle, size)), ("font", core::mem::offset_of!(TextStyle, font)), ("weight", core::mem::offset_of!(TextStyle, weight)),
            ("size_points", core::mem::offset_of!(TextStyle, size_points)), ("line_height", core::mem::offset_of!(TextStyle, line_height)), ("wrap", core::mem::offset_of!(TextStyle, wrap)),
            ("align", core::mem::offset_of!(TextStyle, align)), ("max_lines", core::mem::offset_of!(TextStyle, max_lines)), ("italic", core::mem::offset_of!(TextStyle, italic)),
            ("ellipsis", core::mem::offset_of!(TextStyle, ellipsis)), ("features", core::mem::offset_of!(TextStyle, features)), ("tracking", core::mem::offset_of!(TextStyle, tracking)),
        ]);
        assert_layout("TextSpan",size_of::<TextSpan>(),align_of::<TextSpan>(),&[
            ("start", core::mem::offset_of!(TextSpan, start)),("end", core::mem::offset_of!(TextSpan, end)),("weight", core::mem::offset_of!(TextSpan, weight)),("flags", core::mem::offset_of!(TextSpan, flags)),("color", core::mem::offset_of!(TextSpan, color)),("background", core::mem::offset_of!(TextSpan, background)),("underline", core::mem::offset_of!(TextSpan, underline)),
        ]);
        assert_layout("TextSpans",size_of::<TextSpans>(),align_of::<TextSpans>(),&[("size", core::mem::offset_of!(TextSpans, size)),("count", core::mem::offset_of!(TextSpans, count)),("spans", core::mem::offset_of!(TextSpans, spans))]);
        assert_layout("TextMetrics", size_of::<TextMetrics>(), align_of::<TextMetrics>(), &[
            ("width", core::mem::offset_of!(TextMetrics, width)), ("height", core::mem::offset_of!(TextMetrics, height)), ("baseline", core::mem::offset_of!(TextMetrics, baseline)), ("lines", core::mem::offset_of!(TextMetrics, lines)),
        ]);
        assert_layout("TextGlyph", size_of::<TextGlyph>(), align_of::<TextGlyph>(), &[
            ("rect", core::mem::offset_of!(TextGlyph, rect)), ("uv", core::mem::offset_of!(TextGlyph, uv)), ("atlas", core::mem::offset_of!(TextGlyph, atlas)), ("reserved", core::mem::offset_of!(TextGlyph, reserved)),
        ]);
    }

    /// The engine's own value types that cross as they are (a node, a look, the theme): `pxui.h` mirrors them field for field.
    pub(crate) fn value_types_rows() {
        use crate::{layout::*, node::*, theme::*};
        assert_layout("PxuiSize", size_of::<Size>(), align_of::<Size>(), &[]);
        assert_layout("PxuiTrack", size_of::<Track>(), align_of::<Track>(), &[]);
        assert_layout("PxuiMark", size_of::<Mark>(), align_of::<Mark>(), &[]);
        assert_layout("PxuiTransition", size_of::<Transition>(), align_of::<Transition>(), &[]);
        assert_layout("PxuiOptAlign", size_of::<OptAlign>(), align_of::<OptAlign>(), &[]);
        assert_layout("PxuiOptGrid", size_of::<OptGrid>(), align_of::<OptGrid>(), &[]);
        assert_layout("PxuiOptAnchor", size_of::<OptAnchor>(), align_of::<OptAnchor>(), &[]);
        assert_layout("PxuiOptPoint", size_of::<OptPoint>(), align_of::<OptPoint>(), &[]);
        assert_layout("PxuiOptBar", size_of::<OptBar>(), align_of::<OptBar>(), &[]);
        assert_layout("PxuiOptTransition", size_of::<OptTransition>(), align_of::<OptTransition>(), &[]);
        assert_layout("PxuiOptColor", size_of::<OptColor>(), align_of::<OptColor>(), &[]);
        assert_layout("PxuiGrid", size_of::<Grid>(), align_of::<Grid>(), &[]);
        assert_layout("PxuiCell", size_of::<Cell>(), align_of::<Cell>(), &[("col", core::mem::offset_of!(Cell, col)), ("row", core::mem::offset_of!(Cell, row)), ("col_span", core::mem::offset_of!(Cell, col_span)), ("row_span", core::mem::offset_of!(Cell, row_span))]);
        assert_layout("PxuiPlacement", size_of::<Placement>(), align_of::<Placement>(), &[("side", core::mem::offset_of!(Placement, side)), ("align", core::mem::offset_of!(Placement, align)), ("gap", core::mem::offset_of!(Placement, gap)), ("margin", core::mem::offset_of!(Placement, margin)), ("flip", core::mem::offset_of!(Placement, flip))]);
        assert_layout("PxuiAnchor", size_of::<Anchor>(), align_of::<Anchor>(), &[("rect", core::mem::offset_of!(Anchor, rect)), ("placement", core::mem::offset_of!(Anchor, placement))]);
        assert_layout("PxuiFont", size_of::<Font>(), align_of::<Font>(), &[("family", core::mem::offset_of!(Font, family)), ("size", core::mem::offset_of!(Font, size)), ("weight", core::mem::offset_of!(Font, weight)), ("tracking", core::mem::offset_of!(Font, tracking)), ("caps", core::mem::offset_of!(Font, caps))]);
        assert_layout("PxuiShadow", size_of::<Shadow>(), align_of::<Shadow>(), &[("color", core::mem::offset_of!(Shadow, color)), ("offset", core::mem::offset_of!(Shadow, offset)), ("blur", core::mem::offset_of!(Shadow, blur)), ("spread", core::mem::offset_of!(Shadow, spread)), ("inner", core::mem::offset_of!(Shadow, inner))]);
        assert_layout("PxuiRing", size_of::<Ring>(), align_of::<Ring>(), &[("color", core::mem::offset_of!(Ring, color)), ("width", core::mem::offset_of!(Ring, width)), ("sides", core::mem::offset_of!(Ring, sides))]);
        assert_layout("PxuiLook", size_of::<Look>(), align_of::<Look>(), &[
            ("fill", core::mem::offset_of!(Look, fill)), ("gradient", core::mem::offset_of!(Look, gradient)), ("angle", core::mem::offset_of!(Look, angle)), ("border", core::mem::offset_of!(Look, border)), ("border_w", core::mem::offset_of!(Look, border_w)), ("sides", core::mem::offset_of!(Look, sides)),
            ("radius", core::mem::offset_of!(Look, radius)), ("shadows", core::mem::offset_of!(Look, shadows)), ("text", core::mem::offset_of!(Look, text)), ("font", core::mem::offset_of!(Look, font)), ("opacity", core::mem::offset_of!(Look, opacity)), ("ring", core::mem::offset_of!(Look, ring)),
            ("selection", core::mem::offset_of!(Look, selection)), ("caret", core::mem::offset_of!(Look, caret)), ("dashed", core::mem::offset_of!(Look, dashed)),
        ]);
        assert_layout("PxuiState", size_of::<State>(), align_of::<State>(), &[("hovered", core::mem::offset_of!(State, hovered)), ("pressed", core::mem::offset_of!(State, pressed)), ("focused", core::mem::offset_of!(State, focused)), ("disabled", core::mem::offset_of!(State, disabled)), ("checked", core::mem::offset_of!(State, checked))]);
        assert_layout("PxuiBar", size_of::<Bar>(), align_of::<Bar>(), &[("color", core::mem::offset_of!(Bar, color)), ("radius", core::mem::offset_of!(Bar, radius)), ("width", core::mem::offset_of!(Bar, width)), ("gap", core::mem::offset_of!(Bar, gap)), ("margin", core::mem::offset_of!(Bar, margin)), ("whole", core::mem::offset_of!(Bar, whole))]);
        assert_layout("PxuiNode", size_of::<Node>(), align_of::<Node>(), &[
            ("size", core::mem::offset_of!(Node, size)), ("axis", core::mem::offset_of!(Node, axis)), ("justify", core::mem::offset_of!(Node, justify)), ("align", core::mem::offset_of!(Node, align)), ("wrap", core::mem::offset_of!(Node, wrap)), ("role", core::mem::offset_of!(Node, role)),
            ("state", core::mem::offset_of!(Node, state)), ("z", core::mem::offset_of!(Node, z)), ("flags", core::mem::offset_of!(Node, flags)), ("w", core::mem::offset_of!(Node, w)), ("h", core::mem::offset_of!(Node, h)), ("min_w", core::mem::offset_of!(Node, min_w)),
            ("min_h", core::mem::offset_of!(Node, min_h)), ("max_w", core::mem::offset_of!(Node, max_w)), ("max_h", core::mem::offset_of!(Node, max_h)), ("grow", core::mem::offset_of!(Node, grow)), ("shrink", core::mem::offset_of!(Node, shrink)), ("aspect", core::mem::offset_of!(Node, aspect)),
            ("padding", core::mem::offset_of!(Node, padding)), ("margin", core::mem::offset_of!(Node, margin)), ("gap", core::mem::offset_of!(Node, gap)), ("row_gap", core::mem::offset_of!(Node, row_gap)), ("grid", core::mem::offset_of!(Node, grid)), ("cell", core::mem::offset_of!(Node, cell)),
            ("align_self", core::mem::offset_of!(Node, align_self)), ("anchor", core::mem::offset_of!(Node, anchor)), ("a11y", core::mem::offset_of!(Node, a11y)), ("at", core::mem::offset_of!(Node, at)), ("scroll_step", core::mem::offset_of!(Node, scroll_step)), ("look", core::mem::offset_of!(Node, look)),
            ("inset", core::mem::offset_of!(Node, inset)), ("mark", core::mem::offset_of!(Node, mark)), ("bar", core::mem::offset_of!(Node, bar)), ("transition", core::mem::offset_of!(Node, transition)),
        ]);
        assert_layout("PxuiMotion", size_of::<Motion>(), align_of::<Motion>(), &[("fast", core::mem::offset_of!(Motion, fast)), ("normal", core::mem::offset_of!(Motion, normal)), ("slow", core::mem::offset_of!(Motion, slow)), ("ease", core::mem::offset_of!(Motion, ease))]);
        assert_layout("PxuiTokens", size_of::<Tokens>(), align_of::<Tokens>(), &[
            ("panel", core::mem::offset_of!(Tokens, panel)), ("foreground", core::mem::offset_of!(Tokens, foreground)), ("control", core::mem::offset_of!(Tokens, control)), ("input", core::mem::offset_of!(Tokens, input)), ("track", core::mem::offset_of!(Tokens, track)),
            ("accent", core::mem::offset_of!(Tokens, accent)), ("invalid", core::mem::offset_of!(Tokens, invalid)), ("ink_ground", core::mem::offset_of!(Tokens, ink_ground)), ("space", core::mem::offset_of!(Tokens, space)), ("radius", core::mem::offset_of!(Tokens, radius)),
            ("display_size", core::mem::offset_of!(Tokens, display_size)), ("title_size", core::mem::offset_of!(Tokens, title_size)), ("font_size", core::mem::offset_of!(Tokens, font_size)), ("label_size", core::mem::offset_of!(Tokens, label_size)), ("motion", core::mem::offset_of!(Tokens, motion)),
            ("reduced_motion", core::mem::offset_of!(Tokens, reduced_motion)), ("density", core::mem::offset_of!(Tokens, density)),
        ]);
        assert_layout("PxuiStates", size_of::<States>(), align_of::<States>(), &[("idle", core::mem::offset_of!(States, idle)), ("hover", core::mem::offset_of!(States, hover)), ("pressed", core::mem::offset_of!(States, pressed)), ("focus", core::mem::offset_of!(States, focus)), ("disabled", core::mem::offset_of!(States, disabled)), ("checked", core::mem::offset_of!(States, checked))]);
        // the kit parts follow `ring`, `scrollbar_w` and `scrollbar_gap` back to back, in `theme_widgets!` order, then the bodies
        assert_layout("PxuiThemeSnapshot", size_of::<ThemeSnapshot>(), align_of::<ThemeSnapshot>(), &[
            ("tokens", core::mem::offset_of!(ThemeSnapshot, tokens)), ("row_h", core::mem::offset_of!(ThemeSnapshot, row_h)), ("control_h", core::mem::offset_of!(ThemeSnapshot, control_h)), ("ring", core::mem::offset_of!(ThemeSnapshot, ring)),
            ("scrollbar_w", core::mem::offset_of!(ThemeSnapshot, scrollbar_w)), ("scrollbar_gap", core::mem::offset_of!(ThemeSnapshot, scrollbar_gap)), ("panel", core::mem::offset_of!(ThemeSnapshot, panel)), ("body", core::mem::offset_of!(ThemeSnapshot, body)),
        ]);
        assert_eq!(core::mem::offset_of!(ThemeSnapshot, body), core::mem::offset_of!(ThemeSnapshot, panel) + WIDGET_COUNT * size_of::<States>(), "the parts are an array of States, then the bodies");
    }

    pub(crate) fn frozen_rows() {
        // pxui.h calls `Point` PxuiPoint; the manifest pins both names
        assert_layout("PxuiPoint", size_of::<geom::Point>(), align_of::<geom::Point>(), &[("x", core::mem::offset_of!(geom::Point, x)), ("y", core::mem::offset_of!(geom::Point, y))]);
        assert_layout(
            "Point",
            size_of::<geom::Point>(),
            align_of::<geom::Point>(),
            &[("x", core::mem::offset_of!(geom::Point, x)), ("y", core::mem::offset_of!(geom::Point, y))],
        );
        assert_layout(
            "Rect",
            size_of::<geom::Rect>(),
            align_of::<geom::Rect>(),
            &[
                ("x", core::mem::offset_of!(geom::Rect, x)),
                ("y", core::mem::offset_of!(geom::Rect, y)),
                ("w", core::mem::offset_of!(geom::Rect, w)),
                ("h", core::mem::offset_of!(geom::Rect, h)),
            ],
        );
        assert_layout(
            "Edges",
            size_of::<geom::Edges>(),
            align_of::<geom::Edges>(),
            &[
                ("l", core::mem::offset_of!(geom::Edges, l)),
                ("t", core::mem::offset_of!(geom::Edges, t)),
                ("r", core::mem::offset_of!(geom::Edges, r)),
                ("b", core::mem::offset_of!(geom::Edges, b)),
            ],
        );
        assert_layout("Flags", size_of::<Flags>(), align_of::<Flags>(), &[]);
        assert_layout("Size", size_of::<Size>(), align_of::<Size>(), &[]);
        assert_layout(
            "Role",
            size_of::<a11y::Role>(),
            align_of::<a11y::Role>(),
            &[],
        );
        assert_layout(
            "Rgba",
            size_of::<geom::Rgba>(),
            align_of::<geom::Rgba>(),
            &[
                ("r", core::mem::offset_of!(geom::Rgba, r)),
                ("g", core::mem::offset_of!(geom::Rgba, g)),
                ("b", core::mem::offset_of!(geom::Rgba, b)),
                ("a", core::mem::offset_of!(geom::Rgba, a)),
            ],
        );
        assert_layout(
            "Instance",
            size_of::<draw::Instance>(),
            align_of::<draw::Instance>(),
            &[
                ("rect", core::mem::offset_of!(draw::Instance, rect)),
                ("clip", core::mem::offset_of!(draw::Instance, clip)),
                ("color", core::mem::offset_of!(draw::Instance, color)),
                ("color2", core::mem::offset_of!(draw::Instance, color2)),
                ("aux", core::mem::offset_of!(draw::Instance, aux)),
                ("radii", core::mem::offset_of!(draw::Instance, radii)),
                ("width", core::mem::offset_of!(draw::Instance, width)),
                ("kind", core::mem::offset_of!(draw::Instance, kind)),
                ("flags", core::mem::offset_of!(draw::Instance, flags)),
                ("view", core::mem::offset_of!(draw::Instance, view)),
                ("clip_radius", core::mem::offset_of!(draw::Instance, clip_radius)),
                ("extra", core::mem::offset_of!(draw::Instance, extra)),
            ],
        );
        assert_layout(
            "View",
            size_of::<View>(),
            align_of::<View>(),
            &[
                ("offset", core::mem::offset_of!(View, offset)),
                ("opacity", core::mem::offset_of!(View, opacity)),
                ("clip", core::mem::offset_of!(View, clip)),
                ("scale", core::mem::offset_of!(View, scale)),
                ("transform", core::mem::offset_of!(View, transform)),
                ("pivot", core::mem::offset_of!(View, pivot)),
            ],
        );
        assert_layout(
            "Str",
            size_of::<Str>(),
            align_of::<Str>(),
            &[("ptr", core::mem::offset_of!(Str, ptr)), ("len", core::mem::offset_of!(Str, len))],
        );
        assert_layout(
            "Event",
            size_of::<Event>(),
            align_of::<Event>(),
            &[
                ("kind", core::mem::offset_of!(Event, kind)),
                ("code", core::mem::offset_of!(Event, code)),
                ("x", core::mem::offset_of!(Event, x)),
                ("y", core::mem::offset_of!(Event, y)),
                ("value", core::mem::offset_of!(Event, value)),
                ("text", core::mem::offset_of!(Event, text)),
            ],
        );
        assert_layout(
            "FrameIn",
            core::mem::offset_of!(FrameIn,appearance), // frozen v1 prefix, accepted from older callers
            align_of::<FrameIn>(),
            &[
                ("size", core::mem::offset_of!(FrameIn, size)),
                ("events", core::mem::offset_of!(FrameIn, events)),
                ("event_count", core::mem::offset_of!(FrameIn, event_count)),
                ("width", core::mem::offset_of!(FrameIn, width)),
                ("height", core::mem::offset_of!(FrameIn, height)),
                ("scale", core::mem::offset_of!(FrameIn, scale)),
                ("time", core::mem::offset_of!(FrameIn, time)),
            ],
        );
        assert_layout("FrameInV1_8",core::mem::offset_of!(FrameIn,environment),align_of::<FrameIn>(),&[
            ("size", core::mem::offset_of!(FrameIn, size)),("events", core::mem::offset_of!(FrameIn, events)),("event_count", core::mem::offset_of!(FrameIn, event_count)),
            ("width", core::mem::offset_of!(FrameIn, width)),("height", core::mem::offset_of!(FrameIn, height)),("scale", core::mem::offset_of!(FrameIn, scale)),("time", core::mem::offset_of!(FrameIn, time)),
            ("appearance", core::mem::offset_of!(FrameIn, appearance)),("reduced_motion", core::mem::offset_of!(FrameIn, reduced_motion)),
        ]);
        assert_layout("Touch",size_of::<Touch>(),align_of::<Touch>(),&[("device", core::mem::offset_of!(Touch, device)),("finger", core::mem::offset_of!(Touch, finger)),("x", core::mem::offset_of!(Touch, x)),("y", core::mem::offset_of!(Touch, y)),("time", core::mem::offset_of!(Touch, time)),("pressure", core::mem::offset_of!(Touch, pressure)),("phase", core::mem::offset_of!(Touch, phase))]);
        assert_layout("Environment",size_of::<Environment>(),align_of::<Environment>(),&[
            ("safe_area", core::mem::offset_of!(Environment, safe_area)),("keyboard_frame", core::mem::offset_of!(Environment, keyboard_frame)),("keyboard_from", core::mem::offset_of!(Environment, keyboard_from)),
            ("keyboard_started", core::mem::offset_of!(Environment, keyboard_started)),("keyboard_duration", core::mem::offset_of!(Environment, keyboard_duration)),("keyboard_curve", core::mem::offset_of!(Environment, keyboard_curve)),("keyboard_visible", core::mem::offset_of!(Environment, keyboard_visible)),
        ]);
        assert_layout("FrameInV1_24",core::mem::offset_of!(FrameIn,keyboard),align_of::<FrameIn>(),&[
            ("size", core::mem::offset_of!(FrameIn, size)),("events", core::mem::offset_of!(FrameIn, events)),("event_count", core::mem::offset_of!(FrameIn, event_count)),("width", core::mem::offset_of!(FrameIn, width)),
            ("height", core::mem::offset_of!(FrameIn, height)),("scale", core::mem::offset_of!(FrameIn, scale)),("time", core::mem::offset_of!(FrameIn, time)),("appearance", core::mem::offset_of!(FrameIn, appearance)),("reduced_motion", core::mem::offset_of!(FrameIn, reduced_motion)),("environment", core::mem::offset_of!(FrameIn, environment)),
        ]);
        assert_layout("Keyboard",size_of::<Keyboard>(),align_of::<Keyboard>(),&[("frame", core::mem::offset_of!(Keyboard, frame)),("flags", core::mem::offset_of!(Keyboard, flags))]);
        assert_layout("TextInputOptions",size_of::<PxuiTextInputOptions>(),align_of::<PxuiTextInputOptions>(),&[("autocorrect", core::mem::offset_of!(PxuiTextInputOptions, autocorrect)),("capitalization", core::mem::offset_of!(PxuiTextInputOptions, capitalization)),("action", core::mem::offset_of!(PxuiTextInputOptions, action))]);
        assert_layout("FrameInV1_26",size_of::<FrameIn>(),align_of::<FrameIn>(),&[
            ("size", core::mem::offset_of!(FrameIn, size)),("events", core::mem::offset_of!(FrameIn, events)),("event_count", core::mem::offset_of!(FrameIn, event_count)),("width", core::mem::offset_of!(FrameIn, width)),
            ("height", core::mem::offset_of!(FrameIn, height)),("scale", core::mem::offset_of!(FrameIn, scale)),("time", core::mem::offset_of!(FrameIn, time)),("appearance", core::mem::offset_of!(FrameIn, appearance)),("reduced_motion", core::mem::offset_of!(FrameIn, reduced_motion)),("environment", core::mem::offset_of!(FrameIn, environment)),("keyboard", core::mem::offset_of!(FrameIn, keyboard)),
        ]);
        assert_layout(
            "FrameOut",
            size_of::<FrameOut>(),
            align_of::<FrameOut>(),
            &[
                ("size", core::mem::offset_of!(FrameOut, size)),
                ("damage", core::mem::offset_of!(FrameOut, damage)),
                ("has_damage", core::mem::offset_of!(FrameOut, has_damage)),
                ("reserved", core::mem::offset_of!(FrameOut, reserved)),
                ("layer_count", core::mem::offset_of!(FrameOut, layer_count)),
                ("layers", core::mem::offset_of!(FrameOut, layers)),
            ],
        );
        assert_layout(
            "Gpu",
            size_of::<Gpu>(),
            align_of::<Gpu>(),
            &[
                ("size", core::mem::offset_of!(Gpu, size)),
                ("backend", core::mem::offset_of!(Gpu, backend)),
                ("instance", core::mem::offset_of!(Gpu, instance)),
                ("physical_device", core::mem::offset_of!(Gpu, physical_device)),
                ("device", core::mem::offset_of!(Gpu, device)),
                ("queue", core::mem::offset_of!(Gpu, queue)),
                ("queue_family", core::mem::offset_of!(Gpu, queue_family)),
                ("api_version", core::mem::offset_of!(Gpu, api_version)),
                ("features", core::mem::offset_of!(Gpu, features)),
                ("get_instance_proc_addr", core::mem::offset_of!(Gpu, get_instance_proc_addr)),
                ("pipeline_cache", core::mem::offset_of!(Gpu, pipeline_cache)),
                ("mtl_device", core::mem::offset_of!(Gpu, mtl_device)),
                ("wl_display", core::mem::offset_of!(Gpu, wl_display)),
                ("wl_surface", core::mem::offset_of!(Gpu, wl_surface)),
                ("metal_layer", core::mem::offset_of!(Gpu, metal_layer)),
                ("hwnd", core::mem::offset_of!(Gpu, hwnd)),
                ("owner_token", core::mem::offset_of!(Gpu, owner_token)),
                ("release_owner", core::mem::offset_of!(Gpu, release_owner)),
            ],
        );
        assert_layout(
            "Commands",
            size_of::<Commands>(),
            align_of::<Commands>(),
            &[
                ("size", core::mem::offset_of!(Commands, size)),
                ("vk_command_buffer", core::mem::offset_of!(Commands, vk_command_buffer)),
                ("mtl_command_buffer", core::mem::offset_of!(Commands, mtl_command_buffer)),
            ],
        );
        assert_layout(
            "Pass",
            size_of::<Pass>(),
            align_of::<Pass>(),
            &[
                ("size", core::mem::offset_of!(Pass, size)),
                ("vk_command_buffer", core::mem::offset_of!(Pass, vk_command_buffer)),
                ("mtl_render_encoder", core::mem::offset_of!(Pass, mtl_render_encoder)),
                ("width_px", core::mem::offset_of!(Pass, width_px)),
                ("height_px", core::mem::offset_of!(Pass, height_px)),
                ("scale", core::mem::offset_of!(Pass, scale)),
                ("mtl_command_buffer", core::mem::offset_of!(Pass, mtl_command_buffer)),
                ("first_layer", core::mem::offset_of!(Pass, first_layer)),
                ("layer_count", core::mem::offset_of!(Pass, layer_count)),
                ("split", core::mem::offset_of!(Pass, split)),
                ("split_user", core::mem::offset_of!(Pass, split_user)),
            ],
        );
        assert_layout(
            "Image",
            size_of::<Image>(),
            align_of::<Image>(),
            &[
                ("size", core::mem::offset_of!(Image, size)),
                ("vk_image_view", core::mem::offset_of!(Image, vk_image_view)),
                ("mtl_texture", core::mem::offset_of!(Image, mtl_texture)),
                ("width", core::mem::offset_of!(Image, width)),
                ("height", core::mem::offset_of!(Image, height)),
                ("format", core::mem::offset_of!(Image, format)),
            ],
        );
        assert_layout(
            "Span",
            size_of::<Span>(),
            align_of::<Span>(),
            &[
                ("size", core::mem::offset_of!(Span, size)),
                ("view", core::mem::offset_of!(Span, view)),
                ("clip_radius_f16", core::mem::offset_of!(Span, clip_radius_f16)),
                ("clip", core::mem::offset_of!(Span, clip)),
                ("at", core::mem::offset_of!(Span, at)),
                ("end", core::mem::offset_of!(Span, end)),
            ],
        );
    }
}

#[cfg(test)]
mod builders {
    use super::*;
    use crate::a11y::Role;
    use crate::geom::{Edges, Rgba};
    use crate::layout::{Align, Justify};

    const INK: Rgba = Rgba { r: 10, g: 20, b: 30, a: 255 };
    const FONT: Font = Font::BODY.size(14).weight(700);
    const LOOK: Look = Look::NONE.fill(INK).radius(6.0).border(INK, 1.0).text(INK).font(FONT);
    const NODE: Node = Node::row().w(Size::Px(44.0)).h_px(30.0).grow(0.0).gap(8.0).pad(2.0).pad_x(4.0).pad_y(6.0)
        .center().align(Align::End).role(Role::Button).flags(Flags(5));
    const COLUMN: Node = Node::column();
    const STACK: Node = Node::stack().w(Size::Fit).h(Size::Grow);

    // Node has no PartialEq (Size holds a fn pointer, the record has padding): compare the Debug text of every field.
    fn same(a: &Node, b: &Node) {
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }

    #[test]
    fn builder_chains_equal_struct_literals() {
        same(&NODE, &Node {
            axis: Axis::Row,
            w: Size::Px(44.0),
            h: Size::Px(30.0),
            grow: 0.0,
            gap: 8.0,
            padding: Edges { l: 4.0, t: 6.0, r: 4.0, b: 6.0 },
            justify: Justify::Center,
            align: Align::End,
            role: Role::Button,
            flags: Flags(5),
            ..Node::DEFAULT
        });
        same(&COLUMN, &Node { axis: Axis::Column, ..Node::DEFAULT });
        same(&STACK, &Node { axis: Axis::Stack, w: Size::Fit, h: Size::Grow, ..Node::DEFAULT });
        assert_eq!(Edges::xy(1.0, 2.0), Edges { l: 1.0, t: 2.0, r: 1.0, b: 2.0 });
        assert_eq!(FONT, Font { size: 14, weight: 700, ..Font::BODY });
        assert_eq!(LOOK, Look {
            fill: INK,
            radius: [6.0; 4],
            border: INK,
            border_w: 1.0,
            text: INK,
            font: Font { size: 14, weight: 700, ..Font::BODY },
            ..Look::NONE
        });
    }

    #[test]
    fn pad_after_pad_x_keeps_the_later_edge() {
        let n = Node::row().pad_x(4.0).pad(2.0);
        assert_eq!(n.padding, Edges::all(2.0));
    }

    #[test]
    fn node_focusable_and_a11y_focusable_are_distinct_names() {
        assert_eq!(Flags::FOCUSABLE.bits(), 2);
        assert_eq!(a11y::state::FOCUSABLE, 1);
        assert_eq!(Flags::CLICKABLE | Flags::TAB_ONLY | Flags::FOCUSABLE, Flags::TAB_STOP | Flags::CLICKABLE);
        assert_eq!(size_of::<Flags>(), size_of::<i64>());
        let n = Node::row().flags(Flags::FOCUSABLE);
        assert_eq!(n.flags, Flags::FOCUSABLE);
        assert_eq!(n.a11y, 0, "a node flag does not set an a11y state");
    }
}

/// Frozen identity vectors (ABI policy: identity hashes are part of the promise). A change to any value here changes every saved
/// node identity; deliberately changing one means a new major, never an edit of this table.
#[cfg(test)]
mod identity_vectors {
    use crate::geom::hash;
    use crate::ui::{hash_range, int_key, key_of, Key, ROOT_KEY};

    #[test]
    fn hashes_match_the_published_vectors() {
        assert_eq!(hash::bytes(0, b""), 16294208416658607535);
        assert_eq!(hash::bytes(1, b"abc"), 2202381260512884014);
        assert_eq!(hash::bytes(1, b"abcdefghi"), 408265595569890452);
        assert_eq!(hash::mix(0, 0), -2152535657050944081);
        assert_eq!(hash::mix(5, 1), 7958955049054603978);
        assert_eq!(hash_range(7, "xxcdxx", 2, 4), 5608514096287000090);
    }

    #[test]
    fn labels_and_integer_keys_match_the_published_vectors() {
        assert_eq!(key_of(ROOT_KEY, "panel"), 440531318097480381);
        assert_eq!(key_of(ROOT_KEY, ""), -6041832124274164334);
        assert_eq!(key_of(ROOT_KEY, "Play##hot"), 1891176646085460787);
        assert_eq!(key_of(ROOT_KEY, "Play###a"), 3246571133750751553);
        assert_eq!(key_of(7, "Za\u{17c}\u{f3}\u{142}\u{107}"), 663447996966360505);
        assert_eq!(int_key(5, 1), 7958955049054603978);
        assert_eq!(int_key(5, -1), -1659232436457042047);
        assert_eq!(int_key(ROOT_KEY, 0), -6041832124274164334);
        assert_eq!(int_key(0, 0), -2152535657050944081);
    }

    #[test]
    fn an_id_key_is_its_nul_and_hex_label_under_the_parent() {
        let mut buf = [0u8; 17];
        let label = Key::Id(0x1234).label(&mut buf);
        assert_eq!(key_of(7, label), -3152285460647825701);
    }

    #[test]
    fn one_integer_under_two_parents_gives_two_keys() {
        for n in [-1, 0, 1, 2, 1 << 40] {
            assert_ne!(int_key(5, n), int_key(6, n));
            assert_ne!(key_of(5, "row"), key_of(6, "row"));
        }
        assert_eq!(int_key(5, 3), int_key(5, 3));
    }
}
