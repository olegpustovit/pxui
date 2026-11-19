//! The selection list and the table (U2b.1): selectable virtual rows over the section geometry (lead-in, sticky heading, fixed or
//! measured rows, trailer, or every row fitted without scrolling) and a table of columns under a sorting, resizing heading. The
//! selection lives in the caller's row flags (`SELECTED` the widget writes, `DISABLED` the caller writes) and anchor; the widget
//! applies clicks (Command toggles, Shift extends from the anchor), the keyboard (arrows, Page keys, Home, End with Shift, Space,
//! Command-A) and reports what changed. A warm frame allocates nothing.
use crate::a11y::state as st;
use crate::bodies::{body_of, in_item, RowConfig, TableHeadConfig};
use crate::ui_core::*;
use crate::ui_kit::Response;
use crate::ui_widgets::{clamp_step, default_row, default_table_head, ITEM_LIST, ITEM_TABLE, ROW_FLAGS};
use input::{Event, Key};
use pxui_types::ffi::PxuiColumnWidth;
use pxui_types::ui::{SectionList, SelectionList};

/// A row flag the widget writes: the row is selected.
pub const SELECTED: u8 = 1;
/// A row flag the caller writes: the row takes no selection or presses and reads as disabled.
pub const DISABLED: u8 = 2;
/// A row flag the caller writes: a retained list rebuilds the row every frame (content that changes with the frame, such as an
/// animation that follows the row's place on screen).
pub const LIVE: u8 = 4;
/// The most columns a table or heading takes.
pub const MAX_COLUMNS: usize = 64;

/// What a selection list or table row's builder is told about the row.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct ListCell {
    pub selected: bool,
    pub enabled: bool,
    pub focused: bool,
    pub held: bool,
    /// The row's box (the empty box for the lead-in, heading and trailer).
    pub handle: Handle,
    /// The part's logical content Y.
    pub y: f32,
}

/// What a selection list or table's rows report this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct ListResult {
    pub handle: Handle,
    pub activated: Option<usize>,
    pub selected: Option<usize>,
    pub secondary: Option<usize>,
    pub row: Option<Handle>,
    /// The insertion index a carried payload was dropped at (a reorderable list).
    pub drop_at: Option<usize>,
    /// A row moved this frame (a reorderable list): `(from, to)`, `to` the final index.
    pub moved: Option<(usize, usize)>,
    /// The list holds keyboard focus this frame.
    pub focused: bool,
}

/// One table column as the engine takes it (validated by the caller; its title travels beside it in a `titles` slice): `share` > 0
/// splits the width left by the point columns by weight, else `w` points (0: hidden); `grip` lets a point column be resized;
/// `sortable` makes its heading cycle the sort.
#[derive(Clone, Copy, Debug, Default)]
pub struct ColumnSpec {
    pub id: i64,
    pub share: f32,
    pub w: f32,
    pub grip: bool,
    pub sortable: bool,
}

impl ColumnSpec {
    fn hidden(&self) -> bool { self.share <= 0.0 && self.w <= 0.0 }
    fn gripped(&self) -> bool { self.grip && self.share <= 0.0 }
}

/// What a heading reports this frame: the clicked heading's response (else the heading row's), a sort and a width change.
#[derive(Clone, Copy, Debug, Default)]
pub struct HeadResult {
    pub response: Response,
    pub sorted: bool,
    pub resized: bool,
}

fn select_alone(flags: &mut [u8], i: usize) {
    for f in flags.iter_mut() { *f &= !SELECTED; }
    if flags[i] & DISABLED == 0 { flags[i] |= SELECTED; }
}

fn select_range(flags: &mut [u8], a: usize, b: usize) {
    let (lo, hi) = (a.min(b), a.max(b));
    for (i, f) in flags.iter_mut().enumerate() {
        if (lo..=hi).contains(&i) && *f & DISABLED == 0 { *f |= SELECTED } else { *f &= !SELECTED }
    }
}

