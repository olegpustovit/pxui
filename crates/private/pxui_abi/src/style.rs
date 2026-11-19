use pxui_core::ui::{Look, State, UiBox};
use pxui_types::ffi::{PxuiLookDesc, PxuiStateDesc};
use crate::state::{PxuiUi, core_box, fail, get};
use crate::node::{look, state};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox};

pub(crate) fn decode_look(value:PxuiLookDesc)->Option<Look> {
    valid_look(&value).then(|| look(value))
}

/// The descriptor's tags and reserved bytes are in range and its numbers finite: what `decode_look` checks before converting.
fn valid_look(value:&PxuiLookDesc)->bool {
    if value.has_gradient>1 || value.reserved!=[0;3] || value.reserved_tail!=[0;3]
        || value.dashed>1 || value.font.caps>1 || value.font.reserved!=[0;3]
        || value.ring.sides&!0xF0!=0 || value.sides&!0xF0!=0
        || value.shadows.iter().any(|s|s.inner>1 || s.reserved!=[0;3])
    { return false; }
    let [a,b]=value.shadows;
    let numbers=[value.angle,value.border_w,value.opacity,value.ring.width,value.font.tracking,
        value.radius[0],value.radius[1],value.radius[2],value.radius[3],
        a.offset[0],a.offset[1],a.blur,a.spread,b.offset[0],b.offset[1],b.blur,b.spread];
    numbers.iter().all(|number|number.is_finite())
}

/// The index, in `pxui_ui_style`'s order (idle, hover, pressed, focus, disabled, checked), of the look `States::pick` takes for `s`.
fn picked(s:State)->usize {
    if s.disabled { 4 } else if s.pressed { 2 } else if s.hovered { 1 } else if s.checked { 5 } else if s.focused { 3 } else { 0 }
}

fn decode_state(value:PxuiStateDesc)->Option<State> {
    if value.reserved!=[0;3] || [value.hovered,value.pressed,value.focused,value.disabled,value.checked].iter().any(|v|*v>1) { return None; }
    Some(state(value))
}

unsafe fn decode_args<'a>(ui:*mut PxuiUi,box_:*const PxuiBox,state_:*const PxuiStateDesc)->Result<(&'a mut PxuiUi,UiBox,State),i32> {
    let Some(ui)=(unsafe{get(ui)}) else{return Err(PXUI_ERR_ARGUMENT)};
    if box_.is_null()||state_.is_null(){return Err(unsafe{fail(ui,"null style descriptor")});}
    let Some(state)=decode_state(unsafe{*state_}) else{return Err(unsafe{fail(ui,"invalid style state")});};
    let Some(box_)=core_box(unsafe{*box_}) else{return Err(unsafe{fail(ui,"invalid style box")});};
    Ok((ui,box_,state))
}

export!(pub unsafe fn pxui_ui_style(ui:*mut PxuiUi,box_:*const PxuiBox,states_:*const PxuiLookDesc,state_:*const PxuiStateDesc)->i32 {
    let (ui,box_,state)=match unsafe{decode_args(ui,box_,state_)} {Ok(v)=>v,Err(rc)=>return rc};
    if states_.is_null(){return unsafe{fail(ui,"null states descriptor")};}
    let values=unsafe{core::slice::from_raw_parts(states_,6)};
    // all six are checked; only the one the state picks is converted (`Ui::style` is `set_look` of `States::pick`), so a body's look
    // per row costs one conversion, not six
    if !values.iter().all(valid_look) {return unsafe{fail(ui,"invalid states descriptor")};}
    unsafe{(&mut *ui.engine).set_look(&box_,look(values[picked(state)]),state);}
    0
});

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn style_descriptors_reject_invalid_tags_and_non_finite_values() {
        let mut desc=unsafe{core::mem::zeroed::<PxuiLookDesc>()};
        assert!(decode_look(desc).is_some());
        desc.has_gradient=2;
        assert!(decode_look(desc).is_none());
        let mut desc=unsafe{core::mem::zeroed::<PxuiLookDesc>()};
        desc.angle=f32::NAN;
        assert!(decode_look(desc).is_none());
        let mut state=PxuiStateDesc::default();
        state.hovered=2;
        assert!(decode_state(state).is_none());
    }

    /// `picked` names the look `States::pick` takes, for every combination of the five state bits.
    #[test]
    fn picked_matches_states_pick() {
        let looks:[Look;6]=core::array::from_fn(|i| Look { opacity: i as f32, ..Look::NONE });
        let states=pxui_core::ui::States { idle: looks[0], hover: looks[1], pressed: looks[2], focus: looks[3], disabled: looks[4], checked: looks[5] };
        for bits in 0..32u8 {
            let s=State { hovered: bits&1!=0, pressed: bits&2!=0, focused: bits&4!=0, disabled: bits&8!=0, checked: bits&16!=0 };
            assert_eq!(states.pick(s).opacity, looks[picked(s)].opacity, "state bits {bits:#07b}");
        }
    }
}
