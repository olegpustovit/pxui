#![allow(clippy::all)]
#![deny(missing_docs)]
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
pub mod text_edit;
pub mod theme;
pub mod ui;

pub use node::{Axis, Bar, Dir, Mark, Node};
pub use text::{Label, TextGlyph, TextMetrics, TextSpan, TextSpans};
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
    /// Uses a fixed length in logical points.
    Px(f32),
    /// Uses a fraction of the available extent.
    Pct(f32),
    /// Evaluates a host function against the available extent.
    Rel(extern "C" fn(f32) -> f32),
    /// Fills the space available from the parent.
    Grow,
    /// Sizes to fit the content.
    Fit,
}

impl Size {
    /// Reports whether this size fills the available extent.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::Size::Grow.is_grow());
    /// ```
    pub fn is_grow(&self) -> bool {
        matches!(self, Self::Grow)
    }
}

/// Painter view transform.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Distance from the relevant origin in logical points.
    pub offset: [f32; 2],
    /// Multiplier for the element alpha, from zero to one.
    pub opacity: f32,
    /// Clip region in logical points.
    pub clip: [f32; 4],
    /// Drawable pixels per logical point.
    pub scale: f32,
    /// Transform applied to local coordinates.
    pub transform: [f32; 4],
    /// Origin about which the transform is applied.
    pub pivot: [f32; 2],
}

impl View {
    /// Transform that leaves coordinates and opacity unchanged.
    pub const IDENTITY: Self = Self {
        offset: [0.0; 2],
        opacity: 1.0,
        clip: [-32768.0, -32768.0, 32767.0, 32767.0],
        scale: 1.0,
        transform: [1.0, 0.0, 0.0, 1.0],
        pivot: [0.0; 2],
    };

    /// Creates a view rotated by radians about the supplied pivot.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let view = pxui_types::View::rotation(0.0, [0.0, 0.0]);
    /// assert_eq!(view.transform, pxui_types::View::IDENTITY.transform);
    /// ```
    pub fn rotation(radians: f32, pivot: [f32; 2]) -> Self {
        let (s, c) = radians.sin_cos();
        Self {
            transform: [c, -s, s, c],
            pivot,
            ..Self::IDENTITY
        }
    }
}

/// Maximum number of draw layers available in a frame.
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
/// The app was built for another major version of the interface or a newer minor than this library (`pxui_ui_create`'s status): the
/// library cannot serve it, and nothing was created.
pub const PXUI_ERR_VERSION: i32 = -6;

/// Key codes of `Event::KEY_PRESSED` and `KEY_RELEASED`: a Unicode scalar value below `KEY_SPECIAL`
/// is that character; `KEY_SPECIAL + n` is the special key `n` (arrows, enter, F-keys...) in the
/// order documented in `pxui.h`; `KEY_UNKNOWN | code` carries a platform code the library has no name for.
pub const KEY_SPECIAL: u32 = 0x11_0000;
/// Bit identifying a platform key code without a portable name.
pub const KEY_UNKNOWN: u32 = 0x8000_0000;

/// Borrowed byte span containing UTF-8 text where required.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Str {
    /// Pointer to the first byte of the borrowed span.
    pub ptr: *const u8,
    /// Length of the byte span in bytes.
    pub len: usize,
}

impl From<&'static str> for Str {
    /// A wire span whose text remains valid for the lifetime of the process.
    fn from(value: &'static str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }
}

/// How the platform keyboard capitalizes what is typed into a text field (`TextTraits::capitalization`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum Capitalize {
    /// Nothing is capitalized for the user.
    None = 0,
    /// The first letter of each sentence.
    #[default]
    Sentences = 1,
    /// The first letter of each word.
    Words = 2,
    /// Every letter.
    Letters = 3,
}

/// What the return key of a text field's keyboard reads and does (`TextTraits::action`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum ReturnKey {
    /// The ordinary return key: the kit's default.
    #[default]
    Return = 0,
    /// Enter ends editing: it dismisses focus and the keyboard.
    Done = 1,
    /// The key reads Search; submission is the app's.
    Search = 2,
}