/// The first enabled row from `at` stepping by `dir` (then back the other way), so the keyboard steps over disabled rows; `None` when
/// every row is disabled.
fn enabled_toward(flags: &[u8], at: usize, dir: i64) -> Option<usize> {
    let ok = |i: &usize| flags[*i] & DISABLED == 0;
    if dir > 0 { (at..flags.len()).find(ok).or_else(|| (0..at).rev().find(ok)) } else { (0..=at).rev().find(ok).or_else(|| (at + 1..flags.len()).find(ok)) }
}

/// The row `k` names after the row at `from` moved to `to` (the rows between shift by one).
pub(crate) fn moved_index(k: usize, (from, to): (usize, usize)) -> usize {
    if k == from { to } else if from < to && (from + 1..=to).contains(&k) { k - 1 } else if to < from && (to..from).contains(&k) { k + 1 } else { k }
}

/// The frame's keys on a focused list: the keyboard row before and after, Enter, the row whose selection a key changed and, in a
/// reorderable list, the keyboard row's move (Alt with Up or Down).
#[allow(clippy::type_complexity)]
fn key_selection(this: &Ui, frame: &Handle, count: usize, page: i64, flags: &mut [u8], anchor: &mut Option<usize>, multi: bool, reorder: bool) -> (usize, usize, bool, Option<usize>, Option<(usize, usize)>) {
    let cursor = (this.state(frame, 0).max(0) as usize).min(count.saturating_sub(1));
    let (mut c, mut enter, mut space, mut all, mut changed, mut moved) = (cursor, false, false, false, None, None);
    for (event, mods) in this.key_events_ref(frame) {
        let Event::KeyPressed(key) = event else { continue };
        if reorder && moved.is_none() && mods.contains(&Key::Alt) && matches!(key, Key::ArrowUp | Key::ArrowDown) {
            let to = if *key == Key::ArrowUp { c.checked_sub(1) } else { Some(c + 1).filter(|t| *t < count) };
            moved = to.map(|to| (c, to));
            continue;
        }
        let to = match key {
            Key::ArrowDown => Some(clamp_step(c, 1, count)),
            Key::ArrowUp => Some(clamp_step(c, -1, count)),
            Key::PageDown => Some(clamp_step(c, page, count)),
            Key::PageUp => Some(clamp_step(c, -page, count)),
            Key::Home => Some(0),
            Key::End => Some(count.saturating_sub(1)),
            Key::Enter => { enter = true; None }
            Key::Space | Key::KeyChar(' ') => { space = true; None }
            Key::KeyChar('a' | 'A') if multi && this.command_modifiers(mods) => { all = true; None }
            _ => None,
        };
        if let Some(to) = to.filter(|_| count > 0).and_then(|to| enabled_toward(flags, to, if to < c { -1 } else { 1 })) {
            c = to;
            if multi && mods.contains(&Key::Shift) {
                select_range(flags, anchor.unwrap_or(cursor).min(count - 1), c);
            } else {
                select_alone(flags, c);
                *anchor = Some(c);
            }
            changed = Some(c);
        }
    }
    if space && count > 0 && flags[c] & DISABLED == 0 {
        if multi { flags[c] ^= SELECTED } else { select_alone(flags, c) }
        *anchor = Some(c);
        changed = Some(c);
    }
    if all && count > 0 {
        for f in flags.iter_mut() { if *f & DISABLED == 0 { *f |= SELECTED; } }
        changed = Some(c);
    }
    (cursor, c, enter && count > 0 && flags[c] & DISABLED == 0, changed, moved)
}

/// The rows' shared state while a list or table builds.
struct Rows {
    /// rows take a payload between them (insertion line, `drop_at`)
    reorder: bool,
    /// rows take a payload over them, shown in this look
    drop_look: Option<Look>,
    /// a click changed the selection after rows before it were built from the old one
    late: bool,
    multi: bool,
    open_on_click: bool,
    focused: bool,
    c: usize,
    /// a row an assistive technology's Move action moved (a reorderable list)
    moved: Option<(usize, usize)>,
    out: ListResult,
    kind: u8,
    role: Role,
    inset: f32,
    /// the row's end padding (the theme's second spacing step), read once a list
    side: f32,
    gap: f32,
    /// the theme's row looks and the insertion line's look (the drop look, else the accent), read once a list, not once a row
    states: States,
    line: Look,
    /// the caller's content revision folded with what else the rows show (the body variant, a table's widths): rows are kept while
    /// it holds (`None`: rebuilt every frame)
    revision: Option<u64>,
}

