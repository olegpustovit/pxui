//! Reorderable list rows (U2b.3): the drag a row's `Part::HANDLE` starts, the rows it passes making room, the keyboard (Alt with Up
//! or Down) and accessibility moves (Move up, Move down), and the rows' move animation. One drag at a time, kept in one fixed record
//! per `Ui`; the rows that animate sit in a fixed ring. Rows are placed by their layout `at`, so a row moves with everything it built.
//! Nothing allocates while dragging.
use super::*;
use super::gesture::{GestureCfg, TOUCH_DRAG};
use pxui_types::theme::Part;

/// Rows animating at once: past it the oldest jumps to its place.
const RING: usize = 32;

/// One row's move: its box key, where it showed and where it goes, from the frame clock's `t0`.
#[derive(Clone, Copy, Default)]
struct RowMove { key: i64, from: f32, to: f32, t0: f64 }

/// The drag in progress (`list` 0: none), the list whose rows animate to new places until `until`, and the rows animating.
#[derive(Clone, Copy, Default)]
pub(crate) struct Reorder {
    /// the list's box key, the lifted row's and its handle's
    list: i64,
    row: i64,
    handle: i64,
    /// the lifted row's index before the drag, and the index its centre has passed to this frame
    from: usize,
    to: usize,
    /// the pointer's distance below the lifted row's top at the press, the row's height and its top in the rows this frame
    grab: f32,
    h: f32,
    y: f32,
    open: i64,
    until: f64,
    moves: [RowMove; RING],
    next: usize,
    live: usize,
}

/// One reorderable list's rows while they build.
pub(crate) struct RowsMove {
    list: i64,
    /// the rows' top on screen, as last laid out
    top: f32,
    /// the lifted row's centre in the rows, while this list drags
    centre: Option<f32>,
    /// Alt with Up (-1) or Down (1) pressed on the focused node
    alt: i8,
    count: usize,
    pub(crate) moved: Option<(usize, usize)>,
}

impl Ui {
    /// Starts building the rows `extent` (`rows_h` high, `count` rows) of the reorderable list `body`.
    pub(crate) fn rows_move_begin(&mut self, body: &Handle, extent: &Handle, rows_h: f32, count: usize) -> RowsMove {
        let top = self.rect(extent).y;
        let mut m = RowsMove { list: body.box_key, top, centre: None, alt: 0, count, moved: None };
        let r = &mut self.reorder;
        if r.list == body.box_key {
            r.y = (self.pointer.1 - r.grab - top).clamp(0.0, (rows_h - r.h).max(0.0));
            r.to = r.from;
            m.centre = Some(r.y + r.h / 2.0);
        }
        for (event, mods) in self.key_events_of(self.focus) {
            if let (Event::KeyPressed(key), true) = (event, mods.contains(&Key::Alt)) {
                m.alt = match key { Key::ArrowUp => -1, Key::ArrowDown => 1, _ => m.alt };
            }
        }
        m
    }

