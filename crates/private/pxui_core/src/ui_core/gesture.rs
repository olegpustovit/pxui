//! Touch gestures above scroll: delayed highlight and long press, horizontal row swipe, interactive
//! edge back and pull to refresh. The finger side lives in `touch`; this holds the per-node retained
//! state, the closed-form settle spring and the app-facing read. Touch input only: a mouse or
//! trackpad never starts one. Timing runs on the frame clock, so a held finger needs no events.
use super::{Direction, Ui, UiBox};

pub const LONG_PRESS: u32 = 1;
pub const ROW_SWIPE: u32 = 2;
pub const EDGE_BACK: u32 = 4;
pub const REFRESH: u32 = 8;
/// A node taking the whole touch at once: a held press and drag like a mouse, never a tap, scroll or other gesture.
pub const TOUCH_DRAG: u32 = 32;
/// Internal kind bit of a dismiss node (set by `Ui::dismiss`, never by `gesture`).
pub(super) const DISMISS: u32 = 16;
/// `Ui::dismiss` flags: finger down or settling, and the release verdict (this frame).
pub const DISMISSING: u32 = 8192;
pub const DISMISS_COMMIT: u32 = 16384;
pub const DISMISS_CANCEL: u32 = 32768;
pub const CLOSE_SWIPE: u32 = 1;
pub const END_REFRESH: u32 = 2;
pub const HIGHLIGHTED: u32 = 1;
pub const LONG_PRESSED: u32 = 2;
pub const SWIPING: u32 = 4;
pub const SWIPE_OPEN: u32 = 8;
pub const BACK_ACTIVE: u32 = 16;
pub const BACK_COMMIT: u32 = 32;
pub const BACK_CANCEL: u32 = 64;
pub const REFRESH_FIRED: u32 = 128;
pub const REFRESHING: u32 = 256;
/// A full swipe committed the leading or trailing action (this frame), the row then settles closed.
pub const SWIPE_COMMIT_LEADING: u32 = 512;
pub const SWIPE_COMMIT_TRAILING: u32 = 1024;
/// The row rests open on the leading or trailing side (`SWIPE_OPEN` is set for either).
pub const SWIPE_OPEN_LEADING: u32 = 2048;
pub const SWIPE_OPEN_TRAILING: u32 = 4096;

// UITableViewCell/UIScrollView touch delay (~0.1 s) before a cell highlights; a quick tap still flashes it.
pub(super) const HIGHLIGHT_DELAY: f64 = 0.1;
// UILongPressGestureRecognizer.minimumPressDuration.
pub(super) const LONG_PRESS_DELAY: f64 = 0.5;
// UIScreenEdgePanGestureRecognizer's edge band (~20 pt), measured from the leading edge.
pub(super) const EDGE_WIDTH: f64 = 20.0;
// A flick this fast (points/s) decides a swipe or a back; slower releases decide by position.
pub(super) const SNAP_VELOCITY: f64 = 300.0;
pub(super) const BACK_VELOCITY: f64 = 500.0;
// UIKit sheet: a release this fast (points/s) downward dismisses; otherwise a quarter of the height.
const DISMISS_VELOCITY: f32 = 600.0;
const DISMISS_FRACTION: f32 = 0.25;
// Default UIRefreshControl pull distance, points.
const REFRESH_DISTANCE: f32 = 60.0;
// Critically damped settle (rad/s): about 0.35 s to rest, the order of UINavigationController's pop.
const OMEGA: f64 = 20.0;
const SETTLE_SECONDS: f64 = 0.6;

/// Per-node gesture request: which gestures the node takes, the row-swipe action widths (`reveal`
/// is the trailing side, `leading` the leading one; 0 disables a side), the full-swipe commit
/// distance (0: no full swipe) and the refresh pull distance (points; 0 picks 60).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GestureCfg {
    pub kinds: u32,
    pub reveal: f32,
    pub threshold: f32,
    pub leading: f32,
    pub full: f32,
}

/// What the gestures did to a node: flag bits above, the signed horizontal shift to paint a swiped
/// row's content at (right to left mirrors it), back progress 0..1 and the pull-down in points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GestureState {
    pub flags: u32,
    pub swipe: f32,
    pub back: f32,
    pub pull: f32,
}