/// One FNV-1a step of `value` into `hash`: a retained row's key folds its inputs one word each.
fn mix(hash: u64, value: u64) -> u64 { (hash ^ value).wrapping_mul(0x0000_0100_0000_01b3) }

/// Row `i` in its list row box (entered, placed by the list). Without a revision the box is the row: the clickable row in the list
/// look, the selection its presses make, then the caller's content through the row body, rebuilt every frame (a row costs the list
/// one node a frame, as a section list's does). With the caller's revision (`ListState::revision`) the box holds a view and the row is
/// a node inside it, kept with everything its body and content built while the row's key, its selected, disabled and focused state,
/// the payload over it and the revision hold (hovering, pressing and keys reach a kept row's nodes and rebuild it, as any view's); a
/// `LIVE` row, and every row of a reorderable list, rebuilds every frame.
fn build_row(ui: &mut Ui, r: &mut Rows, flags: &mut [u8], anchor: &mut Option<usize>, i: usize, y: f32, content: &mut dyn FnMut(&mut Ui, usize, &ListCell)) {
    let n = ui.entered();
    let enabled = flags[i] & DISABLED == 0;
    let (over, side) = (drop_over(ui, r, &n, i), r.side);
    let Some(revision) = r.revision else {
        // the list's size and place stay (`h` is its row height, or `Fit` for measured rows); the row's own layout joins them
        let node = ui.node_new(&n);
        (node.flags, node.axis, node.align, node.padding, node.gap) = (if enabled { ROW_FLAGS } else { Flags::NONE }, Axis::Row, Align::Center, Edges { l: r.inset, r: side, ..Edges::ZERO }, r.gap);
        let place = node.at;
        row_in(ui, r, flags, anchor, i, y, n, over, content);
        // a body lays out its row (`set_node_layout`); the row's place in the list stays the list's
        ui.node_new(&n).at = place;
        return;
    };
    // the box only places the row: out of the accessibility tree, the row inside it is the list item
    let h = if matches!(ui.node_new(&n).h, Size::Fit) { Size::Fit } else { Size::Grow };
    ui.node_new(&n).role = Role::Unknown;
    let focused = r.focused && i == r.c;
    let mut key = mix(mix(mix(revision, u64::from(flags[i] & (SELECTED | DISABLED))), u64::from(focused)), over as u64);
    if flags[i] & LIVE != 0 || r.reorder { key = mix(key, ui.frame_no() as u64); }
    ui.view_entry("row", key, Size::Grow, h, true, &mut |ui| {
        let row = ui.node_ref("row", &Node { flags: if enabled { ROW_FLAGS } else { Flags::NONE }, w: Size::Grow, h, axis: Axis::Row, align: Align::Center, padding: Edges { l: r.inset, r: side, ..Edges::ZERO }, gap: r.gap, ..Node::DEFAULT });
        let saved = ui.enter(&row);
        row_in(ui, r, flags, anchor, i, y, row, over, content);
        ui.leave(saved);
    });
}

/// The payload over row `i` (box `n`) this frame: 1 or 2 a reorderable list's insertion line above or below it, 3 a payload over a
/// list that takes drops on its rows (shown in the drop look), else 0. Asked every frame, so the box is a drop zone while it shows.
fn drop_over(ui: &mut Ui, r: &Rows, n: &Handle, i: usize) -> u8 {
    if !ui.drop_pending() { return 0; }
    let over = if r.reorder { ui.drop_side(n, i).map(|(after, dropped)| if dropped { 0 } else if after { 2 } else { 1 }) }
        else if r.drop_look.is_some() { ui.drop_target_ref(n).map(|_| 3) } else { None };
    over.unwrap_or(0)
}

