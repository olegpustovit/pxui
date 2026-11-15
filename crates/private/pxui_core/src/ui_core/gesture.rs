//! Touch gestures above scroll: delayed highlight and long press, horizontal row swipe, interactive
//! edge back and pull to refresh. The finger side lives in `touch`; this holds the per-node retained
//! state, the closed-form settle spring and the app-facing read. Touch input only: a mouse or
//! trackpad never starts one. Timing runs on the frame clock, so a held finger needs no events.
use super::{Direction, Ui, UiBox};

pub const LONG_PRESS: u32 = 1;
pub const ROW_SWIPE: u32 = 2;
pub const EDGE_BACK: u32 = 4;
pub const REFRESH: u32 = 8;
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

// UITableViewCell/UIScrollView touch delay (~0.1 s) before a cell highlights; a quick tap still flashes it.
pub(super) const HIGHLIGHT_DELAY: f64 = 0.1;
// UILongPressGestureRecognizer.minimumPressDuration.
pub(super) const LONG_PRESS_DELAY: f64 = 0.5;
// UIScreenEdgePanGestureRecognizer's edge band (~20 pt), measured from the leading edge.
pub(super) const EDGE_WIDTH: f64 = 20.0;
// A flick this fast (points/s) decides a swipe or a back; slower releases decide by position.
pub(super) const SNAP_VELOCITY: f64 = 300.0;
pub(super) const BACK_VELOCITY: f64 = 500.0;
// Default UIRefreshControl pull distance, points.
const REFRESH_DISTANCE: f32 = 60.0;
// Critically damped settle (rad/s): about 0.35 s to rest, the order of UINavigationController's pop.
const OMEGA: f64 = 20.0;
const SETTLE_SECONDS: f64 = 0.6;

/// Per-node gesture request: which gestures the node takes, the row-swipe action width and the
/// refresh pull distance (points; 0 picks 60).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GestureCfg {
    pub kinds: u32,
    pub reveal: f32,
    pub threshold: f32,
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
    pub(super) highlighted: bool,
    pub(super) swiping: bool,
    pub(super) open: bool,
    /// revealed amount 0..reveal (past it while stretched), direction neutral
    pub(super) swipe: f32,
    swipe_spring: Option<Spring>,
    pub(super) back_active: bool,
    pub(super) back: f32,
    back_spring: Option<(Spring, bool)>,
    pub(super) refreshing: bool,
    pulses: u32,
    pulse_frame: i64,
}
impl GSlot {
    pub(super) const REST: GSlot = GSlot {
        kinds: 0, reveal: 0.0, threshold: 0.0, highlighted: false, swiping: false, open: false, swipe: 0.0, swipe_spring: None,
        back_active: false, back: 0.0, back_spring: None, refreshing: false, pulses: 0, pulse_frame: -1,
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

    /// Finger at `amount` points toward the actions past where the swipe began.
    pub(super) fn swipe_move(&mut self, key: i64, amount: f64) {
        let Some(slot) = self.table.find(key) else { return };
        let reveal = f64::from(self.gestures[slot].reveal);
        let shown = if amount > reveal { reveal + band(amount - reveal, reveal) } else { amount.max(0.0) };
        self.gestures[slot].swipe = shown as f32;
        self.mark_key(key);
    }

    /// Lift: a flick toward or away decides, else which side of half way the row is on.
    pub(super) fn swipe_end(&mut self, key: i64, velocity: f64, now: f64) {
        let Some(slot) = self.table.find(key) else { return };
        let s = &mut self.gestures[slot];
        let at = f64::from(s.swipe);
        let open = if velocity.abs() >= SNAP_VELOCITY { velocity > 0.0 } else { at > f64::from(s.reveal) / 2.0 };
        let to = if open { f64::from(s.reveal) } else { 0.0 };
        (s.swiping, s.open, s.swipe_spring) = (false, open, Some(Spring { from: at, to, v0: velocity, t0: now, eps: 0.25 }));
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
            (s.kinds, s.reveal, s.threshold) = (c.kinds, finite(c.reveal), finite(c.threshold));
        }
        if commands & CLOSE_SWIPE != 0 {
            let s = &mut self.gestures[slot];
            if !s.swiping && (s.open || s.swipe > 0.0) {
                (s.open, s.swipe_spring) = (false, Some(Spring { from: f64::from(s.swipe), to: 0.0, v0: 0.0, t0: now, eps: 0.25 }));
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
        for (on, bit) in [(s.highlighted, HIGHLIGHTED), (s.swiping, SWIPING), (s.open, SWIPE_OPEN), (s.back_active, BACK_ACTIVE), (s.refreshing, REFRESHING)] {
            if on {
                flags |= bit;
            }
        }
        GestureState { flags, swipe: (-self.sign() * f64::from(s.swipe)) as f32, back: s.back, pull: (-self.glides[0][slot].visual).max(0.0) as f32 }
    }
}
