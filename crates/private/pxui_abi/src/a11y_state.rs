use crate::{PxuiUi, get};
use pxui_core::{a11y::Role, ui::UiBox};
use pxui_types::PxuiBox;

fn core_box(raw: PxuiBox) -> Option<UiBox> {
    Some(UiBox { index: usize::try_from(raw.index).ok()?, box_key: raw.key, box_slot: usize::try_from(raw.slot).ok()?, generation: raw.generation })
}

fn live_box(engine: &pxui_core::ui::Ui, raw: PxuiBox) -> Option<UiBox> {
    let b = core_box(raw)?;
    engine.live(&b).then_some(b)
}

fn valid_number(now: f64, min: f64, max: f64, step: f64) -> bool {
    [now, min, max, step].iter().all(|v| v.is_finite()) && min <= now && now <= max && step >= 0.0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_a11y_number(ui: *mut PxuiUi, box_: *const PxuiBox, now: f64, min: f64, max: f64, step: f64) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if box_.is_null() || !valid_number(now, min, max, step) {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid accessibility number") };
    }
    let engine = unsafe { &mut *state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { crate::fail(state as *mut PxuiUi, "stale accessibility box") }; };
    engine.a11y_number(&box_, now, min, max, step);
    0
}

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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_set_role(ui: *mut PxuiUi, box_: *const PxuiBox, role: u32, states: u32) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if box_.is_null() || role > Role::Separator as u32 { return unsafe { crate::fail(state as *mut PxuiUi, "invalid role arguments") }; }
    let engine = unsafe { &mut *state.engine };
    let Some(box_) = live_box(engine, unsafe { *box_ }) else { return unsafe { crate::fail(state as *mut PxuiUi, "stale accessibility box") }; };
    let role = unsafe { core::mem::transmute::<u8, Role>(role as u8) };
    engine.set_role(&box_, role, states);
    0
}