/// Row `i` itself on `row` (entered): its signal, the selection its presses make, its look, role and the caller's content through the
/// row body.
#[allow(clippy::too_many_arguments)]
fn row_in(ui: &mut Ui, r: &mut Rows, flags: &mut [u8], anchor: &mut Option<usize>, i: usize, y: f32, n: Handle, drop: u8, content: &mut dyn FnMut(&mut Ui, usize, &ListCell)) {
    let enabled = flags[i] & DISABLED == 0;
    let s = ui.signal_owned(&n);
    if enabled {
        if s.clicked {
            let (shift, command) = ui.press_mods(&n);
            if r.multi && command {
                flags[i] ^= SELECTED;
                *anchor = Some(i);
            } else if r.multi && shift {
                select_range(flags, anchor.unwrap_or(r.c).min(flags.len() - 1), i);
            } else {
                select_alone(flags, i);
                *anchor = Some(i);
                if r.open_on_click { r.out.activated = Some(i); r.out.row = Some(n); }
            }
            r.c = i;
            r.out.selected = Some(i);
            r.late = true;
        }
        if s.double_clicked { r.out.activated = Some(i); r.out.row = Some(n); }
        if s.pressed && s.button == Some(input::MouseButton::RightButton) { r.out.secondary = Some(i); r.out.row = Some(n); }
    }
    let on = flags[i] & SELECTED != 0;
    let focused = r.focused && i == r.c;
    let state = State { hovered: s.hovered && enabled, pressed: s.held && enabled, checked: on, focused, disabled: !enabled };
    ui.style(&n, &r.states, state);
    ui.set_role(&n, r.role, st::SELECTABLE | if on { st::SELECTED } else { 0 });
    ui.node_new(&n).state.disabled = !enabled;
    ui.set_movable(&n, r.reorder);
    if r.reorder && r.moved.is_none() {
        let to = i as i64 + i64::from(ui.signal_move(&n));
        if to != i as i64 && to >= 0 && (to as usize) < flags.len() { r.moved = Some((i, to as usize)); }
    }
    if let (3, Some(look)) = (drop, r.drop_look) { ui.node_mut(&n).look = look; }
    let cell = ListCell { selected: on, enabled, focused, held: s.held && enabled, handle: n, y };
    let config = RowConfig { state, index: i, row: n, drop, line: r.line };
    let body = if r.kind == ITEM_TABLE { body_of!(ui, table_row) } else { body_of!(ui, selection_list) };
    let outer = (core::mem::replace(&mut ui.bodies.item, n), core::mem::replace(&mut ui.bodies.item_kind, r.kind));
    in_item!(ui, body, n.box_key, n, default_row(&config, &mut |ui| content(ui, i, &cell)));
    (ui.bodies.item, ui.bodies.item_kind) = outer;
}

/// Every part of `section` built in a column that fits its content (no scrolling, no virtual window): `f(ui, index, y)` as
/// `Ui::section_list` calls it, the trailer included.
fn fit_rows(this: &mut Ui, section: &SectionList, ids: &dyn Fn(usize) -> i64, f: &mut dyn FnMut(&mut Ui, usize, f32)) -> Handle {
    let body = this.node("rows".into(), Node { w: Size::Grow, h: Size::Fit, axis: Axis::Column, role: Role::List, ..Node::DEFAULT });
    let saved = this.enter(&body);
    let mut y = 0.0;
    for (key, h, index) in [("leading", section.leading_height, usize::MAX), ("header", section.header_height, usize::MAX - 1)] {
        if h > 0.0 {
            let part = this.node(key.into(), Node { w: Size::Grow, h: Size::Px(h), ..Node::DEFAULT });
            let inner = this.enter(&part);
            f(this, index, y);
            this.leave(inner);
            y += h;
        }
    }
    let h = if section.measured { Size::Fit } else { Size::Px(section.row_height) };
    for i in 0..section.count {
        let row = this.node(pxui_types::ui::Key::Id(ids(i)), Node { w: Size::Grow, h, role: Role::ListItem, ..Node::DEFAULT });
        let inner = this.enter(&row);
        f(this, i, y);
        this.leave(inner);
        y += if section.measured { this.rect(&row).h } else { section.row_height };
    }
    if section.trailing_height > 0.0 {
        let part = this.node("trailing".into(), Node { w: Size::Grow, h: Size::Px(section.trailing_height), ..Node::DEFAULT });
        let inner = this.enter(&part);
        f(this, usize::MAX - 2, y);
        this.leave(inner);
    }
    this.leave(saved);
    body
}

