//! Plain UI data and pure functions shared by the engine and the wrapper: box flags, keys, small results and text helpers. One
//! definition each; `pxui_core` and `pxui` re-export them. The option records that hold closures (`Rc<dyn Fn>`) are not here: they
//! are not ABI types, and the engine and the wrapper each keep theirs (`opts` in both).
use crate::geom::{hash, Rect, Rgba as Color};
use crate::theme::Look;
use std::borrow::Cow;

/// Fixed rows in one scroll region, with a scrolling lead-in and a sticky column heading.
#[derive(Clone, Copy, Debug)]
pub struct SectionList {
    /// Number of rows.
    pub count: usize,
    /// Height of each row in points.
    pub row_height: f32,
    /// Height in points of the lead-in that scrolls above the header.
    pub leading_height: f32,
    /// Height in points of the sticky column heading.
    pub header_height: f32,
    /// Height in points of the space after the last row.
    pub trailing_height: f32,
    /// The scrollbar floats over the rows instead of taking width from them.
    pub overlay_scrollbar: bool,
}

/// The part of a `SectionList` a build callback is asked to fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionPart {
    /// The scrolling lead-in above the heading.
    Leading,
    /// The sticky column heading.
    Header,
    /// Row `n`, counted from 0.
    Row(usize),
}

/// Points a pressed pointer travels before a press is a drag: a carry starts, a click ends.
pub const DRAG_THRESHOLD: f32 = 4.0;

/// What a carry holds: a `kind` and a `value`, both strings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Payload {
    /// What sort of thing is carried; a drop target accepts by comparing it.
    pub kind: String,
    /// The thing itself, in the app's own encoding.
    pub value: String,
}

/// What a drop target sees of a carry this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Drop {
    /// the pointer is over the box while a payload is held
    Hover(Payload),
    /// the payload was released over the box, this frame only
    Dropped(Payload),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    /// The pointer shape for dragging an edge left or right.
    HorizontalResize,
    /// The pointer shape for dragging an edge up or down.
    VerticalResize,
    /// The text insertion (I-beam) pointer.
    Text,
}

/// Which history step a keystroke asked for (`Ui::passed_undo`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndoKind {
    /// Step back.
    Undo,
    /// Step forward again.
    Redo,
}


/// What `Ui::animate` moves a node to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// Move the node's look to this one.
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
    /// Nothing happened.
    None,
    /// Row `n` was chosen.
    Pick(i64),
    /// Row `n` was asked to be deleted.
    Delete(i64),
    /// The query was submitted.
    Submit,
    /// Backspace or Left on an empty query: go up a level.
    Back,
    /// The picker was dismissed.
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
    /// What the registered bodies of the menu's rows read from their config (`PxuiMenuRowConfig::variant`); 0 for none.
    pub variant: u16,
}

impl Menu {
    /// The plain kit menu: the theme's row height, at least 150 points wide, every row shown.
    pub const DEFAULT: Menu = Menu { row_h: None, width: None, max_rows: None, variant: 0 };

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

    /// Sets the variant the registered body of each row reads from its config (`variant`, 0 by default): one body builds different
    /// structures by it, so the app picks per call without comparing text. The built-in body ignores it.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const COMPACT: pxui_types::ui::Menu = pxui_types::ui::Menu::DEFAULT.variant(1);
    /// assert_eq!(COMPACT.variant, 1);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Menu { self.variant = variant; self }
}

/// What a `Ui::menu` call reports for the frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuResponse {
    /// The index of the row chosen this frame (a click, or Enter on the focused row); the menu has closed itself.
    pub chosen: Option<usize>,
    /// The menu closed without a choice (Escape, window focus loss or a press outside): stop calling `menu`.
    pub dismissed: bool,
}

/// How `Ui::dialog` varies the modal dialog, as a `Copy` value built with `const fn` builders from `Dialog::DEFAULT`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dialog {
    /// The sheet's width in points.
    pub width: f32,
    /// What the registered dialog body reads from its config (`PxuiDialogConfig::variant`); 0 for none. Only the dialog's body is given
    /// it: the buttons and the widgets of the dialog's content see 0.
    pub variant: u16,
}

impl Dialog {
    /// The plain dialog: a 320-point sheet.
    pub const DEFAULT: Dialog = Dialog { width: 320.0, variant: 0 };

