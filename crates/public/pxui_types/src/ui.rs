//! Plain UI data and pure functions shared by the engine and the wrapper: box flags, keys, option structs,
//! small results and text helpers. One definition each; `pxui_core` and `pxui` re-export them.
use crate::geom::{hash, Rect, Rgba as Color};
use crate::node::Axis;
use crate::theme::Look;
use crate::{Flags, Size};
use std::{borrow::Cow, rc::Rc};

pub const NO_FLAGS: Flags = 0;
pub const CLICKABLE: Flags = 1;
pub const FOCUSABLE: Flags = 2;
pub const SCROLL: Flags = 4;
pub const CLIP: Flags = 8;
pub const BLOCKING: Flags = 16;
/// Kit controls enter keyboard mode through Tab; pointer editing retains the existing host
/// shortcut behavior. Text-entry rows also focus on a press.
pub const TAB_ONLY: Flags = 32;
/// Keyboard traversal focus for a control whose pointer interaction leaves keyboard input with its
/// host. Escape relinquishes this keyboard focus.
pub const TAB_STOP: Flags = FOCUSABLE | TAB_ONLY;
/// a multiline editor keeps Tab for itself (two spaces) instead of moving the focus
pub const KEEP_TAB: Flags = 64;
/// a press on such a box leaves the keyboard focus where it is (a completion row under an editor)
pub const KEEP_FOCUS: Flags = 128;
/// a scrolling box whose thumb is drawn over its content (an inspector body): no gutter
pub const OVER_THUMB: Flags = 4096;
/// A press whose topmost hit box is not clickable is captured by the nearest ancestor with this
/// flag.
pub const CAPTURE_UNCLAIMED: Flags = 8192;
/// the control marks its own keyboard focus: the engine strokes no ring round it
pub const FOCUS_MARK: Flags = 256;
pub const TAB_STOP_MARKED: Flags = TAB_STOP | FOCUS_MARK;
/// The node's text is cut with an ellipsis to the room its look leaves (the placed width less the
/// text inset) when it does not fit.
pub const FIT_TEXT: Flags = 16384;
/// scrolls sideways (the wheel's and the trackpad's horizontal motion); with `SCROLL` on both axes
pub const SCROLL_X: Flags = 32768;
/// a flow child of a scroll box that stops at its top edge instead of scrolling out (CSS sticky);
/// it paints over its siblings
pub const STICKY: Flags = 65536;
/// with `CLIP`: the children's clip takes the corner radius of the node's look (CSS `overflow:
/// hidden` with `border-radius`); without it a clip is square
pub const ROUND_CLIP: Flags = 131072;
/// Nothing the node's view builds follows its hover: the pointer moving onto or off it dirties no
/// view (the app reads `hovered_within` between frames and patches what shows it).
pub const HOVER_FREE: Flags = 262144;
/// The node's square look paints its fill and border instances even when they are clear, so a
/// patch to a look that shows them rewrites its range in place (it never grows the layer).
pub const KEEP_ROOM: Flags = 524288;
/// Nothing the node's view builds follows a drag of it: while the pointer holds it and only moves
/// (no press, release or key), its view is not dirtied. The owner reads the drag between builds
/// (`signal` on the kept handle; `field_drag` for a `value_field`) and patches what shows it.
pub const DRAG_FREE: Flags = 1048576;

/// Points a pressed pointer travels before a press is a drag: a carry starts, a click ends.
pub const DRAG_THRESHOLD: f32 = 4.0;

/// What a carry holds: a `kind` and a `value`, both strings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Payload {
    pub kind: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Drop {
    /// the pointer is over the box while a payload is held
    Hover(Payload),
    /// the payload was released over the box, this frame only
    Dropped(Payload),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    HorizontalResize,
    VerticalResize,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndoKind {
    Undo,
    Redo,
}

pub type HitFn = Rc<dyn Fn(Rect) -> Rect>;

/// What `Ui::animate` moves a node to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Look(Look),
    Offset(f32, f32),
    Opacity(f32),
}

