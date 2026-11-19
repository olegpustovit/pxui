//! The desktop widget set (proposal P5 "Widget set"): checkbox, radio group, slider, progress,
//! spinner, tabs, segmented control, dropdown, menu bar, tooltip on a node, scroll view, virtual
//! list, tree and table, dialog, toasts, icon button, badge, search field and image. Each is a plain
//! method over `&mut Ui` taking a value and returning the new value (or an intent); each reads its
//! looks from one `States` of the theme and lays its parts out as flow children, so a theme file
//! restyles it and `set_dir(Rtl)` mirrors it. No standard appearance paints with a closure (the
//! image is a texture, the search field's caret the text widgets' canvas). Keyboard: every widget
//! is one Tab stop. As on macOS, a click focuses what edits or holds a selection (the slider, the
//! list, tree and table, the search field) and leaves buttons, boxes, tabs and menus `TAB_ONLY`;
//! Enter/Space activate, the arrows move within a group, Escape leaves. A warm frame allocates
//! nothing. Progress and spinner animate through patches (`busy_step`) without rebuilding their view.
use crate::a11y::state as st;
use crate::bodies::{RowConfig, TableHeadConfig, TreeRowConfig, body_of, in_body, in_item, in_place, BadgeConfig, DropdownConfig, MenuItemConfig, SegmentConfig, TabConfig, CheckConfig, SectionIndexConfig, IconButtonConfig, ProgressConfig, ScrollConfig, SearchFieldConfig, SliderConfig, SpinnerConfig, ToastConfig};
use crate::draw::Fit;
use crate::geom::Rgba;
use crate::ui_core::*;
use crate::ui_kit::{part_key, text_only, ButtonOpts, Response};
use input::{Event, Key};
use std::ops::RangeInclusive;

/// A node of a tree, in preorder: its depth (0 at the top) and whether it has children (not 0). A flag is a byte, not a
/// `bool`, so a caller's array of them is usable as it is, whatever its bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeNode {
    pub depth: u16,
    pub children: u8,
}

pub use pxui_types::ui::{Dialog, DialogButton, Chosen, Segment, Segmented};

/// The rows a tree shows, into `visible` (node indices, in order): every node, minus the subtrees of closed parents. Its count. A
/// function of its own, out of line: the hot loop of a large tree keeps its slices in registers (inlined in `tree`, one was reloaded
/// from the stack a node).
#[inline(never)]
fn flatten_tree(nodes: &[TreeNode], open: &[u8], visible: &mut [u32]) -> usize {
    let (mut i, mut count) = (0, 0);
    while i < nodes.len() {
        visible[count] = i as u32;
        count += 1;
        let mut j = i + 1;
        if nodes[i].children != 0 && open.get(i).copied().unwrap_or(0) == 0 {
            while j < nodes.len() && nodes[j].depth > nodes[i].depth {
                j += 1;
            }
        }
        i = j;
    }
    count
}
use pxui_types::theme::BodyKind;

pub use pxui_types::ui::{Checkbox, Dropdown, Slider, Toast};

/// Behavior for a caller-painted horizontal adjustable value. Pointer motion previews; only a
/// captured left-button release commits. Keyboard and accessibility adjustments commit directly.
/// Not a mirror of the Rust `SliderResponse`: it is the source of the C-ABI 1.x `PxuiScrubOut` record (`pxui_abi` `write_scrub`).
#[derive(Clone, Copy, Debug)]
pub struct Scrub {
    pub node: Handle,
    pub preview: Option<f32>,
    pub commit: Option<f32>,
    pub held: bool,
}

/// The key presses of `keys` (a focused node's signal), in order.
fn pressed(keys: &[Event], f: &mut dyn FnMut(Key)) {
    for e in keys {
        if let Event::KeyPressed(k) = e {
            f(*k);
        }
    }
}

/// `i` moved by `by` within `0..n`, wrapping.
/// A pick-one item's key under `seed`: its id in a namespace of its own, else its place (`int_key(seed, i)`, unchanged for
/// positional items); an id never meets a place (the namespace key is negative, places are not).
pub(crate) fn item_key(seed: i64, id: Option<i64>, i: usize) -> i64 {
    const ID_SPACE: i64 = -0x4944;
    match id { Some(id) => int_key(int_key(seed, ID_SPACE), id), None => int_key(seed, i as i64) }
}

fn wrap(i: usize, by: i64, n: usize) -> usize {
    if n == 0 { 0 } else { (i as i64 + by).rem_euclid(n as i64) as usize }
}

/// `i` moved by `by` within `0..n`, held at the ends.
pub(crate) fn clamp_step(i: usize, by: i64, n: usize) -> usize {
    if n == 0 { 0 } else { (i as i64 + by).clamp(0, n as i64 - 1) as usize }
}

/// What `BodyTable::item` is a row of.
pub(crate) const ITEM_NONE: u8 = 0;
pub(crate) const ITEM_LIST: u8 = 1;
const ITEM_TREE: u8 = 2;
pub(crate) const ITEM_TABLE: u8 = 3;
pub(crate) const ITEM_TEXT_CELL: u8 = 4;
pub(crate) const ROW_FLAGS: Flags = Flags::CLICKABLE.or(Flags::BLOCKING);
const STOP_FLAGS: Flags = Flags::CLICKABLE.or(Flags::FOCUSABLE).or(Flags::BLOCKING).or(Flags::TAB_ONLY);

/// The built-in dialog: the title bar, the content, then the buttons at the end of a row. Each button is built through the button
/// body as `Part::button(i)` of the dialog, in the row (the body's `line`), which is the 20-point row of the buttons with 6 points above
/// and below (as tall as the buttons when one has a detail line under its label); behaviour finds a button by its part and ignores the
/// presses of a disabled one.
pub(crate) fn default_dialog(this: &mut Ui, title: &str, buttons: &[DialogButton], content: &mut dyn FnMut(&mut Ui)) {
    let t = this.theme();
    let (bar, invalid) = (t.dialog.idle, t.tokens.invalid);
    let rh = this.row_px() as f32;
    let head = this.part(Part::TITLE, Node { h: Size::Px(rh + 8.0), look: bar, inset: Edges { l: this.space(2), ..Edges::ZERO }, ..Node::DEFAULT });
    this.text(&head, shown(title));
    content(this);
    let row = this.node("buttons".into(), Node { h: Size::Px(rh), axis: Axis::Row, align: Align::Center, justify: Justify::End, gap: 4.0, margin: Edges { t: 6.0, b: 6.0, ..Edges::ZERO }, ..Node::DEFAULT });
    let saved = this.enter(&row);
    for (i, b) in buttons.iter().enumerate() {
        let opts = ButtonOpts { enabled: b.enabled, justify: Justify::End, ink: b.danger.then_some(invalid), ..ButtonOpts::DEFAULT };
        this.button_in(&row, this.own_part_key(Part::button(i as u8)), b.text, &opts, ["", b.detail, ""]);
    }
    // each button's body lays the row out as its line; a button with a detail line makes it as tall as it needs
    if buttons.iter().any(|b| !b.detail.is_empty()) { this.node_new(&row).h = Size::Fit; }
    this.leave(saved);
}

/// The built-in toast: a row-and-a-half high box in the toast look, text 12 points in.
pub(crate) fn default_toast(this: &mut Ui, c: &ToastConfig, text: &str) {
    let (states, rh, side) = (this.theme().toast, this.row_px() as f32, this.space(2));
    let n = this.node_new(&c.node);
    (n.h, n.inset) = (Size::Px(rh + 12.0), Edges { l: side, r: side, ..Edges::ZERO });
    this.style(&c.node, &states, c.state);
    this.text(&c.node, text);
}

/// The built-in scroll view: the content in the node as the kit made it.
pub(crate) fn default_scroll(this: &mut Ui, _c: &ScrollConfig, content: &mut dyn FnMut(&mut Ui)) {
    content(this);
}

/// The built-in row of a list or a table: the kit's look is already on it; the content goes inside.
pub(crate) fn default_row(this: &mut Ui, c: &RowConfig, content: &mut dyn FnMut(&mut Ui)) {
    content(this);
    // the insertion line of a payload carried between the rows, over the row's top or bottom edge
    if c.drop == 1 || c.drop == 2 {
        let h = this.rect(&c.row).h.max(2.0);
        this.node_keyed(int_key(c.row.box_key, -2), Node { w: Size::Grow, h: Size::Px(2.0), at: Some((0.0, if c.drop == 1 { 0.0 } else { h - 2.0 })).into(), look: c.line, ..Node::DEFAULT });
    }
}

/// The built-in tree row: the disclosure mark (a spacer without children), then the content.
pub(crate) fn default_tree_row(this: &mut Ui, c: &TreeRowConfig, content: &mut dyn FnMut(&mut Ui)) {
    let chevron = this.theme().chevron.idle;
    let d = this.node_keyed(part_key(c.row.box_key, Part::DISCLOSURE), Node { flags: if c.has { ROW_FLAGS } else { Flags::NONE }, w: Size::Px(10.0), h: Size::Grow, look: if c.has { chevron } else { Look::NONE }, ..Node::DEFAULT });
    if c.has {
        this.node_new(&d).mark = Mark::Chevron(if c.open { Dir::Down } else if this.rtl() { Dir::Left } else { Dir::Right });
    }
    content(this);
}

/// The built-in table header cell: the column look for its state, the title and, when sorted by it, the sort chevron.
pub(crate) fn default_table_head(this: &mut Ui, c: &TableHeadConfig, title: &str) {
    let chevron = this.theme().chevron.idle;
    let look = *this.theme().column.pick(c.state);
    this.set_look(&c.node, look, c.state);
    let rh = this.row_px() as f32;
    this.text_leaf(int_key(c.node.box_key, 0), rh, look, title);
    if let Some(ascending) = c.ascending {
        let m = this.node("sort".into(), Node { w: Size::Px(6.0), h: Size::Grow, look: chevron, ..Node::DEFAULT });
        this.node_mut(&m).mark = Mark::Chevron(if ascending { Dir::Up } else { Dir::Down });
    }
}

/// The built-in tab: as wide as its text, in the tab look for its state; a `detail` (a count or badge) follows the text, a gap away.
pub(crate) fn default_tab(this: &mut Ui, c: &TabConfig, text: &str, detail: &str) {
    let look = *this.theme().tab.pick(c.state);
    if detail.is_empty() {
        let w = this.text_width(None, text);
        this.node_new(&c.node).w = Size::Px(w);
        this.set_look(&c.node, look, c.state);
        this.text(&c.node, text);
        return;
    }
    let (gap, rh) = (this.space(1), this.row_px() as f32);
    let w = this.text_width(None, text) + gap + this.text_width(None, detail);
    let n = this.node_new(&c.node);
    (n.w, n.axis, n.align, n.gap) = (Size::Px(w), Axis::Row, Align::Center, gap);
    this.set_look(&c.node, look, c.state);
    let saved = this.enter(&c.node);
    this.text_leaf(int_key(c.node.box_key, 0), rh, look, text);
    this.text_leaf(int_key(c.node.box_key, 1), rh, look, detail);
    this.leave(saved);
}

/// How `segmented_opts` varies the kit's segmented control; `DEFAULT` is the kit's plain strip.
pub struct SegmentedOpts {
    /// A disabled strip takes no press, key or focus, shows its segments' `disabled` look and reports disabled to accessibility.
    pub enabled: bool,
    /// The strip's height in points (the theme's control height when `None`).
    pub h: Option<f32>,
    /// The strip fills its line and its segments share its width, instead of each being as wide as its text.
    pub grow: bool,
    /// The look per state of every segment (`checked` for the choice); `None` is the theme's `segment`.
    pub states: Option<States>,
    /// The strip's width in points, its segments sharing it as with `grow`; `None` is as `grow` says.
    pub w: Option<f32>,
    /// A segment's text inset, and half the room around its text, in points (8 the kit's).
    pub padding: f32,
    /// The least width of a segment in points (0: none).
    pub min_segment: f32,
    /// A press on an option focuses it, and it is then the strip's Tab stop (the chosen option); by default a press chooses without focusing.
    pub focus_on_click: bool,
}