/// The native keyboard traits of a single-line text field (the part of `pxui::TextField` the platform reads), as a `Copy` value built with
/// `const fn` builders from `TextTraits::DEFAULT` or `TextTraits::SEARCH`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextTraits {
    pub(crate) autocorrect: bool,
    pub(crate) capitalization: Capitalize,
    pub(crate) action: ReturnKey,
}
impl Default for TextTraits {
    fn default() -> Self { Self::DEFAULT }
}
/// The C record of the text input traits (`PxuiTextFieldDesc`, `pxui_ui_text_input_options`): `TextTraits` as three numbers. For C
/// callers and the library's own conversions, not for authoring.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PxuiTextInputOptions {
    /// `size_of::<PxuiTextInputOptions>()` (16): a record whose size is short is refused; `Default` and `From<TextTraits>` set it.
    pub size: u32,
    /// 0 or 1: the platform keyboard corrects words while typing.
    pub autocorrect: u32,
    /// 0 none, 1 sentences, 2 words, 3 letters.
    pub capitalization: u32,
    /// The return key: 0 Return, 1 Done (Enter ends editing), 2 Search (submission is the app's).
    pub action: u32,
}
impl Default for PxuiTextInputOptions {
    fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, autocorrect: 0, capitalization: 0, action: 0 } }
}
impl From<TextTraits> for PxuiTextInputOptions {
    fn from(o: TextTraits) -> Self { Self {size:core::mem::size_of::<Self>() as u32,autocorrect:o.autocorrect as u32,capitalization:o.capitalization as u32,action:o.action as u32} }
}
impl TextTraits {
    /// The traits of three numbers as the C records carry them, or none when one is out of range (autocorrect over 1, capitalization over 3,
    /// action over 2).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::TextTraits;
    /// assert_eq!(TextTraits::from_numbers(0, 0, 2), Some(TextTraits::SEARCH));
    /// assert_eq!(TextTraits::from_numbers(2, 0, 0), None);
    /// ```
    pub const fn from_numbers(autocorrect: u32, capitalization: u32, action: u32) -> Option<TextTraits> {
        let capitalization = match capitalization { 0 => Capitalize::None, 1 => Capitalize::Sentences, 2 => Capitalize::Words, 3 => Capitalize::Letters, _ => return None };
        let action = match action { 0 => ReturnKey::Return, 1 => ReturnKey::Done, 2 => ReturnKey::Search, _ => return None };
        if autocorrect > 1 { return None; }
        Some(TextTraits { autocorrect: autocorrect == 1, capitalization, action })
    }
    /// The kit's text field: autocorrect on, sentence capitalization, Return.
    pub const DEFAULT: Self = Self {autocorrect:true,capitalization:Capitalize::Sentences,action:ReturnKey::Return};
    /// A search box: no autocorrect, no capitalization, the Search key.
    pub const SEARCH: Self = Self {autocorrect:false,capitalization:Capitalize::None,action:ReturnKey::Search};
    /// These traits with autocorrect on or off.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::TextTraits;
    /// assert!(!TextTraits::DEFAULT.autocorrect(false).autocorrects());
    /// ```
    pub const fn autocorrect(mut self, autocorrect: bool) -> Self { self.autocorrect = autocorrect; self }
    /// These traits with the keyboard's capitalization.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{Capitalize, TextTraits};
    /// assert_eq!(TextTraits::DEFAULT.capitalization(Capitalize::Words).capitalizes(), Capitalize::Words);
    /// ```
    pub const fn capitalization(mut self, capitalization: Capitalize) -> Self { self.capitalization = capitalization; self }
    /// These traits with the return key's action.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{ReturnKey, TextTraits};
    /// assert_eq!(TextTraits::SEARCH.action(ReturnKey::Done).return_key(), ReturnKey::Done);
    /// ```
    pub const fn action(mut self, action: ReturnKey) -> Self { self.action = action; self }
    /// Whether the platform keyboard corrects words.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::TextTraits::DEFAULT.autocorrects());
    /// ```
    pub const fn autocorrects(&self) -> bool { self.autocorrect }
    /// The platform keyboard's capitalization.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::TextTraits::DEFAULT.capitalizes(), pxui_types::Capitalize::Sentences);
    /// ```
    pub const fn capitalizes(&self) -> Capitalize { self.capitalization }
    /// The return key's action.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::TextTraits::DEFAULT.return_key(), pxui_types::ReturnKey::Return);
    /// ```
    pub const fn return_key(&self) -> ReturnKey { self.action }
}

/// Sized `TOUCH` payload borrowed through `Event::text`, with `size_of::<Touch>()` bytes.
/// Phase: 0 began, 1 moved, 2 ended, 3 cancelled. Position: logical points; time: source monotonic seconds.
/// Pressure is finite in 0..=1. Positions may lie outside the window; identities retain all 64 bits.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touch {
    /// `size_of::<Touch>()` (56): `Touch::new` and `default` set it; `Event::text.len` carries the record's length and the library
    /// refuses a record whose `size` is short or past it.
    pub size:u32,
    /// Phase: 0 began, 1 moved, 2 ended, 3 cancelled; any other value is refused.
    pub phase:u32,
    /// Stable id of the touch input device.
    pub device:u64,
    /// Id of the contact on that device.
    pub finger:u64,
    /// Horizontal coordinate in logical points.
    pub x:f64,
    /// Vertical coordinate in logical points.
    pub y:f64,
    /// Time in seconds on the host monotonic clock.
    pub time:f64,
    /// Normalized pressure reported by the touch device.
    pub pressure:f32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved:u32,
}
impl Default for Touch { fn default() -> Self { Touch::new(0, 0, [0.0; 2], 0.0, 0.0, 0) } }

impl Touch {
    /// Builds a touch sample from its native scalar values.
    ///
    /// Position is in logical surface points and time is monotonic seconds. The consumer validates phase and pressure.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let _ = pxui_types::Touch::new(1, 2, [10.0, 20.0], 0.0, 1.0, 0);
    /// ```
    pub const fn new(device: u64, finger: u64, at: [f64; 2], time: f64, pressure: f32, phase: u32) -> Touch {
        Touch { size: core::mem::size_of::<Touch>() as u32, phase, device, finger, x: at[0], y: at[1], time, pressure, reserved: 0 }
    }
}

/// One input event: 64 bytes, size-prefixed, one kind per event (`FILE_DROPPED` 11 is gone and stays reserved).
///
/// `surface` is the surface the event is for and must be the surface of the frame that carries it (`FrameIn::surface`); `new` and
/// `default` name `FrameIn::MAIN`. `text` and the touch payload are borrowed for the frame call only: build events into
/// `FrameIn::events(&[Event])`, which keeps the borrow. TEXT_EDITING carries its start and length in `x` and `y`; FILE_DIALOG carries
/// its id in `value`.
/// shortcut: the FILE_DIALOG id is an f64 (exact up to 2^53), upgrade with an integer id slot when a host numbers dialogs past that.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Event {
    /// `size_of::<Event>()` (64); a longer record is read as its first 64 bytes, a shorter one refused. `new` and `default` set it.
    pub size: u32,
    /// Discriminant that determines the meaning of the remaining fields.
    pub kind: u32,
    /// Nonzero id of the surface this record addresses.
    pub surface: u64,
    /// Code interpreted according to the event kind.
    pub code: u32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u32,
    /// Horizontal coordinate in logical points.
    pub x: f64,
    /// Vertical coordinate in logical points.
    pub y: f64,
    /// Scalar payload interpreted according to `kind`.
    pub value: f64,
    /// Borrowed event payload, interpreted according to `kind`.
    pub text: Str,
}
impl Default for Event { fn default() -> Self { Event::new(0) } }