    /// Sets the sheet's width in points; it must be finite and not negative.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const WIDE: pxui_types::ui::Dialog = pxui_types::ui::Dialog::DEFAULT.width(420.0);
    /// assert_eq!(WIDE.width, 420.0);
    /// ```
    pub const fn width(mut self, width: f32) -> Dialog { self.width = width; self }

    /// Sets the variant the registered dialog body reads from its config (`variant`, 0 by default): one body builds different
    /// structures by it, so the app picks per call without comparing text. The built-in body ignores it.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const ALERT: pxui_types::ui::Dialog = pxui_types::ui::Dialog::DEFAULT.variant(1);
    /// assert_eq!(ALERT.variant, 1);
    /// ```
    pub const fn variant(mut self, variant: u16) -> Dialog { self.variant = variant; self }
}

/// One button of `Ui::dialog`, a `Copy` value built with `const fn` builders from `DialogButton::new`; the buttons sit at the end of
/// the dialog's last row in the order given. Return presses the `default` one, Escape the `cancel` one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DialogButton<'a> {
    /// The label, which is also the accessible name.
    pub text: &'a str,
    /// Return presses this button (the first one marked when several are).
    pub default: bool,
    /// Escape, and a press outside the dialog, choose this button (the first one marked when several are).
    pub cancel: bool,
    /// The label shows in the theme's invalid colour.
    pub danger: bool,
    /// A disabled button takes no press, shows its disabled look and is skipped by Return and Escape.
    pub enabled: bool,
}

impl<'a> DialogButton<'a> {
    /// A plain, enabled button labelled `text`, which it borrows (a literal or a runtime string).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const KEEP: pxui_types::ui::DialogButton = pxui_types::ui::DialogButton::new("Keep");
    /// assert!(KEEP.enabled && !KEEP.default);
    /// ```
    pub const fn new(text: &'a str) -> DialogButton<'a> { DialogButton { text, default: false, cancel: false, danger: false, enabled: true } }

    /// Marks the button Return presses.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const OK: pxui_types::ui::DialogButton = pxui_types::ui::DialogButton::new("OK").default();
    /// assert!(OK.default);
    /// ```
    pub const fn default(mut self) -> DialogButton<'a> { self.default = true; self }

    /// Marks the button Escape (and a press outside the dialog) chooses.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const NO: pxui_types::ui::DialogButton = pxui_types::ui::DialogButton::new("Cancel").cancel();
    /// assert!(NO.cancel);
    /// ```
    pub const fn cancel(mut self) -> DialogButton<'a> { self.cancel = true; self }

    /// Inks the label in the theme's invalid colour (a destructive choice).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const DELETE: pxui_types::ui::DialogButton = pxui_types::ui::DialogButton::new("Delete").danger();
    /// assert!(DELETE.danger);
    /// ```
    pub const fn danger(mut self) -> DialogButton<'a> { self.danger = true; self }

    /// Sets whether the button takes input.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// let b = pxui_types::ui::DialogButton::new("Save").enabled(false);
    /// assert!(!b.enabled);
    /// ```
    pub const fn enabled(mut self, enabled: bool) -> DialogButton<'a> { self.enabled = enabled; self }
}

/// What a dialog's frame did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DialogResponse {
    /// The index of the button pressed, or chosen by Return or Escape.
    pub chosen: Option<usize>,
    /// Escape, a press outside the dialog or a lost window focus closed it: the caller stops calling `dialog`. `chosen` is then the
    /// cancel button, when there is an enabled one.
    pub dismissed: bool,
}

/// One ranked suggestion of `Language::complete`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    /// the byte span `insert` replaces
    pub replace: (i64, i64),
    /// the text that goes in
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
    /// Plain code.
    Code,
    /// Inside a string literal.
    String,
    /// Inside a comment.
    Comment,
}




/// Options of a floating panel; build from `PanelOpts::default()` with the setters.
#[derive(Clone)]
pub struct PanelOpts {
    /// a floating sheet's hairline (the theme's popup look in this colour); `look` replaces both
    pub stroke: Option<Color>,
    /// A look that replaces the theme's for the sheet.
    pub look: Option<Look>,
    /// A window (title-bar look) rather than a sheet.
    pub window: bool,
    /// Left edge in points.
    pub x: f32,
    /// Top edge in points.
    pub y: f32,
    /// Width in points.
    pub width: f32,
    /// Fixed height in points; `None` sizes to the content.
    pub height: Option<f32>,
    /// Largest height in points; `None` for no limit.
    pub max_height: Option<f32>,
    /// Inner padding in points.
    pub padding: i64,
}