/// The columns laid out to `avail` points: each column's `(id, width)` written to `widths` in slice order (hidden 0); a gripped
/// point column keeps the width `widths` held for its id (its dragged width), shared columns split what the point columns and the
/// one-point separators leave by weight.
fn lay_columns(columns: &[ColumnSpec], widths: &mut [PxuiColumnWidth], avail: f32) {
    let mut points = [0.0f32; MAX_COLUMNS];
    let (mut fixed, mut shares) = (0.0f32, 0.0f32);
    for (i, c) in columns.iter().enumerate() {
        if c.hidden() { continue; }
        fixed += 1.0;
        if c.share > 0.0 { shares += c.share; continue; }
        points[i] = if c.gripped() { widths.iter().find(|w| w.id == c.id && w.w > 0.0).map_or(c.w, |w| w.w) } else { c.w };
        fixed += points[i];
    }
    let rest = (avail - fixed).max(0.0);
    for (i, c) in columns.iter().enumerate() {
        widths[i] = PxuiColumnWidth { id: c.id, w: if c.hidden() { 0.0 } else if c.share > 0.0 { (rest * c.share / shares).floor() } else { points[i] }, reserved: 0 };
    }
}

/// The most sortable headings whose "Sort by" names the engine keeps (round robin past it).
const SORT_NAMES: usize = 64;

/// A sortable heading's composed accessible name, kept until its title or sort changes.
#[derive(Default)]
pub(crate) struct SortName {
    key: i64,
    by: Option<bool>,
    text: String,
}

/// The engine's bounded store of composed "Sort by" names (`SORT_NAMES` entries).
#[derive(Default)]
pub(crate) struct SortNames {
    names: Vec<SortName>,
    next: usize,
}

const SORT_BY: &str = "Sort by ";

fn sort_tail(by: Option<bool>) -> &'static str {
    match by { Some(true) => ", ascending", Some(false) => ", descending", None => "" }
}

/// The headings of `columns` inside the heading row `head` (entered here), each `widths` wide, built through the `table_head`
/// body; a click on a sortable heading cycles `sort` (none, ascending, descending, none), a gripped column's grip resizes it.
fn head_row(this: &mut Ui, head: Handle, columns: &[ColumnSpec], titles: &[&str], widths: &mut [PxuiColumnWidth], sort: &mut Option<(i64, bool)>) -> HeadResult {
    let body = body_of!(this, table_head);
    let mut out = HeadResult { response: Response { handle: head, ..Response::default() }, sorted: false, resized: false };
    let saved = this.enter(&head);
    let a11y = this.a11y_on;
    for (i, c) in columns.iter().enumerate() {
        if c.hidden() { continue; }
        // a column without a grip is followed by the heading row's 1-point separator, its margin (no node of its own)
        let margin = if c.gripped() { Edges::ZERO } else { Edges { r: 1.0, ..Edges::ZERO } };
        let n = this.node_keyed(int_key(head.box_key, c.id), Node { flags: ROW_FLAGS, w: Size::Px(widths[i].w), h: Size::Grow, axis: Axis::Row, align: Align::Center, justify: Justify::SpaceBetween, padding: Edges { l: 8.0, r: 8.0, ..Edges::ZERO }, margin, role: Role::ColumnHeader, ..Node::DEFAULT });
        let ns = this.signal_owned(&n);
        if ns.clicked && c.sortable {
            *sort = match *sort {
                Some((id, true)) if id == c.id => Some((id, false)),
                Some((id, false)) if id == c.id => None,
                _ => Some((c.id, true)),
            };
            out.sorted = true;
            out.response = Response::of(&ns, false, n);
        }
        let by = sort.filter(|(id, _)| *id == c.id).map(|(_, ascending)| ascending);
        if a11y {
            if c.sortable { this.sort_name(&n, titles[i], by) } else { this.a11y_name(&n, titles[i]) }
        }
        let config = TableHeadConfig { state: State { hovered: ns.hovered, pressed: ns.held, checked: by.is_some(), ..State::default() }, index: i, ascending: by, node: n };
        in_item!(this, body, n.box_key, n, default_table_head(&config, titles[i]));
        if c.gripped() {
            let (_, gs) = this.grip(format_args!("grip{}", c.id), Axis::Row, None);
            if gs.held && gs.drag.0 != 0.0 {
                widths[i].w = (widths[i].w + if this.rtl() { -gs.drag.0 } else { gs.drag.0 }).max(32.0);
                out.resized = true;
            }
        }
    }
    this.leave(saved);
    out
}