impl Event {
    /// Builds an event that carries no borrowed payload.
    ///
    /// Supply a scalar-only event kind. The frame validates the kind and its remaining fields.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Event;
    /// const LOST: Event = Event::new(Event::FOCUS_LOST);
    /// ```
    pub const fn new(kind: u32) -> Event {
        Event { size: core::mem::size_of::<Event>() as u32, kind, surface: FrameIn::MAIN, code: 0, reserved: 0, x: 0.0, y: 0.0, value: 0.0, text: Str { ptr: core::ptr::null(), len: 0 } }
    }

    /// Builds a key edge with a raw key code.
    ///
    /// Use KEY_PRESSED or KEY_RELEASED. The host preserves the code as supplied.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Event;
    /// let _ = Event::key(Event::KEY_PRESSED, u32::from('x'));
    /// ```
    pub const fn key(kind: u32, code: u32) -> Event { Event { code, ..Event::new(kind) } }

    /// Builds a pointer event with its position and button code.
    ///
    /// Position is in logical surface points. A motion event may use code zero.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Event;
    /// const MOVE: Event = Event::pointer(Event::POINTER_MOVED, 0, 10.0, 20.0);
    /// ```
    pub const fn pointer(kind: u32, code: u32, x: f64, y: f64) -> Event { Event { code, x, y, ..Event::new(kind) } }

    /// Key down, with the key code in `code`.
    pub const KEY_PRESSED: u32 = 1;
    /// Key up, with the key code in `code`.
    pub const KEY_RELEASED: u32 = 2;
    /// Pointer movement to the logical position in `x` and `y`.
    pub const POINTER_MOVED: u32 = 3;
    /// Pointer button down at the given logical position.
    pub const POINTER_PRESSED: u32 = 4;
    /// Pointer button up at the given logical position.
    pub const POINTER_RELEASED: u32 = 5;
    /// Cancels the active pointer interaction.
    pub const POINTER_CANCELLED: u32 = 6;
    /// Wheel movement with horizontal and vertical scroll deltas.
    pub const SCROLLED: u32 = 7;
    /// Committed UTF-8 text input.
    pub const TEXT_INPUT: u32 = 8;
    /// Text obtained from the native clipboard.
    pub const PASTE: u32 = 9;
    /// Input method preedit text with selection in `x` and `y`.
    pub const TEXT_EDITING: u32 = 10;
    // 11 (FILE_DROPPED, one path) is reserved: FILES_DROPPED carries any number of paths.
    /// File drag movement over the surface.
    pub const FILE_DRAG_MOVED: u32 = 12;
    /// Ends an external file drag.
    pub const FILE_DRAG_ENDED: u32 = 13;
    /// Pinch gesture update.
    pub const PINCHED: u32 = 14;
    /// File chooser result carrying its dialog id in `value`.
    pub const FILE_DIALOG: u32 = 15;
    /// Native dialog opened.
    pub const DIALOG_OPENED: u32 = 16;
    /// Logical window dimensions changed.
    pub const RESIZED: u32 = 17;
    /// Surface lost focus and held keys are cleared.
    pub const FOCUS_LOST: u32 = 18;
    /// Surface gained focus.
    pub const FOCUS_GAINED: u32 = 19;
    /// Surface requested close.
    pub const CLOSED: u32 = 20;
    /// Drawable dimensions changed.
    pub const PIXEL_SIZE_CHANGED: u32 = 21;
    /// Surface visibility changed.
    pub const VISIBILITY_CHANGED: u32 = 22;
    /// Trackpad-originated scroll update.
    pub const TRACKPAD_SCROLLED: u32 = 23;
    /// Native environment or keyboard geometry changed.
    pub const ENVIRONMENT_CHANGED: u32 = 24;
    /// Borrowed sized `Touch` record in `text`.
    pub const TOUCH: u32 = 25;
    /// External drop carrying one or more file paths.
    pub const FILES_DROPPED: u32 = 26;
}

/// Host geometry in logical window points. Rectangles are [x, y, width, height].
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Environment {
    /// `size_of::<Environment>()` (80): `new` and `default` set it, and the frame call writes it itself.
    pub size: u32,
    /// UIKit's raw animation curve, including platform values beyond 0..3.
    pub keyboard_curve: u32,
    /// 0 or 1; set after native did-show, cleared after native did-hide.
    pub keyboard_visible: u32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: u32,
    /// Insets in top, right, bottom, left order; backgrounds may draw behind them.
    pub safe_area: [f32; 4],
    /// Keyboard's animation target and starting frame, including offscreen hide targets.
    pub keyboard_frame: [f32; 4],
    /// Keyboard rectangle at the start of the native animation.
    pub keyboard_from: [f32; 4],
    /// Start time uses FrameIn::time's clock. Duration and curve come from the host.
    pub keyboard_started: f64,
    /// Duration of the native keyboard animation in seconds.
    pub keyboard_duration: f64,
}
impl Default for Environment { fn default() -> Self { Environment::new([0.0; 4]) } }

impl Environment {
    /// Builds the environment with the surface's safe-area insets.
    ///
    /// Insets are in logical points, top, right, bottom, left. No keyboard geometry is set.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const ENV: pxui_types::Environment = pxui_types::Environment::new([24.0, 0.0, 0.0, 0.0]);
    /// ```
    pub const fn new(safe_area: [f32; 4]) -> Environment {
        Environment { size: core::mem::size_of::<Environment>() as u32, keyboard_curve: 0, keyboard_visible: 0, reserved: 0, safe_area,
            keyboard_frame: [0.0; 4], keyboard_from: [0.0; 4], keyboard_started: 0.0, keyboard_duration: 0.0 }
    }