/// The result of a `picker` for a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    None,
    Pick(i64),
    Delete(i64),
    Submit,
    Back,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuResult {
    Open,
    Pick(i64),
    Dismiss,
}

/// One ranked suggestion of `Language::complete`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    /// the byte span `insert` replaces
    pub replace: (i64, i64),
    pub insert: String,
    /// the row
    pub label: String,
    /// the row's right column: a type, a category
    pub detail: String,
    /// one line under the rows while the row is selected
    pub doc: String,
}

/// What a character typed at the caret lands in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeContext {
    Code,
    String,
    Comment,
}

#[derive(Clone)]
pub struct BoxOpts {
    pub flags: Flags,
    pub w: Size,
    pub h: Size,
    pub max_h: f32,
    pub axis: Axis,
    pub padding: f32,
    pub at: Option<(f32, f32)>,
    pub scroll_step: f32,
    pub hit: Option<HitFn>,
    pub hit_rect: Option<Rect>,
}

impl Default for BoxOpts {
    fn default() -> BoxOpts {
        BoxOpts {
            flags: NO_FLAGS,
            w: Size::Grow,
            h: Size::Fit,
            max_h: f32::INFINITY,
            axis: Axis::Column,
            padding: 0.0,
            at: None,
            scroll_step: 24.0,
            hit: None,
            hit_rect: None,
        }
    }
}

impl BoxOpts {
    pub fn flags(mut self, flags: Flags) -> BoxOpts {
        self.flags = flags;
        self
    }
    pub fn w(mut self, w: Size) -> BoxOpts {
        self.w = w;
        self
    }
    pub fn h(mut self, h: Size) -> BoxOpts {
        self.h = h;
        self
    }
    pub fn max_h(mut self, max_h: f32) -> BoxOpts {
        self.max_h = max_h;
        self
    }
    pub fn axis(mut self, axis: Axis) -> BoxOpts {
        self.axis = axis;
        self
    }
    pub fn padding(mut self, padding: f32) -> BoxOpts {
        self.padding = padding;
        self
    }
    pub fn at(mut self, at: (f32, f32)) -> BoxOpts {
        self.at = Some(at);
        self
    }
    pub fn scroll_step(mut self, step: f32) -> BoxOpts {
        self.scroll_step = step;
        self
    }
    pub fn hit(mut self, hit: HitFn) -> BoxOpts {
        self.hit = Some(hit);
        self
    }
}

#[derive(Clone)]
pub struct PanelOpts {
    /// a floating sheet's hairline (the theme's popup look in this colour); `look` replaces both
    pub stroke: Option<Color>,
    pub look: Option<Look>,
    pub window: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: Option<f32>,
    pub max_height: Option<f32>,
    pub padding: i64,
}

impl Default for PanelOpts {
    fn default() -> PanelOpts {
        PanelOpts { stroke: None, look: None, window: false, x: 12.0, y: 12.0, width: 280.0, height: None, max_height: None, padding: 0 }
    }
}

impl PanelOpts {
    pub fn at(mut self, x: f32, y: f32) -> PanelOpts {
        self.x = x;
        self.y = y;
        self
    }
    pub fn width(mut self, width: f32) -> PanelOpts {
        self.width = width;
        self
    }
    pub fn height(mut self, height: f32) -> PanelOpts {
        self.height = Some(height);
        self
    }
    pub fn max_height(mut self, height: f32) -> PanelOpts {
        self.max_height = Some(height);
        self
    }
    pub fn padding(mut self, padding: i64) -> PanelOpts {
        self.padding = padding;
        self
    }
    pub fn window(mut self, window: bool) -> PanelOpts {
        self.window = window;
        self
    }
}

/// What a `line_field`'s keys asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineStep {
    None,
    Next,
    Previous,
    Close,
}


/// A text button of an inspector head: its label, its key in ink-3, whether it is on (the control
/// fill) and whether it can be pressed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadAction {
    pub caption: String,
    pub keycap: String,
    pub active: bool,
    pub usable: bool,
}