impl SegmentedOpts {
    pub const DEFAULT: SegmentedOpts = SegmentedOpts { enabled: true, h: None, grow: false, states: None, w: None, padding: 8.0, min_segment: 0.0, focus_on_click: false };
}

/// The built-in segment: the text's width and twice `padding` (or its share of the strip with `grow`), at least `min_segment`, `padding`
/// in, a point over the one before it.
pub(crate) fn default_segment(this: &mut Ui, c: &SegmentConfig, states: Option<&States>, text: &str) {
    let w = if c.grow { Size::Grow } else { Size::Px((this.text_width(None, text) + 2.0 * c.padding).max(c.min_segment)) };
    let n = this.node_new(&c.node);
    (n.w, n.margin, n.inset) = (w, Edges { l: if c.index == 0 { 0.0 } else { -1.0 }, ..Edges::ZERO }, Edges { l: c.padding, ..Edges::ZERO });
    if c.grow { n.min_w = c.min_segment; }
    let look = *states.unwrap_or(&this.theme().segment).pick(c.state);
    this.set_look(&c.node, look, c.state);
    this.text(&c.node, text);
}

/// The built-in row of a dropdown's or menu bar's open menu: 8 in, in the menu look.
pub(crate) fn default_menu_item(this: &mut Ui, c: &MenuItemConfig, text: &str) {
    this.node_new(&c.node).inset = Edges { l: this.space(2), ..Edges::ZERO };
    let look = *this.theme().menu.pick(c.state);
    this.set_look(&c.node, look, c.state);
    this.text(&c.node, text);
}

/// The built-in menu bar title: its text's width and 16, 8 in, in the menu look.
pub(crate) fn default_menu_title(this: &mut Ui, c: &MenuItemConfig, text: &str) {
    let w = this.text_width(None, text) + 16.0;
    let n = this.node_new(&c.node);
    (n.w, n.inset) = (Size::Px(w), Edges { l: 8.0, ..Edges::ZERO });
    let look = *this.theme().menu.pick(c.state);
    this.set_look(&c.node, look, c.state);
    this.text(&c.node, text);
}

/// The built-in dropdown button: the dropdown look for its state, the chosen option's text and a chevron.
pub(crate) fn default_dropdown(this: &mut Ui, c: &DropdownConfig, text: &str) {
    let t = this.theme();
    let (look, idle, chevron) = (*t.dropdown.pick(c.state), t.dropdown.idle, t.chevron.idle);
    this.set_look(&c.button, look, c.state);
    let node = this.node_keyed(int_key(c.key, 2), Node { flags: Flags::FIT_TEXT, h: Size::Grow, look: text_only(&idle), ..Node::DEFAULT });
    this.text(&node, text);
    let mark = this.node_keyed(int_key(c.key, 3), Node { w: Size::Px(6.0), h: Size::Grow, look: chevron, ..Node::DEFAULT });
    this.node_mut(&mark).mark = Mark::Chevron(Dir::Down);
}

impl Ui {
    pub(crate) fn rtl(&self) -> bool {
        self.dir() == Direction::Rtl
    }

    /// Left and right swap under right-to-left: the step the arrow means along a row.
    fn along(&self, k: Key) -> i64 {
        let s = if self.rtl() { -1 } else { 1 };
        match k {
            Key::ArrowRight => s,
            Key::ArrowLeft => -s,
            _ => 0,
        }
    }

    /// The row a `selection_list`, `tree` or `table` is building: the node a text row's or cell's text is put in and takes its look
    /// from, and where a cell's widgets get its accessible name.
    pub fn item(&self) -> Handle {
        self.bodies.item
    }

    /// The kit's text of the collection item being built: a `table` cell's text (it names the cell), a `selection_list` or `tree`
    /// row's text. A row-high label in the look of `item`.
    /// A list row's text names nothing (as before), a tree row's names the row. Panics outside a collection item (`in_item`).
    pub fn item_text(&mut self, text: &str) {
        assert!(self.in_item(), "Ui.item_text: outside a selection_list or tree row or a table cell");
        match self.bodies.item_kind {
            ITEM_LIST => { self.row_text(text); }
            ITEM_TREE => { let n = self.row_text(text); self.a11y_name(&n, text); }
            ITEM_TEXT_CELL => { self.text_cell.clear(); self.text_cell.push_str(text); self.text_cell_set = true; }
            _ => self.cell_text(text),
        }
    }

    /// Whether a `selection_list`/`tree` row or a `table` cell is being built (where `item_text` puts its text).
    pub fn in_item(&self) -> bool { self.bodies.item_kind != ITEM_NONE }

    /// A text row's text: a row-high label in the look of `item`, which it returns (the C text-row exports' row; it names nothing).
    pub fn row_text(&mut self, text: &str) -> Handle {
        let (n, rh) = (self.bodies.item, self.row_px() as f32);
        let look = self.node_mut(&n).look;
        self.text_leaf(int_key(n.box_key, 1), rh, look, text);
        n
    }

    /// A table cell's text: the cell (the node being built into) is named by it, and it is a row-high label in the look of `item`.
    pub fn cell_text(&mut self, text: &str) {
        let (c, row, rh) = (self.entered(), self.bodies.item, self.row_px() as f32);
        self.a11y_name(&c, text);
        let look = text_only(&self.node_mut(&row).look);
        self.text_leaf(int_key(c.box_key, 0), rh, look, text);
    }

    /// The theme's scroll thumb, 2 points in from the ends.
    pub(crate) fn thumb(&self) -> Bar {
        let t = self.theme();
        let thumb = t.scrollbar.idle;
        Bar::new(thumb.fill, t.scrollbar_w).radius(thumb.radius[0]).gap(t.scrollbar_gap).margin(2.0).whole()
    }

    /// A kit row `rh` high laid out as a row: `space(2)` in from both ends, children centred, 8 apart.
    fn kit_line(&mut self, key: i64, flags: Flags, role: Role) -> Handle {
        let rh = self.row_px() as f32;
        let side = self.space(2);
        self.node_keyed(key, Node { flags, h: Size::Px(rh), axis: Axis::Row, align: Align::Center, padding: Edges { l: side, r: side, ..Edges::ZERO }, gap: self.space(1), role, ..Node::DEFAULT })
    }

    /// A text leaf as wide as its text, `h` high, in `look`'s text.
    fn text_leaf(&mut self, key: i64, h: f32, look: Look, text: &str) -> Handle {
        let w = self.text_width(Some(crate::ui_core::font_px(&look.font, self.font_px())), text);
        let n = self.node_keyed(key, Node { w: Size::Px(w.max(1.0)), h: Size::Px(h), shrink: 1.0, flags: Flags::FIT_TEXT, look: text_only(&look), ..Node::DEFAULT });
        self.text(&n, text);
        n
    }

    /// A checkbox row keyed by `key` (hashed under the parent's seed): the 12-point box (an accent square inside when on) and `text`,
    /// which names it; `checkbox.variant` reaches a registered body. Reports the row's response (`clicked` flips the value).
    pub fn checkbox(&mut self, key: &str, text: &str, value: bool, checkbox: &Checkbox) -> Response {
        let variant = checkbox.variant;
        if variant != 0 { self.set_variant(BodyKind::checkbox as u32, variant); }
        let key = key_of(self.current_seed(), key);
        let enabled = !checkbox.disabled && !self.scope_disabled();
        let row = if checkbox.passive {
            self.node_keyed(key, Node { w: Size::Fit, axis: Axis::Row, align: Align::Center, gap: self.space(1), role: Role::CheckBox, ..Node::DEFAULT })
        } else {
            self.kit_line(key, if enabled { STOP_FLAGS } else { Flags::BLOCKING | Flags::TAB_ONLY }, Role::CheckBox)
        };
        let mut s = if checkbox.passive { Signal::default() } else { self.signal_owned(&row) };
        // a disabled checkbox reports no click from any input (a press begun while it was enabled, an assistive technology's press)
        s.clicked &= enabled;
        let value = value ^ s.clicked;
        self.set_role(&row, Role::CheckBox, st::CHECKABLE | if value { st::CHECKED } else { 0 } | if checkbox.passive { st::READ_ONLY } else { 0 });
        self.a11y_name(&row, text);
        let states = match checkbox.look { Some(look) => { let mut states = *look; self.resolve_states(&mut states); states } None => self.theme().checkbox };
        let config = CheckConfig {
            state: State { hovered: enabled && s.hovered, pressed: enabled && s.held, disabled: !enabled, ..State::default() }, on: value, row,
            size: checkbox.size.filter(|side| *side > 0.0).unwrap_or(12.0), states, passive: checkbox.passive,
        };
        in_body!(self, row.box_key, row, checkbox, default_checkbox(&config, text));
        self.node_new(&row).state.disabled = !enabled;
        if variant != 0 { self.set_variant(BodyKind::checkbox as u32, 0); }
        Response::of(&s, self.focused(&row), row)
    }