/// A dismiss node's state: `DISMISS*` flag bits (8192/16384/32768, clear of the gesture flags) and the downward offset in points to translate the node by
/// (negative while rubber-banding upward, the node height once a commit has settled).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DismissState {
    pub flags: u32,
    pub offset: f32,
}

#[derive(Clone, Copy)]
pub(super) struct Spring {
    from: f64,
    to: f64,
    v0: f64,
    t0: f64,
    // at rest once within this of `to`
    eps: f64,
}
impl Spring {
    /// Position at `t`, and whether it has come to rest (then exactly `to`).
    fn at(&self, t: f64) -> (f64, bool) {
        let s = (t - self.t0).max(0.0);
        let c = self.from - self.to;
        let x = self.to + (c + (self.v0 + OMEGA * c) * s) * (-OMEGA * s).exp();
        if s > SETTLE_SECONDS || (s > 0.05 && (x - self.to).abs() < self.eps) {
            return (self.to, true);
        }
        (x, false)
    }
}

#[derive(Clone, Copy)]
pub(super) struct GSlot {
    pub(super) kinds: u32,
    pub(super) reveal: f32,
    pub(super) threshold: f32,
    pub(super) leading: f32,
    pub(super) full: f32,
    pub(super) highlighted: bool,
    pub(super) swiping: bool,
    /// the side a swipe rests open on: 1 trailing, -1 leading, 0 closed
    pub(super) open: i8,
    /// signed revealed amount, positive toward the trailing actions (past the width while stretched)
    pub(super) swipe: f32,
    /// the finger's unstretched travel from closed, same sign, for the full-swipe test
    swipe_raw: f32,
    swipe_spring: Option<Spring>,
    pub(super) back_active: bool,
    pub(super) back: f32,
    back_spring: Option<(Spring, bool)>,
    pub(super) refreshing: bool,
    dismiss_fraction: f32,
    dismiss_velocity: f32,
    dismiss: f32,
    dismiss_dragging: bool,
    dismiss_spring: Option<Spring>,
    pulses: u32,
    pulse_frame: i64,
}
impl GSlot {
    pub(super) const REST: GSlot = GSlot {
        kinds: 0, reveal: 0.0, threshold: 0.0, leading: 0.0, full: 0.0, highlighted: false, swiping: false, open: 0, swipe: 0.0, swipe_raw: 0.0, swipe_spring: None,
        back_active: false, back: 0.0, back_spring: None, refreshing: false, dismiss_fraction: 0.0, dismiss_velocity: 0.0, dismiss: 0.0, dismiss_dragging: false, dismiss_spring: None, pulses: 0, pulse_frame: -1,
    };
    pub(super) fn hold(&self) -> f64 {
        if self.refreshing { -f64::from(self.threshold_or_default()) } else { 0.0 }
    }
    pub(super) fn threshold_or_default(&self) -> f32 {
        if self.threshold > 0.0 { self.threshold } else { REFRESH_DISTANCE }
    }
}

/// UIScrollView's rubber band: x of travel past the edge shows as `(1 - 1/(x*0.55/d + 1)) * d`.
fn band(x: f64, d: f64) -> f64 {
    if d <= 0.0 { 0.0 } else { (1.0 - 1.0 / (x * 0.55 / d + 1.0)) * d }
}

impl Ui {
    fn sign(&self) -> f64 {
        if self.dir == Direction::Rtl { -1.0 } else { 1.0 }
    }

    pub(super) fn gpulse(&mut self, slot: usize, bits: u32, frame: i64) {
        let s = &mut self.gestures[slot];
        if s.pulse_frame != frame {
            (s.pulse_frame, s.pulses) = (frame, 0);
        }
        s.pulses |= bits;
    }

    fn gsettle_push(&mut self, slot: usize) {
        if !self.gsettle.contains(&slot) {
            self.gsettle.push(slot);
        }
    }

    /// The topmost node under `point` that takes gesture `kind` (modal-blocked ones do not count).
    pub(super) fn gesture_at(&mut self, point: (f32, f32), kind: u32) -> i64 {
        for i in (0..self.hit_count).rev() {
            let key = self.hits[i].key;
            if let Some(slot) = self.table.find(key).filter(|s| self.gestures[*s].kinds & kind != 0 && self.hit_contains(i, point)) {
                let edge = kind != EDGE_BACK || self.in_edge(slot, point);
                return if edge && self.modal_target(key) == key { key } else { 0 };
            }
        }
        0
    }