/// What a head reports for a frame: the name as edited, the index of the pressed action, and
/// whether the trailing `reset` button was pressed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Head {
    pub renamed: String,
    pub chosen: Option<i64>,
    pub reset_pressed: bool,
}


/// The text being edited with its caret and selection anchor (byte offsets).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEdit {
    pub text: String,
    pub caret: usize,
    pub anchor: usize,
}


/// Options of `value_field` beyond its place and key.
#[derive(Clone)]
pub struct ValueFieldOpts {
    pub size: Option<i64>,
    pub display: Option<String>,
    pub fraction: Option<f32>,
    pub slide: Option<Rc<dyn Fn(f32) -> String>>,
    pub scrub: Option<Rc<dyn Fn(&str, f32, bool) -> String>>,
    pub left: bool,
    pub edit: bool,
    pub lead: Option<(String, Color)>,
    pub trail: Option<(String, Color)>,
    pub line: Option<Color>,
    pub bare: bool,
    pub placeholder: Option<String>,
    pub tracking: Option<f32>,
    /// the owner follows a drag of the slider between builds (`field_drag`, `patch_field`): the
    /// drag dirties no view (`DRAG_FREE`)
    pub kept_drag: bool,
}

impl Default for ValueFieldOpts {
    fn default() -> ValueFieldOpts {
        ValueFieldOpts {
            size: None,
            display: None,
            fraction: None,
            slide: None,
            scrub: None,
            left: false,
            edit: false,
            lead: None,
            trail: None,
            line: None,
            bare: false,
            placeholder: None,
            tracking: None,
            kept_drag: false,
        }
    }
}


pub fn ellipsis<'a>(width: &mut dyn FnMut(&str) -> f32, limit: f32, label: &'a str) -> Cow<'a, str> {
    let dots = width("\u{2026}");
    // `cut`: the longest prefix that fits with the ellipsis after it; -1 when the ellipsis alone does
    // not
    let mut cut: i64 = if dots <= limit { 0 } else { -1 };
    let mut used = 0.0;
    for (index, c) in label.char_indices() {
        let next = index + c.len_utf8();
        used += width(&label[index..next]);
        if used > limit {
            return if cut < 0 { Cow::Borrowed("") } else { Cow::Owned(format!("{}\u{2026}", &label[..cut as usize])) };
        }
        if used + dots <= limit {
            cut = next as i64;
        }
    }
    Cow::Borrowed(label)
}

pub fn contains(rect: Rect, point: (f32, f32)) -> bool { rect.contains(point) }

pub fn intersect(a: Rect, b: Rect) -> Rect { a.intersect(b) }

// ---------------------------------------------------------------- keys

/// The seed of a root: the key every top-level label hashes under.
pub const ROOT_KEY: i64 = 0x2c1b3c6d;

/// Keys 0 and 1 are the open-addressing empty/tombstone marks.
pub fn hash_range(seed: i64, text: &str, first: usize, last: usize) -> i64 {
    let hash = hash::bytes(seed as u64, &text.as_bytes()[first..last]) as i64;
    if hash == 0 || hash == 1 { hash + 2 } else { hash }
}

pub fn find_marker(text: &str, marker: &str) -> Option<usize> {
    if text.contains('#') { text.find(marker) } else { None }
}

pub fn key_of(seed: i64, text: &str) -> i64 {
    // the id after ### replaces the label's hash, under the same parent: two panels that build the
    // same widget keep separate state
    match find_marker(text, "###") {
        Some(index) => hash_range(seed, text, index + 3, text.len()),
        None => hash_range(seed, text, 0, text.len()),
    }
}

/// The label without its `##` suffix.
pub fn shown(text: &str) -> &str {
    match find_marker(text, "##") {
        Some(index) => &text[..index],
        None => text,
    }
}

/// The label without its `##` suffix, owned.
pub fn display(text: &str) -> String {
    shown(text).to_owned()
}