    /// The radio group over `count` items, item `i` being `item(i)`: an item with an id is keyed by it (its row, focus and a
    /// secondary press follow it through a reorder), one without by its place in a namespace ids never reach; `selected` none
    /// checks no row (an arrow then chooses the first or last). Reports the group's response, the choice and the row a secondary
    /// press began on.
    pub fn pick_radio<'a>(&mut self, key: &str, name: &str, count: usize, item: &dyn Fn(usize) -> Segment<'a>, selected: Option<usize>, variant: u16) -> (Response, Option<usize>, Option<usize>) {
        if variant != 0 { self.set_variant(BodyKind::radio as u32, variant); }
        let group = self.node_ref(key, &Node { flags: Flags::FOCUSABLE | Flags::TAB_ONLY, role: Role::Group, ..Node::DEFAULT });
        self.a11y_name(&group, name);
        let s = self.signal_owned(&group);
        let mut sel = selected.filter(|_| count > 0).map(|i| i.min(count - 1));
        pressed(&s.keys, &mut |k| match k {
            Key::ArrowDown | Key::ArrowRight => sel = Some(sel.map_or(0, |i| wrap(i, 1, count))),
            Key::ArrowUp | Key::ArrowLeft => sel = Some(sel.map_or(count.saturating_sub(1), |i| wrap(i, -1, count))),
            _ => {}
        });
        if count == 0 { sel = None; }
        let mut secondary = None;
        let saved = self.enter(&group);
        for i in 0..count {
            let option = item(i);
            let row = self.kit_line(item_key(group.box_key, option.id, i), ROW_FLAGS, Role::Radio);
            let rs = self.signal_owned(&row);
            if rs.clicked {
                sel = Some(i);
            }
            if rs.pressed && rs.button == Some(input::MouseButton::RightButton) { secondary = Some(i); }
            let on = sel == Some(i);
            self.set_role(&row, Role::Radio, st::CHECKABLE | if on { st::CHECKED } else { 0 });
            self.a11y_name(&row, option.name.unwrap_or(option.text));
            let config = CheckConfig { state: State { hovered: rs.hovered, pressed: rs.held, ..State::default() }, on, row, size: 12.0, states: self.theme().radio, passive: false };
            in_body!(self, row.box_key, row, radio, default_radio(&config, option.text));
        }
        self.leave(saved);
        if variant != 0 { self.set_variant(BodyKind::radio as u32, 0); }
        let mut r = Response::of(&s, self.focused(&group), group);
        r.secondary |= secondary.is_some();
        (r, sel, secondary)
    }

    /// `slider` with the held preview and the commit (see `slider_body`).
    pub fn slider_result(&mut self, key: &str, text: &str, value: f32, range: RangeInclusive<f32>, slider: &Slider) -> (Response, f32, Option<f32>, Option<f32>) {
        let variant = slider.variant;
        if variant != 0 { self.set_variant(BodyKind::slider as u32, variant); }
        let result = slider_body(self, key, text, value, range);
        if variant != 0 { self.set_variant(BodyKind::slider as u32, 0); }
        result
    }

    /// A behavior-only scrubber. Size it with the normal node layout API and paint into `node`.
    /// A collapsed range is read-only. Arrows step once, Page keys ten times; Home/End select ends.
    pub fn scrub(&mut self, label: &str, value: f32, range: RangeInclusive<f32>, step: f32) -> Scrub {
        let (lo, hi) = (*range.start(), *range.end());
        let enabled = hi > lo;
        let node = self.node_ref(label, &Node {
            flags: Flags::CLIP | if enabled { Flags::CLICKABLE | Flags::FOCUSABLE | Flags::FOCUS_MARK } else { Flags::NONE },
            w: Size::Grow, h: Size::Px(self.control_h()),
            role: if enabled { Role::Slider } else { Role::Progress },
            ..Node::DEFAULT
        });
        self.a11y_name(&node, shown(label));
        let value = value.clamp(lo, hi);
        let mut result = Scrub { node, preview: None, commit: None, held: false };
        if !enabled {
            self.cancel_press(&node);
            if self.focused(&node) { self.unfocus(); }
        } else {
            let signal = self.signal(&node);
            let rect = self.rect(&node);
            if signal.button == Some(input::MouseButton::LeftButton) && rect.w > 0.0 {
                result.held = signal.held;
                let at = |x: f32| {
                    let fraction = ((x - rect.x) / rect.w).clamp(0.0, 1.0);
                    let fraction = if self.rtl() { 1.0 - fraction } else { fraction };
                    if fraction == 0.0 { lo } else if fraction == 1.0 { hi } else { (lo + fraction * (hi - lo)).clamp(lo, hi) }
                };
                if signal.held { result.preview = Some(at(signal.pointer.0)); }
                if signal.released { result.commit = Some(at(signal.release_point.0)); }
            }
            if let Some(next) = signal.a11y_value.filter(|v| v.is_finite()) {
                result.commit = Some(next.clamp(f64::from(lo), f64::from(hi)) as f32);
                result.preview = result.commit;
                self.cancel_press(&node);
                result.held = false;
            }
            if self.focused(&node) {
                let mut next = result.preview.or(result.commit).unwrap_or(value);
                let mut adjusted = false;
                for (event, modifiers) in self.key_events_ref(&node) {
                    if modifiers.iter().any(|key| matches!(key, Key::Meta | Key::Ctrl | Key::Alt)) { continue; }
                    if let Event::KeyPressed(key) = event {
                        next = match key {
                            Key::ArrowRight | Key::ArrowLeft => next + self.along(*key) as f32 * step,
                            Key::ArrowUp => next + step,
                            Key::ArrowDown => next - step,
                            Key::PageUp => next + 10.0 * step,
                            Key::PageDown => next - 10.0 * step,
                            Key::Home => lo,
                            Key::End => hi,
                            _ => continue,
                        }.clamp(lo, hi);
                        result.commit = Some(next);
                        adjusted = true;
                    }
                }
                if adjusted {
                    self.cancel_press(&node);
                    result.held = false;
                }
            }
        }
        if !result.held && result.commit.is_some() { result.preview = result.commit; }
        self.a11y_number(&node, result.preview.unwrap_or(value) as f64, lo as f64, hi as f64, step as f64);
        result
    }

    /// A vertical section index (the A-Z strip): `letters` share the strip's height, and a press or drag on it chooses the letter under the
    /// pointer (the arrows, Home/End and an assistive technology's SetValue move the choice too). Returns the strip's response and the
    /// letter chosen this frame (`Some` while the pointer holds the strip, on its release and for a key or an assistive action; `None` when
    /// nothing moved it); an
    /// empty `letters` is a disabled strip. Keyed by `key` (hashed under the parent's seed), named `name`.
    pub fn section_index(&mut self, key: &str, name: &str, letters: &[&str]) -> (Response, Option<usize>) {
        let n = letters.len();
        let enabled = n > 0 && !self.scope_disabled();
        let node = self.node_ref(key, &Node {
            flags: Flags::CLIP | if enabled { Flags::CLICKABLE | Flags::FOCUSABLE | Flags::FOCUS_MARK } else { Flags::BLOCKING },
            w: Size::Px(self.control_h()), h: Size::Grow, axis: Axis::Column, role: if enabled { Role::Slider } else { Role::Progress },
            ..Node::DEFAULT
        });
        self.a11y_name(&node, name);
        let last = n.saturating_sub(1);
        let mut s = if enabled { self.signal(&node) } else { Signal::default() };
        let rect = self.rect(&node);
        let current = (self.state(&node, 0).max(0) as usize).min(last);
        let mut chosen = None;
        if enabled {
            let at = |y: f32| if rect.h > 0.0 { (((y - rect.y) / rect.h * n as f32).floor().max(0.0) as usize).min(last) } else { 0 };
            if s.button == Some(input::MouseButton::LeftButton) {
                if s.held { chosen = Some(at(s.pointer.1)); }
                if s.released { chosen = Some(at(s.release_point.1)); }
            }
            if let Some(v) = s.a11y_value.filter(|v| v.is_finite()) {
                chosen = Some((v.max(0.0) as usize).min(last));
                self.cancel_press(&node);
            }
            if self.focused(&node) {
                let mut next = chosen.unwrap_or(current);
                let mut moved = false;
                for (event, modifiers) in self.key_events_ref(&node) {
                    if modifiers.iter().any(|key| matches!(key, Key::Meta | Key::Ctrl | Key::Alt)) { continue; }
                    if let Event::KeyPressed(key) = event {
                        next = match key {
                            Key::ArrowDown => (next + 1).min(last),
                            Key::ArrowUp => next.saturating_sub(1),
                            Key::Home => 0,
                            Key::End => last,
                            _ => continue,
                        };
                        moved = true;
                    }
                }
                if moved { chosen = Some(next); self.cancel_press(&node); }
            }
        } else {
            s.clicked = false;
        }
        if let Some(i) = chosen { self.set_state(&node, i as i64); }
        let active = chosen.unwrap_or(current);
        self.a11y_number(&node, active as f64, 0.0, last as f64, 1.0);
        let focused = self.focused(&node);
        let config = SectionIndexConfig { state: State { hovered: enabled && s.hovered, pressed: enabled && s.held, focused, disabled: !enabled, ..State::default() }, node, active: (n > 0).then_some(active) };
        in_body!(self, node.box_key, node, section_index, default_section_index(&config, letters));
        (Response::of(&s, focused, node), chosen)
    }

    /// A progress row: the label and a 4-point track, with a bar `value` (0 to 1) of the way or, with `None`, a third-wide bar that
    /// `busy_step` sweeps across (the default body; a theme's `body.progress` picks another). Behaviour: the track's accessible
    /// name, number and busy state, on the node the body builds as `Part::TRACK`. Returns the row, for `busy_step`.
    /// Keyed by `key` (hashed under the parent's seed), showing and named `text`.
    pub fn progress(&mut self, key: &str, text: &str, value: Option<f32>) -> Handle {
        let (key, label) = (key_of(self.current_seed(), key), text);
        let row = self.kit_line(key, Flags::NONE, Role::Unknown);
        let config = ProgressConfig { value, row };
        in_body!(self, key, row, progress, default_progress(&config, label));
        // a body without a track part gets the facts on the row: the widget's own box
        let track = self.part_box(key, Part::TRACK).unwrap_or(row);
        self.a11y_name(&track, label);
        match value {
            Some(v) => {
                self.set_role(&track, Role::Progress, 0);
                self.a11y_number(&track, (v.clamp(0.0, 1.0) * 100.0) as f64, 0.0, 100.0, 1.0);
            }
            None => self.set_role(&track, Role::Progress, st::BUSY),
        }
        row
    }

    /// A 16-point spinner: eight dots round a circle, the lead dot in the accent. `busy_step` turns
    /// it. Returns its node.
    /// Keyed by `key` (hashed under the parent's seed), named `name`.
    pub fn spinner(&mut self, key: &str, name: &str) -> Handle {
        let n = self.node_ref(key, &Node { role: Role::Progress, a11y: st::BUSY, ..Node::DEFAULT });
        self.a11y_name(&n, name);
        in_body!(self, n.box_key, n, spinner, default_spinner(&SpinnerConfig { node: n }, name));
        n
    }

    /// Between frames: move an indeterminate `progress` bar or turn a `spinner` to time `now` through
    /// patches (no build, no layout; the view keeps its output). Returns whether it moves, so the
    /// host keeps drawing; `false` for a determinate bar, a stale handle or reduced motion.
    pub fn busy_step(&mut self, h: &Handle, now: f64) -> bool {
        if !self.live(h) || self.theme().tokens.reduced_motion {
            return false;
        }
        // a progress row has a track or a bar part (a spinner has dots only); its busy node is the track, else the row; a determinate
        // bar does not move
        let track = self.part_box(h.box_key, Part::TRACK);
        let bar = self.part_box(h.box_key, Part::BAR);
        if track.is_some() || bar.is_some() {
            let (Some(bar), busy) = (bar, track.unwrap_or(*h)) else { return false };
            if self.node_mut(&busy).a11y & st::BUSY == 0 {
                return false;
            }
            let r = self.rect(&busy);
            let w = r.w * 0.3;
            let p = (now.rem_euclid(1.4) / 1.4) as f32;
            let x = if self.rtl() { r.x + r.w - p * (r.w + w) } else { r.x - w + p * (r.w + w) };
            return self.patch_rect(&bar, Rect::new(x, r.y, w, r.h));
        }
        let t = self.theme();
        let (dim, lead) = (t.spinner.idle, t.spinner.checked);
        let at = (now * 10.0).rem_euclid(8.0) as i64;
        let mut moved = false;
        for i in 0..8 {
            let Some(dot) = self.part_box(h.box_key, Part::dot(i as u8)) else { return moved };
            let behind = (at - i).rem_euclid(8) as f32;
            moved |= self.patch_look(&dot, &lead.lerp(&dim, (behind / 3.0).min(1.0)));
        }
        moved
    }

    /// A row of tabs, the one in use underlined: `count` tabs, tab `i` being `tab(i)` (the C ABI's texts without a copy). One Tab stop
    /// (the bar); Left and Right (Home, End) switch (from no tab in use, Right takes the first and Left the last), a press on a tab
    /// chooses it. A tab is keyed as `item_key` keys a segment: by its id, else by its place in a separate namespace, never by its
    /// text. `selected` is the tab in use (`None`: none is; past the tabs: the last); the bar is named `name`, `variant` is given to the
    /// tabs' bodies and each tab's detail is drawn beside its text by the built-in body. With `focus_on_click` a press (not a secondary
    /// one) on a tab focuses the bar on the press itself, whether or not the caller applies the choice. Reports the bar's response, the
    /// tab in use after this frame's input and the tab a secondary press began on.
    #[allow(clippy::too_many_arguments)]
    pub fn tabs<'a>(&mut self, key: &str, name: &str, count: usize, tab: &dyn Fn(usize) -> Segment<'a>, selected: Option<usize>, variant: u16, focus_on_click: bool) -> (Response, Option<usize>, Option<usize>) {
        if variant != 0 { self.set_variant(BodyKind::tabs as u32, variant); }
        let rh = self.row_px() as f32;
        let bar_look = self.theme().header.idle;
        let body = body_of!(self, tabs);
        let bar = self.node_ref(key, &Node { flags: Flags::FOCUSABLE | Flags::TAB_ONLY, h: Size::Px(rh), axis: Axis::Row, padding: Edges { l: self.space(2), r: self.space(2), ..Edges::ZERO }, gap: self.space(2), look: bar_look, role: Role::TabList, ..Node::DEFAULT });
        self.a11y_name(&bar, name);
        let s = self.signal_owned(&bar);
        let mut sel = selected.filter(|_| count > 0).map(|i| i.min(count - 1));
        pressed(&s.keys, &mut |k| {
            if count == 0 { return; }
            match k {
                Key::Home => sel = Some(0),
                Key::End => sel = Some(count - 1),
                k => {
                    let by = self.along(k);
                    if by != 0 { sel = Some(sel.map_or(if by > 0 { 0 } else { count - 1 }, |i| wrap(i, by, count))); }
                }
            }
        });
        let saved = self.enter(&bar);
        let (before, mut old, mut focus, mut secondary) = (sel, None, false, None);
        for i in 0..count {
            let tab = tab(i);
            let n = self.node_keyed(item_key(bar.box_key, tab.id, i), Node { flags: ROW_FLAGS, h: Size::Grow, role: Role::Tab, ..Node::DEFAULT });
            let ts = self.signal_owned(&n);
            if ts.clicked {
                sel = Some(i);
            }
            let right = ts.button == Some(input::MouseButton::RightButton);
            focus |= ts.pressed && !right;
            if ts.pressed && right { secondary = Some(i); }
            let state = State { hovered: ts.hovered, pressed: ts.held, ..State::default() };
            if before == Some(i) {
                old = Some((n, state));
            }
            let config = TabConfig { state: State { checked: sel == Some(i), ..state }, index: i, node: n };
            self.set_role(&n, Role::Tab, st::SELECTABLE | if sel == Some(i) { st::SELECTED } else { 0 });
            self.a11y_name(&n, tab.name.unwrap_or(tab.text));
            in_item!(flat; self, body, n.box_key, n, default_tab(&config, tab.text, tab.detail));
        }
        // a click on a later tab unselects the earlier one in this frame, not the next
        if let Some((n, state)) = old.filter(|_| sel != before) {
            // ponytail: a registered body's old tab keeps its look for the click's frame; the built-in look is set again here
            if body.is_none() {
                let look = *self.theme().tab.pick(state);
                self.set_look(&n, look, state);
            }
            self.set_role(&n, Role::Tab, st::SELECTABLE);
        }
        self.leave(saved);
        if focus_on_click && focus && !self.focused(&bar) { self.focus_box(&bar); }
        if variant != 0 { self.set_variant(BodyKind::tabs as u32, 0); }
        let mut response = Response::of(&s, self.focused(&bar), bar);
        response.secondary |= secondary.is_some();
        (response, sel, secondary)
    }

    /// A segmented control keyed by `key` (hashed under the parent's seed, duplicates in order), the strip named `name` to accessibility
    /// and each option by its `Segment` (its name, else its text), varied by `opts`; reports what happened to the strip, the choice
    /// (`None`: no option chosen) and the option a secondary press began on.
    pub fn segmented_opts(&mut self, key: &str, name: &str, options: &[Segment], selected: Option<usize>, opts: &SegmentedOpts) -> (Response, Option<usize>, Option<usize>) {
        self.segmented_options(key, name, options.len(), &|i| options[i], selected, opts)
    }

    /// The one body of `segmented_opts`: `count` options, option `i` being `option(i)` (the C ABI's texts without a copy).
    ///
    /// A radio group: each option is a Radio that assistive technology can focus (FOCUSABLE, with the Focus action); only the chosen
    /// option (the first with none chosen) is a Tab stop. Left and Right on the focused option move the focus and the choice to its
    /// neighbour, Space or Enter chooses it, a press chooses the option under it without focusing it. When the choice changes while an
    /// option holds the focus (a key, or the caller's `selected`), the focus moves to the new choice; when the options shrink below the
    /// focused one, the focus moves to the Tab stop (the strip itself when no option is left). `selected` past the options is clamped to
    /// the last. The response is the strip's, with `clicked` (and `activated`) for a press or key on an option, `hovered`, `held` and
    /// `focused` for the strip or any option and `secondary` for a secondary press on an option, whose index is the third value. With `opts.focus_on_click` a press on an
    /// option also focuses it on the press itself (the chosen option, so the Tab stop), and the focus stays there when the caller does not
    /// apply the choice; the focus follows only a change of the `selected` the caller passes.
    #[allow(clippy::too_many_arguments)]
    pub fn segmented_options<'a>(&mut self, key: &str, name: &str, count: usize, option: &dyn Fn(usize) -> Segment<'a>, selected: Option<usize>, opts: &SegmentedOpts) -> (Response, Option<usize>, Option<usize>) {
        let enabled = opts.enabled && !self.scope_disabled();
        self.require_building();
        let unique = self.unique_key_repeated(key_of(self.current_seed(), key));
        let key = unique.0;
        let ch = opts.h.filter(|h| *h > 0.0).unwrap_or_else(|| self.control_h());
        let rh = self.row_px() as f32;
        let (side, gap) = (self.space(2), self.space(1));
        let line = self.node_unique(unique, &Node { flags: Flags::NONE, h: Size::Px(rh.max(ch)), axis: Axis::Row, align: Align::Center, padding: Edges { l: side, r: side, ..Edges::ZERO }, gap, role: Role::Unknown, ..Node::DEFAULT });
        let body = body_of!(self, segmented);
        let saved = self.enter(&line);
        let strip_w = match opts.w { Some(w) => Size::Px(w), None if opts.grow => Size::Grow, None => Size::Fit };
        let grow = opts.grow || opts.w.is_some();
        let strip = self.node_keyed(int_key(key, 0), Node { flags: Flags::NONE, w: strip_w, h: Size::Px(ch), axis: Axis::Row, role: Role::Group, ..Node::DEFAULT });
        self.a11y_name(&strip, shown(name));
        let s = self.signal(&strip);
        let mut sel = selected.filter(|_| count > 0).map(|i| i.min(count - 1));
        // the caller's choice, before this frame's keys and presses move it
        let stored = sel;
        // the option holding the focus, if one does: its arrows move the focus and the choice
        let focus = self.focus_key();
        // no focus (key 0) is held by none: the options are read only to find one that holds it
        let held = if focus == 0 { None } else { (0..count).find(|i| item_key(strip.box_key, option(*i).id, *i) == focus) };
        let mut at = held;
        if let (true, Some(from)) = (enabled, held) {
            for (event, _) in self.key_events_of(focus) {
                if let Event::KeyPressed(k) = event {
                    let by = self.along(*k);
                    if by != 0 { let to = clamp_step(at.unwrap_or(from), by, count); (at, sel) = (Some(to), Some(to)); }
                }
            }
        }
        // retained: the caller's choice last frame (line) and the option that held the focus (strip); a focus on an option that is gone, or a
        // choice the caller moved while an option held the focus, goes to the Tab stop; a press or key the caller did not apply (its choice
        // stays) leaves the focus on the option it moved to
        let code = stored.map_or(-1, |i| i as i64);
        let moved = held.is_some() && self.state(&line, code) != code;
        let lost = held.is_none() && focus != 0 && self.state(&strip, 0) == focus;
        let follow = enabled && (moved || lost);
        let states = opts.states.map(|mut s| { self.resolve_states(&mut s); s });
        let (mut clicked, mut by_key, mut secondary, mut tab) = (false, false, None, None);
        let (mut hovered, mut pressed) = (s.hovered, s.held);
        let inner = self.enter(&strip);
        for i in 0..count {
            let option = option(i);
            let n = self.node_keyed(item_key(strip.box_key, option.id, i), Node { flags: if enabled { ROW_FLAGS } else { Flags::BLOCKING }, h: Size::Grow, role: Role::Radio, ..Node::DEFAULT });
            let ns = self.signal(&n);
            (hovered, pressed) = (hovered || ns.hovered, pressed || ns.held);
            if enabled && ns.clicked {
                (sel, clicked, by_key) = (Some(i), true, !ns.released);
            }
            if enabled && ns.pressed && ns.button == Some(input::MouseButton::RightButton) { secondary = Some(i); }
            let focused = enabled && (if follow { i == sel.unwrap_or(0) } else { at == Some(i) } || (opts.focus_on_click && ns.pressed && ns.button != Some(input::MouseButton::RightButton)));
            let state = State { hovered: enabled && ns.hovered, pressed: enabled && ns.held, checked: sel == Some(i), disabled: !enabled, focused };
            let config = SegmentConfig { state, index: i, selected: sel, stored, group: strip.box_key, node: n, grow, padding: opts.padding, min_segment: opts.min_segment, line, strip };
            let at_focusable = if enabled { st::FOCUSABLE } else { 0 };
            self.set_role(&n, Role::Radio, st::CHECKABLE | at_focusable | if sel == Some(i) { st::CHECKED } else { 0 });
            self.a11y_name(&n, option.name.unwrap_or(option.text));
            in_item!(flat; self, body, n.box_key, n, default_segment(&config, states.as_ref(), option.text));
            if !enabled { self.node_new(&n).state.disabled = true; }
            if enabled && i == sel.unwrap_or(0) { tab = Some(n); }
            if focused && !self.focused(&n) { self.focus_box(&n); }
        }
        // the one Tab stop: the chosen option (the first with none)
        if let Some(n) = tab { self.node_new(&n).flags |= Flags::FOCUSABLE | Flags::TAB_ONLY; }
        if lost && count == 0 { self.focus_box(&strip); }
        self.leave(inner);
        self.leave(saved);
        if !enabled { self.node_new(&strip).state.disabled = true; }
        let now = self.focus_key();
        let focused = now != 0 && (0..count).any(|i| item_key(strip.box_key, option(i).id, i) == now);
        self.set_state(&line, code);
        self.set_state(&strip, if focused { now } else { 0 });
        let response = Response { clicked, hovered, held: pressed, focused, activated: clicked && by_key, secondary: secondary.is_some(), handle: strip };
        (response, sel, secondary)
    }

    /// Menu rows in an anchored sheet below `anchor`, `current` lit: the row clicked, if any.
    fn menu_sheet<'a>(&mut self, anchor: Rect, width: f32, count: usize, items: &dyn Fn(usize) -> Segment<'a>, current: Option<usize>) -> Option<usize> {
        let t = self.theme();
        let sheet = t.popup.idle;
        let body = body_of!(self, menu_item);
        let rh = self.row_px() as f32;
        let place = Placement { side: Side::Below, align: Along::Start, gap: 2.0, margin: 4.0, flip: true };
        let mut picked = None;
        let menu = self.anchored_label("menu", anchor, place, Size::Px(width), Size::Fit, &mut |ui| {
            for i in 0..count {
                let item = items(i);
                let n = ui.node_keyed(item_key(ui.current_seed(), item.id, i), Node { flags: ROW_FLAGS, h: Size::Px(rh), role: Role::MenuItem, ..Node::DEFAULT });
                let s = ui.signal_owned(&n);
                if s.clicked {
                    picked = Some(i);
                }
                ui.a11y_name(&n, item.name.unwrap_or(item.text));
                let config = MenuItemConfig { state: State { hovered: s.hovered, pressed: s.held, checked: current == Some(i), ..State::default() }, index: i, node: n };
                in_item!(flat; ui, body, n.box_key, n, default_menu_item(&config, item.text));
            }
        });
        let n = self.node_mut(&menu);
        (n.look, n.padding, n.role, n.flags) = (sheet, Edges { t: 4.0, b: 4.0, ..Edges::ZERO }, Role::Menu, Flags::BLOCKING);
        picked
    }

    /// Whether this frame has a press outside `a` and `b`.
    fn pressed_outside(&self, a: Rect, b: Rect) -> bool {
        self.frame_events().iter().any(|e| matches!(e, Event::PointerPressed(_, p) if { let p = (p.0 as f32, p.1 as f32); !a.contains(p) && !b.contains(p) }))
    }

    /// The dropdown over `count` items, item `i` being `item(i)` (its menu row keyed as `pick_radio` keys its row); `selected` none
    /// shows no option on the button (opening the menu then lights the first). Reports the button's response, the choice and the
    /// menu row a secondary press began on (none: the menu rows take primary presses only).
    pub fn pick_dropdown<'a>(&mut self, key: &str, text: &str, count: usize, item: &dyn Fn(usize) -> Segment<'a>, selected: Option<usize>, variant: u16) -> (Response, Option<usize>, Option<usize>) {
        if variant != 0 { self.set_variant(BodyKind::dropdown as u32, variant); }
        let key = key_of(self.current_seed(), key);
        let row = self.kit_line(key, Flags::NONE, Role::Unknown);
        let rh = self.row_px() as f32;
        let ch = self.control_h();
        let t = self.theme();
        let label_look = t.row.idle;
        let saved = self.enter(&row);
        self.text_leaf(int_key(key, 0), rh, label_look, text);
        let button = self.node_keyed(int_key(key, 1), Node { flags: STOP_FLAGS, h: Size::Px(ch), axis: Axis::Row, align: Align::Center, gap: 4.0, padding: Edges { l: 4.0, r: 6.0, ..Edges::ZERO }, role: Role::ComboBox, ..Node::DEFAULT });
        let s = self.signal_owned(&button);
        let mut sel = selected.filter(|_| count > 0).map(|i| i.min(count - 1));
        // the open menu's lit row, or -1 closed
        let mut lit = self.state(&button, -1);
        let mut pick = false;
        let by_key = s.keys.iter().any(|e| matches!(e, Event::KeyPressed(Key::Enter | Key::Space | Key::KeyChar(' '))));
        pressed(&s.keys, &mut |k| match k {
            Key::ArrowDown if lit < 0 => lit = sel.unwrap_or(0) as i64,
            Key::ArrowDown => lit = clamp_step(lit as usize, 1, count) as i64,
            Key::ArrowUp if lit >= 0 => lit = clamp_step(lit as usize, -1, count) as i64,
            Key::Escape => lit = -1,
            _ => {}
        });
        if s.clicked {
            if by_key && lit >= 0 {
                pick = true;
            } else {
                lit = if lit < 0 { sel.unwrap_or(0) as i64 } else { -1 };
            }
        }
        if pick && (lit as usize) < count {
            sel = Some(lit as usize);
            lit = -1;
        }
        let anchor = self.rect(&button);
        if lit >= 0 {
            let menu_rect = self.box_of_key(key_of(button.box_key, "menu")).map_or(anchor, |m| self.rect(&m));
            if !s.clicked && self.pressed_outside(anchor, menu_rect) {
                lit = -1;
            }
        }
        let open = lit >= 0;
        self.set_role(&button, Role::ComboBox, st::EXPANDABLE | if open { st::EXPANDED } else { 0 });
        self.a11y_name(&button, text);
        let inner = self.enter(&button);
        let config = DropdownConfig { state: State { hovered: s.hovered, pressed: s.held, focused: self.focused(&button), ..State::default() }, open, key, button };
        in_place!(self, button.box_key, dropdown, default_dropdown(&config, sel.map_or("", |i| item(i).text)));
        if open {
            if let Some(i) = self.menu_sheet(anchor, anchor.w.max(80.0), count, item, Some(lit as usize)) {
                sel = Some(i);
                lit = -1;
            }
        }
        self.leave(inner);
        self.leave(saved);
        self.set_state(&button, lit);
        if variant != 0 { self.set_variant(BodyKind::dropdown as u32, 0); }
        (Response::of(&s, self.focused(&button), button), sel, None)
    }

    /// `menu_bar`, also reporting a dismissal: a menu open when the frame began closed without a pick (Escape, a press outside, or
    /// its title clicked again).
    pub fn menu_bar_result<'a>(&mut self, label: &str, count: usize, entry: &dyn Fn(usize) -> (&'a str, usize), item: &dyn Fn(usize, usize) -> &'a str) -> (Option<(usize, usize)>, bool) {
        let rh = self.row_px() as f32;
        let t = self.theme();
        let bar_look = t.menubar.idle;
        let body = body_of!(self, menu_bar);
        let bar = self.node_ref(label, &Node { flags: Flags::FOCUSABLE | Flags::TAB_ONLY | Flags::BLOCKING, h: Size::Px(rh), axis: Axis::Row, padding: Edges { l: 4.0, ..Edges::ZERO }, look: bar_look, role: Role::MenuBar, ..Node::DEFAULT });
        self.a11y_name(&bar, shown(label));
        let s = self.signal_owned(&bar);
        // state: menu * 65536 + (lit + 1) * 2 + open
        let packed = self.state(&bar, 0);
        let (mut menu, mut lit, mut open) = ((packed >> 16) as usize, ((packed & 0xffff) >> 1) as i64 - 1, packed & 1 == 1);
        let was_open = open;
        let menus = count;
        let count = |m: usize| if m < menus { entry(m).1 } else { 0 };
        let mut picked = None;
        let by_key = s.keys.iter().any(|e| matches!(e, Event::KeyPressed(Key::Enter | Key::Space | Key::KeyChar(' '))));
        pressed(&s.keys, &mut |k| match k {
            Key::ArrowLeft | Key::ArrowRight => {
                menu = wrap(menu, self.along(k), menus);
                lit = -1;
            }
            Key::ArrowDown if !open => (open, lit) = (true, 0),
            Key::ArrowDown => lit = clamp_step(lit.max(0) as usize, 1, count(menu)) as i64,
            Key::ArrowUp if open => lit = clamp_step(lit.max(0) as usize, -1, count(menu)) as i64,
            Key::Escape => open = false,
            _ => {}
        });
        if s.clicked && by_key {
            if open && lit >= 0 {
                picked = Some((menu, lit as usize));
                open = false;
            } else {
                (open, lit) = (true, 0);
            }
        }
        let saved = self.enter(&bar);
        let mut anchor = Rect::new(0.0, 0.0, 0.0, 0.0);
        for i in 0..menus {
            let title = entry(i).0;
            let n = self.node_keyed(int_key(bar.box_key, i as i64), Node { flags: ROW_FLAGS, h: Size::Grow, role: Role::MenuItem, ..Node::DEFAULT });
            let ns = self.signal_owned(&n);
            if ns.clicked {
                (open, menu, lit) = (!(open && menu == i), i, -1);
            } else if ns.hovered && open && menu != i {
                (menu, lit) = (i, -1);
            }
            if i == menu {
                anchor = self.rect(&n);
            }
            let on = open && menu == i;
            let config = MenuItemConfig { state: State { hovered: ns.hovered, pressed: ns.held, checked: on, ..State::default() }, index: i, node: n };
            self.set_role(&n, Role::MenuItem, st::EXPANDABLE | if on { st::EXPANDED } else { 0 });
            self.a11y_name(&n, title);
            in_item!(flat; self, body, n.box_key, n, default_menu_title(&config, title));
        }
        if open {
            let menu_rect = self.box_of_key(key_of(bar.box_key, "menu")).map_or(anchor, |m| self.rect(&m));
            if self.pressed_outside(self.rect(&bar), menu_rect) {
                open = false;
            }
        }
        if open {
            if menu < menus {
                let at = menu;
                if let Some(i) = self.menu_sheet(anchor, 160.0, count(at), &|i| Segment::new(item(at, i)), (lit >= 0).then_some(lit as usize)) {
                    picked = Some((menu, i));
                    open = false;
                }
            }
        }
        self.leave(saved);
        self.set_state(&bar, ((menu as i64) << 16) | ((lit + 1) << 1) | open as i64);
        (picked, was_open && !open && picked.is_none())
    }

    /// The kit tooltip for node `h` while the pointer rests on it.
    pub fn tooltip(&mut self, h: &Handle, text: &str) {
        // keyed by its owner: the same text on two boxes is two waits, and a changed text keeps the wait
        if self.live(h) && self.signal_owned(h).hovered {
            self.tooltip_sheet(None, h.box_key, text);
        }
    }

    /// A box laid out by `node` whose content (`f`'s children, flowing on `node`'s axis) scrolls on both axes, with the theme's
    /// thumbs. The scroll view adds its scroll flags, look, thumbs, row scroll step and role, and keeps its one-point frame inset
    /// inside `node`'s padding. Returns its node.
    pub fn scroll_view(&mut self, label: &str, node: Node, f: &mut dyn FnMut(&mut Ui)) -> Handle {
        let bar = self.thumb();
        let look = self.theme().scroll_view.idle;
        let rh = self.row_px() as f32;
        let p = node.padding;
        let padding = Edges { l: p.l + 1.0, t: p.t + 1.0, r: p.r + 1.0, b: p.b + 1.0 };
        let n = self.node_ref(label, &Node { flags: node.flags | Flags::SCROLL | Flags::SCROLL_X | Flags::CLIP | Flags::BLOCKING, look, bar: Some(bar).into(), scroll_step: rh, padding, role: Role::ScrollArea, ..node });
        in_body!(self, n.box_key, n, scroll_view, default_scroll(&ScrollConfig { node: n }, f));
        n
    }

    /// The row being built by `ui.list`: its kit row look on a clickable child that fills it.
    fn virtual_row(&mut self, states: &States, checked: bool, cursor: bool, inset: f32, role: Role, a11y: u32) -> (Handle, Signal, State) {
        let rh = self.row_px() as f32;
        if let Some(row) = self.box_of_key(self.current_seed()) {
            self.set_role(&row, Role::Unknown, 0);
        }
        let n = self.node("row".into(), Node { flags: ROW_FLAGS, h: Size::Px(rh), axis: Axis::Row, align: Align::Center, padding: Edges { l: inset, r: self.space(2), ..Edges::ZERO }, gap: 4.0, role, a11y, ..Node::DEFAULT });
        let s = self.signal_owned(&n);
        let state = State { hovered: s.hovered, pressed: s.held, checked, focused: cursor, ..State::default() };
        self.style(&n, states, state);
        (n, s, state)
    }

    /// Keys of a focused list over `count` rows: the new cursor and whether Enter or Space came.
    fn list_keys(&self, keys: &[Event], cursor: usize, count: usize, page: i64) -> (usize, bool, bool) {
        let (mut c, mut enter, mut space) = (cursor, false, false);
        pressed(keys, &mut |k| match k {
            Key::ArrowDown => c = clamp_step(c, 1, count),
            Key::ArrowUp => c = clamp_step(c, -1, count),
            Key::PageDown => c = clamp_step(c, page, count),
            Key::PageUp => c = clamp_step(c, -page, count),
            Key::Home => c = 0,
            Key::End => c = count.saturating_sub(1),
            Key::Enter => enter = true,
            Key::Space | Key::KeyChar(' ') => space = true,
            _ => {}
        });
        (c, enter, space)
    }

    /// Keep row `row` of the virtual list `body` in view.
    fn reveal_row(&mut self, body: &Handle, row: usize) {
        let rh = self.row_px() as f32;
        let (top, h) = (self.scroll_offset(body), self.rect(body).h);
        let y = row as f32 * rh;
        if y < top {
            self.set_scroll_offset(body, y);
        } else if y + rh > top + h && h > 0.0 {
            self.set_scroll_offset(body, y + rh - h);
        }
    }

    /// A tree over `nodes` (preorder), virtualised: the first rows of `visible` (a scratch the caller
    /// keeps, at least a row per node) list the nodes shown, the expanded (`open`, not 0) nodes'
    /// children under them, each row indented 14 points a level with a chevron when it has children.
    /// A click selects (`selected`, a node index), a click on the chevron or a double click expands or
    /// collapses. Keys: Up and Down move, Right expands or steps in, Left collapses or steps to the
    /// parent, Enter activates, Space toggles. Returns the node a double click or Enter activated and
    /// the number of rows shown.
    /// `cell(ui, node)` builds the children of the row that shows tree node `node`, after the disclosure mark, inside the row node
    /// (`Ui::item`); `item_text` puts the kit's text there and names the row.
    /// Keyed by `key` (hashed under the parent's seed).
    pub fn tree(&mut self, key: &str, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], cell: &mut dyn FnMut(&mut Ui, usize)) -> TreeResponse {
        let (activated, shown) = self.tree_label(key, nodes, open, selected, visible, cell);
        TreeResponse { activated, shown }
    }

    fn tree_label(&mut self, label: &str, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], cell: &mut dyn FnMut(&mut Ui, usize)) -> (Option<usize>, usize) {
        assert!(visible.len() >= nodes.len(), "tree: `visible` holds at least a row per node");
        // the visible rows are re-derived each build (specification/pxui.md, designed limits)
        let flatten = |visible: &mut [u32], open: &[u8]| flatten_tree(nodes, open, visible);
        let mut rows_shown = flatten(visible, open);
        let rh = self.row_px() as f32;
        let frame = self.node_ref(label, &Node { flags: Flags::FOCUSABLE | Flags::CLIP, w: Size::Grow, h: Size::Grow, look: self.theme().scroll_view.idle, padding: Edges::all(1.0), ..Node::DEFAULT });
        let s = self.signal_owned(&frame);
        let focused = self.focused(&frame);
        let page = (self.rect(&frame).h / rh).floor().max(1.0) as i64;
        let cursor_node = self.state(&frame, 0).max(0) as usize;
        let cursor = visible[..rows_shown].iter().position(|v| *v as usize == cursor_node).unwrap_or(0);
        let (mut c, enter, space) = self.list_keys(&s.keys, cursor, rows_shown, page);
        let mut activated = None;
        if let Some(&node) = visible[..rows_shown].get(c) {
            let node = node as usize;
            let is_open = open.get(node).copied().unwrap_or(0) != 0;
            let mut toggle = space && nodes[node].children != 0;
            pressed(&s.keys, &mut |k| match self.along(k) {
                1 if nodes[node].children != 0 && !is_open => toggle = true,
                1 if nodes[node].children != 0 => c = clamp_step(c, 1, rows_shown),
                -1 if nodes[node].children != 0 && is_open => toggle = true,
                -1 => c = visible[..c].iter().rposition(|v| nodes[*v as usize].depth < nodes[node].depth).unwrap_or(c),
                _ => {}
            });
            if toggle {
                if let Some(o) = open.get_mut(node) {
                    *o = (*o == 0) as u8;
                }
                rows_shown = flatten(visible, open);
            }
            if enter {
                activated = Some(node);
            }
            if c != cursor {
                *selected = visible[..rows_shown].get(c).map(|v| *v as usize);
            }
        }
        let t = self.theme();
        let states = t.tree;
        let row_body = body_of!(self, tree);
        let saved = self.enter(&frame);
        let mut toggled = None;
        let rows: &[u32] = &visible[..rows_shown];
        let body = self.list("rows", rows_shown, rh, &mut |ui, i| {
            let node = rows[i] as usize;
            let (has, is_open) = (nodes[node].children != 0, open.get(node).copied().unwrap_or(0) != 0);
            let on = *selected == Some(node);
            let a11y = st::SELECTABLE | if on { st::SELECTED } else { 0 } | if has { st::EXPANDABLE | if is_open { st::EXPANDED } else { 0 } } else { 0 };
            let inset = ui.space(2) + 14.0 * nodes[node].depth as f32;
            let (n, rs, state) = ui.virtual_row(&states, on, focused && i == c, inset, Role::TreeItem, a11y);
            let config = TreeRowConfig { state, index: i, node, depth: nodes[node].depth as usize, has, open: is_open, row: n };
            let outer = (core::mem::replace(&mut ui.bodies.item, n), core::mem::replace(&mut ui.bodies.item_kind, ITEM_TREE));
            in_item!(ui, row_body, n.box_key, n, default_tree_row(&config, &mut |ui| cell(ui, node)));
            (ui.bodies.item, ui.bodies.item_kind) = outer;
            if has && ui.part_box(n.box_key, Part::DISCLOSURE).is_some_and(|d| ui.signal_owned(&d).clicked) {
                toggled = Some(node);
            }
            if rs.clicked {
                *selected = Some(node);
                c = i;
            }
            if rs.double_clicked && has {
                toggled = Some(node);
            } else if rs.double_clicked {
                activated = Some(node);
            }
        });
        self.leave(saved);
        if let Some(o) = toggled.and_then(|node| open.get_mut(node)) {
            *o = (*o == 0) as u8;
        }
        self.node_mut(&body).bar = Some(self.thumb()).into();
        self.set_role(&body, Role::Tree, 0);
        self.a11y_name(&body, shown(label));
        if c != cursor {
            self.reveal_row(&body, c);
        }
        let cursor_node = visible[..rows_shown].get(c).copied().unwrap_or(0);
        self.set_state(&frame, cursor_node as i64);
        (activated, rows_shown)
    }

    /// A modal dialog titled `title`: its title bar, `body`, then `buttons` at the end of a row, built through the dialog body and
    /// the button body. Return chooses the `default` button, Escape the `cancel` one (a press outside it and a lost window focus
    /// too, with `dismissed` set), a click the button clicked; a disabled button is none of these. `key` names the sheet.
    pub fn dialog(&mut self, key: &str, title: &str, buttons: &[DialogButton], style: &Dialog, body: &mut dyn FnMut(&mut Ui)) -> Chosen {
        let mut chosen = None;
        let enter = self.key_pressed(Key::Enter, None);
        let shown_now = self.modal_named(&Overlay::DEFAULT.w_px(style.w), key, None, &mut |ui| {
            let owner = ui.current_seed();
            in_place!(ui, owner, dialog, default_dialog(title, buttons, body));
            // the heading and the buttons are the body's nodes: their presses are read here, and a role or a name only where the body
            // left it unset (a body's own role, states and name stand)
            if let Some(head) = ui.part_box(owner, Part::TITLE) {
                ui.default_role(&head, Role::Heading, shown(title));
            }
            for (i, b) in buttons.iter().enumerate() {
                if let Some(part) = ui.part_box(owner, Part::button(i as u8)) {
                    ui.default_role(&part, Role::Button, shown(b.text));
                    if b.enabled && chosen.is_none() && ui.signal(&part).clicked {
                        chosen = Some(i);
                    }
                }
            }
        }).is_some();
        // the sheet is named by the title, not by the key it is found by
        if shown_now {
            if let Some(sheet) = self.box_of_key(key_of(self.current_seed(), key)) {
                self.a11y_name(&sheet, shown(title));
            }
        }
        let first = |pick: fn(&DialogButton) -> bool| buttons.iter().position(|b| pick(b) && b.enabled);
        if !shown_now {
            return Chosen { chosen: first(|b| b.cancel), chosen_id: None, dismissed: true };
        }
        if enter && chosen.is_none() {
            chosen = first(|b| b.primary);
        }
        Chosen { chosen, chosen_id: None, dismissed: false }
    }

    /// Toasts stacked in the window's lower end corner (the newest lowest), 12 points from its
    /// edges, the stack keyed by `key`. Each goes `seconds` after it first showed: the engine keeps that time by the toast's
    /// id and asks for the frame (`repaint_after`), so the host needs no timer. Returns a toast
    /// clicked away or expired (one a frame; the caller drops it).
    pub fn toasts(&mut self, key: &str, items: &[Toast], seconds: f64) -> Chosen {
        let chosen = self.toast_stack(key, items, seconds);
        Chosen { chosen, chosen_id: chosen.and_then(|i| items.get(i).map(|t| t.id)), dismissed: false }
    }

    /// The toast clicked away or expired this frame (`toasts`' index).
    fn toast_stack(&mut self, key: &str, items: &[Toast], seconds: f64) -> Option<usize> {
        let label = key;
        if items.is_empty() {
            return None;
        }
        let ((w, h), m) = (self.view_size(), self.space(2));
        let x = if self.rtl() { m } else { w - m };
        let place = Placement { side: Side::Above, align: if self.rtl() { Along::Start } else { Along::End }, gap: 0.0, margin: m, flip: false };
        let mut gone = None;
        let now = self.now();
        let stack = self.anchored_label(label, Rect::new(x, h - m, 0.0, 0.0), place, Size::Px(260.0), Size::Fit, &mut |ui| {
            for (i, item) in items.iter().enumerate() {
                // keyed by the item's id, so dropping one keeps the others' times
                let n = ui.node_keyed(int_key(ui.current_seed(), item.id), Node { flags: ROW_FLAGS | Flags::FIT_TEXT, role: Role::StatusBar, ..Node::DEFAULT });
                let s = ui.signal_owned(&n);
                ui.a11y_name(&n, item.text);
                let config = ToastConfig { state: State { hovered: s.hovered, ..State::default() }, node: n };
                in_body!(flat; ui, n.box_key, n, toasts, default_toast(&config, item.text));
                let born = f64::from_bits(ui.state(&n, now.to_bits() as i64) as u64);
                ui.set_state(&n, born.to_bits() as i64);
                if s.clicked || now >= born + seconds {
                    gone = gone.or(Some(i));
                } else {
                    ui.repaint_at_time(born + seconds);
                }
            }
        });
        self.node_mut(&stack).gap = 8.0;
        gone
    }

    /// An icon button as the wrapper declares it: `key` is hashed under the parent's seed, `icon` is what the body draws (a glyph, or
    /// the app's own drawing in the look's text ink), `name` the accessible name and `icon_button` the variation (see `IconButton`).
    pub fn icon_button(&mut self, key: &str, icon: Icon, name: &str, button: &IconButton) -> Response {
        let mut glyph = [0u8; 4];
        match icon {
            Icon::Glyph(ch) => self.icon_button_text(key, ch.encode_utf8(&mut glyph), None, name, button, None),
            Icon::Draw(f, data) => self.icon_button_text(key, "", Some((f, data)), name, button, None),
        }
    }

    /// `icon_button` with the glyph as any string (the C ABI's) beside an optional drawing, which wins; `signal`, when given, receives
    /// the box's signal as it was read for it, without its key events (`key_events_ref`): the C ABI's signal without a second read.
    pub fn icon_button_text(&mut self, key: &str, glyph: &str, draw: Option<(IconDraw, [usize; 4])>, name: &str, button: &IconButton, signal: Option<&mut Signal>) -> Response {
        let enabled = button.enabled && !self.scope_disabled();
        let flags = if enabled { STOP_FLAGS } else { Flags::BLOCKING | Flags::TAB_ONLY };
        let b = self.node_ref(key, &Node { flags, role: Role::Button, ..Node::DEFAULT });
        let mut s = self.signal(&b);
        // a disabled button reports no click from any input (a press begun while it was enabled, an assistive technology's press)
        s.clicked &= enabled;
        let focused = self.focused(&b);
        let state = State { hovered: s.hovered, pressed: s.held, focused, disabled: !enabled, checked: button.selected == Some(true) };
        self.a11y_name(&b, shown(name));
        let states = match button.states { Some(mut states) => { self.resolve_states(&mut states); states }, None => self.theme().icon_button };
        let ink = states.pick(state).text;
        let config = IconButtonConfig { state, node: b, w: button.w, h: button.h, press_scale: button.press_scale, ink, draw, press: button.press };
        in_body!(flat; self, b.box_key, b, icon_button, default_icon_button(&config, &states, name, glyph));
        self.node_new(&b).state.disabled = !enabled;
        if let Some(on) = button.selected { self.set_role(&b, Role::Button, st::CHECKABLE | if on { st::CHECKED } else { 0 }); }
        let response = Response::of(&s, focused, b);
        if let Some(out) = signal { *out = s; }
        response
    }

    /// A small label in the badge look keyed by `key` (hashed under the parent's seed), named `name` (empty: none), `text` its value
    /// (`patch_text` updates a count without a rebuild). Returns its node.
    pub fn badge(&mut self, key: &str, name: &str, text: &str) -> Handle {
        let n = self.node_ref(key, &Node { role: Role::Label, ..Node::DEFAULT });
        self.a11y_name(&n, name);
        self.a11y_value(&n, text, (0, 0));
        in_body!(flat; self, n.box_key, n, badge, default_badge(&BadgeConfig { node: n }, name, text));
        n
    }

    /// A search row: a well holding `icon`, the query (`placeholder` while empty) and, with a query, a
    /// clear button. A click focuses it and the text widgets' editing applies; Escape clears it.
    /// Returns the new query on the frames that change it, borrowed until the next call on the Ui. `traits` are the on-screen
    /// keyboard's while it is focused (`TextTraits::SEARCH`, or with another return key).
    pub fn search_field(&mut self, label: &str, icon: &str, value: &str, placeholder: &str, traits: pxui_types::TextTraits) -> Option<&str> {
        search_field_body(self, label, icon, value, placeholder, traits).then_some(self.returned.as_str())
    }

    /// An image `w` x `h` placed by `fit` (CSS `object-fit`) in the image look's frame, named `name` for accessibility. No key: every
    /// image is the n-th positional image of its scope, as labels are the n-th positional label.
    pub fn image(&mut self, name: &str, image: &Image, fit: Fit, w: Size, h: Size) -> Handle {
        self.require_building();
        let unique = self.unique_key_repeated(positional_key(self.current_seed(), Positional::Image));
        self.image_at(unique, name, image, fit, w, h)
    }

    /// An image keyed by `key` (a label or id under the current parent, as `node` keys it) and named `name` for accessibility only.
    pub fn image_keyed(&mut self, key: &str, name: &str, image: &Image, fit: Fit, w: Size, h: Size) -> Handle {
        self.require_building();
        let unique = self.unique_key(key_of(self.current_seed(), key));
        self.image_at(unique, name, image, fit, w, h)
    }

    fn image_at(&mut self, unique: (i64, Option<usize>), name: &str, image: &Image, fit: Fit, w: Size, h: Size) -> Handle {
        let look = self.theme().image.idle;
        let n = self.node_unique(unique, &Node { w, h, look, padding: Edges::all(1.0), role: Role::Image, ..Node::DEFAULT });
        self.a11y_name(&n, name);
        let image = image.clone();
        // the picture takes its frame's rounded corners: a round clip inside the frame's border
        let radius = look.radius.iter().fold(0.0f32, |a, b| a.max(*b)) - 1.0;
        self.canvas(&n, move |p, r| {
            let (clip, round) = (p.clip_rect, p.clip_radius);
            let inner = Rect::new(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0);
            if radius > 0.0 {
                (p.clip_rect, p.clip_radius) = (clip.intersect(inner), radius);
            }
            p.image_fit(inner.x, inner.y, inner.w, inner.h, &image, fit, Rgba::WHITE);
            (p.clip_rect, p.clip_radius) = (clip, round);
        });
        n
    }
}

