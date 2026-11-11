use pxui_types::{Str, Mark, Node, PxuiBox, Size, theme::{Font, Look, Shadow}, ffi::{PxuiFontDesc, PxuiLookDesc, PxuiMarkDesc, PxuiRingDesc, PxuiShadowDesc, PxuiSizeDesc, PxuiStateDesc}};
use crate::{check, Ui};

library!(sys {
    fn pxui_ui_node(ui:*mut PxuiUi,label:Str,node:*const Node,out:*mut PxuiBox)->i32;
    fn pxui_ui_node_keyed(ui:*mut PxuiUi,key:i64,node:*const Node,out:*mut PxuiBox)->i32;
});

pub(crate) fn size(s:Size)->PxuiSizeDesc { match s {Size::Px(value)=>PxuiSizeDesc{kind:0,value,relative:None},Size::Pct(value)=>PxuiSizeDesc{kind:1,value,relative:None},Size::Rel(relative)=>PxuiSizeDesc{kind:2,value:0.0,relative:Some(relative)},Size::Grow=>PxuiSizeDesc{kind:3,value:0.0,relative:None},Size::Fit=>PxuiSizeDesc{kind:4,value:0.0,relative:None}} }
fn font(v:Font)->PxuiFontDesc { PxuiFontDesc{family:v.family,weight:v.weight,size:v.size,tracking:v.tracking,caps:v.caps as u8,reserved:[0;3]} }
fn shadow(v:Shadow)->PxuiShadowDesc { PxuiShadowDesc{color:v.color,offset:v.offset,blur:v.blur,spread:v.spread,inner:v.inner as u8,reserved:[0;3]} }
pub(crate) fn look(v:Look)->PxuiLookDesc { PxuiLookDesc{fill:v.fill,gradient:v.gradient.get().unwrap_or_default(),has_gradient:v.gradient.is_some() as u8,reserved:[0;3],angle:v.angle,border:v.border,border_w:v.border_w,sides:v.sides as u32,radius:v.radius,shadows:[shadow(v.shadows[0]),shadow(v.shadows[1])],text:v.text,font:font(v.font),opacity:v.opacity,ring:PxuiRingDesc{color:v.ring.color,width:v.ring.width,sides:v.ring.sides as u32},selection:v.selection,caret:v.caret,dashed:v.dashed as u8,reserved_tail:[0;3]} }
pub(crate) fn state(v:pxui_types::theme::State)->PxuiStateDesc { PxuiStateDesc{hovered:v.hovered as u8,pressed:v.pressed as u8,focused:v.focused as u8,disabled:v.disabled as u8,checked:v.checked as u8,reserved:[0;3]} }
pub(crate) fn mark(v:Mark)->PxuiMarkDesc { match v {Mark::None=>PxuiMarkDesc{kind:0,..Default::default()},Mark::Chevron(dir)=>PxuiMarkDesc{kind:1,direction:dir as u32,..Default::default()},Mark::Line{width,rising}=>PxuiMarkDesc{kind:2,width,rising:rising as u8,..Default::default()}} }

impl Ui {
    /// Declares a node under the current parent, keyed by `label`. The node crosses as it is, by reference: the library checks
    /// it where it lies and copies it once into the engine (a grid's tracks are copied too).
    #[inline]
    #[track_caller]
    pub fn node(&mut self,label:&str,node:Node)->PxuiBox {
        let mut out=PxuiBox::default();
        check(self,unsafe{sys::pxui_ui_node(self.0.as_ptr(),crate::as_str(label),&node,&mut out)});
        out
    }

    /// Declares a node using an explicit key in the current parent scope.
    #[inline]
    #[track_caller]
    pub fn node_keyed(&mut self,key:i64,node:Node)->PxuiBox {
        let mut out=PxuiBox::default();
        check(self,unsafe{sys::pxui_ui_node_keyed(self.0.as_ptr(),key,&node,&mut out)});
        out
    }
}