/// Integer-derived child keys: no label string per frame.
pub fn int_key(seed: i64, n: i64) -> i64 {
    let hash = hash::mix(seed, n);
    if hash == 0 || hash == 1 { hash + 2 } else { hash }
}

// ---------------------------------------------------------------- text helpers

/// Where the top of kit text of `size` points goes to sit centred in a row at `y` of height `h`: its
/// top is half of what the row has over the text size. Measured against the kit's reference
/// renders: 13-point text in a 24-point row has its ascenders at 7 and its descenders at 18.
pub fn text_top_for(font_size: i64, size: Option<i64>, y: f32, h: f32) -> f32 {
    y + ((h - size.unwrap_or(font_size) as f32) / 2.0).floor()
}

pub fn line_start(text: &str, i: usize) -> usize {
    let b = text.as_bytes();
    let mut i = i;
    while i > 0 && b[i - 1] != b'\n' {
        i -= 1;
    }
    i
}

pub fn line_end(text: &str, i: usize) -> usize {
    let b = text.as_bytes();
    let n = b.len();
    let mut i = i;
    while i < n && b[i] != b'\n' {
        i += 1;
    }
    i
}

/// Case-insensitive (ASCII) subsequence match. Allocates nothing.
pub fn fuzzy_match(query: &str, text: &str) -> bool {
    fuzzy_score(query, text).is_some()
}

/// `fuzzy_match` with a rank: `None` when `query` is not a subsequence of `text`, else higher for a
/// better match. Each matched byte scores 1, plus 5 when it follows the previous match directly and
/// 8 when it starts a word (the text's start, after a non-alphanumeric byte, or an upper-case byte
/// after a lower-case one); each byte skipped before a match costs 1. A byte that cannot continue the
/// previous match takes the next word start that still leaves the rest of the query matchable.
/// No allocation (queries past 64 bytes match leftmost); O(text x query) at worst.
pub fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    let (q, t) = (query.as_bytes(), text.as_bytes());
    let word_start = |i: usize| i == 0 || !t[i - 1].is_ascii_alphanumeric() || (t[i].is_ascii_uppercase() && t[i - 1].is_ascii_lowercase());
    // the latest position each query byte can take with the rest still matchable after it
    let mut limit = [usize::MAX; 64];
    let mut end = t.len();
    for k in (0..q.len()).rev() {
        end = (0..end).rev().find(|&i| t[i].eq_ignore_ascii_case(&q[k]))?;
        if k < 64 {
            limit[k] = end;
        }
    }
    let (mut score, mut at, mut last) = (0, 0usize, None::<usize>);
    for (k, &c) in q.iter().enumerate() {
        let hit = |i: &usize| t[*i].eq_ignore_ascii_case(&c);
        let first = (at..t.len()).find(hit)?;
        let i = if last.is_some_and(|l| l + 1 == first) || word_start(first) || k >= 64 {
            first
        } else {
            (first..=limit[k]).find(|i| hit(i) && word_start(*i)).unwrap_or(first)
        };
        score += 1 + if last.is_some_and(|l| l + 1 == i) { 5 } else { 0 } + if word_start(i) { 8 } else { 0 } - (i - at) as i32;
        (at, last) = (i + 1, Some(i));
    }
    Some(score)
}