// Method bodies outside `impl` blocks: each module is its own codegen unit, so the big bodies
// compile in parallel with the rest of the crate (rustc places methods with their type).
/// True when the query changed: the new one is in `returned`.
fn search_field_body(this: &mut Ui, label: &str, icon: &str, value: &str, placeholder: &str, traits: pxui_types::TextTraits) -> bool {
    let key = key_of(this.current_seed(), label);
    let row = this.kit_line(key, Flags::NONE, Role::Unknown);
    this.input_box = Some(row);
    let saved = this.enter(&row);
    let well = this.node_keyed(int_key(key, 0), Node::DEFAULT);
    let mut changed = false;
    let mut editor = |ui: &mut Ui| -> bool {
        let field = ui.node_keyed(int_key(key, 1), Node { flags: Flags::CLICKABLE | Flags::FOCUSABLE | Flags::BLOCKING | Flags::FOCUS_MARK, h: Size::Grow, inset: Edges { l: 2.0, ..Edges::ZERO }, role: Role::TextField, a11y: st::EDITABLE, ..Node::DEFAULT });
        ui.input_box = Some(field);
        let s = ui.signal_owned(&field);
        let focused = ui.focused(&field);
        let mut edit = None;
        if focused {
            ui.text_input_options = traits;
            // the frame's edit buffer, handed back after painting is queued: a focused query copies into no new string
            let buffer = core::mem::take(&mut ui.edit_work);
            let mut e = ui.load_text_edit_into(field.box_key, value, buffer);
            let r = ui.rect(&field);
            let shift = ui.press_shift(&field);
            ui.point_text_caret(None, &mut e, &s, shift, r.x + 2.0, r.x + r.w - 2.0);
            for (event, modifiers) in ui.key_events(&field) {
                if event == Event::KeyPressed(Key::Escape) {
                    e.text.clear();
                    (e.caret, e.anchor) = (0, 0);
                } else {
                    ui.edit_text_event(false, &mut e, &|_| true, &modifiers, &event);
                }
            }
            ui.save_text_edit(&e);
            if e.text != value {
                ui.returned.clear();
                ui.returned.push_str(&e.text);
                changed = true;
            }
            edit = Some(e);
        }
        if s.hovered {
            ui.request_cursor(Cursor::Text);
        }
        let t = ui.theme();
        let (states, hint) = (t.search, t.placeholder.idle);
        let look = *states.pick(State { focused, ..State::default() });
        // a focused field's edit holds the query, changed or not
        let current = edit.as_ref().map_or(value, |e| e.text.as_str());
        let filled = !current.is_empty();
        ui.a11y_name(&field, shown(label));
        ui.a11y_value(&field, current, (0, 0));
        ui.node_mut(&field).look = Look { text: if current.is_empty() { hint.text } else { look.text }, ..text_only(&look) };
        match edit {
            Some(edit) => {
                let font_size = ui.font_px();
                let marks = Some((look.selection, look.caret));
                let (caret, anchor) = (edit.caret, edit.anchor);
                // the edit text and composition travel in the painter (`paint_begin`), as a text field's do: no copy
                ui.canvas(&field, move |paint, rect| {
                    let r = crate::ui_kit::ints(rect);
                    let (text, composition) = (std::mem::take(&mut paint.edit_text), std::mem::take(&mut paint.composition));
                    crate::ui_text::paint_text_edit_ref(paint, None, false, 2.0, marks, r, crate::ui_kit::label_y(font_size, r.1, r.3), &composition, &text, caret, anchor);
                    (paint.edit_text, paint.composition) = (text, composition);
                });
                ui.edit_work = edit.text;
            }
            None => ui.text(&field, if value.is_empty() { placeholder } else { value }),
        }
        filled
    };
    let config = SearchFieldConfig { state: State { focused: this.focus_key() == int_key(key, 1), ..State::default() }, well, key };
    in_body!(this, key, well, search_field, default_search_field(&config, icon, &mut editor));
    if let Some(clear) = this.part_built(key, Part::CLEAR) {
        this.set_role(&clear, Role::Button, 0);
        this.a11y_name(&clear, "Clear");
        if this.signal_owned(&clear).clicked {
            this.returned.clear();
            changed = true;
        }
    }
    this.leave(saved);
    changed
}

