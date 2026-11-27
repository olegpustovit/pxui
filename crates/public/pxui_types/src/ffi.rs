//! C-compatible frame, platform, accessibility, and theme exchange records.
use crate::{Event, PxuiBox, Str, geom::{Edges, Rect, Rgba}};

/// Borrowed contiguous span of ABI records.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Slice<T> { #[doc = "Pointer to the first element of the borrowed span."] pub ptr: *const T, #[doc = "Number of elements in the span."] pub len: usize }

impl From<&'static [u8]> for Slice<u8> {
    /// A wire byte span whose backing storage remains alive for the process.
    fn from(value: &'static [u8]) -> Slice<u8> { Slice { ptr: value.as_ptr(), len: value.len() } }
}
impl<T> Default for Slice<T> { fn default() -> Self { Self { ptr: core::ptr::null(), len: 0 } } }

/// An array of records that crosses as `{ptr, len, stride}`: record `i` starts at `ptr + i * stride` bytes. A record grows at its end, so
/// the stride is the writer's record size and a reader steps by it, reading the prefix it knows (each record's own `size` says how much
/// it holds). `stride` below `size_of::<T>()` (or not a multiple of the record's alignment) is refused; the library always writes
/// `size_of::<T>()`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Strided<T> { #[doc = "Pointer to the first element of the borrowed span."] pub ptr: *const T, #[doc = "Number of elements in the span."] pub len: usize, #[doc = "Distance in bytes between adjacent records."] pub stride: usize }
impl<T> Default for Strided<T> { fn default() -> Self { Self { ptr: core::ptr::null(), len: 0, stride: core::mem::size_of::<T>() } } }
impl<T> Strided<T> {
    /// `records` as a dense span (stride the record size).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ffi::Strided;
    /// let span = Strided::of(&[1u32, 2, 3]);
    /// assert_eq!((span.len, span.stride), (3, 4));
    /// ```
    pub const fn of(records: &[T]) -> Self { Self { ptr: records.as_ptr(), len: records.len(), stride: core::mem::size_of::<T>() } }

    /// Whether a reader may step through it: a pointer (aligned for the record) when there are records, and a stride that holds one and keeps each aligned.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ffi::Strided;
    /// let data = [1u32, 2];
    /// assert!(Strided::of(&data).is_valid());
    /// assert!(!Strided { stride: 3, ..Strided::of(&data) }.is_valid());
    /// ```
    pub fn is_valid(&self) -> bool {
        (self.len == 0 || !self.ptr.is_null()) && self.stride >= core::mem::size_of::<T>() && self.stride % core::mem::align_of::<T>() == 0
            && (self.ptr as usize) % core::mem::align_of::<T>() == 0
    }

    /// Record `index`, read at `ptr + index * stride`. The span must be valid, `index` below `len`, and the records live for `'a`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ffi::Strided;
    /// let data = [7u32, 8];
    /// assert_eq!(unsafe { *Strided::of(&data).at(1) }, 8);
    /// ```
    pub unsafe fn at<'a>(&self, index: usize) -> &'a T where T: 'a { unsafe { &*self.ptr.cast::<u8>().add(index * self.stride).cast::<T>() } }

    /// The records as a slice when the stride is the record size (what this library writes), else none.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ffi::Strided;
    /// let data = [7u32, 8];
    /// assert_eq!(unsafe { Strided::of(&data).as_slice() }, Some(&data[..]));
    /// ```
    pub unsafe fn as_slice<'a>(&self) -> Option<&'a [T]> where T: 'a {
        if self.len == 0 { return Some(&[]); }
        (self.stride == core::mem::size_of::<T>()).then(|| unsafe { core::slice::from_raw_parts(self.ptr, self.len) })
    }
}

/// Snapshot of native input and frame timing.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Input {
    /// Size of this record in bytes.
    pub size: u32,
    /// Time in seconds on the host monotonic clock.
    pub time: f64,
    /// Elapsed seconds since the previous frame.
    pub dt: f64,
    /// Frame serial.
    pub frame: i64,
    /// Input update serial.
    pub tick: i64,
    /// Width and height in logical points.
    pub logical_size: [i64; 2],
    /// Pointer position in logical points.
    pub pointer: [f64; 2],
    /// Key codes currently held down.
    pub keys: Slice<u32>,
    /// Pointer buttons currently held down.
    pub buttons: Slice<u32>,
    /// Input events attached to this frame.
    pub events: Strided<Event>,
    /// Drawable pixels per logical point on each axis.
    pub pixel_scale: [f64; 2],
    /// Pointer movement in logical points since the previous frame.
    pub mouse_delta: [f64; 2],
    /// Frame rate in frames per second.
    pub fps: f64,
}

impl Default for Input {
    fn default() -> Self {
        Self { size: core::mem::size_of::<Self>() as u32, time: 0.0, dt: 0.0, frame: 0, tick: 0, logical_size: [0; 2], pointer: [0.0; 2],
            keys: Slice::default(), buttons: Slice::default(), events: Strided::default(), pixel_scale: [1.0; 2], mouse_delta: [0.0; 2], fps: 60.0 }
    }
}

/// Authoritative host input at the end of the events in a frame.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiInputSnapshot {
    /// Size of this record in bytes.
    pub size: u32,
    /// Held key codes.
    pub keys: Slice<u32>,
    /// Held pointer button codes.
    pub buttons: Slice<u32>,
    /// Pointer position in logical points.
    pub pointer: [f64; 2],
    /// Pointer movement since the previous frame in logical points.
    pub mouse_delta: [f64; 2],
    /// Drawable pixels per logical point on each axis.
    pub pixel_scale: [f64; 2],
    /// Elapsed seconds since the previous frame.
    pub dt: f64,
    /// Frame serial.
    pub frame: i64,
    /// Input update serial.
    pub tick: i64,
    /// Frame rate in frames per second.
    pub fps: f64,
}

/// Requests to apply to the native window, input method and clipboard.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PlatformOut {
    /// Size of this record in bytes.
    pub size: u32,
    /// 0 default, 1 horizontal resize, 2 vertical resize, 3 text
    pub cursor: u32,
    /// Nonzero when `cursor` is present.
    pub has_cursor: u8,
    /// Nonzero when `ime` is present.
    pub has_ime: u8,
    /// Nonzero when `background` is present.
    pub has_background: u8,
    /// Nonzero when `clipboard` is present.
    pub has_clipboard: u8,
    /// Input method candidate rectangle in logical points.
    pub ime_rect: [i64; 4],
    /// Caret position within the input method target.
    pub ime_caret: i64,
    /// Text to copy to the native clipboard.
    pub clipboard: Str,
    /// Requested native window background colour.
    pub background: Rgba,
    /// Nonzero when the app should fetch the native clipboard.
    pub paste: u8,
    /// Reserved for the ABI. Write zero on input.
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
    /// Inclusive start of the range.
    pub start: u32,
    /// Exclusive end of the range.
    pub end: u32,
}

/// One layer of the last frame (a floating root's, drawn in order): the key of its root box, its retained id, its instances (an
/// empty layer draws nothing), how many texture breaks they hold and the instance ranges changed since it was last drawn. Borrowed
/// until the next frame.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiLayerInfo {
    /// `size_of::<PxuiLayerInfo>()`; the library writes it (a reader steps by `PxuiLayerSpan.stride`).
    pub size: u32,
    /// Identifier supplied by the caller.
    pub id: u32,
    /// Draw breaks separating runs of instances.
    pub breaks: u32,
    /// What the layer is: 0 base (the window's content), 1 popup or menu, 2 modal sheet, 3 tooltip, 4 what a drag carries, 5 the merged
    /// last layer (every root past `MAX_LAYERS`; a host presents it in the window). A host may present a floating layer in a surface of its
    /// own; input for it then arrives with that surface's id.
    pub role: u32,
    /// Key used to identify the element across frames.
    pub key: i64,
    /// Borrowed draw instances in paint order.
    pub instances: Slice<crate::draw::Instance>,
    /// Dirty ranges changed since the previous frame.
    pub dirty: Slice<PxuiDirty>,
    /// Where a floating layer is placed, in window points: the rectangle its root was anchored to, else the root's own; empty for the base.
    pub anchor: crate::geom::Rect,
}

/// Bounding rectangle that needs redrawing after the frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Damage {
    /// Nonzero when `damage` is present.
    pub has_damage: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 3],
    /// Rectangle in logical points.
    pub rect: Rect,
}

/// Cumulative reasons observed by a UI with `PXUI_FRAME_REASONS=1`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiFrameReasons {
    /// Immediate repaints requested by the frame or animated keyboard.
    pub requested: u64,
    /// Immediate repaints for capture, scrolling or a settling gesture.
    pub interaction: u64,
    /// Immediate repaints requested by a one-shot transition.
    pub transition: u64,
    /// Immediate repaints requested by a finite keyframe run.
    pub finite_keys: u64,
    /// Immediate repaints requested by display-rate loops or ambient loops after input.
    pub active_loop: u64,
    /// Repaints scheduled at the ambient loop interval.
    pub idle_loop: u64,
    /// Successful animation steps.
    pub stepped: u64,
    /// Steps refused because the UI was not ready or time was invalid.
    pub step_state: u64,
    /// Steps refused because the UI needed to rebuild.
    pub step_build: u64,
    /// Steps refused because interactive motion needed a build.
    pub step_interaction: u64,
    /// Steps refused because a timer or hover delay was due.
    pub step_deadline: u64,
    /// Resolved node key for the most recently observed display-rate, non-ambient animation.
    pub transition_key: i64,
    /// Curve of that animation’s current segment. Kind zero means no sample has been recorded.
    pub transition_curve: PxuiCurveDesc,
    /// Reserved for the ABI.
    pub reserved: u32,
}

/// Up to eight damaged rectangles in window points, each as `[x, y, width, height]`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PxuiDamageList {
    /// Number of valid entries in `rects`.
    pub count: u8,
    /// Nonzero when the whole viewport needs redrawing.
    pub full: u8,
    /// Reserved for the ABI, written as zero.
    pub reserved: [u8; 6],
    /// Damaged rectangles. A full list reports the viewport.
    pub rects: [[f32; 4]; 8],
}

/// One geometry and opacity stop in a compositor animation.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PxuiPromotedKey {
    /// Seconds from the beginning of the forward leg.
    pub time: f32,
    /// Window rectangle in points as `[x, y, width, height]`.
    pub rect: [f32; 4],
    /// Composed opacity, from zero to one.
    pub opacity: f32,
    /// Bézier control points for the segment ending at this stop.
    pub curve: [f32; 4],
}

/// A retained view the host can composite above its canvas.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PxuiPromoted {
    /// Canvas layer ordinal.
    pub layer: u32,
    /// Ordinal of the excluded instance in the layer.
    pub instance: u32,
    /// Stable view-table index for this run.
    pub view: u16,
    /// Number of used keys. One key means static content.
    pub count: u8,
    /// Nonzero when the loop alternates forward and reverse.
    pub autoreverse: u8,
    /// Ambient refresh rate, or zero for the display rate.
    pub hz: u16,
    /// Reserved for the ABI, written as zero.
    pub reserved: u16,
    /// Changes when the retained content is repainted.
    pub content_revision: u64,
    /// Start of the run in the UI clock's seconds.
    pub start: f64,
    /// Phase into the first cycle, in seconds.
    pub offset: f32,
    /// Fill color as straight RGBA bytes.
    pub color: [u8; 4],
    /// Cover of every displayed pose, in window points.
    pub swept: [f32; 4],
    /// Fixed window-space clip in points as `[x, y, width, height]`.
    pub clip: [f32; 4],
    /// The initial pose and subsequent endpoints in ascending time order.
    pub keys: [PxuiPromotedKey; 9],
}

/// Tagged value exposed to assistive technology.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct A11yValue {
    /// 0 none, 1 text range, 2 number (ABI 1.126: its text range, when not empty, says the number in words, a section index's letter)
    pub tag: u32,
    /// Start byte offset in the tree string buffer.
    pub text_start: u32,
    /// End byte offset in the tree string buffer.
    pub text_end: u32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u32,
    /// Current numeric value.
    pub now: f64,
    /// Minimum permitted value.
    pub min: f64,
    /// Maximum permitted value.
    pub max: f64,
    /// Increment for a numeric value.
    pub step: f64,
}

impl A11yNode {
    /// Whether the node has `role`.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let node = pxui_types::ffi::A11yNode::default();
    /// assert_eq!(node.is_role(pxui_types::a11y::Role::Button), node.role == pxui_types::a11y::Role::Button as u32);
    /// ```
    #[inline] pub fn is_role(&self, role: crate::a11y::Role) -> bool { self.role == role as u32 }
}

/// One accessibility node, size-prefixed: `size` is the writer's record size, read as the prefix a reader knows; the tree's `nodes.stride` steps.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct A11yNode {
    /// Size of this record in bytes.
    pub size: u32,
    /// Index of the parent, with the no-parent sentinel for roots.
    pub parent: u32,
    /// Identifier supplied by the caller.
    pub id: u64,
    /// Accessibility role code.
    pub role: u32,
    /// Start byte offset of the accessible name in the tree string buffer.
    pub name_start: u32,
    /// End byte offset of the accessible name in the tree string buffer.
    pub name_end: u32,
    /// Start byte offset of the accessible description in the tree string buffer.
    pub description_start: u32,
    /// End byte offset of the accessible description in the tree string buffer.
    pub description_end: u32,
    /// Value carried by this record.
    pub value: A11yValue,
    /// Rectangle in logical points.
    pub rect: Rect,
    /// Bit mask of accessibility states.
    pub states: u32,
    /// Bit mask of supported accessibility actions.
    pub actions: u32,
    /// Anchor and caret in Unicode scalar values, as in the engine's text widgets and AT-SPI.
    pub selection: [u32; 2],
    /// Number of immediate children.
    pub child_count: u32,
    /// Index of the first child, with the no-child sentinel for an empty list.
    pub first_child: u32,
    /// Index of the following sibling, with the sentinel at the end of a chain.
    pub next_sibling: u32,
    /// Index of the final child, with the no-child sentinel for an empty list.
    pub last_child: u32,
}
impl Default for A11yNode {
    fn default() -> Self {
        Self { size: core::mem::size_of::<Self>() as u32, parent: 0, id: 0, role: 0, name_start: 0, name_end: 0, description_start: 0, description_end: 0, value: A11yValue::default(),
            rect: Rect::default(), states: 0, actions: 0, selection: [0; 2], child_count: 0, first_child: 0, next_sibling: 0, last_child: 0 }
    }
}

/// One action an assistive technology asked for on `surface`, size-prefixed like `A11yNode` (`size_of::<A11yAction>()`, 40; `Default` sets it).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct A11yAction {
    /// Size of this record in bytes.
    pub size: u32,
    /// One action bit: 1 press, 2 focus, 4 set value, 8 scroll, 16 expand, 32 collapse, 64 select, 128 move up and 256 move down
    /// (ABI 1.89: a reorderable list row, offered in its node's `actions`).
    pub action_code: u32,
    /// Identifier supplied by the caller.
    pub id: u64,
    /// Finite action data; scroll deltas saturate to the finite point range before layout clamps them to content.
    pub payload: f64,
    /// Exclusive end of the range.
    pub end: u32,
    /// Zero on input.
    pub reserved: u32,
    /// The registered destination surface; defaults to `FrameIn::MAIN` and must match the receiving Ui.
    pub surface: u64,
}
impl Default for A11yAction {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, action_code: 0, id: 0, payload: 0.0, end: 0, reserved: 0, surface: crate::FrameIn::MAIN } }
}

/// Borrowed accessible nodes, strings and queued actions.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct A11yTree {
    /// `size_of::<A11yTree>()`: a record whose size is short is refused (`pxui_a11y_update`).
    pub size: u32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u32,
    /// `nodes.stride` is the node record the writer used (the tree's node stride).
    pub nodes: Strided<A11yNode>,
    /// Borrowed UTF-8 storage addressed by the accessibility nodes.
    pub strings: Str,
    /// Revision of the borrowed data, used to detect changes.
    pub revision: u64,
    /// Index of the first child, with the no-child sentinel for an empty list.
    pub first_child: u32,
    /// Number of immediate children.
    pub child_count: u32,
    /// Borrowed pending accessibility actions.
    pub actions: Strided<A11yAction>,
}
impl Default for A11yTree {
    fn default() -> Self {
        Self { size: core::mem::size_of::<Self>() as u32, reserved: 0, nodes: Strided::default(), strings: Str::default(), revision: 0, first_child: u32::MAX, child_count: 0, actions: Strided::default() }
    }
}

/// Borrowed frame results and requests for the host.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Output {
    /// Size of this record in bytes.
    pub size: u32,
    /// Highest active ambient rate in Hz. Zero means display rate or no active animation (ABI 2.9).
    pub repaint_hz: u32,
    /// Revision of the draw list for detecting changes.
    pub draw_revision: u64,
    /// Positive infinity means no scheduled repaint.
    pub repaint_at: f64,
    /// Count of nodes built in the frame.
    pub built_nodes: u64,
    /// Count of retained views rebuilt in the frame.
    pub rebuilt_views: u64,
    /// Count of nodes arranged during layout.
    pub arranged_nodes: u64,
    /// Requests to apply to the native window, input method and clipboard.
    pub platform: PlatformOut,
    /// Bounding box of the regions of the frame that changed.
    pub damage: Damage,
    /// Accessibility state bits declared by the widget.
    pub a11y: A11yTree,
    /// the last frame's layers in draw order, borrowed until the next frame
    pub layers: Strided<PxuiLayerInfo>,
    /// Damage in logical window points (ABI 2.6).
    pub damage_list: PxuiDamageList,
}
impl Default for Output {
    fn default() -> Self {
        Self { size: core::mem::size_of::<Self>() as u32, repaint_hz: 0, draw_revision: 0, repaint_at: f64::INFINITY, built_nodes: 0, rebuilt_views: 0, arranged_nodes: 0,
            platform: PlatformOut { size: core::mem::size_of::<PlatformOut>() as u32, ..PlatformOut::default() }, damage: Damage::default(),
            a11y: A11yTree::default(), layers: Strided::default(), damage_list: PxuiDamageList::default() }
    }
}

/// Theme animation durations and easing.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiMotion {
    /// Fast transition duration in seconds.
    pub fast: f32,
    /// Standard transition duration in seconds.
    pub normal: f32,
    /// Slow transition duration in seconds.
    pub slow: f32,
    /// Theme cubic Bézier easing coordinates.
    pub ease: [f32; 4],
}

/// Stable snapshot of Gallery paint tokens; independent of the private theme graph.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ThemeTokens {
    /// Size of this record in bytes.
    pub size: u32,
    /// Panel background colour.
    pub panel: Rgba,
    /// Default foreground colour.
    pub foreground: Rgba,
    /// Control background colour.
    pub control: Rgba,
    /// Text input background colour.
    pub input: Rgba,
    /// Track background colour.
    pub track: Rgba,
    /// Accent colour for selection and emphasis.
    pub accent: Rgba,
    /// Optional contrast surface behind text.
    pub ink_ground: Rgba,
    /// Nonzero when `ink_ground` is present.
    pub has_ink_ground: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 3],
    /// Theme spacing scale in logical points.
    pub space: [f32; 4],
    /// Corner radius values in logical points.
    pub radius: [f32; 3],
    /// Display text size in logical points.
    pub display_size: i64,
    /// Title text size in logical points.
    pub title_size: i64,
    /// Text size in logical points.
    pub font_size: i64,
    /// Label text size in logical points.
    pub label_size: i64,
    /// Theme timings and easing values.
    pub motion: PxuiMotion,
    /// Nonzero when animations should respect reduced motion.
    pub reduced_motion: u8,
    /// 0 or 1: the theme is dark (`Tokens::dark`).
    pub dark: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved_tail: [u8; 2],
    /// Theme density multiplier.
    pub density: f32,
}

/// The fields of a declared node an app edits in place after its signal is known (`Ui::node_mut`): its look, its mark and the
/// inset of its text.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiNodeFields {
    /// Size of this record in bytes.
    pub size: u32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u32,
    /// Visual appearance of the element.
    pub look: PxuiLookDesc,
    /// Procedural mark drawn in the box.
    pub mark: PxuiMarkDesc,
    /// Inner offsets in logical points.
    pub inset: Edges,
}

/// A byte range `start..end` of a text.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiRange {
    /// Inclusive start of the range.
    pub start: i64,
    /// Exclusive end of the range.
    pub end: i64,
}

/// A byte range with a colour: a syntax span or a chip.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiColorSpan {
    /// Inclusive start of the range.
    pub start: i64,
    /// Exclusive end of the range.
    pub end: i64,
    /// Colour of the element.
    pub color: Rgba,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u32,
}

/// A message under a line of a text area, optionally pointing at a byte range of it.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiNote {
    /// Zero-based text line of the diagnostic.
    pub line: i64,
    /// Inclusive start of the range.
    pub start: i64,
    /// Exclusive end of the range.
    pub end: i64,
    /// Nonzero when `range` is present.
    pub has_range: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 7],
    /// Borrowed UTF-8 text.
    pub text: Str,
}

/// A replacement of bytes `start..end` of the text with `text`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiSplice {
    /// Inclusive start of the range.
    pub start: i64,
    /// Exclusive end of the range.
    pub end: i64,
    /// Borrowed UTF-8 text.
    pub text: Str,
}

/// One suggestion of a language's `complete`: the byte range `insert` replaces, the row, its detail and its doc line.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiCompletion {
    /// Inclusive byte offset at which to start replacing text.
    pub start: i64,
    /// Exclusive byte offset at which to end replacing text.
    pub end: i64,
    /// Replacement text inserted when accepting the completion.
    pub insert: Str,
    /// Label supplied by the caller.
    pub label: Str,
    /// Secondary text accompanying the main label.
    pub detail: Str,
    /// Documentation displayed for this completion.
    pub doc: Str,
}

/// A token's byte range and its description (`has` is 0 for none).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiDescription {
    /// Nonzero when this optional record contains a value.
    pub has: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 7],
    /// Inclusive start of the range.
    pub start: i64,
    /// Exclusive end of the range.
    pub end: i64,
    /// Borrowed UTF-8 text.
    pub text: Str,
}

/// An optional byte range (`has` is 0 for none).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiMaybeRange {
    /// Nonzero when this optional record contains a value.
    pub has: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 7],
    /// Inclusive start of the range.
    pub start: i64,
    /// Exclusive end of the range.
    pub end: i64,
}

/// What a language's `rewrite` returns: the text it keeps and where the caret lands in it.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiRewrite {
    /// Borrowed UTF-8 text.
    pub text: Str,
    /// Caret position or colour, as specified by the record.
    pub caret: i64,
}

/// A text area (`TextAreaOpts`): its place, its text and the data options. `flags` are the `PxuiTextAreaDesc::*` bits.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiTextAreaDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Bit flags defined for this record.
    pub flags: u32,
    /// Horizontal coordinate in logical points.
    pub x: f32,
    /// Vertical coordinate in logical points.
    pub y: f32,
    /// Width in logical points.
    pub width: f32,
    /// Height in logical points.
    pub height: f32,
    /// Requested visible extent or target range.
    pub reveal: i64,
    /// Change token for repeating a reveal request.
    pub reveal_nonce: i64,
    /// Text range requested for selection.
    pub select: PxuiRange,
    /// Label supplied by the caller.
    pub label: Str,
    /// Value carried by this record.
    pub value: Str,
    /// Editor error ranges.
    pub errors: Slice<i64>,
    /// Editor diagnostic messages.
    pub messages: Slice<PxuiNote>,
    /// Borrowed run of styled text spans.
    pub spans: Slice<PxuiRange>,
    /// Optional externally requested text replacement.
    pub splice: Slice<PxuiSplice>,
    /// Inline editor controls.
    pub chips: Slice<PxuiColorSpan>,
    /// what starts a line comment (the language's `comment`)
    pub comment: Str,
    /// the language's bracket pairs, opener then closer, as Unicode scalar values
    pub pairs: Slice<u32>,
}
impl PxuiTextAreaDesc {
    /// Bit enabling line wrapping.
    pub const WRAP: u32 = 1;
    /// Bit indicating that the reveal option is present.
    pub const HAS_REVEAL: u32 = 2;
    /// Bit indicating that the select option is present.
    pub const HAS_SELECT: u32 = 4;
    /// Bit indicating that the language option is present.
    pub const HAS_LANGUAGE: u32 = 8;
    /// Bit indicating that the rewrite option is present.
    pub const HAS_REWRITE: u32 = 16;
    /// Mask of all supported bits in this record.
    pub const KNOWN: u32 = 31;
}