    /// Adds the keyboard's animation: its target and starting frames, start time (`FrameIn::time`'s clock), duration, UIKit curve and
    /// whether it is shown.
    ///
    /// Frames are [x, y, width, height] in logical points; the frame call validates them.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let env = pxui_types::Environment::new([0.0; 4]).keyboard_animation([0.0, 500.0, 390.0, 344.0], [0.0, 844.0, 390.0, 344.0], 1.0, 0.25, 7, true);
    /// assert_eq!(env.keyboard_visible, 1);
    /// ```
    pub const fn keyboard_animation(mut self, frame: [f32; 4], from: [f32; 4], started: f64, duration: f64, curve: u32, visible: bool) -> Environment {
        self.keyboard_frame = frame;
        self.keyboard_from = from;
        self.keyboard_started = started;
        self.keyboard_duration = duration;
        self.keyboard_curve = curve;
        self.keyboard_visible = visible as u32;
        self
    }
}

/// Native presentation geometry, in logical window points; does not change Environment's frozen layout.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyboard {
    /// `size_of::<Keyboard>()` (24): `new` and `default` set it, and the frame call writes it itself.
    pub size: u32,
    /// Bit flags defined for this record.
    pub flags: u32,
    /// Keyboard bounds in logical window points.
    pub frame: [f32; 4],
}
impl Default for Keyboard { fn default() -> Self { Keyboard::new(geom::Rect::new(0.0, 0.0, 0.0, 0.0), 0) } }
impl Keyboard {
    /// Builds native keyboard presentation geometry.
    ///
    /// The frame is in logical surface points. The frame validates the validity and animation flags.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Rect, Keyboard};
    /// assert!(Keyboard::new(Rect::new(0.0, 0.0, 320.0, 200.0), Keyboard::VALID).valid());
    /// ```
    pub const fn new(frame: geom::Rect, flags: u32) -> Keyboard {
        Keyboard { size: core::mem::size_of::<Keyboard>() as u32, flags, frame: [frame.x, frame.y, frame.w, frame.h] }
    }

    /// Bit indicating the keyboard rectangle is available.
    pub const VALID: u32 = 1;
    /// Bit indicating the native keyboard is in motion.
    pub const ANIMATING: u32 = 2;
    /// Whether the geometry is valid: a whole record (`size`), flags 0..=3 without ANIMATING alone, finite non-negative frame.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Keyboard;
    /// assert!(Keyboard::default().valid() && !Keyboard { flags: Keyboard::ANIMATING, ..Keyboard::default() }.valid());
    /// ```
    pub fn valid(&self) -> bool {
        self.size as usize >= core::mem::size_of::<Keyboard>() && self.flags <= 3 && self.flags != Self::ANIMATING
            && self.frame.iter().all(|v| v.is_finite()) && self.frame[2] >= 0.0 && self.frame[3] >= 0.0
            && (self.frame[0]+self.frame[2]).is_finite() && (self.frame[1]+self.frame[3]).is_finite()
    }
}

/// Input for one UI frame in logical points.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FrameIn {
    /// The byte size of the ABI record supplied by the host.
    pub size: u32,
    /// The frame's events, at most 4096: `events.stride` (not below the event size) steps, each event's own `size` says what it holds. The records and
    /// the payloads they point at must live through the frame call.
    pub events: ffi::Strided<Event>,
    /// The logical width of the surface in points.
    pub width: f32,
    /// The logical height of the surface in points.
    pub height: f32,
    /// Drawable pixels per logical point.
    pub scale: f32,
    /// Time in seconds on the host's monotonic clock.
    pub time: f64,
    /// Host appearance: UNKNOWN, LIGHT or DARK. Added after the frozen 48-byte v1 prefix.
    pub appearance: u32,
    /// 0 or 1; the OS preference overrides themes that permit motion.
    pub reduced_motion: u32,
    /// ABI 1.24 tail; absent tails use the default environment.
    pub environment: Environment,
    /// ABI 1.26 tail: current native keyboard geometry, including intermediate animation frames.
    pub keyboard: Keyboard,
    /// The surface this frame is for: registered on the Ui (`Ui::add_surface`), else the frame is refused; every event carries it.
    pub surface: u64,
}
impl Default for FrameIn {
    fn default() -> Self { Self::new(0.0, 0.0) }
}
impl FrameIn {
    /// The surface id `Ui::new` registers and `FrameIn::new` names; an app with another surface registers its own with `Ui::add_surface`.
    pub const MAIN: u64 = 1;
    /// The wire value for unknown appearance.
    pub const APPEARANCE_UNKNOWN:u32=0;
    /// The wire value for light appearance.
    pub const APPEARANCE_LIGHT:u32=1;
    /// The wire value for dark appearance.
    pub const APPEARANCE_DARK:u32=2;

    /// Creates an empty frame for a surface of `width` by `height` logical points.
    ///
    /// Starts at time zero and scale one with no events, insets, or keyboard.
    /// Allocates: never.
    /// Panics: never. The frame call validates its dimensions and settings.
    ///
    /// ```
    /// let input = pxui_types::FrameIn::new(320.0, 240.0);
    /// assert_eq!((input.width, input.height, input.scale), (320.0, 240.0, 1.0));
    /// ```
    pub const fn new(width: f32, height: f32) -> FrameIn {
        FrameIn {
            size: core::mem::size_of::<FrameIn>() as u32, events: ffi::Strided { ptr: core::ptr::null(), len: 0, stride: core::mem::size_of::<Event>() },
            width, height, scale: 1.0, time: 0.0, appearance: Self::APPEARANCE_UNKNOWN, reduced_motion: 0,
            environment: Environment::new([0.0; 4]),
            keyboard: Keyboard::new(geom::Rect::new(0.0, 0.0, 0.0, 0.0), 0), surface: Self::MAIN,
        }
    }

}