    fn in_edge(&self, slot: usize, point: (f32, f32)) -> bool {
        let s = &self.slots[slot];
        let from_leading = if self.dir == Direction::Rtl { f64::from(s.rx + s.rw - point.0) } else { f64::from(point.0 - s.rx) };
        (0.0..=EDGE_WIDTH).contains(&from_leading)
    }

    pub(super) fn gesture_width(&self, key: i64) -> f64 {
        self.table.find(key).map_or(0.0, |slot| f64::from(self.slots[slot].rw))
    }

    pub(super) fn swipe_amount(&self, key: i64) -> f64 {
        self.table.find(key).map_or(0.0, |slot| f64::from(self.gestures[slot].swipe))
    }

    pub(super) fn swipe_begin(&mut self, key: i64) -> f64 {
        let Some(slot) = self.table.find(key) else { return 0.0 };
        let s = &mut self.gestures[slot];
        (s.swipe_spring, s.swiping) = (None, true);
        f64::from(s.swipe)
    }

    /// Whether a swipe toward the trailing (`trailing`) or leading side has actions to reveal.
    pub(super) fn swipe_has(&self, key: i64, trailing: bool) -> bool {
        self.table.find(key).is_some_and(|slot| {
            let g = &self.gestures[slot];
            (if trailing { g.reveal } else { g.leading }) > 0.0
        })
    }

    /// Finger at `amount` points from closed, positive toward the trailing actions; a side without
    /// actions stays shut.
    pub(super) fn swipe_move(&mut self, key: i64, amount: f64) {
        let Some(slot) = self.table.find(key) else { return };
        let g = &mut self.gestures[slot];
        let width = f64::from(if amount > 0.0 { g.reveal } else { g.leading });
        let past = amount.abs();
        let shown = if past > width { width + band(past - width, width) } else { past };
        let live = width > 0.0;
        (g.swipe, g.swipe_raw) = if live { ((shown * amount.signum()) as f32, amount as f32) } else { (0.0, 0.0) };
        self.mark_key(key);
    }

    /// Lift: past the full-swipe distance commits that side's action (a cancel never does); else a
    /// flick decides, else which side of half way the row is on.
    pub(super) fn swipe_end(&mut self, key: i64, velocity: f64, now: f64, cancelled: bool) {
        let Some(slot) = self.table.find(key) else { return };
        let s = &mut self.gestures[slot];
        let (at, trailing, leading) = (f64::from(s.swipe), f64::from(s.reveal), f64::from(s.leading));
        let commit = !cancelled && s.full > 0.0 && s.swipe_raw.abs() >= s.full && s.swipe_raw * s.swipe > 0.0;
        let to = if commit {
            0.0
        } else if velocity.abs() >= SNAP_VELOCITY {
            if velocity > 0.0 { if at < 0.0 { 0.0 } else { trailing } } else if at > 0.0 { 0.0 } else { -leading }
        } else if at > trailing / 2.0 {
            trailing
        } else if at < -leading / 2.0 {
            -leading
        } else {
            0.0
        };
        let pulse = if s.swipe_raw > 0.0 { SWIPE_COMMIT_TRAILING } else { SWIPE_COMMIT_LEADING };
        (s.swiping, s.open, s.swipe_spring) = (false, if to > 0.0 { 1 } else if to < 0.0 { -1 } else { 0 }, Some(Spring { from: at, to, v0: velocity, t0: now, eps: 0.25 }));
        s.swipe_raw = 0.0;
        if commit {
            self.gpulse(slot, pulse, self.frame_number);
        }
        self.gsettle_push(slot);
        self.mark_key(key);
    }

    pub(super) fn back_move(&mut self, key: i64, progress: f64) {
        let Some(slot) = self.table.find(key) else { return };
        let s = &mut self.gestures[slot];
        (s.back_active, s.back_spring, s.back) = (true, None, progress.clamp(0.0, 1.0) as f32);
        self.mark_key(key);
    }

