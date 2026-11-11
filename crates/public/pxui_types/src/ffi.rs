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
pub struct PxuiDirty { pub start: u32, pub end: u32 }

/// One layer of the last frame (a floating root's, drawn in order): the key of its root box, its retained id, its instances (an
/// empty layer draws nothing), how many texture breaks they hold and the instance ranges changed since it was last drawn. Borrowed
/// until the next frame.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiLayerInfo { pub key: i64, pub id: u32, pub breaks: u32, pub instances: Slice<crate::draw::Instance>, pub dirty: Slice<PxuiDirty> }

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Damage { pub has_damage: u8, pub reserved: [u8; 3], pub rect: Rect }

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
    pub selection: [u32; 2],
    pub child_count: u32,
    pub first_child: u32,
    pub next_sibling: u32,
    pub last_child: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct A11yAction { pub id: u64, pub action_code: u32, pub reserved: u32, pub payload: f64, pub end: u32, pub reserved_tail: u32 }

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
pub struct ThemeSource { pub size: u32, pub flags: u32, pub source: Str }
impl Default for ThemeSource { fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, flags: 0, source: Str::default() } } }

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiMotion { pub fast: f32, pub normal: f32, pub slow: f32, pub ease: [f32; 4] }

/// Stable snapshot of Gallery paint tokens; independent of the private theme graph.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ThemeTokens {
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
pub struct PxuiDialogOut { pub tag: u32, pub index: usize }

/// Small snapshot of the theme values used by the public Gallery surface.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiThemeSnapshot {
    pub size: u32, pub part_count: u32, pub tokens: ThemeTokens,
    pub row_h: f32, pub control_h: f32, pub scrollbar_w: f32, pub scrollbar_gap: f32, pub ring: PxuiRingDesc,
}

/// The fields of a declared node an app edits in place after its signal is known (`Ui::node_mut`): its look, its mark and the
/// inset of its text.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiNodeFields { pub size: u32, pub reserved: u32, pub look: PxuiLookDesc, pub mark: PxuiMarkDesc, pub inset: Edges }

/// A byte range `start..end` of a text.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiRange { pub start: i64, pub end: i64 }

/// A byte range with a colour: a syntax span or a chip.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiColorSpan { pub start: i64, pub end: i64, pub color: Rgba, pub reserved: u32 }

/// A message under a line of a text area, optionally pointing at a byte range of it.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiNote { pub line: i64, pub start: i64, pub end: i64, pub has_range: u8, pub reserved: [u8; 7], pub text: Str }

/// A replacement of bytes `start..end` of the text with `text`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiSplice { pub start: i64, pub end: i64, pub text: Str }

/// One suggestion of a language's `complete`: the byte range `insert` replaces, the row, its detail and its doc line.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiCompletion { pub start: i64, pub end: i64, pub insert: Str, pub label: Str, pub detail: Str, pub doc: Str }

/// A token's byte range and its description (`has` is 0 for none).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiDescription { pub has: u8, pub reserved: [u8; 7], pub start: i64, pub end: i64, pub text: Str }

/// An optional byte range (`has` is 0 for none).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiMaybeRange { pub has: u8, pub reserved: [u8; 7], pub start: i64, pub end: i64 }

/// What a language's `rewrite` returns: the text it keeps and where the caret lands in it.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiRewrite { pub text: Str, pub caret: i64 }

/// A text area (`TextAreaOpts`): its place, its text and the data options. `flags` are the `PxuiTextAreaDesc::*` bits.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiTextAreaDesc {
    pub size: u32, pub flags: u32, pub x: f32, pub y: f32, pub width: f32, pub height: f32,
    pub reveal: i64, pub reveal_nonce: i64, pub select: PxuiRange, pub label: Str, pub value: Str,
    pub errors: Slice<i64>, pub messages: Slice<PxuiNote>, pub spans: Slice<PxuiRange>, pub splice: Slice<PxuiSplice>, pub chips: Slice<PxuiColorSpan>,
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
    pub size: u32, pub reserved: u32, pub user: *mut core::ffi::c_void,
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
pub struct PxuiSwatch { pub has: u8, pub reserved: [u8; 3], pub color: Rgba }

/// A submenu of a context menu: the row of the menu that opens it, its rows (`enabled` is as long as `rows`), their keys
/// and the row marked with the accent square (when `has_current`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiSubmenu { pub row: i64, pub current: i64, pub has_current: u8, pub reserved: [u8; 7], pub rows: Slice<Str>, pub enabled: Slice<u8>, pub keys: Slice<Str> }

/// What a context menu has beyond its rows (`MenuOpts`): keys and swatches are per row (shorter than the rows is fine),
/// `danger` the destructive rows, `lead_from` the first row that is not drawn with a leading mark.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiMenuExtra { pub size: u32, pub reserved: u32, pub lead_from: i64, pub keys: Slice<Str>, pub swatches: Slice<PxuiSwatch>, pub danger: Slice<i64>, pub submenus: Slice<PxuiSubmenu> }

/// C mirror of `Size`; kinds are Px=0, Pct=1, Rel=2, Grow=3, Fit=4.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiSizeDesc { pub kind: u32, pub value: f32, pub relative: Option<extern "C" fn(f32) -> f32> }
impl PxuiSizeDesc { pub const PX: u32 = 0; pub const PCT: u32 = 1; pub const REL: u32 = 2; pub const GROW: u32 = 3; pub const FIT: u32 = 4; }

