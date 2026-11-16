//! Plain UI data and pure functions shared by the engine and the wrapper: box flags, keys, small results and text helpers. One
//! definition each; `pxui_core` and `pxui` re-export them. The option records that hold closures (`Rc<dyn Fn>`) are not here: they
//! are not ABI types, and the engine and the wrapper each keep theirs (`opts` in both).
use crate::geom::{hash, Rect, Rgba as Color};
use crate::theme::Look;
use std::borrow::Cow;

/// Fixed rows in one scroll region, with a scrolling lead-in and a sticky column heading.
#[derive(Clone, Copy, Debug)]
pub struct SectionList {
    pub count: usize,
    pub row_height: f32,
    pub leading_height: f32,
    pub header_height: f32,
    pub trailing_height: f32,
    pub overlay_scrollbar: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionPart {
    Leading,
    Header,
    Row(usize),
}

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


/// What `Ui::animate` moves a node to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Look(Look),
    /// A retained view's contents move through its table entry; an ordinary node moves its own paint.
    Offset(f32, f32),
    /// A retained view's contents fade together; an ordinary node fades its own paint.
    Opacity(f32),
    /// A view's contents, after its manual transform, about a fractional point of its box.
    /// The anchor is fixed for the segment; (0, 0) is top-left, (0.5, 0.5) centre.
    Scale { x: f32, y: f32, anchor: (f32, f32) },
}

/// One stop of `Ui::animate_keys`: the node reaches `target` `time` seconds after the start, the segment before it shaped by `curve`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe {
    /// Seconds after the start; the stops' times ascend from 0.
    pub time: f32,
    /// Where the stop moves its channel; the other channels keep the stop before's.
    pub target: Target,
    /// The shape of the segment that ends at this stop.
    pub curve: crate::theme::Curve,
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

/// One row of a `Ui::menu`, a `Copy` value built with `const fn` builders from `MenuItem::new`.
///
/// Empty text is a separator line (never chosen, drawn at the menu's separator height).
#[derive(Clone, Copy, Debug)]
pub struct MenuItem<'a> {
    /// The row's label.
    pub text: &'a str,
    /// Whether the row can be chosen; a disabled row shows the disabled look and takes no press.
    pub enabled: bool,
    /// Whether the row reads in the error ink (a delete).
    pub danger: bool,
    /// `Some` keeps a slot before the label for the current-choice mark; `Some(true)` draws the accent square in it.
    pub checked: Option<bool>,
    /// The shortcut text at the row's end (empty for none).
    pub shortcut: &'a str,
    /// A small colour square before the label.
    pub swatch: Option<Color>,
    /// The rows of the second menu this row opens beside the menu while the pointer is on it (empty for none).
    pub submenu: &'a [MenuItem<'a>],
}

impl<'a> MenuItem<'a> {
    /// An enabled row with `text` and nothing else.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::MenuItem;
    /// const PLAY: MenuItem = MenuItem::new("Play next");
    /// assert!(PLAY.enabled && !PLAY.danger);
    /// ```
    pub const fn new(text: &'a str) -> MenuItem<'a> { MenuItem { text, enabled: true, danger: false, checked: None, shortcut: "", swatch: None, submenu: &[] } }

    /// Sets whether the row can be chosen.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let off = pxui_types::ui::MenuItem::new("Paste").enabled(false);
    /// assert!(!off.enabled);
    /// ```
    pub const fn enabled(mut self, enabled: bool) -> MenuItem<'a> { self.enabled = enabled; self }

    /// Inks the row in the theme's error colour.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const DELETE: pxui_types::ui::MenuItem = pxui_types::ui::MenuItem::new("Delete").danger();
    /// assert!(DELETE.danger);
    /// ```
    pub const fn danger(mut self) -> MenuItem<'a> { self.danger = true; self }

    /// Gives the row the mark slot, marked when `checked`; the menu keeps the slot on every row that has one.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let row = pxui_types::ui::MenuItem::new("Small").checked(true);
    /// assert_eq!(row.checked, Some(true));
    /// ```
    pub const fn checked(mut self, checked: bool) -> MenuItem<'a> { self.checked = Some(checked); self }

    /// Shows `shortcut` at the row's end in the secondary ink (display only; the menu binds no key).
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let row = pxui_types::ui::MenuItem::new("Copy").key("Ctrl C");
    /// assert_eq!(row.shortcut, "Ctrl C");
    /// ```
    pub const fn key(mut self, shortcut: &'a str) -> MenuItem<'a> { self.shortcut = shortcut; self }

    /// Draws a small colour square before the label.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::{geom::Rgba, ui::MenuItem};
    /// let row = MenuItem::new("Red").swatch(Rgba { r: 255, g: 0, b: 0, a: 255 });
    /// assert!(row.swatch.is_some());
    /// ```
    pub const fn swatch(mut self, color: Color) -> MenuItem<'a> { self.swatch = Some(color); self }

    /// Gives the row a chevron and a second menu of `rows` (rows of its own, no deeper submenus) while the pointer is on it.
    /// The rows of the submenus are numbered after the rows of the menu, in the order of the rows that open them.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::MenuItem;
    /// const SIZES: [MenuItem; 2] = [MenuItem::new("Small"), MenuItem::new("Large")];
    /// let row = MenuItem::new("Size").submenu(&SIZES);
    /// assert_eq!(row.submenu.len(), 2);
    /// ```
    pub const fn submenu(mut self, rows: &'a [MenuItem<'a>]) -> MenuItem<'a> { self.submenu = rows; self }
}

/// How `Ui::menu` varies the kit's menu, as a `Copy` value built with `const fn` builders from `Menu::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Menu {
    /// A row's height in points (`None`: the theme's row).
    pub row_h: Option<f32>,
    /// The least width in points (`None`: 150); the menu is as wide as its longest row.
    pub width: Option<f32>,
    /// The most rows shown at once (`None`: all); a menu with more rows scrolls its row column.
    pub max_rows: Option<usize>,
}

