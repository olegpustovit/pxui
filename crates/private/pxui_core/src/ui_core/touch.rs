use super::gesture::{DISMISS, EDGE_BACK, HIGHLIGHT_DELAY, LONG_PRESS, LONG_PRESS_DELAY, ROW_SWIPE, TOUCH_DRAG};
use super::{Direction, SCROLL, SCROLL_X, Ui, p32};
use input::{Event, MouseButton, TouchPhase};

type Id = (u64, u64);
#[derive(Clone, Copy)]
struct Gesture {
    id: Id,
    owner: i64,
    start: (f64, f64),
    point: (f64, f64),
    time: f64,
    scroll: [i64; 2],
    // Pending, pointer, vertical scroll, horizontal scroll, row swipe, edge back, long press, dismiss.
    mode: u8,
    axis: u8,
    velocity: (f64, f64),
    caught: bool,
    // Nodes taking a long press, a row swipe or an edge back under the first contact (0: none).
    press: i64,
    swipe: i64,
    edge: i64,
    dismiss: i64,
    // Frame time of touch down; the revealed amount a swipe began from; the screen's width.
    down: f64,
    base: f64,
    width: f64,
    // Horizontal speed of the last segment, points per second.
    vx: f64,
    vy: f64,
}

pub(super) struct Touches {
    pub(super) using_touch: bool,
    fingers: [Option<Id>; 16],
    gesture: Option<Gesture>,
    blocked: bool,
    // shortcut: 16 contacts; overflow cancels until focus resets, grow only if a host needs more.
    overflow: bool,
    // The frame clock of the frame routing input: gesture springs and timers run on it.
    pub(super) clock: f64,
    // When a pending press next needs a frame (infinity: none); rearmed each frame.
    pub(super) wake: f64,
}
impl Touches {
    pub(super) const EMPTY: Self = Self {
        using_touch: false,
        fingers: [None; 16],
        gesture: None,
        blocked: false,
        overflow: false,
        clock: 0.0,
        wake: f64::INFINITY,
    };
}

impl Ui {
    pub(super) fn touch_owner(&self, event: &Event) -> Option<i64> {
        let Event::Touch(t) = event else {
            return None;
        };
        self.touches
            .gesture
            .filter(|g| g.id == (t.device, t.finger) && self.active.is_none())
            .map(|g| g.owner)
    }

    fn finish_touch(&mut self, gesture: Gesture, cancelled: bool, time: f64) {
        for (axis, key) in gesture.scroll.into_iter().enumerate() {
            if key == 0 || (axis == 1 && key == gesture.scroll[0]) {
                continue;
            }
            if cancelled && let Some(slot) = self.table.find(key) {
                for glides in &mut self.glides {
                    glides[slot].mode = 0;
                    glides[slot].event_time = f64::NEG_INFINITY;
                }
                if self.glides.iter().any(|axis| axis[slot].moving())
                    && !self.moving.contains(&slot)
                {
                    self.moving.push(slot);
                }
            }
            let value = self.accumulator(key);
            value.scroll_lift = true;
            value.scroll_finger = true;
            value.scroll_velocity = if !cancelled && time - gesture.time <= 0.1 {
                gesture.velocity
            } else {
                (0.0, 0.0)
            };
        }
    }

    fn cancel_touch(&mut self) -> [Option<Event>; 3] {
        let mut events = [None, None, None];
        if let Some(gesture) = self.touches.gesture.take() {
            self.finish_touch(gesture, true, gesture.time);
            if gesture.mode == 1 {
                events[0] = Some(Event::PointerCancelled(MouseButton::LeftButton));
            }
            self.end_special(gesture, (0.0, 0.0), true);
        }
        events
    }

    /// A swipe settles by position (a cancel has no flick) and a cancelled back returns; a pending
    /// press loses its highlight. `vx` is the lift's horizontal speed, screen points per second.
    fn end_special(&mut self, g: Gesture, (vx, vy): (f64, f64), cancelled: bool) {
        let (s, clock) = (if self.dir == Direction::Rtl { -1.0 } else { 1.0 }, self.touches.clock);
        if g.press != 0 {
            self.set_highlight(g.press, false);
        }
        match g.mode {
            4 => self.swipe_end(g.swipe, -vx * s, clock, cancelled),
            5 => self.back_end(g.edge, vx * s, g.width, clock, cancelled),
            7 => self.dismiss_end(g.dismiss, vy, clock, cancelled),
            _ => {}
        }
    }

