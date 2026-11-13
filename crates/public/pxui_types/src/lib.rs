#![allow(clippy::all)]
//! Plain, stable value types shared by PXUI and applications.

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
pub use text::{TextGlyph, TextMetrics, TextStyle};
pub use theme::{Curve, Font, Look, Motion, Ring, Shadow, State, States, Tokens};

/// Node behavior bits. Kept as an integer so unknown future bits remain representable.
pub type Flags = i64;

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
}
impl Default for FrameIn {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, events: core::ptr::null(), event_count: 0, width: 0.0, height: 0.0, scale: 1.0, time: 0.0 } }
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
pub struct PxuiBox { pub index: u64, pub box_key: i64, pub box_slot: u64, pub generation: u32 }

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiPanel {
    pub size: u32, pub x: f32, pub y: f32, pub width: f32, pub height: f32,
    pub padding: i64, pub has_height: u8, pub window: u8,
    pub has_max_height: u8, pub has_stroke: u8, pub has_look: u8, pub reserved: [u8; 3],
    pub max_height: f32, pub stroke: geom::Rgba, pub look: ffi::PxuiLookDesc,
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
    pub size: u32, pub format: u32, pub samples: u8, pub reserved: [u8; 3],
    pub mask_width: u32, pub mask_height: u32, pub color_width: u32, pub color_height: u32,
    pub reserved_tail: u32,
}
impl Default for PxuiTargetDesc { fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, format: 0, samples: 1, reserved: [0; 3], mask_width: 0, mask_height: 0, color_width: 0, color_height: 0, reserved_tail: 0 } } }

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiUiDesc { pub size: u32, pub platform: u32, pub font_size: i32 }
impl Default for PxuiUiDesc { fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, platform: 0, font_size: 0 } } }

#[cfg(test)]
mod abi_layout {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    fn assert_layout(name: &str, size: usize, align: usize, offsets: &[usize]) {
        let row = include_str!("../../../../abi/layout.v1.txt")
            .lines()
            .find(|line| line.split_whitespace().next() == Some(name))
            .unwrap_or_else(|| panic!("missing layout row for {name}"));
        let mut fields = row.split_whitespace().skip(1);
        assert_eq!(
            fields.next().unwrap().parse::<usize>().unwrap(),
            size,
            "{name} size"
        );
        assert_eq!(
            fields.next().unwrap().parse::<usize>().unwrap(),
            align,
            "{name} alignment"
        );
        let expected: Vec<usize> = fields.map(|field| field.parse().unwrap()).collect();
        assert_eq!(expected, offsets, "{name} field offsets");
    }