impl Default for PanelOpts {
    fn default() -> PanelOpts {
        PanelOpts { stroke: None, look: None, window: false, x: 12.0, y: 12.0, width: 280.0, height: None, max_height: None, padding: 0 }
    }
}

impl PanelOpts {
    /// The options with the panel's top-left at `x`, `y` points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui_types::ui::PanelOpts;
    /// let opts = PanelOpts::default().at(20.0, 30.0);
    /// assert_eq!((opts.x, opts.y), (20.0, 30.0));
    /// ```
    pub fn at(mut self, x: f32, y: f32) -> PanelOpts {
        self.x = x;
        self.y = y;
        self
    }
    /// The options with the panel `width` points wide (default 280).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::PanelOpts::default().width(320.0).width, 320.0);
    /// ```
    pub fn width(mut self, width: f32) -> PanelOpts {
        self.width = width;
        self
    }
    /// The options with a fixed height of `height` points instead of the content's.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::PanelOpts::default().height(200.0).height, Some(200.0));
    /// ```
    pub fn height(mut self, height: f32) -> PanelOpts {
        self.height = Some(height);
        self
    }
    /// The options with the panel no taller than `height` points.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::PanelOpts::default().max_height(400.0).max_height, Some(400.0));
    /// ```
    pub fn max_height(mut self, height: f32) -> PanelOpts {
        self.max_height = Some(height);
        self
    }
    /// The options with `padding` points of inner padding.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui_types::ui::PanelOpts::default().padding(8).padding, 8);
    /// ```
    pub fn padding(mut self, padding: i64) -> PanelOpts {
        self.padding = padding;
        self
    }
    /// The options with the panel drawn as a window (`true`) or a sheet (`false`, the default).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert!(pxui_types::ui::PanelOpts::default().window(true).window);
    /// ```
    pub fn window(mut self, window: bool) -> PanelOpts {
        self.window = window;
        self
    }
}

/// What a `line_field`'s keys asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineStep {
    /// No navigation key was pressed.
    None,
    /// Move on to the next field.
    Next,
    /// Move back to the previous field.
    Previous,
    /// Escape: close the field.
    Close,
}


/// A text button of an inspector head: its label, its key in ink-3, whether it is on (the control
/// fill) and whether it can be pressed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadAction {
    /// The button's label.
    pub caption: String,
    /// The key shown beside it in ink-3.
    pub keycap: String,
    /// Whether it is on (drawn with the control fill).
    pub active: bool,
    /// Whether it can be pressed.
    pub usable: bool,
}

/// What a head reports for a frame: the name as edited, the index of the pressed action, and
/// whether the trailing `reset` button was pressed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Head {
    /// The name as edited.
    pub renamed: String,
    /// Index of the action pressed this frame, if any.
    pub chosen: Option<i64>,
    /// Whether the trailing reset button was pressed.
    pub reset_pressed: bool,
}


/// The text being edited with its caret and selection anchor (byte offsets).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEdit {
    /// The text.
    pub text: String,
    /// Caret byte offset (a char boundary).
    pub caret: usize,
    /// The other end of the selection, as a byte offset; equal to `caret` when nothing is selected.
    pub anchor: usize,
}





/// `label` cut to fit in `limit` points, ending in an ellipsis (U+2026) when it was cut; `width` measures a string in points.
///
/// The whole label when it fits; otherwise its longest prefix that fits with the ellipsis after it, and an empty string when even
/// the ellipsis alone does not fit.
/// Allocates: only when it cuts (the shortened `String`); a label that fits is borrowed.
/// Identity: `label` is display text, not a key; nothing is keyed or cached.
/// Panics: never (unless `width` does).
///
/// ```
/// use pxui_types::ui::ellipsis;
/// let mut width = |s: &str| s.chars().count() as f32 * 10.0;
/// assert_eq!(ellipsis(&mut width, 55.0, "abcdefgh"), "abcd\u{2026}");
/// assert_eq!(ellipsis(&mut width, 100.0, "abc"), "abc");
/// assert_eq!(ellipsis(&mut width, 5.0, "abc"), "");
/// ```
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

