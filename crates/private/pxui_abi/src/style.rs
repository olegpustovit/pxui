use pxui_core::ui::{Look, State, States, UiBox};
use pxui_types::ffi::{PxuiLookDesc, PxuiStateDesc};
use crate::state::{PxuiUi, core_box, fail, get};
use crate::node::{look, state};
use crate::{PXUI_ERR_ARGUMENT, PxuiBox};

pub(crate) fn decode_look(value:PxuiLookDesc)->Option<Look> {
    if value.has_gradient>1 || value.reserved!=[0;3] || value.reserved_tail!=[0;3]
        || value.dashed>1 || value.font.caps>1 || value.font.reserved!=[0;3]
        || value.ring.sides>15 || value.sides>15
        || value.shadows.iter().any(|s|s.inner>1 || s.reserved!=[0;3])
    { return None; }
    let [a,b]=value.shadows;
    let numbers=[value.angle,value.border_w,value.opacity,value.ring.width,value.font.tracking,
        value.radius[0],value.radius[1],value.radius[2],value.radius[3],
        a.offset[0],a.offset[1],a.blur,a.spread,b.offset[0],b.offset[1],b.blur,b.spread];
    if numbers.iter().any(|number|!number.is_finite()) { return None; }
    Some(look(value))
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

export!(pub unsafe fn pxui_ui_set_look(ui:*mut PxuiUi,box_:*const PxuiBox,look_:*const PxuiLookDesc,state_:*const PxuiStateDesc)->i32 {
    let (ui,box_,state)=match unsafe{decode_args(ui,box_,state_)} {Ok(v)=>v,Err(rc)=>return rc};
    if look_.is_null(){return unsafe{fail(ui,"null look descriptor")};}
    let Some(look)=decode_look(unsafe{*look_}) else{return unsafe{fail(ui,"invalid look descriptor")};};
    unsafe{(&mut *ui.engine).set_look(&box_,look,state);}
    0
});

export!(pub unsafe fn pxui_ui_style(ui:*mut PxuiUi,box_:*const PxuiBox,states_:*const PxuiLookDesc,state_:*const PxuiStateDesc)->i32 {
    let (ui,box_,state)=match unsafe{decode_args(ui,box_,state_)} {Ok(v)=>v,Err(rc)=>return rc};
    if states_.is_null(){return unsafe{fail(ui,"null states descriptor")};}
    let values=unsafe{core::slice::from_raw_parts(states_,6)};
    let mut decoded=values.iter().copied().map(decode_look);
    let Some(idle)=decoded.next().flatten() else{return unsafe{fail(ui,"invalid states descriptor")};};
    let Some(hover)=decoded.next().flatten() else{return unsafe{fail(ui,"invalid states descriptor")};};
    let Some(pressed)=decoded.next().flatten() else{return unsafe{fail(ui,"invalid states descriptor")};};
    let Some(focus)=decoded.next().flatten() else{return unsafe{fail(ui,"invalid states descriptor")};};
    let Some(disabled)=decoded.next().flatten() else{return unsafe{fail(ui,"invalid states descriptor")};};
    let Some(checked)=decoded.next().flatten() else{return unsafe{fail(ui,"invalid states descriptor")};};
    unsafe{(&mut *ui.engine).style(&box_,&States{idle,hover,pressed,focus,disabled,checked},state);}
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
}