/// The native appearance reported with a frame.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Appearance {
    /// The host did not report an appearance.
    Unknown = FrameIn::APPEARANCE_UNKNOWN,
    /// The host reports light appearance.
    Light = FrameIn::APPEARANCE_LIGHT,
    /// The host reports dark appearance.
    Dark = FrameIn::APPEARANCE_DARK,
}

/// Frame result holding damage information and borrowed layers.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameOut {
    /// Size of this record in bytes.
    pub size: u32,
    /// Region of the frame that changed.
    pub damage: [f32; 4],
    /// Nonzero when `damage` is present.
    pub has_damage: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 3],
    /// Number of layers in the range.
    pub layer_count: u32,
    /// Borrowed layer records produced by the frame.
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
    /// Size of this record in bytes.
    pub size: u32,
    /// Native GPU backend code.
    pub backend: u32,
    /// Vulkan instance used to create the device.
    pub instance: *mut core::ffi::c_void,
    /// Vulkan physical device backing the renderer.
    pub physical_device: *mut core::ffi::c_void,
    /// Vulkan logical device.
    pub device: *mut core::ffi::c_void,
    /// Vulkan graphics queue.
    pub queue: *mut core::ffi::c_void,
    /// Vulkan graphics queue family index.
    pub queue_family: u32,
    /// Vulkan API version selected by the host.
    pub api_version: u32,
    /// Feature bits available to this operation.
    pub features: u64,
    /// Vulkan entry point used to look up instance and device functions.
    pub get_instance_proc_addr: *mut core::ffi::c_void,
    /// Optional host Vulkan pipeline cache.
    pub pipeline_cache: *mut core::ffi::c_void,
    /// Host Metal device.
    pub mtl_device: *mut core::ffi::c_void,
    /// Optional Wayland display associated with the target.
    pub wl_display: *mut core::ffi::c_void,
    /// Optional Wayland surface associated with the target.
    pub wl_surface: *mut core::ffi::c_void,
    /// Optional native Metal layer associated with the target.
    pub metal_layer: *mut core::ffi::c_void,
    /// Optional native Windows window handle.
    pub hwnd: *mut core::ffi::c_void,
    /// Opaque token for the host-owned device lease.
    pub owner_token: *mut core::ffi::c_void,
    /// Callback invoked when the device lease ends.
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
    /// Size of this record in bytes.
    pub size: u32,
    /// Vulkan command buffer into which the library records work.
    pub vk_command_buffer: *mut core::ffi::c_void,
    /// Metal command buffer into which the library records work.
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
    /// Size of this record in bytes.
    pub size: u32,
    /// Vulkan command buffer into which the library records work.
    pub vk_command_buffer: *mut core::ffi::c_void,
    /// Metal render encoder for the target pass.
    pub mtl_render_encoder: *mut core::ffi::c_void,
    /// Drawable width in physical pixels.
    pub width_px: u32,
    /// Drawable height in physical pixels.
    pub height_px: u32,
    /// Drawable pixels per logical point.
    pub scale: f32,
    /// Metal command buffer into which the library records work.
    pub mtl_command_buffer: *mut core::ffi::c_void,
    /// Index of the first layer in the requested range.
    pub first_layer: u32,
    /// Number of layers in the range.
    pub layer_count: u32,
    /// Callback to end and reopen the pass around a backdrop capture.
    pub split: Option<PassSplit>,
    /// User pointer forwarded unchanged to the pass split callback.
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
    /// Size of this record in bytes.
    pub size: u32,
    /// Vulkan image view sampled by the renderer.
    pub vk_image_view: *mut core::ffi::c_void,
    /// Metal texture sampled by the renderer.
    pub mtl_texture: *mut core::ffi::c_void,
    /// Native texture width in pixels.
    pub width: u32,
    /// Native texture height in pixels.
    pub height: u32,
    /// Pixel format code.
    pub format: u32,
}
impl Default for Image { fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, vk_image_view: core::ptr::null_mut(), mtl_texture: core::ptr::null_mut(), width: 0, height: 0, format: 0 } } }

impl Image {
    /// Describes a host-owned native RGBA8 GPU texture.
    ///
    /// A const initializer of this descriptor's fields: it copies, uploads and retains nothing. Leave the handle for the unused backend
    /// null. The host owns the native view or texture and keeps it valid for every GPU use of a frame that drew it; pxui never
    /// destroys it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Image;
    /// let image = Image::rgba8(32, 24, core::ptr::null_mut(), core::ptr::null_mut());
    /// assert!(!image.valid());
    /// ```
    pub const fn rgba8(width: u32, height: u32, vk_image_view: *mut core::ffi::c_void, mtl_texture: *mut core::ffi::c_void) -> Image {
        Image { size: core::mem::size_of::<Image>() as u32, vk_image_view, mtl_texture, width, height, format: Image::FORMAT_RGBA8 }
    }

    /// Texture format code for `R8`.
    pub const FORMAT_R8: u32 = 0;
    /// Texture format code for `RGBA8`.
    pub const FORMAT_RGBA8: u32 = 1;
    /// Texture format code for `BGRA8`.
    pub const FORMAT_BGRA8: u32 = 2;
    /// Texture format code for `BGRA8_SRGB`.
    pub const FORMAT_BGRA8_SRGB: u32 = 3;
    /// Texture format code for `RGBA16F`.
    pub const FORMAT_RGBA16F: u32 = 4;
    /// Texture format code for `RGBA32F`.
    pub const FORMAT_RGBA32F: u32 = 5;
    /// Texture format code for `DEPTH32F`.
    pub const FORMAT_DEPTH32F: u32 = 6;