/// Whether `point` is inside `rect`: `Rect::contains` (left and top edges count, right and bottom do not).
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::{geom::Rect, ui::contains};
/// assert!(contains(Rect::new(0.0, 0.0, 4.0, 4.0), (1.0, 1.0)));
/// ```
pub fn contains(rect: Rect, point: (f32, f32)) -> bool { rect.contains(point) }

/// The overlap of `a` and `b`: `Rect::intersect` (zero width or height when they do not overlap).
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::{geom::Rect, ui::intersect};
/// assert_eq!(intersect(Rect::new(0.0, 0.0, 4.0, 4.0), Rect::new(2.0, 2.0, 4.0, 4.0)), Rect::new(2.0, 2.0, 2.0, 2.0));
/// ```
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

/// The key of bytes `first..last` of `text` under `seed`; never 0 or 1, which the engine's tables keep as empty and tombstone marks.
///
/// `first` and `last` are byte offsets and are not checked for char boundaries.
/// Allocates: never.
/// Panics: when `first > last` or `last` is past the end of `text`.
///
/// ```
/// use pxui_types::ui::hash_range;
/// assert_eq!(hash_range(7, "abcdef", 2, 4), hash_range(7, "xxcdxx", 2, 4));
/// ```
pub fn hash_range(seed: i64, text: &str, first: usize, last: usize) -> i64 {
    let hash = hash::bytes(seed as u64, &text.as_bytes()[first..last]) as i64;
    if hash == 0 || hash == 1 { hash + 2 } else { hash }
}

/// The byte index of the first `marker` (`##` or `###`) in `text`, or `None`; text with no `#` at all is rejected without searching.
///
/// A `marker` that has no `#` in it is never found.
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::ui::find_marker;
/// assert_eq!(find_marker("Play##space", "##"), Some(4));
/// assert_eq!(find_marker("Play", "##"), None);
/// ```
pub fn find_marker(text: &str, marker: &str) -> Option<usize> {
    if text.contains('#') { text.find(marker) } else { None }
}

/// The key of the label `text` under the parent seed `seed`: the id after a `###` replaces the label's hash, otherwise all of `text`
/// is hashed (a `##` suffix is part of the key; `shown` is what strips it from the display).
///
/// Never 0 or 1. Two labels that differ only before `###` share a key.
/// Allocates: never.
/// Identity: `text` is the node label; the same text under the same seed collides.
/// Panics: never.
///
/// ```
/// use pxui_types::ui::key_of;
/// assert_eq!(key_of(1, "Play###a"), key_of(1, "Stop###a"));
/// assert_ne!(key_of(1, "Play##a"), key_of(1, "Play##b"));
/// ```
pub fn key_of(seed: i64, text: &str) -> i64 {
    // the id after ### replaces the label's hash, under the same parent: two panels that build the
    // same widget keep separate state
    match find_marker(text, "###") {
        Some(index) => hash_range(seed, text, index + 3, text.len()),
        None => hash_range(seed, text, 0, text.len()),
    }
}

/// The label without its `##` suffix (everything from the first `##` on), borrowed from `text`.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// assert_eq!(pxui_types::ui::shown("Play##hotkey"), "Play");
/// assert_eq!(pxui_types::ui::shown("Play"), "Play");
/// ```
pub fn shown(text: &str) -> &str {
    match find_marker(text, "##") {
        Some(index) => &text[..index],
        None => text,
    }
}

/// The label without its `##` suffix, as an owned `String`.
///
/// Allocates: one `String`.
/// Panics: never.
///
/// ```
/// assert_eq!(pxui_types::ui::display("Play##hotkey"), "Play".to_owned());
/// ```
pub fn display(text: &str) -> String {
    shown(text).to_owned()
}

/// The key of child number `n` under `seed`, derived from the integers alone (no label string per frame); never 0 or 1.
///
/// Allocates: never.
/// Identity: `n` is the child's number; two equal `n` under one seed collide.
/// Panics: never.
///
/// ```
/// use pxui_types::ui::int_key;
/// assert_ne!(int_key(5, 1), int_key(5, 2));
/// assert_eq!(int_key(5, 1), int_key(5, 1));
/// ```
pub fn int_key(seed: i64, n: i64) -> i64 {
    let hash = hash::mix(seed, n);
    if hash == 0 || hash == 1 { hash + 2 } else { hash }
}

