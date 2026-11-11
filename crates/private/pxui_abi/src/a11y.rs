use crate::{PxuiUi, get, text};
use pxui_core::ui::UiBox;
use pxui_types::{PxuiBox, Str};

const MAX_NAME_BYTES: usize = 1 << 20;

fn core_box(raw: PxuiBox) -> Option<UiBox> {
    Some(UiBox {
        index: usize::try_from(raw.index).ok()?,
        box_key: raw.key,
        box_slot: usize::try_from(raw.slot).ok()?,
        generation: raw.generation,
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_a11y_name(ui: *mut PxuiUi, box_: *const PxuiBox, name: Str) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return -1; };
    if box_.is_null() || name.len > MAX_NAME_BYTES {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid accessibility name arguments") };
    }
    let Some(name) = (unsafe { text(name) }) else {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid accessibility name UTF-8") };
    };
    let Some(box_) = core_box(unsafe { *box_ }) else {
        return unsafe { crate::fail(state as *mut PxuiUi, "invalid accessibility box") };
    };
    unsafe { (&mut *state.engine).a11y_name(&box_, name); }
    0
}