    /// Whether PXUI can sample this descriptor: a `size` covering this record, a nonzero width and height, a native handle
    /// (`vk_image_view` or `mtl_texture`) and a colour format (below `FORMAT_DEPTH32F`).
    ///
    /// The one rule the library and the wrapper both apply; it does not check that the handle is alive.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::Image;
    /// assert!(!Image::default().valid());
    /// assert!({ let mut record = Image::default(); record.mtl_texture = std::ptr::dangling_mut(); record.width = 8; record.height = 4; record.format = Image::FORMAT_RGBA8; record }.valid());
    /// ```
    pub fn valid(&self) -> bool {
        self.size >= core::mem::size_of::<Image>() as u32 && self.width != 0 && self.height != 0
            && (!self.vk_image_view.is_null() || !self.mtl_texture.is_null()) && self.format < Image::FORMAT_DEPTH32F
    }
}

/// Opaque box handle checked against its owning UI.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PxuiBox {
    /// Index within the owning collection.
    pub index: u64,
    /// Stable key carried by the box handle.
    pub box_key: i64,
    /// Handle slot used to look up the retained box.
    pub box_slot: u64,
    /// Generation checked before using the handle.
    pub generation: u32,
    /// Serial of the `Ui` that made the handle (nonzero): every call compares it with its own `Ui`'s and refuses another's, or a zeroed
    /// handle (`PxuiBox::default()`), as stale. Fills the padding: still 32 bytes.
    pub ui: u32,
}

/// Panel descriptor with logical bounds and appearance.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiPanel {
    /// Size of this record in bytes.
    pub size: u32,
    /// Horizontal coordinate in logical points.
    pub x: f32,
    /// Vertical coordinate in logical points.
    pub y: f32,
    /// Width in logical points.
    pub width: f32,
    /// Height in logical points.
    pub height: f32,
    /// The inner padding in points, fractions kept, in the 1.x record's alignment hole (offsets unchanged). It counts only while it
    /// agrees with `padding` (within half a point, as the wrapper writes them), so a 1.x caller's record keeps its integer meaning.
    #[doc(hidden)]
    pub padding_points: f32,
    /// Padding in points, fractions kept; read only while it agrees with padding within half a point.
    pub padding: i64,
    /// Nonzero when `height` is present.
    pub has_height: u8,
    /// Whether to use window appearance and behavior.
    pub window: u8,
    /// Nonzero when `max_height` is present.
    pub has_max_height: u8,
    /// Nonzero when `stroke` is present.
    pub has_stroke: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 4],
    /// Maximum height in logical points.
    pub max_height: f32,
    /// Appearance of the outline.
    pub stroke: geom::Rgba,
    /// The look in place of the theme's, read where it lies for the call (ABI 1.132: a pointer, the record no longer carries the 152-byte
    /// look); null for the theme's.
    pub look: *const ffi::PxuiLookDesc,
}
impl Default for PxuiPanel {
    fn default() -> Self {
        Self { size: core::mem::size_of::<Self>() as u32, x: 0.0, y: 0.0, width: 0.0, height: 0.0, padding_points: 0.0, padding: 0, has_height: 0, window: 0,
            has_max_height: 0, has_stroke: 0, reserved: [0; 4], max_height: 0.0, stroke: geom::Rgba::default(), look: core::ptr::null() }
    }
}

/// Native renderer target and atlas dimensions.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiTargetDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Pixel format code.
    pub format: u32,
    /// Render target sample count.
    pub samples: u8,
    /// Reserved for the ABI. Write zero on input.
    pub reserved: [u8; 3],
    /// Width of a mask atlas page in pixels.
    pub mask_width: u32,
    /// Height of a mask atlas page in pixels.
    pub mask_height: u32,
    /// Width of a colour atlas page in pixels.
    pub color_width: u32,
    /// Height of a colour atlas page in pixels.
    pub color_height: u32,
    /// Reserved for the ABI. Write zero on input.
    pub reserved_tail: u32,
}
impl Default for PxuiTargetDesc { fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, format: 0, samples: 1, reserved: [0; 3], mask_width: 0, mask_height: 0, color_width: 0, color_height: 0, reserved_tail: 0 } } }

/// Options for creating a UI and declaring the ABI it was built against.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PxuiUiDesc {
    /// Size of this record in bytes.
    pub size: u32,
    /// Platform code used to choose keyboard and input conventions.
    pub platform: u32,
    /// Default font size in logical points.
    pub font_size: i32,
    /// ABI 1.60 tail: the version (`major << 16 | minor`) the caller was built against; only its major is checked (a newer minor may run on an older
    /// library and ask `pxui_supports`); 0 (a 12-byte record from before 1.60) means "unstated".
    pub abi_built: u32,
}
/// The ids `pxui_supports` answers for (`PXUI_FEATURE_*` in `pxui.h`): the parts of the 2.0 interface a build may lack. Everything else in the
/// header is in every build of its major version and is not asked about. Append only: an id is never reused or renumbered, 0 is never an id, and
/// a library answers 0 for an id it does not know (a feature of a later minor, say). No feature is named for a version.
pub mod feature {
    /// The accessibility bridge of a window the app owns (`pxui_a11y_*`): present on Linux (AT-SPI2); elsewhere the platform's own bridge is the host's.
    pub const A11Y_BRIDGE: u32 = 1;
    /// The Metal renderer backend. Answers from the target today (Apple); must switch to `cfg!(feature = ..)` if a backend becomes optional.
    pub const METAL: u32 = 2;
    /// The Vulkan renderer backend. Answers from the target today (Windows, Linux); must switch to `cfg!(feature = ..)` if a backend becomes optional.
    pub const VULKAN: u32 = 3;
    /// The extension set (`abi/pxui.ext.txt`: the Rays vocabulary and the diagnostics): exported, with no stability promise.
    pub const EXTENSION_SET: u32 = 4;
}