/// Optional language callback returning the absolute colour spans intersecting [start, end).
pub type AreaColorRange = Option<extern "C" fn(*mut core::ffi::c_void, Str, i64, i64, *mut Slice<PxuiColorSpan>)>;
/// Optional language callback returning pairs with an endpoint in [start, end).
pub type AreaBracketsRange = Option<extern "C" fn(*mut core::ffi::c_void, Str, i64, i64, *mut Slice<PxuiRange>)>;
/// Editor callback receiving the user pointer and a changed byte range.
pub type AreaNotify = Option<extern "C" fn(*mut core::ffi::c_void, i64, i64)>;
/// The app's closures of a text area (`TextAreaOpts::on_*` and `Language`), called only during the widget's call with
/// `user`. A callback that returns data writes it to its `out` (borrowed until the next callback). `colorize` and the
/// language members are all set or all null (`rewrite` alone may be null).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiAreaCallbacks {
    /// Size of this record in bytes.
    pub size: u32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u32,
    /// User pointer forwarded unchanged to the callback.
    pub user: *mut core::ffi::c_void,
    /// Callback for a context action over the editor.
    pub on_context: Option<extern "C" fn(*mut core::ffi::c_void, f32, f32)>,
    /// Callback for a numeric scrub gesture.
    pub on_scrub: Option<extern "C" fn(*mut core::ffi::c_void, u32)>,
    /// Callback for an edit from numeric scrubbing.
    pub on_scrub_edit: Option<extern "C" fn(*mut core::ffi::c_void, i64, i64, Str)>,
    /// Callback for a pointer click in the editor.
    pub on_click: Option<extern "C" fn(*mut core::ffi::c_void, i64, u8)>,
    /// Callback reporting the caret.
    pub on_caret: Option<extern "C" fn(*mut core::ffi::c_void, i64)>,
    /// Callback for caret movement.
    pub on_caret_move: Option<extern "C" fn(*mut core::ffi::c_void, i64)>,
    /// Callback reporting a new selection.
    pub on_select: AreaNotify,
    /// Callback for dropped text or objects.
    pub on_drop: Option<extern "C" fn(*mut core::ffi::c_void, i64, u8, Str, Str)>,
    /// Callback that fills syntax colours for the supplied text.
    pub colorize: Option<extern "C" fn(*mut core::ffi::c_void, Str, *mut Slice<PxuiColorSpan>)>,
    /// Callback that finds matching bracket ranges.
    pub brackets: Option<extern "C" fn(*mut core::ffi::c_void, Str, *mut Slice<PxuiRange>)>,
    /// Callback that computes indentation for the editor.
    pub indent: Option<extern "C" fn(*mut core::ffi::c_void, Str, i64, *mut Str)>,
    /// Callback that supplies completion candidates.
    pub complete: Option<extern "C" fn(*mut core::ffi::c_void, Str, i64, *mut Slice<PxuiCompletion>)>,
    /// Callback that supplies a description for the given range.
    pub describe: Option<extern "C" fn(*mut core::ffi::c_void, Str, i64, *mut PxuiDescription)>,
    /// Callback that finds a numeric token at the given offset.
    pub number_at: Option<extern "C" fn(*mut core::ffi::c_void, Str, i64, *mut PxuiMaybeRange)>,
    /// Callback that supplies semantic context at the given offset.
    pub context_at: Option<extern "C" fn(*mut core::ffi::c_void, Str, i64) -> u32>,
    /// Callback that rewrites an edit and returns the caret position.
    pub rewrite: Option<extern "C" fn(*mut core::ffi::c_void, Str, i64, *mut PxuiRewrite)>,
}

/// One row of a context menu (`pxui_ui_context_menu`), read at its own `size` from a strided span: `text` and `shortcut` (empty: none) are
/// UTF-8, `enabled`, `danger`, `selected` (the current choice of a settings menu: CHECKED and SELECTED), `has_swatch` and `has_id` 0 or 1,
/// `checked` 0 no mark slot, 1 a slot, 2 a slot with the mark; `swatch` is read when `has_swatch` is 1 and `id` (the row's key in place of
/// its position, the chosen index is still the row's) when `has_id` is 1. `submenu` holds the rows it opens (none: no submenu); a submenu's
/// rows carry no submenu of their own. `reserved` is zero.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiMenuRow {
    /// `size_of::<PxuiMenuRow>()` (or more).
    pub size: u32,
    /// 0 or 1: the row can be chosen.
    pub enabled: u8,
    /// 0 or 1: a destructive row.
    pub danger: u8,
    /// 0 no mark slot, 1 a slot, 2 a slot with the mark.
    pub checked: u8,
    /// 0 or 1: the current choice.
    pub selected: u8,
    /// 0 or 1: `swatch` is shown.
    pub has_swatch: u8,
    /// 0 or 1: `id` keys the row.
    pub has_id: u8,
    /// Zero.
    pub reserved: [u8; 2],
    /// The colour of the row's swatch.
    pub swatch: Rgba,
    /// The row's key (read when `has_id` is 1).
    pub id: i64,
    /// The row's text.
    pub text: Str,
    /// The shortcut shown at the row's end (empty: none).
    pub shortcut: Str,
    /// The rows this row opens.
    pub submenu: Strided<PxuiMenuRow>,
}

impl PxuiMenuRow {
    /// The plain enabled row showing `text`, with no mark, swatch, shortcut, id or submenu.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let row = pxui_types::ffi::PxuiMenuRow::new("Open");
    /// assert_eq!((row.size as usize, row.enabled), (core::mem::size_of::<pxui_types::ffi::PxuiMenuRow>(), 1));
    /// ```
    pub fn new(text: &str) -> PxuiMenuRow {
        PxuiMenuRow { size: core::mem::size_of::<PxuiMenuRow>() as u32, enabled: 1, danger: 0, checked: 0, selected: 0, has_swatch: 0, has_id: 0, reserved: [0; 2],
            swatch: Rgba::default(), id: 0, text: Str { ptr: text.as_ptr(), len: text.len() }, shortcut: Str::default(), submenu: Strided::default() }
    }
}

/// What a menu has beyond its rows (`pxui_ui_context_menu`): `row_h` the row height in points (0: the theme's), `max_rows` the rows shown
/// before the column scrolls (0: all), `padding` the space above the first row and below the last (finite, not negative), `name` the
/// sheet's accessible name when `has_name` is 1, `focus_on_click` 1 for a press to focus its row, `max_h` the most height of the sheet in
/// points, its edges and padding included (finite, not negative; 0 no bound; the smaller of it and `max_rows` bounds the sheet) and
/// `variant` the `menu_row` body variant (0: the theme's). The caller sets `size`; a shorter record is refused.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiMenuOptions {
    /// `size_of::<PxuiMenuOptions>()` (or more).
    pub size: u32,
    /// The body variant the rows are built with (0: the theme's).
    pub variant: u16,
    /// Zero.
    pub reserved: u16,
    /// The row height in points (0: the theme's).
    pub row_h: f32,
    /// The rows shown before the column scrolls (0: all).
    pub max_rows: u32,
    /// The padding above the first row and below the last.
    pub padding: f32,
    /// 0 or 1: `name` is the sheet's accessible name.
    pub has_name: u8,
    /// 0 or 1: a press focuses its row.
    pub focus_on_click: u8,
    /// Zero.
    pub reserved2: [u8; 6],
    /// The sheet's accessible name (read when `has_name` is 1).
    pub name: Str,
    /// The most height of the sheet in points (0: no bound).
    pub max_h: f32,
    /// Zero.
    pub reserved3: u32,
}

/// What `pxui_ui_diagnostics` reads and writes about one `Ui` (extension set: no stability promise; `PXUI_FEATURE_EXTENSION_SET`). In:
/// `size`, `timing` (0 leaves the phase timing as it is, 1 turns it on, 2 off; the clock reads cost a little) and `rebuilt_prefix` (empty:
/// every view). Out: `calls` (calls into the library on this `Ui`, its borrowed handles, renderers and paint handles, since it was created;
/// this call is not one), `text_layouts` (the text cache's layout count; `has_text_layouts` 0 when the build keeps none), `rebuilt` (views whose
/// label starts with `rebuilt_prefix` the last frame built instead of keeping), `phase_nanos` and, when `has_cpu` is 1, `phase_cpu_nanos` (the
/// last frame's wall and thread CPU time in the phases begin, build, intrinsic, arrange, paint, a11y, finish; zero until timing is on and a
/// frame has run). The caller sets `size`; the out fields are written whole by a successful call.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiDiagnostics {
    /// `size_of::<PxuiDiagnostics>()` (or more).
    pub size: u32,
    /// In: 0 leave the phase timing, 1 on, 2 off.
    pub timing: u8,
    /// Out: 1 when `text_layouts` is the cache's count.
    pub has_text_layouts: u8,
    /// Out: 1 when `phase_cpu_nanos` is a thread CPU time (Apple platforms today).
    pub has_cpu: u8,
    /// Zero.
    pub reserved: u8,
    /// In: the label prefix `rebuilt` counts (empty: every view).
    pub rebuilt_prefix: Str,
    /// Out: calls into the library on this `Ui` since it was created.
    pub calls: u64,
    /// Out: the text cache's layout count.
    pub text_layouts: u64,
    /// Out: views whose label starts with `rebuilt_prefix` the last frame built.
    pub rebuilt: u64,
    /// Out: the last frame's wall time per phase, in nanoseconds.
    pub phase_nanos: [u64; 7],
    /// Out: the last frame's thread CPU time per phase, in nanoseconds (read when `has_cpu` is 1).
    pub phase_cpu_nanos: [u64; 7],
}

impl PxuiDiagnostics {
    /// A record asking for nothing but the counters: timing left as it is, every view counted.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ffi::PxuiDiagnostics::new().size as usize, core::mem::size_of::<pxui_types::ffi::PxuiDiagnostics>());
    /// ```
    pub fn new() -> PxuiDiagnostics {
        PxuiDiagnostics { size: core::mem::size_of::<PxuiDiagnostics>() as u32, timing: 0, has_text_layouts: 0, has_cpu: 0, reserved: 0, rebuilt_prefix: Str::default(),
            calls: 0, text_layouts: 0, rebuilt: 0, phase_nanos: [0; 7], phase_cpu_nanos: [0; 7] }
    }
}

/// A label's style as it crosses the ABI (`pxui_ui_label`, `pxui_paint_label`, `PxuiLabelConfig::style`; ABI 1.45): `Label` with
/// each optional field's bit in `set` (`SET_*`); a field whose bit is clear is the theme's label style. `color` is an app colour
/// token; `digits` 1 draws the text one cached character at a time with every digit on the advance of `0`. The label's box (height,
/// role, whole pixels) crosses beside it as a `PxuiLabelBox` (ABI 1.54).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiLabelStyle {
    /// Size of this record in bytes.
    pub size: u32,
    /// Bits indicating which values are explicitly set.
    pub set: u32,
    /// Typeface or font descriptor used for text.
    pub font: u16,
    /// Font weight, with zero selecting the default where supported.
    pub weight: u16,
    /// Colour of the element.
    pub color: u16,
    /// Maximum number of lines, or zero for the default.
    pub max_lines: u16,
    /// Whether to shape uppercase text.
    pub caps: u8,
    /// Digit spacing option.
    pub digits: u8,
    /// Truncation option when text overflows.
    pub ellipsis: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u8,
    /// Fractional font size in logical points.
    pub size_points: f32,
    /// Additional letter spacing in ems.
    pub tracking: f32,
    /// Line advance in logical points.
    pub line_height: f32,
    /// Text inset in logical points.
    pub pad: f32,
    /// Whether content wraps when it reaches the available extent.
    pub wrap: u32,
    /// Alignment within the available space.
    pub align: u32,
    /// Feature bits available to this operation.
    pub features: u32,
    /// Width in logical points.
    pub width: PxuiSizeDesc,
}

impl PxuiLabelStyle {
    /// Bit applying the explicit font override.
    pub const SET_FONT: u32 = 1;
    /// Bit applying the explicit size override.
    pub const SET_SIZE: u32 = 2;
    /// Bit applying the explicit weight override.
    pub const SET_WEIGHT: u32 = 4;
    /// Bit applying the explicit tracking override.
    pub const SET_TRACKING: u32 = 8;
    /// Bit applying the explicit caps override.
    pub const SET_CAPS: u32 = 16;
    /// Bit applying the explicit color override.
    pub const SET_COLOR: u32 = 32;
    /// Bit applying the explicit width override.
    pub const SET_WIDTH: u32 = 64;
    /// Bit applying the explicit line height override.
    pub const SET_LINE_HEIGHT: u32 = 128;
    /// Bit applying the explicit pad override.
    pub const SET_PAD: u32 = 256;
    /// Every bit a 1.45 library knows.
    pub const SET_ALL: u32 = 511;
}

/// A label's box as it crosses the ABI beside its `PxuiLabelStyle` (`pxui_ui_label`, `PxuiLabelConfig::label_box`; ABI 1.54):
/// `height` the node's height with its lines centred in it when `SET_HEIGHT` is in `set`, `role` its accessible role when `SET_ROLE`
/// is (clear: `Role::Label`), `whole_pixels` 1 to round a fitted width up to whole pixels.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PxuiLabelBox {
    /// Size of this record in bytes.
    pub size: u32,
    /// Bits indicating which values are explicitly set.
    pub set: u32,
    /// Height in logical points.
    pub height: f32,
    /// Accessibility role code.
    pub role: u8,
    /// Whether to snap the box to drawable pixels.
    pub whole_pixels: u8,
    /// The body variant the widget is built with (0: the theme's), read as its registered body's config `variant`.
    pub variant: u16,
}

impl PxuiLabelBox {
    /// `height` is set.
    pub const SET_HEIGHT: u32 = 1;
    /// `role` is set.
    pub const SET_ROLE: u32 = 2;
    /// Every bit this library knows.
    pub const SET_ALL: u32 = 3;
}

impl From<&crate::Label> for PxuiLabelBox {
    /// The box record of `label`: nothing set (and `whole_pixels` 0) for a label that leaves its box to the style.
    fn from(l: &crate::Label) -> PxuiLabelBox { PxuiLabelBox::from(l.fields()) }
}

impl From<&crate::text::LabelFields> for PxuiLabelBox {
    /// The box record of a label's fields.
    fn from(l: &crate::text::LabelFields) -> PxuiLabelBox {
        let role = l.role != crate::a11y::Role::Label;
        PxuiLabelBox {
            size: core::mem::size_of::<PxuiLabelBox>() as u32,
            set: if l.height.is_some() { Self::SET_HEIGHT } else { 0 } | if role { Self::SET_ROLE } else { 0 },
            height: l.height.unwrap_or(0.0), role: if role { l.role as u8 } else { 0 }, whole_pixels: l.whole_pixels as u8, variant: l.variant,
        }
    }
}

impl From<&crate::Label> for PxuiLabelStyle {
    /// The record of `label`, with `digits` 0.
    fn from(l: &crate::Label) -> PxuiLabelStyle { PxuiLabelStyle::from(l.fields()) }
}

impl From<&crate::text::LabelFields> for PxuiLabelStyle {
    /// The record of a label's fields, with `digits` 0.
    fn from(l: &crate::text::LabelFields) -> PxuiLabelStyle {
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
            caps: l.caps.unwrap_or(false) as u8, digits: 0, ellipsis: l.ellipsis as u8, reserved: 0,
            size_points: l.size.unwrap_or(0.0), tracking: l.tracking.unwrap_or(0.0), line_height: l.line_height.unwrap_or(0.0), pad: l.pad.unwrap_or(0.0),
            wrap: l.wrap as u32, align: l.align as u32, features: l.features, width,
        }
    }
}

/// The C record of a resolved text style (`pxui_shaper_*`, `pxui_text_*`, `pxui_paint_text_*`, `pxui_ui_resolve_label`): every field
/// concrete, tracking in points, `wrap`/`align`/`ellipsis` the numbers of `text::Wrap`/`TextAlign`/`Ellipsis`. For C callers and the
/// library's own conversions; Rust code gets an opaque `pxui::TextStyle` from `Ui::resolve`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PxuiTextStyle {
    /// The record's size in bytes (`size_of::<PxuiTextStyle>()`).
    pub size: u32,
    /// The font family: an index into the shaper's font order.
    pub font: u16,
    /// The weight (400 regular, 700 bold).
    pub weight: u16,
    /// The size in points.
    pub size_points: f32,
    /// The height of one line in points, 0 for 1.2 times the size.
    pub line_height: f32,
    /// `Wrap as u32`: how lines break inside the width.
    pub wrap: u32,
    /// `TextAlign as u32`: where lines sit inside the width.
    pub align: u32,
    /// The most lines shown, 0 for all.
    pub max_lines: u16,
    /// 1 for italic.
    pub italic: u8,
    /// `Ellipsis as u8`: how cut text ends. max_lines 0 uses one visible line when enabled.
    pub ellipsis: u8,
    /// OpenType feature bits (`FEATURE_TABULAR_NUMBERS`); 0 preserves the font defaults.
    pub features: u32,
    /// Extra space after every glyph, in points.
    pub tracking: f32,
}

impl Default for PxuiTextStyle {
    fn default() -> Self {
        Self {
            size: core::mem::size_of::<Self>() as u32,
            font: 0,
            weight: 400,
            size_points: 14.0,
            line_height: 0.0,
            wrap: 0,
            align: 0,
            max_lines: 0,
            italic: 0,
            ellipsis: 0,
            features: 0,
            tracking: 0.0,
        }
    }
}

impl PxuiTextStyle {
    /// The `features` bit for tabular numbers: every digit one width (`Label::tabular`).
    pub const FEATURE_TABULAR_NUMBERS: u32 = 1;
}

/// C mirror of `Size`; kinds are Px=0, Pct=1, Rel=2, Grow=3, Fit=4.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiSizeDesc {
    /// Discriminant that determines the meaning of the remaining fields.
    pub kind: u32,
    /// Value carried by this record.
    pub value: f32,
    /// Callback that computes length from the available extent.
    pub relative: Option<extern "C" fn(f32) -> f32>,
}
impl PxuiSizeDesc {
    /// Size in logical points.
    pub const PX: u32 = 0;
    /// Size as a fraction of the available extent.
    pub const PCT: u32 = 1;
    /// Size computed by a callback.
    pub const REL: u32 = 2;
    /// Size that fills the available extent.
    pub const GROW: u32 = 3;
    /// Size determined by the contents.
    pub const FIT: u32 = 4;
}

/// Size-prefixed (`size_of::<PxuiAnchorDesc>()`, 40): a record whose `size` is short is refused; `Default` sets it.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiAnchorDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Nonzero when `anchor` is present.
    pub has_anchor: u8,
    /// Whether placement may switch sides to stay visible.
    pub flip: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 2],
    /// Rectangle in logical points.
    pub rect: Rect,
    /// Preferred side of the anchor rectangle.
    pub side: u32,
    /// Alignment within the available space.
    pub align: u32,
    /// Spacing between adjacent elements in logical points.
    pub gap: f32,
    /// Outer spacing in logical points.
    pub margin: f32,
}
impl Default for PxuiAnchorDesc {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, has_anchor: 0, flip: 0, reserved: [0; 2], rect: Rect::default(), side: 0, align: 0, gap: 0.0, margin: 0.0 } }
}

/// C representation of a font choice.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiFontDesc {
    /// Font family identifier.
    pub family: u16,
    /// Font weight, with zero selecting the default where supported.
    pub weight: u16,
    /// The size in points, fractions kept, in the 1.x record's alignment hole (offsets unchanged); read only while it agrees with
    /// `size` within half a point, so a 1.x caller's record keeps its integer meaning.
    #[doc(hidden)]
    pub size_points: f32,
    /// Nominal integer font size in logical points.
    pub size: i64,
    /// Additional letter spacing in ems.
    pub tracking: f32,
    /// Whether to shape uppercase text.
    pub caps: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 3],
}

/// C representation of an inner or outer shadow.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiShadowDesc {
    /// Colour of the element.
    pub color: Rgba,
    /// Distance from the relevant origin in logical points.
    pub offset: [f32; 2],
    /// Shadow blur radius in logical points.
    pub blur: f32,
    /// Additional shadow extent in logical points.
    pub spread: f32,
    /// Whether the shadow is drawn inside the element.
    pub inner: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 3],
}

/// C representation of a focus ring.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiRingDesc {
    /// Colour of the element.
    pub color: Rgba,
    /// Focus ring width in logical points.
    pub width: f32,
    /// Mask of sides that receive the border or ring.
    pub sides: u32,
}

/// Size-prefixed (`size_of::<PxuiLookDesc>()`, 160): read through a pointer, a record whose `size` is short is refused before a byte
/// past it is read; an array of looks (`states`) is spaced by its first element's `size`, every element carrying the same one.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiLookDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Background colour or gradient start.
    pub fill: Rgba,
    /// Optional colour at the end of the gradient.
    pub gradient: Rgba,
    /// Nonzero when `gradient` is present.
    pub has_gradient: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 3],
    /// Gradient direction in degrees.
    pub angle: f32,
    /// Border colour.
    pub border: Rgba,
    /// Border width in logical points.
    pub border_w: f32,
    /// Mask of sides that receive the border or ring.
    pub sides: u32,
    /// Corner radius values in logical points.
    pub radius: [f32; 4],
    /// Outer and inner shadows in paint order.
    pub shadows: [PxuiShadowDesc; 2],
    /// Borrowed UTF-8 text.
    pub text: Rgba,
    /// Typeface or font descriptor used for text.
    pub font: PxuiFontDesc,
    /// Multiplier for the element alpha, from zero to one.
    pub opacity: f32,
    /// Focus ring appearance.
    pub ring: PxuiRingDesc,
    /// Selection endpoints.
    pub selection: Rgba,
    /// Caret position or colour, as specified by the record.
    pub caret: Rgba,
    /// Whether the border uses a dashed line.
    pub dashed: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved_tail: [u8; 3],
}
impl Default for PxuiLookDesc {
    fn default() -> Self {
        Self { size: core::mem::size_of::<Self>() as u32, fill: Rgba::default(), gradient: Rgba::default(), has_gradient: 0, reserved: [0; 3], angle: 0.0, border: Rgba::default(), border_w: 0.0, sides: 0,
            radius: [0.0; 4], shadows: [PxuiShadowDesc::default(); 2], text: Rgba::default(), font: PxuiFontDesc::default(), opacity: 0.0, ring: PxuiRingDesc::default(),
            selection: Rgba::default(), caret: Rgba::default(), dashed: 0, reserved_tail: [0; 3] }
    }
}

/// C representation of a procedural mark.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiMarkDesc {
    /// Discriminant that determines the meaning of the remaining fields.
    pub kind: u32,
    /// Direction in which the mark points.
    pub direction: u32,
    /// Width in logical points.
    pub width: f32,
    /// Whether the line rises from left to right.
    pub rising: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 3],
}

/// Size-prefixed (`size_of::<PxuiStateDesc>()`, 12): embedded in the body configs (the library writes it) and read by `pxui_ui_style`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiStateDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Whether the pointer is over the element.
    pub hovered: u8,
    /// Whether the pointer or keyboard is pressing the element.
    pub pressed: u8,
    /// Whether the element holds keyboard focus.
    pub focused: u8,
    /// Whether the element is disabled.
    pub disabled: u8,
    /// Whether the element is checked.
    pub checked: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 3],
}
impl Default for PxuiStateDesc {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, hovered: 0, pressed: 0, focused: 0, disabled: 0, checked: 0, reserved: [0; 3] } }
}
impl From<PxuiStateDesc> for crate::theme::State {
    /// The record as a `State` (a nonzero byte is true), so a body config's `state` (and a search field's `clear`) reads typed:
    /// `State::from(config.state)`, for `States::pick` and `Ui::style`.
    fn from(s: PxuiStateDesc) -> crate::theme::State {
        crate::theme::State { hovered: s.hovered != 0, pressed: s.pressed != 0, focused: s.focused != 0, disabled: s.disabled != 0, checked: s.checked != 0 }
    }
}

/// Size-prefixed (`size_of::<PxuiCurveDesc>()`, 164).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiCurveDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Discriminant that determines the meaning of the remaining fields.
    pub kind: u32,
    /// Animation duration in seconds.
    pub duration: f32,
    /// Cubic Bézier control coordinates in x1, y1, x2, y2 order.
    pub points: [f32; 4],
    /// Natural angular frequency of the spring.
    pub omega: f32,
    /// Number of items.
    pub count: u32,
    /// Normalized sample positions along the animation.
    pub positions: [f32; 16],
    /// Values associated with the record in declared order.
    pub values: [f32; 16],
}
impl Default for PxuiCurveDesc {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, kind: 0, duration: 0.0, points: [0.0; 4], omega: 0.0, count: 0, positions: [0.0; 16], values: [0.0; 16] } }
}
impl From<crate::Curve> for PxuiCurveDesc {
    /// The record of `value` (its kind and the fields that kind reads; the rest zero).
    fn from(value: crate::Curve) -> PxuiCurveDesc {
        use crate::Curve;
        match value {
            Curve::Bezier { duration, points } => PxuiCurveDesc { kind: PxuiCurveDesc::BEZIER, duration, points, ..PxuiCurveDesc::default() },
            Curve::Spring { omega } => PxuiCurveDesc { kind: PxuiCurveDesc::SPRING, omega, ..PxuiCurveDesc::default() },
            Curve::Table { duration, count, positions, values } => PxuiCurveDesc { kind: PxuiCurveDesc::TABLE, duration, count, positions, values, ..PxuiCurveDesc::default() },
            Curve::Steps { duration, count } => PxuiCurveDesc { kind: PxuiCurveDesc::STEPS, duration, count, ..PxuiCurveDesc::default() },
        }
    }
}