    /// Lift: `velocity` is points per second toward committing, across a screen `width` wide.
    pub(super) fn back_end(&mut self, key: i64, velocity: f64, width: f64, now: f64, cancelled: bool) {
        let Some(slot) = self.table.find(key) else { return };
        let s = &mut self.gestures[slot];
        let at = f64::from(s.back);
        let commit = !cancelled && if velocity.abs() >= BACK_VELOCITY { velocity > 0.0 } else { at >= 0.5 };
        let v0 = if width > 0.0 && !cancelled { velocity / width } else { 0.0 };
        s.back_spring = Some((Spring { from: at, to: f64::from(u8::from(commit)), v0, t0: now, eps: 0.002 }, commit));
        self.gsettle_push(slot);
        self.mark_key(key);
    }

    /// A downward drag may start a dismiss when no inner scroll holds the contact: UIKit hands a sheet the
    /// drag only when the content is at its top (or a glide is not being caught).
    pub(super) fn dismiss_ready(&self, scroll: i64, caught: bool) -> bool {
        !caught && self.table.find(scroll).is_none_or(|slot| self.scroll_y[slot] + self.glides[0][slot].visual as f32 <= 0.5)
    }

    /// The dismiss offset a drag continues from (a settling spring is caught where it is).
    pub(super) fn dismiss_begin(&mut self, key: i64) -> f64 {
        let Some(slot) = self.table.find(key) else { return 0.0 };
        let s = &mut self.gestures[slot];
        (s.dismiss_spring, s.dismiss_dragging) = (None, true);
        f64::from(s.dismiss)
    }

    /// Finger at `raw` points from rest: upward past 0 rubber-bands.
    pub(super) fn dismiss_move(&mut self, key: i64, raw: f64) {
        let Some(slot) = self.table.find(key) else { return };
        let height = f64::from(self.slots[slot].rh);
        self.gestures[slot].dismiss = (if raw < 0.0 { -band(-raw, height) } else { raw }) as f32;
        self.mark_key(key);
    }

    /// Lift: a downward flick or the distance commits (a cancel never does); the offset settles to the height or 0.
    pub(super) fn dismiss_end(&mut self, key: i64, velocity: f64, now: f64, cancelled: bool) {
        let Some(slot) = self.table.find(key) else { return };
        let height = self.slots[slot].rh;
        let s = &mut self.gestures[slot];
        let (fraction, fast) = (if s.dismiss_fraction > 0.0 { s.dismiss_fraction } else { DISMISS_FRACTION }, if s.dismiss_velocity > 0.0 { s.dismiss_velocity } else { DISMISS_VELOCITY });
        let commit = !cancelled && s.dismiss > 0.0 && (velocity >= f64::from(fast) || s.dismiss >= fraction * height);
        let to = if commit { f64::from(height) } else { 0.0 };
        (s.dismiss_dragging, s.dismiss_spring) = (false, Some(Spring { from: f64::from(s.dismiss), to, v0: if cancelled { 0.0 } else { velocity }, t0: now, eps: 0.25 }));
        self.gpulse(slot, if commit { DISMISS_COMMIT } else { DISMISS_CANCEL }, self.frame_number);
        self.gsettle_push(slot);
        self.mark_key(key);
    }

    pub(super) fn set_highlight(&mut self, key: i64, on: bool) {
        if let Some(slot) = self.table.find(key).filter(|s| self.gestures[*s].highlighted != on) {
            self.gestures[slot].highlighted = on;
            self.mark_key(key);
        }
    }

    pub(super) fn flash_highlight(&mut self, key: i64) {
        if let Some(slot) = self.table.find(key) {
            self.gpulse(slot, HIGHLIGHTED, self.frame_number);
        }
    }

    pub(super) fn long_press(&mut self, key: i64) {
        if let Some(slot) = self.table.find(key) {
            self.gestures[slot].highlighted = false;
            self.gpulse(slot, LONG_PRESSED, self.frame_number);
            self.mark_key(key);
        }
    }

    /// Per frame, after input: arm the highlight/long-press timers and advance the settle springs.
    pub(super) fn gesture_tick(&mut self, t: f64) {
        self.touches.wake = f64::INFINITY;
        self.press_tick(t);
        let mut list = std::mem::take(&mut self.gsettle);
        let frame = self.frame_number;
        list.retain(|&slot| {
            let mut live = false;
            let s = &mut self.gestures[slot];
            if let Some(sp) = s.swipe_spring {
                let (x, done) = sp.at(t);
                s.swipe = x as f32;
                if done { s.swipe_spring = None } else { live = true }
            }
            if let Some(sp) = s.dismiss_spring {
                let (x, done) = sp.at(t);
                s.dismiss = x as f32;
                if done { s.dismiss_spring = None } else { live = true }
            }
            if let Some((sp, commit)) = s.back_spring {
                let (x, done) = sp.at(t);
                s.back = x as f32;
                if done {
                    (s.back_spring, s.back, s.back_active) = (None, 0.0, false);
                    self.gpulse(slot, if commit { BACK_COMMIT } else { BACK_CANCEL }, frame);
                } else {
                    live = true;
                }
            }
            live
        });
        self.gsettle = list;
    }

