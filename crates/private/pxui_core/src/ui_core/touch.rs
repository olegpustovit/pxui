use super::{SCROLL, SCROLL_X, Ui, p32};
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
    // Pending, pointer, vertical scroll, horizontal scroll.
    mode: u8,
    axis: u8,
    velocity: (f64, f64),
    caught: bool,
}

pub(super) struct Touches {
    pub(super) using_touch: bool,
    fingers: [Option<Id>; 16],
    gesture: Option<Gesture>,
    blocked: bool,
    // shortcut: 16 contacts; overflow cancels until focus resets, grow only if a host needs more.
    overflow: bool,
}
impl Touches {
    pub(super) const EMPTY: Self = Self {
        using_touch: false,
        fingers: [None; 16],
        gesture: None,
        blocked: false,
        overflow: false,
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
        }
        events
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
            let vertical = self.scroll_target_flags(p32(t.point), SCROLL);
            let horizontal = self.scroll_target_flags(p32(t.point), SCROLL_X);
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
            let mode = if scroll == [0, 0] && owner != 0 { 1 } else { 0 };
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
            if gesture.mode >= 2 {
                let axis = usize::from(gesture.mode == 3);
                let dt = t.time - gesture.time;
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
            if gesture.mode >= 2 && gesture.point != t.point {
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
            self.finish_touch(gesture, false, t.time);
            self.touches.gesture = None;
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