    /// Builds the content of row `row` with `f` inside it. In a reorderable list (`m`) the content may build the row's handle (`Ui::part`
    /// outside a body), then the row moves (`rows_move_row`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn build_row_part(&mut self, row: &Handle, m: Option<&mut RowsMove>, a11y: bool, index: usize, top: f32, h: f32, f: &mut dyn FnMut(&mut Ui)) {
        let part = self.enter(row);
        let Some(m) = m else {
            self.slots[row.box_slot].movable = false;
            f(self);
            self.leave(part);
            return;
        };
        let owner = core::mem::replace(&mut self.part_owner, row.box_key);
        self.row_handle = None;
        self.movable_rows += 1;
        f(self);
        self.movable_rows -= 1;
        self.part_owner = owner;
        self.leave(part);
        let handle = self.row_handle.take();
        self.rows_move_row(m, row, a11y, handle, index, top, h);
    }

    /// Row `index` (box `row`, `top` and `h` its rest place in the rows; with `a11y` the row carries the Move actions itself, else its
    /// builder's node does, as a selection list's; `handle` the `Part::HANDLE` its build made): a handle press lifts it, a passed row
    /// makes room, a focused Alt arrow or a Move action moves it, and it is placed where it shows this frame.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rows_move_row(&mut self, m: &mut RowsMove, row: &Handle, a11y: bool, handle: Option<Handle>, index: usize, top: f32, h: f32) {
        self.slots[row.box_slot].movable = a11y;
        let step = match if a11y { self.signal_move(row) } else { 0 } { 0 if m.alt != 0 && self.focus != 0 && self.hit_within(self.focus, row.box_key) => m.alt, s => s };
        if step != 0 && m.moved.is_none() {
            let to = index as i64 + i64::from(step);
            if to >= 0 && (to as usize) < m.count { m.moved = Some((index, to as usize)); }
        }
        if let Some(hd) = handle {
            self.node_mut(&hd).flags |= crate::ui_widgets::ROW_FLAGS;
            self.default_role(&hd, Role::Button, "Reorder");
            self.gesture(&hd, Some(GestureCfg { kinds: TOUCH_DRAG, reveal: 0.0, threshold: 0.0, leading: 0.0, full: 0.0 }), 0);
            let s = self.signal(&hd);
            if s.pressed && s.button == Some(MouseButton::LeftButton) && self.reorder.list == 0 {
                let r = self.rect(row);
                let (moves, next, live) = (self.reorder.moves, self.reorder.next, self.reorder.live);
                self.reorder = Reorder { list: m.list, row: row.box_key, handle: hd.box_key, from: index, to: index, grab: s.pointer.1 - r.y, h: if r.h > 0.0 { r.h } else { h }, y: top,
                    open: m.list, until: f64::INFINITY, moves, next, live };
                m.centre = Some(top + self.reorder.h / 2.0);
            }
        }
        let mut target = top;
        if let (Some(c), true) = (m.centre, self.reorder.list == m.list) {
            let (r, mid) = (&mut self.reorder, top + h / 2.0);
            if row.box_key == r.row {
                target = r.y;
                self.node_mut(row).z = 1;
            // a lifted row held at the rows' end (its place clamped, its centre on the end row's middle) has passed the end row
            } else if index > r.from && (c > mid || (index + 1 == m.count && r.y + r.h >= top + h)) {
                (target, r.to) = (top - r.h, r.to.max(index));
            } else if index < r.from && (c < mid || (index == 0 && r.y <= top)) {
                (target, r.to) = (top + r.h, r.to.min(index));
            }
        }
        let shown = if self.reorder.list == m.list && row.box_key == self.reorder.row { target } else { self.row_shown(m, row, target) };
        if shown != top { self.node_mut(row).at = Some((0.0, shown)).into(); }
    }

    /// Ends the rows of `m`: a released (or lost) drag reports its move; a move opens the rows' animation and builds the frame again,
    /// so the app's new order shows at once.
    pub(crate) fn rows_move_end(&mut self, m: &mut RowsMove, body: &Handle) {
        let (slow, now, active) = (f64::from(self.theme().tokens.motion.slow), self.now, self.active);
        // a cancelled pointer, lost focus or Escape puts the lifted row back
        let cancelled = self.reorder.list == m.list
            && self.frame_events().iter().any(|e| matches!(e, Event::PointerCancelled(_) | Event::FocusLost | Event::KeyPressed(Key::Escape)));
        let r = &mut self.reorder;
        let dropped = r.list == m.list && (cancelled || active != Some(r.handle));
        if dropped {
            if r.to != r.from && !cancelled { m.moved = m.moved.or(Some((r.from, r.to))); }
            (r.list, r.row, r.handle) = (0, 0, 0);
        }
        // the dropped row settles from where it was let go, the moved rows from where they showed
        if dropped || m.moved.is_some() { (r.open, r.until) = (m.list, now + slow); }
        if m.moved.is_some() { self.build_again(body); } else if dropped { self.request_frame(); }
    }

    /// Where row `row` shows on its way to `target`: from where it was laid out along the theme's move curve (`motion.slow`, the
    /// theme's ease) while its list's animation is open, else at once.
    fn row_shown(&mut self, m: &RowsMove, row: &Handle, target: f32) -> f32 {
        let r = &self.reorder;
        let open = r.open == m.list && self.now < r.until;
        if r.live == 0 && !open { return target; }
        let t = self.theme().tokens;
        if t.reduced_motion || t.motion.slow <= 0.0 { return target; }
        let curve = pxui_types::theme::Curve::Bezier { duration: t.motion.slow, points: t.motion.ease };
        let laid = self.slots[row.box_slot].rw > 0.0;
        let was = self.rect(row).y - m.top;
        let now = self.now;
        let r = &mut self.reorder;
        let at = r.moves.iter().position(|e| e.key == row.box_key);
        let from = match at {
            Some(i) if r.moves[i].to == target => {
                let e = r.moves[i];
                let p = curve.at((now - e.t0) as f32);
                if p >= 1.0 { r.moves[i].key = 0; r.live -= 1; return target; }
                self.request_frame();
                return e.from + (target - e.from) * p;
            }
            Some(i) => { r.moves[i] = RowMove { key: row.box_key, from: was, to: target, t0: now }; was }
            None if open && laid && (was - target).abs() > 0.5 => {
                let i = r.next;
                if r.moves[i].key == 0 { r.live += 1; }
                r.moves[i] = RowMove { key: row.box_key, from: was, to: target, t0: now };
                r.next = (i + 1) % RING;
                was
            }
            None => return target,
        };
        self.request_frame();
        from
    }

    /// Up (-1) or Down (1) from an assistive technology's Move action on `b` this frame, else 0.
    pub(crate) fn signal_move(&self, b: &Handle) -> i8 {
        if !self.live(b) { return 0; }
        self.signals.get(&b.box_key).map_or(0, |a| a.a11y_move)
    }

    /// The lifted row of list `body` when it is outside the rows in view `window`: it is built too, so a drag outlives scrolling
    /// its row out of the virtual window.
    pub(crate) fn lifted_row(&self, body: &Handle, count: usize, window: core::ops::Range<usize>) -> Option<usize> {
        let r = &self.reorder;
        (r.list == body.box_key && r.from < count && !window.contains(&r.from)).then_some(r.from)
    }

    /// The node `Ui::part` builds as `part`: a reorderable row's handle is noted for its row (`rows_move_row`).
    pub fn note_part(&mut self, part: Part, b: Handle) {
        if part == Part::HANDLE { self.row_handle = Some(b); }
    }

    /// Marks `b` as a reorderable row's node offering Move up and Move down to assistive technology (or not).
    pub(crate) fn set_movable(&mut self, b: &Handle, on: bool) {
        if self.live(b) { self.slots[b.box_slot].movable = on; }
    }

    /// Whether a reorderable list row is building (where `Ui::part` may build its handle outside a body).
    pub fn in_movable_row(&self) -> bool { self.movable_rows > 0 }
}