/// C mirror of a grid track; kinds are Px=0, Fr=1, Fit=2.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiTrackDesc { pub kind: u32, pub value: f32 }
impl PxuiTrackDesc { pub const PX: u32 = 0; pub const FR: u32 = 1; pub const FIT: u32 = 2; }

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiGridDesc { pub has_grid: u8, pub reserved: [u8; 7], pub cols: Slice<PxuiTrackDesc>, pub rows: Slice<PxuiTrackDesc> }

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiCellDesc { pub col: u16, pub row: u16, pub col_span: u16, pub row_span: u16 }

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiAnchorDesc {
    pub has_anchor: u8, pub reserved: [u8; 3], pub rect: Rect, pub side: u32, pub align: u32,
    pub gap: f32, pub margin: f32, pub flip: u8, pub reserved_tail: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiFontDesc { pub family: u16, pub weight: u16, pub size: i64, pub tracking: f32, pub caps: u8, pub reserved: [u8; 3] }

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiShadowDesc { pub color: Rgba, pub offset: [f32; 2], pub blur: f32, pub spread: f32, pub inner: u8, pub reserved: [u8; 3] }

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiRingDesc { pub color: Rgba, pub width: f32, pub sides: u32 }

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiLookDesc {
    pub fill: Rgba, pub gradient: Rgba, pub has_gradient: u8, pub reserved: [u8; 3], pub angle: f32,
    pub border: Rgba, pub border_w: f32, pub sides: u32, pub radius: [f32; 4], pub shadows: [PxuiShadowDesc; 2],
    pub text: Rgba, pub font: PxuiFontDesc, pub opacity: f32, pub ring: PxuiRingDesc,
    pub selection: Rgba, pub caret: Rgba, pub dashed: u8, pub reserved_tail: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiMarkDesc { pub kind: u32, pub direction: u32, pub width: f32, pub rising: u8, pub reserved: [u8; 3] }

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiBarDesc { pub has_bar: u8, pub whole: u8, pub reserved: [u8; 2], pub color: Rgba, pub radius: f32, pub width: f32, pub gap: f32, pub margin: f32 }

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiStateDesc { pub hovered: u8, pub pressed: u8, pub focused: u8, pub disabled: u8, pub checked: u8, pub reserved: [u8; 3] }

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiCurveDesc { pub kind: u32, pub duration: f32, pub points: [f32; 4], pub omega: f32 }
impl PxuiCurveDesc { pub const NONE: u32 = 0; pub const BEZIER: u32 = 1; pub const SPRING: u32 = 2; }

/// Complete C-safe node input. Optional values use explicit `has_*` fields or tagged kinds above.
/// Enum values follow the corresponding pxui_types enums in declaration order.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct NodeDesc {
    pub size: u32, pub flags: i64, pub w: PxuiSizeDesc, pub h: PxuiSizeDesc,
    pub min_w: f32, pub min_h: f32, pub max_w: f32, pub max_h: f32, pub grow: f32, pub shrink: f32, pub aspect: f32,
    pub axis: u32, pub padding: crate::geom::Edges, pub margin: crate::geom::Edges, pub gap: f32, pub row_gap: f32,
    pub justify: u32, pub align: u32, pub align_self: u32, pub wrap: u8, pub reserved: [u8; 3],
    pub grid: PxuiGridDesc, pub cell: PxuiCellDesc, pub z: i16, pub reserved_z: u16, pub role: u32, pub a11y: u32,
    pub anchor: PxuiAnchorDesc, pub has_at: u32, pub at: [f32; 2], pub scroll_step: f32,
    pub look: PxuiLookDesc, pub inset: crate::geom::Edges, pub mark: PxuiMarkDesc, pub bar: PxuiBarDesc,
    pub state: PxuiStateDesc, pub transition: PxuiCurveDesc,
}
impl NodeDesc {
    pub const AXIS_ROW: u32 = 0;
    pub const AXIS_COLUMN: u32 = 1;
    pub const AXIS_STACK: u32 = 2;
    pub const JUSTIFY_START: u32 = 0;
    pub const JUSTIFY_CENTER: u32 = 1;
    pub const JUSTIFY_END: u32 = 2;
    pub const JUSTIFY_SPACE_BETWEEN: u32 = 3;
    pub const JUSTIFY_SPACE_AROUND: u32 = 4;
    pub const JUSTIFY_SPACE_EVENLY: u32 = 5;
    pub const ALIGN_START: u32 = 0;
    pub const ALIGN_CENTER: u32 = 1;
    pub const ALIGN_END: u32 = 2;
    pub const ALIGN_STRETCH: u32 = 3;
    pub const ALIGN_BASELINE: u32 = 4;
    pub const ALIGN_SELF_NONE: u32 = u32::MAX;
}
impl PxuiMarkDesc {
    pub const NONE: u32 = 0; pub const CHEVRON: u32 = 1; pub const LINE: u32 = 2;
    pub const DIR_DOWN: u32 = 0; pub const DIR_UP: u32 = 1; pub const DIR_LEFT: u32 = 2; pub const DIR_RIGHT: u32 = 3;
}

/// Opaque `enter` state: pass it back to `leave` unchanged.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiSaved { pub size: u32, pub reserved: u32, pub parents: usize, pub seeds: usize }

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
impl Default for PxuiSignal {
    fn default() -> Self {
        Self { size: core::mem::size_of::<Self>() as u32, flags: 0, has_subtree_press: 0, button: u8::MAX, has_a11y_value: 0, has_a11y_select: 0,
            reserved: [0; 4], subtree_press: 0, clicks: 0, drag: [0.0; 2], pointer: [0.0; 2], press_point: [0.0; 2],
            release_point: [0.0; 2], scroll: [0.0; 2], pinch: 1.0, a11y_value: 0.0, a11y_select: [0; 2], keys: Slice::default() }
    }
}
impl PxuiSignal {
    pub const HOVERED: u32 = 1; pub const PRESSED: u32 = 2; pub const HELD: u32 = 4; pub const RELEASED: u32 = 8;
    pub const CLICKED: u32 = 16; pub const DOUBLE_CLICKED: u32 = 32; pub const DRAGGING: u32 = 64;
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
pub type PxuiNodeDesc = NodeDesc;

/// What a paint callback needs to know about its painter, read once per callback: the sizes, the clip and the theme tokens.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiPaintInfo {
    pub size: u32, pub rtl: u8, pub reserved: [u8; 3], pub font_size: i64, pub row_height: i64, pub density: i64,
    pub clip: Rect, pub clip_radius: f32, pub theme: ThemeTokens,
}

/// `value_field`'s place, text and options. Each `HAS_*` flag says whether the option next to it is set; `lead` and `trail` are
/// the framing texts, coloured by `lead_color` and `trail_color`. The slide, scrub and validity callbacks cross beside it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiValueFieldDesc {
    pub size: u32, pub flags: u32, pub font_size: i64, pub fraction: f32, pub tracking: f32,
    pub line: Rgba, pub lead_color: Rgba, pub trail_color: Rgba, pub reserved: [u8; 4],
    pub x: f32, pub y: f32, pub width: f32, pub height: f32,
    pub display: Str, pub placeholder: Str, pub lead: Str, pub trail: Str, pub label: Str, pub value: Str,
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
    pub size: u32, pub has_stroke: u8, pub has_max_height: u8, pub dismiss_initial: u8, pub attached: u8,
    pub stroke: Rgba, pub max_height: f32, pub x: f32, pub y: f32, pub width: f32, pub height: f32, pub label: Str,
}

/// One text button of an inspector head.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiHeadActionDesc { pub caption: Str, pub keycap: Str, pub active: u8, pub usable: u8, pub reserved: [u8; 6] }