// ---------------------------------------------------------------- text helpers

/// Where the top of kit text of `size` points goes to sit centred in a row at `y` of height `h`: its
/// top is half of what the row has over the text size. Measured against the kit's reference
/// renders: 13-point text in a 24-point row has its ascenders at 7 and its descenders at 18.
///
/// `size` of `None` is `font_size`; the offset rounds down to a whole point. All values are points.
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::ui::text_top_for;
/// assert_eq!(text_top_for(13, None, 100.0, 24.0), 105.0);
/// ```
pub fn text_top_for(font_size: i64, size: Option<i64>, y: f32, h: f32) -> f32 {
    y + ((h - size.unwrap_or(font_size) as f32) / 2.0).floor()
}

/// The byte index where the line holding byte `i` of `text` starts: just after the previous `\n`, or 0.
///
/// Allocates: never.
/// Panics: when `i` is past the end of `text`.
///
/// ```
/// assert_eq!(pxui_types::ui::line_start("ab\ncd", 4), 3);
/// ```
pub fn line_start(text: &str, i: usize) -> usize {
    let b = text.as_bytes();
    let mut i = i;
    while i > 0 && b[i - 1] != b'\n' {
        i -= 1;
    }
    i
}

/// The byte index where the line holding byte `i` of `text` ends: the next `\n` at or after `i`, or the end of `text`.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// assert_eq!(pxui_types::ui::line_end("ab\ncd", 0), 2);
/// assert_eq!(pxui_types::ui::line_end("ab\ncd", 3), 5);
/// ```
pub fn line_end(text: &str, i: usize) -> usize {
    let b = text.as_bytes();
    let n = b.len();
    let mut i = i;
    while i < n && b[i] != b'\n' {
        i += 1;
    }
    i
}

/// Whether the bytes of `query` appear in order in `text`, comparing ASCII letters without regard to case (an empty query matches).
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::ui::fuzzy_match;
/// assert!(fuzzy_match("fb", "FooBar"));
/// assert!(!fuzzy_match("bf", "FooBar"));
/// ```
pub fn fuzzy_match(query: &str, text: &str) -> bool {
    fuzzy_score(query, text).is_some()
}

/// `fuzzy_match` with a rank: `None` when `query` is not a subsequence of `text`, else higher for a
/// better match. Each matched byte scores 1, plus 5 when it follows the previous match directly and
/// 8 when it starts a word (the text's start, after a non-alphanumeric byte, or an upper-case byte
/// after a lower-case one); each byte skipped before a match costs 1. A byte that cannot continue the
/// previous match takes the next word start that still leaves the rest of the query matchable.
/// No allocation (queries past 64 bytes match leftmost); O(text x query) at worst.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::ui::fuzzy_score;
/// assert!(fuzzy_score("abc", "abcd") > fuzzy_score("abc", "xaxbxc"));
/// assert_eq!(fuzzy_score("zz", "abc"), None);
/// ```
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
/// How far a number-literal scrub has got.
pub enum ScrubPhase {
    /// The pointer is still dragging; the value is provisional.
    Live,
    /// The drag ended; the value is final.
    Done,
}





/// A number literal dragged `dx` points: a float moves a tenth of its last decimal place per
/// point (Shift: ten times that) and keeps one more decimal; an integer moves one per six points,
/// as the inspector's integers do.
///
/// `dx` is the pointer travel in points; `coarse` (Shift) makes a float move ten times as far and does not affect integers. A literal
/// that is neither an integer nor a decimal comes back unchanged, and an integer that does not move is returned as it was written.
/// Allocates: one `String`.
/// Panics: never.
///
/// ```
/// use pxui_types::ui::scrubbed;
/// assert_eq!(scrubbed("10", 12.0, false), "12");
/// assert_eq!(scrubbed("1.5", 10.0, false), "1.6");
/// assert_eq!(scrubbed("1.5", 10.0, true), "2.5");
/// assert_eq!(scrubbed("0x10", 12.0, false), "0x10");
/// ```
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
