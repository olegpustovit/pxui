use pxui_types::{PxuiBox, ffi::PxuiStateDesc, theme::{Look, State, States}};
use crate::{node, Ui};

#[cfg(feature="static")]
mod sys { pub use pxui_abi_link::{pxui_ui_set_look,pxui_ui_style}; }
#[cfg(not(feature="static"))]
mod sys {
    use pxui_types::{ffi::{PxuiLookDesc,PxuiStateDesc},PxuiBox};
    use crate::sys::PxuiUi;
    #[link(name="pxui")]
    unsafe extern "C" {
        pub fn pxui_ui_set_look(ui:*mut PxuiUi,box_:*const PxuiBox,look:*const PxuiLookDesc,state:*const PxuiStateDesc)->i32;
        pub fn pxui_ui_style(ui:*mut PxuiUi,box_:*const PxuiBox,states:*const PxuiLookDesc,state:*const PxuiStateDesc)->i32;
    }
}

fn encode_state(value:State)->PxuiStateDesc { node::state(value) }
fn status(rc:i32)->Result<(),i32> { if rc==0 {Ok(())} else {Err(rc)} }

impl Ui {
    /// Applies one look to a live node and records its interaction state for transitions.
    pub fn set_look(&mut self,box_:&PxuiBox,look:Look,state:State)->Result<(),i32> {
        let look=node::look(look); let state=encode_state(state);
        status(unsafe{sys::pxui_ui_set_look(self.0.as_ptr(),box_,&look,&state)})
    }

    /// Picks and applies the matching interaction look to a live node.
    pub fn style(&mut self,box_:&PxuiBox,states:&States,state:State)->Result<(),i32> {
        let values=[node::look(states.idle),node::look(states.hover),node::look(states.pressed),node::look(states.focus),node::look(states.disabled),node::look(states.checked)];
        let state=encode_state(state);
        status(unsafe{sys::pxui_ui_style(self.0.as_ptr(),box_,values.as_ptr(),&state)})
    }
}
