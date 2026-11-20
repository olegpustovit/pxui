use pxui_core::ui::{Look, State, UiBox};
use pxui_types::ffi::{PxuiLookDesc, PxuiStateDesc};
use crate::state::{PxuiUi, core_box_of, fail, get};
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

fn decode_state(value:PxuiStateDesc)->Option<State> {
    if value.reserved!=[0;3] || [value.hovered,value.pressed,value.focused,value.disabled,value.checked].iter().any(|v|*v>1) { return None; }
    Some(state(value))
}

unsafe fn decode_args<'a>(ui:*mut PxuiUi,box_:*const PxuiBox,state_:*const PxuiStateDesc)->Result<(&'a mut PxuiUi,UiBox,State),i32> {
    let Some(ui)=(unsafe{get(ui)}) else{return Err(PXUI_ERR_ARGUMENT)};
    if box_.is_null()||state_.is_null(){return Err(unsafe{fail(ui,"null style descriptor")});}
    let Some(state)=decode_state(unsafe{*state_}) else{return Err(unsafe{fail(ui,"invalid style state")});};
    let Some(box_)=(unsafe{core_box_of(ui,*box_)}) else{return Err(unsafe{crate::state::fail_code(ui,"stale style box",crate::PXUI_ERR_STALE)});};
    Ok((ui,box_,state))
}

// ABI 1.108 (D28, changed in place): the one look the state picks (`States::pick`), checked and applied; the caller picks it.
export!(pub unsafe fn pxui_ui_style(ui:*mut PxuiUi,box_:*const PxuiBox,look_:*const PxuiLookDesc,state_:*const PxuiStateDesc)->i32 {
    let (ui,box_,state)=match unsafe{decode_args(ui,box_,state_)} {Ok(v)=>v,Err(rc)=>return rc};
    if look_.is_null(){return unsafe{fail(ui,"null look descriptor")};}
    let Some(look)=decode_look(unsafe{*look_}) else{return unsafe{fail(ui,"invalid look descriptor")};};
    unsafe{(&mut *ui.engine).set_look(&box_,look,state);}
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