    #[test]
    fn abi_descriptors_match_layout_snapshot() {
        assert_layout("PxuiBox", size_of::<PxuiBox>(), align_of::<PxuiBox>(), &[
            offset_of!(PxuiBox, index), offset_of!(PxuiBox, box_key), offset_of!(PxuiBox, box_slot), offset_of!(PxuiBox, generation),
        ]);
        assert_layout("PxuiPanel", size_of::<PxuiPanel>(), align_of::<PxuiPanel>(), &[
            offset_of!(PxuiPanel, size), offset_of!(PxuiPanel, x), offset_of!(PxuiPanel, y), offset_of!(PxuiPanel, width),
            offset_of!(PxuiPanel, height), offset_of!(PxuiPanel, padding), offset_of!(PxuiPanel, has_height), offset_of!(PxuiPanel, window),
            offset_of!(PxuiPanel, has_max_height), offset_of!(PxuiPanel, has_stroke), offset_of!(PxuiPanel, has_look), offset_of!(PxuiPanel, reserved),
            offset_of!(PxuiPanel, max_height), offset_of!(PxuiPanel, stroke), offset_of!(PxuiPanel, look),
        ]);
        assert_layout("PxuiTargetDesc", size_of::<PxuiTargetDesc>(), align_of::<PxuiTargetDesc>(), &[
            offset_of!(PxuiTargetDesc, size), offset_of!(PxuiTargetDesc, format), offset_of!(PxuiTargetDesc, samples),
            offset_of!(PxuiTargetDesc, reserved), offset_of!(PxuiTargetDesc, mask_width), offset_of!(PxuiTargetDesc, mask_height),
            offset_of!(PxuiTargetDesc, color_width), offset_of!(PxuiTargetDesc, color_height), offset_of!(PxuiTargetDesc, reserved_tail),
        ]);
        assert_layout("PxuiUiDesc", size_of::<PxuiUiDesc>(), align_of::<PxuiUiDesc>(), &[
            offset_of!(PxuiUiDesc, size), offset_of!(PxuiUiDesc, platform), offset_of!(PxuiUiDesc, font_size),
        ]);
        assert_layout("TextStyle", size_of::<TextStyle>(), align_of::<TextStyle>(), &[
            offset_of!(TextStyle, size), offset_of!(TextStyle, font), offset_of!(TextStyle, weight),
            offset_of!(TextStyle, size_points), offset_of!(TextStyle, line_height), offset_of!(TextStyle, wrap),
            offset_of!(TextStyle, align), offset_of!(TextStyle, max_lines), offset_of!(TextStyle, italic),
            offset_of!(TextStyle, ellipsis), offset_of!(TextStyle, features), offset_of!(TextStyle, tracking),
        ]);
        assert_layout("TextMetrics", size_of::<TextMetrics>(), align_of::<TextMetrics>(), &[
            offset_of!(TextMetrics, width), offset_of!(TextMetrics, height), offset_of!(TextMetrics, baseline), offset_of!(TextMetrics, lines),
        ]);
        assert_layout("TextGlyph", size_of::<TextGlyph>(), align_of::<TextGlyph>(), &[
            offset_of!(TextGlyph, rect), offset_of!(TextGlyph, uv), offset_of!(TextGlyph, atlas), offset_of!(TextGlyph, reserved),
        ]);
    }

