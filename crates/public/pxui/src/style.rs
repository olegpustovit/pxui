use crate::Handle;
use pxui_types::{ffi::PxuiStateDesc, theme::{State, States}};
use crate::{node, Ui, check};

#[cfg(feature="static")]
mod sys { pub use pxui_abi_link::pxui_ui_style; }
#[cfg(not(feature="static"))]
mod sys {
    use pxui_types::{ffi::{PxuiLookDesc,PxuiStateDesc},PxuiBox};
    use crate::sys::PxuiUi;
    #[link(name="pxui")]
    unsafe extern "C" {
        pub fn pxui_ui_style(ui:*mut PxuiUi,box_:*const PxuiBox,look:*const PxuiLookDesc,state:*const PxuiStateDesc)->i32;
    }
}

#[inline]
fn encode_state(value:State)->PxuiStateDesc { node::state(value) }

impl Ui {
    /// Applies a state-dependent look to a node during the build.
    ///
    /// The engine resolves named colours under the active theme and keeps the node's state for transitions.
    /// Use States::of for one look in every state, and patch_look for a retained update between builds. Stale handles are ignored.
    /// Only the look `state` picks (`States::pick`) crosses to the library, which checks and applies it; the other five are not read,
    /// so an invalid value in a look for a state the node is not in has no effect.
    /// Allocates: never.
    /// Panics: the look `state` picks contains an invalid value.
    ///
    /// ```
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("badge", pxui::Node::DEFAULT);
    ///     ui.style(&node, &pxui::ui::States::of(pxui::ui::Look::NONE), pxui::ui::State::default());
    /// });
    /// ```
    #[inline]
    #[track_caller]
    pub fn style(&mut self, box_: &Handle, states: &States, state: State) {
        let look = node::look(*states.pick(state));
        let state = encode_state(state);
        check(self, unsafe { sys::pxui_ui_style(self.0.as_ptr(), box_, &look, &state) })
    }
}
