//! C-compatible frame, platform, accessibility, and theme exchange records.
use crate::{Event, PxuiBox, Str, geom::{Edges, Rect, Rgba}};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Slice<T> { pub ptr: *const T, pub len: usize }
impl<T> Default for Slice<T> { fn default() -> Self { Self { ptr: core::ptr::null(), len: 0 } } }

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub size: u32,
    pub time: f64,
    pub dt: f64,
    pub frame: i64,
    pub tick: i64,
    pub logical_size: [i64; 2],
    pub pointer: [f64; 2],
    pub keys: Slice<u32>,
    pub buttons: Slice<u32>,
    pub events: Slice<Event>,
    pub pixel_scale: [f64; 2],
    pub mouse_delta: [f64; 2],
    pub fps: f64,
}

impl Default for Input {
    fn default() -> Self {
        Self { size: core::mem::size_of::<Self>() as u32, time: 0.0, dt: 0.0, frame: 0, tick: 0, logical_size: [0; 2], pointer: [0.0; 2],
            keys: Slice::default(), buttons: Slice::default(), events: Slice::default(), pixel_scale: [1.0; 2], mouse_delta: [0.0; 2], fps: 60.0 }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PlatformOut {
    pub size: u32,
    /// 0 default, 1 horizontal resize, 2 vertical resize, 3 text
    pub cursor: u32,
    pub has_cursor: u8,
    pub has_ime: u8,
    pub has_background: u8,
    pub has_clipboard: u8,
    pub ime_rect: [i64; 4],
    pub ime_caret: i64,
    pub clipboard: Str,
    pub background: Rgba,
    pub paste: u8,
    pub reserved: [u8; 7],
}
impl Default for PlatformOut {
    fn default() -> Self {
        Self { size: 0, cursor: 0, has_cursor: 0, has_ime: 0, has_background: 0, has_clipboard: 0,
            ime_rect: [0; 4], ime_caret: 0, clipboard: Str::default(), background: Rgba { r: 0, g: 0, b: 0, a: 0 },
            paste: 0, reserved: [0; 7] }
    }
}

/// Instances `start..end` of a layer changed since it was last drawn.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PxuiDirty {
    pub start: u32,
    pub end: u32,
}

/// One layer of the last frame (a floating root's, drawn in order): the key of its root box, its retained id, its instances (an
/// empty layer draws nothing), how many texture breaks they hold and the instance ranges changed since it was last drawn. Borrowed
/// until the next frame.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiLayerInfo {
    pub key: i64,
    pub id: u32,
    pub breaks: u32,
    pub instances: Slice<crate::draw::Instance>,
    pub dirty: Slice<PxuiDirty>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Damage {
    pub has_damage: u8,
    pub reserved: [u8; 3],
    pub rect: Rect,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct A11yValue {
    /// 0 none, 1 text range, 2 number
    pub tag: u32,
    pub text_start: u32,
    pub text_end: u32,
    pub reserved: u32,
    pub now: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

impl A11yNode {
    /// Whether the node has `role`.
    #[inline] pub fn is_role(&self, role: crate::a11y::Role) -> bool { self.role == role as u32 }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct A11yNode {
    pub id: u64,
    pub parent: u32,
    pub role: u32,
    pub name_start: u32,
    pub name_end: u32,
    pub description_start: u32,
    pub description_end: u32,
    pub value: A11yValue,
    pub rect: Rect,
    pub states: u32,
    pub actions: u32,
    /// Anchor and caret in Unicode scalar values, as in the engine's text widgets and AT-SPI.
    pub selection: [u32; 2],
    pub child_count: u32,
    pub first_child: u32,
    pub next_sibling: u32,
    pub last_child: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct A11yAction {
    pub id: u64,
    pub action_code: u32,
    pub reserved: u32,
    pub payload: f64,
    pub end: u32,
    pub reserved_tail: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct A11yTree {
    pub nodes: Slice<A11yNode>,
    pub strings: Str,
    pub revision: u64,
    pub first_child: u32,
    pub child_count: u32,
    pub actions: Slice<A11yAction>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Output {
    pub size: u32,
    pub reserved: u32,
    pub draw_revision: u64,
    /// Positive infinity means no scheduled repaint.
    pub repaint_at: f64,
    pub built_nodes: u64,
    pub rebuilt_views: u64,
    pub arranged_nodes: u64,
    pub platform: PlatformOut,
    pub damage: Damage,
    pub a11y: A11yTree,
    /// the last frame's layers in draw order, borrowed until the next frame
    pub layers: Slice<PxuiLayerInfo>,
}
impl Default for Output {
    fn default() -> Self {
        Self { size: core::mem::size_of::<Self>() as u32, reserved: 0, draw_revision: 0, repaint_at: f64::INFINITY, built_nodes: 0, rebuilt_views: 0, arranged_nodes: 0,
            platform: PlatformOut { size: core::mem::size_of::<PlatformOut>() as u32, ..PlatformOut::default() }, damage: Damage::default(),
            a11y: A11yTree { nodes: Slice::default(), strings: Str::default(), revision: 0, first_child: u32::MAX, child_count: 0, actions: Slice::default() }, layers: Slice::default() }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ThemeSource {
    pub size: u32,
    pub flags: u32,
    pub source: Str,
}
impl Default for ThemeSource { fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, flags: 0, source: Str::default() } } }

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiMotion {
    pub fast: f32,
    pub normal: f32,
    pub slow: f32,
    pub ease: [f32; 4],
}

/// Stable snapshot of Gallery paint tokens; independent of the private theme graph.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ThemeTokens {
    pub size: u32,
    pub panel: Rgba,
    pub foreground: Rgba,
    pub control: Rgba,
    pub input: Rgba,
    pub track: Rgba,
    pub accent: Rgba,
    pub ink_ground: Rgba,
    pub has_ink_ground: u8,
    pub reserved: [u8; 3],
    pub space: [f32; 4],
    pub radius: [f32; 3],
    pub display_size: i64,
    pub title_size: i64,
    pub font_size: i64,
    pub label_size: i64,
    pub motion: PxuiMotion,
    pub reduced_motion: u8,
    pub reserved_tail: [u8; 3],
    pub density: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiDialogOut {
    pub tag: u32,
    pub index: usize,
}

/// The fields of a declared node an app edits in place after its signal is known (`Ui::node_mut`): its look, its mark and the
/// inset of its text.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiNodeFields {
    pub size: u32,
    pub reserved: u32,
    pub look: PxuiLookDesc,
    pub mark: PxuiMarkDesc,
    pub inset: Edges,
}

/// A byte range `start..end` of a text.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiRange {
    pub start: i64,
    pub end: i64,
}

/// A byte range with a colour: a syntax span or a chip.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiColorSpan {
    pub start: i64,
    pub end: i64,
    pub color: Rgba,
    pub reserved: u32,
}

/// A message under a line of a text area, optionally pointing at a byte range of it.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiNote {
    pub line: i64,
    pub start: i64,
    pub end: i64,
    pub has_range: u8,
    pub reserved: [u8; 7],
    pub text: Str,
}

/// A replacement of bytes `start..end` of the text with `text`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiSplice {
    pub start: i64,
    pub end: i64,
    pub text: Str,
}

/// One suggestion of a language's `complete`: the byte range `insert` replaces, the row, its detail and its doc line.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiCompletion {
    pub start: i64,
    pub end: i64,
    pub insert: Str,
    pub label: Str,
    pub detail: Str,
    pub doc: Str,
}

/// A token's byte range and its description (`has` is 0 for none).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiDescription {
    pub has: u8,
    pub reserved: [u8; 7],
    pub start: i64,
    pub end: i64,
    pub text: Str,
}

/// An optional byte range (`has` is 0 for none).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiMaybeRange {
    pub has: u8,
    pub reserved: [u8; 7],
    pub start: i64,
    pub end: i64,
}

/// What a language's `rewrite` returns: the text it keeps and where the caret lands in it.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiRewrite {
    pub text: Str,
    pub caret: i64,
}

/// A text area (`TextAreaOpts`): its place, its text and the data options. `flags` are the `PxuiTextAreaDesc::*` bits.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiTextAreaDesc {
    pub size: u32,
    pub flags: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub reveal: i64,
    pub reveal_nonce: i64,
    pub select: PxuiRange,
    pub label: Str,
    pub value: Str,
    pub errors: Slice<i64>,
    pub messages: Slice<PxuiNote>,
    pub spans: Slice<PxuiRange>,
    pub splice: Slice<PxuiSplice>,
    pub chips: Slice<PxuiColorSpan>,
    /// what starts a line comment (the language's `comment`)
    pub comment: Str,
    /// the language's bracket pairs, opener then closer, as Unicode scalar values
    pub pairs: Slice<u32>,
}
impl PxuiTextAreaDesc {
    pub const WRAP: u32 = 1;
    pub const HAS_REVEAL: u32 = 2;
    pub const HAS_SELECT: u32 = 4;
    pub const HAS_LANGUAGE: u32 = 8;
    pub const HAS_REWRITE: u32 = 16;
    pub const KNOWN: u32 = 31;
}

pub type AreaNotify = Option<extern "C" fn(*mut core::ffi::c_void, i64, i64)>;
/// The app's closures of a text area (`TextAreaOpts::on_*` and `Language`), called only during the widget's call with
/// `user`. A callback that returns data writes it to its `out` (borrowed until the next callback). `colorize` and the
/// language members are all set or all null (`rewrite` alone may be null).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiAreaCallbacks {
    pub size: u32,
    pub reserved: u32,
    pub user: *mut core::ffi::c_void,
    pub on_context: Option<extern "C" fn(*mut core::ffi::c_void, f32, f32)>,
    pub on_scrub: Option<extern "C" fn(*mut core::ffi::c_void, u32)>,
    pub on_scrub_edit: Option<extern "C" fn(*mut core::ffi::c_void, i64, i64, Str)>,
    pub on_click: Option<extern "C" fn(*mut core::ffi::c_void, i64, u8)>,
    pub on_caret: Option<extern "C" fn(*mut core::ffi::c_void, i64)>,
    pub on_caret_move: Option<extern "C" fn(*mut core::ffi::c_void, i64)>,
    pub on_select: AreaNotify,
    pub on_drop: Option<extern "C" fn(*mut core::ffi::c_void, i64, u8, Str, Str)>,
    pub colorize: Option<extern "C" fn(*mut core::ffi::c_void, Str, *mut Slice<PxuiColorSpan>)>,
    pub brackets: Option<extern "C" fn(*mut core::ffi::c_void, Str, *mut Slice<PxuiRange>)>,
    pub indent: Option<extern "C" fn(*mut core::ffi::c_void, Str, i64, *mut Str)>,
    pub complete: Option<extern "C" fn(*mut core::ffi::c_void, Str, i64, *mut Slice<PxuiCompletion>)>,
    pub describe: Option<extern "C" fn(*mut core::ffi::c_void, Str, i64, *mut PxuiDescription)>,
    pub number_at: Option<extern "C" fn(*mut core::ffi::c_void, Str, i64, *mut PxuiMaybeRange)>,
    pub context_at: Option<extern "C" fn(*mut core::ffi::c_void, Str, i64) -> u32>,
    pub rewrite: Option<extern "C" fn(*mut core::ffi::c_void, Str, i64, *mut PxuiRewrite)>,
}

/// A menu row's colour swatch: none while `has` is 0.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiSwatch {
    pub has: u8,
    pub reserved: [u8; 3],
    pub color: Rgba,
}

/// A submenu of a context menu: the row of the menu that opens it, its rows (`enabled` is as long as `rows`), their keys
/// and the row marked with the accent square (when `has_current`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiSubmenu {
    pub row: i64,
    pub current: i64,
    pub has_current: u8,
    pub reserved: [u8; 7],
    pub rows: Slice<Str>,
    pub enabled: Slice<u8>,
    pub keys: Slice<Str>,
}

/// What a menu has beyond its rows (`Ui::menu`): keys and swatches are per row (shorter than the rows is fine),
/// `danger` the destructive rows, `lead_from` the first row that is not drawn with a leading mark. From ABI 1.49 (a `size` that
/// covers them): `row_h` the row height in points (0: the theme's), `max_rows` the rows shown before the column scrolls (0: all), and `checked` per row 0 no mark slot, 1 a slot, 2 a slot with
/// the mark, which replaces `has_selected`/`selected` and `lead_from` when it is not empty.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiMenuExtra {
    pub size: u32,
    pub reserved: u32,
    pub lead_from: i64,
    pub keys: Slice<Str>,
    pub swatches: Slice<PxuiSwatch>,
    pub danger: Slice<i64>,
    pub submenus: Slice<PxuiSubmenu>,
    pub row_h: f32,
    pub max_rows: u32,
    pub checked: Slice<u8>,
}

/// A label's style as it crosses the ABI (`pxui_ui_label`, `pxui_paint_label`, `PxuiLabelConfig::style`; ABI 1.45): `Label` with
/// each optional field's bit in `set` (`SET_*`); a field whose bit is clear is the theme's label style. `color` is an app colour
/// token; `digits` 1 draws the text one cached character at a time with every digit on the advance of `0`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiLabelStyle {
    pub size: u32,
    pub set: u32,
    pub font: u16,
    pub weight: u16,
    pub color: u16,
    pub max_lines: u16,
    pub caps: u8,
    pub digits: u8,
    pub ellipsis: u8,
    pub reserved: u8,
    pub size_points: f32,
    pub tracking: f32,
    pub line_height: f32,
    pub pad: f32,
    pub wrap: u32,
    pub align: u32,
    pub features: u32,
    pub width: PxuiSizeDesc,
}

impl PxuiLabelStyle {
    pub const SET_FONT: u32 = 1;
    pub const SET_SIZE: u32 = 2;
    pub const SET_WEIGHT: u32 = 4;
    pub const SET_TRACKING: u32 = 8;
    pub const SET_CAPS: u32 = 16;
    pub const SET_COLOR: u32 = 32;
    pub const SET_WIDTH: u32 = 64;
    pub const SET_LINE_HEIGHT: u32 = 128;
    pub const SET_PAD: u32 = 256;
    /// Every bit a 1.45 library knows.
    pub const SET_ALL: u32 = 511;
}