impl PxuiCurveDesc {
    /// Selects the unset or inactive option.
    pub const NONE: u32 = 0;
    /// Selects cubic Bézier interpolation.
    pub const BEZIER: u32 = 1;
    /// Selects spring interpolation.
    pub const SPRING: u32 = 2;
    /// Selects sampled interpolation.
    pub const TABLE: u32 = 3;
    /// Selects a step curve.
    pub const STEPS: u32 = 4;
}

impl PxuiMarkDesc {
    /// Selects the unset or inactive option.
    pub const NONE: u32 = 0;
    /// Draws a chevron.
    pub const CHEVRON: u32 = 1;
    /// Draws a diagonal line.
    pub const LINE: u32 = 2;
    /// Points downward.
    pub const DIR_DOWN: u32 = 0;
    /// Points upward.
    pub const DIR_UP: u32 = 1;
    /// Points left.
    pub const DIR_LEFT: u32 = 2;
    /// Points right.
    pub const DIR_RIGHT: u32 = 3;
}

/// What `pxui_ui_signal` reports for one box. `keys` is borrowed from the `Ui` and valid until the next frame.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiSignal {
    /// Size of this record in bytes.
    pub size: u32,
    /// Bit flags defined for this record.
    pub flags: u32,
    /// Nonzero when `subtree_press` is present.
    pub has_subtree_press: u8,
    /// Code of the pointer button associated with the interaction.
    pub button: u8,
    /// Nonzero when `a11y_value` is present.
    pub has_a11y_value: u8,
    /// Nonzero when `a11y_select` is present.
    pub has_a11y_select: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 4],
    /// Origin of the active press inside this subtree.
    pub subtree_press: i64,
    /// Number of consecutive clicks.
    pub clicks: i64,
    /// Pointer displacement from the start of the drag.
    pub drag: [f32; 2],
    /// Pointer position in logical points.
    pub pointer: [f32; 2],
    /// Pointer position at press in logical points.
    pub press_point: [f32; 2],
    /// Pointer position at release in logical points.
    pub release_point: [f32; 2],
    /// Scroll displacement in logical points.
    pub scroll: [f32; 2],
    /// Scale change reported by the pinch gesture.
    pub pinch: f64,
    /// New value requested by assistive technology.
    pub a11y_value: f64,
    /// Selection requested by assistive technology.
    pub a11y_select: [u32; 2],
    /// Key codes currently held down.
    pub keys: Strided<Event>,
}

/// Result of a behavior-only scrubber; flag bits are preview, commit and held, respectively.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiScrubOut {
    /// Size of this record in bytes.
    pub size: u32,
    /// Bit flags defined for this record.
    pub flags: u32,
    /// Handle of the corresponding UI node.
    pub node: PxuiBox,
    /// Intermediate value while the gesture is active.
    pub preview: f32,
    /// Final value committed by the gesture.
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
    /// Size of this record in bytes.
    pub size: u32,
    /// Mask of gestures this operation recognizes.
    pub kinds: u32,
    /// Requested visible extent or target range.
    pub reveal: f32,
    /// Gesture activation threshold in logical points.
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
    /// Size of this record in bytes.
    pub size: u32,
    /// Width of the leading action region in logical points.
    pub leading: f32,
    /// Width of the trailing action region in logical points.
    pub trailing: f32,
    /// Whether to activate a full swipe.
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
    /// Size of this record in bytes.
    pub size: u32,
    /// Fraction of the available distance.
    pub fraction: f32,
    /// Velocity in logical points per second.
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
    /// Size of this record in bytes.
    pub size: u32,
    /// Bit flags defined for this record.
    pub flags: u32,
    /// Distance from the relevant origin in logical points.
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
    /// Size of this record in bytes.
    pub size: u32,
    /// Bit flags defined for this record.
    pub flags: u32,
    /// Progress of the swipe gesture.
    pub swipe: f32,
    /// Progress of the navigation gesture.
    pub back: f32,
    /// Progress of the pull gesture.
    pub pull: f32,
}
impl Default for PxuiGestureOut {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, flags: 0, swipe: 0.0, back: 0.0, pull: 0.0 } }
}
impl Default for PxuiSignal {
    fn default() -> Self {
        Self { size: core::mem::size_of::<Self>() as u32, flags: 0, has_subtree_press: 0, button: u8::MAX, has_a11y_value: 0, has_a11y_select: 0,
            reserved: [0; 4], subtree_press: 0, clicks: 0, drag: [0.0; 2], pointer: [0.0; 2], press_point: [0.0; 2],
            release_point: [0.0; 2], scroll: [0.0; 2], pinch: 1.0, a11y_value: 0.0, a11y_select: [0; 2], keys: Strided::default() }
    }
}
impl PxuiSignal {
    /// Pointer hovers over the node.
    pub const HOVERED: u32 = 1;
    /// This frame started a press on the node.
    pub const PRESSED: u32 = 2;
    /// The node still holds an active press.
    pub const HELD: u32 = 4;
    /// This frame released the active press.
    pub const RELEASED: u32 = 8;
    /// The node received a click.
    pub const CLICKED: u32 = 16;
    /// The node received a double click.
    pub const DOUBLE_CLICKED: u32 = 32;
    /// The node is being dragged.
    pub const DRAGGING: u32 = 64;
}

/// C header name for [`Input`].
pub type PxuiInput = Input;
/// C header name for [`Output`].
pub type PxuiOutput = Output;
/// C header name for [`PlatformOut`].
pub type PxuiPlatformOut = PlatformOut;
/// C header name for [`Damage`].
pub type PxuiDamage = Damage;
/// C header name for [`A11yValue`].
pub type PxuiA11yValue = A11yValue;
/// C header name for [`A11yNode`].
pub type PxuiA11yNode = A11yNode;
/// C header name for [`A11yAction`].
pub type PxuiA11yAction = A11yAction;
/// A span of actions, as `pxui_a11y_pump` hands them out.
pub type A11yActionSpan = Strided<A11yAction>;
/// C header name for [`A11yTree`].
pub type PxuiA11yTree = A11yTree;
/// C header name for [`ThemeTokens`].
pub type PxuiThemeTokens = ThemeTokens;

/// What a paint callback needs to know about its painter, read once per callback: the sizes, the clip and the theme tokens.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiPaintInfo {
    /// Size of this record in bytes.
    pub size: u32,
    /// Whether layout follows right-to-left direction.
    pub rtl: u8,
    /// Byte 0 is 1 during palette-aware shape painting. In that callback, shape color words may
    /// contain `Rgba::token` encodings. The other bytes are reserved and zero.
    pub reserved: [u8; 3],
    /// Text size in logical points.
    pub font_size: i64,
    /// Row height in logical points.
    pub row_height: i64,
    /// Theme density multiplier.
    pub density: i64,
    /// Clip region in logical points.
    pub clip: Rect,
    /// Corner radii of the clip in logical points.
    pub clip_radius: f32,
    /// Theme snapshot used for the current frame.
    pub theme: ThemeTokens,
}

/// `value_field`'s place, text and options. Each `HAS_*` flag says whether the option next to it is set; `lead` and `trail` are
/// the framing texts, coloured by `lead_color` and `trail_color`. The slide, scrub and validity callbacks cross beside it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiValueFieldDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Bit flags defined for this record.
    pub flags: u32,
    /// Text size in logical points.
    pub font_size: i64,
    /// Fraction of the available distance.
    pub fraction: f32,
    /// Additional letter spacing in ems.
    pub tracking: f32,
    /// Optional line size override for the value field.
    pub line: Rgba,
    /// Colour of the text before the value.
    pub lead_color: Rgba,
    /// Colour of the text after the value.
    pub trail_color: Rgba,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 4],
    /// Horizontal coordinate in logical points.
    pub x: f32,
    /// Vertical coordinate in logical points.
    pub y: f32,
    /// Width in logical points.
    pub width: f32,
    /// Height in logical points.
    pub height: f32,
    /// Text displayed when the field is not being edited.
    pub display: Str,
    /// Text shown when the editor is empty.
    pub placeholder: Str,
    /// Text or icon before the main value.
    pub lead: Str,
    /// Text after the main value.
    pub trail: Str,
    /// Label supplied by the caller.
    pub label: Str,
    /// Value carried by this record.
    pub value: Str,
}

impl PxuiValueFieldDesc {
    /// Bit positioning the value on the leading side.
    pub const LEFT: u32 = 1;
    /// Bit allowing edits to the value.
    pub const EDIT: u32 = 2;
    /// Bit selecting a field without its normal background.
    pub const BARE: u32 = 4;
    /// Bit enabling drag behavior for a retained field.
    pub const KEPT_DRAG: u32 = 8;
    /// Bit indicating that the size option is present.
    pub const HAS_SIZE: u32 = 16;
    /// Bit indicating that the fraction option is present.
    pub const HAS_FRACTION: u32 = 32;
    /// Bit indicating that the tracking option is present.
    pub const HAS_TRACKING: u32 = 64;
    /// Bit indicating that the line option is present.
    pub const HAS_LINE: u32 = 128;
    /// Bit indicating that the lead option is present.
    pub const HAS_LEAD: u32 = 256;
    /// Bit indicating that the trail option is present.
    pub const HAS_TRAIL: u32 = 512;
    /// Bit indicating that the display option is present.
    pub const HAS_DISPLAY: u32 = 1024;
    /// Bit indicating that the placeholder option is present.
    pub const HAS_PLACEHOLDER: u32 = 2048;
    /// Mask of all supported bits in this record.
    pub const KNOWN: u32 = 4095;
}

/// A popup sheet: `popup`'s plain arguments (its build callback crosses beside this record).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiPopupDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Nonzero when `stroke` is present.
    pub has_stroke: u8,
    /// Nonzero when `max_height` is present.
    pub has_max_height: u8,
    /// Whether the opening press can also dismiss the popup.
    pub dismiss_initial: u8,
    /// Whether the popup is attached to the parent element.
    pub attached: u8,
    /// Appearance of the outline.
    pub stroke: Rgba,
    /// Maximum height in logical points.
    pub max_height: f32,
    /// Horizontal coordinate in logical points.
    pub x: f32,
    /// Vertical coordinate in logical points.
    pub y: f32,
    /// Width in logical points.
    pub width: f32,
    /// Height in logical points.
    pub height: f32,
    /// Label supplied by the caller.
    pub label: Str,
}

/// A kit button with options (`button_with`): its arguments beside the build callback that replaces its text. Kinds 0, 1, 2 are
/// the kit's size, `Px` and `Grow`; `justify` 0, 1, 2 is start, centre, end; `states` null is the theme's button, else six looks (idle, hover, pressed, focus, disabled, checked), each checked as `pxui_ui_style` checks its look.
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
    /// ABI 1.54 (a 96-byte record is the 1.48 one, read without it): 0 or 1, the box exactly `w` by `h` with nothing around it in
    /// its line and its text centred (`Button::centered`).
    pub centered: u8,
    /// Zero.
    pub reserved2: u8,
    /// The body variant the widget is built with (0: the theme's), read as its registered body's config `variant`.
    pub variant: u16,
    /// Zero.
    pub reserved5: [u8; 4],
    /// ABI 1.56 (a 96- or 104-byte record is read without it): 0 or 1, the text is drawn through the label line box in `label` and
    /// `label_box` (`Button::label`) in place of the kit's text.
    pub has_label: u8,
    /// The contents' axis (`Button::axis`): 0 the kit's, 1 row, 2 column, 3 stack.
    pub axis: u8,
    /// The contents' place across the box (`Button::content_align`): 0 the kit's, 1 start, 2 centre, 3 end.
    pub content_align: u8,
    /// 0 or 1: a press focuses the button (`Button::focus_on_click`); 0 is the kit's, focus by Tab only.
    pub focus_on_click: u8,
    /// The space between the contents in points (`Button::gap`), finite and not negative.
    pub gap: f32,
    /// The text's style when `has_label` is 1 (as `pxui_ui_label` takes it).
    pub label: PxuiLabelStyle,
    /// The text's box when `has_label` is 1 (as `pxui_ui_label` takes it).
    pub label_box: PxuiLabelBox,
    /// ABI 1.71 (a 96-, 104- or 192-byte record is read without these): the leading icon's glyph (empty: no glyph), drawn before the
    /// text in the look's text colour (`Button::icon`).
    pub lead: Str,
    /// The app's drawing of the leading icon in place of the glyph, or null: called at paint time as `PxuiIconButtonDesc::draw` is (the
    /// word `pxui_ui_icon_word` set before this call is its icon word).
    pub lead_draw: PxuiIconDraw,
    /// Passed to `lead_draw`; not owned: it must stay valid for as long as the frame can paint.
    pub lead_user: *mut core::ffi::c_void,
    /// A secondary line under the text (empty: none; `Button::detail`).
    pub detail: Str,
    /// A trailing text at the box's end, such as a shortcut (empty: none; `Button::accessory`).
    pub accessory: Str,
    /// 0 or 1: `detail` is drawn in `detail_label` and `detail_box` (else the theme's label style).
    pub has_detail_label: u8,
    /// 0 or 1: `accessory` is drawn in `accessory_label` and `accessory_box` (else the theme's label style).
    pub has_accessory_label: u8,
    /// Zero.
    pub reserved3: [u8; 6],
    /// The detail's style and box when `has_detail_label` is 1 (as `pxui_ui_label` takes them).
    pub detail_label: PxuiLabelStyle,
    /// The detail's box when `has_detail_label` is 1.
    pub detail_box: PxuiLabelBox,
    /// The accessory's style and box when `has_accessory_label` is 1 (as `pxui_ui_label` takes them).
    pub accessory_label: PxuiLabelStyle,
    /// The accessory's box when `has_accessory_label` is 1.
    pub accessory_box: PxuiLabelBox,
    /// ABI 1.91: the leading icon's square in points (`Button::icon_size`; finite, not negative; 0: the control-high one).
    pub icon_size: f32,
}

/// A modal dialog (ABI 1.53, `pxui_ui_dialog`): the sheet's `key`, the `title`, the button labels (at most 64) and their
/// flags as `PxuiDialogConfig` carries them (`default_at`, `cancel_at`: an index or -1; `danger`, `disabled`: bit `i` for button `i`),
/// the sheet's `width` in points (finite, not negative).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiDialogDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Width in logical points.
    pub width: f32,
    /// Key used to identify the element across frames.
    pub key: Str,
    /// Primary title shown to the user.
    pub title: Str,
    /// Pointer buttons currently held down.
    pub buttons: Slice<Str>,
    /// Index of the default button, with the no-button sentinel for none.
    pub default_at: i64,
    /// Index of the cancel button, with the no-button sentinel for none.
    pub cancel_at: i64,
    /// Whether to use the destructive action appearance.
    pub danger: u64,
    /// Whether the element is disabled.
    pub disabled: u64,
    /// ABI 1.71 (an 88-byte record is read without it): each button's detail line under its label (`DialogButton::detail`), as many as
    /// `buttons` or none (length 0); an empty detail is none.
    pub details: Slice<Str>,
    /// The body variant the widget is built with (0: the theme's), read as its registered body's config `variant`.
    pub variant: u16,
    /// Zero.
    pub reserved_variant: [u8; 6],
}

/// What a dialog's frame did (ABI 1.53): `chosen` is the button chosen when `has_chosen`, `dismissed` is 1 when the dialog closed
/// (Escape, a press outside, a lost window focus). The caller sets `size`; a shorter record is refused.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiDialogResult {
    /// Size of this record in bytes.
    pub size: u32,
    /// Nonzero when `chosen` is present.
    pub has_chosen: u8,
    /// Whether the surface closed without a choice.
    pub dismissed: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 2],
    /// Index of the chosen item.
    pub chosen: u64,
}

impl PxuiDialogResult {
    /// An empty record of this size, for a call to fill.
    pub const EMPTY: PxuiDialogResult = PxuiDialogResult { size: core::mem::size_of::<PxuiDialogResult>() as u32, has_chosen: 0, dismissed: 0, reserved: [0; 2], chosen: 0 };
}

/// A widget's identity and its accessible name, apart (ABI 1.68, every call that takes one): `key` is hashed under the parent's seed
/// and is identity only; `name` is what the widget shows and is read as (empty: none). The caller sets `size`; a shorter record and a
/// non-zero `reserved` are refused.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiKeyedDesc {
    /// `size_of::<PxuiKeyedDesc>()`.
    pub size: u32,
    /// The body variant the widget is built with (0: the theme's): a registered body of the widget's kind reads it as its config's `variant`.
    pub variant: u16,
    /// Zero.
    pub reserved: u16,
    /// The identity: a static string or `Key::label` of an item id; never display text.
    pub key: Str,
    /// The shown and accessible text (empty: none).
    pub name: Str,
}

impl PxuiKeyedDesc {
    /// The record for `key` and `name`, borrowing both for as long as the record is used.
    ///
    /// Allocates: never.
    /// Identity: `key` is the widget's identity; `name` is never part of it.
    /// Panics: never.
    ///
    /// ```
    /// let d = pxui_types::ffi::PxuiKeyedDesc::new("export", "Export");
    /// assert_eq!(d.size as usize, core::mem::size_of::<pxui_types::ffi::PxuiKeyedDesc>());
    /// ```
    pub fn new(key: &str, name: &str) -> PxuiKeyedDesc {
        PxuiKeyedDesc { size: core::mem::size_of::<PxuiKeyedDesc>() as u32, variant: 0, reserved: 0, key: Str { ptr: key.as_ptr(), len: key.len() }, name: Str { ptr: name.as_ptr(), len: name.len() } }
    }
}

/// How a modal is presented, apart from its key and name (ABI 1.86, `pxui_ui_modal`): the width, the presentation and the
/// drag-to-dismiss thresholds. The caller sets `size`; a shorter record and a non-zero `reserved` are refused.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiSheetDesc {
    /// `size_of::<PxuiSheetDesc>()`.
    pub size: u32,
    /// 1 when `width` is set, 0 for the presentation's own.
    pub has_width: u8,
    /// `PXUI_PRESENT_CENTERED` 0, `PXUI_PRESENT_BOTTOM` 1, `PXUI_PRESENT_FULL` 2 or `PXUI_PRESENT_TOP` 3 (ABI 1.90).
    pub present: u8,
    /// Zero.
    pub reserved: [u8; 2],
    /// The width in points (finite, not negative).
    pub width: f32,
    /// The fraction of the sheet's height a downward release past commits (finite, not negative; 0: a quarter).
    pub dismiss_fraction: f32,
    /// The downward release speed in points per second that commits (finite, not negative; 0: 600).
    pub dismiss_velocity: f32,
    /// ABI 1.90: a top-anchored modal's top edge in points below the content area's top (finite, not negative; read for present 3).
    pub top: f32,
    /// ABI 1.90: the height cap in points of a bottom sheet or a top-anchored modal (finite, not negative; 0: the presentation's own).
    pub max_h: f32,
}

/// One row of a picker's fill (ABI 1.86, the rows the `PxuiPickerRows` callback of `pxui_ui_picker` hands back): the label and
/// detail, the optional artwork or icon and the heading of the group the row starts (empty: none). The caller sets `size`; a different
/// size, an unknown `art_kind`, a non-zero `reserved`, a glyph that is not a Unicode scalar value or text that is not UTF-8 is refused.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiPickRow {
    /// `size_of::<PxuiPickRow>()`.
    pub size: u32,
    /// `PXUI_PICK_ART_NONE` 0, `PXUI_PICK_ART_GLYPH` 1 (`art` is a Unicode scalar value), `PXUI_PICK_ART_IMAGE` 2 (`art` is an
    /// `Images` id) or `PXUI_PICK_ART_DRAW` 3 (`art` is the app's 32-bit icon word, ABI 1.117).
    pub art_kind: u8,
    /// Zero.
    pub reserved: [u8; 3],
    /// The code point, the image id or the icon word, by `art_kind`.
    pub art: u64,
    /// The row's text.
    pub label: Str,
    /// The row's secondary text (empty: none).
    pub detail: Str,
    /// The heading of the group this row starts (empty: none).
    pub heading: Str,
}

impl PxuiPickRow {
    /// A row of `label` and `detail`, no artwork or heading.
    ///
    /// Allocates: never.
    /// Identity: none; a picker keys its rows by their index.
    /// Panics: never.
    ///
    /// ```
    /// const ROW: pxui_types::ffi::PxuiPickRow = pxui_types::ffi::PxuiPickRow::new("Song", "Artist");
    /// assert_eq!(ROW.size as usize, core::mem::size_of::<pxui_types::ffi::PxuiPickRow>());
    /// ```
    pub const fn new(label: &str, detail: &str) -> PxuiPickRow {
        PxuiPickRow {
            size: core::mem::size_of::<PxuiPickRow>() as u32, art_kind: 0, reserved: [0; 3], art: 0,
            label: Str { ptr: label.as_ptr(), len: label.len() }, detail: Str { ptr: detail.as_ptr(), len: detail.len() }, heading: Str { ptr: core::ptr::null(), len: 0 },
        }
    }
}

/// What happened to a kit button or icon button this frame, in one record (ABI 1.48, `pxui_ui_button_plain`,
/// `pxui_ui_button`, `pxui_ui_icon_button_plain`, `pxui_ui_icon_button`): the `CLICKED`, `HOVERED`, `HELD`,
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
    /// A secondary (right-button) press began on the box this frame (ABI 1.59).
    pub const SECONDARY: u32 = 32;
    /// An empty record of this size, for a call to fill.
    pub const EMPTY: PxuiResponse = PxuiResponse { size: core::mem::size_of::<PxuiResponse>() as u32, flags: 0, node: PxuiBox { index: 0, box_key: 0, box_slot: 0, generation: 0, ui: 0 } };
}

/// What a segmented control's frame chose (ABI 1.67, `pxui_ui_segmented`, `pxui_ui_pick`, `pxui_ui_tabs`, `pxui_ui_section_index`): `chosen` is the choice when `has_chosen` is 1 (0: no
/// option is chosen), `secondary` the option a secondary press began on this frame when `has_secondary` is 1. The caller sets `size`; a
/// shorter record is refused.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiSegmentedResult {
    /// Size of this record in bytes.
    pub size: u32,
    /// 1 when `chosen` is the choice; 0 when no option is chosen.
    pub has_chosen: u8,
    /// 1 when `secondary` is the option a secondary press began on this frame.
    pub has_secondary: u8,
    /// Zero.
    pub reserved: [u8; 2],
    /// The chosen option's index.
    pub chosen: u64,
    /// The index of the option a secondary press began on.
    pub secondary: u64,
}

impl PxuiSegmentedResult {
    /// An empty record of this size, for a call to fill.
    pub const EMPTY: PxuiSegmentedResult = PxuiSegmentedResult { size: core::mem::size_of::<PxuiSegmentedResult>() as u32, has_chosen: 0, has_secondary: 0, reserved: [0; 2], chosen: 0, secondary: 0 };
}

/// An icon drawing callback: `(user, paint, rect, ink, info)`. `paint` is the `PxuiPaint` of a paint callback and `info` is valid for the call.
pub type PxuiIconDraw = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, Rect, Rgba, *const PxuiPaintInfo)>;

/// An icon button with options (`icon_button`): kinds 0, 1, 2 are the kit's square, `Px` and `Grow`; `states` null is the theme's
/// `icon_button`, else six looks (idle, hover, pressed, focus, disabled, checked), each checked as `pxui_ui_style` checks its look.
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
    /// ABI 1.54 (a 96-byte record is the 1.46 one, read without it): two `Curve`s (`repr(C, u8)`, `PxuiCurve` in C), the press
    /// scale's in on press and out on release, valid for the call; null for the kit's springs (60 in, 20 out).
    pub press: *const crate::Curve,
    /// ABI 1.71 (a 96- or 104-byte record reads 0): 0 or 1, the button is an on/off key (`IconButton::selected`): accessible CHECKABLE,
    /// and CHECKED with its checked look when `checked` is 1. With `checkable` 0, `checked` is 0.
    pub checkable: u8,
    /// 0 or 1: the key is on (read when `checkable` is 1).
    pub checked: u8,
    /// Zero.
    /// The body variant the widget is built with (0: the theme's), read as its registered body's config `variant`.
    pub variant: u16,
    /// Zero.
    pub reserved2: [u8; 4],
}

/// A text field (`pxui_ui_text_field`): `key` is its identity among its siblings, `name` what a screen reader calls it (empty: unnamed),
/// `value` the text it shows; the three traits are the numbers of `PxuiTextInputOptions` (autocorrect 0 or 1, capitalization 0 to 3, action 0
/// to 2). The caller sets `size`; a shorter record is refused.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiTextFieldDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// 0 or 1: the platform keyboard corrects words while typing.
    pub autocorrect: u8,
    /// 0 none, 1 sentences, 2 words, 3 letters.
    pub capitalization: u8,
    /// The return key: 0 Return, 1 Done, 2 Search.
    pub action: u8,
    /// Zero.
    pub reserved: u8,
    /// The field's key.
    pub key: Str,
    /// The field's accessible name; empty for none.
    pub name: Str,
    /// The text the field shows.
    pub value: Str,
    /// The body variant the widget is built with (0: the theme's), read as its registered body's config `variant`.
    pub variant: u16,
    /// Zero.
    pub reserved_variant: [u8; 6],
}