    /// Enable the row swipe with action widths on both sides (0 disables a side) and a full-swipe
    /// commit distance (0: none), keeping the node's other gestures.
    pub fn swipe_actions(&mut self, b: &UiBox, leading: f32, trailing: f32, full: f32) {
        if self.live(b) {
            let finite = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
            let s = &mut self.gestures[b.box_slot];
            (s.kinds, s.leading, s.reveal, s.full) = (s.kinds | ROW_SWIPE, finite(leading), finite(trailing), finite(full));
        }
    }

    /// Take the vertical drag-to-dismiss on a node (call after `gesture` each build, which resets the node's kinds) and
    /// read it. `fraction` of the node height (0 picks 0.25) or a downward release at `velocity` points/s (0 picks 600)
    /// commits; the app translates the node by `offset` and closes on the commit pulse.
    pub fn dismiss(&mut self, b: &UiBox, fraction: f32, velocity: f32) -> DismissState {
        if !self.live(b) {
            return DismissState::default();
        }
        let finite = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
        let s = &mut self.gestures[b.box_slot];
        (s.kinds, s.dismiss_fraction, s.dismiss_velocity) = (s.kinds | DISMISS, finite(fraction), finite(velocity));
        let pulses = if s.pulse_frame == self.frame_number { s.pulses & (DISMISS_COMMIT | DISMISS_CANCEL) } else { 0 };
        DismissState { flags: pulses | if s.dismiss_dragging || s.dismiss_spring.is_some() { DISMISSING } else { 0 }, offset: s.dismiss }
    }

    /// Read (and, with `cfg`, set) a node's gesture state; `commands` close a swiped row or end a
    /// refresh. A stale handle reads as rest.
    pub fn gesture(&mut self, b: &UiBox, cfg: Option<GestureCfg>, commands: u32) -> GestureState {
        if !self.live(b) {
            return GestureState::default();
        }
        let slot = b.box_slot;
        let now = self.now;
        if let Some(c) = cfg {
            let s = &mut self.gestures[slot];
            let finite = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
            (s.kinds, s.reveal, s.threshold, s.leading, s.full) = (c.kinds, finite(c.reveal), finite(c.threshold), finite(c.leading), finite(c.full));
        }
        if commands & CLOSE_SWIPE != 0 {
            let s = &mut self.gestures[slot];
            if !s.swiping && (s.open != 0 || s.swipe != 0.0) {
                (s.open, s.swipe_spring) = (0, Some(Spring { from: f64::from(s.swipe), to: 0.0, v0: 0.0, t0: now, eps: 0.25 }));
                self.gsettle_push(slot);
            }
        }
        if commands & END_REFRESH != 0 && self.gestures[slot].refreshing {
            self.gestures[slot].refreshing = false;
            if !self.finger_on_scroll(b.box_key) && self.glides[0][slot].visual != 0.0 {
                self.glides[0][slot].event_time = f64::NEG_INFINITY;
                if !self.moving.contains(&slot) {
                    self.moving.push(slot);
                }
            }
        }
        let s = &self.gestures[slot];
        let pulses = if s.pulse_frame == self.frame_number { s.pulses } else { 0 };
        let mut flags = pulses;
        for (on, bit) in [(s.highlighted, HIGHLIGHTED), (s.swiping, SWIPING), (s.open != 0, SWIPE_OPEN), (s.open > 0, SWIPE_OPEN_TRAILING), (s.open < 0, SWIPE_OPEN_LEADING), (s.back_active, BACK_ACTIVE), (s.refreshing, REFRESHING)] {
            if on {
                flags |= bit;
            }
        }
        GestureState { flags, swipe: (-self.sign() * f64::from(s.swipe)) as f32, back: s.back, pull: (-self.glides[0][slot].visual).max(0.0) as f32 }
    }
}