impl From<&crate::Label> for PxuiLabelStyle {
    /// The record of `label`, with `digits` 0.
    fn from(l: &crate::Label) -> PxuiLabelStyle {
        let bit = |on: bool, b: u32| if on { b } else { 0 };
        let set = bit(l.font.is_some(), Self::SET_FONT) | bit(l.size.is_some(), Self::SET_SIZE) | bit(l.weight.is_some(), Self::SET_WEIGHT)
            | bit(l.tracking.is_some(), Self::SET_TRACKING) | bit(l.caps.is_some(), Self::SET_CAPS) | bit(l.color.is_some(), Self::SET_COLOR)
            | bit(l.width.is_some(), Self::SET_WIDTH) | bit(l.line_height.is_some(), Self::SET_LINE_HEIGHT) | bit(l.pad.is_some(), Self::SET_PAD);
        let width = match l.width {
            Some(crate::Size::Px(value)) => PxuiSizeDesc { kind: PxuiSizeDesc::PX, value, relative: None },
            Some(crate::Size::Pct(value)) => PxuiSizeDesc { kind: PxuiSizeDesc::PCT, value, relative: None },
            Some(crate::Size::Rel(relative)) => PxuiSizeDesc { kind: PxuiSizeDesc::REL, value: 0.0, relative: Some(relative) },
            Some(crate::Size::Fit) => PxuiSizeDesc { kind: PxuiSizeDesc::FIT, value: 0.0, relative: None },
            Some(crate::Size::Grow) | None => PxuiSizeDesc { kind: PxuiSizeDesc::GROW, value: 0.0, relative: None },
        };
        PxuiLabelStyle {
            size: core::mem::size_of::<PxuiLabelStyle>() as u32, set,
            font: l.font.unwrap_or(0), weight: l.weight.unwrap_or(0), color: l.color.map_or(0, |c| c.0), max_lines: l.max_lines,
            caps: l.caps.unwrap_or(false) as u8, digits: 0, ellipsis: l.ellipsis, reserved: 0,
            size_points: l.size.unwrap_or(0.0), tracking: l.tracking.unwrap_or(0.0), line_height: l.line_height.unwrap_or(0.0), pad: l.pad.unwrap_or(0.0),
            wrap: l.wrap, align: l.align, features: l.features, width,
        }
    }
}

/// C mirror of `Size`; kinds are Px=0, Pct=1, Rel=2, Grow=3, Fit=4.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiSizeDesc {
    pub kind: u32,
    pub value: f32,
    pub relative: Option<extern "C" fn(f32) -> f32>,
}
impl PxuiSizeDesc {
    pub const PX: u32 = 0;
    pub const PCT: u32 = 1;
    pub const REL: u32 = 2;
    pub const GROW: u32 = 3;
    pub const FIT: u32 = 4;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiAnchorDesc {
    pub has_anchor: u8,
    pub reserved: [u8; 3],
    pub rect: Rect,
    pub side: u32,
    pub align: u32,
    pub gap: f32,
    pub margin: f32,
    pub flip: u8,
    pub reserved_tail: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiFontDesc {
    pub family: u16,
    pub weight: u16,
    pub size: i64,
    pub tracking: f32,
    pub caps: u8,
    pub reserved: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiShadowDesc {
    pub color: Rgba,
    pub offset: [f32; 2],
    pub blur: f32,
    pub spread: f32,
    pub inner: u8,
    pub reserved: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiRingDesc {
    pub color: Rgba,
    pub width: f32,
    pub sides: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiLookDesc {
    pub fill: Rgba,
    pub gradient: Rgba,
    pub has_gradient: u8,
    pub reserved: [u8; 3],
    pub angle: f32,
    pub border: Rgba,
    pub border_w: f32,
    pub sides: u32,
    pub radius: [f32; 4],
    pub shadows: [PxuiShadowDesc; 2],
    pub text: Rgba,
    pub font: PxuiFontDesc,
    pub opacity: f32,
    pub ring: PxuiRingDesc,
    pub selection: Rgba,
    pub caret: Rgba,
    pub dashed: u8,
    pub reserved_tail: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiMarkDesc {
    pub kind: u32,
    pub direction: u32,
    pub width: f32,
    pub rising: u8,
    pub reserved: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiStateDesc {
    pub hovered: u8,
    pub pressed: u8,
    pub focused: u8,
    pub disabled: u8,
    pub checked: u8,
    pub reserved: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiCurveDesc {
    pub kind: u32,
    pub duration: f32,
    pub points: [f32; 4],
    pub omega: f32,
    pub count: u32,
    pub positions: [f32; 16],
    pub values: [f32; 16],
}
impl PxuiCurveDesc {
    pub const NONE: u32 = 0;
    pub const BEZIER: u32 = 1;
    pub const SPRING: u32 = 2;
    pub const TABLE: u32 = 3;
    pub const STEPS: u32 = 4;
}

impl PxuiMarkDesc {
    pub const NONE: u32 = 0;
    pub const CHEVRON: u32 = 1;
    pub const LINE: u32 = 2;
    pub const DIR_DOWN: u32 = 0;
    pub const DIR_UP: u32 = 1;
    pub const DIR_LEFT: u32 = 2;
    pub const DIR_RIGHT: u32 = 3;
}

/// Opaque `enter` state: pass it back to `leave` unchanged.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiSaved {
    pub size: u32,
    pub reserved: u32,
    pub parents: usize,
    pub seeds: usize,
}

/// What `pxui_ui_signal` reports for one box. `keys` is borrowed from the `Ui` and valid until the next frame.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiSignal {
    pub size: u32,
    pub flags: u32,
    pub has_subtree_press: u8,
    pub button: u8,
    pub has_a11y_value: u8,
    pub has_a11y_select: u8,
    pub reserved: [u8; 4],
    pub subtree_press: i64,
    pub clicks: i64,
    pub drag: [f32; 2],
    pub pointer: [f32; 2],
    pub press_point: [f32; 2],
    pub release_point: [f32; 2],
    pub scroll: [f32; 2],
    pub pinch: f64,
    pub a11y_value: f64,
    pub a11y_select: [u32; 2],
    pub keys: Slice<Event>,
}

/// Result of a behavior-only scrubber; flag bits are preview, commit and held, respectively.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiScrubOut {
    pub size: u32,
    pub flags: u32,
    pub node: PxuiBox,
    pub preview: f32,
    pub commit: f32,
}
impl Default for PxuiScrubOut {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, flags: 0, node: PxuiBox::default(), preview: 0.0, commit: 0.0 } }
}
/// ABI 1.38 gesture request: `kinds` bits 1 long press, 2 row swipe, 4 edge back, 8 pull to refresh, 32 touch drag (a held press and drag, ABI 1.42);
/// `reveal` is the row-swipe action width and `threshold` the pull distance (0: 60), in points.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiGestureIn {
    pub size: u32,
    pub kinds: u32,
    pub reveal: f32,
    pub threshold: f32,
}
impl Default for PxuiGestureIn {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, kinds: 0, reveal: 0.0, threshold: 0.0 } }
}

/// ABI 1.40 row-swipe actions on both sides: `leading` and `trailing` are action widths in points (0 disables
/// a side; leading follows the layout direction), `full` the finger travel that commits the edge action on
/// release (0: no full swipe).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiSwipeIn {
    pub size: u32,
    pub leading: f32,
    pub trailing: f32,
    pub full: f32,
}
impl Default for PxuiSwipeIn {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, leading: 0.0, trailing: 0.0, full: 0.0 } }
}

/// ABI 1.41 drag-to-dismiss: `fraction` of the node height (0 picks 0.25) and a downward release speed in points
/// per second (0 picks 600) commit on release.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiDismissIn {
    pub size: u32,
    pub fraction: f32,
    pub velocity: f32,
}
impl Default for PxuiDismissIn {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, fraction: 0.0, velocity: 0.0 } }
}

/// ABI 1.41 dismiss state: flags 8192 dragging or settling, 16384 committed and 32768 cancelled (this frame), and
/// the downward offset in points to translate the node by.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiDismissOut {
    pub size: u32,
    pub flags: u32,
    pub offset: f32,
}
impl Default for PxuiDismissOut {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, flags: 0, offset: 0.0 } }
}

/// ABI 1.38 gesture state: flag bits, the signed shift of a swiped row's content, back progress
/// 0..1 and the pull-down in points.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiGestureOut {
    pub size: u32,
    pub flags: u32,
    pub swipe: f32,
    pub back: f32,
    pub pull: f32,
}
impl Default for PxuiGestureOut {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, flags: 0, swipe: 0.0, back: 0.0, pull: 0.0 } }
}
impl Default for PxuiSignal {
    fn default() -> Self {
        Self { size: core::mem::size_of::<Self>() as u32, flags: 0, has_subtree_press: 0, button: u8::MAX, has_a11y_value: 0, has_a11y_select: 0,
            reserved: [0; 4], subtree_press: 0, clicks: 0, drag: [0.0; 2], pointer: [0.0; 2], press_point: [0.0; 2],
            release_point: [0.0; 2], scroll: [0.0; 2], pinch: 1.0, a11y_value: 0.0, a11y_select: [0; 2], keys: Slice::default() }
    }
}
impl PxuiSignal {
    pub const HOVERED: u32 = 1;
    pub const PRESSED: u32 = 2;
    pub const HELD: u32 = 4;
    pub const RELEASED: u32 = 8;
    pub const CLICKED: u32 = 16;
    pub const DOUBLE_CLICKED: u32 = 32;
    pub const DRAGGING: u32 = 64;
}

pub type PxuiInput = Input;
pub type PxuiOutput = Output;
pub type PxuiPlatformOut = PlatformOut;
pub type PxuiDamage = Damage;
pub type PxuiA11yValue = A11yValue;
pub type PxuiA11yNode = A11yNode;
pub type PxuiA11yAction = A11yAction;
/// A span of actions, as `pxui_a11y_pump` hands them out.
pub type A11yActionSpan = Slice<A11yAction>;
pub type PxuiA11yTree = A11yTree;
pub type PxuiThemeSource = ThemeSource;
pub type PxuiThemeTokens = ThemeTokens;

/// What a paint callback needs to know about its painter, read once per callback: the sizes, the clip and the theme tokens.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiPaintInfo {
    pub size: u32,
    pub rtl: u8,
    pub reserved: [u8; 3],
    pub font_size: i64,
    pub row_height: i64,
    pub density: i64,
    pub clip: Rect,
    pub clip_radius: f32,
    pub theme: ThemeTokens,
}

/// `value_field`'s place, text and options. Each `HAS_*` flag says whether the option next to it is set; `lead` and `trail` are
/// the framing texts, coloured by `lead_color` and `trail_color`. The slide, scrub and validity callbacks cross beside it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiValueFieldDesc {
    pub size: u32,
    pub flags: u32,
    pub font_size: i64,
    pub fraction: f32,
    pub tracking: f32,
    pub line: Rgba,
    pub lead_color: Rgba,
    pub trail_color: Rgba,
    pub reserved: [u8; 4],
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub display: Str,
    pub placeholder: Str,
    pub lead: Str,
    pub trail: Str,
    pub label: Str,
    pub value: Str,
}

impl PxuiValueFieldDesc {
    pub const LEFT: u32 = 1;
    pub const EDIT: u32 = 2;
    pub const BARE: u32 = 4;
    pub const KEPT_DRAG: u32 = 8;
    pub const HAS_SIZE: u32 = 16;
    pub const HAS_FRACTION: u32 = 32;
    pub const HAS_TRACKING: u32 = 64;
    pub const HAS_LINE: u32 = 128;
    pub const HAS_LEAD: u32 = 256;
    pub const HAS_TRAIL: u32 = 512;
    pub const HAS_DISPLAY: u32 = 1024;
    pub const HAS_PLACEHOLDER: u32 = 2048;
    pub const KNOWN: u32 = 4095;
}

/// A popup sheet: `popup`'s plain arguments (its build callback crosses beside this record).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiPopupDesc {
    pub size: u32,
    pub has_stroke: u8,
    pub has_max_height: u8,
    pub dismiss_initial: u8,
    pub attached: u8,
    pub stroke: Rgba,
    pub max_height: f32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub label: Str,
}

/// A kit button with options (`button_with`): its arguments beside the build callback that replaces its text. Kinds 0, 1, 2 are
/// the kit's size, `Px` and `Grow`; `justify` 0, 1, 2 is start, centre, end; `states` null is the theme's button, else six looks (idle, hover, pressed, focus, disabled, checked) as `pxui_ui_style` takes them.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiButtonDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// 0 or 1: the button takes input (0 shows the disabled look and blocks presses and focus).
    pub enabled: u8,
    /// 0 or 1: the current one of a group (the checked look, accessible SELECTED).
    pub selected: u8,
    /// 0 or 1: the text in the theme's invalid colour.
    pub danger: u8,
    /// Placement in its line: 0 start, 1 centre, 2 end.
    pub justify: u8,
    /// Width kind: 0 the kit's, 1 `w` points, 2 fill.
    pub w_kind: u8,
    /// Height kind: 0 the kit's, 1 `h` points, 2 fill.
    pub h_kind: u8,
    /// 0 or 1: `name` is the accessible name (else `text`).
    pub has_name: u8,
    /// Zero.
    pub reserved: u8,
    /// Width in points when `w_kind` is 1.
    pub w: f32,
    /// Height in points when `h_kind` is 1.
    pub h: f32,
    /// Extra padding inside the box: left, top, right, bottom, in points, finite and not negative.
    pub padding: [f32; 4],
    /// Null for the theme's button, else six looks: idle, hover, pressed, focus, disabled, checked.
    pub states: *const PxuiLookDesc,
    /// The node's key, hashed under the parent's seed.
    pub key: Str,
    /// The button's text (empty when a build callback draws its contents).
    pub text: Str,
    /// The accessible name when `has_name` is 1.
    pub name: Str,
}

/// What happened to a kit button or icon button this frame, in one record (ABI 1.48, `pxui_ui_button_plain`,
/// `pxui_ui_button_response`, `pxui_ui_icon_button_plain`, `pxui_ui_icon_button_response`): the `CLICKED`, `HOVERED`, `HELD`,
/// `FOCUSED` and `ACTIVATED` bits and the button's box. The caller sets `size`; a shorter record is refused.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiResponse {
    /// Size of this record in bytes.
    pub size: u32,
    /// The `PxuiResponse::*` bits.
    pub flags: u32,
    /// The button's box (the node a press lands on).
    pub node: PxuiBox,
}