/// An inspector head. `kind`, `badge`, `index` and `reset` are read when their flag is set.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiHeaderDesc {
    pub size: u32, pub has_kind: u8, pub has_badge: u8, pub has_index: u8, pub has_reset: u8, pub reset_enabled: u8, pub reserved: [u8; 3],
    pub key: Str, pub kind: Str, pub badge: Str, pub index: Str, pub reset: Str, pub title: Str, pub detail: Str, pub actions: Slice<PxuiHeadActionDesc>,
}

/// What a head reports: the name as edited (borrowed until the next frame), the pressed action (`chosen` when `has_chosen`)
/// and whether `reset` was pressed.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiHeadOut { pub renamed: Str, pub chosen: i64, pub has_chosen: u8, pub reset_pressed: u8, pub reserved: [u8; 6] }

/// `BoxOpts` crossing: `axis` is 0 row, 1 column, 2 stack. `hit` (with `hit_user` and `hit_release`, called once when the
/// box's slot lets go of it) replaces the box's hit rectangle by the one it returns for the laid-out rectangle.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiBoxDesc {
    pub size: u32, pub axis: u32, pub flags: i64, pub w: PxuiSizeDesc, pub h: PxuiSizeDesc,
    pub max_h: f32, pub padding: f32, pub scroll_step: f32, pub has_at: u8, pub has_hit_rect: u8, pub reserved: [u8; 2],
    pub at: [f32; 2], pub hit_rect: Rect,
    pub hit: Option<extern "C" fn(*mut core::ffi::c_void, Rect) -> Rect>, pub hit_user: *mut core::ffi::c_void,
    pub hit_release: Option<extern "C" fn(*mut core::ffi::c_void)>,
}

impl ThemeSource {
    pub const COMPACT: u32 = 1;
    pub const REDUCED_MOTION: u32 = 2;
}


/// A build callback of a container descriptor: `(user, ui)`, the `ui` a borrowed `PxuiUi` valid for the call.
pub type UiBuild = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void)>;

// Descriptors of the kit widgets (`kit.rs` of `pxui_abi` and `pxui`): the one definition both sides use. They carry no `size`:
// a widget that needs more takes a new descriptor.
pub type PxuiBuild = UiBuild;
pub type PxuiGridBuild = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, usize)>;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiMenuDesc { pub title: Str, pub items: Slice<Str> }
#[repr(C)]
pub struct PxuiGridListDesc { pub label: Str, pub count: usize, pub cell_w: f32, pub cell_h: f32, pub build: PxuiGridBuild, pub user: *mut core::ffi::c_void }
#[repr(C)]
pub struct PxuiSplitDesc { pub label: Str, pub axis: u32, pub ratio: f32, pub min_a: f32, pub min_b: f32, pub build_a: PxuiBuild, pub user_a: *mut core::ffi::c_void, pub build_b: PxuiBuild, pub user_b: *mut core::ffi::c_void }
#[repr(C)]
pub struct PxuiMenuBarDesc { pub label: Str, pub menus: Slice<PxuiMenuDesc> }
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiMenuOut { pub menu: usize, pub item: usize }
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiHintDesc { pub key: Str, pub value: Str }
#[repr(C)]
pub struct PxuiFooterDesc { pub has_right: u8, pub reserved: [u8; 7], pub right: Str, pub hints: Slice<PxuiHintDesc> }
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiKeyframeDesc { pub time: f32, pub target: u32, pub reserved: [u8; 4], pub values: [f32; 4], pub look: PxuiLookDesc, pub curve: PxuiCurveDesc }

#[repr(C)]
pub struct PxuiInspectorRowDesc { pub key: Str, pub label: Str, pub has_width: u8, pub has_pin: u8, pub pin: u8, pub reserved: [u8; 5], pub width: f32, pub row: PxuiBox, pub control_x: f32, pub control_y: f32, pub control_w: f32 }
#[repr(C)]
pub struct PxuiInspectorSectionDesc { pub key: Str, pub label: Str, pub expanded: u8, pub has_set: u8, pub set_expanded: u8, pub reserved: [u8; 5], pub build: PxuiBuild, pub user: *mut core::ffi::c_void, pub out_open: u8 }
#[repr(C)]
pub struct PxuiInspectorValueDesc { pub key: Str, pub label: Str, pub value: Str, pub has_width: u8, pub reserved: [u8; 7], pub width: f32 }
#[repr(C)]
pub struct PxuiInspectorTextDesc { pub key: Str, pub text: Str }
pub type PxuiTableCell = Option<extern "C" fn(*mut core::ffi::c_void, i64, i64) -> Str>;
#[repr(C)]
pub struct PxuiTableDesc { pub label: Str, pub at: [f32; 2], pub width: f32, pub height: f32, pub headers: Slice<Str>, pub rows: i64, pub cell: PxuiTableCell, pub user: *mut core::ffi::c_void }

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiU8MutSpan { pub ptr: *mut u8, pub len: usize }
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiF32MutSpan { pub ptr: *mut f32, pub len: usize }
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiU32MutSpan { pub ptr: *mut u32, pub len: usize }

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiTreeNode { pub depth: u16, pub children: u8 }

pub type PxuiCollectionEmit = Option<extern "C" fn(*mut core::ffi::c_void, Str)>;
pub type PxuiCollectionText = Option<extern "C" fn(*mut core::ffi::c_void, usize, usize, PxuiCollectionEmit, *mut core::ffi::c_void)>;

#[repr(C)]
pub struct PxuiListViewDesc {
    pub label: Str,
    pub count: usize,
    pub multi: u8,
    pub reserved: [u8; 7],
    pub selected: PxuiU8MutSpan,
    pub text: PxuiCollectionText,
    pub user: *mut core::ffi::c_void,
}

#[repr(C)]
pub struct PxuiTreeViewDesc {
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
    pub label: Str,
    pub titles: Slice<Str>,
    pub widths: PxuiF32MutSpan,
    pub rows: usize,
    pub has_sort: u8,
    pub ascending: u8,
    pub reserved: [u8; 6],
    pub sort_column: usize,
    pub selected: *mut usize,
    pub text: PxuiCollectionText,
    pub user: *mut core::ffi::c_void,
}