impl Ui {
    /// Names the sortable heading `n` "Sort by <title>[, ascending|, descending]": the text is composed once per title or sort change
    /// into the engine's bounded store and copied while it holds.
    fn sort_name(&mut self, n: &Handle, title: &str, by: Option<bool>) {
        let tail = sort_tail(by);
        let fresh = |e: &SortName| e.by == by && e.text.len() == SORT_BY.len() + title.len() + tail.len() && e.text[SORT_BY.len()..].starts_with(title);
        let store = &mut self.sort_names;
        let at = match store.names.iter().position(|e| e.key == n.box_key) {
            Some(at) => at,
            None if store.names.len() < SORT_NAMES => { store.names.push(SortName { key: n.box_key, by: None, text: String::new() }); store.names.len() - 1 }
            None => { store.next = (store.next + 1) % SORT_NAMES; store.names[store.next].key = n.box_key; store.names[store.next].text.clear(); store.next }
        };
        let e = &mut store.names[at];
        if !fresh(e) {
            e.by = by;
            e.text.clear();
            for part in [SORT_BY, title, tail] { e.text.push_str(part); }
        }
        let text = std::mem::take(&mut self.sort_names.names[at].text);
        self.a11y_name(n, &text);
        self.sort_names.names[at].text = text;
    }

    /// A selectable list of `count` rows filling its parent (fitting its rows with `SelectionList::fit`) with the list's section
    /// geometry: `cell(ui, index, cell)` builds the lead-in (`usize::MAX`), the heading (`usize::MAX - 1`), the trailer
    /// (`usize::MAX - 2`) and each row in view inside the row node the `selection_list` body shapes. `flags` holds one byte a row
    /// (`SELECTED`, `DISABLED`; at least `count`), `anchor` the row Shift extends from. Row `i`'s box is keyed by `ids(i)`.
    #[allow(clippy::too_many_arguments)]
    pub fn selection_rows(&mut self, label: &str, count: usize, ids: &dyn Fn(usize) -> i64, flags: &mut [u8], anchor: &mut Option<usize>, revision: Option<u64>, list: &SelectionList, cell: &mut dyn FnMut(&mut Ui, usize, &ListCell)) -> ListResult {
        let count = count.min(flags.len());
        let flags = &mut flags[..count];
        let look = self.theme().scroll_view.idle;
        // a bare list has no frame: no look and no inset
        let (look, padding) = if list.bare { (Look::NONE, Edges::ZERO) } else { (look, Edges::all(1.0)) };
        let frame = self.node_ref(label, &Node { flags: Flags::FOCUSABLE | Flags::CLIP, w: Size::Grow, h: if list.fit { Size::Fit } else { Size::Grow }, look, padding, ..Node::DEFAULT });
        let focused = self.focused(&frame);
        let page = (self.rect(&frame).h / list.row_height).floor().max(1.0) as i64;
        let (cursor, c, enter, changed, key_moved) = key_selection(self, &frame, count, page, flags, anchor, list.multi, list.reorder);
        self.list_drop = None;
        let mut rows = Rows { reorder: list.reorder, drop_look: list.drop_look, late: false, multi: list.multi, open_on_click: list.open_on_click, focused, c, moved: None, out: ListResult { selected: changed, activated: enter.then_some(c), focused, ..ListResult::default() }, kind: ITEM_LIST, role: Role::ListItem, inset: self.space(2), side: self.space(2), gap: 4.0, states: self.theme().listbox, line: list.drop_look.unwrap_or(Look { fill: self.theme().tokens.accent, ..Look::NONE }),
            revision: revision.map(|v| mix(v, u64::from(self.bodies.variant))) };
        // the rows take drops themselves (their body draws the line), so the section draws none
        let section = SectionList { count, row_height: list.row_height, leading_height: list.leading_height, header_height: list.header_height, trailing_height: list.trailing_height, overlay_scrollbar: list.overlay_scrollbar, measured: list.measured, reorder: false };
        let saved = self.enter(&frame);
        let mut part = |ui: &mut Ui, index: usize, y: f32| {
            if index >= usize::MAX - 2 { cell(ui, index, &ListCell { enabled: true, y, ..ListCell::default() }) } else { build_row(ui, &mut rows, flags, anchor, index, y, cell) }
        };
        // a handle drag moves the rows of a scrolling list (a fitted one moves rows by the keyboard and accessibility only)
        self.row_moves = list.reorder && !list.fit;
        let body = if list.fit { fit_rows(self, &section, ids, &mut part) } else { self.section_list("rows", &section, Some(ids), true, &mut part) };
        self.row_moves = false;
        let dragged = if list.fit { None } else { self.list_moved() };
        self.leave(saved);
        if !list.fit { let bar = self.thumb(); self.node_mut(&body).bar = Some(bar).into(); }
        self.set_role(&body, Role::List, if list.multi { st::SELECTABLE } else { 0 });
        self.a11y_name(&body, shown(label));
        let (mut c, mut out) = (rows.c, rows.out);
        out.drop_at = self.list_drop.filter(|d| d.1).map(|d| d.0);
        out.moved = key_moved.or(rows.moved).or(dragged);
        if let Some((from, to)) = out.moved {
            // the row's flags travel with it; the keyboard row and the anchor follow the rows they named
            if from < to { flags[from..=to].rotate_left(1) } else { flags[to..=from].rotate_right(1) }
            c = moved_index(c, (from, to));
            *anchor = anchor.map(|a| moved_index(a, (from, to)));
            self.build_again(&frame);
        }
        if rows.late { self.build_again(&frame); }
        if c != cursor { self.reveal(&body, c); }
        self.set_state(&frame, c as i64);
        out.row = out.row.or_else(|| out.activated.map(|_| body));
        out.handle = body;
        out
    }