/// What a code editor knows about its text. A host supplies one (the editor's Lisp); the text area
/// itself is language-free. Byte positions are byte offsets into the text.
#[derive(Clone)]
pub struct Language {
    /// sorted, non-overlapping byte spans and their colour; the rest is the foreground
    pub colorize: Rc<dyn Fn(&str) -> Vec<(i64, i64, Color)>>,
    /// the matched bracket pairs as (open, close) byte positions: the pair at the caret is lit
    pub brackets: Rc<dyn Fn(&str) -> Vec<(i64, i64)>>,
    /// `indent text caret`: the indentation Enter puts after the line break at `caret`
    pub indent: Rc<dyn Fn(&str, i64) -> String>,
    /// brackets typed in pairs: the opener wraps the selection or inserts both, the closer typed
    /// before itself steps over it, Backspace between an empty pair takes both
    pub pairs: Vec<(char, char)>,
    /// `complete text caret`: the ranked suggestions for the token ending at `caret`, best first
    pub complete: Rc<dyn Fn(&str, i64) -> Vec<Completion>>,
    /// `describe text byte`: the token at `byte` and its description, shown as a tooltip
    pub describe: Rc<dyn Fn(&str, i64) -> Option<(i64, i64, String)>>,
    /// `number_at text byte`: the numeric literal at `byte`; dragging it sideways changes the value
    pub number_at: Rc<dyn Fn(&str, i64) -> Option<(i64, i64)>>,
    /// `context_at text caret`: what a character typed at `caret` lands in
    pub context_at: Rc<dyn Fn(&str, i64) -> CodeContext>,
    /// what starts a line comment: Command-/ comments or uncomments the lines the selection touches
    pub comment: String,
    /// `rewrite text caret`, run after every frame that edited the text: the text as the language
    /// keeps it and where `caret` lands in it
    pub rewrite: Option<Rc<dyn Fn(&str, i64) -> (String, i64)>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrubPhase {
    Live,
    Done,
}

/// Options of `text_area_submit` (see there).
#[derive(Clone)]
pub struct TextAreaOpts {
    pub wrap: bool,
    pub errors: Vec<i64>,
    pub messages: Vec<(i64, Option<(i64, i64)>, String)>,
    pub spans: Vec<(i64, i64)>,
    pub reveal: Option<i64>,
    pub reveal_nonce: i64,
    pub select: Option<(i64, i64)>,
    pub splice: Vec<(i64, i64, String)>,
    pub language: Option<Language>,
    pub on_context: Option<Rc<dyn Fn((f32, f32))>>,
    pub on_scrub: Option<Rc<dyn Fn(ScrubPhase)>>,
    pub on_scrub_edit: Option<Rc<dyn Fn((i64, i64), &str)>>,
    pub on_click: Option<Rc<dyn Fn(i64, bool)>>,
    pub on_caret: Option<Rc<dyn Fn(i64)>>,
    pub on_caret_move: Option<Rc<dyn Fn(i64)>>,
    pub on_select: Option<Rc<dyn Fn((i64, i64))>>,
    pub on_drop: Option<Rc<dyn Fn(i64, Drop)>>,
    pub chips: Vec<(i64, i64, Color)>,
}

impl Default for TextAreaOpts {
    fn default() -> TextAreaOpts {
        TextAreaOpts {
            wrap: false,
            errors: Vec::new(),
            messages: Vec::new(),
            spans: Vec::new(),
            reveal: None,
            reveal_nonce: 0,
            select: None,
            splice: Vec::new(),
            language: None,
            on_context: None,
            on_scrub: None,
            on_scrub_edit: None,
            on_click: None,
            on_caret: None,
            on_caret_move: None,
            on_select: None,
            on_drop: None,
            chips: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct PickerOpts {
    pub limit: i64,
    pub mark: Option<Rc<dyn Fn(i64) -> Option<Color>>>,
    pub off: Option<Rc<dyn Fn(i64) -> bool>>,
    pub slash: bool,
    pub at_rest: bool,
}

impl Default for PickerOpts {
    fn default() -> PickerOpts {
        PickerOpts { limit: 10, mark: None, off: None, slash: true, at_rest: false }
    }
}

/// A submenu of a `context_menu`.
#[derive(Clone, Debug, PartialEq)]
pub struct Submenu {
    /// the row of the menu that opens it (drawn with a chevron)
    pub row: i64,
    pub rows: Vec<(String, bool)>,
    pub keys: Vec<String>,
    /// the row marked with the accent square
    pub current: Option<i64>,
}

#[derive(Clone, Default)]
pub struct MenuOpts {
    pub width: Option<f32>,
    pub selected: Option<i64>,
    pub swatches: Vec<Option<Color>>,
    pub keys: Vec<String>,
    pub danger: Vec<i64>,
    pub submenus: Vec<Submenu>,
    pub lead_from: i64,
}