impl PxuiResponse {
    /// A press and release inside the box this frame, or Enter/Space while it is focused.
    pub const CLICKED: u32 = 1;
    /// The pointer is over the box.
    pub const HOVERED: u32 = 2;
    /// The box holds the pointer's capture.
    pub const HELD: u32 = 4;
    /// The box has the keyboard focus.
    pub const FOCUSED: u32 = 8;
    /// The click came from the keyboard (implies `CLICKED`).
    pub const ACTIVATED: u32 = 16;
    /// An empty record of this size, for a call to fill.
    pub const EMPTY: PxuiResponse = PxuiResponse { size: core::mem::size_of::<PxuiResponse>() as u32, flags: 0, node: PxuiBox { index: 0, box_key: 0, box_slot: 0, generation: 0 } };
}

/// An icon drawing callback: `(user, paint, rect, ink, info)`. `paint` is the `PxuiPaint` of a paint callback and `info` is valid for the call.
pub type PxuiIconDraw = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, Rect, Rgba, *const PxuiPaintInfo)>;

/// An icon button with options (`icon_button`): kinds 0, 1, 2 are the kit's square, `Px` and `Grow`; `states` null is the theme's
/// `icon_button`, else six looks (idle, hover, pressed, focus, disabled, checked) as `pxui_ui_style` takes them.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiIconButtonDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// 0 or 1: the button takes input (0 shows the disabled look and blocks presses and focus).
    pub enabled: u8,
    /// Width kind: 0 the kit's, 1 `w` points, 2 fill.
    pub w_kind: u8,
    /// Height kind: 0 the kit's, 1 `h` points, 2 fill.
    pub h_kind: u8,
    /// Zero.
    pub reserved: u8,
    /// Width in points when `w_kind` is 1.
    pub w: f32,
    /// Height in points when `h_kind` is 1.
    pub h: f32,
    /// The scale the contents spring to while pressed: finite and above zero (1.0: none).
    pub press_scale: f32,
    /// Null for the theme's icon button, else six looks: idle, hover, pressed, focus, disabled, checked.
    pub states: *const PxuiLookDesc,
    /// The node's key, hashed under the parent's seed.
    pub key: Str,
    /// The accessible name.
    pub name: Str,
    /// The glyph the button draws (ignored when `draw` is set).
    pub icon: Str,
    /// The app's drawing of the icon, or null for the glyph. Called at paint time with `draw_user`, the painter, the 20-point
    /// square to draw in (centred in the plate) and the look's text colour for the button's state.
    pub draw: PxuiIconDraw,
    /// Passed to `draw`; not owned: it must stay valid for as long as the frame can paint.
    pub draw_user: *mut core::ffi::c_void,
}

/// A kit toggle with options (`toggle_response`). `row_h` 0 is the kit's row height; `switch_w` and `switch_h` 0 are the kit's 28 x 14;
/// `inset` 0 is the kit's distance of the label and the switch from the row's sides; `detail` is a secondary line under the label
/// (empty: none); `track` and `knob` null are the theme's, else six looks each (idle, hover, pressed, focus, disabled, checked).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiToggleDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// 0 or 1: the toggle takes input (0 shows the disabled look and blocks presses and focus).
    pub enabled: u8,
    /// 0 or 1: the value now; a click is reported and the caller keeps the value.
    pub on: u8,
    /// 0 or 1: a press anywhere on the row toggles it (else only on the switch).
    pub press_row: u8,
    /// Zero.
    pub reserved: u8,
    /// The row's height in points (finite, not negative).
    pub row_h: f32,
    /// The switch's width in points (finite, not negative).
    pub switch_w: f32,
    /// The switch's height in points (finite, not negative).
    pub switch_h: f32,
    /// The distance of the label and the switch from the row's sides in points (finite, not negative).
    pub inset: f32,
    /// Null for the theme's switch, else six looks.
    pub track: *const PxuiLookDesc,
    /// Null for the theme's knob, else six looks.
    pub knob: *const PxuiLookDesc,
    /// The node's key, hashed under the parent's seed.
    pub key: Str,
    /// The row's text and accessible name.
    pub text: Str,
    /// The secondary line under the text.
    pub detail: Str,
}

/// A kit segmented control with options (`segmented_response`); the options go beside it as a slice of strings. `h` 0 is the theme's control
/// height; `states` null is the theme's segment, else six looks (idle, hover, pressed, focus, disabled, checked).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiSegmentedDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// 0 or 1: the strip takes input.
    pub enabled: u8,
    /// 0 or 1: the strip fills its line and its segments share its width.
    pub grow: u8,
    /// Zero.
    pub reserved: [u8; 2],
    /// The strip's height in points (finite, not negative).
    pub h: f32,
    /// Null for the theme's segment, else six looks.
    pub states: *const PxuiLookDesc,
    /// The node's key, hashed under the parent's seed.
    pub key: Str,
    /// The strip's accessible name.
    pub name: Str,
    /// The current choice (clamped to the options).
    pub selected: usize,
}

/// One text button of an inspector head.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiHeadActionDesc {
    pub size: u32,
    pub active: u8,
    pub usable: u8,
    pub reserved: [u8; 2],
    pub caption: Str,
    pub keycap: Str,
}

/// An inspector head. `kind`, `badge`, `index` and `reset` are read when their flag is set.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiHeaderDesc {
    pub size: u32,
    pub has_kind: u8,
    pub has_badge: u8,
    pub has_index: u8,
    pub has_reset: u8,
    pub reset_enabled: u8,
    pub reserved: [u8; 3],
    pub key: Str,
    pub kind: Str,
    pub badge: Str,
    pub index: Str,
    pub reset: Str,
    pub title: Str,
    pub detail: Str,
    pub actions: Slice<PxuiHeadActionDesc>,
}

/// What a head reports: the name as edited (borrowed until the next frame), the pressed action (`chosen` when `has_chosen`)
/// and whether `reset` was pressed.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiHeadOut {
    pub size: u32,
    pub has_chosen: u8,
    pub reset_pressed: u8,
    pub reserved: [u8; 2],
    pub renamed: Str,
    pub chosen: i64,
}

/// `BoxOpts` crossing: `axis` is 0 row, 1 column, 2 stack. `hit` (with `hit_user` and `hit_release`, called once when the
/// box's slot lets go of it) replaces the box's hit rectangle by the one it returns for the laid-out rectangle.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiBoxDesc {
    pub size: u32,
    pub axis: u32,
    pub flags: i64,
    pub w: PxuiSizeDesc,
    pub h: PxuiSizeDesc,
    pub max_h: f32,
    pub padding: f32,
    pub scroll_step: f32,
    pub has_at: u8,
    pub has_hit_rect: u8,
    pub reserved: [u8; 2],
    pub at: [f32; 2],
    pub hit_rect: Rect,
    pub hit: Option<extern "C" fn(*mut core::ffi::c_void, Rect) -> Rect>,
    pub hit_user: *mut core::ffi::c_void,
    pub hit_release: Option<extern "C" fn(*mut core::ffi::c_void)>,
}

impl ThemeSource {
    pub const COMPACT: u32 = 1;
    pub const REDUCED_MOTION: u32 = 2;
}


/// A build callback of a container descriptor: `(user, ui)`, the `ui` a borrowed `PxuiUi` valid for the call.
pub type UiBuild = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void)>;

// Descriptors of the kit widgets (`kit.rs` of `pxui_abi` and `pxui`): the one definition both sides use. Each starts with `size`,
// read before anything else: a descriptor passed alone may grow by appending fields; one in a span (menus, hints, head actions,
// keyframes) is read at its exact size, since a span's stride cannot grow.
pub type PxuiBuild = UiBuild;
pub type PxuiGridBuild = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, usize)>;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiMenuDesc {
    pub size: u32,
    pub reserved: u32,
    pub title: Str,
    pub items: Slice<Str>,
}
#[repr(C)]
pub struct PxuiGridListDesc {
    pub size: u32,
    pub reserved: u32,
    pub label: Str,
    pub count: usize,
    pub cell_w: f32,
    pub cell_h: f32,
    pub build: PxuiGridBuild,
    pub user: *mut core::ffi::c_void,
}