    pub(super) fn finger_on_scroll(&self, key: i64) -> bool {
        self.touches.gesture.is_some_and(|g| g.scroll[0] == key)
    }

    /// Timers of a pending contact on a long-press node: the highlight shows after a short delay,
    /// the long press fires and takes the contact, and each asks for a frame when it is due.
    pub(super) fn press_tick(&mut self, t: f64) {
        let Some(mut g) = self.touches.gesture.filter(|g| g.mode == 0 && g.press != 0) else {
            return;
        };
        let elapsed = t - g.down;
        if elapsed >= LONG_PRESS_DELAY {
            g.mode = 6;
            self.touches.gesture = Some(g);
            self.long_press(g.press);
        } else {
            let lit = elapsed >= HIGHLIGHT_DELAY;
            if lit {
                self.set_highlight(g.press, true);
            }
            self.touches.wake = g.down + if lit { LONG_PRESS_DELAY } else { HIGHLIGHT_DELAY };
        }
    }

    pub(super) fn cancel_touch_for_popup(&mut self, root: Option<i64>, related: &[i64]) {
        if let Some(gesture) = self.touches.gesture
            && root.is_none_or(|key| !self.belongs(gesture.owner, key, related))
        {
            self.cancel_touch();
            self.touches.blocked = self.touches.fingers.iter().any(Option::is_some);
            if self.active.is_none() {
                self.cancelled.push(MouseButton::LeftButton);
            }
        }
    }