/// The built-in search field: the 20-point well in the search look, the icon, the editing surface and, with a query, the 16-point clear
/// button (a multiplication sign) after it.
pub(crate) fn default_search_field(this: &mut Ui, c: &SearchFieldConfig, icon: &str, editor: &mut dyn FnMut(&mut Ui) -> bool) {
    let ch = this.control_h();
    let t = this.theme();
    let (states, hint) = (t.search, t.placeholder.idle);
    let n = this.node_new(&c.well);
    (n.h, n.axis, n.align, n.gap, n.padding) = (Size::Px(ch), Axis::Row, Align::Center, 4.0, Edges { l: 4.0, r: 2.0, ..Edges::ZERO });
    this.style(&c.well, &states, c.state);
    let glyph = this.text_leaf(int_key(c.key, 2), ch, hint, icon);
    this.node_mut(&glyph).look.text = hint.text;
    if editor(this) {
        let clear = this.part(Part::CLEAR, Node { flags: ROW_FLAGS, w: Size::Px(16.0), h: Size::Px(16.0), look: Look { text: hint.text, ..Look::NONE }, inset: Edges { l: 4.0, ..Edges::ZERO }, ..Node::DEFAULT });
        this.text(&clear, "\u{d7}");
    }
}

/// The built-in spinner: 16 points square, eight 3-point dots round a circle, the lead dot in the accent.
pub(crate) fn default_spinner(this: &mut Ui, c: &SpinnerConfig, _label: &str) {
    let (dim, lead) = (this.theme().spinner.idle, this.theme().spinner.checked);
    let n = this.node_new(&c.node);
    (n.w, n.h) = (Size::Px(16.0), Size::Px(16.0));
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        let at = (8.0 + 6.0 * a.sin() - 1.5, 8.0 - 6.0 * a.cos() - 1.5);
        this.part(Part::dot(i as u8), Node { w: Size::Px(3.0), h: Size::Px(3.0), at: Some(at).into(), look: if i == 0 { lead } else { dim }, ..Node::DEFAULT });
    }
}