/// The callback receives usize::MAX for the lead-in, MAX-1 for the heading, and an index for rows.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiSectionListDesc {
    pub size:u32,
    pub overlay_scrollbar:u8,
    pub reserved:[u8;3],
    pub label:Str,
    pub count:usize,
    pub row_height:f32,
    pub leading_height:f32,
    pub header_height:f32,
    pub trailing_height:f32,
    pub build:PxuiGridBuild,
    pub user:*mut core::ffi::c_void,
    /// ABI 1.49: the item id of row `index` (`user` is the build's); row boxes are keyed by it. Absent or null: by index.
    pub key:PxuiRowKey,
}
/// The item id of a virtual row: `(user, index) -> id` (ABI 1.49).
pub type PxuiRowKey = Option<extern "C" fn(*mut core::ffi::c_void, usize) -> i64>;
pub type PxuiMeasuredSectionBuild = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, usize, f32)>;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiMeasuredSectionDesc {
    pub size:u32,
    pub overlay_scrollbar:u8,
    pub reserved:[u8;3],
    pub label:Str,
    pub count:usize,
    pub row_height:f32,
    pub leading_height:f32,
    pub header_height:f32,
    pub trailing_height:f32,
    pub build:PxuiMeasuredSectionBuild,
    pub user:*mut core::ffi::c_void,
    /// ABI 1.49: as `PxuiSectionListDesc::key`.
    pub key:PxuiRowKey,
}
#[repr(C)]
pub struct PxuiSplitDesc {
    pub size: u32,
    pub axis: u32,
    pub label: Str,
    pub ratio: f32,
    pub min_a: f32,
    pub min_b: f32,
    pub reserved: u32,
    pub build_a: PxuiBuild,
    pub user_a: *mut core::ffi::c_void,
    pub build_b: PxuiBuild,
    pub user_b: *mut core::ffi::c_void,
}
#[repr(C)]
pub struct PxuiMenuBarDesc {
    pub size: u32,
    pub reserved: u32,
    pub label: Str,
    pub menus: Slice<PxuiMenuDesc>,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiMenuOut {
    pub size: u32,
    pub reserved: u32,
    pub menu: usize,
    pub item: usize,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiHintDesc {
    pub size: u32,
    pub reserved: u32,
    pub key: Str,
    pub value: Str,
}
#[repr(C)]
pub struct PxuiFooterDesc {
    pub size: u32,
    pub has_right: u8,
    pub reserved: [u8; 3],
    pub right: Str,
    pub hints: Slice<PxuiHintDesc>,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiKeyframeDesc {
    pub size: u32,
    pub time: f32,
    pub target: u32,
    pub reserved: u32,
    pub values: [f32; 4],
    pub look: PxuiLookDesc,
    pub curve: PxuiCurveDesc,
}

#[repr(C)]
pub struct PxuiInspectorRowDesc {
    pub size: u32,
    pub has_width: u8,
    pub has_pin: u8,
    pub pin: u8,
    pub reserved: u8,
    pub key: Str,
    pub label: Str,
    pub width: f32,
    pub reserved_tail: u32,
    pub row: PxuiBox,
    pub control_x: f32,
    pub control_y: f32,
    pub control_w: f32,
}
#[repr(C)]
pub struct PxuiInspectorSectionDesc {
    pub size: u32,
    pub expanded: u8,
    pub has_set: u8,
    pub set_expanded: u8,
    pub reserved: u8,
    pub key: Str,
    pub label: Str,
    pub build: PxuiBuild,
    pub user: *mut core::ffi::c_void,
    pub out_open: u8,
}
#[repr(C)]
pub struct PxuiInspectorValueDesc {
    pub size: u32,
    pub has_width: u8,
    pub reserved: [u8; 3],
    pub key: Str,
    pub label: Str,
    pub value: Str,
    pub width: f32,
}
#[repr(C)]
pub struct PxuiInspectorTextDesc {
    pub size: u32,
    pub reserved: u32,
    pub key: Str,
    pub text: Str,
}
pub type PxuiTableCell = Option<extern "C" fn(*mut core::ffi::c_void, i64, i64) -> Str>;
#[repr(C)]
pub struct PxuiTableDesc {
    pub size: u32,
    pub reserved: u32,
    pub label: Str,
    pub at: [f32; 2],
    pub width: f32,
    pub height: f32,
    pub headers: Slice<Str>,
    pub rows: i64,
    pub cell: PxuiTableCell,
    pub user: *mut core::ffi::c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiU8MutSpan {
    pub ptr: *mut u8,
    pub len: usize,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiF32MutSpan {
    pub ptr: *mut f32,
    pub len: usize,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiU32MutSpan {
    pub ptr: *mut u32,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiTreeNode {
    pub depth: u16,
    pub children: u8,
}

pub type PxuiCollectionEmit = Option<extern "C" fn(*mut core::ffi::c_void, Str)>;
pub type PxuiCollectionText = Option<extern "C" fn(*mut core::ffi::c_void, usize, usize, PxuiCollectionEmit, *mut core::ffi::c_void)>;

#[repr(C)]
pub struct PxuiListViewDesc {
    pub size: u32,
    pub multi: u8,
    pub reserved: [u8; 3],
    pub label: Str,
    pub count: usize,
    pub selected: PxuiU8MutSpan,
    pub text: PxuiCollectionText,
    pub user: *mut core::ffi::c_void,
}

#[repr(C)]
pub struct PxuiTreeViewDesc {
    pub size: u32,
    pub reserved: u32,
    pub label: Str,
    pub nodes: Slice<PxuiTreeNode>,
    pub open: PxuiU8MutSpan,
    pub selected: *mut usize,
    pub visible: PxuiU32MutSpan,
    pub visible_len: *mut usize,
    pub text: PxuiCollectionText,
    pub user: *mut core::ffi::c_void,
}

#[repr(C)]
pub struct PxuiTableViewDesc {
    pub size: u32,
    pub has_sort: u8,
    pub ascending: u8,
    pub reserved: [u8; 2],
    pub label: Str,
    pub titles: Slice<Str>,
    pub widths: PxuiF32MutSpan,
    pub rows: usize,
    pub sort_column: usize,
    pub selected: *mut usize,
    pub text: PxuiCollectionText,
    pub user: *mut core::ffi::c_void,
}


// ---- widget bodies (`pxui_body_register`): what a body is given, one record per kind. The records hold what the widget's behaviour
// already computed; the text beside each (a label, a value) is in the record, borrowed until the body returns. A body builds nodes
// with the ordinary calls on the `ui` it is given, finds nothing but the parts it names (`pxui_part`) and runs the caller's content
// through its slots (`pxui_slot`). Each record starts with `size` and grows only at its end; a flag is 0 or 1, a handle a `PxuiBox`.

/// A body of widget content the caller gave (a panel's children, a table cell): valid only during the body call that received it,
/// run with `pxui_slot`. A slot that is stale is refused.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct PxuiSlot { pub serial: u64 }

/// A registered body: `(user, ui, config)`, `ui` a borrowed `PxuiUi` valid for the call and `config` the kind's record below.
pub type PxuiBodyFn = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *const core::ffi::c_void)>;

/// A `_with` form's row or cell content: `(user, ui, row, column)` builds the children of one row or cell (`column` is 0 in a list
/// or a tree), `ui` a borrowed `PxuiUi` valid for the call.
pub type PxuiCellBuild = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, usize, usize)>;

#[repr(C)]
pub struct PxuiListViewWithDesc {
    pub size: u32,
    pub multi: u8,
    pub reserved: [u8; 3],
    pub label: Str,
    pub count: usize,
    pub selected: PxuiU8MutSpan,
    pub build: PxuiCellBuild,
    pub user: *mut core::ffi::c_void,
}

#[repr(C)]
pub struct PxuiTreeViewWithDesc {
    pub size: u32,
    pub reserved: u32,
    pub label: Str,
    pub nodes: Slice<PxuiTreeNode>,
    pub open: PxuiU8MutSpan,
    pub selected: *mut usize,
    pub visible: PxuiU32MutSpan,
    pub visible_len: *mut usize,
    pub build: PxuiCellBuild,
    pub user: *mut core::ffi::c_void,
}

#[repr(C)]
pub struct PxuiTableViewWithDesc {
    pub size: u32,
    pub has_sort: u8,
    pub ascending: u8,
    pub reserved: [u8; 2],
    pub label: Str,
    pub titles: Slice<Str>,
    pub widths: PxuiF32MutSpan,
    pub rows: usize,
    pub sort_column: usize,
    pub selected: *mut usize,
    pub build: PxuiCellBuild,
    pub user: *mut core::ffi::c_void,
}

use crate::theme::States as PxuiStates;

macro_rules! configs {
    ($($(#[$doc:meta])* $name:ident { $($(#[$fdoc:meta])* $f:ident: $t:ty),* $(,)? })*) => {
        $($(#[$doc])* #[repr(C)] #[derive(Clone, Copy)] pub struct $name { pub size: u32, $($(#[$fdoc])* pub $f: $t),* })*
        #[cfg(test)]
        fn config_layouts(check: &mut dyn FnMut(&str, usize, usize, &[(&str, usize)])) {
            use core::mem::{align_of, size_of};
            $(check(stringify!($name), size_of::<$name>(), align_of::<$name>(), &[("size", core::mem::offset_of!($name, size)), $((stringify!($f), core::mem::offset_of!($name, $f))),*]);)*
        }
    };
}

configs! {
    /// `row` is entered and owns `Part::HIT`.
    PxuiToggleConfig {
        state: PxuiStateDesc, on: u8, row: PxuiBox, label: Str,
        /// The row's height in points the caller asked for (0 the kit's).
        row_h: f32,
        /// The switch's width and height in points the caller asked for (0 the kit's 28 x 14); the knob is 6 less than its height.
        switch_w: f32,
        switch_h: f32,
        /// The looks of the switch and of its knob (`pick(checked)`): the caller's or the theme's; valid for the call.
        track: *const PxuiStates,
        knob: *const PxuiStates,
        /// The distance of the label and the switch from the row's sides in points the caller asked for (0 the kit's).
        inset: f32,
        /// 0 or 1: a press anywhere on the row toggles it (behaviour then ignores `Part::HIT`).
        press_row: u8,
        /// A secondary line under the label (empty: none); valid for the call.
        detail: Str,
    }
    /// `track` is entered; `fraction` is the value's place, 0 to 1. Behaviour reads `Part::RAIL` and `Part::THUMB`.
    PxuiSliderConfig { state: PxuiStateDesc, fraction: f32, track: PxuiBox, label: Str }
    PxuiProgressConfig { has_value: u8, value: f32, row: PxuiBox, label: Str }
    /// `states.pick(state)` is the look the caller asked for; `states` is valid for the call.
    PxuiButtonConfig {
        state: PxuiStateDesc, bare: u8, icon: u8, at_end: u8, line: PxuiBox, row: PxuiBox, states: *const PxuiStates, label: Str,
        /// Width kind the caller asked for: 0 the kit's, 1 `w` points, 2 fill.
        w_kind: u8,
        /// Height kind the caller asked for: 0 the kit's, 1 `h` points, 2 fill.
        h_kind: u8,
        /// Placement in the line: 0 start, 1 centre, 2 end (2 also sets `at_end`).
        justify: u8,
        /// Zero.
        reserved: u8,
        /// Width in points when `w_kind` is 1.
        w: f32,
        /// Height in points when `h_kind` is 1.
        h: f32,
        /// Extra padding inside the box: left, top, right, bottom, in points.
        padding: [f32; 4],
    }
    /// `states.pick(state)` is the look the caller asked for; `states` is valid for the call.
    PxuiIconButtonConfig {
        state: PxuiStateDesc, node: PxuiBox, label: Str, icon: Str, states: *const PxuiStates,
        /// Width kind the caller asked for: 0 the kit's square, 1 `w` points, 2 fill.
        w_kind: u8,
        /// Height kind the caller asked for: 0 the kit's square, 1 `h` points, 2 fill.
        h_kind: u8,
        /// Zero.
        reserved: u8,
        /// Width in points when `w_kind` is 1.
        w: f32,
        /// Height in points when `h_kind` is 1.
        h: f32,
        /// The scale the contents spring to while pressed (1.0: none).
        press_scale: f32,
        /// The app's drawing of the icon (null for a glyph or a drawing that did not cross the ABI) and its user data: the body
        /// paints it, in `ink`, by calling it from a paint callback of its own on a node (`pxui_ui_draw`).
        draw: PxuiIconDraw,
        draw_user: *mut core::ffi::c_void,
        /// The look's text colour for `state`.
        ink: Rgba,
    }
    /// A checkbox or a radio option.
    PxuiCheckConfig { state: PxuiStateDesc, on: u8, row: PxuiBox, label: Str }
    PxuiSpinnerConfig { node: PxuiBox, label: Str }
    PxuiBadgeConfig { node: PxuiBox, label: Str, text: Str }
    /// The record grows at its end in ABI 1.45 by `style`, the label's style as the caller gave it (unset fields: the theme's);
    /// a body reads it only when `size` covers it.
    PxuiLabelConfig { first: u8, node: PxuiBox, text: Str, style: PxuiLabelStyle }
    PxuiFooterConfig { row: PxuiBox, has_right: u8, right: Str, hints: Slice<PxuiHintDesc> }
    PxuiGripConfig { state: PxuiStateDesc, vertical: u8, floating: u8, grip: PxuiBox }
    /// A panel, window, popup or modal sheet.
    PxuiPanelConfig { node: PxuiBox, label: Str, content: PxuiSlot }
    PxuiDialogConfig { title: Str, buttons: Slice<Str>, content: PxuiSlot }
    PxuiSectionConfig { open: u8, gap: f32, title: PxuiBox, label: Str, content: PxuiSlot }
    PxuiInspectorRowConfig { state: PxuiStateDesc, has_pin: u8, pin: u8, label_w: f32, row: PxuiBox, label: Str }
    /// `axis` is 0 row, 1 column, 2 stack.
    PxuiSplitConfig { axis: u32, ratio: f32, min_a: f32, min_b: f32, split: PxuiBox, first: PxuiSlot, second: PxuiSlot }
    PxuiScrollConfig { node: PxuiBox, content: PxuiSlot }
    PxuiToastConfig { state: PxuiStateDesc, node: PxuiBox, text: Str }
    PxuiTipConfig { width: f32, height: f32, tip: PxuiBox, lines: Slice<Str> }
    PxuiTabConfig { state: PxuiStateDesc, index: usize, node: PxuiBox, text: Str }
    PxuiSegmentConfig {
        state: PxuiStateDesc, index: usize, selected: usize, group: i64, node: PxuiBox, text: Str,
        /// The look per state of a segment: the caller's or the theme's; valid for the call.
        states: *const PxuiStates,
        /// 0 or 1: the segments share the strip's width instead of each being as wide as its text.
        grow: u8,
    }
    PxuiDropdownConfig { state: PxuiStateDesc, open: u8, key: i64, button: PxuiBox, text: Str }
    /// A menu bar title or a row of an open menu.
    PxuiMenuItemConfig { state: PxuiStateDesc, index: usize, node: PxuiBox, text: Str }
    PxuiMenuRowConfig {
        state: PxuiStateDesc, selected: u8, chevron: u8, danger: u8, has_swatch: u8, swatch: Rgba, has_shortcut: u8, index: i64, lead: f32, swatch_pad: f32,
        row: PxuiBox, text: Str, shortcut: Str, row_h: f32
    }
    /// A `list_view` or `table_view` row; `content` is the text or the caller's cells.
    PxuiRowConfig { state: PxuiStateDesc, index: usize, row: PxuiBox, content: PxuiSlot }
    PxuiTreeRowConfig { state: PxuiStateDesc, has: u8, open: u8, index: usize, node: usize, depth: usize, row: PxuiBox, content: PxuiSlot }
    PxuiTableHeadConfig { state: PxuiStateDesc, has_ascending: u8, ascending: u8, index: usize, node: PxuiBox, title: Str }
    PxuiTableCellConfig { header: u8, left: f32, right: f32, height: f32, row: i64, column: i64, key: i64, text: Str }
    PxuiPickerRowConfig {
        state: PxuiStateDesc, current: u8, doomed: u8, unavailable: u8, marked: u8, has_swatch: u8, swatch: Rgba, index: i64,
        row: PxuiBox, label: Str, detail: Str, query: Str
    }
    /// `editor` is a slot that builds the editing surface and returns whether the query is not empty (`pxui_slot`'s `out`). Behaviour
    /// reads `Part::CLEAR`.
    PxuiSearchFieldConfig { state: PxuiStateDesc, key: i64, well: PxuiBox, icon: Str, editor: PxuiSlot }
    PxuiTextFieldConfig { state: PxuiStateDesc, row: PxuiBox, label: Str, value: Str }
    PxuiLineFieldConfig { state: PxuiStateDesc, empty: u8, field: PxuiBox, text: Str }
    /// `opts` holds the caller's options (its strings borrowed for the call); its place, label and value are unset.
    PxuiValueFieldConfig { state: PxuiStateDesc, invalid: u8, w: f32, h: f32, field: PxuiBox, opts: PxuiValueFieldDesc, text: Str }
    PxuiPickerFieldConfig { state: PxuiStateDesc, empty: u8, slash: u8, prefix: f32, inset: f32, search: PxuiBox, placeholder: Str }
}


#[cfg(test)]
pub(crate) mod layout {
    use super::*;
    use core::mem::{align_of, size_of};
    use crate::test_layout_rows::row as check;
    /// Every ffi row, collected into the layout rows by `test_layout_rows`.
    pub(crate) fn ffi_rows() {
        config_layouts(&mut |name, size, align, offsets| check(name, size, align, offsets));
        check("PxuiLabelStyle", size_of::<PxuiLabelStyle>(), align_of::<PxuiLabelStyle>(), &[("size", core::mem::offset_of!(PxuiLabelStyle, size)), ("set", core::mem::offset_of!(PxuiLabelStyle, set)), ("font", core::mem::offset_of!(PxuiLabelStyle, font)), ("weight", core::mem::offset_of!(PxuiLabelStyle, weight)), ("color", core::mem::offset_of!(PxuiLabelStyle, color)), ("max_lines", core::mem::offset_of!(PxuiLabelStyle, max_lines)), ("caps", core::mem::offset_of!(PxuiLabelStyle, caps)), ("digits", core::mem::offset_of!(PxuiLabelStyle, digits)), ("ellipsis", core::mem::offset_of!(PxuiLabelStyle, ellipsis)), ("reserved", core::mem::offset_of!(PxuiLabelStyle, reserved)), ("size_points", core::mem::offset_of!(PxuiLabelStyle, size_points)), ("tracking", core::mem::offset_of!(PxuiLabelStyle, tracking)), ("line_height", core::mem::offset_of!(PxuiLabelStyle, line_height)), ("pad", core::mem::offset_of!(PxuiLabelStyle, pad)), ("wrap", core::mem::offset_of!(PxuiLabelStyle, wrap)), ("align", core::mem::offset_of!(PxuiLabelStyle, align)), ("features", core::mem::offset_of!(PxuiLabelStyle, features)), ("width", core::mem::offset_of!(PxuiLabelStyle, width))]);
        check("PxuiMenuDesc", size_of::<PxuiMenuDesc>(), align_of::<PxuiMenuDesc>(), &[("size", core::mem::offset_of!(PxuiMenuDesc, size)), ("reserved", core::mem::offset_of!(PxuiMenuDesc, reserved)), ("title", core::mem::offset_of!(PxuiMenuDesc, title)), ("items", core::mem::offset_of!(PxuiMenuDesc, items))]);
        check("PxuiGridListDesc", size_of::<PxuiGridListDesc>(), align_of::<PxuiGridListDesc>(), &[("size", core::mem::offset_of!(PxuiGridListDesc, size)), ("reserved", core::mem::offset_of!(PxuiGridListDesc, reserved)), ("label", core::mem::offset_of!(PxuiGridListDesc, label)), ("count", core::mem::offset_of!(PxuiGridListDesc, count)), ("cell_w", core::mem::offset_of!(PxuiGridListDesc, cell_w)), ("cell_h", core::mem::offset_of!(PxuiGridListDesc, cell_h)), ("build", core::mem::offset_of!(PxuiGridListDesc, build)), ("user", core::mem::offset_of!(PxuiGridListDesc, user))]);
        check("PxuiSectionListDesc", size_of::<PxuiSectionListDesc>(), align_of::<PxuiSectionListDesc>(), &[("size", core::mem::offset_of!(PxuiSectionListDesc, size)), ("overlay_scrollbar", core::mem::offset_of!(PxuiSectionListDesc, overlay_scrollbar)), ("reserved", core::mem::offset_of!(PxuiSectionListDesc, reserved)), ("label", core::mem::offset_of!(PxuiSectionListDesc, label)), ("count", core::mem::offset_of!(PxuiSectionListDesc, count)), ("row_height", core::mem::offset_of!(PxuiSectionListDesc, row_height)), ("leading_height", core::mem::offset_of!(PxuiSectionListDesc, leading_height)), ("header_height", core::mem::offset_of!(PxuiSectionListDesc, header_height)), ("trailing_height", core::mem::offset_of!(PxuiSectionListDesc, trailing_height)), ("build", core::mem::offset_of!(PxuiSectionListDesc, build)), ("user", core::mem::offset_of!(PxuiSectionListDesc, user)), ("key", core::mem::offset_of!(PxuiSectionListDesc, key))]);
        check("PxuiMeasuredSectionDesc", size_of::<PxuiMeasuredSectionDesc>(), align_of::<PxuiMeasuredSectionDesc>(), &[("size", core::mem::offset_of!(PxuiMeasuredSectionDesc, size)), ("overlay_scrollbar", core::mem::offset_of!(PxuiMeasuredSectionDesc, overlay_scrollbar)), ("reserved", core::mem::offset_of!(PxuiMeasuredSectionDesc, reserved)), ("label", core::mem::offset_of!(PxuiMeasuredSectionDesc, label)), ("count", core::mem::offset_of!(PxuiMeasuredSectionDesc, count)), ("row_height", core::mem::offset_of!(PxuiMeasuredSectionDesc, row_height)), ("leading_height", core::mem::offset_of!(PxuiMeasuredSectionDesc, leading_height)), ("header_height", core::mem::offset_of!(PxuiMeasuredSectionDesc, header_height)), ("trailing_height", core::mem::offset_of!(PxuiMeasuredSectionDesc, trailing_height)), ("build", core::mem::offset_of!(PxuiMeasuredSectionDesc, build)), ("user", core::mem::offset_of!(PxuiMeasuredSectionDesc, user)), ("key", core::mem::offset_of!(PxuiMeasuredSectionDesc, key))]);
        check("PxuiSplitDesc", size_of::<PxuiSplitDesc>(), align_of::<PxuiSplitDesc>(), &[("size", core::mem::offset_of!(PxuiSplitDesc, size)), ("axis", core::mem::offset_of!(PxuiSplitDesc, axis)), ("label", core::mem::offset_of!(PxuiSplitDesc, label)), ("ratio", core::mem::offset_of!(PxuiSplitDesc, ratio)), ("min_a", core::mem::offset_of!(PxuiSplitDesc, min_a)), ("min_b", core::mem::offset_of!(PxuiSplitDesc, min_b)), ("reserved", core::mem::offset_of!(PxuiSplitDesc, reserved)), ("build_a", core::mem::offset_of!(PxuiSplitDesc, build_a)), ("user_a", core::mem::offset_of!(PxuiSplitDesc, user_a)), ("build_b", core::mem::offset_of!(PxuiSplitDesc, build_b)), ("user_b", core::mem::offset_of!(PxuiSplitDesc, user_b))]);
        check("PxuiMenuBarDesc", size_of::<PxuiMenuBarDesc>(), align_of::<PxuiMenuBarDesc>(), &[("size", core::mem::offset_of!(PxuiMenuBarDesc, size)), ("reserved", core::mem::offset_of!(PxuiMenuBarDesc, reserved)), ("label", core::mem::offset_of!(PxuiMenuBarDesc, label)), ("menus", core::mem::offset_of!(PxuiMenuBarDesc, menus))]);
        check("PxuiMenuOut", size_of::<PxuiMenuOut>(), align_of::<PxuiMenuOut>(), &[("size", core::mem::offset_of!(PxuiMenuOut, size)), ("reserved", core::mem::offset_of!(PxuiMenuOut, reserved)), ("menu", core::mem::offset_of!(PxuiMenuOut, menu)), ("item", core::mem::offset_of!(PxuiMenuOut, item))]);
        check("PxuiHintDesc", size_of::<PxuiHintDesc>(), align_of::<PxuiHintDesc>(), &[("size", core::mem::offset_of!(PxuiHintDesc, size)), ("reserved", core::mem::offset_of!(PxuiHintDesc, reserved)), ("key", core::mem::offset_of!(PxuiHintDesc, key)), ("value", core::mem::offset_of!(PxuiHintDesc, value))]);
        check("PxuiFooterDesc", size_of::<PxuiFooterDesc>(), align_of::<PxuiFooterDesc>(), &[("size", core::mem::offset_of!(PxuiFooterDesc, size)), ("has_right", core::mem::offset_of!(PxuiFooterDesc, has_right)), ("reserved", core::mem::offset_of!(PxuiFooterDesc, reserved)), ("right", core::mem::offset_of!(PxuiFooterDesc, right)), ("hints", core::mem::offset_of!(PxuiFooterDesc, hints))]);
        check("PxuiKeyframeDesc", size_of::<PxuiKeyframeDesc>(), align_of::<PxuiKeyframeDesc>(), &[("size", core::mem::offset_of!(PxuiKeyframeDesc, size)), ("time", core::mem::offset_of!(PxuiKeyframeDesc, time)), ("target", core::mem::offset_of!(PxuiKeyframeDesc, target)), ("reserved", core::mem::offset_of!(PxuiKeyframeDesc, reserved)), ("values", core::mem::offset_of!(PxuiKeyframeDesc, values)), ("look", core::mem::offset_of!(PxuiKeyframeDesc, look)), ("curve", core::mem::offset_of!(PxuiKeyframeDesc, curve))]);
        check("PxuiInspectorRowDesc", size_of::<PxuiInspectorRowDesc>(), align_of::<PxuiInspectorRowDesc>(), &[("size", core::mem::offset_of!(PxuiInspectorRowDesc, size)), ("has_width", core::mem::offset_of!(PxuiInspectorRowDesc, has_width)), ("has_pin", core::mem::offset_of!(PxuiInspectorRowDesc, has_pin)), ("pin", core::mem::offset_of!(PxuiInspectorRowDesc, pin)), ("reserved", core::mem::offset_of!(PxuiInspectorRowDesc, reserved)), ("key", core::mem::offset_of!(PxuiInspectorRowDesc, key)), ("label", core::mem::offset_of!(PxuiInspectorRowDesc, label)), ("width", core::mem::offset_of!(PxuiInspectorRowDesc, width)), ("reserved_tail", core::mem::offset_of!(PxuiInspectorRowDesc, reserved_tail)), ("row", core::mem::offset_of!(PxuiInspectorRowDesc, row)), ("control_x", core::mem::offset_of!(PxuiInspectorRowDesc, control_x)), ("control_y", core::mem::offset_of!(PxuiInspectorRowDesc, control_y)), ("control_w", core::mem::offset_of!(PxuiInspectorRowDesc, control_w))]);
        check("PxuiInspectorSectionDesc", size_of::<PxuiInspectorSectionDesc>(), align_of::<PxuiInspectorSectionDesc>(), &[("size", core::mem::offset_of!(PxuiInspectorSectionDesc, size)), ("expanded", core::mem::offset_of!(PxuiInspectorSectionDesc, expanded)), ("has_set", core::mem::offset_of!(PxuiInspectorSectionDesc, has_set)), ("set_expanded", core::mem::offset_of!(PxuiInspectorSectionDesc, set_expanded)), ("reserved", core::mem::offset_of!(PxuiInspectorSectionDesc, reserved)), ("key", core::mem::offset_of!(PxuiInspectorSectionDesc, key)), ("label", core::mem::offset_of!(PxuiInspectorSectionDesc, label)), ("build", core::mem::offset_of!(PxuiInspectorSectionDesc, build)), ("user", core::mem::offset_of!(PxuiInspectorSectionDesc, user)), ("out_open", core::mem::offset_of!(PxuiInspectorSectionDesc, out_open))]);
        check("PxuiInspectorValueDesc", size_of::<PxuiInspectorValueDesc>(), align_of::<PxuiInspectorValueDesc>(), &[("size", core::mem::offset_of!(PxuiInspectorValueDesc, size)), ("has_width", core::mem::offset_of!(PxuiInspectorValueDesc, has_width)), ("reserved", core::mem::offset_of!(PxuiInspectorValueDesc, reserved)), ("key", core::mem::offset_of!(PxuiInspectorValueDesc, key)), ("label", core::mem::offset_of!(PxuiInspectorValueDesc, label)), ("value", core::mem::offset_of!(PxuiInspectorValueDesc, value)), ("width", core::mem::offset_of!(PxuiInspectorValueDesc, width))]);
        check("PxuiInspectorTextDesc", size_of::<PxuiInspectorTextDesc>(), align_of::<PxuiInspectorTextDesc>(), &[("size", core::mem::offset_of!(PxuiInspectorTextDesc, size)), ("reserved", core::mem::offset_of!(PxuiInspectorTextDesc, reserved)), ("key", core::mem::offset_of!(PxuiInspectorTextDesc, key)), ("text", core::mem::offset_of!(PxuiInspectorTextDesc, text))]);
        check("PxuiTableDesc", size_of::<PxuiTableDesc>(), align_of::<PxuiTableDesc>(), &[("size", core::mem::offset_of!(PxuiTableDesc, size)), ("reserved", core::mem::offset_of!(PxuiTableDesc, reserved)), ("label", core::mem::offset_of!(PxuiTableDesc, label)), ("at", core::mem::offset_of!(PxuiTableDesc, at)), ("width", core::mem::offset_of!(PxuiTableDesc, width)), ("height", core::mem::offset_of!(PxuiTableDesc, height)), ("headers", core::mem::offset_of!(PxuiTableDesc, headers)), ("rows", core::mem::offset_of!(PxuiTableDesc, rows)), ("cell", core::mem::offset_of!(PxuiTableDesc, cell)), ("user", core::mem::offset_of!(PxuiTableDesc, user))]);
        check("PxuiU8MutSpan", size_of::<PxuiU8MutSpan>(), align_of::<PxuiU8MutSpan>(), &[("ptr", core::mem::offset_of!(PxuiU8MutSpan, ptr)), ("len", core::mem::offset_of!(PxuiU8MutSpan, len))]);
        check("PxuiF32MutSpan", size_of::<PxuiF32MutSpan>(), align_of::<PxuiF32MutSpan>(), &[("ptr", core::mem::offset_of!(PxuiF32MutSpan, ptr)), ("len", core::mem::offset_of!(PxuiF32MutSpan, len))]);
        check("PxuiU32MutSpan", size_of::<PxuiU32MutSpan>(), align_of::<PxuiU32MutSpan>(), &[("ptr", core::mem::offset_of!(PxuiU32MutSpan, ptr)), ("len", core::mem::offset_of!(PxuiU32MutSpan, len))]);
        check("PxuiTreeNode", size_of::<PxuiTreeNode>(), align_of::<PxuiTreeNode>(), &[("depth", core::mem::offset_of!(PxuiTreeNode, depth)), ("children", core::mem::offset_of!(PxuiTreeNode, children))]);
        check("PxuiListViewDesc", size_of::<PxuiListViewDesc>(), align_of::<PxuiListViewDesc>(), &[("size", core::mem::offset_of!(PxuiListViewDesc, size)), ("multi", core::mem::offset_of!(PxuiListViewDesc, multi)), ("reserved", core::mem::offset_of!(PxuiListViewDesc, reserved)), ("label", core::mem::offset_of!(PxuiListViewDesc, label)), ("count", core::mem::offset_of!(PxuiListViewDesc, count)), ("selected", core::mem::offset_of!(PxuiListViewDesc, selected)), ("text", core::mem::offset_of!(PxuiListViewDesc, text)), ("user", core::mem::offset_of!(PxuiListViewDesc, user))]);
        check("PxuiTreeViewDesc", size_of::<PxuiTreeViewDesc>(), align_of::<PxuiTreeViewDesc>(), &[("size", core::mem::offset_of!(PxuiTreeViewDesc, size)), ("reserved", core::mem::offset_of!(PxuiTreeViewDesc, reserved)), ("label", core::mem::offset_of!(PxuiTreeViewDesc, label)), ("nodes", core::mem::offset_of!(PxuiTreeViewDesc, nodes)), ("open", core::mem::offset_of!(PxuiTreeViewDesc, open)), ("selected", core::mem::offset_of!(PxuiTreeViewDesc, selected)), ("visible", core::mem::offset_of!(PxuiTreeViewDesc, visible)), ("visible_len", core::mem::offset_of!(PxuiTreeViewDesc, visible_len)), ("text", core::mem::offset_of!(PxuiTreeViewDesc, text)), ("user", core::mem::offset_of!(PxuiTreeViewDesc, user))]);
        check("PxuiTableViewDesc", size_of::<PxuiTableViewDesc>(), align_of::<PxuiTableViewDesc>(), &[("size", core::mem::offset_of!(PxuiTableViewDesc, size)), ("has_sort", core::mem::offset_of!(PxuiTableViewDesc, has_sort)), ("ascending", core::mem::offset_of!(PxuiTableViewDesc, ascending)), ("reserved", core::mem::offset_of!(PxuiTableViewDesc, reserved)), ("label", core::mem::offset_of!(PxuiTableViewDesc, label)), ("titles", core::mem::offset_of!(PxuiTableViewDesc, titles)), ("widths", core::mem::offset_of!(PxuiTableViewDesc, widths)), ("rows", core::mem::offset_of!(PxuiTableViewDesc, rows)), ("sort_column", core::mem::offset_of!(PxuiTableViewDesc, sort_column)), ("selected", core::mem::offset_of!(PxuiTableViewDesc, selected)), ("text", core::mem::offset_of!(PxuiTableViewDesc, text)), ("user", core::mem::offset_of!(PxuiTableViewDesc, user))]);
        check("PxuiSlot", size_of::<PxuiSlot>(), align_of::<PxuiSlot>(), &[("serial", core::mem::offset_of!(PxuiSlot, serial))]);
        check("PxuiListViewWithDesc", size_of::<PxuiListViewWithDesc>(), align_of::<PxuiListViewWithDesc>(), &[("size", core::mem::offset_of!(PxuiListViewWithDesc, size)), ("multi", core::mem::offset_of!(PxuiListViewWithDesc, multi)), ("reserved", core::mem::offset_of!(PxuiListViewWithDesc, reserved)), ("label", core::mem::offset_of!(PxuiListViewWithDesc, label)), ("count", core::mem::offset_of!(PxuiListViewWithDesc, count)), ("selected", core::mem::offset_of!(PxuiListViewWithDesc, selected)), ("build", core::mem::offset_of!(PxuiListViewWithDesc, build)), ("user", core::mem::offset_of!(PxuiListViewWithDesc, user))]);
        check("PxuiTreeViewWithDesc", size_of::<PxuiTreeViewWithDesc>(), align_of::<PxuiTreeViewWithDesc>(), &[("size", core::mem::offset_of!(PxuiTreeViewWithDesc, size)), ("reserved", core::mem::offset_of!(PxuiTreeViewWithDesc, reserved)), ("label", core::mem::offset_of!(PxuiTreeViewWithDesc, label)), ("nodes", core::mem::offset_of!(PxuiTreeViewWithDesc, nodes)), ("open", core::mem::offset_of!(PxuiTreeViewWithDesc, open)), ("selected", core::mem::offset_of!(PxuiTreeViewWithDesc, selected)), ("visible", core::mem::offset_of!(PxuiTreeViewWithDesc, visible)), ("visible_len", core::mem::offset_of!(PxuiTreeViewWithDesc, visible_len)), ("build", core::mem::offset_of!(PxuiTreeViewWithDesc, build)), ("user", core::mem::offset_of!(PxuiTreeViewWithDesc, user))]);
        check("PxuiTableViewWithDesc", size_of::<PxuiTableViewWithDesc>(), align_of::<PxuiTableViewWithDesc>(), &[("size", core::mem::offset_of!(PxuiTableViewWithDesc, size)), ("has_sort", core::mem::offset_of!(PxuiTableViewWithDesc, has_sort)), ("ascending", core::mem::offset_of!(PxuiTableViewWithDesc, ascending)), ("reserved", core::mem::offset_of!(PxuiTableViewWithDesc, reserved)), ("label", core::mem::offset_of!(PxuiTableViewWithDesc, label)), ("titles", core::mem::offset_of!(PxuiTableViewWithDesc, titles)), ("widths", core::mem::offset_of!(PxuiTableViewWithDesc, widths)), ("rows", core::mem::offset_of!(PxuiTableViewWithDesc, rows)), ("sort_column", core::mem::offset_of!(PxuiTableViewWithDesc, sort_column)), ("selected", core::mem::offset_of!(PxuiTableViewWithDesc, selected)), ("build", core::mem::offset_of!(PxuiTableViewWithDesc, build)), ("user", core::mem::offset_of!(PxuiTableViewWithDesc, user))]);
        check("PxuiNodeFields", size_of::<PxuiNodeFields>(), align_of::<PxuiNodeFields>(), &[("size", core::mem::offset_of!(PxuiNodeFields, size)), ("reserved", core::mem::offset_of!(PxuiNodeFields, reserved)), ("look", core::mem::offset_of!(PxuiNodeFields, look)), ("mark", core::mem::offset_of!(PxuiNodeFields, mark)), ("inset", core::mem::offset_of!(PxuiNodeFields, inset))]);
        check("PxuiRange", size_of::<PxuiRange>(), align_of::<PxuiRange>(), &[("start", core::mem::offset_of!(PxuiRange, start)), ("end", core::mem::offset_of!(PxuiRange, end))]);
        check("PxuiColorSpan", size_of::<PxuiColorSpan>(), align_of::<PxuiColorSpan>(), &[("start", core::mem::offset_of!(PxuiColorSpan, start)), ("end", core::mem::offset_of!(PxuiColorSpan, end)), ("color", core::mem::offset_of!(PxuiColorSpan, color)), ("reserved", core::mem::offset_of!(PxuiColorSpan, reserved))]);
        check("PxuiNote", size_of::<PxuiNote>(), align_of::<PxuiNote>(), &[("line", core::mem::offset_of!(PxuiNote, line)), ("start", core::mem::offset_of!(PxuiNote, start)), ("end", core::mem::offset_of!(PxuiNote, end)), ("has_range", core::mem::offset_of!(PxuiNote, has_range)), ("reserved", core::mem::offset_of!(PxuiNote, reserved)), ("text", core::mem::offset_of!(PxuiNote, text))]);
        check("PxuiSplice", size_of::<PxuiSplice>(), align_of::<PxuiSplice>(), &[("start", core::mem::offset_of!(PxuiSplice, start)), ("end", core::mem::offset_of!(PxuiSplice, end)), ("text", core::mem::offset_of!(PxuiSplice, text))]);
        check("PxuiCompletion", size_of::<PxuiCompletion>(), align_of::<PxuiCompletion>(), &[("start", core::mem::offset_of!(PxuiCompletion, start)), ("end", core::mem::offset_of!(PxuiCompletion, end)), ("insert", core::mem::offset_of!(PxuiCompletion, insert)), ("label", core::mem::offset_of!(PxuiCompletion, label)), ("detail", core::mem::offset_of!(PxuiCompletion, detail)), ("doc", core::mem::offset_of!(PxuiCompletion, doc))]);
        check("PxuiDescription", size_of::<PxuiDescription>(), align_of::<PxuiDescription>(), &[("has", core::mem::offset_of!(PxuiDescription, has)), ("reserved", core::mem::offset_of!(PxuiDescription, reserved)), ("start", core::mem::offset_of!(PxuiDescription, start)), ("end", core::mem::offset_of!(PxuiDescription, end)), ("text", core::mem::offset_of!(PxuiDescription, text))]);
        check("PxuiMaybeRange", size_of::<PxuiMaybeRange>(), align_of::<PxuiMaybeRange>(), &[("has", core::mem::offset_of!(PxuiMaybeRange, has)), ("reserved", core::mem::offset_of!(PxuiMaybeRange, reserved)), ("start", core::mem::offset_of!(PxuiMaybeRange, start)), ("end", core::mem::offset_of!(PxuiMaybeRange, end))]);
        check("PxuiRewrite", size_of::<PxuiRewrite>(), align_of::<PxuiRewrite>(), &[("text", core::mem::offset_of!(PxuiRewrite, text)), ("caret", core::mem::offset_of!(PxuiRewrite, caret))]);
        check("PxuiTextAreaDesc", size_of::<PxuiTextAreaDesc>(), align_of::<PxuiTextAreaDesc>(), &[("size", core::mem::offset_of!(PxuiTextAreaDesc, size)), ("flags", core::mem::offset_of!(PxuiTextAreaDesc, flags)), ("x", core::mem::offset_of!(PxuiTextAreaDesc, x)), ("y", core::mem::offset_of!(PxuiTextAreaDesc, y)), ("width", core::mem::offset_of!(PxuiTextAreaDesc, width)), ("height", core::mem::offset_of!(PxuiTextAreaDesc, height)), ("reveal", core::mem::offset_of!(PxuiTextAreaDesc, reveal)), ("reveal_nonce", core::mem::offset_of!(PxuiTextAreaDesc, reveal_nonce)), ("select", core::mem::offset_of!(PxuiTextAreaDesc, select)), ("label", core::mem::offset_of!(PxuiTextAreaDesc, label)), ("value", core::mem::offset_of!(PxuiTextAreaDesc, value)), ("errors", core::mem::offset_of!(PxuiTextAreaDesc, errors)), ("messages", core::mem::offset_of!(PxuiTextAreaDesc, messages)), ("spans", core::mem::offset_of!(PxuiTextAreaDesc, spans)), ("splice", core::mem::offset_of!(PxuiTextAreaDesc, splice)), ("chips", core::mem::offset_of!(PxuiTextAreaDesc, chips)), ("comment", core::mem::offset_of!(PxuiTextAreaDesc, comment)), ("pairs", core::mem::offset_of!(PxuiTextAreaDesc, pairs))]);
        check("PxuiAreaCallbacks", size_of::<PxuiAreaCallbacks>(), align_of::<PxuiAreaCallbacks>(), &[("size", core::mem::offset_of!(PxuiAreaCallbacks, size)), ("reserved", core::mem::offset_of!(PxuiAreaCallbacks, reserved)), ("user", core::mem::offset_of!(PxuiAreaCallbacks, user)), ("on_context", core::mem::offset_of!(PxuiAreaCallbacks, on_context)), ("on_scrub", core::mem::offset_of!(PxuiAreaCallbacks, on_scrub)), ("on_scrub_edit", core::mem::offset_of!(PxuiAreaCallbacks, on_scrub_edit)), ("on_click", core::mem::offset_of!(PxuiAreaCallbacks, on_click)), ("on_caret", core::mem::offset_of!(PxuiAreaCallbacks, on_caret)), ("on_caret_move", core::mem::offset_of!(PxuiAreaCallbacks, on_caret_move)), ("on_select", core::mem::offset_of!(PxuiAreaCallbacks, on_select)), ("on_drop", core::mem::offset_of!(PxuiAreaCallbacks, on_drop)), ("colorize", core::mem::offset_of!(PxuiAreaCallbacks, colorize)), ("brackets", core::mem::offset_of!(PxuiAreaCallbacks, brackets)), ("indent", core::mem::offset_of!(PxuiAreaCallbacks, indent)), ("complete", core::mem::offset_of!(PxuiAreaCallbacks, complete)), ("describe", core::mem::offset_of!(PxuiAreaCallbacks, describe)), ("number_at", core::mem::offset_of!(PxuiAreaCallbacks, number_at)), ("context_at", core::mem::offset_of!(PxuiAreaCallbacks, context_at)), ("rewrite", core::mem::offset_of!(PxuiAreaCallbacks, rewrite))]);
        check("PxuiSwatch", size_of::<PxuiSwatch>(), align_of::<PxuiSwatch>(), &[("has", core::mem::offset_of!(PxuiSwatch, has)), ("reserved", core::mem::offset_of!(PxuiSwatch, reserved)), ("color", core::mem::offset_of!(PxuiSwatch, color))]);
        check("PxuiSubmenu", size_of::<PxuiSubmenu>(), align_of::<PxuiSubmenu>(), &[("row", core::mem::offset_of!(PxuiSubmenu, row)), ("current", core::mem::offset_of!(PxuiSubmenu, current)), ("has_current", core::mem::offset_of!(PxuiSubmenu, has_current)), ("reserved", core::mem::offset_of!(PxuiSubmenu, reserved)), ("rows", core::mem::offset_of!(PxuiSubmenu, rows)), ("enabled", core::mem::offset_of!(PxuiSubmenu, enabled)), ("keys", core::mem::offset_of!(PxuiSubmenu, keys))]);
        check("PxuiMenuExtra", size_of::<PxuiMenuExtra>(), align_of::<PxuiMenuExtra>(), &[("size", core::mem::offset_of!(PxuiMenuExtra, size)), ("reserved", core::mem::offset_of!(PxuiMenuExtra, reserved)), ("lead_from", core::mem::offset_of!(PxuiMenuExtra, lead_from)), ("keys", core::mem::offset_of!(PxuiMenuExtra, keys)), ("swatches", core::mem::offset_of!(PxuiMenuExtra, swatches)), ("danger", core::mem::offset_of!(PxuiMenuExtra, danger)), ("submenus", core::mem::offset_of!(PxuiMenuExtra, submenus)), ("row_h", core::mem::offset_of!(PxuiMenuExtra, row_h)), ("max_rows", core::mem::offset_of!(PxuiMenuExtra, max_rows)), ("checked", core::mem::offset_of!(PxuiMenuExtra, checked))]);
        check("Slice", size_of::<Slice<u32>>(), align_of::<Slice<u32>>(), &[("ptr", core::mem::offset_of!(Slice<u32>, ptr)), ("len", core::mem::offset_of!(Slice<u32>, len))]);
        check("Input", size_of::<Input>(), align_of::<Input>(), &[("size", core::mem::offset_of!(Input, size)), ("time", core::mem::offset_of!(Input, time)), ("dt", core::mem::offset_of!(Input, dt)), ("frame", core::mem::offset_of!(Input, frame)), ("tick", core::mem::offset_of!(Input, tick)), ("logical_size", core::mem::offset_of!(Input, logical_size)), ("pointer", core::mem::offset_of!(Input, pointer)), ("keys", core::mem::offset_of!(Input, keys)), ("buttons", core::mem::offset_of!(Input, buttons)), ("events", core::mem::offset_of!(Input, events)), ("pixel_scale", core::mem::offset_of!(Input, pixel_scale)), ("mouse_delta", core::mem::offset_of!(Input, mouse_delta)), ("fps", core::mem::offset_of!(Input, fps))]);
        check("PlatformOut", size_of::<PlatformOut>(), align_of::<PlatformOut>(), &[("size", core::mem::offset_of!(PlatformOut, size)), ("cursor", core::mem::offset_of!(PlatformOut, cursor)), ("has_cursor", core::mem::offset_of!(PlatformOut, has_cursor)), ("has_ime", core::mem::offset_of!(PlatformOut, has_ime)), ("has_background", core::mem::offset_of!(PlatformOut, has_background)), ("has_clipboard", core::mem::offset_of!(PlatformOut, has_clipboard)), ("ime_rect", core::mem::offset_of!(PlatformOut, ime_rect)), ("ime_caret", core::mem::offset_of!(PlatformOut, ime_caret)), ("clipboard", core::mem::offset_of!(PlatformOut, clipboard)), ("background", core::mem::offset_of!(PlatformOut, background)), ("paste", core::mem::offset_of!(PlatformOut, paste)), ("reserved", core::mem::offset_of!(PlatformOut, reserved))]);
        check("Damage", size_of::<Damage>(), align_of::<Damage>(), &[("has_damage", core::mem::offset_of!(Damage, has_damage)), ("reserved", core::mem::offset_of!(Damage, reserved)), ("rect", core::mem::offset_of!(Damage, rect))]);
        check("A11yValue", size_of::<A11yValue>(), align_of::<A11yValue>(), &[("tag", core::mem::offset_of!(A11yValue, tag)), ("text_start", core::mem::offset_of!(A11yValue, text_start)), ("text_end", core::mem::offset_of!(A11yValue, text_end)), ("reserved", core::mem::offset_of!(A11yValue, reserved)), ("now", core::mem::offset_of!(A11yValue, now)), ("min", core::mem::offset_of!(A11yValue, min)), ("max", core::mem::offset_of!(A11yValue, max)), ("step", core::mem::offset_of!(A11yValue, step))]);
        check("A11yNode", size_of::<A11yNode>(), align_of::<A11yNode>(), &[("id", core::mem::offset_of!(A11yNode, id)), ("parent", core::mem::offset_of!(A11yNode, parent)), ("role", core::mem::offset_of!(A11yNode, role)), ("name_start", core::mem::offset_of!(A11yNode, name_start)), ("name_end", core::mem::offset_of!(A11yNode, name_end)), ("description_start", core::mem::offset_of!(A11yNode, description_start)), ("description_end", core::mem::offset_of!(A11yNode, description_end)), ("value", core::mem::offset_of!(A11yNode, value)), ("rect", core::mem::offset_of!(A11yNode, rect)), ("states", core::mem::offset_of!(A11yNode, states)), ("actions", core::mem::offset_of!(A11yNode, actions)), ("selection", core::mem::offset_of!(A11yNode, selection)), ("child_count", core::mem::offset_of!(A11yNode, child_count)), ("first_child", core::mem::offset_of!(A11yNode, first_child)), ("next_sibling", core::mem::offset_of!(A11yNode, next_sibling)), ("last_child", core::mem::offset_of!(A11yNode, last_child))]);
        check("A11yAction", size_of::<A11yAction>(), align_of::<A11yAction>(), &[("id", core::mem::offset_of!(A11yAction, id)), ("action_code", core::mem::offset_of!(A11yAction, action_code)), ("reserved", core::mem::offset_of!(A11yAction, reserved)), ("payload", core::mem::offset_of!(A11yAction, payload)), ("end", core::mem::offset_of!(A11yAction, end)), ("reserved_tail", core::mem::offset_of!(A11yAction, reserved_tail))]);
        check("A11yTree", size_of::<A11yTree>(), align_of::<A11yTree>(), &[("nodes", core::mem::offset_of!(A11yTree, nodes)), ("strings", core::mem::offset_of!(A11yTree, strings)), ("revision", core::mem::offset_of!(A11yTree, revision)), ("first_child", core::mem::offset_of!(A11yTree, first_child)), ("child_count", core::mem::offset_of!(A11yTree, child_count)), ("actions", core::mem::offset_of!(A11yTree, actions)), ("actions.len", core::mem::offset_of!(A11yTree, actions.len))]);
        check("Output", size_of::<Output>(), align_of::<Output>(), &[("size", core::mem::offset_of!(Output, size)), ("reserved", core::mem::offset_of!(Output, reserved)), ("draw_revision", core::mem::offset_of!(Output, draw_revision)), ("repaint_at", core::mem::offset_of!(Output, repaint_at)), ("built_nodes", core::mem::offset_of!(Output, built_nodes)), ("rebuilt_views", core::mem::offset_of!(Output, rebuilt_views)), ("arranged_nodes", core::mem::offset_of!(Output, arranged_nodes)), ("platform", core::mem::offset_of!(Output, platform)), ("damage", core::mem::offset_of!(Output, damage)), ("a11y", core::mem::offset_of!(Output, a11y)), ("layers", core::mem::offset_of!(Output, layers))]);
        check("PxuiLayerInfo", size_of::<PxuiLayerInfo>(), align_of::<PxuiLayerInfo>(), &[("key", core::mem::offset_of!(PxuiLayerInfo, key)), ("id", core::mem::offset_of!(PxuiLayerInfo, id)), ("breaks", core::mem::offset_of!(PxuiLayerInfo, breaks)), ("instances", core::mem::offset_of!(PxuiLayerInfo, instances)), ("dirty", core::mem::offset_of!(PxuiLayerInfo, dirty))]);
        check("PxuiDirty", size_of::<PxuiDirty>(), align_of::<PxuiDirty>(), &[("start", core::mem::offset_of!(PxuiDirty, start)), ("end", core::mem::offset_of!(PxuiDirty, end))]);
        check("ThemeSource", size_of::<ThemeSource>(), align_of::<ThemeSource>(), &[("size", core::mem::offset_of!(ThemeSource, size)), ("flags", core::mem::offset_of!(ThemeSource, flags)), ("source", core::mem::offset_of!(ThemeSource, source))]);
        check("PxuiMotion", size_of::<PxuiMotion>(), align_of::<PxuiMotion>(), &[("fast", core::mem::offset_of!(PxuiMotion, fast)), ("normal", core::mem::offset_of!(PxuiMotion, normal)), ("slow", core::mem::offset_of!(PxuiMotion, slow)), ("ease", core::mem::offset_of!(PxuiMotion, ease))]);
        check("PxuiThemeTokens", size_of::<ThemeTokens>(), align_of::<ThemeTokens>(), &[("size", core::mem::offset_of!(ThemeTokens, size)), ("panel", core::mem::offset_of!(ThemeTokens, panel)), ("foreground", core::mem::offset_of!(ThemeTokens, foreground)), ("control", core::mem::offset_of!(ThemeTokens, control)), ("input", core::mem::offset_of!(ThemeTokens, input)), ("track", core::mem::offset_of!(ThemeTokens, track)), ("accent", core::mem::offset_of!(ThemeTokens, accent)), ("ink_ground", core::mem::offset_of!(ThemeTokens, ink_ground)), ("has_ink_ground", core::mem::offset_of!(ThemeTokens, has_ink_ground)), ("reserved", core::mem::offset_of!(ThemeTokens, reserved)), ("space", core::mem::offset_of!(ThemeTokens, space)), ("radius", core::mem::offset_of!(ThemeTokens, radius)), ("display_size", core::mem::offset_of!(ThemeTokens, display_size)), ("title_size", core::mem::offset_of!(ThemeTokens, title_size)), ("font_size", core::mem::offset_of!(ThemeTokens, font_size)), ("label_size", core::mem::offset_of!(ThemeTokens, label_size)), ("motion", core::mem::offset_of!(ThemeTokens, motion)), ("reduced_motion", core::mem::offset_of!(ThemeTokens, reduced_motion)), ("reserved_tail", core::mem::offset_of!(ThemeTokens, reserved_tail)), ("density", core::mem::offset_of!(ThemeTokens, density))]);
        check("PxuiDialogOut", size_of::<PxuiDialogOut>(), align_of::<PxuiDialogOut>(), &[("tag", core::mem::offset_of!(PxuiDialogOut, tag)), ("index", core::mem::offset_of!(PxuiDialogOut, index))]);
        check("PxuiSizeDesc", size_of::<PxuiSizeDesc>(), align_of::<PxuiSizeDesc>(), &[("kind", core::mem::offset_of!(PxuiSizeDesc, kind)), ("value", core::mem::offset_of!(PxuiSizeDesc, value)), ("relative", core::mem::offset_of!(PxuiSizeDesc, relative))]);
        check("PxuiAnchorDesc", size_of::<PxuiAnchorDesc>(), align_of::<PxuiAnchorDesc>(), &[("has_anchor", core::mem::offset_of!(PxuiAnchorDesc, has_anchor)), ("rect", core::mem::offset_of!(PxuiAnchorDesc, rect)), ("side", core::mem::offset_of!(PxuiAnchorDesc, side)), ("align", core::mem::offset_of!(PxuiAnchorDesc, align)), ("gap", core::mem::offset_of!(PxuiAnchorDesc, gap)), ("margin", core::mem::offset_of!(PxuiAnchorDesc, margin)), ("flip", core::mem::offset_of!(PxuiAnchorDesc, flip)), ("reserved_tail", core::mem::offset_of!(PxuiAnchorDesc, reserved_tail))]);
        check("PxuiFontDesc", size_of::<PxuiFontDesc>(), align_of::<PxuiFontDesc>(), &[("family", core::mem::offset_of!(PxuiFontDesc, family)), ("weight", core::mem::offset_of!(PxuiFontDesc, weight)), ("size", core::mem::offset_of!(PxuiFontDesc, size)), ("tracking", core::mem::offset_of!(PxuiFontDesc, tracking)), ("caps", core::mem::offset_of!(PxuiFontDesc, caps)), ("reserved", core::mem::offset_of!(PxuiFontDesc, reserved))]);
        check("PxuiShadowDesc", size_of::<PxuiShadowDesc>(), align_of::<PxuiShadowDesc>(), &[("color", core::mem::offset_of!(PxuiShadowDesc, color)), ("offset", core::mem::offset_of!(PxuiShadowDesc, offset)), ("blur", core::mem::offset_of!(PxuiShadowDesc, blur)), ("spread", core::mem::offset_of!(PxuiShadowDesc, spread)), ("inner", core::mem::offset_of!(PxuiShadowDesc, inner)), ("reserved", core::mem::offset_of!(PxuiShadowDesc, reserved))]);
        check("PxuiRingDesc", size_of::<PxuiRingDesc>(), align_of::<PxuiRingDesc>(), &[("color", core::mem::offset_of!(PxuiRingDesc, color)), ("width", core::mem::offset_of!(PxuiRingDesc, width)), ("sides", core::mem::offset_of!(PxuiRingDesc, sides))]);
        check("PxuiLookDesc", size_of::<PxuiLookDesc>(), align_of::<PxuiLookDesc>(), &[("fill", core::mem::offset_of!(PxuiLookDesc, fill)), ("gradient", core::mem::offset_of!(PxuiLookDesc, gradient)), ("has_gradient", core::mem::offset_of!(PxuiLookDesc, has_gradient)), ("angle", core::mem::offset_of!(PxuiLookDesc, angle)), ("border", core::mem::offset_of!(PxuiLookDesc, border)), ("border_w", core::mem::offset_of!(PxuiLookDesc, border_w)), ("sides", core::mem::offset_of!(PxuiLookDesc, sides)), ("radius", core::mem::offset_of!(PxuiLookDesc, radius)), ("shadows", core::mem::offset_of!(PxuiLookDesc, shadows)), ("text", core::mem::offset_of!(PxuiLookDesc, text)), ("font", core::mem::offset_of!(PxuiLookDesc, font)), ("opacity", core::mem::offset_of!(PxuiLookDesc, opacity)), ("ring", core::mem::offset_of!(PxuiLookDesc, ring)), ("selection", core::mem::offset_of!(PxuiLookDesc, selection)), ("caret", core::mem::offset_of!(PxuiLookDesc, caret)), ("dashed", core::mem::offset_of!(PxuiLookDesc, dashed)), ("reserved_tail", core::mem::offset_of!(PxuiLookDesc, reserved_tail))]);
        check("PxuiMarkDesc", size_of::<PxuiMarkDesc>(), align_of::<PxuiMarkDesc>(), &[("kind", core::mem::offset_of!(PxuiMarkDesc, kind)), ("direction", core::mem::offset_of!(PxuiMarkDesc, direction)), ("width", core::mem::offset_of!(PxuiMarkDesc, width)), ("rising", core::mem::offset_of!(PxuiMarkDesc, rising)), ("reserved", core::mem::offset_of!(PxuiMarkDesc, reserved))]);
        check("PxuiStateDesc", size_of::<PxuiStateDesc>(), align_of::<PxuiStateDesc>(), &[("hovered", core::mem::offset_of!(PxuiStateDesc, hovered)), ("pressed", core::mem::offset_of!(PxuiStateDesc, pressed)), ("focused", core::mem::offset_of!(PxuiStateDesc, focused)), ("disabled", core::mem::offset_of!(PxuiStateDesc, disabled)), ("checked", core::mem::offset_of!(PxuiStateDesc, checked)), ("reserved", core::mem::offset_of!(PxuiStateDesc, reserved))]);
        check("PxuiCurveDesc", size_of::<PxuiCurveDesc>(), align_of::<PxuiCurveDesc>(), &[("kind", core::mem::offset_of!(PxuiCurveDesc, kind)), ("duration", core::mem::offset_of!(PxuiCurveDesc, duration)), ("points", core::mem::offset_of!(PxuiCurveDesc, points)), ("omega", core::mem::offset_of!(PxuiCurveDesc, omega)), ("count", core::mem::offset_of!(PxuiCurveDesc, count)), ("positions", core::mem::offset_of!(PxuiCurveDesc, positions)), ("values", core::mem::offset_of!(PxuiCurveDesc, values))]);
        check("PxuiSaved", size_of::<PxuiSaved>(), align_of::<PxuiSaved>(), &[("size", core::mem::offset_of!(PxuiSaved, size)), ("reserved", core::mem::offset_of!(PxuiSaved, reserved)), ("parents", core::mem::offset_of!(PxuiSaved, parents)), ("seeds", core::mem::offset_of!(PxuiSaved, seeds))]);
        check("PxuiSignal", size_of::<PxuiSignal>(), align_of::<PxuiSignal>(), &[("size", core::mem::offset_of!(PxuiSignal, size)), ("flags", core::mem::offset_of!(PxuiSignal, flags)), ("has_subtree_press", core::mem::offset_of!(PxuiSignal, has_subtree_press)), ("button", core::mem::offset_of!(PxuiSignal, button)), ("has_a11y_value", core::mem::offset_of!(PxuiSignal, has_a11y_value)), ("has_a11y_select", core::mem::offset_of!(PxuiSignal, has_a11y_select)), ("reserved", core::mem::offset_of!(PxuiSignal, reserved)), ("subtree_press", core::mem::offset_of!(PxuiSignal, subtree_press)), ("clicks", core::mem::offset_of!(PxuiSignal, clicks)), ("drag", core::mem::offset_of!(PxuiSignal, drag)), ("pointer", core::mem::offset_of!(PxuiSignal, pointer)), ("press_point", core::mem::offset_of!(PxuiSignal, press_point)), ("release_point", core::mem::offset_of!(PxuiSignal, release_point)), ("scroll", core::mem::offset_of!(PxuiSignal, scroll)), ("pinch", core::mem::offset_of!(PxuiSignal, pinch)), ("a11y_value", core::mem::offset_of!(PxuiSignal, a11y_value)), ("a11y_select", core::mem::offset_of!(PxuiSignal, a11y_select)), ("keys", core::mem::offset_of!(PxuiSignal, keys))]);
        check("PxuiScrubOut", size_of::<PxuiScrubOut>(), align_of::<PxuiScrubOut>(), &[("size", core::mem::offset_of!(PxuiScrubOut, size)), ("flags", core::mem::offset_of!(PxuiScrubOut, flags)), ("node", core::mem::offset_of!(PxuiScrubOut, node)), ("preview", core::mem::offset_of!(PxuiScrubOut, preview)), ("commit", core::mem::offset_of!(PxuiScrubOut, commit))]);
        check("PxuiGestureIn", size_of::<PxuiGestureIn>(), align_of::<PxuiGestureIn>(), &[("size", core::mem::offset_of!(PxuiGestureIn, size)), ("kinds", core::mem::offset_of!(PxuiGestureIn, kinds)), ("reveal", core::mem::offset_of!(PxuiGestureIn, reveal)), ("threshold", core::mem::offset_of!(PxuiGestureIn, threshold))]);
        check("PxuiSwipeIn", size_of::<PxuiSwipeIn>(), align_of::<PxuiSwipeIn>(), &[("size", core::mem::offset_of!(PxuiSwipeIn, size)), ("leading", core::mem::offset_of!(PxuiSwipeIn, leading)), ("trailing", core::mem::offset_of!(PxuiSwipeIn, trailing)), ("full", core::mem::offset_of!(PxuiSwipeIn, full))]);
        check("PxuiDismissIn", size_of::<PxuiDismissIn>(), align_of::<PxuiDismissIn>(), &[("size", core::mem::offset_of!(PxuiDismissIn, size)), ("fraction", core::mem::offset_of!(PxuiDismissIn, fraction)), ("velocity", core::mem::offset_of!(PxuiDismissIn, velocity))]);
        check("PxuiDismissOut", size_of::<PxuiDismissOut>(), align_of::<PxuiDismissOut>(), &[("size", core::mem::offset_of!(PxuiDismissOut, size)), ("flags", core::mem::offset_of!(PxuiDismissOut, flags)), ("offset", core::mem::offset_of!(PxuiDismissOut, offset))]);
        check("PxuiGestureOut", size_of::<PxuiGestureOut>(), align_of::<PxuiGestureOut>(), &[("size", core::mem::offset_of!(PxuiGestureOut, size)), ("flags", core::mem::offset_of!(PxuiGestureOut, flags)), ("swipe", core::mem::offset_of!(PxuiGestureOut, swipe)), ("back", core::mem::offset_of!(PxuiGestureOut, back)), ("pull", core::mem::offset_of!(PxuiGestureOut, pull))]);
        check("PxuiBoxDesc", size_of::<PxuiBoxDesc>(), align_of::<PxuiBoxDesc>(), &[("size", core::mem::offset_of!(PxuiBoxDesc, size)), ("axis", core::mem::offset_of!(PxuiBoxDesc, axis)), ("flags", core::mem::offset_of!(PxuiBoxDesc, flags)), ("w", core::mem::offset_of!(PxuiBoxDesc, w)), ("h", core::mem::offset_of!(PxuiBoxDesc, h)), ("max_h", core::mem::offset_of!(PxuiBoxDesc, max_h)), ("padding", core::mem::offset_of!(PxuiBoxDesc, padding)), ("scroll_step", core::mem::offset_of!(PxuiBoxDesc, scroll_step)), ("has_at", core::mem::offset_of!(PxuiBoxDesc, has_at)), ("has_hit_rect", core::mem::offset_of!(PxuiBoxDesc, has_hit_rect)), ("reserved", core::mem::offset_of!(PxuiBoxDesc, reserved)), ("at", core::mem::offset_of!(PxuiBoxDesc, at)), ("hit_rect", core::mem::offset_of!(PxuiBoxDesc, hit_rect)), ("hit", core::mem::offset_of!(PxuiBoxDesc, hit)), ("hit_user", core::mem::offset_of!(PxuiBoxDesc, hit_user)), ("hit_release", core::mem::offset_of!(PxuiBoxDesc, hit_release))]);
        check("PxuiValueFieldDesc", size_of::<PxuiValueFieldDesc>(), align_of::<PxuiValueFieldDesc>(), &[("size", core::mem::offset_of!(PxuiValueFieldDesc, size)), ("flags", core::mem::offset_of!(PxuiValueFieldDesc, flags)), ("font_size", core::mem::offset_of!(PxuiValueFieldDesc, font_size)), ("fraction", core::mem::offset_of!(PxuiValueFieldDesc, fraction)), ("tracking", core::mem::offset_of!(PxuiValueFieldDesc, tracking)), ("line", core::mem::offset_of!(PxuiValueFieldDesc, line)), ("lead_color", core::mem::offset_of!(PxuiValueFieldDesc, lead_color)), ("trail_color", core::mem::offset_of!(PxuiValueFieldDesc, trail_color)), ("reserved", core::mem::offset_of!(PxuiValueFieldDesc, reserved)), ("x", core::mem::offset_of!(PxuiValueFieldDesc, x)), ("y", core::mem::offset_of!(PxuiValueFieldDesc, y)), ("width", core::mem::offset_of!(PxuiValueFieldDesc, width)), ("height", core::mem::offset_of!(PxuiValueFieldDesc, height)), ("display", core::mem::offset_of!(PxuiValueFieldDesc, display)), ("placeholder", core::mem::offset_of!(PxuiValueFieldDesc, placeholder)), ("lead", core::mem::offset_of!(PxuiValueFieldDesc, lead)), ("trail", core::mem::offset_of!(PxuiValueFieldDesc, trail)), ("label", core::mem::offset_of!(PxuiValueFieldDesc, label)), ("value", core::mem::offset_of!(PxuiValueFieldDesc, value))]);
        check("PxuiPaintInfo", size_of::<PxuiPaintInfo>(), align_of::<PxuiPaintInfo>(), &[("size", core::mem::offset_of!(PxuiPaintInfo, size)), ("rtl", core::mem::offset_of!(PxuiPaintInfo, rtl)), ("reserved", core::mem::offset_of!(PxuiPaintInfo, reserved)), ("font_size", core::mem::offset_of!(PxuiPaintInfo, font_size)), ("row_height", core::mem::offset_of!(PxuiPaintInfo, row_height)), ("density", core::mem::offset_of!(PxuiPaintInfo, density)), ("clip", core::mem::offset_of!(PxuiPaintInfo, clip)), ("clip_radius", core::mem::offset_of!(PxuiPaintInfo, clip_radius)), ("theme", core::mem::offset_of!(PxuiPaintInfo, theme))]);
        check("PxuiPopupDesc", size_of::<PxuiPopupDesc>(), align_of::<PxuiPopupDesc>(), &[("size", core::mem::offset_of!(PxuiPopupDesc, size)), ("has_stroke", core::mem::offset_of!(PxuiPopupDesc, has_stroke)), ("has_max_height", core::mem::offset_of!(PxuiPopupDesc, has_max_height)), ("dismiss_initial", core::mem::offset_of!(PxuiPopupDesc, dismiss_initial)), ("attached", core::mem::offset_of!(PxuiPopupDesc, attached)), ("stroke", core::mem::offset_of!(PxuiPopupDesc, stroke)), ("max_height", core::mem::offset_of!(PxuiPopupDesc, max_height)), ("x", core::mem::offset_of!(PxuiPopupDesc, x)), ("y", core::mem::offset_of!(PxuiPopupDesc, y)), ("width", core::mem::offset_of!(PxuiPopupDesc, width)), ("height", core::mem::offset_of!(PxuiPopupDesc, height)), ("label", core::mem::offset_of!(PxuiPopupDesc, label))]);
        check("PxuiButtonDesc", size_of::<PxuiButtonDesc>(), align_of::<PxuiButtonDesc>(), &[("size", core::mem::offset_of!(PxuiButtonDesc, size)), ("enabled", core::mem::offset_of!(PxuiButtonDesc, enabled)), ("selected", core::mem::offset_of!(PxuiButtonDesc, selected)), ("danger", core::mem::offset_of!(PxuiButtonDesc, danger)), ("justify", core::mem::offset_of!(PxuiButtonDesc, justify)), ("w_kind", core::mem::offset_of!(PxuiButtonDesc, w_kind)), ("h_kind", core::mem::offset_of!(PxuiButtonDesc, h_kind)), ("has_name", core::mem::offset_of!(PxuiButtonDesc, has_name)), ("reserved", core::mem::offset_of!(PxuiButtonDesc, reserved)), ("w", core::mem::offset_of!(PxuiButtonDesc, w)), ("h", core::mem::offset_of!(PxuiButtonDesc, h)), ("padding", core::mem::offset_of!(PxuiButtonDesc, padding)), ("states", core::mem::offset_of!(PxuiButtonDesc, states)), ("key", core::mem::offset_of!(PxuiButtonDesc, key)), ("text", core::mem::offset_of!(PxuiButtonDesc, text)), ("name", core::mem::offset_of!(PxuiButtonDesc, name))]);
        check("PxuiResponse", size_of::<PxuiResponse>(), align_of::<PxuiResponse>(), &[("size", core::mem::offset_of!(PxuiResponse, size)), ("flags", core::mem::offset_of!(PxuiResponse, flags)), ("node", core::mem::offset_of!(PxuiResponse, node))]);
        check("PxuiIconButtonDesc", size_of::<PxuiIconButtonDesc>(), align_of::<PxuiIconButtonDesc>(), &[("size", core::mem::offset_of!(PxuiIconButtonDesc, size)), ("enabled", core::mem::offset_of!(PxuiIconButtonDesc, enabled)), ("w_kind", core::mem::offset_of!(PxuiIconButtonDesc, w_kind)), ("h_kind", core::mem::offset_of!(PxuiIconButtonDesc, h_kind)), ("reserved", core::mem::offset_of!(PxuiIconButtonDesc, reserved)), ("w", core::mem::offset_of!(PxuiIconButtonDesc, w)), ("h", core::mem::offset_of!(PxuiIconButtonDesc, h)), ("press_scale", core::mem::offset_of!(PxuiIconButtonDesc, press_scale)), ("states", core::mem::offset_of!(PxuiIconButtonDesc, states)), ("key", core::mem::offset_of!(PxuiIconButtonDesc, key)), ("name", core::mem::offset_of!(PxuiIconButtonDesc, name)), ("icon", core::mem::offset_of!(PxuiIconButtonDesc, icon)), ("draw", core::mem::offset_of!(PxuiIconButtonDesc, draw)), ("draw_user", core::mem::offset_of!(PxuiIconButtonDesc, draw_user))]);
        check("PxuiToggleDesc", size_of::<PxuiToggleDesc>(), align_of::<PxuiToggleDesc>(), &[("size", core::mem::offset_of!(PxuiToggleDesc, size)), ("enabled", core::mem::offset_of!(PxuiToggleDesc, enabled)), ("on", core::mem::offset_of!(PxuiToggleDesc, on)), ("press_row", core::mem::offset_of!(PxuiToggleDesc, press_row)), ("reserved", core::mem::offset_of!(PxuiToggleDesc, reserved)), ("row_h", core::mem::offset_of!(PxuiToggleDesc, row_h)), ("switch_w", core::mem::offset_of!(PxuiToggleDesc, switch_w)), ("switch_h", core::mem::offset_of!(PxuiToggleDesc, switch_h)), ("inset", core::mem::offset_of!(PxuiToggleDesc, inset)), ("track", core::mem::offset_of!(PxuiToggleDesc, track)), ("knob", core::mem::offset_of!(PxuiToggleDesc, knob)), ("key", core::mem::offset_of!(PxuiToggleDesc, key)), ("text", core::mem::offset_of!(PxuiToggleDesc, text)), ("detail", core::mem::offset_of!(PxuiToggleDesc, detail))]);
        check("PxuiSegmentedDesc", size_of::<PxuiSegmentedDesc>(), align_of::<PxuiSegmentedDesc>(), &[("size", core::mem::offset_of!(PxuiSegmentedDesc, size)), ("enabled", core::mem::offset_of!(PxuiSegmentedDesc, enabled)), ("grow", core::mem::offset_of!(PxuiSegmentedDesc, grow)), ("reserved", core::mem::offset_of!(PxuiSegmentedDesc, reserved)), ("h", core::mem::offset_of!(PxuiSegmentedDesc, h)), ("states", core::mem::offset_of!(PxuiSegmentedDesc, states)), ("key", core::mem::offset_of!(PxuiSegmentedDesc, key)), ("name", core::mem::offset_of!(PxuiSegmentedDesc, name)), ("selected", core::mem::offset_of!(PxuiSegmentedDesc, selected))]);
        check("PxuiHeadActionDesc", size_of::<PxuiHeadActionDesc>(), align_of::<PxuiHeadActionDesc>(), &[("size", core::mem::offset_of!(PxuiHeadActionDesc, size)), ("active", core::mem::offset_of!(PxuiHeadActionDesc, active)), ("usable", core::mem::offset_of!(PxuiHeadActionDesc, usable)), ("reserved", core::mem::offset_of!(PxuiHeadActionDesc, reserved)), ("caption", core::mem::offset_of!(PxuiHeadActionDesc, caption)), ("keycap", core::mem::offset_of!(PxuiHeadActionDesc, keycap))]);
        check("PxuiHeaderDesc", size_of::<PxuiHeaderDesc>(), align_of::<PxuiHeaderDesc>(), &[("size", core::mem::offset_of!(PxuiHeaderDesc, size)), ("has_kind", core::mem::offset_of!(PxuiHeaderDesc, has_kind)), ("has_badge", core::mem::offset_of!(PxuiHeaderDesc, has_badge)), ("has_index", core::mem::offset_of!(PxuiHeaderDesc, has_index)), ("has_reset", core::mem::offset_of!(PxuiHeaderDesc, has_reset)), ("reset_enabled", core::mem::offset_of!(PxuiHeaderDesc, reset_enabled)), ("reserved", core::mem::offset_of!(PxuiHeaderDesc, reserved)), ("key", core::mem::offset_of!(PxuiHeaderDesc, key)), ("kind", core::mem::offset_of!(PxuiHeaderDesc, kind)), ("badge", core::mem::offset_of!(PxuiHeaderDesc, badge)), ("index", core::mem::offset_of!(PxuiHeaderDesc, index)), ("reset", core::mem::offset_of!(PxuiHeaderDesc, reset)), ("title", core::mem::offset_of!(PxuiHeaderDesc, title)), ("detail", core::mem::offset_of!(PxuiHeaderDesc, detail)), ("actions", core::mem::offset_of!(PxuiHeaderDesc, actions))]);
        check("PxuiHeadOut", size_of::<PxuiHeadOut>(), align_of::<PxuiHeadOut>(), &[("size", core::mem::offset_of!(PxuiHeadOut, size)), ("has_chosen", core::mem::offset_of!(PxuiHeadOut, has_chosen)), ("reset_pressed", core::mem::offset_of!(PxuiHeadOut, reset_pressed)), ("reserved", core::mem::offset_of!(PxuiHeadOut, reserved)), ("renamed", core::mem::offset_of!(PxuiHeadOut, renamed)), ("chosen", core::mem::offset_of!(PxuiHeadOut, chosen))]);
    }
}