    /// The engine's own value types that cross as they are (a node, a look, the theme): `pxui.h` mirrors them field for field.
    #[test]
    fn value_types_match_layout_snapshot() {
        use crate::{layout::*, node::*, theme::*};
        assert_layout("PxuiSize", size_of::<Size>(), align_of::<Size>(), &[]);
        assert_layout("PxuiTrack", size_of::<Track>(), align_of::<Track>(), &[]);
        assert_layout("PxuiMark", size_of::<Mark>(), align_of::<Mark>(), &[]);
        assert_layout("PxuiCurve", size_of::<Curve>(), align_of::<Curve>(), &[]);
        assert_layout("PxuiOptAlign", size_of::<OptAlign>(), align_of::<OptAlign>(), &[]);
        assert_layout("PxuiOptGrid", size_of::<OptGrid>(), align_of::<OptGrid>(), &[]);
        assert_layout("PxuiOptAnchor", size_of::<OptAnchor>(), align_of::<OptAnchor>(), &[]);
        assert_layout("PxuiOptPoint", size_of::<OptPoint>(), align_of::<OptPoint>(), &[]);
        assert_layout("PxuiOptBar", size_of::<OptBar>(), align_of::<OptBar>(), &[]);
        assert_layout("PxuiOptCurve", size_of::<OptCurve>(), align_of::<OptCurve>(), &[]);
        assert_layout("PxuiOptColor", size_of::<OptColor>(), align_of::<OptColor>(), &[]);
        assert_layout("PxuiGrid", size_of::<Grid>(), align_of::<Grid>(), &[]);
        assert_layout("PxuiCell", size_of::<Cell>(), align_of::<Cell>(), &[offset_of!(Cell, col), offset_of!(Cell, row), offset_of!(Cell, col_span), offset_of!(Cell, row_span)]);
        assert_layout("PxuiPlacement", size_of::<Placement>(), align_of::<Placement>(), &[offset_of!(Placement, side), offset_of!(Placement, align), offset_of!(Placement, gap), offset_of!(Placement, margin), offset_of!(Placement, flip)]);
        assert_layout("PxuiAnchor", size_of::<Anchor>(), align_of::<Anchor>(), &[offset_of!(Anchor, rect), offset_of!(Anchor, placement)]);
        assert_layout("PxuiFont", size_of::<Font>(), align_of::<Font>(), &[offset_of!(Font, family), offset_of!(Font, size), offset_of!(Font, weight), offset_of!(Font, tracking), offset_of!(Font, caps)]);
        assert_layout("PxuiShadow", size_of::<Shadow>(), align_of::<Shadow>(), &[offset_of!(Shadow, color), offset_of!(Shadow, offset), offset_of!(Shadow, blur), offset_of!(Shadow, spread), offset_of!(Shadow, inner)]);
        assert_layout("PxuiRing", size_of::<Ring>(), align_of::<Ring>(), &[offset_of!(Ring, color), offset_of!(Ring, width), offset_of!(Ring, sides)]);
        assert_layout("PxuiLook", size_of::<Look>(), align_of::<Look>(), &[
            offset_of!(Look, fill), offset_of!(Look, gradient), offset_of!(Look, angle), offset_of!(Look, border), offset_of!(Look, border_w), offset_of!(Look, sides),
            offset_of!(Look, radius), offset_of!(Look, shadows), offset_of!(Look, text), offset_of!(Look, font), offset_of!(Look, opacity), offset_of!(Look, ring),
            offset_of!(Look, selection), offset_of!(Look, caret), offset_of!(Look, dashed),
        ]);
        assert_layout("PxuiState", size_of::<State>(), align_of::<State>(), &[offset_of!(State, hovered), offset_of!(State, pressed), offset_of!(State, focused), offset_of!(State, disabled), offset_of!(State, checked)]);
        assert_layout("PxuiBar", size_of::<Bar>(), align_of::<Bar>(), &[offset_of!(Bar, color), offset_of!(Bar, radius), offset_of!(Bar, width), offset_of!(Bar, gap), offset_of!(Bar, margin), offset_of!(Bar, whole)]);
        assert_layout("PxuiNode", size_of::<Node>(), align_of::<Node>(), &[
            offset_of!(Node, size), offset_of!(Node, axis), offset_of!(Node, justify), offset_of!(Node, align), offset_of!(Node, wrap), offset_of!(Node, role),
            offset_of!(Node, state), offset_of!(Node, z), offset_of!(Node, flags), offset_of!(Node, w), offset_of!(Node, h), offset_of!(Node, min_w),
            offset_of!(Node, min_h), offset_of!(Node, max_w), offset_of!(Node, max_h), offset_of!(Node, grow), offset_of!(Node, shrink), offset_of!(Node, aspect),
            offset_of!(Node, padding), offset_of!(Node, margin), offset_of!(Node, gap), offset_of!(Node, row_gap), offset_of!(Node, grid), offset_of!(Node, cell),
            offset_of!(Node, align_self), offset_of!(Node, anchor), offset_of!(Node, a11y), offset_of!(Node, at), offset_of!(Node, scroll_step), offset_of!(Node, look),
            offset_of!(Node, inset), offset_of!(Node, mark), offset_of!(Node, bar), offset_of!(Node, transition),
        ]);
        assert_layout("PxuiMotion", size_of::<Motion>(), align_of::<Motion>(), &[offset_of!(Motion, fast), offset_of!(Motion, normal), offset_of!(Motion, slow), offset_of!(Motion, ease)]);
        assert_layout("PxuiTokens", size_of::<Tokens>(), align_of::<Tokens>(), &[
            offset_of!(Tokens, panel), offset_of!(Tokens, foreground), offset_of!(Tokens, control), offset_of!(Tokens, input), offset_of!(Tokens, track),
            offset_of!(Tokens, accent), offset_of!(Tokens, invalid), offset_of!(Tokens, ink_ground), offset_of!(Tokens, space), offset_of!(Tokens, radius),
            offset_of!(Tokens, display_size), offset_of!(Tokens, title_size), offset_of!(Tokens, font_size), offset_of!(Tokens, label_size), offset_of!(Tokens, motion),
            offset_of!(Tokens, reduced_motion), offset_of!(Tokens, density),
        ]);
        assert_layout("PxuiStates", size_of::<States>(), align_of::<States>(), &[offset_of!(States, idle), offset_of!(States, hover), offset_of!(States, pressed), offset_of!(States, focus), offset_of!(States, disabled), offset_of!(States, checked)]);
        // the kit parts follow `ring`, `scrollbar_w` and `scrollbar_gap` back to back, in `theme_widgets!` order, then the bodies
        assert_layout("PxuiThemeSnapshot", size_of::<ThemeSnapshot>(), align_of::<ThemeSnapshot>(), &[
            offset_of!(ThemeSnapshot, tokens), offset_of!(ThemeSnapshot, row_h), offset_of!(ThemeSnapshot, control_h), offset_of!(ThemeSnapshot, ring),
            offset_of!(ThemeSnapshot, scrollbar_w), offset_of!(ThemeSnapshot, scrollbar_gap), offset_of!(ThemeSnapshot, panel), offset_of!(ThemeSnapshot, body),
        ]);
        assert_eq!(offset_of!(ThemeSnapshot, body), offset_of!(ThemeSnapshot, panel) + WIDGET_COUNT * size_of::<States>(), "the parts are an array of States, then the bodies");
    }