/// The built-in badge: the text in the badge look on a 16-point pill as wide as the text and 4 points either side.
pub(crate) fn default_badge(this: &mut Ui, c: &BadgeConfig, _label: &str, text: &str) {
    let look = this.theme().badge.idle;
    let w = this.text_width(Some(crate::ui_core::font_px(&look.font, this.font_px())), text) + 8.0;
    let n = this.node_new(&c.node);
    (n.w, n.h, n.look, n.inset) = (Size::Px(w), Size::Px(16.0), look, Edges { l: 4.0, ..Edges::ZERO });
    this.text(&c.node, text);
}

/// An app drawing of an icon: paints into the 20-point square `rect` in `ink` (the look's text colour for the button's state). The
/// four words are the caller's (the C ABI's callback, its user data, its library state and the icon's word); a plain Rust drawing ignores them.
pub type IconDraw = fn(&mut Paint, Rect, crate::geom::Rgba, [usize; 4]);

/// What an icon button shows: a glyph of the icon font its look names, or the app's own drawing.
#[derive(Clone, Copy)]
pub enum Icon {
    Glyph(char),
    Draw(IconDraw, [usize; 4]),
}

/// How `icon_button` varies the kit's icon button; `DEFAULT` is the plain one.
pub struct IconButton {
    /// The look per state; `None` is the theme's `icon_button`.
    pub states: Option<States>,
    /// Width and height in points (`Px`) or filling the line (`Grow`); `Fit` is the kit's control square.
    pub w: Size,
    pub h: Size,
    /// A disabled button takes no press or focus, shows its `disabled` look and reports disabled to accessibility.
    pub enabled: bool,
    /// The scale the button's contents spring to while pressed (1.0: no animation, no view).
    pub press_scale: f32,
    /// The curves the scale runs in on press and back out on release; `None` is the kit's (`PRESS_IN`, `PRESS_OUT`). Read only
    /// during the call (the C ABI's are the caller's, borrowed for it).
    pub press: Option<&'static [Curve; 2]>,
    /// An on/off key (`IconButton::selected`): `Some(on)` shows its `checked` look when on and reads as CHECKABLE (CHECKED when on);
    /// `None` is a plain button.
    pub selected: Option<bool>,
}

