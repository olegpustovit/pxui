use crate::state::{PxuiUi, fail, get, live_box};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox};
use pxui_core::a11y::Role;

fn valid_number(now: f64, min: f64, max: f64, step: f64) -> bool {
    [now, min, max, step].iter().all(|v| v.is_finite()) && min <= now && now <= max && step >= 0.0
}

export!(pub unsafe fn pxui_ui_a11y_number(ui: *mut PxuiUi, box_: *const PxuiBox, now: f64, min: f64, max: f64, step: f64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if box_.is_null() || !valid_number(now, min, max, step) {
        return unsafe { fail(ui, "invalid accessibility number") };
    }
    let engine = unsafe { &mut *state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { fail(ui, "stale accessibility box") }; };
    engine.a11y_number(&box_, now, min, max, step);
    0
});

#[cfg(test)]
mod tests {
    use super::valid_number;

    #[test]
    fn accessibility_numbers_require_ordered_finite_bounds() {
        assert!(valid_number(2.0, 0.0, 4.0, 0.5));
        assert!(!valid_number(f64::NAN, 0.0, 4.0, 0.5));
        assert!(!valid_number(5.0, 0.0, 4.0, 0.5));
        assert!(!valid_number(2.0, 3.0, 1.0, 0.5));
        assert!(!valid_number(2.0, 0.0, 4.0, -0.5));
    }
}

export!(pub unsafe fn pxui_ui_set_role(ui: *mut PxuiUi, box_: *const PxuiBox, role: u32, states: u32) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT; };
    if box_.is_null() || role > Role::Separator as u32 { return unsafe { fail(ui, "invalid role arguments") }; }
    let engine = unsafe { &mut *state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { fail(ui, "stale accessibility box") }; };
    let role = unsafe { core::mem::transmute::<u8, Role>(role as u8) };
    engine.set_role(&box_, role, states);
    0
});