/// A kit toggle with options (`pxui_ui_toggle`). `row_h` 0 is the kit's row height; `switch_w` and `switch_h` 0 are the kit's 28 x 14;
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
    /// ABI 1.55 (a 88-byte record is the 1.51 one, read without it): 1 when `style` is the title's text style, 0 for the theme's toggle font.
    pub has_style: u8,
    /// Zero.
    pub reserved2: u8,
    /// The body variant the widget is built with (0: the theme's), read as its registered body's config `variant`.
    pub variant: u16,
    /// Zero.
    pub reserved3: [u8; 4],
    /// The title's text style (`Toggle::label`), its unset fields the theme's toggle look; read when `has_style` is 1.
    pub style: PxuiLabelStyle,
}

/// ABI 1.85: a kit checkbox with its style (`pxui_ui_checkbox`; all zero but `size`, `key` and `text` is the plain checkbox).
/// `box_size` 0 is the kit's 12-point box; `look` null is the theme's checkbox, else six looks (idle, hover, pressed, focus, disabled,
/// checked); `disabled` 1 shows the disabled look and blocks presses and focus; `passive` 1 makes a mark with no input of its own. A click
/// is reported and the caller keeps `on`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiCheckboxDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// 0 or 1: the checkbox takes no input.
    pub disabled: u8,
    /// 0 or 1: the value now.
    pub on: u8,
    /// 0 or 1: a passive mark (no hit area, focus or hover; with empty text the box alone).
    pub passive: u8,
    /// Zero.
    pub reserved: u8,
    /// The box's side in points (finite, not negative; 0 the kit's 12).
    pub box_size: f32,
    /// Null for the theme's checkbox, else six looks.
    pub look: *const PxuiLookDesc,
    /// The checkbox's key (identity only, as `PxuiKeyedDesc.key`).
    pub key: Str,
    /// The text beside the box and the accessible name.
    pub text: Str,
    /// The body variant the widget is built with (0: the theme's), read as its registered body's config `variant`.
    pub variant: u16,
    /// Zero.
    pub reserved_variant: [u8; 6],
}

/// A kit segmented control (`pxui_ui_segmented`); the options go beside it as a slice of `PxuiPickItem`s. `h` 0 is the theme's control
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
    /// The current choice, clamped to the options; read only when `has_selected` is 1 (or the record is older than 1.67).
    pub selected: usize,
    /// ABI 1.55 (a 64-byte record is the 1.51 one, read without it): the strip's width in points, its segments sharing it (0: as `grow` says).
    pub w: f32,
    /// The segment's text inset and half the room around its text in points (a 64-byte record is 8).
    pub padding: f32,
    /// The least width of a segment in points (0: none).
    pub min_segment: f32,
    /// Zero.
    /// The body variant the widget is built with (0: the theme's), read as its registered body's config `variant`.
    pub variant: u16,
    /// Zero.
    pub reserved2: u16,
    /// Empty: each item carries its own accessible name (`PxuiPickItem.name`).
    pub names: Slice<Str>,
    /// ABI 1.67 (a 64- or 96-byte record is read without it, as 1): 1 when `selected` is the choice, 0 for no choice (final; `selected`
    /// then not read); the result then reports none.
    pub has_selected: u8,
    /// Zero.
    pub reserved3: [u8; 7],
    /// ABI 1.69 (a record shorter than this is read as 0): 1 for a press on an option to focus it (`Segmented::focus_on_click`), 0 for a press that
    /// chooses without focusing.
    pub focus_on_click: u8,
    /// Zero.
    pub reserved4: [u8; 7],
}

/// ABI 1.76: one item of a pick-one control (`pxui_ui_pick`, `pxui_ui_segmented`), borrowed for the call.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiPickItem {
    /// 1 when `id` keys the item (its node, focus and secondary press follow it through a reorder), 0 for an item keyed by its place.
    pub has_id: u8,
    /// Zero.
    pub reserved: [u8; 7],
    /// The item's id, read when `has_id` is 1.
    pub id: i64,
    /// The text shown.
    pub text: Str,
    /// The accessible name (empty: the text).
    pub name: Str,
    /// Additional text for the item's body (may be empty).
    pub detail: Str,
}

/// One text button of an inspector head.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiHeadActionDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Whether the action is active.
    pub active: u8,
    /// Whether the action can be triggered.
    pub usable: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 2],
    /// Action caption shown to the user.
    pub caption: Str,
    /// Keyboard shortcut caption.
    pub keycap: Str,
}

/// An inspector head. `kind`, `badge`, `index` and `reset` are read when their flag is set.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiHeaderDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Nonzero when `kind` is present.
    pub has_kind: u8,
    /// Nonzero when `badge` is present.
    pub has_badge: u8,
    /// Nonzero when `index` is present.
    pub has_index: u8,
    /// Nonzero when `reset` is present.
    pub has_reset: u8,
    /// Whether the reset control can be used.
    pub reset_enabled: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 3],
    /// Key used to identify the element across frames.
    pub key: Str,
    /// Discriminant that determines the meaning of the remaining fields.
    pub kind: Str,
    /// Optional badge displayed beside the title.
    pub badge: Str,
    /// Index within the owning collection.
    pub index: Str,
    /// Optional caption of the reset control.
    pub reset: Str,
    /// Primary title shown to the user.
    pub title: Str,
    /// Secondary text accompanying the main label.
    pub detail: Str,
    /// Actions exposed by the record.
    pub actions: Slice<PxuiHeadActionDesc>,
}

/// What a head reports: the name as edited (borrowed until the next frame), the pressed action (`chosen` when `has_chosen`)
/// and whether `reset` was pressed.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiHeadOut {
    /// Size of this record in bytes.
    pub size: u32,
    /// Nonzero when `chosen` is present.
    pub has_chosen: u8,
    /// Whether the reset control was activated.
    pub reset_pressed: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 2],
    /// Borrowed title returned by the header, valid for the frame.
    pub renamed: Str,
    /// Index of the chosen item.
    pub chosen: i64,
}



/// A build callback of a container descriptor: `(user, ui)`, the `ui` a borrowed `PxuiUi` valid for the call.
pub type UiBuild = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void)>;

// Descriptors of the kit widgets (`kit.rs` of `pxui_abi` and `pxui`): the one definition both sides use. Each starts with `size`,
// read before anything else: a descriptor passed alone may grow by appending fields; one in a span (menus, hints, head actions,
// keyframes) is read at its exact size, since a span's stride cannot grow.
/// Body builder called with a UI and its user pointer.
pub type PxuiBuild = UiBuild;
/// Grid builder called with each visible item index.
pub type PxuiGridBuild = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, usize)>;
/// A button's content callback (ABI 1.56, `pxui_ui_button`): `(user, ui, state)`, `state` the button's interaction state this
/// frame, valid for the call.
pub type PxuiButtonBuild = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *const PxuiStateDesc)>;

/// One menu and its ordered items.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiMenuDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u32,
    /// Primary title shown to the user.
    pub title: Str,
    /// Items in declaration order.
    pub items: Slice<Str>,
}
/// Geometry and builder callback for a virtual grid.
#[repr(C)]
pub struct PxuiGridListDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u32,
    /// Label supplied by the caller.
    pub label: Str,
    /// Number of items.
    pub count: usize,
    /// Cell width in logical points.
    pub cell_w: f32,
    /// Cell height in logical points.
    pub cell_h: f32,
    /// Callback that supplies the contents.
    pub build: PxuiGridBuild,
    /// User pointer forwarded unchanged to the callback.
    pub user: *mut core::ffi::c_void,
}

/// The item id of a virtual row: `(user, index) -> id` (ABI 1.49).
pub type PxuiRowKey = Option<extern "C" fn(*mut core::ffi::c_void, usize) -> i64>;
/// What a selection list or table row builder is told about its row (ABI 1.87; 1.119: size-prefixed, the layout this record owns
/// and the library's row record follows), written by the library and lent for the callback: its `size`, flags 0 or 1 (read them as
/// `!= 0`), the row's box (the empty box for the lead-in, heading and trailer), its content Y.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiCellState {
    /// `size_of::<PxuiCellState>()`, written by the library: a later library may lend a longer record.
    pub size: u32,
    /// 1 when the row is selected.
    pub selected: u8,
    /// 1 when the row takes presses (the caller did not flag it disabled).
    pub enabled: u8,
    /// 1 when the row is the focused list's keyboard row.
    pub focused: u8,
    /// 1 while a press is held on the row.
    pub held: u8,
    /// The row's box.
    pub handle: PxuiBox,
    /// The part's logical content Y in points.
    pub y: f32,
    /// Zero.
    pub reserved: u32,
}
/// A section or selection list part builder (ABI 1.87; the section list's since 1.103): `(user, ui, index, cell)`, index `usize::MAX`
/// the lead-in, `MAX - 1` the heading, `MAX - 2` the trailer, else a row.
pub type PxuiListCell = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, usize, *const PxuiCellState)>;
/// A table cell builder (ABI 1.87): `(user, ui, row, column, cell)`.
pub type PxuiTableCellBuild = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, usize, usize, *const PxuiCellState)>;

/// A section list's geometry for `pxui_ui_section_list` (ABI 1.87): the build is given the lead-in (`usize::MAX`), the heading
/// (`usize::MAX - 1`), each row in view and the trailer (`usize::MAX - 2`, with `trailing_height` > 0). The caller sets `size`; a shorter record and non-zero `reserved` are refused.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiSectionListDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// 1: the scrollbar is drawn over the rows.
    pub overlay_scrollbar: u8,
    /// 1: rows take the heights they lay out to (`row_height` their estimate).
    pub measured: u8,
    /// 1 (ABI 1.88): a payload carried between the rows shows the insertion line and its drop reports `drop_at`.
    pub reorder: u8,
    /// Zero.
    pub reserved: u8,
    /// Rows (at most a million).
    pub count: usize,
    /// Row height in points (positive, finite).
    pub row_height: f32,
    /// The scrolling lead-in's height in points (finite, not negative).
    pub leading_height: f32,
    /// The sticky heading's height in points (finite, not negative).
    pub header_height: f32,
    /// The trailer's height in points (finite, not negative).
    pub trailing_height: f32,
}

/// What a list reports (ABI 1.87; library-written up to the caller's `size`, which must cover this record): indices are
/// `usize::MAX` for none.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiListResult {
    /// Size of this record in bytes.
    pub size: u32,
    /// Zero.
    pub reserved: u32,
    /// The list's scroll box.
    pub handle: PxuiBox,
    /// The box of the row `activated` or `secondary` names (the empty box for none).
    pub row: PxuiBox,
    /// The row a double click or Enter (or, opening on click, a click) activated.
    pub activated: usize,
    /// The row whose selection the widget changed this frame.
    pub selected: usize,
    /// The row a secondary press began on.
    pub secondary: usize,
    /// The anchor Shift extends from, after this frame.
    pub anchor: usize,
    /// ABI 1.88: the insertion index a carried payload was dropped at (a reorderable list).
    pub drop_at: usize,
    /// ABI 1.89: a row moved this frame (a reorderable list): the row at `moved_from` now sits at `moved_to` (its final index).
    pub moved_from: usize,
    /// Destination row of the completed reorder.
    pub moved_to: usize,
}

impl PxuiListResult {
    /// An empty record of this size, for a call to fill.
    pub const EMPTY: PxuiListResult = PxuiListResult { size: core::mem::size_of::<PxuiListResult>() as u32, reserved: 0, handle: PxuiBox { index: 0, box_key: 0, box_slot: 0, generation: 0, ui: 0 }, row: PxuiBox { index: 0, box_key: 0, box_slot: 0, generation: 0, ui: 0 }, activated: usize::MAX, selected: usize::MAX, secondary: usize::MAX, anchor: usize::MAX, drop_at: usize::MAX, moved_from: usize::MAX, moved_to: usize::MAX };
}

/// A selection list for `pxui_ui_selection_list` (ABI 1.87): the section geometry, the row flags (one byte a row: 1 selected,
/// written by the list; 2 disabled, 4 live and 8 no drop (ABI 1.120: with `has_drop_look` the row is no drop zone and keeps its
/// look), written by the caller) and the anchor (`usize::MAX`: none); with `has_revision` 1 the
/// rows are kept while `revision`, their keys and their state hold (ABI 1.106). The caller sets `size`; a shorter record, a `mode`
/// above 3, a flag above 1, non-zero `reserved` or a flag span shorter than `count` are refused.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiSelectionListDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// ABI 1.128: the `SelectMode` discriminant (0 single, 1 multi, 2 toggle: a plain click toggles, 3 none: rows activate and
    /// nothing is selected); above 3 is refused.
    pub mode: u8,
    /// 1: the list is as tall as its rows and does not scroll.
    pub fit: u8,
    /// 1: a single click also activates the row.
    pub open_on_click: u8,
    /// 1: the scrollbar is drawn over the rows.
    pub overlay_scrollbar: u8,
    /// 1: rows take the heights they lay out to.
    pub measured: u8,
    /// 1 (ABI 1.88): rows take a payload carried between them (`PxuiRowConfig.drop` 1 and 2, `drop_at`).
    pub reorder: u8,
    /// 1 (ABI 1.88): rows take a payload over them, shown in `drop_look` (`PxuiRowConfig.drop` 3).
    pub has_drop_look: u8,
    /// 1 (ABI 1.100): the list draws no frame (no frame look, no 1-point inset).
    pub bare: u8,
    /// 1 (ABI 1.106): `revision` is the rows' content revision; a row is kept while it, the row's key and state hold.
    pub has_revision: u8,
    /// Zero.
    pub reserved: [u8; 3],
    /// The section geometry, as `PxuiSectionListDesc`'s.
    pub count: usize,
    /// Row height in points (positive, finite).
    pub row_height: f32,
    /// The lead-in's height in points.
    pub leading_height: f32,
    /// The heading's height in points.
    pub header_height: f32,
    /// The trailer's height in points.
    pub trailing_height: f32,
    /// One flag byte a row, updated in place.
    pub rows: PxuiU8MutSpan,
    /// The anchor row (`usize::MAX`: none).
    pub anchor: usize,
    /// ABI 1.88: the look a row takes while a payload is over it (read when `has_drop_look` is 1; checked as `pxui_ui_style` checks one).
    pub drop_look: PxuiLookDesc,
    /// ABI 1.106: the rows' content revision (read when `has_revision` is 1).
    pub revision: u64,
}

/// One table column (ABI 1.87): `share` > 0 splits the width the point columns leave by weight, else `w` points (0: hidden);
/// `grip` 1 lets a point column be resized, `sortable` 1 makes its heading cycle the sort, `id` keys it when `has_id` is 1 (else its
/// index). The caller sets `size`; flags above 1, non-zero `reserved`, a negative or non-finite size, a title past 64 KiB or a
/// repeated id are refused.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiColumn {
    /// Size of this record in bytes.
    pub size: u32,
    /// 1: the heading cycles the sort.
    pub sortable: u8,
    /// 1: a point column has a resize grip.
    pub grip: u8,
    /// 1: `id` keys the column.
    pub has_id: u8,
    /// Zero.
    pub reserved: u8,
    /// The heading text.
    pub title: Str,
    /// The column's id.
    pub id: i64,
    /// The share of the remaining width (0: a point column).
    pub share: f32,
    /// The point width (0: hidden).
    pub w: f32,
}

/// A column's laid-out width, by id (ABI 1.87).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiColumnWidth {
    /// The column's id.
    pub id: i64,
    /// Its width in points.
    pub w: f32,
    /// Zero.
    pub reserved: u32,
}

/// A caller-owned span of column widths, one a column, rewritten by the call.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiColumnWidthSpan {
    /// The first width.
    pub ptr: *mut PxuiColumnWidth,
    /// The number of widths.
    pub len: usize,
}

/// A heading for `pxui_ui_table_head` (ABI 1.87): the columns (at most 64), their widths (one a column), the sort (column id
/// `sort_id`, ascending when `ascending` is 1, none when `has_sort` is 0) and the height. The caller sets `size`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiTableHeadDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// 1: `sort_id` names the sorted column.
    pub has_sort: u8,
    /// 1: the sort is ascending.
    pub ascending: u8,
    /// Zero.
    pub reserved: [u8; 2],
    /// The columns.
    pub columns: Slice<PxuiColumn>,
    /// The widths, rewritten.
    pub widths: PxuiColumnWidthSpan,
    /// The sorted column's id.
    pub sort_id: i64,
    /// The heading height in points (positive, finite).
    pub h: f32,
    /// Zero.
    pub reserved2: u32,
}

/// What a heading reports (ABI 1.87; library-written, the caller's `size` covers it): the clicked heading's response (else the
/// heading row's), whether the sort changed (the new sort in `has_sort`/`ascending`/`sort_id`) and whether a width changed.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiHeadResult {
    /// Size of this record in bytes.
    pub size: u32,
    /// 1 when a click changed the sort.
    pub sorted: u8,
    /// 1 when a grip changed a width.
    pub resized: u8,
    /// 1 when `sort_id` names the sorted column.
    pub has_sort: u8,
    /// 1 when the sort is ascending.
    pub ascending: u8,
    /// The sorted column's id.
    pub sort_id: i64,
    /// The response.
    pub response: PxuiResponse,
}

/// A table for `pxui_ui_table` (ABI 1.87): the heading's fields, `rows` rows `row_h` high under a heading `head_h` high, the row
/// flags, anchor and revision as `PxuiSelectionListDesc`'s. The caller sets `size`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PxuiTableDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// ABI 1.130 (was `multi`, same byte): the selection mode, as `PxuiSelectionListDesc.mode`: 0 single, 1 multi, 2 toggle, 3 none.
    pub mode: u8,
    /// 1: `sort_id` names the sorted column.
    pub has_sort: u8,
    /// 1: the sort is ascending.
    pub ascending: u8,
    /// 1 (ABI 1.106): `revision` is the rows' content revision, as `PxuiSelectionListDesc.has_revision`.
    pub has_revision: u8,
    /// The columns.
    pub columns: Slice<PxuiColumn>,
    /// The widths, rewritten.
    pub widths: PxuiColumnWidthSpan,
    /// The sorted column's id.
    pub sort_id: i64,
    /// Rows (at most a million).
    pub rows: usize,
    /// Row height in points (positive, finite).
    pub row_h: f32,
    /// Heading height in points (positive, finite).
    pub head_h: f32,
    /// One flag byte a row, updated in place.
    pub flags: PxuiU8MutSpan,
    /// The anchor row (`usize::MAX`: none).
    pub anchor: usize,
    /// The `table_head` body variant the heading is built with (0: the theme's); the rows' `table_row` variant is the call's.
    pub head_variant: u16,
    /// ABI 1.130: 1: a press activates its row (`ListResult.activated`), as `PxuiSelectionListDesc.open_on_click`.
    pub open_on_click: u8,
    /// ABI 1.130: 1: no frame look and no 1-point inset, as `PxuiSelectionListDesc.bare`.
    pub bare: u8,
    /// Zero.
    pub reserved2: [u8; 4],
    /// ABI 1.106: the rows' content revision (read when `has_revision` is 1).
    pub revision: u64,
}

/// What a table reports (ABI 1.87; library-written, the caller's `size` covers it): the rows' and the heading's results.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiTableResult {
    /// Size of this record in bytes.
    pub size: u32,
    /// Zero.
    pub reserved: u32,
    /// The rows (its own `size` set too).
    pub rows: PxuiListResult,
    /// The heading (its own `size` set too).
    pub head: PxuiHeadResult,
}

/// Split pane geometry and callbacks for both bodies.
#[repr(C)]
pub struct PxuiSplitDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Layout direction for the children.
    pub axis: u32,
    /// Label supplied by the caller.
    pub label: Str,
    /// Fraction of the split assigned to the first pane.
    pub ratio: f32,
    /// Minimum extent of the first split pane in logical points.
    pub min_a: f32,
    /// Minimum extent of the second split pane in logical points.
    pub min_b: f32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u32,
    /// Callback that supplies the contents of the first split pane.
    pub build_a: PxuiBuild,
    /// User pointer forwarded unchanged to the first callback.
    pub user_a: *mut core::ffi::c_void,
    /// Callback that supplies the contents of the second split pane.
    pub build_b: PxuiBuild,
    /// User pointer forwarded unchanged to the second callback.
    pub user_b: *mut core::ffi::c_void,
}
/// Menu bar style and ordered menus.
#[repr(C)]
pub struct PxuiMenuBarDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// The body variant the widget is built with (0: the theme's), read as its registered body's config `variant`.
    pub variant: u16,
    /// Zero.
    pub reserved: u16,
    /// Label supplied by the caller.
    pub label: Str,
    /// Menus in display order.
    pub menus: Slice<PxuiMenuDesc>,
}
/// Indices identifying a choice from a menu bar.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiMenuOut {
    /// Size of this record in bytes.
    pub size: u32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u32,
    /// Index of the chosen menu.
    pub menu: usize,
    /// Index of the chosen item within the menu.
    pub item: usize,
}
/// Key caption and action text for a footer hint.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiHintDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u32,
    /// Key used to identify the element across frames.
    pub key: Str,
    /// Value carried by this record.
    pub value: Str,
}
/// Footer hints and optional trailing text.
#[repr(C)]
pub struct PxuiFooterDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Nonzero when `right` is present.
    pub has_right: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 3],
    /// Optional text at the trailing side of the footer.
    pub right: Str,
    /// Keyboard and action hints displayed in the footer.
    pub hints: Slice<PxuiHintDesc>,
}
/// Animation keyframe and the property it changes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiKeyframeDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Time in seconds on the host monotonic clock.
    pub time: f32,
    /// Property targeted by the animation.
    pub target: u32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u32,
    /// Values associated with the record in declared order.
    pub values: [f32; 4],
    /// Visual appearance of the element.
    pub look: PxuiLookDesc,
    /// Curve used to interpolate this animation.
    pub curve: PxuiCurveDesc,
}

/// Arguments and returned bounds for an inspector row.
#[repr(C)]
pub struct PxuiInspectorRowDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Nonzero when `width` is present.
    pub has_width: u8,
    /// Nonzero when `pin` is present.
    pub has_pin: u8,
    /// State of the inspector row pin.
    pub pin: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u8,
    /// Key used to identify the element across frames.
    pub key: Str,
    /// Label supplied by the caller.
    pub label: Str,
    /// Width in logical points.
    pub width: f32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved_tail: u32,
    /// Row index or handle, as specified by the operation.
    pub row: PxuiBox,
    /// Horizontal origin of the row control in logical points.
    pub control_x: f32,
    /// Vertical origin of the row control in logical points.
    pub control_y: f32,
    /// Width available to the row control in logical points.
    pub control_w: f32,
}
/// Inspector section expansion state and contents callback.
#[repr(C)]
pub struct PxuiInspectorSectionDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Current expansion state of the section.
    pub expanded: u8,
    /// Nonzero when `set` is present.
    pub has_set: u8,
    /// Requested expansion state.
    pub set_expanded: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u8,
    /// Key used to identify the element across frames.
    pub key: Str,
    /// Label supplied by the caller.
    pub label: Str,
    /// Callback that supplies the contents.
    pub build: PxuiBuild,
    /// User pointer forwarded unchanged to the callback.
    pub user: *mut core::ffi::c_void,
    /// Optional output for the section expansion state.
    pub out_open: u8,
}
/// Caption and value text for an inspector readout.
#[repr(C)]
pub struct PxuiInspectorValueDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Nonzero when `width` is present.
    pub has_width: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 3],
    /// Key used to identify the element across frames.
    pub key: Str,
    /// Label supplied by the caller.
    pub label: Str,
    /// Value carried by this record.
    pub value: Str,
    /// Width in logical points.
    pub width: f32,
}
/// Text and key for an inspector message.
#[repr(C)]
pub struct PxuiInspectorTextDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u32,
    /// Key used to identify the element across frames.
    pub key: Str,
    /// Borrowed UTF-8 text.
    pub text: Str,
}

/// Caller-owned writable bytes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiU8MutSpan {
    /// Pointer to the first element of the borrowed span.
    pub ptr: *mut u8,
    /// Number of elements in the span.
    pub len: usize,
}
/// Caller-owned writable span of 32-bit floats.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiF32MutSpan {
    /// Pointer to the first element of the borrowed span.
    pub ptr: *mut f32,
    /// Number of elements in the span.
    pub len: usize,
}
/// Caller-owned writable span of 32-bit unsigned integers.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiU32MutSpan {
    /// Pointer to the first element of the borrowed span.
    pub ptr: *mut u32,
    /// Number of elements in the span.
    pub len: usize,
}

/// Tree topology for a single item in preorder.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PxuiTreeNode {
    /// Nesting depth of the tree node.
    pub depth: u16,
    /// Number of immediate children.
    pub children: u8,
}