impl IconButton {
    pub const DEFAULT: IconButton = IconButton { states: None, w: Size::Fit, h: Size::Fit, enabled: true, press_scale: 1.0, press: None, selected: None };
}

impl Ui {
    /// Paints the app's drawing of an icon button's `c` (when it has one) on `node`: its 20-point (`control_h`) square at the centre of
    /// the node's rectangle, in `c.ink`. For a body that builds its own plate; the built-in body calls it on its plate.
    pub fn icon_draw(&mut self, node: &Handle, c: &IconButtonConfig) {
        if let Some(draw) = c.draw { self.icon_draw_in(node, draw, c.ink); }
    }

    /// Paints the app's icon drawing `draw` on `node` in `ink`: its 20-point (`control_h`) square at the centre of the node's rectangle.
    /// A button's leading icon (`ButtonConfig::lead_draw`) and an icon button's drawing go through it.
    pub fn icon_draw_in(&mut self, node: &Handle, (f, data): (IconDraw, [usize; 4]), ink: crate::geom::Rgba) {
        let ch = self.control_h();
        self.draw(node, move |paint, rect| f(paint, Rect { x: rect.x + (rect.w - ch) / 2.0, y: rect.y + (rect.h - ch) / 2.0, w: ch, h: ch }, ink, data));
    }
}