    /// A table filling its parent: a heading of `columns` (as `table_head`) over `rows` selectable rows `row_h` high, each cell
    /// `widths` wide (laid out here to the table's width); `cell(ui, row, column, cell)` builds one cell. Selection as
    /// `selection_rows` with `flags`, `anchor` and `multi`; the heading is built with the `table_head` variant `head_variant` (0: the
    /// theme's). Reports the rows' and the heading's results.
    #[allow(clippy::too_many_arguments)]
    pub fn table(&mut self, label: &str, columns: &[ColumnSpec], titles: &[&str], widths: &mut [PxuiColumnWidth], sort: &mut Option<(i64, bool)>, rows: usize, ids: &dyn Fn(usize) -> i64, flags: &mut [u8], anchor: &mut Option<usize>, revision: Option<u64>, multi: bool, row_h: f32, head_h: f32, head_variant: u16, cell: &mut dyn FnMut(&mut Ui, usize, usize, &ListCell)) -> (ListResult, HeadResult) {
        let count = rows.min(flags.len());
        let flags = &mut flags[..count];
        let columns = &columns[..columns.len().min(widths.len()).min(titles.len()).min(MAX_COLUMNS)];
        let t = self.theme();
        let (frame_look, head_look) = (t.scroll_view.idle, t.table_head.idle);
        let bar = self.thumb();
        let frame = self.node_ref(label, &Node { flags: Flags::FOCUSABLE | Flags::SCROLL_X | Flags::CLIP, w: Size::Grow, h: Size::Grow, look: frame_look, bar: Some(bar).into(), padding: Edges::all(1.0), role: Role::Table, ..Node::DEFAULT });
        let focused = self.focused(&frame);
        let page = (self.rect(&frame).h / row_h - 1.0).floor().max(1.0) as i64;
        let (cursor, c, enter, changed, _) = key_selection(self, &frame, count, page, flags, anchor, multi, false);
        let avail = self.rect(&frame).w - 2.0;
        if columns.iter().any(|c| c.share > 0.0) { self.expect(frame.box_slot, avail + 2.0, f32::NAN); }
        lay_columns(columns, widths, avail);
        let total: f32 = columns.iter().zip(widths.iter()).filter(|(c, _)| !c.hidden()).map(|(_, w)| w.w + 1.0).sum();
        let saved = self.enter(&frame);
        let sheet = self.node("sheet".into(), Node { w: Size::Grow, min_w: total, h: Size::Grow, ..Node::DEFAULT });
        let inner = self.enter(&sheet);
        let head = self.node("head".into(), Node { h: Size::Px(head_h), axis: Axis::Row, look: head_look, ..Node::DEFAULT });
        // the call's one variant slot holds the rows' (`table_row`) variant; the heading's is set around the heading alone
        let rows_variant = (self.bodies.variant, self.bodies.variant_kind);
        if head_variant != 0 { self.set_variant(pxui_types::theme::BodyKind::table_head as u32, head_variant); }
        let head_out = head_row(self, head, columns, titles, widths, sort);
        (self.bodies.variant, self.bodies.variant_kind) = rows_variant;
        let widths = &*widths;
        // a kept row's cells are as wide as their columns were: a resized column rebuilds the rows
        let revision = revision.map(|v| widths.iter().fold(mix(v, u64::from(self.bodies.variant)), |h, w| mix(h, u64::from(w.w.to_bits()))));
        let mut rows = Rows { reorder: false, drop_look: None, late: false, multi, open_on_click: false, focused, c, moved: None, out: ListResult { selected: changed, activated: enter.then_some(c), focused, ..ListResult::default() }, kind: ITEM_TABLE, role: Role::Row, inset: 0.0, side: self.space(2), gap: 1.0, states: self.theme().listbox, line: Look { fill: self.theme().tokens.accent, ..Look::NONE }, revision };
        let mut cells = |ui: &mut Ui, row: usize, info: &ListCell| {
            let n = info.handle;
            for (col, (spec, w)) in columns.iter().zip(widths).enumerate() {
                if spec.hidden() { continue; }
                let c = ui.node_keyed(int_key(n.box_key, spec.id), Node { w: Size::Px(w.w), h: Size::Grow, axis: Axis::Row, align: Align::Center, padding: Edges { l: 8.0, r: 8.0, ..Edges::ZERO }, role: Role::Cell, ..Node::DEFAULT });
                let inner = ui.enter(&c);
                cell(ui, row, col, info);
                ui.leave(inner);
            }
        };
        let body = self.section_list("rows", &SectionList::new(count).row_h(row_h), Some(ids), false, &mut |ui, i, y| build_row(ui, &mut rows, flags, anchor, i, y, &mut cells));
        let bar = self.thumb();
        self.node_mut(&body).bar = Some(bar).into();
        self.set_role(&body, Role::Unknown, 0);
        self.leave(inner);
        self.leave(saved);
        let (c, mut out) = (rows.c, rows.out);
        if rows.late { self.build_again(&frame); }
        if c != cursor { self.reveal(&body, c); }
        self.set_state(&frame, c as i64);
        out.handle = body;
        (out, head_out)
    }

    /// A heading row of `columns` keyed by `label`, `h` points high and laid out to its parent's width: `widths` (one entry a column)
    /// is rewritten with every column's `(id, laid-out width)`, `sort` cycles on a sortable heading's click (see `table`).
    pub fn table_head(&mut self, label: &str, columns: &[ColumnSpec], titles: &[&str], widths: &mut [PxuiColumnWidth], sort: &mut Option<(i64, bool)>, h: f32) -> HeadResult {
        let columns = &columns[..columns.len().min(widths.len()).min(titles.len()).min(MAX_COLUMNS)];
        let look = self.theme().table_head.idle;
        let head = self.node_ref(label, &Node { w: Size::Grow, h: Size::Px(h), axis: Axis::Row, look, ..Node::DEFAULT });
        let avail = self.rect(&head).w;
        if columns.iter().any(|c| c.share > 0.0) { self.expect(head.box_slot, avail, f32::NAN); }
        lay_columns(columns, widths, avail);
        head_row(self, head, columns, titles, widths, sort)
    }
}