impl Menu {
    /// The plain kit menu: the theme's row height, at least 150 points wide, every row shown.
    pub const DEFAULT: Menu = Menu { row_h: None, width: None, max_rows: None };

    /// Sets a row's height in points; the label, mark, swatch and shortcut centre in it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const TALL: pxui_types::ui::Menu = pxui_types::ui::Menu::DEFAULT.row_h(36.0);
    /// assert_eq!(TALL.row_h, Some(36.0));
    /// ```
    pub const fn row_h(mut self, row_h: f32) -> Menu { self.row_h = Some(row_h); self }

    /// Sets the least width in points.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const WIDE: pxui_types::ui::Menu = pxui_types::ui::Menu::DEFAULT.width(200.0);
    /// assert_eq!(WIDE.width, Some(200.0));
    /// ```
    pub const fn width(mut self, width: f32) -> Menu { self.width = Some(width); self }

    /// Shows at most `rows` rows at once: a menu with more rows is that tall and its row column scrolls (wheel, drag or touch).
    /// The submenu of a row is not limited, and is placed level with its row unscrolled.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const SHORT: pxui_types::ui::Menu = pxui_types::ui::Menu::DEFAULT.max_rows(8);
    /// assert_eq!(SHORT.max_rows, Some(8));
    /// ```
    pub const fn max_rows(mut self, rows: usize) -> Menu { self.max_rows = Some(rows); self }
}

/// What a `Ui::menu` call reports for the frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuResponse {
    /// The index of the row chosen this frame (a click, or Enter on the focused row); the menu has closed itself.
    pub chosen: Option<usize>,
    /// The menu closed without a choice (Escape, window focus loss or a press outside): stop calling `menu`.
    pub dismissed: bool,
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

/// A node's key among its siblings: a fixed label or an item id, never display text.
///
/// Like React's `key`: unique among the children of one parent, so list rows pass their item id. A key is resolved under the
/// parent's seed, so the same key under two parents names two nodes. Two siblings with one key take order-stable `#dup` keys.
/// Write it with `.into()` at a call site: `ui.node("toolbar".into(), node)`, `ui.node(song.id.into(), node)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A fixed label, hashed as the label of `key_of` under the parent's seed (a `###` suffix replaces it).
    Label(&'static str),
    /// An item id, hashed as the label of a NUL byte and its 16 lowercase hex digits under the parent's seed.
    Id(i64),
}
impl From<&'static str> for Key { #[inline] fn from(label: &'static str) -> Key { Key::Label(label) } }
impl From<i64> for Key { #[inline] fn from(id: i64) -> Key { Key::Id(id) } }
impl From<u64> for Key { #[inline] fn from(id: u64) -> Key { Key::Id(id as i64) } }

impl Key {
    /// The label this key is hashed as under the parent's seed: a `Label` itself, an `Id` written into `buf` as a NUL byte and
    /// its 16 lowercase hex digits (so an id never collides with a printable label).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::Key;
    /// let mut buf = [0u8; 17];
    /// assert_eq!(Key::Label("row").label(&mut buf), "row");
    /// assert_eq!(Key::Id(255).label(&mut buf), "\u{0}00000000000000ff");
    /// ```
    #[inline]
    pub fn label<'b>(&self, buf: &'b mut [u8; 17]) -> &'b str {
        match *self { Key::Label(label) => label, Key::Id(id) => id_text(id, buf) }
    }
}

/// `id` as a label: a NUL byte and its 16 lowercase hex digits, written into `out`. Out of line: label keys inline without it.
#[cold]
#[inline(never)]
fn id_text(id: i64, out: &mut [u8; 17]) -> &str {
    out[0] = 0;
    for (at, digit) in out[1..].iter_mut().enumerate() { *digit = b"0123456789abcdef"[(id as u64 >> (60 - 4 * at)) as usize & 15]; }
    // NUL and ASCII hex digits: always UTF-8
    core::str::from_utf8(out).unwrap_or_default()
}

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


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrubPhase {
    Live,
    Done,
}





/// A number literal dragged `dx` points: a float moves a tenth of its last decimal place per
/// point (Shift: ten times that) and keeps one more decimal; an integer moves one per six points,
/// as the inspector's integers do.
pub fn scrubbed(literal: &str, dx: f32, coarse: bool) -> String {
    let dx = dx as f64;
    let int_value = literal.parse::<i64>().ok();
    match (literal.find('.'), int_value) {
        (None, Some(v)) => {
            let n = (dx / 6.0).round();
            if n == 0.0 {
                literal.to_owned()
            } else {
                (((v as f64) + n).round() as i64).to_string()
            }
        }
        (Some(dot), _) => {
            let decimals = literal.len() - dot - 1;
            let step = 10.0f64.powf(-((decimals + 1) as f64)) * if coarse { 10.0 } else { 1.0 };
            let v = (literal).parse::<f64>().ok().unwrap_or(0.0) + dx * step;
            let s = format!("{:.*}", decimals + 1, v);
            let mut n = s.len();
            while n > 0 && s.as_bytes()[n - 1] == b'0' {
                n -= 1;
            }
            if n > 0 && s.as_bytes()[n - 1] == b'.' { s[..n + 1].to_owned() } else { s[..n].to_owned() }
        }
        (None, None) => literal.to_owned(),
    }
}