/// The kit's curves of a pressed icon button's scale: a fast spring in, a slower one out.
pub const PRESS_IN: Curve = Curve::Spring { omega: 60.0 };
pub const PRESS_OUT: Curve = Curve::Spring { omega: 20.0 };

/// The built-in icon button: a square (the kit's 20 points, or the caller's size), the glyph centred. With a press scale below or above
/// 1 the plate and glyph sit in a view that springs to that scale while the button is held and back on release.
pub(crate) fn default_icon_button(this: &mut Ui, c: &IconButtonConfig, states: &States, _label: &str, icon: &str) {
    let ch = this.control_h();
    let tw = this.text_width(None, icon);
    let size = |s: Size| match s { Size::Fit => Size::Px(ch), s => s };
    let n = this.node_new(&c.node);
    (n.w, n.h) = (size(c.w), size(c.h));
    let look = *states.pick(c.state);
    let width = match c.w { Size::Px(w) => w, _ => ch };
    let inset = Edges { l: ((width - tw) / 2.0).floor(), ..Edges::ZERO };
    if c.press_scale == 1.0 {
        this.set_look(&c.node, look, c.state);
        let n = this.node_new(&c.node);
        n.look.ring = Ring::NONE;
        n.inset = inset;
        if c.draw.is_some() { this.icon_draw(&c.node, c); } else { this.text(&c.node, icon); }
        return;
    }
    let word = |color: crate::geom::Rgba| u32::from_le_bytes([color.r, color.g, color.b, color.a]) as i64;
    let bits = (c.state.hovered as i64) | (c.state.pressed as i64) << 1 | (c.state.focused as i64) << 2 | (c.state.disabled as i64) << 3 | (c.state.checked as i64) << 4;
    let art = c.draw.map_or(0, |(f, data)| (f as usize as i64) ^ (data[0] as i64).rotate_left(16) ^ (data[1] as i64).rotate_left(32) ^ (data[3] as i64).rotate_left(48));
    let mut revision = key_of(word(look.fill) | word(look.text) << 32 ^ art, icon);
    revision = key_of(revision ^ word(look.border) ^ bits << 40 ^ (width.to_bits() as i64) << 8 ^ look.font.size.rotate_left(20) ^ i64::from(look.font.family) << 52, "icon_button");
    let saved = this.enter(&c.node);
    let state = c.state;
    let visual = this.view("visual", revision as u64, Size::Grow, Size::Grow, &mut |ui| {
        let surface = ui.node("surface".into(), Node { w: Size::Grow, h: Size::Grow, ..Node::DEFAULT });
        ui.set_look(&surface, look, state);
        let n = ui.node_new(&surface);
        n.look.ring = Ring::NONE;
        n.inset = inset;
        if c.draw.is_some() { ui.icon_draw(&surface, c); } else { ui.text(&surface, icon); }
    });
    this.leave(saved);
    let held = i64::from(c.state.pressed);
    if this.state(&visual, 0) != held {
        this.set_state(&visual, held);
        let scale = if c.state.pressed { c.press_scale } else { 1.0 };
        this.animate(&visual, Target::Scale { x: scale, y: scale, anchor: (0.5, 0.5) }, match (c.press, c.state.pressed) { (Some(p), true) => p[0], (Some(p), false) => p[1], (None, true) => PRESS_IN, (None, false) => PRESS_OUT });
    }
}

/// The built-in checkbox and radio: the 12-point box (an accent square inside when on) or ring, then the label, in the row's hover look.
pub(crate) fn default_checkbox(this: &mut Ui, c: &CheckConfig, text: &str) {
    check_parts(this, c, text, false)
}

pub(crate) fn default_radio(this: &mut Ui, c: &CheckConfig, text: &str) {
    check_parts(this, c, text, true)
}

fn check_parts(this: &mut Ui, c: &CheckConfig, text: &str, round: bool) {
    let (hovered, held, on) = (c.state.hovered, c.state.pressed, c.on);
    let t = this.theme();
    let (inner, label) = (if round { t.notice.idle } else { t.mark.idle }, *t.toggle.pick(State { hovered, ..State::default() }));
    this.set_look(&c.row, Look { text: Rgba::CLEAR, ..label }, State { hovered, disabled: c.state.disabled, ..State::default() });
    let rh = this.row_px() as f32;
    let bx = this.node_keyed(int_key(c.row.box_key, 0), Node { w: Size::Px(c.size), h: Size::Px(c.size), padding: Edges::all(c.size / 4.0), ..Node::DEFAULT });
    this.style(&bx, &c.states, State { hovered, pressed: held, checked: on, disabled: c.state.disabled, ..State::default() });
    if on {
        let inside = this.enter(&bx);
        this.node_keyed(int_key(c.row.box_key, 1), Node { w: Size::Px(c.size / 2.0), h: Size::Px(c.size / 2.0), look: inner, ..Node::DEFAULT });
        this.leave(inside);
    }
    if !(c.passive && text.is_empty()) {
        this.text_leaf(int_key(c.row.box_key, 2), rh, label, text);
    }
}

/// The built-in section index: one letter per child sharing the strip's height, in the row ink, the chosen one in the accent (`mark`).
pub(crate) fn default_section_index(this: &mut Ui, c: &SectionIndexConfig, letters: &[&str]) {
    let (ink, accent) = (this.theme().row.idle, this.theme().mark.idle);
    for (i, letter) in letters.iter().enumerate() {
        let look = Look { text: if c.state.pressed && c.active == Some(i) { accent.fill } else { ink.text }, ..text_only(&ink) };
        let n = this.node_keyed(int_key(c.node.box_key, i as i64), Node { w: Size::Grow, h: Size::Grow, look, ..Node::DEFAULT });
        this.text(&n, letter);
    }
}

/// The built-in progress row: the label, then a 4-point track holding the bar.
pub(crate) fn default_progress(this: &mut Ui, c: &ProgressConfig, label: &str) {
    let (track_look, bar, label_look) = (this.theme().progress.idle, this.theme().mark.idle, this.theme().row.idle);
    let rh = this.row_px() as f32;
    this.text_leaf(int_key(c.row.box_key, 1), rh, label_look, shown(label));
    let track = this.part(Part::TRACK, Node { flags: Flags::CLIP, h: Size::Px(4.0), look: track_look, role: Role::Progress, ..Node::DEFAULT });
    let inner = this.enter(&track);
    match c.value {
        Some(v) => {
            this.node_keyed(int_key(track.box_key, 0), Node { w: Size::Pct(v.clamp(0.0, 1.0)), h: Size::Grow, look: bar, ..Node::DEFAULT });
        }
        None => {
            this.part(Part::BAR, Node { w: Size::Pct(0.3), h: Size::Grow, look: bar, ..Node::DEFAULT });
        }
    }
    this.leave(inner);
}

/// The built-in slider: a 2-point rail, the filled part of it and an 8 x 14 thumb in a lane split by the value.
pub(crate) fn default_slider(this: &mut Ui, c: &SliderConfig, _label: &str) {
    let t = this.theme();
    let (rail, fill, thumb) = (t.separator.idle, t.mark.idle, t.slider);
    let key = this.part_owner;
    this.part(Part::RAIL, Node { h: Size::Px(2.0), look: rail, ..Node::DEFAULT });
    this.node_keyed(int_key(key, 3), Node { w: Size::Pct(c.fraction), h: Size::Px(2.0), look: fill, ..Node::DEFAULT });
    let lane = this.node_keyed(int_key(key, 4), Node { h: Size::Px(14.0), axis: Axis::Row, ..Node::DEFAULT });
    let tl = this.enter(&lane);
    this.node_keyed(int_key(key, 5), Node { h: Size::Px(1.0), grow: c.fraction, ..Node::DEFAULT });
    let th = this.part(Part::THUMB, Node { w: Size::Px(8.0), h: Size::Px(14.0), ..Node::DEFAULT });
    this.style(&th, &thumb, c.state);
    this.node_keyed(int_key(key, 7), Node { h: Size::Px(1.0), grow: 1.0 - c.fraction, ..Node::DEFAULT });
    this.leave(tl);
}

/// Reports the track's response, the value, the held preview and the commit (a release, a key or an assistive SetValue; the
/// preview equals it on that frame).
fn slider_body(this: &mut Ui, key: &str, text: &str, value: f32, range: RangeInclusive<f32>) -> (Response, f32, Option<f32>, Option<f32>) {
    let (lo, hi) = (*range.start(), *range.end());
    let span = (hi - lo).max(f32::MIN_POSITIVE);
    let key = key_of(this.current_seed(), key);
    let row = this.kit_line(key, Flags::NONE, Role::Unknown);
    let rh = this.row_px() as f32;
    let ch = this.control_h();
    let label_look = this.theme().row.idle;
    let saved = this.enter(&row);
    this.text_leaf(int_key(key, 0), rh, label_look, text);
    let track = this.node_keyed(int_key(key, 1), Node { flags: Flags::CLICKABLE | Flags::FOCUSABLE | Flags::BLOCKING, h: Size::Px(ch), axis: Axis::Stack, align: Align::Center, role: Role::Slider, ..Node::DEFAULT });
    let s = this.signal_owned(&track);
    let mut v = value.clamp(lo.min(hi), hi.max(lo));
    let step = span / 100.0;
    let mut keyed = false;
    pressed(&s.keys, &mut |k| {
        keyed |= matches!(k, Key::ArrowRight | Key::ArrowLeft | Key::ArrowUp | Key::ArrowDown | Key::PageUp | Key::PageDown | Key::Home | Key::End);
        v += match k {
            Key::ArrowRight | Key::ArrowLeft => this.along(k) as f32 * step,
            Key::ArrowUp => step,
            Key::ArrowDown => -step,
            Key::PageUp => 10.0 * step,
            Key::PageDown => -10.0 * step,
            Key::Home => lo - v,
            Key::End => hi - v,
            _ => 0.0,
        };
    });
    if s.held || s.released {
        // across the rail, a half thumb in from each end: from the parts' rectangles of the last frame
        let r = this.part_rect(key, Part::RAIL).unwrap_or_else(|| this.rect(&track));
        let thumb = this.part_rect(key, Part::THUMB).map_or(0.0, |t| t.w);
        let f = ((s.pointer.0 - r.x - thumb / 2.0) / (r.w - thumb).max(1.0)).clamp(0.0, 1.0);
        v = lo + if this.rtl() { 1.0 - f } else { f } * span;
    }
    if let Some(a) = s.a11y_value {
        v = a as f32;
    }
    let v = v.clamp(lo.min(hi), hi.max(lo));
    let commit = (keyed || (s.released && !s.held) || s.a11y_value.is_some()).then_some(v);
    let preview = if commit.is_some() || s.held { Some(v) } else { None };
    let f = ((v - lo) / span).clamp(0.0, 1.0);
    this.a11y_name(&track, text);
    this.a11y_number(&track, v as f64, lo.min(hi) as f64, hi.max(lo) as f64, step as f64);
    let config = SliderConfig { state: State { hovered: s.hovered, pressed: s.held, ..State::default() }, fraction: f, track };
    in_body!(this, key, track, slider, default_slider(&config, text));
    this.leave(saved);
    (Response::of(&s, this.focused(&track), track), v, preview, commit)
}