// ---- widget bodies (`pxui_body_register`): what a body is given, one record per kind. The records hold what the widget's behaviour
// already computed; the text beside each (a label, a value) is in the record, borrowed until the body returns. A body builds nodes
// with the ordinary calls on the `ui` it is given, finds nothing but the parts it names (`pxui_ui_part`) and runs the caller's content
// through its slots (`pxui_ui_slot`). Each record starts with `size` and grows only at its end; a flag is 0 or 1, a handle a `PxuiBox`.

/// A body of widget content the caller gave (a panel's children, a table cell): valid only during the body call that received it,
/// run with `pxui_ui_slot`. A slot that is stale is refused.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct PxuiSlot { #[doc = "Serial identifying the handle in its owner."] pub serial: u64 }

/// A registered body: `(user, ui, config)`, `ui` a borrowed `PxuiUi` valid for the call and `config` the kind's record below.
pub type PxuiBodyFn = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *const core::ffi::c_void)>;

/// A `_with` form's row or cell content: `(user, ui, row, column)` builds the children of one row or cell (`column` is 0 in a list
/// or a tree), `ui` a borrowed `PxuiUi` valid for the call.
pub type PxuiCellBuild = Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, usize, usize)>;


/// A tree for `pxui_ui_tree`: preorder `nodes`, one `open` flag a node, the selected node (`usize::MAX`: none) and the shown rows written
/// into `visible`; with `has_revision` 1 the shown rows are kept in `visible` between builds while `revision`, the spans and the tree's
/// own expansions hold (ABI 1.112). The caller sets `size`; a flag above 1 or non-zero `reserved` is refused.
#[repr(C)]
pub struct PxuiTreeViewDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// 1 (ABI 1.112): `revision` is the content revision of `nodes` and `open`; the caller changes it when it changes either.
    pub has_revision: u8,
    /// Zero.
    pub reserved: [u8; 3],
    /// Label supplied by the caller.
    pub label: Str,
    /// Borrowed nodes in traversal order.
    pub nodes: Slice<PxuiTreeNode>,
    /// Expansion flags indexed by node.
    pub open: PxuiU8MutSpan,
    /// Current selection or whether this item is selected.
    pub selected: *mut usize,
    /// Output storage for the visible traversal indices.
    pub visible: PxuiU32MutSpan,
    /// Count of visible traversal indices written.
    pub visible_len: *mut usize,
    /// Callback that supplies the contents.
    pub build: PxuiCellBuild,
    /// User pointer forwarded unchanged to the callback.
    pub user: *mut core::ffi::c_void,
    /// ABI 1.112: the content revision of `nodes` and `open` (read when `has_revision` is 1).
    pub revision: u64,
}


use crate::theme::States as PxuiStates;