    #[test]
    fn frozen_layout_v1() {
        assert_layout(
            "Point",
            size_of::<geom::Point>(),
            align_of::<geom::Point>(),
            &[offset_of!(geom::Point, x), offset_of!(geom::Point, y)],
        );
        assert_layout(
            "Rect",
            size_of::<geom::Rect>(),
            align_of::<geom::Rect>(),
            &[
                offset_of!(geom::Rect, x),
                offset_of!(geom::Rect, y),
                offset_of!(geom::Rect, w),
                offset_of!(geom::Rect, h),
            ],
        );
        assert_layout(
            "Edges",
            size_of::<geom::Edges>(),
            align_of::<geom::Edges>(),
            &[
                offset_of!(geom::Edges, l),
                offset_of!(geom::Edges, t),
                offset_of!(geom::Edges, r),
                offset_of!(geom::Edges, b),
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
                offset_of!(geom::Rgba, r),
                offset_of!(geom::Rgba, g),
                offset_of!(geom::Rgba, b),
                offset_of!(geom::Rgba, a),
            ],
        );
        assert_layout(
            "Instance",
            size_of::<draw::Instance>(),
            align_of::<draw::Instance>(),
            &[
                offset_of!(draw::Instance, rect),
                offset_of!(draw::Instance, clip),
                offset_of!(draw::Instance, color),
                offset_of!(draw::Instance, color2),
                offset_of!(draw::Instance, aux),
                offset_of!(draw::Instance, radii),
                offset_of!(draw::Instance, width),
                offset_of!(draw::Instance, kind),
                offset_of!(draw::Instance, flags),
                offset_of!(draw::Instance, view),
                offset_of!(draw::Instance, clip_radius),
                offset_of!(draw::Instance, extra),
            ],
        );
        assert_layout(
            "View",
            size_of::<View>(),
            align_of::<View>(),
            &[
                offset_of!(View, offset),
                offset_of!(View, opacity),
                offset_of!(View, clip),
                offset_of!(View, scale),
                offset_of!(View, transform),
                offset_of!(View, pivot),
            ],
        );
        assert_layout(
            "Str",
            size_of::<Str>(),
            align_of::<Str>(),
            &[offset_of!(Str, ptr), offset_of!(Str, len)],
        );
        assert_layout(
            "Event",
            size_of::<Event>(),
            align_of::<Event>(),
            &[
                offset_of!(Event, kind),
                offset_of!(Event, code),
                offset_of!(Event, x),
                offset_of!(Event, y),
                offset_of!(Event, value),
                offset_of!(Event, text),
            ],
        );
        assert_layout(
            "FrameIn",
            size_of::<FrameIn>(),
            align_of::<FrameIn>(),
            &[
                offset_of!(FrameIn, size),
                offset_of!(FrameIn, events),
                offset_of!(FrameIn, event_count),
                offset_of!(FrameIn, width),
                offset_of!(FrameIn, height),
                offset_of!(FrameIn, scale),
                offset_of!(FrameIn, time),
            ],
        );
        assert_layout(
            "FrameOut",
            size_of::<FrameOut>(),
            align_of::<FrameOut>(),
            &[
                offset_of!(FrameOut, size),
                offset_of!(FrameOut, damage),
                offset_of!(FrameOut, has_damage),
                offset_of!(FrameOut, reserved),
                offset_of!(FrameOut, layer_count),
                offset_of!(FrameOut, layers),
            ],
        );
        assert_layout(
            "Gpu",
            size_of::<Gpu>(),
            align_of::<Gpu>(),
            &[
                offset_of!(Gpu, size),
                offset_of!(Gpu, backend),
                offset_of!(Gpu, instance),
                offset_of!(Gpu, physical_device),
                offset_of!(Gpu, device),
                offset_of!(Gpu, queue),
                offset_of!(Gpu, queue_family),
                offset_of!(Gpu, api_version),
                offset_of!(Gpu, features),
                offset_of!(Gpu, get_instance_proc_addr),
                offset_of!(Gpu, pipeline_cache),
                offset_of!(Gpu, mtl_device),
                offset_of!(Gpu, wl_display),
                offset_of!(Gpu, wl_surface),
                offset_of!(Gpu, metal_layer),
                offset_of!(Gpu, hwnd),
                offset_of!(Gpu, owner_token),
                offset_of!(Gpu, release_owner),
            ],
        );
        assert_layout(
            "Commands",
            size_of::<Commands>(),
            align_of::<Commands>(),
            &[
                offset_of!(Commands, size),
                offset_of!(Commands, vk_command_buffer),
                offset_of!(Commands, mtl_command_buffer),
            ],
        );
        assert_layout(
            "Pass",
            size_of::<Pass>(),
            align_of::<Pass>(),
            &[
                offset_of!(Pass, size),
                offset_of!(Pass, vk_command_buffer),
                offset_of!(Pass, mtl_render_encoder),
                offset_of!(Pass, width_px),
                offset_of!(Pass, height_px),
                offset_of!(Pass, scale),
                offset_of!(Pass, mtl_command_buffer),
                offset_of!(Pass, first_layer),
                offset_of!(Pass, layer_count),
                offset_of!(Pass, split),
                offset_of!(Pass, split_user),
            ],
        );
        assert_layout(
            "Image",
            size_of::<Image>(),
            align_of::<Image>(),
            &[
                offset_of!(Image, size),
                offset_of!(Image, vk_image_view),
                offset_of!(Image, mtl_texture),
                offset_of!(Image, width),
                offset_of!(Image, height),
                offset_of!(Image, format),
            ],
        );
        assert_layout(
            "Span",
            size_of::<Span>(),
            align_of::<Span>(),
            &[
                offset_of!(Span, size),
                offset_of!(Span, view),
                offset_of!(Span, clip_radius_f16),
                offset_of!(Span, clip),
                offset_of!(Span, at),
                offset_of!(Span, end),
            ],
        );
    }
}