/// The ABI version this crate's records describe (`major << 16 | minor`); an app states it as `PxuiUiDesc::abi_built`.
pub const ABI_BUILT: u32 = 2 << 16;
impl Default for PxuiUiDesc { fn default() -> Self { Self { size: core::mem::size_of::<Self>() as u32, platform: 0, font_size: 0, abi_built: ABI_BUILT } } }

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
            ("has_max_height", core::mem::offset_of!(PxuiPanel, has_max_height)), ("has_stroke", core::mem::offset_of!(PxuiPanel, has_stroke)), ("reserved", core::mem::offset_of!(PxuiPanel, reserved)),
            ("max_height", core::mem::offset_of!(PxuiPanel, max_height)), ("stroke", core::mem::offset_of!(PxuiPanel, stroke)), ("look", core::mem::offset_of!(PxuiPanel, look)),
        ]);
        assert_layout("PxuiTargetDesc", size_of::<PxuiTargetDesc>(), align_of::<PxuiTargetDesc>(), &[
            ("size", core::mem::offset_of!(PxuiTargetDesc, size)), ("format", core::mem::offset_of!(PxuiTargetDesc, format)), ("samples", core::mem::offset_of!(PxuiTargetDesc, samples)),
            ("reserved", core::mem::offset_of!(PxuiTargetDesc, reserved)), ("mask_width", core::mem::offset_of!(PxuiTargetDesc, mask_width)), ("mask_height", core::mem::offset_of!(PxuiTargetDesc, mask_height)),
            ("color_width", core::mem::offset_of!(PxuiTargetDesc, color_width)), ("color_height", core::mem::offset_of!(PxuiTargetDesc, color_height)), ("reserved_tail", core::mem::offset_of!(PxuiTargetDesc, reserved_tail)),
        ]);
        assert_layout("PxuiUiDesc", core::mem::offset_of!(PxuiUiDesc, abi_built), align_of::<PxuiUiDesc>(), &[
            ("size", core::mem::offset_of!(PxuiUiDesc, size)), ("platform", core::mem::offset_of!(PxuiUiDesc, platform)), ("font_size", core::mem::offset_of!(PxuiUiDesc, font_size)),
        ]);
        assert_layout("PxuiUiDescV1_60", size_of::<PxuiUiDesc>(), align_of::<PxuiUiDesc>(), &[
            ("size", core::mem::offset_of!(PxuiUiDesc, size)), ("platform", core::mem::offset_of!(PxuiUiDesc, platform)), ("font_size", core::mem::offset_of!(PxuiUiDesc, font_size)), ("abi_built", core::mem::offset_of!(PxuiUiDesc, abi_built)),
        ]);
        assert_layout("TextStyle", size_of::<crate::ffi::PxuiTextStyle>(), align_of::<crate::ffi::PxuiTextStyle>(), &[
            ("size", core::mem::offset_of!(crate::ffi::PxuiTextStyle, size)), ("font", core::mem::offset_of!(crate::ffi::PxuiTextStyle, font)), ("weight", core::mem::offset_of!(crate::ffi::PxuiTextStyle, weight)),
            ("size_points", core::mem::offset_of!(crate::ffi::PxuiTextStyle, size_points)), ("line_height", core::mem::offset_of!(crate::ffi::PxuiTextStyle, line_height)), ("wrap", core::mem::offset_of!(crate::ffi::PxuiTextStyle, wrap)),
            ("align", core::mem::offset_of!(crate::ffi::PxuiTextStyle, align)), ("max_lines", core::mem::offset_of!(crate::ffi::PxuiTextStyle, max_lines)), ("italic", core::mem::offset_of!(crate::ffi::PxuiTextStyle, italic)),
            ("ellipsis", core::mem::offset_of!(crate::ffi::PxuiTextStyle, ellipsis)), ("features", core::mem::offset_of!(crate::ffi::PxuiTextStyle, features)), ("tracking", core::mem::offset_of!(crate::ffi::PxuiTextStyle, tracking)),
        ]);
        assert_layout("TextSpan",size_of::<TextSpan>(),align_of::<TextSpan>(),&[
            ("start", core::mem::offset_of!(TextSpan, start)),("end", core::mem::offset_of!(TextSpan, end)),("weight", core::mem::offset_of!(TextSpan, weight)),("flags", core::mem::offset_of!(TextSpan, flags)),("color", core::mem::offset_of!(TextSpan, color)),("background", core::mem::offset_of!(TextSpan, background)),("underline", core::mem::offset_of!(TextSpan, underline)),
        ]);
        assert_layout("TextSpans",size_of::<TextSpans>(),align_of::<TextSpans>(),&[("size", core::mem::offset_of!(TextSpans, size)),("reserved", core::mem::offset_of!(TextSpans, reserved)),("spans", core::mem::offset_of!(TextSpans, spans)),("spans.len", core::mem::offset_of!(TextSpans, spans.len)),("spans.stride", core::mem::offset_of!(TextSpans, spans.stride))]);
        assert_layout("TextMetrics", size_of::<TextMetrics>(), align_of::<TextMetrics>(), &[
            ("size", core::mem::offset_of!(TextMetrics, size)),
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
        // the fractional size sits in PxuiFont's tail padding: the 1.x size and offsets above are unchanged
        assert_eq!((size_of::<Font>(), core::mem::offset_of!(Font, size_points)), (32, 28));
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
        // the bodies follow `scrollbar_gap`, then the kit parts back to back in `theme_widgets!` order: the growing part is last
        assert_layout("PxuiThemeSnapshot", size_of::<ThemeSnapshot>(), align_of::<ThemeSnapshot>(), &[
            ("size", core::mem::offset_of!(ThemeSnapshot, size)), ("part_count", core::mem::offset_of!(ThemeSnapshot, part_count)), ("tokens", core::mem::offset_of!(ThemeSnapshot, tokens)), ("row_h", core::mem::offset_of!(ThemeSnapshot, row_h)), ("control_h", core::mem::offset_of!(ThemeSnapshot, control_h)), ("ring", core::mem::offset_of!(ThemeSnapshot, ring)),
            ("scrollbar_w", core::mem::offset_of!(ThemeSnapshot, scrollbar_w)), ("scrollbar_gap", core::mem::offset_of!(ThemeSnapshot, scrollbar_gap)), ("body", core::mem::offset_of!(ThemeSnapshot, body)), ("panel", core::mem::offset_of!(ThemeSnapshot, panel)),
        ]);
        assert_layout("PxuiBodies", size_of::<theme::Bodies>(), align_of::<theme::Bodies>(), &[("size", core::mem::offset_of!(theme::Bodies, size)), ("toggle", core::mem::offset_of!(theme::Bodies, toggle)),
            ("section_index", core::mem::offset_of!(theme::Bodies, section_index)), ("reserved", core::mem::offset_of!(theme::Bodies, reserved))]);
        assert_eq!(core::mem::offset_of!(ThemeSnapshot, panel), core::mem::offset_of!(ThemeSnapshot, body) + size_of::<theme::Bodies>(), "the parts follow the bodies");
        assert_eq!(size_of::<ThemeSnapshot>(), core::mem::offset_of!(ThemeSnapshot, panel) + WIDGET_COUNT * size_of::<States>(), "the parts are an array of States, last");
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
        assert_layout("FrameIn", size_of::<FrameIn>(), align_of::<FrameIn>(), &[
            ("size", core::mem::offset_of!(FrameIn, size)), ("events", core::mem::offset_of!(FrameIn, events)), ("events.len", core::mem::offset_of!(FrameIn, events.len)),
            ("events.stride", core::mem::offset_of!(FrameIn, events.stride)), ("width", core::mem::offset_of!(FrameIn, width)), ("height", core::mem::offset_of!(FrameIn, height)),
            ("scale", core::mem::offset_of!(FrameIn, scale)), ("time", core::mem::offset_of!(FrameIn, time)), ("appearance", core::mem::offset_of!(FrameIn, appearance)),
            ("reduced_motion", core::mem::offset_of!(FrameIn, reduced_motion)), ("environment", core::mem::offset_of!(FrameIn, environment)),
            ("keyboard", core::mem::offset_of!(FrameIn, keyboard)), ("surface", core::mem::offset_of!(FrameIn, surface)),
        ]);
        assert_layout("Touch",size_of::<Touch>(),align_of::<Touch>(),&[("device", core::mem::offset_of!(Touch, device)),("finger", core::mem::offset_of!(Touch, finger)),("x", core::mem::offset_of!(Touch, x)),("y", core::mem::offset_of!(Touch, y)),("time", core::mem::offset_of!(Touch, time)),("pressure", core::mem::offset_of!(Touch, pressure)),("phase", core::mem::offset_of!(Touch, phase))]);
        assert_layout("Environment",size_of::<Environment>(),align_of::<Environment>(),&[
            ("safe_area", core::mem::offset_of!(Environment, safe_area)),("keyboard_frame", core::mem::offset_of!(Environment, keyboard_frame)),("keyboard_from", core::mem::offset_of!(Environment, keyboard_from)),
            ("keyboard_started", core::mem::offset_of!(Environment, keyboard_started)),("keyboard_duration", core::mem::offset_of!(Environment, keyboard_duration)),("keyboard_curve", core::mem::offset_of!(Environment, keyboard_curve)),("keyboard_visible", core::mem::offset_of!(Environment, keyboard_visible)),
        ]);
        assert_layout("Keyboard",size_of::<Keyboard>(),align_of::<Keyboard>(),&[("frame", core::mem::offset_of!(Keyboard, frame)),("flags", core::mem::offset_of!(Keyboard, flags))]);
        assert_layout("TextInputOptions",size_of::<PxuiTextInputOptions>(),align_of::<PxuiTextInputOptions>(),&[("size", core::mem::offset_of!(PxuiTextInputOptions, size)),("autocorrect", core::mem::offset_of!(PxuiTextInputOptions, autocorrect)),("capitalization", core::mem::offset_of!(PxuiTextInputOptions, capitalization)),("action", core::mem::offset_of!(PxuiTextInputOptions, action))]);
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

    const INK: Rgba = Rgba::rgb(10, 20, 30);
    const FONT: Font = Font::BODY.size(14.0).weight(700);
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
        same(&NODE, &Node::DEFAULT.axis(Axis::Row).w(Size::Px(44.0)).h(Size::Px(30.0)).grow(0.0).gap(8.0).padding(Edges::new(4.0, 6.0, 4.0, 6.0)).justify(Justify::Center).align(Align::End).role(Role::Button).flags(Flags(5)));
        same(&COLUMN, &Node::DEFAULT.axis(Axis::Column));
        same(&STACK, &Node::DEFAULT.axis(Axis::Stack).w(Size::Fit).h(Size::Grow));
        assert_eq!(Edges::xy(1.0, 2.0), Edges::new(1.0, 2.0, 1.0, 2.0));
        assert_eq!(FONT, { let mut record = Font::BODY.weight(700); record.size = 14; record.size_points = 14.0; record });
        assert_eq!(LOOK, Look::NONE.fill(INK).radii([6.0; 4]).border(INK, 1.0).text(INK).font({ let mut record = Font::BODY.weight(700); record.size = 14; record.size_points = 14.0; record }));
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
    fn every_tail_length_hashes_as_the_zero_padded_word() {
        let text = b"abcdefghijklmnop\x80\xff";
        for len in 0..=text.len() {
            let bytes = &text[..len];
            let mut expect = hash::mix(0x5eed, len as i64) as u64;
            for chunk in bytes.chunks(8) { let mut word = [0u8; 8]; word[..chunk.len()].copy_from_slice(chunk); expect = hash::mix(expect as i64, u64::from_le_bytes(word) as i64) as u64; }
            assert_eq!(hash::bytes(0x5eed, bytes), expect, "{len} bytes");
        }
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