// A config's fields after `@variant` come after its `variant` (the records grew at their end after ABI 1.52).
macro_rules! configs {
    ($($(#[$doc:meta])* $name:ident { $($(#[$fdoc:meta])* $f:ident: $t:ty),* $(,)? } $(@variant { $($(#[$adoc:meta])* $a:ident: $at:ty),* $(,)? })?)*) => {
        $($(#[$doc])* #[repr(C)] #[derive(Clone, Copy)] pub struct $name {
            /// Size of this record in bytes.
            pub size: u32, $($(#[$fdoc])* pub $f: $t,)*
            /// The variant the widget call asked for (`Button::variant` and its siblings; ABI 1.52), 0 when it asked for none: one registered body
            /// builds different structures by it. Read it only when `size` covers it.
            pub variant: u16,
            $($($(#[$adoc])* pub $a: $at,)*)?
        })*
        #[cfg(test)]
        fn config_layouts(check: &mut dyn FnMut(&str, usize, usize, &[(&str, usize)])) {
            use core::mem::{align_of, size_of};
            $(check(stringify!($name), size_of::<$name>(), align_of::<$name>(), &[("size", core::mem::offset_of!($name, size)), $((stringify!($f), core::mem::offset_of!($name, $f)),)* ("variant", core::mem::offset_of!($name, variant)) $($(, (stringify!($a), core::mem::offset_of!($name, $a)))*)?]);)*
        }
    };
}

configs! {
    /// `row` is entered and owns `Part::HIT`.
    PxuiToggleConfig {
        #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Current on/off state after input for this frame."] on: u8, #[doc = "Handle of the row the body is drawing."] row: PxuiBox, #[doc = "Borrowed label used by the widget."] label: Str,
        /// The row's height in points the caller asked for (0 the kit's).
        row_h: f32,
        /// The switch's width and height in points the caller asked for (0 the kit's 28 x 14); the knob is 6 less than its height.
        switch_w: f32,
        #[doc = "Requested switch height in logical points."] switch_h: f32,
        /// The looks of the switch and of its knob (`pick(checked)`): the caller's or the theme's; valid for the call. From ABI 1.124 they
        /// carry the caller's looks as given, with colour tokens intact: a C body resolves them with `pxui_ui_color`.
        track: *const PxuiStates,
        #[doc = "Look of the switch knob, valid for the body call."] knob: *const PxuiStates,
        /// The distance of the label and the switch from the row's sides in points the caller asked for (0 the kit's).
        inset: f32,
        /// 0 or 1: a press anywhere on the row toggles it (behaviour then ignores `Part::HIT`).
        press_row: u8,
        /// A secondary line under the label (empty: none); valid for the call.
        detail: Str,
    } @variant {
        /// ABI 1.55 (read when `size` covers it): 1 when the caller gave the title's text style (`Toggle::label`) in `style`, 0 for the
        /// theme's toggle font.
        has_style: u8,
        /// Zero.
        reserved2: [u8; 5],
        /// The title's text style; unset fields are the theme's toggle look.
        style: PxuiLabelStyle,
        /// ABI 1.58 (read when `size` covers it): the value the caller passed (the stored value before this frame's flip); `on` is the
        /// value after it.
        stored: u8,
        /// 1 when the toggle flipped this frame (a press, a key or an assistive action).
        clicked: u8,
        /// Zero.
        reserved3: [u8; 6],
    }
    /// `track` is entered; `fraction` is the value's place, 0 to 1. Behaviour reads `Part::RAIL` and `Part::THUMB`.
    PxuiSliderConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Current progress as a fraction from zero to one."] fraction: f32, #[doc = "Handle of the entered track."] track: PxuiBox, #[doc = "Borrowed label used by the widget."] label: Str }
    /// Body configuration for a progress indicator.
    PxuiProgressConfig { #[doc = "Nonzero when a progress value was supplied."] has_value: u8, #[doc = "Value supplied to the widget."] value: f32, #[doc = "Handle of the row the body is drawing."] row: PxuiBox, #[doc = "Borrowed label used by the widget."] label: Str }
    /// `states.pick(state)` is the look the caller asked for, as the caller gave it (U2b.4.30: its colours may name tokens, which `pxui_ui_style` resolves; resolve one with `pxui_ui_color` before a C paint call), or the theme's (resolved); `states` is valid for the call.
    PxuiButtonConfig {
        #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Whether the body should omit its standard background."] bare: u8, #[doc = "Nonzero when called for an icon-only button."] icon: u8, #[doc = "Whether the button aligns to the end of its line."] at_end: u8, #[doc = "Handle of the line containing the widget."] line: PxuiBox, #[doc = "Handle of the row the body is drawing."] row: PxuiBox, #[doc = "Borrowed looks for the widget states, valid for the body call."] states: *const PxuiStates, #[doc = "Borrowed label used by the widget."] label: Str,
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
    } @variant {
        /// ABI 1.54 (read when `size` covers it): 1 when the caller asked for `Button::centered`, the box exactly its size with nothing
        /// around it in its line and its text centred in it.
        centered: u8,
        /// ABI 1.127 (in a reserved byte): 1 when the caller asked for `Button::danger`, a destructive action the built-in body inks in
        /// the theme's error colour.
        danger: u8,
        /// Zero.
        reserved2: [u8; 8],
        /// ABI 1.56 (read when `size` covers it): 1 when the caller gave the text a style (`Button::label`): draw it through the label
        /// line box in `text_style` and `text_box`, inked in the look's text colour when the style sets none.
        has_label: u8,
        /// The contents' axis the caller asked for (`Button::axis`): 0 the kit's, 1 row, 2 column, 3 stack.
        axis: u8,
        /// The contents' place across the box the caller asked for (`Button::content_align`): 0 the kit's, 1 start, 2 centre, 3 end;
        /// down the box they are centred.
        content_align: u8,
        /// Zero.
        reserved3: u8,
        /// The space between the contents in points (`Button::gap`).
        gap: f32,
        /// The text's style when `has_label` is 1.
        text_style: PxuiLabelStyle,
        /// The text's box when `has_label` is 1.
        text_box: PxuiLabelBox,
        /// ABI 1.71 (read when `size` covers it): the caller's leading icon (`Button::icon`): its glyph (empty: none), or the app's drawing
        /// and its user data (null for a glyph or a drawing that did not cross the ABI), painted as `PxuiIconButtonConfig::draw` is.
        lead: Str,
        /// The app's drawing of the leading icon (null for a glyph or a drawing that did not cross the ABI).
        lead_draw: PxuiIconDraw,
        /// Passed to `lead_draw`.
        lead_user: *mut core::ffi::c_void,
        /// The caller's secondary line under the text (`Button::detail`; empty: none); valid for the call.
        detail: Str,
        /// The caller's trailing text at the box's end (`Button::accessory`; empty: none); valid for the call.
        accessory: Str,
        /// 1 when the caller gave the detail a style (`Button::detail_label`) in `detail_style` and `detail_box`, 0 for the theme's label.
        has_detail_style: u8,
        /// 1 when the caller gave the accessory a style (`Button::accessory_label`) in `accessory_style` and `accessory_box`, 0 for the theme's label.
        has_accessory_style: u8,
        /// Zero.
        reserved4: [u8; 6],
        /// The detail's style and box when `has_detail_style` is 1.
        detail_style: PxuiLabelStyle,
        /// The detail's box when `has_detail_style` is 1.
        detail_box: PxuiLabelBox,
        /// The accessory's style and box when `has_accessory_style` is 1.
        accessory_style: PxuiLabelStyle,
        /// The accessory's box when `has_accessory_style` is 1.
        accessory_box: PxuiLabelBox,
        /// ABI 1.91: the word of the leading icon's drawing (`Icon::Draw`'s, D18; 0 for a glyph), so a body draws the app's icon by id.
        lead_word: u32,
        /// ABI 1.91: the leading icon's square in points (`Button::icon_size`; 0: the control-high one).
        icon_size: f32,
        /// ABI 1.99: the kit's built-in button body for this config, run with `pxui_ui_slot` (the Rust `Ui::slot`): a registered body
        /// that does not style a config (its variant 0, say) draws the kit's button exactly as an unregistered theme would.
        built_in: PxuiSlot,
    }
    /// `states.pick(state)` is the look the caller asked for, as the caller gave it (U2b.4.30: its colours may name tokens, which `pxui_ui_style` resolves; resolve one with `pxui_ui_color` before a C paint call), or the theme's (resolved); `states` is valid for the call.
    PxuiIconButtonConfig {
        #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Handle of the widget node."] node: PxuiBox, #[doc = "Borrowed label used by the widget."] label: Str, #[doc = "Borrowed glyph text; empty when using the drawing callback."] icon: Str, #[doc = "Borrowed looks for the widget states, valid for the body call."] states: *const PxuiStates,
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
        #[doc = "User pointer passed unchanged to the icon drawing callback."] draw_user: *mut core::ffi::c_void,
        /// The look's text colour for `state`.
        ink: Rgba,
    } @variant {
        /// ABI 1.54 (read when `size` covers it): two `Curve`s, the press scale's in on press and out on release, valid for the
        /// call; null for the kit's springs.
        press: *const crate::Curve,
        /// ABI 1.91: the word of the icon's drawing (`Icon::Draw`'s, D18; 0 for a glyph), so a body draws the app's icon by id.
        draw_word: u32,
        /// ABI 1.101: the kit's built-in icon button body for this config, run with `pxui_ui_slot` (the Rust `Ui::slot`): a registered body
        /// that does not style a config draws the kit's icon button exactly as an unregistered theme would.
        built_in: PxuiSlot,
    }
    /// A checkbox or a radio option.
    PxuiCheckConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Current on/off state after input for this frame."] on: u8, #[doc = "Handle of the row the body is drawing."] row: PxuiBox, #[doc = "Borrowed label used by the widget."] label: Str } @variant {
        /// ABI 1.85 (read when `size` covers it): the box's look per state (`pick(checked)`): the caller's (`Checkbox::look`)
        /// as the caller gave it (U2b.4.30: its colours may name tokens, which `pxui_ui_style` resolves; resolve one with `pxui_ui_color` before a C paint call), or the theme's (resolved); valid for the call.
        states: *const PxuiStates,
        /// The box's side in points (`Checkbox::size`; 12 unless the caller asked for another).
        box_size: f32,
        /// 1 for a passive mark (`Checkbox::passive`): the row is a plain node that takes no input, and with empty text the body builds
        /// no label.
        passive: u8,
        /// Zero.
        reserved2: [u8; 3],
    }
    /// Body configuration for an indeterminate activity indicator.
    PxuiSpinnerConfig { #[doc = "Handle of the widget node."] node: PxuiBox, #[doc = "Borrowed label used by the widget."] label: Str }
    /// Body configuration for a badge.
    PxuiBadgeConfig { #[doc = "Handle of the widget node."] node: PxuiBox, #[doc = "Borrowed label used by the widget."] label: Str, #[doc = "Borrowed text displayed by the body."] text: Str }
    /// The record grows at its end in ABI 1.45 by `style`, the label's style as the caller gave it (unset fields: the theme's);
    /// a body reads it only when `size` covers it.
    PxuiLabelConfig { #[doc = "Whether this is the first item in the group."] first: u8, #[doc = "Handle of the widget node."] node: PxuiBox, #[doc = "Borrowed text displayed by the body."] text: Str, #[doc = "Text style supplied by the caller."] style: PxuiLabelStyle } @variant {
        /// ABI 1.54 (read when `size` covers it): the label's box as the caller gave it (height, role, whole pixels).
        label_box: PxuiLabelBox,
    }
    /// Body configuration for footer text and hints.
    PxuiFooterConfig { #[doc = "Handle of the row the body is drawing."] row: PxuiBox, #[doc = "Nonzero when trailing footer text was supplied."] has_right: u8, #[doc = "Borrowed trailing footer text."] right: Str, #[doc = "Borrowed footer hints in display order."] hints: Slice<PxuiHintDesc> }
    /// Body configuration for a resize grip.
    PxuiGripConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Whether the grip is vertical."] vertical: u8, #[doc = "Whether the grip floats above adjacent content."] floating: u8, #[doc = "Handle of the grip node."] grip: PxuiBox }
    /// A panel, window, popup or modal sheet.
    PxuiPanelConfig { #[doc = "Handle of the widget node."] node: PxuiBox, #[doc = "Borrowed label used by the widget."] label: Str, #[doc = "Slot that builds the supplied contents."] content: PxuiSlot } @variant {
        /// ABI 1.86 (read when `size` covers it): how the sheet is presented (`Overlay::present`): 0 centred (every sheet but a modal's),
        /// 1 on the content area's bottom edge, 2 filling it, 3 anchored at its top (ABI 1.90, a modal asked to be `Top`; a body draws a
        /// value it does not know as 0). A body may dress the sheet by it; its place and drag-to-dismiss stay with
        /// behaviour. Eight bytes so the field starts past the record's 1.52 size.
        present: u64,
    }
    /// The record grows at its end in ABI 1.53 by the buttons' flags (the first 64 buttons): `default_at` and `cancel_at` are the index
    /// of the button Return and Escape choose (-1 for none), bit `i` of `danger` marks button `i` as a destructive choice (its label in the
    /// theme's invalid colour) and bit `i` of `disabled` as one that takes no press; a body reads them only when `size` covers them.
    PxuiDialogConfig { #[doc = "Title of the widget or handle of its title node, according to the field type."] title: Str, #[doc = "Borrowed dialog button labels in declaration order."] buttons: Slice<Str>, #[doc = "Slot that builds the supplied contents."] content: PxuiSlot, #[doc = "Index selected by Return, or -1 for none."] default_at: i64, #[doc = "Index selected by Escape, or -1 for none."] cancel_at: i64, #[doc = "Mask of destructive buttons, one bit per index."] danger: u64, #[doc = "Mask of disabled buttons, one bit per index."] disabled: u64 } @variant {
        /// ABI 1.71 (read when `size` covers it): each button's detail line (`DialogButton::detail`; empty: none), as many as `buttons`;
        /// valid for the call.
        details: Slice<Str>,
        /// ABI 1.131 (U2b.4.38): the kit's buttons for this dialog, run with `pxui_ui_slot` (the Rust `Ui::slot`) where the body places
        /// them: the row of `Part::button(i)` buttons the built-in dialog builds, each through the button body with the dialog's
        /// `variant`, so their presses, focus and looks for each state are the kit's and the theme's.
        built_in: PxuiSlot,
    }
    /// Body configuration for a collapsible section.
    PxuiSectionConfig { #[doc = "Current expansion state."] open: u8, #[doc = "Spacing between the title and contents in logical points."] gap: f32, #[doc = "Title of the widget or handle of its title node, according to the field type."] title: PxuiBox, #[doc = "Borrowed label used by the widget."] label: Str, #[doc = "Slot that builds the supplied contents."] content: PxuiSlot }
    /// Body configuration for an inspector row.
    PxuiInspectorRowConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Nonzero when the row has a pin control."] has_pin: u8, #[doc = "Current pin state."] pin: u8, #[doc = "Width of the label column in logical points."] label_w: f32, #[doc = "Handle of the row the body is drawing."] row: PxuiBox, #[doc = "Borrowed label used by the widget."] label: Str }
    /// `axis` is 0 row, 1 column, 2 stack.
    PxuiSplitConfig { #[doc = "Axis along which the panes are laid out."] axis: u32, #[doc = "Fraction of the split assigned to the first pane."] ratio: f32, #[doc = "Minimum extent of the first pane in logical points."] min_a: f32, #[doc = "Minimum extent of the second pane in logical points."] min_b: f32, #[doc = "Handle of the split container."] split: PxuiBox, #[doc = "Slot that builds the first pane."] first: PxuiSlot, #[doc = "Slot that builds the second pane."] second: PxuiSlot }
    /// Body configuration for a scroll view.
    PxuiScrollConfig { #[doc = "Handle of the widget node."] node: PxuiBox, #[doc = "Slot that builds the supplied contents."] content: PxuiSlot }
    /// Body configuration for a toast.
    PxuiToastsConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Handle of the widget node."] node: PxuiBox, #[doc = "Borrowed text displayed by the body."] text: Str }
    /// Body configuration for wrapped tooltip text.
    PxuiTooltipConfig { #[doc = "Width in logical points."] width: f32, #[doc = "Height in logical points."] height: f32, #[doc = "Handle of the tooltip container."] tip: PxuiBox, #[doc = "Borrowed tooltip lines in display order."] lines: Slice<Str> }
    /// Body configuration for a tab in the tab bar.
    PxuiTabsConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Index of this item in declaration order."] index: usize, #[doc = "Handle of the widget node."] node: PxuiBox, #[doc = "Borrowed text displayed by the body."] text: Str } @variant {
        /// ABI 1.84 (read when `size` covers it): the tab's detail after its text (`Segment::detail`: a count or badge; empty: none);
        /// valid for the call.
        detail: Str,
        /// ABI 1.124 (U2b.4.29; read when `size` covers it): the bar, the tab list holding the tabs (the theme's row high, `space(2)`
        /// padding and gap, the header look): a body may restyle or resize it (`pxui_ui_set_node_layout`, `pxui_ui_style`), as a
        /// segment body its `strip`.
        bar: PxuiBox,
    }
    /// A section index (ABI 1.85): the strip of letters of `Ui::section_index`, entered as `node`; `letters` are the letters (valid for the
    /// call), `active` the letter the pointer holds or the last one chosen when `has_active` is 1.
    PxuiSectionIndexConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Nonzero when `active` names a letter."] has_active: u8, #[doc = "Index of the active letter."] active: usize, #[doc = "Handle of the widget node."] node: PxuiBox, #[doc = "Borrowed section letters in display order."] letters: Slice<Str> }
    /// `selected` is the chosen option's index when `has_selected` is 1 (ABI 1.67, read when `size` covers it; 0 with `selected` 0 for no
    /// choice, final); `state.focused` the segment holding the keyboard focus (from ABI 1.67 the strip's one Tab stop is the chosen
    /// segment, else the first).
    PxuiSegmentedConfig {
        #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Index of this item in declaration order."] index: usize, #[doc = "Current selection or selection state of this item."] selected: usize, #[doc = "Key of the group containing this segment."] group: i64, #[doc = "Handle of the widget node."] node: PxuiBox, #[doc = "Borrowed text displayed by the body."] text: Str,
        /// The look per state of a segment: the caller's as the caller gave it (U2b.4.30: its colours may name tokens, which `pxui_ui_style` resolves; resolve one with `pxui_ui_color` before a C paint call), or the theme's (resolved); valid for the call.
        states: *const PxuiStates,
        /// 0 or 1: the segments share the strip's width instead of each being as wide as its text.
        grow: u8,
    } @variant {
        /// ABI 1.55 (read when `size` covers it): the text's inset and half the room around the text in points (8 the kit's).
        padding: f32,
        /// The least width of a segment in points (0: none).
        min_segment: f32,
        /// ABI 1.58 (read when `size` covers it): the line the strip sits in (the row's height, `space(2)` padding at its ends) and the
        /// strip (a group holding the segments, no look of its own): a body may restyle or resize both, so it owns the container.
        line: PxuiBox,
        #[doc = "Handle of the segment strip."] strip: PxuiBox,
        /// ABI 1.67 (read when `size` covers it): 1 when `selected` is the choice, 0 when no option is chosen (`selected` then 0).
        has_selected: u8,
        /// Zero.
        reserved3: [u8; 7],
        /// ABI 1.69 (read when `size` covers it): the choice the caller passed this frame, before a press or key moved `selected`, as
        /// `PxuiToggleConfig::stored` is the toggle's: `stored` is that option's index when `has_stored` is 1, 0 when the caller passed no choice
        /// (`stored` then 0). Clamped to the options, as `selected`.
        stored: usize,
        #[doc = "Nonzero when the caller supplied a previously selected option."] has_stored: u8,
        /// Zero.
        reserved4: [u8; 7],
    }
    /// Body configuration for a dropdown trigger.
    PxuiDropdownConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Current expansion state."] open: u8, #[doc = "Widget identity key."] key: i64, #[doc = "Handle of the dropdown trigger."] button: PxuiBox, #[doc = "Borrowed text displayed by the body."] text: Str }
    /// A menu bar title or a row of an open menu.
    PxuiMenuItemConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Index of this item in declaration order."] index: usize, #[doc = "Handle of the widget node."] node: PxuiBox, #[doc = "Borrowed text displayed by the body."] text: Str }
    /// A context menu row: `selected` is the accent mark of `MenuItem::checked`; `state.checked` (from ABI 1.55) the row being the current
    /// choice of a settings menu (`MenuItem::selected`, the theme's `checked` menu look).
    PxuiMenuRowConfig {
        #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Current selection or selection state of this item."] selected: u8, #[doc = "Nonzero when this menu item opens a submenu."] chevron: u8, #[doc = "Whether to draw a destructive action."] danger: u8, #[doc = "Nonzero when the row carries a colour swatch."] has_swatch: u8, #[doc = "Colour swatch requested by the caller."] swatch: Rgba, #[doc = "Nonzero when shortcut text is present."] has_shortcut: u8, #[doc = "Index of this item in declaration order."] index: i64, #[doc = "Extent of the leading contents in logical points."] lead: f32, #[doc = "Space around the swatch in logical points."] swatch_pad: f32,
        #[doc = "Handle of the row the body is drawing."] row: PxuiBox, #[doc = "Borrowed text displayed by the body."] text: Str, #[doc = "Borrowed text describing the keyboard shortcut."] shortcut: Str, #[doc = "Requested row height in logical points."] row_h: f32
    }
    /// A `selection_list` or `table_row` row; `content` is the text or the caller's cells. `drop` (ABI 1.88): a carried payload, 0 none,
    /// 1 would land before the row, 2 after it (a reorderable list: the body draws the insertion line), 3 over the row (a list with a
    /// drop look, which behaviour has applied to the row).
    PxuiRowConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Index of this item in declaration order."] index: usize, #[doc = "Handle of the row the body is drawing."] row: PxuiBox, #[doc = "Slot that builds the supplied contents."] content: PxuiSlot, #[doc = "Position where releasing the carried payload would drop it."] drop: u8 }
    /// Body configuration for a row in a tree.
    PxuiTreeRowConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Nonzero when this tree row has children."] has: u8, #[doc = "Current expansion state."] open: u8, #[doc = "Index of this item in declaration order."] index: usize, #[doc = "Index of this node in the input tree."] node: usize, #[doc = "Tree depth of this row."] depth: usize, #[doc = "Handle of the row the body is drawing."] row: PxuiBox, #[doc = "Slot that builds the supplied contents."] content: PxuiSlot }
    /// Body configuration for a table column heading.
    PxuiTableHeadConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Nonzero when this column controls sorting."] has_ascending: u8, #[doc = "Whether the active sort is ascending."] ascending: u8, #[doc = "Index of this item in declaration order."] index: usize, #[doc = "Handle of the widget node."] node: PxuiBox, #[doc = "Title of the widget or handle of its title node, according to the field type."] title: Str } @variant {
        /// ABI 1.129: the kit's built-in table heading body for this config, run with `pxui_ui_slot` (the Rust `Ui::slot`): a registered
        /// body that does not style a heading (another table's) draws the kit's heading exactly as an unregistered theme would.
        built_in: PxuiSlot,
    }
    /// Body configuration for a result in the picker.
    PxuiPickerRowConfig {
        #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Whether this is the current picker result."] current: u8, #[doc = "Whether the result is marked for removal."] doomed: u8, #[doc = "Whether the result is unavailable."] unavailable: u8, #[doc = "Whether this picker result is marked."] marked: u8, #[doc = "Nonzero when the row carries a colour swatch."] has_swatch: u8, #[doc = "Colour swatch requested by the caller."] swatch: Rgba, #[doc = "Index of this item in declaration order."] index: i64,
        #[doc = "Handle of the row the body is drawing."] row: PxuiBox, #[doc = "Borrowed label used by the widget."] label: Str, #[doc = "Borrowed secondary text."] detail: Str, #[doc = "Borrowed search query."] query: Str
    } @variant {
        /// ABI 1.86 (extension set; read when `size` covers it): the row's artwork or icon slot, `PXUI_PICK_ART_NONE` 0, `_GLYPH` 1 (`art`
        /// is the icon font character's code point, which the built-in body draws), `_IMAGE` 2 (`art` is an `Images` id, drawn by a
        /// registered body) or `_DRAW` 3 (ABI 1.117: `art` is the app's icon word, drawn by a registered body).
        art_kind: u8,
        /// Zero.
        reserved3: [u8; 7],
        #[doc = "Artwork identifier interpreted according to `art_kind`."] art: u64,
        /// ABI 1.110 (U2b.4.15): 1 when the picker gave the row an image (`image`, which the built-in body draws in the art slot).
        has_image: u8,
        /// ABI 1.110 (U2b.4.15): 1 when the row starts a group: `head` is its heading, a box above the row that behaviour made (its
        /// text, the heading role and name), which the body lays out (`pxui_ui_set_node_layout`) and styles as a button body its `line`.
        has_head: u8,
        /// Zero.
        reserved4: [u8; 6],
        /// The row's image (`has_image`): the descriptor the picker's image callback gave.
        image: crate::Image,
        /// The group heading's box (`has_head`).
        head: PxuiBox,
    }
    /// `editor` is a slot that builds the editing surface and returns whether the query is not empty (`pxui_ui_slot`'s `out`); a
    /// `Part::EDITOR` the body built before it is that surface, edited in its look's font (U2b.4.4). Behaviour reads `Part::CLEAR`.
    PxuiSearchFieldConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Widget identity key."] key: i64, #[doc = "Handle of the search field well."] well: PxuiBox, #[doc = "Icon requested by the caller."] icon: Str, #[doc = "Slot that builds the text editor."] editor: PxuiSlot } @variant {
        /// ABI 1.102: the field's line (the kit's row: the control height, `space(2)` at each side, `well` inside it), which a body
        /// sizes with `pxui_ui_set_node_layout` as a button body sizes `PxuiButtonConfig.line`.
        line: PxuiBox,
        /// ABI 1.122 (U2b.4.23): the clear key's state (the node the body builds as `Part::CLEAR`: hovered, pressed), which
        /// behaviour owns with the key's press, role and name; the body styles the key for it (`pxui_ui_style`).
        clear: PxuiStateDesc,
    }
    /// Body configuration for a single-line text field.
    PxuiTextFieldConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Handle of the row the body is drawing."] row: PxuiBox, #[doc = "Borrowed label used by the widget."] label: Str, #[doc = "Value supplied to the widget."] value: Str }
    /// Body configuration for a line field.
    PxuiLineFieldConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Whether the field has no text."] empty: u8, #[doc = "Handle of the field container."] field: PxuiBox, #[doc = "Borrowed text displayed by the body."] text: Str }
    /// `opts` holds the caller's options (its strings borrowed for the call); its place, label and value are unset.
    PxuiValueFieldConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Whether the current value is invalid."] invalid: u8, #[doc = "Requested width in logical points."] w: f32, #[doc = "Requested height in logical points."] h: f32, #[doc = "Handle of the field container."] field: PxuiBox, #[doc = "Field options supplied by the caller."] opts: PxuiValueFieldDesc, #[doc = "Borrowed text displayed by the body."] text: Str }
    /// Body configuration for the search and results of a picker.
    PxuiPickerFieldConfig { #[doc = "Current interaction state of the widget."] state: PxuiStateDesc, #[doc = "Whether the field has no text."] empty: u8, #[doc = "Whether the picker is showing slash commands."] slash: u8, #[doc = "Width reserved for the query prefix in logical points."] prefix: f32, #[doc = "Inner spacing in logical points."] inset: f32, #[doc = "Handle of the search container."] search: PxuiBox, #[doc = "Borrowed text shown for an empty query."] placeholder: Str } @variant {
        /// ABI 1.111 (U2b.4.16): the results region, the scrolling list built before the body runs (`Fit` high, at most the picker's
        /// limit of kit rows), which a body lays out with `pxui_ui_set_node_layout` (maximum and minimum height, padding inside the
        /// scroll) as a search field body sizes `PxuiSearchFieldConfig.line`.
        list: PxuiBox,
    }
}


impl PxuiRowConfig {
    /// `drop`: no payload is carried over the row.
    pub const DROP_NONE: u8 = 0;
    /// `drop`: a carried payload would land before the row (a reorderable list's body draws the insertion line at its top).
    pub const DROP_BEFORE: u8 = 1;
    /// `drop`: a carried payload would land after the row (the insertion line at its bottom).
    pub const DROP_AFTER: u8 = 2;
    /// `drop`: a carried payload is over the row of a list with a drop look, which behaviour has applied to the row.
    pub const DROP_OVER: u8 = 3;
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
        // `Curve` is `repr(C, u8)`: the kind byte (0 Bezier, 1 Spring, 2 Table, 3 Steps), then the variant's fields from 4
        check("PxuiCurve", size_of::<crate::Curve>(), align_of::<crate::Curve>(), &[("kind", 0), ("data", 4)]);
        check("PxuiLabelBox", size_of::<PxuiLabelBox>(), align_of::<PxuiLabelBox>(), &[("size", core::mem::offset_of!(PxuiLabelBox, size)), ("set", core::mem::offset_of!(PxuiLabelBox, set)), ("height", core::mem::offset_of!(PxuiLabelBox, height)), ("role", core::mem::offset_of!(PxuiLabelBox, role)), ("whole_pixels", core::mem::offset_of!(PxuiLabelBox, whole_pixels)), ("variant", core::mem::offset_of!(PxuiLabelBox, variant))]);
        check("PxuiMenuDesc", size_of::<PxuiMenuDesc>(), align_of::<PxuiMenuDesc>(), &[("size", core::mem::offset_of!(PxuiMenuDesc, size)), ("reserved", core::mem::offset_of!(PxuiMenuDesc, reserved)), ("title", core::mem::offset_of!(PxuiMenuDesc, title)), ("items", core::mem::offset_of!(PxuiMenuDesc, items))]);
        check("PxuiGridListDesc", size_of::<PxuiGridListDesc>(), align_of::<PxuiGridListDesc>(), &[("size", core::mem::offset_of!(PxuiGridListDesc, size)), ("reserved", core::mem::offset_of!(PxuiGridListDesc, reserved)), ("label", core::mem::offset_of!(PxuiGridListDesc, label)), ("count", core::mem::offset_of!(PxuiGridListDesc, count)), ("cell_w", core::mem::offset_of!(PxuiGridListDesc, cell_w)), ("cell_h", core::mem::offset_of!(PxuiGridListDesc, cell_h)), ("build", core::mem::offset_of!(PxuiGridListDesc, build)), ("user", core::mem::offset_of!(PxuiGridListDesc, user))]);
        check("PxuiSplitDesc", size_of::<PxuiSplitDesc>(), align_of::<PxuiSplitDesc>(), &[("size", core::mem::offset_of!(PxuiSplitDesc, size)), ("axis", core::mem::offset_of!(PxuiSplitDesc, axis)), ("label", core::mem::offset_of!(PxuiSplitDesc, label)), ("ratio", core::mem::offset_of!(PxuiSplitDesc, ratio)), ("min_a", core::mem::offset_of!(PxuiSplitDesc, min_a)), ("min_b", core::mem::offset_of!(PxuiSplitDesc, min_b)), ("reserved", core::mem::offset_of!(PxuiSplitDesc, reserved)), ("build_a", core::mem::offset_of!(PxuiSplitDesc, build_a)), ("user_a", core::mem::offset_of!(PxuiSplitDesc, user_a)), ("build_b", core::mem::offset_of!(PxuiSplitDesc, build_b)), ("user_b", core::mem::offset_of!(PxuiSplitDesc, user_b))]);
        check("PxuiMenuBarDesc", size_of::<PxuiMenuBarDesc>(), align_of::<PxuiMenuBarDesc>(), &[("size", core::mem::offset_of!(PxuiMenuBarDesc, size)), ("variant", core::mem::offset_of!(PxuiMenuBarDesc, variant)), ("reserved", core::mem::offset_of!(PxuiMenuBarDesc, reserved)), ("label", core::mem::offset_of!(PxuiMenuBarDesc, label)), ("menus", core::mem::offset_of!(PxuiMenuBarDesc, menus))]);
        check("PxuiMenuOut", size_of::<PxuiMenuOut>(), align_of::<PxuiMenuOut>(), &[("size", core::mem::offset_of!(PxuiMenuOut, size)), ("reserved", core::mem::offset_of!(PxuiMenuOut, reserved)), ("menu", core::mem::offset_of!(PxuiMenuOut, menu)), ("item", core::mem::offset_of!(PxuiMenuOut, item))]);
        check("PxuiHintDesc", size_of::<PxuiHintDesc>(), align_of::<PxuiHintDesc>(), &[("size", core::mem::offset_of!(PxuiHintDesc, size)), ("reserved", core::mem::offset_of!(PxuiHintDesc, reserved)), ("key", core::mem::offset_of!(PxuiHintDesc, key)), ("value", core::mem::offset_of!(PxuiHintDesc, value))]);
        check("PxuiFooterDesc", size_of::<PxuiFooterDesc>(), align_of::<PxuiFooterDesc>(), &[("size", core::mem::offset_of!(PxuiFooterDesc, size)), ("has_right", core::mem::offset_of!(PxuiFooterDesc, has_right)), ("reserved", core::mem::offset_of!(PxuiFooterDesc, reserved)), ("right", core::mem::offset_of!(PxuiFooterDesc, right)), ("hints", core::mem::offset_of!(PxuiFooterDesc, hints))]);
        check("PxuiKeyframeDesc", size_of::<PxuiKeyframeDesc>(), align_of::<PxuiKeyframeDesc>(), &[("size", core::mem::offset_of!(PxuiKeyframeDesc, size)), ("time", core::mem::offset_of!(PxuiKeyframeDesc, time)), ("target", core::mem::offset_of!(PxuiKeyframeDesc, target)), ("reserved", core::mem::offset_of!(PxuiKeyframeDesc, reserved)), ("values", core::mem::offset_of!(PxuiKeyframeDesc, values)), ("look", core::mem::offset_of!(PxuiKeyframeDesc, look)), ("curve", core::mem::offset_of!(PxuiKeyframeDesc, curve))]);
        check("PxuiInspectorRowDesc", size_of::<PxuiInspectorRowDesc>(), align_of::<PxuiInspectorRowDesc>(), &[("size", core::mem::offset_of!(PxuiInspectorRowDesc, size)), ("has_width", core::mem::offset_of!(PxuiInspectorRowDesc, has_width)), ("has_pin", core::mem::offset_of!(PxuiInspectorRowDesc, has_pin)), ("pin", core::mem::offset_of!(PxuiInspectorRowDesc, pin)), ("reserved", core::mem::offset_of!(PxuiInspectorRowDesc, reserved)), ("key", core::mem::offset_of!(PxuiInspectorRowDesc, key)), ("label", core::mem::offset_of!(PxuiInspectorRowDesc, label)), ("width", core::mem::offset_of!(PxuiInspectorRowDesc, width)), ("reserved_tail", core::mem::offset_of!(PxuiInspectorRowDesc, reserved_tail)), ("row", core::mem::offset_of!(PxuiInspectorRowDesc, row)), ("control_x", core::mem::offset_of!(PxuiInspectorRowDesc, control_x)), ("control_y", core::mem::offset_of!(PxuiInspectorRowDesc, control_y)), ("control_w", core::mem::offset_of!(PxuiInspectorRowDesc, control_w))]);
        check("PxuiInspectorSectionDesc", size_of::<PxuiInspectorSectionDesc>(), align_of::<PxuiInspectorSectionDesc>(), &[("size", core::mem::offset_of!(PxuiInspectorSectionDesc, size)), ("expanded", core::mem::offset_of!(PxuiInspectorSectionDesc, expanded)), ("has_set", core::mem::offset_of!(PxuiInspectorSectionDesc, has_set)), ("set_expanded", core::mem::offset_of!(PxuiInspectorSectionDesc, set_expanded)), ("reserved", core::mem::offset_of!(PxuiInspectorSectionDesc, reserved)), ("key", core::mem::offset_of!(PxuiInspectorSectionDesc, key)), ("label", core::mem::offset_of!(PxuiInspectorSectionDesc, label)), ("build", core::mem::offset_of!(PxuiInspectorSectionDesc, build)), ("user", core::mem::offset_of!(PxuiInspectorSectionDesc, user)), ("out_open", core::mem::offset_of!(PxuiInspectorSectionDesc, out_open))]);
        check("PxuiInspectorValueDesc", size_of::<PxuiInspectorValueDesc>(), align_of::<PxuiInspectorValueDesc>(), &[("size", core::mem::offset_of!(PxuiInspectorValueDesc, size)), ("has_width", core::mem::offset_of!(PxuiInspectorValueDesc, has_width)), ("reserved", core::mem::offset_of!(PxuiInspectorValueDesc, reserved)), ("key", core::mem::offset_of!(PxuiInspectorValueDesc, key)), ("label", core::mem::offset_of!(PxuiInspectorValueDesc, label)), ("value", core::mem::offset_of!(PxuiInspectorValueDesc, value)), ("width", core::mem::offset_of!(PxuiInspectorValueDesc, width))]);
        check("PxuiInspectorTextDesc", size_of::<PxuiInspectorTextDesc>(), align_of::<PxuiInspectorTextDesc>(), &[("size", core::mem::offset_of!(PxuiInspectorTextDesc, size)), ("reserved", core::mem::offset_of!(PxuiInspectorTextDesc, reserved)), ("key", core::mem::offset_of!(PxuiInspectorTextDesc, key)), ("text", core::mem::offset_of!(PxuiInspectorTextDesc, text))]);
        check("PxuiU8MutSpan", size_of::<PxuiU8MutSpan>(), align_of::<PxuiU8MutSpan>(), &[("ptr", core::mem::offset_of!(PxuiU8MutSpan, ptr)), ("len", core::mem::offset_of!(PxuiU8MutSpan, len))]);
        check("PxuiF32MutSpan", size_of::<PxuiF32MutSpan>(), align_of::<PxuiF32MutSpan>(), &[("ptr", core::mem::offset_of!(PxuiF32MutSpan, ptr)), ("len", core::mem::offset_of!(PxuiF32MutSpan, len))]);
        check("PxuiU32MutSpan", size_of::<PxuiU32MutSpan>(), align_of::<PxuiU32MutSpan>(), &[("ptr", core::mem::offset_of!(PxuiU32MutSpan, ptr)), ("len", core::mem::offset_of!(PxuiU32MutSpan, len))]);
        check("PxuiTreeNode", size_of::<PxuiTreeNode>(), align_of::<PxuiTreeNode>(), &[("depth", core::mem::offset_of!(PxuiTreeNode, depth)), ("children", core::mem::offset_of!(PxuiTreeNode, children))]);
        check("PxuiSlot", size_of::<PxuiSlot>(), align_of::<PxuiSlot>(), &[("serial", core::mem::offset_of!(PxuiSlot, serial))]);
        check("PxuiTreeViewDesc", size_of::<PxuiTreeViewDesc>(), align_of::<PxuiTreeViewDesc>(), &[("size", core::mem::offset_of!(PxuiTreeViewDesc, size)), ("has_revision", core::mem::offset_of!(PxuiTreeViewDesc, has_revision)), ("reserved", core::mem::offset_of!(PxuiTreeViewDesc, reserved)), ("label", core::mem::offset_of!(PxuiTreeViewDesc, label)), ("nodes", core::mem::offset_of!(PxuiTreeViewDesc, nodes)), ("open", core::mem::offset_of!(PxuiTreeViewDesc, open)), ("selected", core::mem::offset_of!(PxuiTreeViewDesc, selected)), ("visible", core::mem::offset_of!(PxuiTreeViewDesc, visible)), ("visible_len", core::mem::offset_of!(PxuiTreeViewDesc, visible_len)), ("build", core::mem::offset_of!(PxuiTreeViewDesc, build)), ("user", core::mem::offset_of!(PxuiTreeViewDesc, user)), ("revision", core::mem::offset_of!(PxuiTreeViewDesc, revision))]);
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
        check("PxuiMenuRow", size_of::<PxuiMenuRow>(), align_of::<PxuiMenuRow>(), &[("size", core::mem::offset_of!(PxuiMenuRow, size)), ("enabled", core::mem::offset_of!(PxuiMenuRow, enabled)), ("danger", core::mem::offset_of!(PxuiMenuRow, danger)), ("checked", core::mem::offset_of!(PxuiMenuRow, checked)), ("selected", core::mem::offset_of!(PxuiMenuRow, selected)), ("has_swatch", core::mem::offset_of!(PxuiMenuRow, has_swatch)), ("has_id", core::mem::offset_of!(PxuiMenuRow, has_id)), ("reserved", core::mem::offset_of!(PxuiMenuRow, reserved)), ("swatch", core::mem::offset_of!(PxuiMenuRow, swatch)), ("id", core::mem::offset_of!(PxuiMenuRow, id)), ("text", core::mem::offset_of!(PxuiMenuRow, text)), ("shortcut", core::mem::offset_of!(PxuiMenuRow, shortcut)), ("submenu", core::mem::offset_of!(PxuiMenuRow, submenu))]);
        check("PxuiMenuOptions", size_of::<PxuiMenuOptions>(), align_of::<PxuiMenuOptions>(), &[("size", core::mem::offset_of!(PxuiMenuOptions, size)), ("variant", core::mem::offset_of!(PxuiMenuOptions, variant)), ("reserved", core::mem::offset_of!(PxuiMenuOptions, reserved)), ("row_h", core::mem::offset_of!(PxuiMenuOptions, row_h)), ("max_rows", core::mem::offset_of!(PxuiMenuOptions, max_rows)), ("padding", core::mem::offset_of!(PxuiMenuOptions, padding)), ("has_name", core::mem::offset_of!(PxuiMenuOptions, has_name)), ("focus_on_click", core::mem::offset_of!(PxuiMenuOptions, focus_on_click)), ("reserved2", core::mem::offset_of!(PxuiMenuOptions, reserved2)), ("name", core::mem::offset_of!(PxuiMenuOptions, name)), ("max_h", core::mem::offset_of!(PxuiMenuOptions, max_h)), ("reserved3", core::mem::offset_of!(PxuiMenuOptions, reserved3))]);
        check("PxuiDiagnostics", size_of::<PxuiDiagnostics>(), align_of::<PxuiDiagnostics>(), &[("size", core::mem::offset_of!(PxuiDiagnostics, size)), ("timing", core::mem::offset_of!(PxuiDiagnostics, timing)), ("has_text_layouts", core::mem::offset_of!(PxuiDiagnostics, has_text_layouts)), ("has_cpu", core::mem::offset_of!(PxuiDiagnostics, has_cpu)), ("reserved", core::mem::offset_of!(PxuiDiagnostics, reserved)), ("rebuilt_prefix", core::mem::offset_of!(PxuiDiagnostics, rebuilt_prefix)), ("calls", core::mem::offset_of!(PxuiDiagnostics, calls)), ("text_layouts", core::mem::offset_of!(PxuiDiagnostics, text_layouts)), ("rebuilt", core::mem::offset_of!(PxuiDiagnostics, rebuilt)), ("phase_nanos", core::mem::offset_of!(PxuiDiagnostics, phase_nanos)), ("phase_cpu_nanos", core::mem::offset_of!(PxuiDiagnostics, phase_cpu_nanos))]);
        check("Slice", size_of::<Slice<u32>>(), align_of::<Slice<u32>>(), &[("ptr", core::mem::offset_of!(Slice<u32>, ptr)), ("len", core::mem::offset_of!(Slice<u32>, len))]);
        check("Strided", size_of::<Strided<u32>>(), align_of::<Strided<u32>>(), &[("ptr", core::mem::offset_of!(Strided<u32>, ptr)), ("len", core::mem::offset_of!(Strided<u32>, len)), ("stride", core::mem::offset_of!(Strided<u32>, stride))]);
        check("Input", size_of::<Input>(), align_of::<Input>(), &[("size", core::mem::offset_of!(Input, size)), ("time", core::mem::offset_of!(Input, time)), ("dt", core::mem::offset_of!(Input, dt)), ("frame", core::mem::offset_of!(Input, frame)), ("tick", core::mem::offset_of!(Input, tick)), ("logical_size", core::mem::offset_of!(Input, logical_size)), ("pointer", core::mem::offset_of!(Input, pointer)), ("keys", core::mem::offset_of!(Input, keys)), ("buttons", core::mem::offset_of!(Input, buttons)), ("events", core::mem::offset_of!(Input, events)), ("pixel_scale", core::mem::offset_of!(Input, pixel_scale)), ("mouse_delta", core::mem::offset_of!(Input, mouse_delta)), ("fps", core::mem::offset_of!(Input, fps))]);
        check("PxuiInputSnapshot", size_of::<PxuiInputSnapshot>(), align_of::<PxuiInputSnapshot>(), &[
            ("size", core::mem::offset_of!(PxuiInputSnapshot, size)),
            ("keys", core::mem::offset_of!(PxuiInputSnapshot, keys)),
            ("buttons", core::mem::offset_of!(PxuiInputSnapshot, buttons)),
            ("pointer", core::mem::offset_of!(PxuiInputSnapshot, pointer)),
            ("mouse_delta", core::mem::offset_of!(PxuiInputSnapshot, mouse_delta)),
            ("pixel_scale", core::mem::offset_of!(PxuiInputSnapshot, pixel_scale)),
            ("dt", core::mem::offset_of!(PxuiInputSnapshot, dt)),
            ("frame", core::mem::offset_of!(PxuiInputSnapshot, frame)),
            ("tick", core::mem::offset_of!(PxuiInputSnapshot, tick)),
            ("fps", core::mem::offset_of!(PxuiInputSnapshot, fps)),
        ]);
        check("PlatformOut", size_of::<PlatformOut>(), align_of::<PlatformOut>(), &[("size", core::mem::offset_of!(PlatformOut, size)), ("cursor", core::mem::offset_of!(PlatformOut, cursor)), ("has_cursor", core::mem::offset_of!(PlatformOut, has_cursor)), ("has_ime", core::mem::offset_of!(PlatformOut, has_ime)), ("has_background", core::mem::offset_of!(PlatformOut, has_background)), ("has_clipboard", core::mem::offset_of!(PlatformOut, has_clipboard)), ("ime_rect", core::mem::offset_of!(PlatformOut, ime_rect)), ("ime_caret", core::mem::offset_of!(PlatformOut, ime_caret)), ("clipboard", core::mem::offset_of!(PlatformOut, clipboard)), ("background", core::mem::offset_of!(PlatformOut, background)), ("paste", core::mem::offset_of!(PlatformOut, paste)), ("reserved", core::mem::offset_of!(PlatformOut, reserved))]);
        check("PxuiPromotedKey", size_of::<PxuiPromotedKey>(), align_of::<PxuiPromotedKey>(), &[
            ("time", core::mem::offset_of!(PxuiPromotedKey, time)),
            ("rect", core::mem::offset_of!(PxuiPromotedKey, rect)),
            ("opacity", core::mem::offset_of!(PxuiPromotedKey, opacity)),
            ("curve", core::mem::offset_of!(PxuiPromotedKey, curve)),
        ]);
        check("PxuiPromoted", size_of::<PxuiPromoted>(), align_of::<PxuiPromoted>(), &[
            ("layer", core::mem::offset_of!(PxuiPromoted, layer)),
            ("instance", core::mem::offset_of!(PxuiPromoted, instance)),
            ("view", core::mem::offset_of!(PxuiPromoted, view)),
            ("count", core::mem::offset_of!(PxuiPromoted, count)),
            ("autoreverse", core::mem::offset_of!(PxuiPromoted, autoreverse)),
            ("hz", core::mem::offset_of!(PxuiPromoted, hz)),
            ("reserved", core::mem::offset_of!(PxuiPromoted, reserved)),
            ("content_revision", core::mem::offset_of!(PxuiPromoted, content_revision)),
            ("start", core::mem::offset_of!(PxuiPromoted, start)),
            ("offset", core::mem::offset_of!(PxuiPromoted, offset)),
            ("color", core::mem::offset_of!(PxuiPromoted, color)),
            ("swept", core::mem::offset_of!(PxuiPromoted, swept)),
            ("clip", core::mem::offset_of!(PxuiPromoted, clip)),
            ("keys", core::mem::offset_of!(PxuiPromoted, keys)),
        ]);
        check("PxuiDamageList", size_of::<PxuiDamageList>(), align_of::<PxuiDamageList>(), &[("count", core::mem::offset_of!(PxuiDamageList, count)), ("full", core::mem::offset_of!(PxuiDamageList, full)), ("reserved", core::mem::offset_of!(PxuiDamageList, reserved)), ("rects", core::mem::offset_of!(PxuiDamageList, rects))]);
        check("Damage", size_of::<Damage>(), align_of::<Damage>(), &[("has_damage", core::mem::offset_of!(Damage, has_damage)), ("reserved", core::mem::offset_of!(Damage, reserved)), ("rect", core::mem::offset_of!(Damage, rect))]);
        check("A11yValue", size_of::<A11yValue>(), align_of::<A11yValue>(), &[("tag", core::mem::offset_of!(A11yValue, tag)), ("text_start", core::mem::offset_of!(A11yValue, text_start)), ("text_end", core::mem::offset_of!(A11yValue, text_end)), ("reserved", core::mem::offset_of!(A11yValue, reserved)), ("now", core::mem::offset_of!(A11yValue, now)), ("min", core::mem::offset_of!(A11yValue, min)), ("max", core::mem::offset_of!(A11yValue, max)), ("step", core::mem::offset_of!(A11yValue, step))]);
        check("A11yNode", size_of::<A11yNode>(), align_of::<A11yNode>(), &[("size", core::mem::offset_of!(A11yNode, size)), ("parent", core::mem::offset_of!(A11yNode, parent)), ("id", core::mem::offset_of!(A11yNode, id)), ("role", core::mem::offset_of!(A11yNode, role)), ("name_start", core::mem::offset_of!(A11yNode, name_start)), ("name_end", core::mem::offset_of!(A11yNode, name_end)), ("description_start", core::mem::offset_of!(A11yNode, description_start)), ("description_end", core::mem::offset_of!(A11yNode, description_end)), ("value", core::mem::offset_of!(A11yNode, value)), ("rect", core::mem::offset_of!(A11yNode, rect)), ("states", core::mem::offset_of!(A11yNode, states)), ("actions", core::mem::offset_of!(A11yNode, actions)), ("selection", core::mem::offset_of!(A11yNode, selection)), ("child_count", core::mem::offset_of!(A11yNode, child_count)), ("first_child", core::mem::offset_of!(A11yNode, first_child)), ("next_sibling", core::mem::offset_of!(A11yNode, next_sibling)), ("last_child", core::mem::offset_of!(A11yNode, last_child))]);
        check("A11yAction", size_of::<A11yAction>(), align_of::<A11yAction>(), &[("size", core::mem::offset_of!(A11yAction, size)), ("action_code", core::mem::offset_of!(A11yAction, action_code)), ("id", core::mem::offset_of!(A11yAction, id)), ("payload", core::mem::offset_of!(A11yAction, payload)), ("end", core::mem::offset_of!(A11yAction, end)), ("reserved", core::mem::offset_of!(A11yAction, reserved)), ("surface", core::mem::offset_of!(A11yAction, surface))]);
        check("A11yTree", size_of::<A11yTree>(), align_of::<A11yTree>(), &[("size", core::mem::offset_of!(A11yTree, size)), ("reserved", core::mem::offset_of!(A11yTree, reserved)), ("nodes", core::mem::offset_of!(A11yTree, nodes)), ("nodes.len", core::mem::offset_of!(A11yTree, nodes.len)), ("nodes.stride", core::mem::offset_of!(A11yTree, nodes.stride)), ("strings", core::mem::offset_of!(A11yTree, strings)), ("revision", core::mem::offset_of!(A11yTree, revision)), ("first_child", core::mem::offset_of!(A11yTree, first_child)), ("child_count", core::mem::offset_of!(A11yTree, child_count)), ("actions", core::mem::offset_of!(A11yTree, actions)), ("actions.len", core::mem::offset_of!(A11yTree, actions.len)), ("actions.stride", core::mem::offset_of!(A11yTree, actions.stride))]);
        check("Output", size_of::<Output>(), align_of::<Output>(), &[("size", core::mem::offset_of!(Output, size)), ("repaint_hz", core::mem::offset_of!(Output, repaint_hz)), ("draw_revision", core::mem::offset_of!(Output, draw_revision)), ("repaint_at", core::mem::offset_of!(Output, repaint_at)), ("built_nodes", core::mem::offset_of!(Output, built_nodes)), ("rebuilt_views", core::mem::offset_of!(Output, rebuilt_views)), ("arranged_nodes", core::mem::offset_of!(Output, arranged_nodes)), ("platform", core::mem::offset_of!(Output, platform)), ("damage", core::mem::offset_of!(Output, damage)), ("a11y", core::mem::offset_of!(Output, a11y)), ("layers", core::mem::offset_of!(Output, layers)), ("damage_list", core::mem::offset_of!(Output, damage_list))]);
        check("PxuiLayerInfo", size_of::<PxuiLayerInfo>(), align_of::<PxuiLayerInfo>(), &[("size", core::mem::offset_of!(PxuiLayerInfo, size)), ("id", core::mem::offset_of!(PxuiLayerInfo, id)), ("breaks", core::mem::offset_of!(PxuiLayerInfo, breaks)), ("role", core::mem::offset_of!(PxuiLayerInfo, role)), ("key", core::mem::offset_of!(PxuiLayerInfo, key)), ("instances", core::mem::offset_of!(PxuiLayerInfo, instances)), ("dirty", core::mem::offset_of!(PxuiLayerInfo, dirty)), ("anchor", core::mem::offset_of!(PxuiLayerInfo, anchor))]);
        check("PxuiDirty", size_of::<PxuiDirty>(), align_of::<PxuiDirty>(), &[("start", core::mem::offset_of!(PxuiDirty, start)), ("end", core::mem::offset_of!(PxuiDirty, end))]);
        check("PxuiMotion", size_of::<PxuiMotion>(), align_of::<PxuiMotion>(), &[("fast", core::mem::offset_of!(PxuiMotion, fast)), ("normal", core::mem::offset_of!(PxuiMotion, normal)), ("slow", core::mem::offset_of!(PxuiMotion, slow)), ("ease", core::mem::offset_of!(PxuiMotion, ease))]);
        check("PxuiThemeTokens", size_of::<ThemeTokens>(), align_of::<ThemeTokens>(), &[("size", core::mem::offset_of!(ThemeTokens, size)), ("panel", core::mem::offset_of!(ThemeTokens, panel)), ("foreground", core::mem::offset_of!(ThemeTokens, foreground)), ("control", core::mem::offset_of!(ThemeTokens, control)), ("input", core::mem::offset_of!(ThemeTokens, input)), ("track", core::mem::offset_of!(ThemeTokens, track)), ("accent", core::mem::offset_of!(ThemeTokens, accent)), ("ink_ground", core::mem::offset_of!(ThemeTokens, ink_ground)), ("has_ink_ground", core::mem::offset_of!(ThemeTokens, has_ink_ground)), ("reserved", core::mem::offset_of!(ThemeTokens, reserved)), ("space", core::mem::offset_of!(ThemeTokens, space)), ("radius", core::mem::offset_of!(ThemeTokens, radius)), ("display_size", core::mem::offset_of!(ThemeTokens, display_size)), ("title_size", core::mem::offset_of!(ThemeTokens, title_size)), ("font_size", core::mem::offset_of!(ThemeTokens, font_size)), ("label_size", core::mem::offset_of!(ThemeTokens, label_size)), ("motion", core::mem::offset_of!(ThemeTokens, motion)), ("reduced_motion", core::mem::offset_of!(ThemeTokens, reduced_motion)), ("dark", core::mem::offset_of!(ThemeTokens, dark)), ("reserved_tail", core::mem::offset_of!(ThemeTokens, reserved_tail)), ("density", core::mem::offset_of!(ThemeTokens, density))]);
        check("PxuiSizeDesc", size_of::<PxuiSizeDesc>(), align_of::<PxuiSizeDesc>(), &[("kind", core::mem::offset_of!(PxuiSizeDesc, kind)), ("value", core::mem::offset_of!(PxuiSizeDesc, value)), ("relative", core::mem::offset_of!(PxuiSizeDesc, relative))]);
        check("PxuiAnchorDesc", size_of::<PxuiAnchorDesc>(), align_of::<PxuiAnchorDesc>(), &[("size", core::mem::offset_of!(PxuiAnchorDesc, size)), ("has_anchor", core::mem::offset_of!(PxuiAnchorDesc, has_anchor)), ("rect", core::mem::offset_of!(PxuiAnchorDesc, rect)), ("side", core::mem::offset_of!(PxuiAnchorDesc, side)), ("align", core::mem::offset_of!(PxuiAnchorDesc, align)), ("gap", core::mem::offset_of!(PxuiAnchorDesc, gap)), ("margin", core::mem::offset_of!(PxuiAnchorDesc, margin)), ("flip", core::mem::offset_of!(PxuiAnchorDesc, flip)), ("reserved", core::mem::offset_of!(PxuiAnchorDesc, reserved))]);
        check("PxuiFontDesc", size_of::<PxuiFontDesc>(), align_of::<PxuiFontDesc>(), &[("family", core::mem::offset_of!(PxuiFontDesc, family)), ("weight", core::mem::offset_of!(PxuiFontDesc, weight)), ("size", core::mem::offset_of!(PxuiFontDesc, size)), ("tracking", core::mem::offset_of!(PxuiFontDesc, tracking)), ("caps", core::mem::offset_of!(PxuiFontDesc, caps)), ("reserved", core::mem::offset_of!(PxuiFontDesc, reserved))]);
        check("PxuiShadowDesc", size_of::<PxuiShadowDesc>(), align_of::<PxuiShadowDesc>(), &[("color", core::mem::offset_of!(PxuiShadowDesc, color)), ("offset", core::mem::offset_of!(PxuiShadowDesc, offset)), ("blur", core::mem::offset_of!(PxuiShadowDesc, blur)), ("spread", core::mem::offset_of!(PxuiShadowDesc, spread)), ("inner", core::mem::offset_of!(PxuiShadowDesc, inner)), ("reserved", core::mem::offset_of!(PxuiShadowDesc, reserved))]);
        check("PxuiRingDesc", size_of::<PxuiRingDesc>(), align_of::<PxuiRingDesc>(), &[("color", core::mem::offset_of!(PxuiRingDesc, color)), ("width", core::mem::offset_of!(PxuiRingDesc, width)), ("sides", core::mem::offset_of!(PxuiRingDesc, sides))]);
        check("PxuiLookDesc", size_of::<PxuiLookDesc>(), align_of::<PxuiLookDesc>(), &[("size", core::mem::offset_of!(PxuiLookDesc, size)), ("fill", core::mem::offset_of!(PxuiLookDesc, fill)), ("gradient", core::mem::offset_of!(PxuiLookDesc, gradient)), ("has_gradient", core::mem::offset_of!(PxuiLookDesc, has_gradient)), ("angle", core::mem::offset_of!(PxuiLookDesc, angle)), ("border", core::mem::offset_of!(PxuiLookDesc, border)), ("border_w", core::mem::offset_of!(PxuiLookDesc, border_w)), ("sides", core::mem::offset_of!(PxuiLookDesc, sides)), ("radius", core::mem::offset_of!(PxuiLookDesc, radius)), ("shadows", core::mem::offset_of!(PxuiLookDesc, shadows)), ("text", core::mem::offset_of!(PxuiLookDesc, text)), ("font", core::mem::offset_of!(PxuiLookDesc, font)), ("opacity", core::mem::offset_of!(PxuiLookDesc, opacity)), ("ring", core::mem::offset_of!(PxuiLookDesc, ring)), ("selection", core::mem::offset_of!(PxuiLookDesc, selection)), ("caret", core::mem::offset_of!(PxuiLookDesc, caret)), ("dashed", core::mem::offset_of!(PxuiLookDesc, dashed)), ("reserved_tail", core::mem::offset_of!(PxuiLookDesc, reserved_tail))]);
        check("PxuiMarkDesc", size_of::<PxuiMarkDesc>(), align_of::<PxuiMarkDesc>(), &[("kind", core::mem::offset_of!(PxuiMarkDesc, kind)), ("direction", core::mem::offset_of!(PxuiMarkDesc, direction)), ("width", core::mem::offset_of!(PxuiMarkDesc, width)), ("rising", core::mem::offset_of!(PxuiMarkDesc, rising)), ("reserved", core::mem::offset_of!(PxuiMarkDesc, reserved))]);
        check("PxuiStateDesc", size_of::<PxuiStateDesc>(), align_of::<PxuiStateDesc>(), &[("size", core::mem::offset_of!(PxuiStateDesc, size)), ("hovered", core::mem::offset_of!(PxuiStateDesc, hovered)), ("pressed", core::mem::offset_of!(PxuiStateDesc, pressed)), ("focused", core::mem::offset_of!(PxuiStateDesc, focused)), ("disabled", core::mem::offset_of!(PxuiStateDesc, disabled)), ("checked", core::mem::offset_of!(PxuiStateDesc, checked)), ("reserved", core::mem::offset_of!(PxuiStateDesc, reserved))]);
        check("PxuiFrameReasons", size_of::<PxuiFrameReasons>(), align_of::<PxuiFrameReasons>(), &[
            ("requested", core::mem::offset_of!(PxuiFrameReasons, requested)),
            ("interaction", core::mem::offset_of!(PxuiFrameReasons, interaction)),
            ("transition", core::mem::offset_of!(PxuiFrameReasons, transition)),
            ("finite_keys", core::mem::offset_of!(PxuiFrameReasons, finite_keys)),
            ("active_loop", core::mem::offset_of!(PxuiFrameReasons, active_loop)),
            ("idle_loop", core::mem::offset_of!(PxuiFrameReasons, idle_loop)),
            ("stepped", core::mem::offset_of!(PxuiFrameReasons, stepped)),
            ("step_state", core::mem::offset_of!(PxuiFrameReasons, step_state)),
            ("step_build", core::mem::offset_of!(PxuiFrameReasons, step_build)),
            ("step_interaction", core::mem::offset_of!(PxuiFrameReasons, step_interaction)),
            ("step_deadline", core::mem::offset_of!(PxuiFrameReasons, step_deadline)),
            ("transition_key", core::mem::offset_of!(PxuiFrameReasons, transition_key)),
            ("transition_curve", core::mem::offset_of!(PxuiFrameReasons, transition_curve)),
            ("reserved", core::mem::offset_of!(PxuiFrameReasons, reserved)),
        ]);
        check("PxuiCurveDesc", size_of::<PxuiCurveDesc>(), align_of::<PxuiCurveDesc>(), &[("size", core::mem::offset_of!(PxuiCurveDesc, size)), ("kind", core::mem::offset_of!(PxuiCurveDesc, kind)), ("duration", core::mem::offset_of!(PxuiCurveDesc, duration)), ("points", core::mem::offset_of!(PxuiCurveDesc, points)), ("omega", core::mem::offset_of!(PxuiCurveDesc, omega)), ("count", core::mem::offset_of!(PxuiCurveDesc, count)), ("positions", core::mem::offset_of!(PxuiCurveDesc, positions)), ("values", core::mem::offset_of!(PxuiCurveDesc, values))]);
        check("PxuiSignal", size_of::<PxuiSignal>(), align_of::<PxuiSignal>(), &[("size", core::mem::offset_of!(PxuiSignal, size)), ("flags", core::mem::offset_of!(PxuiSignal, flags)), ("has_subtree_press", core::mem::offset_of!(PxuiSignal, has_subtree_press)), ("button", core::mem::offset_of!(PxuiSignal, button)), ("has_a11y_value", core::mem::offset_of!(PxuiSignal, has_a11y_value)), ("has_a11y_select", core::mem::offset_of!(PxuiSignal, has_a11y_select)), ("reserved", core::mem::offset_of!(PxuiSignal, reserved)), ("subtree_press", core::mem::offset_of!(PxuiSignal, subtree_press)), ("clicks", core::mem::offset_of!(PxuiSignal, clicks)), ("drag", core::mem::offset_of!(PxuiSignal, drag)), ("pointer", core::mem::offset_of!(PxuiSignal, pointer)), ("press_point", core::mem::offset_of!(PxuiSignal, press_point)), ("release_point", core::mem::offset_of!(PxuiSignal, release_point)), ("scroll", core::mem::offset_of!(PxuiSignal, scroll)), ("pinch", core::mem::offset_of!(PxuiSignal, pinch)), ("a11y_value", core::mem::offset_of!(PxuiSignal, a11y_value)), ("a11y_select", core::mem::offset_of!(PxuiSignal, a11y_select)), ("keys", core::mem::offset_of!(PxuiSignal, keys))]);
        check("PxuiScrubOut", size_of::<PxuiScrubOut>(), align_of::<PxuiScrubOut>(), &[("size", core::mem::offset_of!(PxuiScrubOut, size)), ("flags", core::mem::offset_of!(PxuiScrubOut, flags)), ("node", core::mem::offset_of!(PxuiScrubOut, node)), ("preview", core::mem::offset_of!(PxuiScrubOut, preview)), ("commit", core::mem::offset_of!(PxuiScrubOut, commit))]);
        check("PxuiGestureIn", size_of::<PxuiGestureIn>(), align_of::<PxuiGestureIn>(), &[("size", core::mem::offset_of!(PxuiGestureIn, size)), ("kinds", core::mem::offset_of!(PxuiGestureIn, kinds)), ("reveal", core::mem::offset_of!(PxuiGestureIn, reveal)), ("threshold", core::mem::offset_of!(PxuiGestureIn, threshold))]);
        check("PxuiSwipeIn", size_of::<PxuiSwipeIn>(), align_of::<PxuiSwipeIn>(), &[("size", core::mem::offset_of!(PxuiSwipeIn, size)), ("leading", core::mem::offset_of!(PxuiSwipeIn, leading)), ("trailing", core::mem::offset_of!(PxuiSwipeIn, trailing)), ("full", core::mem::offset_of!(PxuiSwipeIn, full))]);
        check("PxuiDismissIn", size_of::<PxuiDismissIn>(), align_of::<PxuiDismissIn>(), &[("size", core::mem::offset_of!(PxuiDismissIn, size)), ("fraction", core::mem::offset_of!(PxuiDismissIn, fraction)), ("velocity", core::mem::offset_of!(PxuiDismissIn, velocity))]);
        check("PxuiDismissOut", size_of::<PxuiDismissOut>(), align_of::<PxuiDismissOut>(), &[("size", core::mem::offset_of!(PxuiDismissOut, size)), ("flags", core::mem::offset_of!(PxuiDismissOut, flags)), ("offset", core::mem::offset_of!(PxuiDismissOut, offset))]);
        check("PxuiGestureOut", size_of::<PxuiGestureOut>(), align_of::<PxuiGestureOut>(), &[("size", core::mem::offset_of!(PxuiGestureOut, size)), ("flags", core::mem::offset_of!(PxuiGestureOut, flags)), ("swipe", core::mem::offset_of!(PxuiGestureOut, swipe)), ("back", core::mem::offset_of!(PxuiGestureOut, back)), ("pull", core::mem::offset_of!(PxuiGestureOut, pull))]);
        check("PxuiValueFieldDesc", size_of::<PxuiValueFieldDesc>(), align_of::<PxuiValueFieldDesc>(), &[("size", core::mem::offset_of!(PxuiValueFieldDesc, size)), ("flags", core::mem::offset_of!(PxuiValueFieldDesc, flags)), ("font_size", core::mem::offset_of!(PxuiValueFieldDesc, font_size)), ("fraction", core::mem::offset_of!(PxuiValueFieldDesc, fraction)), ("tracking", core::mem::offset_of!(PxuiValueFieldDesc, tracking)), ("line", core::mem::offset_of!(PxuiValueFieldDesc, line)), ("lead_color", core::mem::offset_of!(PxuiValueFieldDesc, lead_color)), ("trail_color", core::mem::offset_of!(PxuiValueFieldDesc, trail_color)), ("reserved", core::mem::offset_of!(PxuiValueFieldDesc, reserved)), ("x", core::mem::offset_of!(PxuiValueFieldDesc, x)), ("y", core::mem::offset_of!(PxuiValueFieldDesc, y)), ("width", core::mem::offset_of!(PxuiValueFieldDesc, width)), ("height", core::mem::offset_of!(PxuiValueFieldDesc, height)), ("display", core::mem::offset_of!(PxuiValueFieldDesc, display)), ("placeholder", core::mem::offset_of!(PxuiValueFieldDesc, placeholder)), ("lead", core::mem::offset_of!(PxuiValueFieldDesc, lead)), ("trail", core::mem::offset_of!(PxuiValueFieldDesc, trail)), ("label", core::mem::offset_of!(PxuiValueFieldDesc, label)), ("value", core::mem::offset_of!(PxuiValueFieldDesc, value))]);
        check("PxuiPaintInfo", size_of::<PxuiPaintInfo>(), align_of::<PxuiPaintInfo>(), &[("size", core::mem::offset_of!(PxuiPaintInfo, size)), ("rtl", core::mem::offset_of!(PxuiPaintInfo, rtl)), ("reserved", core::mem::offset_of!(PxuiPaintInfo, reserved)), ("font_size", core::mem::offset_of!(PxuiPaintInfo, font_size)), ("row_height", core::mem::offset_of!(PxuiPaintInfo, row_height)), ("density", core::mem::offset_of!(PxuiPaintInfo, density)), ("clip", core::mem::offset_of!(PxuiPaintInfo, clip)), ("clip_radius", core::mem::offset_of!(PxuiPaintInfo, clip_radius)), ("theme", core::mem::offset_of!(PxuiPaintInfo, theme))]);
        check("PxuiPopupDesc", size_of::<PxuiPopupDesc>(), align_of::<PxuiPopupDesc>(), &[("size", core::mem::offset_of!(PxuiPopupDesc, size)), ("has_stroke", core::mem::offset_of!(PxuiPopupDesc, has_stroke)), ("has_max_height", core::mem::offset_of!(PxuiPopupDesc, has_max_height)), ("dismiss_initial", core::mem::offset_of!(PxuiPopupDesc, dismiss_initial)), ("attached", core::mem::offset_of!(PxuiPopupDesc, attached)), ("stroke", core::mem::offset_of!(PxuiPopupDesc, stroke)), ("max_height", core::mem::offset_of!(PxuiPopupDesc, max_height)), ("x", core::mem::offset_of!(PxuiPopupDesc, x)), ("y", core::mem::offset_of!(PxuiPopupDesc, y)), ("width", core::mem::offset_of!(PxuiPopupDesc, width)), ("height", core::mem::offset_of!(PxuiPopupDesc, height)), ("label", core::mem::offset_of!(PxuiPopupDesc, label))]);
        check("PxuiButtonDesc", size_of::<PxuiButtonDesc>(), align_of::<PxuiButtonDesc>(), &[("size", core::mem::offset_of!(PxuiButtonDesc, size)), ("enabled", core::mem::offset_of!(PxuiButtonDesc, enabled)), ("selected", core::mem::offset_of!(PxuiButtonDesc, selected)), ("danger", core::mem::offset_of!(PxuiButtonDesc, danger)), ("justify", core::mem::offset_of!(PxuiButtonDesc, justify)), ("w_kind", core::mem::offset_of!(PxuiButtonDesc, w_kind)), ("h_kind", core::mem::offset_of!(PxuiButtonDesc, h_kind)), ("has_name", core::mem::offset_of!(PxuiButtonDesc, has_name)), ("reserved", core::mem::offset_of!(PxuiButtonDesc, reserved)), ("w", core::mem::offset_of!(PxuiButtonDesc, w)), ("h", core::mem::offset_of!(PxuiButtonDesc, h)), ("padding", core::mem::offset_of!(PxuiButtonDesc, padding)), ("states", core::mem::offset_of!(PxuiButtonDesc, states)), ("key", core::mem::offset_of!(PxuiButtonDesc, key)), ("text", core::mem::offset_of!(PxuiButtonDesc, text)), ("name", core::mem::offset_of!(PxuiButtonDesc, name)), ("centered", core::mem::offset_of!(PxuiButtonDesc, centered)), ("reserved2", core::mem::offset_of!(PxuiButtonDesc, reserved2)), ("variant", core::mem::offset_of!(PxuiButtonDesc, variant)), ("reserved5", core::mem::offset_of!(PxuiButtonDesc, reserved5)), ("has_label", core::mem::offset_of!(PxuiButtonDesc, has_label)), ("axis", core::mem::offset_of!(PxuiButtonDesc, axis)), ("content_align", core::mem::offset_of!(PxuiButtonDesc, content_align)), ("focus_on_click", core::mem::offset_of!(PxuiButtonDesc, focus_on_click)), ("gap", core::mem::offset_of!(PxuiButtonDesc, gap)), ("label", core::mem::offset_of!(PxuiButtonDesc, label)), ("label_box", core::mem::offset_of!(PxuiButtonDesc, label_box)), ("lead", core::mem::offset_of!(PxuiButtonDesc, lead)), ("lead_draw", core::mem::offset_of!(PxuiButtonDesc, lead_draw)), ("lead_user", core::mem::offset_of!(PxuiButtonDesc, lead_user)), ("detail", core::mem::offset_of!(PxuiButtonDesc, detail)), ("accessory", core::mem::offset_of!(PxuiButtonDesc, accessory)), ("has_detail_label", core::mem::offset_of!(PxuiButtonDesc, has_detail_label)), ("has_accessory_label", core::mem::offset_of!(PxuiButtonDesc, has_accessory_label)), ("reserved3", core::mem::offset_of!(PxuiButtonDesc, reserved3)), ("detail_label", core::mem::offset_of!(PxuiButtonDesc, detail_label)), ("detail_box", core::mem::offset_of!(PxuiButtonDesc, detail_box)), ("accessory_label", core::mem::offset_of!(PxuiButtonDesc, accessory_label)), ("accessory_box", core::mem::offset_of!(PxuiButtonDesc, accessory_box))]);
        check("PxuiDialogDesc", size_of::<PxuiDialogDesc>(), align_of::<PxuiDialogDesc>(), &[("size", core::mem::offset_of!(PxuiDialogDesc, size)), ("width", core::mem::offset_of!(PxuiDialogDesc, width)), ("key", core::mem::offset_of!(PxuiDialogDesc, key)), ("title", core::mem::offset_of!(PxuiDialogDesc, title)), ("buttons", core::mem::offset_of!(PxuiDialogDesc, buttons)), ("default_at", core::mem::offset_of!(PxuiDialogDesc, default_at)), ("cancel_at", core::mem::offset_of!(PxuiDialogDesc, cancel_at)), ("danger", core::mem::offset_of!(PxuiDialogDesc, danger)), ("disabled", core::mem::offset_of!(PxuiDialogDesc, disabled)), ("details", core::mem::offset_of!(PxuiDialogDesc, details)), ("variant", core::mem::offset_of!(PxuiDialogDesc, variant)), ("reserved_variant", core::mem::offset_of!(PxuiDialogDesc, reserved_variant))]);
        check("PxuiDialogResult", size_of::<PxuiDialogResult>(), align_of::<PxuiDialogResult>(), &[("size", core::mem::offset_of!(PxuiDialogResult, size)), ("has_chosen", core::mem::offset_of!(PxuiDialogResult, has_chosen)), ("dismissed", core::mem::offset_of!(PxuiDialogResult, dismissed)), ("reserved", core::mem::offset_of!(PxuiDialogResult, reserved)), ("chosen", core::mem::offset_of!(PxuiDialogResult, chosen))]);
        check("PxuiSegmentedResult", size_of::<PxuiSegmentedResult>(), align_of::<PxuiSegmentedResult>(), &[("size", core::mem::offset_of!(PxuiSegmentedResult, size)), ("has_chosen", core::mem::offset_of!(PxuiSegmentedResult, has_chosen)), ("has_secondary", core::mem::offset_of!(PxuiSegmentedResult, has_secondary)), ("reserved", core::mem::offset_of!(PxuiSegmentedResult, reserved)), ("chosen", core::mem::offset_of!(PxuiSegmentedResult, chosen)), ("secondary", core::mem::offset_of!(PxuiSegmentedResult, secondary))]);
        check("PxuiKeyedDesc", size_of::<PxuiKeyedDesc>(), align_of::<PxuiKeyedDesc>(), &[("size", core::mem::offset_of!(PxuiKeyedDesc, size)), ("variant", core::mem::offset_of!(PxuiKeyedDesc, variant)), ("reserved", core::mem::offset_of!(PxuiKeyedDesc, reserved)), ("key", core::mem::offset_of!(PxuiKeyedDesc, key)), ("name", core::mem::offset_of!(PxuiKeyedDesc, name))]);
        check("PxuiSheetDesc", size_of::<PxuiSheetDesc>(), align_of::<PxuiSheetDesc>(), &[("size", core::mem::offset_of!(PxuiSheetDesc, size)), ("has_width", core::mem::offset_of!(PxuiSheetDesc, has_width)), ("present", core::mem::offset_of!(PxuiSheetDesc, present)), ("reserved", core::mem::offset_of!(PxuiSheetDesc, reserved)), ("width", core::mem::offset_of!(PxuiSheetDesc, width)), ("dismiss_fraction", core::mem::offset_of!(PxuiSheetDesc, dismiss_fraction)), ("dismiss_velocity", core::mem::offset_of!(PxuiSheetDesc, dismiss_velocity))]);
        check("PxuiCellState", size_of::<PxuiCellState>(), align_of::<PxuiCellState>(), &[("size", core::mem::offset_of!(PxuiCellState, size)), ("selected", core::mem::offset_of!(PxuiCellState, selected)), ("enabled", core::mem::offset_of!(PxuiCellState, enabled)), ("focused", core::mem::offset_of!(PxuiCellState, focused)), ("held", core::mem::offset_of!(PxuiCellState, held)), ("handle", core::mem::offset_of!(PxuiCellState, handle)), ("y", core::mem::offset_of!(PxuiCellState, y)), ("reserved", core::mem::offset_of!(PxuiCellState, reserved))]);
        check("PxuiSectionListDesc", size_of::<PxuiSectionListDesc>(), align_of::<PxuiSectionListDesc>(), &[("size", core::mem::offset_of!(PxuiSectionListDesc, size)), ("overlay_scrollbar", core::mem::offset_of!(PxuiSectionListDesc, overlay_scrollbar)), ("measured", core::mem::offset_of!(PxuiSectionListDesc, measured)), ("reserved", core::mem::offset_of!(PxuiSectionListDesc, reserved)), ("count", core::mem::offset_of!(PxuiSectionListDesc, count)), ("row_height", core::mem::offset_of!(PxuiSectionListDesc, row_height)), ("leading_height", core::mem::offset_of!(PxuiSectionListDesc, leading_height)), ("header_height", core::mem::offset_of!(PxuiSectionListDesc, header_height)), ("trailing_height", core::mem::offset_of!(PxuiSectionListDesc, trailing_height))]);
        check("PxuiListResult", size_of::<PxuiListResult>(), align_of::<PxuiListResult>(), &[("size", core::mem::offset_of!(PxuiListResult, size)), ("reserved", core::mem::offset_of!(PxuiListResult, reserved)), ("handle", core::mem::offset_of!(PxuiListResult, handle)), ("row", core::mem::offset_of!(PxuiListResult, row)), ("activated", core::mem::offset_of!(PxuiListResult, activated)), ("selected", core::mem::offset_of!(PxuiListResult, selected)), ("secondary", core::mem::offset_of!(PxuiListResult, secondary)), ("anchor", core::mem::offset_of!(PxuiListResult, anchor))]);
        check("PxuiSelectionListDesc", size_of::<PxuiSelectionListDesc>(), align_of::<PxuiSelectionListDesc>(), &[("size", core::mem::offset_of!(PxuiSelectionListDesc, size)), ("mode", core::mem::offset_of!(PxuiSelectionListDesc, mode)), ("fit", core::mem::offset_of!(PxuiSelectionListDesc, fit)), ("open_on_click", core::mem::offset_of!(PxuiSelectionListDesc, open_on_click)), ("overlay_scrollbar", core::mem::offset_of!(PxuiSelectionListDesc, overlay_scrollbar)), ("measured", core::mem::offset_of!(PxuiSelectionListDesc, measured)), ("bare", core::mem::offset_of!(PxuiSelectionListDesc, bare)), ("has_revision", core::mem::offset_of!(PxuiSelectionListDesc, has_revision)), ("reserved", core::mem::offset_of!(PxuiSelectionListDesc, reserved)), ("count", core::mem::offset_of!(PxuiSelectionListDesc, count)), ("row_height", core::mem::offset_of!(PxuiSelectionListDesc, row_height)), ("leading_height", core::mem::offset_of!(PxuiSelectionListDesc, leading_height)), ("header_height", core::mem::offset_of!(PxuiSelectionListDesc, header_height)), ("trailing_height", core::mem::offset_of!(PxuiSelectionListDesc, trailing_height)), ("rows", core::mem::offset_of!(PxuiSelectionListDesc, rows)), ("anchor", core::mem::offset_of!(PxuiSelectionListDesc, anchor)), ("revision", core::mem::offset_of!(PxuiSelectionListDesc, revision))]);
        check("PxuiColumn", size_of::<PxuiColumn>(), align_of::<PxuiColumn>(), &[("size", core::mem::offset_of!(PxuiColumn, size)), ("sortable", core::mem::offset_of!(PxuiColumn, sortable)), ("grip", core::mem::offset_of!(PxuiColumn, grip)), ("has_id", core::mem::offset_of!(PxuiColumn, has_id)), ("reserved", core::mem::offset_of!(PxuiColumn, reserved)), ("title", core::mem::offset_of!(PxuiColumn, title)), ("id", core::mem::offset_of!(PxuiColumn, id)), ("share", core::mem::offset_of!(PxuiColumn, share)), ("w", core::mem::offset_of!(PxuiColumn, w))]);
        check("PxuiColumnWidth", size_of::<PxuiColumnWidth>(), align_of::<PxuiColumnWidth>(), &[("id", core::mem::offset_of!(PxuiColumnWidth, id)), ("w", core::mem::offset_of!(PxuiColumnWidth, w)), ("reserved", core::mem::offset_of!(PxuiColumnWidth, reserved))]);
        check("PxuiColumnWidthSpan", size_of::<PxuiColumnWidthSpan>(), align_of::<PxuiColumnWidthSpan>(), &[("ptr", core::mem::offset_of!(PxuiColumnWidthSpan, ptr)), ("len", core::mem::offset_of!(PxuiColumnWidthSpan, len))]);
        check("PxuiTableHeadDesc", size_of::<PxuiTableHeadDesc>(), align_of::<PxuiTableHeadDesc>(), &[("size", core::mem::offset_of!(PxuiTableHeadDesc, size)), ("has_sort", core::mem::offset_of!(PxuiTableHeadDesc, has_sort)), ("ascending", core::mem::offset_of!(PxuiTableHeadDesc, ascending)), ("reserved", core::mem::offset_of!(PxuiTableHeadDesc, reserved)), ("columns", core::mem::offset_of!(PxuiTableHeadDesc, columns)), ("widths", core::mem::offset_of!(PxuiTableHeadDesc, widths)), ("sort_id", core::mem::offset_of!(PxuiTableHeadDesc, sort_id)), ("h", core::mem::offset_of!(PxuiTableHeadDesc, h)), ("reserved2", core::mem::offset_of!(PxuiTableHeadDesc, reserved2))]);
        check("PxuiHeadResult", size_of::<PxuiHeadResult>(), align_of::<PxuiHeadResult>(), &[("size", core::mem::offset_of!(PxuiHeadResult, size)), ("sorted", core::mem::offset_of!(PxuiHeadResult, sorted)), ("resized", core::mem::offset_of!(PxuiHeadResult, resized)), ("has_sort", core::mem::offset_of!(PxuiHeadResult, has_sort)), ("ascending", core::mem::offset_of!(PxuiHeadResult, ascending)), ("sort_id", core::mem::offset_of!(PxuiHeadResult, sort_id)), ("response", core::mem::offset_of!(PxuiHeadResult, response))]);
        check("PxuiTableDesc", size_of::<PxuiTableDesc>(), align_of::<PxuiTableDesc>(), &[("size", core::mem::offset_of!(PxuiTableDesc, size)), ("mode", core::mem::offset_of!(PxuiTableDesc, mode)), ("has_sort", core::mem::offset_of!(PxuiTableDesc, has_sort)), ("ascending", core::mem::offset_of!(PxuiTableDesc, ascending)), ("has_revision", core::mem::offset_of!(PxuiTableDesc, has_revision)), ("columns", core::mem::offset_of!(PxuiTableDesc, columns)), ("widths", core::mem::offset_of!(PxuiTableDesc, widths)), ("sort_id", core::mem::offset_of!(PxuiTableDesc, sort_id)), ("rows", core::mem::offset_of!(PxuiTableDesc, rows)), ("row_h", core::mem::offset_of!(PxuiTableDesc, row_h)), ("head_h", core::mem::offset_of!(PxuiTableDesc, head_h)), ("flags", core::mem::offset_of!(PxuiTableDesc, flags)), ("anchor", core::mem::offset_of!(PxuiTableDesc, anchor)), ("head_variant", core::mem::offset_of!(PxuiTableDesc, head_variant)), ("open_on_click", core::mem::offset_of!(PxuiTableDesc, open_on_click)), ("bare", core::mem::offset_of!(PxuiTableDesc, bare)), ("reserved2", core::mem::offset_of!(PxuiTableDesc, reserved2)), ("revision", core::mem::offset_of!(PxuiTableDesc, revision))]);
        check("PxuiTableResult", size_of::<PxuiTableResult>(), align_of::<PxuiTableResult>(), &[("size", core::mem::offset_of!(PxuiTableResult, size)), ("reserved", core::mem::offset_of!(PxuiTableResult, reserved)), ("rows", core::mem::offset_of!(PxuiTableResult, rows)), ("head", core::mem::offset_of!(PxuiTableResult, head))]);
        check("PxuiPickRow", size_of::<PxuiPickRow>(), align_of::<PxuiPickRow>(), &[("size", core::mem::offset_of!(PxuiPickRow, size)), ("art_kind", core::mem::offset_of!(PxuiPickRow, art_kind)), ("reserved", core::mem::offset_of!(PxuiPickRow, reserved)), ("art", core::mem::offset_of!(PxuiPickRow, art)), ("label", core::mem::offset_of!(PxuiPickRow, label)), ("detail", core::mem::offset_of!(PxuiPickRow, detail)), ("heading", core::mem::offset_of!(PxuiPickRow, heading))]);
        check("PxuiResponse", size_of::<PxuiResponse>(), align_of::<PxuiResponse>(), &[("size", core::mem::offset_of!(PxuiResponse, size)), ("flags", core::mem::offset_of!(PxuiResponse, flags)), ("node", core::mem::offset_of!(PxuiResponse, node))]);
        check("PxuiIconButtonDesc", size_of::<PxuiIconButtonDesc>(), align_of::<PxuiIconButtonDesc>(), &[("size", core::mem::offset_of!(PxuiIconButtonDesc, size)), ("enabled", core::mem::offset_of!(PxuiIconButtonDesc, enabled)), ("w_kind", core::mem::offset_of!(PxuiIconButtonDesc, w_kind)), ("h_kind", core::mem::offset_of!(PxuiIconButtonDesc, h_kind)), ("reserved", core::mem::offset_of!(PxuiIconButtonDesc, reserved)), ("w", core::mem::offset_of!(PxuiIconButtonDesc, w)), ("h", core::mem::offset_of!(PxuiIconButtonDesc, h)), ("press_scale", core::mem::offset_of!(PxuiIconButtonDesc, press_scale)), ("states", core::mem::offset_of!(PxuiIconButtonDesc, states)), ("key", core::mem::offset_of!(PxuiIconButtonDesc, key)), ("name", core::mem::offset_of!(PxuiIconButtonDesc, name)), ("icon", core::mem::offset_of!(PxuiIconButtonDesc, icon)), ("draw", core::mem::offset_of!(PxuiIconButtonDesc, draw)), ("draw_user", core::mem::offset_of!(PxuiIconButtonDesc, draw_user)), ("press", core::mem::offset_of!(PxuiIconButtonDesc, press)), ("checkable", core::mem::offset_of!(PxuiIconButtonDesc, checkable)), ("checked", core::mem::offset_of!(PxuiIconButtonDesc, checked)), ("variant", core::mem::offset_of!(PxuiIconButtonDesc, variant)), ("reserved2", core::mem::offset_of!(PxuiIconButtonDesc, reserved2))]);
        check("PxuiTextFieldDesc", size_of::<PxuiTextFieldDesc>(), align_of::<PxuiTextFieldDesc>(), &[("size", core::mem::offset_of!(PxuiTextFieldDesc, size)), ("autocorrect", core::mem::offset_of!(PxuiTextFieldDesc, autocorrect)), ("capitalization", core::mem::offset_of!(PxuiTextFieldDesc, capitalization)), ("action", core::mem::offset_of!(PxuiTextFieldDesc, action)), ("reserved", core::mem::offset_of!(PxuiTextFieldDesc, reserved)), ("key", core::mem::offset_of!(PxuiTextFieldDesc, key)), ("name", core::mem::offset_of!(PxuiTextFieldDesc, name)), ("value", core::mem::offset_of!(PxuiTextFieldDesc, value)), ("variant", core::mem::offset_of!(PxuiTextFieldDesc, variant)), ("reserved_variant", core::mem::offset_of!(PxuiTextFieldDesc, reserved_variant))]);
        check("PxuiCheckboxDesc", size_of::<PxuiCheckboxDesc>(), align_of::<PxuiCheckboxDesc>(), &[("size", core::mem::offset_of!(PxuiCheckboxDesc, size)), ("disabled", core::mem::offset_of!(PxuiCheckboxDesc, disabled)), ("on", core::mem::offset_of!(PxuiCheckboxDesc, on)), ("passive", core::mem::offset_of!(PxuiCheckboxDesc, passive)), ("reserved", core::mem::offset_of!(PxuiCheckboxDesc, reserved)), ("box_size", core::mem::offset_of!(PxuiCheckboxDesc, box_size)), ("look", core::mem::offset_of!(PxuiCheckboxDesc, look)), ("key", core::mem::offset_of!(PxuiCheckboxDesc, key)), ("text", core::mem::offset_of!(PxuiCheckboxDesc, text)), ("variant", core::mem::offset_of!(PxuiCheckboxDesc, variant)), ("reserved_variant", core::mem::offset_of!(PxuiCheckboxDesc, reserved_variant))]);
        check("PxuiToggleDesc", size_of::<PxuiToggleDesc>(), align_of::<PxuiToggleDesc>(), &[("size", core::mem::offset_of!(PxuiToggleDesc, size)), ("enabled", core::mem::offset_of!(PxuiToggleDesc, enabled)), ("on", core::mem::offset_of!(PxuiToggleDesc, on)), ("press_row", core::mem::offset_of!(PxuiToggleDesc, press_row)), ("reserved", core::mem::offset_of!(PxuiToggleDesc, reserved)), ("row_h", core::mem::offset_of!(PxuiToggleDesc, row_h)), ("switch_w", core::mem::offset_of!(PxuiToggleDesc, switch_w)), ("switch_h", core::mem::offset_of!(PxuiToggleDesc, switch_h)), ("inset", core::mem::offset_of!(PxuiToggleDesc, inset)), ("track", core::mem::offset_of!(PxuiToggleDesc, track)), ("knob", core::mem::offset_of!(PxuiToggleDesc, knob)), ("key", core::mem::offset_of!(PxuiToggleDesc, key)), ("text", core::mem::offset_of!(PxuiToggleDesc, text)), ("detail", core::mem::offset_of!(PxuiToggleDesc, detail)), ("has_style", core::mem::offset_of!(PxuiToggleDesc, has_style)), ("reserved2", core::mem::offset_of!(PxuiToggleDesc, reserved2)), ("variant", core::mem::offset_of!(PxuiToggleDesc, variant)), ("reserved3", core::mem::offset_of!(PxuiToggleDesc, reserved3)), ("style", core::mem::offset_of!(PxuiToggleDesc, style))]);
        check("PxuiSegmentedDesc", size_of::<PxuiSegmentedDesc>(), align_of::<PxuiSegmentedDesc>(), &[("size", core::mem::offset_of!(PxuiSegmentedDesc, size)), ("enabled", core::mem::offset_of!(PxuiSegmentedDesc, enabled)), ("grow", core::mem::offset_of!(PxuiSegmentedDesc, grow)), ("reserved", core::mem::offset_of!(PxuiSegmentedDesc, reserved)), ("h", core::mem::offset_of!(PxuiSegmentedDesc, h)), ("states", core::mem::offset_of!(PxuiSegmentedDesc, states)), ("key", core::mem::offset_of!(PxuiSegmentedDesc, key)), ("name", core::mem::offset_of!(PxuiSegmentedDesc, name)), ("selected", core::mem::offset_of!(PxuiSegmentedDesc, selected)), ("w", core::mem::offset_of!(PxuiSegmentedDesc, w)), ("padding", core::mem::offset_of!(PxuiSegmentedDesc, padding)), ("min_segment", core::mem::offset_of!(PxuiSegmentedDesc, min_segment)), ("variant", core::mem::offset_of!(PxuiSegmentedDesc, variant)), ("reserved2", core::mem::offset_of!(PxuiSegmentedDesc, reserved2)), ("names", core::mem::offset_of!(PxuiSegmentedDesc, names)), ("has_selected", core::mem::offset_of!(PxuiSegmentedDesc, has_selected)), ("reserved3", core::mem::offset_of!(PxuiSegmentedDesc, reserved3)), ("focus_on_click", core::mem::offset_of!(PxuiSegmentedDesc, focus_on_click)), ("reserved4", core::mem::offset_of!(PxuiSegmentedDesc, reserved4))]);
        check("PxuiPickItem", size_of::<PxuiPickItem>(), align_of::<PxuiPickItem>(), &[("has_id", core::mem::offset_of!(PxuiPickItem, has_id)), ("reserved", core::mem::offset_of!(PxuiPickItem, reserved)), ("id", core::mem::offset_of!(PxuiPickItem, id)), ("text", core::mem::offset_of!(PxuiPickItem, text)), ("name", core::mem::offset_of!(PxuiPickItem, name)), ("detail", core::mem::offset_of!(PxuiPickItem, detail))]);
        check("PxuiHeadActionDesc", size_of::<PxuiHeadActionDesc>(), align_of::<PxuiHeadActionDesc>(), &[("size", core::mem::offset_of!(PxuiHeadActionDesc, size)), ("active", core::mem::offset_of!(PxuiHeadActionDesc, active)), ("usable", core::mem::offset_of!(PxuiHeadActionDesc, usable)), ("reserved", core::mem::offset_of!(PxuiHeadActionDesc, reserved)), ("caption", core::mem::offset_of!(PxuiHeadActionDesc, caption)), ("keycap", core::mem::offset_of!(PxuiHeadActionDesc, keycap))]);
        check("PxuiHeaderDesc", size_of::<PxuiHeaderDesc>(), align_of::<PxuiHeaderDesc>(), &[("size", core::mem::offset_of!(PxuiHeaderDesc, size)), ("has_kind", core::mem::offset_of!(PxuiHeaderDesc, has_kind)), ("has_badge", core::mem::offset_of!(PxuiHeaderDesc, has_badge)), ("has_index", core::mem::offset_of!(PxuiHeaderDesc, has_index)), ("has_reset", core::mem::offset_of!(PxuiHeaderDesc, has_reset)), ("reset_enabled", core::mem::offset_of!(PxuiHeaderDesc, reset_enabled)), ("reserved", core::mem::offset_of!(PxuiHeaderDesc, reserved)), ("key", core::mem::offset_of!(PxuiHeaderDesc, key)), ("kind", core::mem::offset_of!(PxuiHeaderDesc, kind)), ("badge", core::mem::offset_of!(PxuiHeaderDesc, badge)), ("index", core::mem::offset_of!(PxuiHeaderDesc, index)), ("reset", core::mem::offset_of!(PxuiHeaderDesc, reset)), ("title", core::mem::offset_of!(PxuiHeaderDesc, title)), ("detail", core::mem::offset_of!(PxuiHeaderDesc, detail)), ("actions", core::mem::offset_of!(PxuiHeaderDesc, actions))]);
        check("PxuiHeadOut", size_of::<PxuiHeadOut>(), align_of::<PxuiHeadOut>(), &[("size", core::mem::offset_of!(PxuiHeadOut, size)), ("has_chosen", core::mem::offset_of!(PxuiHeadOut, has_chosen)), ("reset_pressed", core::mem::offset_of!(PxuiHeadOut, reset_pressed)), ("reserved", core::mem::offset_of!(PxuiHeadOut, reserved)), ("renamed", core::mem::offset_of!(PxuiHeadOut, renamed)), ("chosen", core::mem::offset_of!(PxuiHeadOut, chosen))]);
    }
}