#[cfg(test)]
mod layout {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};
    fn check(name: &str, size: usize, align: usize, offsets: &[usize]) {
        let row = include_str!("../../../../abi/layout.v1.txt").lines().find(|line| line.split_whitespace().next() == Some(name)).unwrap();
        let mut fields = row.split_whitespace().skip(1);
        assert_eq!(fields.next().unwrap().parse::<usize>().unwrap(), size, "{name} size");
        assert_eq!(fields.next().unwrap().parse::<usize>().unwrap(), align, "{name} alignment");
        assert_eq!(fields.map(|v| v.parse::<usize>().unwrap()).collect::<Vec<_>>(), offsets, "{name} offsets");
    }
    #[test]
    fn ffi_layouts_are_pinned() {
        check("PxuiMenuDesc", size_of::<PxuiMenuDesc>(), align_of::<PxuiMenuDesc>(), &[offset_of!(PxuiMenuDesc, title), offset_of!(PxuiMenuDesc, items)]);
        check("PxuiGridListDesc", size_of::<PxuiGridListDesc>(), align_of::<PxuiGridListDesc>(), &[offset_of!(PxuiGridListDesc, label), offset_of!(PxuiGridListDesc, count), offset_of!(PxuiGridListDesc, cell_w), offset_of!(PxuiGridListDesc, cell_h), offset_of!(PxuiGridListDesc, build), offset_of!(PxuiGridListDesc, user)]);
        check("PxuiSplitDesc", size_of::<PxuiSplitDesc>(), align_of::<PxuiSplitDesc>(), &[offset_of!(PxuiSplitDesc, label), offset_of!(PxuiSplitDesc, axis), offset_of!(PxuiSplitDesc, ratio), offset_of!(PxuiSplitDesc, min_a), offset_of!(PxuiSplitDesc, min_b), offset_of!(PxuiSplitDesc, build_a), offset_of!(PxuiSplitDesc, user_a), offset_of!(PxuiSplitDesc, build_b), offset_of!(PxuiSplitDesc, user_b)]);
        check("PxuiMenuBarDesc", size_of::<PxuiMenuBarDesc>(), align_of::<PxuiMenuBarDesc>(), &[offset_of!(PxuiMenuBarDesc, label), offset_of!(PxuiMenuBarDesc, menus)]);
        check("PxuiMenuOut", size_of::<PxuiMenuOut>(), align_of::<PxuiMenuOut>(), &[offset_of!(PxuiMenuOut, menu), offset_of!(PxuiMenuOut, item)]);
        check("PxuiHintDesc", size_of::<PxuiHintDesc>(), align_of::<PxuiHintDesc>(), &[offset_of!(PxuiHintDesc, key), offset_of!(PxuiHintDesc, value)]);
        check("PxuiFooterDesc", size_of::<PxuiFooterDesc>(), align_of::<PxuiFooterDesc>(), &[offset_of!(PxuiFooterDesc, has_right), offset_of!(PxuiFooterDesc, reserved), offset_of!(PxuiFooterDesc, right), offset_of!(PxuiFooterDesc, hints)]);
        check("PxuiKeyframeDesc", size_of::<PxuiKeyframeDesc>(), align_of::<PxuiKeyframeDesc>(), &[offset_of!(PxuiKeyframeDesc, time), offset_of!(PxuiKeyframeDesc, target), offset_of!(PxuiKeyframeDesc, reserved), offset_of!(PxuiKeyframeDesc, values), offset_of!(PxuiKeyframeDesc, look), offset_of!(PxuiKeyframeDesc, curve)]);
        check("PxuiInspectorRowDesc", size_of::<PxuiInspectorRowDesc>(), align_of::<PxuiInspectorRowDesc>(), &[offset_of!(PxuiInspectorRowDesc, key), offset_of!(PxuiInspectorRowDesc, label), offset_of!(PxuiInspectorRowDesc, has_width), offset_of!(PxuiInspectorRowDesc, has_pin), offset_of!(PxuiInspectorRowDesc, pin), offset_of!(PxuiInspectorRowDesc, reserved), offset_of!(PxuiInspectorRowDesc, width), offset_of!(PxuiInspectorRowDesc, row), offset_of!(PxuiInspectorRowDesc, control_x), offset_of!(PxuiInspectorRowDesc, control_y), offset_of!(PxuiInspectorRowDesc, control_w)]);
        check("PxuiInspectorSectionDesc", size_of::<PxuiInspectorSectionDesc>(), align_of::<PxuiInspectorSectionDesc>(), &[offset_of!(PxuiInspectorSectionDesc, key), offset_of!(PxuiInspectorSectionDesc, label), offset_of!(PxuiInspectorSectionDesc, expanded), offset_of!(PxuiInspectorSectionDesc, has_set), offset_of!(PxuiInspectorSectionDesc, set_expanded), offset_of!(PxuiInspectorSectionDesc, reserved), offset_of!(PxuiInspectorSectionDesc, build), offset_of!(PxuiInspectorSectionDesc, user), offset_of!(PxuiInspectorSectionDesc, out_open)]);
        check("PxuiInspectorValueDesc", size_of::<PxuiInspectorValueDesc>(), align_of::<PxuiInspectorValueDesc>(), &[offset_of!(PxuiInspectorValueDesc, key), offset_of!(PxuiInspectorValueDesc, label), offset_of!(PxuiInspectorValueDesc, value), offset_of!(PxuiInspectorValueDesc, has_width), offset_of!(PxuiInspectorValueDesc, reserved), offset_of!(PxuiInspectorValueDesc, width)]);
        check("PxuiInspectorTextDesc", size_of::<PxuiInspectorTextDesc>(), align_of::<PxuiInspectorTextDesc>(), &[offset_of!(PxuiInspectorTextDesc, key), offset_of!(PxuiInspectorTextDesc, text)]);
        check("PxuiTableDesc", size_of::<PxuiTableDesc>(), align_of::<PxuiTableDesc>(), &[offset_of!(PxuiTableDesc, label), offset_of!(PxuiTableDesc, at), offset_of!(PxuiTableDesc, width), offset_of!(PxuiTableDesc, height), offset_of!(PxuiTableDesc, headers), offset_of!(PxuiTableDesc, rows), offset_of!(PxuiTableDesc, cell), offset_of!(PxuiTableDesc, user)]);
        check("PxuiU8MutSpan", size_of::<PxuiU8MutSpan>(), align_of::<PxuiU8MutSpan>(), &[offset_of!(PxuiU8MutSpan, ptr), offset_of!(PxuiU8MutSpan, len)]);
        check("PxuiF32MutSpan", size_of::<PxuiF32MutSpan>(), align_of::<PxuiF32MutSpan>(), &[offset_of!(PxuiF32MutSpan, ptr), offset_of!(PxuiF32MutSpan, len)]);
        check("PxuiU32MutSpan", size_of::<PxuiU32MutSpan>(), align_of::<PxuiU32MutSpan>(), &[offset_of!(PxuiU32MutSpan, ptr), offset_of!(PxuiU32MutSpan, len)]);
        check("PxuiTreeNode", size_of::<PxuiTreeNode>(), align_of::<PxuiTreeNode>(), &[offset_of!(PxuiTreeNode, depth), offset_of!(PxuiTreeNode, children)]);
        check("PxuiListViewDesc", size_of::<PxuiListViewDesc>(), align_of::<PxuiListViewDesc>(), &[offset_of!(PxuiListViewDesc, label), offset_of!(PxuiListViewDesc, count), offset_of!(PxuiListViewDesc, multi), offset_of!(PxuiListViewDesc, reserved), offset_of!(PxuiListViewDesc, selected), offset_of!(PxuiListViewDesc, text), offset_of!(PxuiListViewDesc, user)]);
        check("PxuiTreeViewDesc", size_of::<PxuiTreeViewDesc>(), align_of::<PxuiTreeViewDesc>(), &[offset_of!(PxuiTreeViewDesc, label), offset_of!(PxuiTreeViewDesc, nodes), offset_of!(PxuiTreeViewDesc, open), offset_of!(PxuiTreeViewDesc, selected), offset_of!(PxuiTreeViewDesc, visible), offset_of!(PxuiTreeViewDesc, visible_len), offset_of!(PxuiTreeViewDesc, text), offset_of!(PxuiTreeViewDesc, user)]);
        check("PxuiTableViewDesc", size_of::<PxuiTableViewDesc>(), align_of::<PxuiTableViewDesc>(), &[offset_of!(PxuiTableViewDesc, label), offset_of!(PxuiTableViewDesc, titles), offset_of!(PxuiTableViewDesc, widths), offset_of!(PxuiTableViewDesc, rows), offset_of!(PxuiTableViewDesc, has_sort), offset_of!(PxuiTableViewDesc, ascending), offset_of!(PxuiTableViewDesc, reserved), offset_of!(PxuiTableViewDesc, sort_column), offset_of!(PxuiTableViewDesc, selected), offset_of!(PxuiTableViewDesc, text), offset_of!(PxuiTableViewDesc, user)]);
        check("PxuiNodeFields", size_of::<PxuiNodeFields>(), align_of::<PxuiNodeFields>(), &[offset_of!(PxuiNodeFields, size), offset_of!(PxuiNodeFields, reserved), offset_of!(PxuiNodeFields, look), offset_of!(PxuiNodeFields, mark), offset_of!(PxuiNodeFields, inset)]);
        check("PxuiRange", size_of::<PxuiRange>(), align_of::<PxuiRange>(), &[offset_of!(PxuiRange, start), offset_of!(PxuiRange, end)]);
        check("PxuiColorSpan", size_of::<PxuiColorSpan>(), align_of::<PxuiColorSpan>(), &[offset_of!(PxuiColorSpan, start), offset_of!(PxuiColorSpan, end), offset_of!(PxuiColorSpan, color), offset_of!(PxuiColorSpan, reserved)]);
        check("PxuiNote", size_of::<PxuiNote>(), align_of::<PxuiNote>(), &[offset_of!(PxuiNote, line), offset_of!(PxuiNote, start), offset_of!(PxuiNote, end), offset_of!(PxuiNote, has_range), offset_of!(PxuiNote, reserved), offset_of!(PxuiNote, text)]);
        check("PxuiSplice", size_of::<PxuiSplice>(), align_of::<PxuiSplice>(), &[offset_of!(PxuiSplice, start), offset_of!(PxuiSplice, end), offset_of!(PxuiSplice, text)]);
        check("PxuiCompletion", size_of::<PxuiCompletion>(), align_of::<PxuiCompletion>(), &[offset_of!(PxuiCompletion, start), offset_of!(PxuiCompletion, end), offset_of!(PxuiCompletion, insert), offset_of!(PxuiCompletion, label), offset_of!(PxuiCompletion, detail), offset_of!(PxuiCompletion, doc)]);
        check("PxuiDescription", size_of::<PxuiDescription>(), align_of::<PxuiDescription>(), &[offset_of!(PxuiDescription, has), offset_of!(PxuiDescription, reserved), offset_of!(PxuiDescription, start), offset_of!(PxuiDescription, end), offset_of!(PxuiDescription, text)]);
        check("PxuiMaybeRange", size_of::<PxuiMaybeRange>(), align_of::<PxuiMaybeRange>(), &[offset_of!(PxuiMaybeRange, has), offset_of!(PxuiMaybeRange, reserved), offset_of!(PxuiMaybeRange, start), offset_of!(PxuiMaybeRange, end)]);
        check("PxuiRewrite", size_of::<PxuiRewrite>(), align_of::<PxuiRewrite>(), &[offset_of!(PxuiRewrite, text), offset_of!(PxuiRewrite, caret)]);
        check("PxuiTextAreaDesc", size_of::<PxuiTextAreaDesc>(), align_of::<PxuiTextAreaDesc>(), &[offset_of!(PxuiTextAreaDesc, size), offset_of!(PxuiTextAreaDesc, flags), offset_of!(PxuiTextAreaDesc, x), offset_of!(PxuiTextAreaDesc, y), offset_of!(PxuiTextAreaDesc, width), offset_of!(PxuiTextAreaDesc, height), offset_of!(PxuiTextAreaDesc, reveal), offset_of!(PxuiTextAreaDesc, reveal_nonce), offset_of!(PxuiTextAreaDesc, select), offset_of!(PxuiTextAreaDesc, label), offset_of!(PxuiTextAreaDesc, value), offset_of!(PxuiTextAreaDesc, errors), offset_of!(PxuiTextAreaDesc, messages), offset_of!(PxuiTextAreaDesc, spans), offset_of!(PxuiTextAreaDesc, splice), offset_of!(PxuiTextAreaDesc, chips), offset_of!(PxuiTextAreaDesc, comment), offset_of!(PxuiTextAreaDesc, pairs)]);
        check("PxuiAreaCallbacks", size_of::<PxuiAreaCallbacks>(), align_of::<PxuiAreaCallbacks>(), &[offset_of!(PxuiAreaCallbacks, size), offset_of!(PxuiAreaCallbacks, reserved), offset_of!(PxuiAreaCallbacks, user), offset_of!(PxuiAreaCallbacks, on_context), offset_of!(PxuiAreaCallbacks, on_scrub), offset_of!(PxuiAreaCallbacks, on_scrub_edit), offset_of!(PxuiAreaCallbacks, on_click), offset_of!(PxuiAreaCallbacks, on_caret), offset_of!(PxuiAreaCallbacks, on_caret_move), offset_of!(PxuiAreaCallbacks, on_select), offset_of!(PxuiAreaCallbacks, on_drop), offset_of!(PxuiAreaCallbacks, colorize), offset_of!(PxuiAreaCallbacks, brackets), offset_of!(PxuiAreaCallbacks, indent), offset_of!(PxuiAreaCallbacks, complete), offset_of!(PxuiAreaCallbacks, describe), offset_of!(PxuiAreaCallbacks, number_at), offset_of!(PxuiAreaCallbacks, context_at), offset_of!(PxuiAreaCallbacks, rewrite)]);
        check("PxuiSwatch", size_of::<PxuiSwatch>(), align_of::<PxuiSwatch>(), &[offset_of!(PxuiSwatch, has), offset_of!(PxuiSwatch, reserved), offset_of!(PxuiSwatch, color)]);
        check("PxuiSubmenu", size_of::<PxuiSubmenu>(), align_of::<PxuiSubmenu>(), &[offset_of!(PxuiSubmenu, row), offset_of!(PxuiSubmenu, current), offset_of!(PxuiSubmenu, has_current), offset_of!(PxuiSubmenu, reserved), offset_of!(PxuiSubmenu, rows), offset_of!(PxuiSubmenu, enabled), offset_of!(PxuiSubmenu, keys)]);
        check("PxuiMenuExtra", size_of::<PxuiMenuExtra>(), align_of::<PxuiMenuExtra>(), &[offset_of!(PxuiMenuExtra, size), offset_of!(PxuiMenuExtra, reserved), offset_of!(PxuiMenuExtra, lead_from), offset_of!(PxuiMenuExtra, keys), offset_of!(PxuiMenuExtra, swatches), offset_of!(PxuiMenuExtra, danger), offset_of!(PxuiMenuExtra, submenus)]);
        check("Slice", size_of::<Slice<u32>>(), align_of::<Slice<u32>>(), &[0, 8]);
        check("Input", size_of::<Input>(), align_of::<Input>(), &[0, 8, 16, 24, 32, 40, 56, 72, 88, 104, 120, 136, 152]);
        check("PlatformOut", size_of::<PlatformOut>(), align_of::<PlatformOut>(), &[0, 4, 8, 9, 10, 11, 16, 48, 56, 72, 76, 77]);
        check("Damage", size_of::<Damage>(), align_of::<Damage>(), &[0, 1, 4]);
        check("A11yValue", size_of::<A11yValue>(), align_of::<A11yValue>(), &[0, 4, 8, 12, 16, 24, 32, 40]);
        check("A11yNode", size_of::<A11yNode>(), align_of::<A11yNode>(), &[0, 8, 12, 16, 20, 24, 28, 32, 80, 96, 100, 104, 112, 116, 120, 124]);
        check("A11yAction", size_of::<A11yAction>(), align_of::<A11yAction>(), &[0, 8, 12, 16, 24, 28]);
        check("A11yTree", size_of::<A11yTree>(), align_of::<A11yTree>(), &[0, 16, 32, 40, 44, 48, 56]);
        check("Output", size_of::<Output>(), align_of::<Output>(), &[0, 4, 8, 16, 24, 32, 40, 48, 136, 160, 224]);
        check("PxuiLayerInfo", size_of::<PxuiLayerInfo>(), align_of::<PxuiLayerInfo>(), &[0, 8, 12, 16, 32]);
        check("PxuiDirty", size_of::<PxuiDirty>(), align_of::<PxuiDirty>(), &[0, 4]);
        check("ThemeSource", size_of::<ThemeSource>(), align_of::<ThemeSource>(), &[0, 4, 8]);
        check("PxuiMotion", size_of::<PxuiMotion>(), align_of::<PxuiMotion>(), &[offset_of!(PxuiMotion, fast), offset_of!(PxuiMotion, normal), offset_of!(PxuiMotion, slow), offset_of!(PxuiMotion, ease)]);
        check("PxuiThemeTokens", size_of::<ThemeTokens>(), align_of::<ThemeTokens>(), &[offset_of!(ThemeTokens, panel), offset_of!(ThemeTokens, foreground), offset_of!(ThemeTokens, control), offset_of!(ThemeTokens, input), offset_of!(ThemeTokens, track), offset_of!(ThemeTokens, accent), offset_of!(ThemeTokens, ink_ground), offset_of!(ThemeTokens, has_ink_ground), offset_of!(ThemeTokens, reserved), offset_of!(ThemeTokens, space), offset_of!(ThemeTokens, radius), offset_of!(ThemeTokens, display_size), offset_of!(ThemeTokens, title_size), offset_of!(ThemeTokens, font_size), offset_of!(ThemeTokens, label_size), offset_of!(ThemeTokens, motion), offset_of!(ThemeTokens, reduced_motion), offset_of!(ThemeTokens, reserved_tail), offset_of!(ThemeTokens, density)]);
        check("PxuiDialogOut", size_of::<PxuiDialogOut>(), align_of::<PxuiDialogOut>(), &[offset_of!(PxuiDialogOut, tag), offset_of!(PxuiDialogOut, index)]);
        check("PxuiThemeSnapshot", size_of::<PxuiThemeSnapshot>(), align_of::<PxuiThemeSnapshot>(), &[offset_of!(PxuiThemeSnapshot, size), offset_of!(PxuiThemeSnapshot, part_count), offset_of!(PxuiThemeSnapshot, tokens), offset_of!(PxuiThemeSnapshot, row_h), offset_of!(PxuiThemeSnapshot, control_h), offset_of!(PxuiThemeSnapshot, scrollbar_w), offset_of!(PxuiThemeSnapshot, scrollbar_gap), offset_of!(PxuiThemeSnapshot, ring)]);
        check("PxuiSizeDesc", size_of::<PxuiSizeDesc>(), align_of::<PxuiSizeDesc>(), &[offset_of!(PxuiSizeDesc, kind), offset_of!(PxuiSizeDesc, value), offset_of!(PxuiSizeDesc, relative)]);
        check("PxuiTrackDesc", size_of::<PxuiTrackDesc>(), align_of::<PxuiTrackDesc>(), &[offset_of!(PxuiTrackDesc, kind), offset_of!(PxuiTrackDesc, value)]);
        check("PxuiGridDesc", size_of::<PxuiGridDesc>(), align_of::<PxuiGridDesc>(), &[offset_of!(PxuiGridDesc, has_grid), offset_of!(PxuiGridDesc, cols), offset_of!(PxuiGridDesc, rows)]);
        check("PxuiCellDesc", size_of::<PxuiCellDesc>(), align_of::<PxuiCellDesc>(), &[offset_of!(PxuiCellDesc, col), offset_of!(PxuiCellDesc, row), offset_of!(PxuiCellDesc, col_span), offset_of!(PxuiCellDesc, row_span)]);
        check("PxuiAnchorDesc", size_of::<PxuiAnchorDesc>(), align_of::<PxuiAnchorDesc>(), &[offset_of!(PxuiAnchorDesc, has_anchor), offset_of!(PxuiAnchorDesc, rect), offset_of!(PxuiAnchorDesc, side), offset_of!(PxuiAnchorDesc, align), offset_of!(PxuiAnchorDesc, gap), offset_of!(PxuiAnchorDesc, margin), offset_of!(PxuiAnchorDesc, flip), offset_of!(PxuiAnchorDesc, reserved_tail)]);
        check("PxuiFontDesc", size_of::<PxuiFontDesc>(), align_of::<PxuiFontDesc>(), &[offset_of!(PxuiFontDesc, family), offset_of!(PxuiFontDesc, weight), offset_of!(PxuiFontDesc, size), offset_of!(PxuiFontDesc, tracking), offset_of!(PxuiFontDesc, caps), offset_of!(PxuiFontDesc, reserved)]);
        check("PxuiShadowDesc", size_of::<PxuiShadowDesc>(), align_of::<PxuiShadowDesc>(), &[offset_of!(PxuiShadowDesc, color), offset_of!(PxuiShadowDesc, offset), offset_of!(PxuiShadowDesc, blur), offset_of!(PxuiShadowDesc, spread), offset_of!(PxuiShadowDesc, inner), offset_of!(PxuiShadowDesc, reserved)]);
        check("PxuiRingDesc", size_of::<PxuiRingDesc>(), align_of::<PxuiRingDesc>(), &[offset_of!(PxuiRingDesc, color), offset_of!(PxuiRingDesc, width), offset_of!(PxuiRingDesc, sides)]);
        check("PxuiLookDesc", size_of::<PxuiLookDesc>(), align_of::<PxuiLookDesc>(), &[offset_of!(PxuiLookDesc, fill), offset_of!(PxuiLookDesc, gradient), offset_of!(PxuiLookDesc, has_gradient), offset_of!(PxuiLookDesc, angle), offset_of!(PxuiLookDesc, border), offset_of!(PxuiLookDesc, border_w), offset_of!(PxuiLookDesc, sides), offset_of!(PxuiLookDesc, radius), offset_of!(PxuiLookDesc, shadows), offset_of!(PxuiLookDesc, text), offset_of!(PxuiLookDesc, font), offset_of!(PxuiLookDesc, opacity), offset_of!(PxuiLookDesc, ring), offset_of!(PxuiLookDesc, selection), offset_of!(PxuiLookDesc, caret), offset_of!(PxuiLookDesc, dashed), offset_of!(PxuiLookDesc, reserved_tail)]);
        check("PxuiMarkDesc", size_of::<PxuiMarkDesc>(), align_of::<PxuiMarkDesc>(), &[offset_of!(PxuiMarkDesc, kind), offset_of!(PxuiMarkDesc, direction), offset_of!(PxuiMarkDesc, width), offset_of!(PxuiMarkDesc, rising), offset_of!(PxuiMarkDesc, reserved)]);
        check("PxuiBarDesc", size_of::<PxuiBarDesc>(), align_of::<PxuiBarDesc>(), &[offset_of!(PxuiBarDesc, has_bar), offset_of!(PxuiBarDesc, whole), offset_of!(PxuiBarDesc, reserved), offset_of!(PxuiBarDesc, color), offset_of!(PxuiBarDesc, radius), offset_of!(PxuiBarDesc, width), offset_of!(PxuiBarDesc, gap), offset_of!(PxuiBarDesc, margin)]);
        check("PxuiStateDesc", size_of::<PxuiStateDesc>(), align_of::<PxuiStateDesc>(), &[offset_of!(PxuiStateDesc, hovered), offset_of!(PxuiStateDesc, pressed), offset_of!(PxuiStateDesc, focused), offset_of!(PxuiStateDesc, disabled), offset_of!(PxuiStateDesc, checked), offset_of!(PxuiStateDesc, reserved)]);
        check("PxuiCurveDesc", size_of::<PxuiCurveDesc>(), align_of::<PxuiCurveDesc>(), &[offset_of!(PxuiCurveDesc, kind), offset_of!(PxuiCurveDesc, duration), offset_of!(PxuiCurveDesc, points), offset_of!(PxuiCurveDesc, omega)]);
        check("PxuiSaved", size_of::<PxuiSaved>(), align_of::<PxuiSaved>(), &[offset_of!(PxuiSaved, size), offset_of!(PxuiSaved, reserved), offset_of!(PxuiSaved, parents), offset_of!(PxuiSaved, seeds)]);
        check("PxuiSignal", size_of::<PxuiSignal>(), align_of::<PxuiSignal>(), &[offset_of!(PxuiSignal, size), offset_of!(PxuiSignal, flags), offset_of!(PxuiSignal, has_subtree_press), offset_of!(PxuiSignal, button), offset_of!(PxuiSignal, has_a11y_value), offset_of!(PxuiSignal, has_a11y_select), offset_of!(PxuiSignal, reserved), offset_of!(PxuiSignal, subtree_press), offset_of!(PxuiSignal, clicks), offset_of!(PxuiSignal, drag), offset_of!(PxuiSignal, pointer), offset_of!(PxuiSignal, press_point), offset_of!(PxuiSignal, release_point), offset_of!(PxuiSignal, scroll), offset_of!(PxuiSignal, pinch), offset_of!(PxuiSignal, a11y_value), offset_of!(PxuiSignal, a11y_select), offset_of!(PxuiSignal, keys)]);
        check("PxuiBoxDesc", size_of::<PxuiBoxDesc>(), align_of::<PxuiBoxDesc>(), &[offset_of!(PxuiBoxDesc, size), offset_of!(PxuiBoxDesc, axis), offset_of!(PxuiBoxDesc, flags), offset_of!(PxuiBoxDesc, w), offset_of!(PxuiBoxDesc, h), offset_of!(PxuiBoxDesc, max_h), offset_of!(PxuiBoxDesc, padding), offset_of!(PxuiBoxDesc, scroll_step), offset_of!(PxuiBoxDesc, has_at), offset_of!(PxuiBoxDesc, has_hit_rect), offset_of!(PxuiBoxDesc, reserved), offset_of!(PxuiBoxDesc, at), offset_of!(PxuiBoxDesc, hit_rect), offset_of!(PxuiBoxDesc, hit), offset_of!(PxuiBoxDesc, hit_user), offset_of!(PxuiBoxDesc, hit_release)]);
        check("PxuiValueFieldDesc", size_of::<PxuiValueFieldDesc>(), align_of::<PxuiValueFieldDesc>(), &[offset_of!(PxuiValueFieldDesc, size), offset_of!(PxuiValueFieldDesc, flags), offset_of!(PxuiValueFieldDesc, font_size), offset_of!(PxuiValueFieldDesc, fraction), offset_of!(PxuiValueFieldDesc, tracking), offset_of!(PxuiValueFieldDesc, line), offset_of!(PxuiValueFieldDesc, lead_color), offset_of!(PxuiValueFieldDesc, trail_color), offset_of!(PxuiValueFieldDesc, reserved), offset_of!(PxuiValueFieldDesc, x), offset_of!(PxuiValueFieldDesc, y), offset_of!(PxuiValueFieldDesc, width), offset_of!(PxuiValueFieldDesc, height), offset_of!(PxuiValueFieldDesc, display), offset_of!(PxuiValueFieldDesc, placeholder), offset_of!(PxuiValueFieldDesc, lead), offset_of!(PxuiValueFieldDesc, trail), offset_of!(PxuiValueFieldDesc, label), offset_of!(PxuiValueFieldDesc, value)]);
        check("PxuiPaintInfo", size_of::<PxuiPaintInfo>(), align_of::<PxuiPaintInfo>(), &[offset_of!(PxuiPaintInfo, size), offset_of!(PxuiPaintInfo, rtl), offset_of!(PxuiPaintInfo, reserved), offset_of!(PxuiPaintInfo, font_size), offset_of!(PxuiPaintInfo, row_height), offset_of!(PxuiPaintInfo, density), offset_of!(PxuiPaintInfo, clip), offset_of!(PxuiPaintInfo, clip_radius), offset_of!(PxuiPaintInfo, theme)]);
        check("PxuiPopupDesc", size_of::<PxuiPopupDesc>(), align_of::<PxuiPopupDesc>(), &[offset_of!(PxuiPopupDesc, size), offset_of!(PxuiPopupDesc, has_stroke), offset_of!(PxuiPopupDesc, has_max_height), offset_of!(PxuiPopupDesc, dismiss_initial), offset_of!(PxuiPopupDesc, attached), offset_of!(PxuiPopupDesc, stroke), offset_of!(PxuiPopupDesc, max_height), offset_of!(PxuiPopupDesc, x), offset_of!(PxuiPopupDesc, y), offset_of!(PxuiPopupDesc, width), offset_of!(PxuiPopupDesc, height), offset_of!(PxuiPopupDesc, label)]);
        check("PxuiHeadActionDesc", size_of::<PxuiHeadActionDesc>(), align_of::<PxuiHeadActionDesc>(), &[offset_of!(PxuiHeadActionDesc, caption), offset_of!(PxuiHeadActionDesc, keycap), offset_of!(PxuiHeadActionDesc, active), offset_of!(PxuiHeadActionDesc, usable), offset_of!(PxuiHeadActionDesc, reserved)]);
        check("PxuiHeaderDesc", size_of::<PxuiHeaderDesc>(), align_of::<PxuiHeaderDesc>(), &[offset_of!(PxuiHeaderDesc, size), offset_of!(PxuiHeaderDesc, has_kind), offset_of!(PxuiHeaderDesc, has_badge), offset_of!(PxuiHeaderDesc, has_index), offset_of!(PxuiHeaderDesc, has_reset), offset_of!(PxuiHeaderDesc, reset_enabled), offset_of!(PxuiHeaderDesc, reserved), offset_of!(PxuiHeaderDesc, key), offset_of!(PxuiHeaderDesc, kind), offset_of!(PxuiHeaderDesc, badge), offset_of!(PxuiHeaderDesc, index), offset_of!(PxuiHeaderDesc, reset), offset_of!(PxuiHeaderDesc, title), offset_of!(PxuiHeaderDesc, detail), offset_of!(PxuiHeaderDesc, actions)]);
        check("PxuiHeadOut", size_of::<PxuiHeadOut>(), align_of::<PxuiHeadOut>(), &[offset_of!(PxuiHeadOut, renamed), offset_of!(PxuiHeadOut, chosen), offset_of!(PxuiHeadOut, has_chosen), offset_of!(PxuiHeadOut, reset_pressed), offset_of!(PxuiHeadOut, reserved)]);
        check("PxuiNodeDesc", size_of::<NodeDesc>(), align_of::<NodeDesc>(), &[offset_of!(NodeDesc, size), offset_of!(NodeDesc, flags), offset_of!(NodeDesc, w), offset_of!(NodeDesc, h), offset_of!(NodeDesc, min_w), offset_of!(NodeDesc, min_h), offset_of!(NodeDesc, max_w), offset_of!(NodeDesc, max_h), offset_of!(NodeDesc, grow), offset_of!(NodeDesc, shrink), offset_of!(NodeDesc, aspect), offset_of!(NodeDesc, axis), offset_of!(NodeDesc, padding), offset_of!(NodeDesc, margin), offset_of!(NodeDesc, gap), offset_of!(NodeDesc, row_gap), offset_of!(NodeDesc, justify), offset_of!(NodeDesc, align), offset_of!(NodeDesc, align_self), offset_of!(NodeDesc, wrap), offset_of!(NodeDesc, reserved), offset_of!(NodeDesc, grid), offset_of!(NodeDesc, cell), offset_of!(NodeDesc, z), offset_of!(NodeDesc, reserved_z), offset_of!(NodeDesc, role), offset_of!(NodeDesc, a11y), offset_of!(NodeDesc, anchor), offset_of!(NodeDesc, has_at), offset_of!(NodeDesc, at), offset_of!(NodeDesc, scroll_step), offset_of!(NodeDesc, look), offset_of!(NodeDesc, inset), offset_of!(NodeDesc, mark), offset_of!(NodeDesc, bar), offset_of!(NodeDesc, state), offset_of!(NodeDesc, transition)]);
    }
}
