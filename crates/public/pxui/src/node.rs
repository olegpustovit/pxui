use crate::Handle;
use pxui_types::{Str, Mark, Node, PxuiBox, Size, theme::{Font, Look, Shadow}, ffi::{PxuiFontDesc, PxuiLookDesc, PxuiMarkDesc, PxuiRingDesc, PxuiShadowDesc, PxuiSizeDesc, PxuiStateDesc}};
use crate::{check, Ui};

library!(sys {
    #[trusted] fn pxui_ui_node(ui:*mut PxuiUi,label:Str,node:&Node,out:*mut PxuiBox)->i32;
});

pub(crate) fn size(s:Size)->PxuiSizeDesc { match s {Size::Px(value)=>PxuiSizeDesc{kind:0,value,relative:None},Size::Pct(value)=>PxuiSizeDesc{kind:1,value,relative:None},Size::Rel(relative)=>PxuiSizeDesc{kind:2,value:0.0,relative:Some(relative)},Size::Grow=>PxuiSizeDesc{kind:3,value:0.0,relative:None},Size::Fit=>PxuiSizeDesc{kind:4,value:0.0,relative:None}} }
fn font(v:Font)->PxuiFontDesc { PxuiFontDesc{family:v.family,weight:v.weight,size:v.size,tracking:v.tracking,caps:v.caps as u8,reserved:[0;3]} }
fn shadow(v:Shadow)->PxuiShadowDesc { PxuiShadowDesc{color:v.color,offset:v.offset,blur:v.blur,spread:v.spread,inner:v.inner as u8,reserved:[0;3]} }
pub(crate) fn look(v:Look)->PxuiLookDesc { PxuiLookDesc{fill:v.fill,gradient:v.gradient.get().unwrap_or_default(),has_gradient:v.gradient.is_some() as u8,reserved:[0;3],angle:v.angle,border:v.border,border_w:v.border_w,sides:v.sides as u32,radius:v.radius,shadows:[shadow(v.shadows[0]),shadow(v.shadows[1])],text:v.text,font:font(v.font),opacity:v.opacity,ring:PxuiRingDesc{color:v.ring.color,width:v.ring.width,sides:v.ring.sides as u32},selection:v.selection,caret:v.caret,dashed:v.dashed as u8,reserved_tail:[0;3]} }
pub(crate) fn state(v:pxui_types::theme::State)->PxuiStateDesc { PxuiStateDesc{hovered:v.hovered as u8,pressed:v.pressed as u8,focused:v.focused as u8,disabled:v.disabled as u8,checked:v.checked as u8,reserved:[0;3]} }
pub(crate) fn mark(v:Mark)->PxuiMarkDesc { match v {Mark::None=>PxuiMarkDesc{kind:0,..Default::default()},Mark::Chevron(dir)=>PxuiMarkDesc{kind:1,direction:dir as u32,..Default::default()},Mark::Line{width,rising}=>PxuiMarkDesc{kind:2,width,rising:rising as u8,..Default::default()}} }

impl Ui {
    #[inline]
    #[track_caller]
    pub(crate) fn node_key(&mut self,key:crate::Key,node:Node)->Handle {
        // written by the library whenever it returns 0; `check` does not return otherwise
        let mut out=core::mem::MaybeUninit::<Handle>::uninit();
        let rc=match key {
            crate::Key::Label(label)=>{ crate::authored(label); self.node_call(label,&node,out.as_mut_ptr()) },
            key=>{ let mut id_label=[0u8;17]; self.node_call(key.label(&mut id_label),&node,out.as_mut_ptr()) }
        };
        check(self,rc);
        unsafe { out.assume_init() }
    }

    /// The library call of `node`. Static: a Rust label is UTF-8 by its type, so the trusted entry checks only its length.
    #[inline(always)]
    fn node_call(&mut self,label:&str,node:&Node,out:*mut Handle)->i32 {
        #[cfg(feature="static")]
        return unsafe{pxui_abi_link::trusted::node_str(self.0.as_ptr(),label,node,out)};
        #[cfg(not(feature="static"))]
        unsafe{sys::pxui_ui_node(self.0.as_ptr(),crate::as_str(label),node,out)}
    }
}