    /// Numeric actions use the existing pointer router; only the original Touch is exported to
    /// nested viewports, so an inner UI never synthesizes the same pointer press twice.
    pub(super) fn touch_events(&mut self, event: &Event) -> [Option<Event>; 3] {
        let none = [None, None, None];
        if matches!(event, Event::PointerPressed(..)) && self.touches.gesture.is_some() {
            let held = self.touches.gesture.is_some_and(|g| g.mode == 1);
            self.cancel_touch();
            self.touches.blocked = self.touches.fingers.iter().any(Option::is_some);
            if held {
                self.active = None;
                if matches!(self.payload, Some((_, super::Origin::FromPointer(_)))) {
                    self.retire_payload();
                }
            }
            self.routed_events.push((
                0,
                Event::PointerCancelled(MouseButton::LeftButton),
                (0.0, 0.0),
            ));
        }
        if matches!(
            event,
            Event::PointerMoved(_) | Event::PointerPressed(..) | Event::PointerReleased(..)
        ) {
            self.touches.using_touch = false;
        }
        if matches!(event, Event::FocusLost) {
            self.cancel_touch();
            let using_touch = self.touches.using_touch;
            self.touches = Touches::EMPTY;
            self.touches.using_touch = using_touch;
            return none;
        }
        if matches!(event, Event::PointerCancelled(MouseButton::LeftButton)) {
            self.cancel_touch();
            self.touches.blocked = self.touches.fingers.iter().any(Option::is_some);
            return none;
        }
        let Event::Touch(t) = event else {
            return none;
        };
        if !t.valid() {
            return none;
        }
        self.touches.using_touch = true;
        let id = (t.device, t.finger);
        if t.phase == TouchPhase::Began {
            if self.touches.fingers.contains(&Some(id)) {
                return none;
            }
            let Some(slot) = self.touches.fingers.iter_mut().find(|slot| slot.is_none()) else {
                self.touches.overflow = true;
                return self.cancel_touch();
            };
            *slot = Some(id);
            if self.touches.fingers.iter().flatten().count() != 1
                || self.touches.blocked
                || self.touches.overflow
                || self.active.is_some()
            {
                self.touches.blocked = true;
                return self.cancel_touch();
            }
            let owner = self.topmost(p32(t.point));
            let owner = self.modal_target(owner);
            // A touch-drag node under the contact takes it now, with no arbitration against scrolls or the swipes and presses of rows beneath it.
            let claim = self.gesture_at(p32(t.point), TOUCH_DRAG);
            let claimed = claim != 0 && owner != 0 && self.hit_within(owner, claim);
            let vertical = if claimed { 0 } else { self.scroll_target_flags(p32(t.point), SCROLL) };
            let horizontal = if claimed { 0 } else { self.scroll_target_flags(p32(t.point), SCROLL_X) };
            let scroll = [vertical, horizontal].map(|key| {
                if self.modal_target(key) == key {
                    key
                } else {
                    0
                }
            });
            let mut caught = false;
            for (axis, key) in scroll.into_iter().enumerate() {
                if key == 0 {
                    continue;
                }
                if let Some(slot) = self.table.find(key) {
                    caught |= self.glides[axis][slot].moving();
                }
                let value = self.accumulator(key);
                value.scroll_touch = true;
                value.scroll_lift = false;
                value.scroll_finger = true;
                value.scroll_velocity = (0.0, 0.0);
            }
            let point = p32(t.point);
            let at = |ui: &mut Self, kind| if claimed { 0 } else { ui.gesture_at(point, kind) };
            let press = if caught { 0 } else { at(self, LONG_PRESS) };
            let (swipe, edge, dismiss) = (at(self, ROW_SWIPE), at(self, EDGE_BACK), at(self, DISMISS));
            // A contact that may become a swipe, back or long press waits like one on a scroll.
            let mode = if scroll == [0, 0] && owner != 0 && press == 0 && swipe == 0 && edge == 0 && dismiss == 0 { 1 } else { 0 };
            self.touches.gesture = Some(Gesture {
                id,
                owner,
                start: t.point,
                point: t.point,
                time: t.time,
                scroll,
                mode,
                axis: 0,
                velocity: (0.0, 0.0),
                caught,
                press,
                swipe,
                edge,
                dismiss,
                down: self.touches.clock,
                base: 0.0,
                width: self.gesture_width(edge),
                vx: 0.0,
                vy: 0.0,
            });
            return if mode == 1 {
                [
                    Some(Event::PointerMoved(t.point)),
                    Some(Event::PointerPressed(MouseButton::LeftButton, t.point)),
                    None,
                ]
            } else {
                none
            };
        }
        let Some(slot) = self
            .touches
            .fingers
            .iter_mut()
            .find(|slot| **slot == Some(id))
        else {
            return none;
        };
        if matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            *slot = None;
        }
        if self.touches.blocked || self.touches.overflow {
            if self.touches.fingers.iter().all(Option::is_none) {
                self.touches.blocked = false;
            }
            return none;
        }
        let Some(mut gesture) = self.touches.gesture else {
            return none;
        };
        if gesture.id != id {
            return none;
        }
        if t.phase == TouchPhase::Cancelled
            || t.time < gesture.time
            || self.hit_index(gesture.owner).is_none()
            || self.modal_target(gesture.owner) != gesture.owner
        {
            self.touches.blocked = self.touches.fingers.iter().any(Option::is_some);
            return self.cancel_touch();
        }
        let travel = (t.point.0 - gesture.start.0, t.point.1 - gesture.start.1);
        let mut actions = none;
        if t.phase == TouchPhase::Moved {
            if gesture.axis == 0 && travel.0 * travel.0 + travel.1 * travel.1 >= 64.0 {
                gesture.axis = if travel.1.abs() >= travel.0.abs() {
                    1
                } else {
                    2
                };
            }
            let mut delta = (t.point.0 - gesture.point.0, t.point.1 - gesture.point.1);
            let dt = t.time - gesture.time;
            (gesture.vx, gesture.vy) = if dt > 0.0 && dt <= 0.1 { (delta.0 / dt, delta.1 / dt) } else { (0.0, 0.0) };
            let s = if self.dir == Direction::Rtl { -1.0 } else { 1.0 };
            // At the axis lock a horizontal move inward from the leading edge is a back, and one
            // toward the actions of a swipe row (or any on an open one) is a swipe.
            if gesture.mode == 0 && gesture.axis != 0 && gesture.edge != 0 {
                if gesture.axis == 2 && travel.0 * s > 0.0 {
                    gesture.mode = 5;
                    gesture.owner = gesture.edge;
                } else {
                    gesture.edge = 0;
                }
            }
            // A downward vertical drag on a dismiss node takes it unless an inner scroll not at its top owns it.
            if gesture.mode == 0 && gesture.axis == 1 && gesture.dismiss != 0 {
                if travel.1 > 0.0 && self.dismiss_ready(gesture.scroll[0], gesture.caught) {
                    (gesture.mode, gesture.base, gesture.owner) = (7, self.dismiss_begin(gesture.dismiss) - travel.1, gesture.dismiss);
                } else {
                    gesture.dismiss = 0;
                }
            }
            if gesture.mode == 0 && gesture.axis == 2 && gesture.swipe != 0 {
                // toward the trailing actions is -s; a closed row needs actions on the side it moves to
                if self.swipe_amount(gesture.swipe) != 0.0 || self.swipe_has(gesture.swipe, travel.0 * s < 0.0) {
                    (gesture.mode, gesture.base, gesture.owner) = (4, self.swipe_begin(gesture.swipe), gesture.swipe);
                } else {
                    gesture.swipe = 0;
                }
            }
            if gesture.mode == 0 && travel.0 * travel.0 + travel.1 * travel.1 >= 100.0 {
                let axis = usize::from(gesture.axis == 2);
                gesture.mode = if gesture.scroll[axis] != 0 {
                    2 + axis as u8
                } else {
                    1
                };
                if gesture.mode == 1 {
                    actions[0] = Some(Event::PointerPressed(
                        MouseButton::LeftButton,
                        gesture.start,
                    ));
                    actions[1] = Some(Event::PointerMoved(t.point));
                } else {
                    gesture.owner = gesture.scroll[axis];
                    delta = travel;
                }
            } else if gesture.mode == 1 {
                actions[0] = Some(Event::PointerMoved(t.point));
            }
            if gesture.mode != 0 && gesture.mode != 6 && gesture.press != 0 {
                self.set_highlight(gesture.press, false);
                gesture.press = 0;
            }
            if gesture.mode == 4 {
                self.swipe_move(gesture.swipe, gesture.base - (t.point.0 - gesture.start.0) * s);
            } else if gesture.mode == 7 {
                self.dismiss_move(gesture.dismiss, gesture.base + travel.1);
            } else if gesture.mode == 5 && gesture.width > 0.0 {
                self.back_move(gesture.edge, (t.point.0 - gesture.start.0) * s / gesture.width);
            }
            if matches!(gesture.mode, 2 | 3) {
                let axis = usize::from(gesture.mode == 3);
                let speed = if dt > 0.0 && dt <= 0.1 {
                    let d = if axis == 0 {
                        gesture.point.1 - t.point.1
                    } else {
                        gesture.point.0 - t.point.0
                    };
                    d / dt
                } else {
                    0.0
                };
                gesture.velocity = if axis == 0 {
                    (speed, 0.0)
                } else {
                    (0.0, speed)
                };
                let value = self.accumulator(gesture.scroll[axis]);
                value.scroll_finger = true;
                if axis == 0 {
                    value.scroll_drag += delta.1 as f32;
                } else {
                    value.scroll_drag_x += delta.0 as f32;
                }
            }
            gesture.point = t.point;
            gesture.time = t.time;
            self.touches.gesture = Some(gesture);
        } else if t.phase == TouchPhase::Ended {
            if matches!(gesture.mode, 2 | 3) && gesture.point != t.point {
                let axis = usize::from(gesture.mode == 3);
                let delta = if axis == 0 {
                    t.point.1 - gesture.point.1
                } else {
                    t.point.0 - gesture.point.0
                };
                let dt = t.time - gesture.time;
                if dt > 0.0 && dt <= 0.1 {
                    gesture.velocity = if axis == 0 {
                        (-delta / dt, 0.0)
                    } else {
                        (0.0, -delta / dt)
                    };
                }
                let value = self.accumulator(gesture.scroll[axis]);
                if axis == 0 {
                    value.scroll_drag += delta as f32;
                } else {
                    value.scroll_drag_x += delta as f32;
                }
            }
            let special = matches!(gesture.mode, 4..=7);
            self.finish_touch(gesture, special, t.time);
            self.touches.gesture = None;
            // a lift after a pause carries no flick; one at the last point keeps the last segment's
            let dt = t.time - gesture.time;
            let v = if dt > 0.1 {
                (0.0, 0.0)
            } else if dt > 0.0 && gesture.point != t.point {
                ((t.point.0 - gesture.point.0) / dt, (t.point.1 - gesture.point.1) / dt)
            } else {
                (gesture.vx, gesture.vy)
            };
            self.end_special(gesture, v, false);
            if gesture.mode == 1 {
                actions = [
                    Some(Event::PointerMoved(t.point)),
                    Some(Event::PointerReleased(MouseButton::LeftButton, t.point)),
                    None,
                ];
            } else if gesture.mode == 0
                && !gesture.caught
                && travel.0 * travel.0 + travel.1 * travel.1 < 100.0
                && self.topmost(p32(gesture.start)) == gesture.owner
            {
                if gesture.press != 0 {
                    self.flash_highlight(gesture.press);
                }
                actions = [
                    Some(Event::PointerMoved(gesture.start)),
                    Some(Event::PointerPressed(
                        MouseButton::LeftButton,
                        gesture.start,
                    )),
                    Some(Event::PointerReleased(MouseButton::LeftButton, t.point)),
                ];
            }
        }
        actions
    }
}
