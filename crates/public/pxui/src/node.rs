use pxui_types::{Bar, Mark, Node, PxuiBox, Size, layout::{Align, Grid, MAX_TRACKS, Track}, theme::{Curve, Font, Look, Shadow}, ffi::{NodeDesc, PxuiAnchorDesc, PxuiBarDesc, PxuiCellDesc, PxuiCurveDesc, PxuiFontDesc, PxuiGridDesc, PxuiLookDesc, PxuiMarkDesc, PxuiRingDesc, PxuiShadowDesc, PxuiSizeDesc, PxuiStateDesc, PxuiTrackDesc, Slice}};
use core::mem::size_of;
use crate::Ui;

#[cfg(feature = "static")]
mod sys { pub use pxui_abi_link::pxui_ui_node_keyed; }
#[cfg(not(feature = "static"))]
mod sys {
    use pxui_types::{ffi::NodeDesc, PxuiBox};
    use crate::sys::PxuiUi;
    #[link(name="pxui")]
    unsafe extern "C" { pub fn pxui_ui_node_keyed(ui:*mut PxuiUi,key:i64,desc:*const NodeDesc,out:*mut PxuiBox)->i32; }
}

pub(crate) fn size(s:Size)->PxuiSizeDesc { match s {Size::Px(value)=>PxuiSizeDesc{kind:0,value,relative:None},Size::Pct(value)=>PxuiSizeDesc{kind:1,value,relative:None},Size::Rel(relative)=>PxuiSizeDesc{kind:2,value:0.0,relative:Some(relative)},Size::Grow=>PxuiSizeDesc{kind:3,value:0.0,relative:None},Size::Fit=>PxuiSizeDesc{kind:4,value:0.0,relative:None}} }
#[inline]
fn align(v:Align)->u32 { v as u32 }
fn track(v:Track)->PxuiTrackDesc { match v {Track::Px(value)=>PxuiTrackDesc{kind:0,value},Track::Fr(value)=>PxuiTrackDesc{kind:1,value},Track::Fit=>PxuiTrackDesc{kind:2,value:0.0}} }
fn font(v:Font)->PxuiFontDesc { PxuiFontDesc{family:v.family,weight:v.weight,size:v.size,tracking:v.tracking,caps:v.caps as u8,reserved:[0;3]} }
fn shadow(v:Shadow)->PxuiShadowDesc { PxuiShadowDesc{color:v.color,offset:[v.offset.0,v.offset.1],blur:v.blur,spread:v.spread,inner:v.inner as u8,reserved:[0;3]} }
pub(crate) fn look(v:Look)->PxuiLookDesc { PxuiLookDesc{fill:v.fill,gradient:v.gradient.unwrap_or_default(),has_gradient:v.gradient.is_some() as u8,reserved:[0;3],angle:v.angle,border:v.border,border_w:v.border_w,sides:v.sides as u32,radius:v.radius,shadows:[shadow(v.shadows[0]),shadow(v.shadows[1])],text:v.text,font:font(v.font),opacity:v.opacity,ring:PxuiRingDesc{color:v.ring.color,width:v.ring.width,sides:v.ring.sides as u32},selection:v.selection,caret:v.caret,dashed:v.dashed as u8,reserved_tail:[0;3]} }
pub(crate) fn state(v:pxui_types::theme::State)->PxuiStateDesc { PxuiStateDesc{hovered:v.hovered as u8,pressed:v.pressed as u8,focused:v.focused as u8,disabled:v.disabled as u8,checked:v.checked as u8,reserved:[0;3]} }
pub(crate) fn mark(v:Mark)->PxuiMarkDesc { match v {Mark::None=>PxuiMarkDesc{kind:0,..Default::default()},Mark::Chevron(dir)=>PxuiMarkDesc{kind:1,direction:dir as u32,..Default::default()},Mark::Line{width,rising}=>PxuiMarkDesc{kind:2,width,rising:rising as u8,..Default::default()}} }
fn bar(v:Option<Bar>)->PxuiBarDesc { match v {None=>PxuiBarDesc::default(),Some(v)=>PxuiBarDesc{has_bar:1,whole:v.whole as u8,reserved:[0;2],color:v.color,radius:v.radius,width:v.width,gap:v.gap,margin:v.margin}} }

/// Writes `g`'s tracks into `store` (columns first) and describes them. None when a list is longer than the ABI takes.
fn encode_grid(g:&Grid,store:&mut [PxuiTrackDesc;2*MAX_TRACKS])->Option<PxuiGridDesc> {
    if g.cols.len()>MAX_TRACKS||g.rows.len()>MAX_TRACKS { return None; }
    let (cols,rows)=store.split_at_mut(MAX_TRACKS);
    for (out,t) in cols.iter_mut().zip(g.cols) { *out=track(*t); }
    for (out,t) in rows.iter_mut().zip(g.rows) { *out=track(*t); }
    Some(PxuiGridDesc{has_grid:1,reserved:[0;7],cols:Slice{ptr:cols.as_ptr(),len:g.cols.len()},rows:Slice{ptr:rows.as_ptr(),len:g.rows.len()}})
}

#[inline]
fn encode(v:&Node,grid:PxuiGridDesc)->NodeDesc {
    let anchor=if let Some(a)=v.anchor { PxuiAnchorDesc{has_anchor:1,reserved:[0;3],rect:a.rect,side:a.placement.side as u32,align:align(a.placement.align),gap:a.placement.gap,margin:a.placement.margin,flip:a.placement.flip as u8,reserved_tail:[0;3]} } else { PxuiAnchorDesc::default() };
    let transition=match v.transition {None=>PxuiCurveDesc::default(),Some(Curve::Bezier{duration,points})=>PxuiCurveDesc{kind:1,duration,points,omega:0.0},Some(Curve::Spring{omega})=>PxuiCurveDesc{kind:2,duration:0.0,points:[0.0;4],omega}};
    let (has_at,at)=v.at.map_or((0,[0.0;2]),|(x,y)|(1,[x,y]));
    NodeDesc{size:size_of::<NodeDesc>() as u32,flags:v.flags,w:size(v.w),h:size(v.h),min_w:v.min_w,min_h:v.min_h,max_w:v.max_w,max_h:v.max_h,grow:v.grow,shrink:v.shrink,aspect:v.aspect,axis:v.axis as u32,padding:v.padding,margin:v.margin,gap:v.gap,row_gap:v.row_gap,justify:v.justify as u32,align:align(v.align),align_self:v.align_self.map_or(u32::MAX,align),wrap:v.wrap as u8,reserved:[0;3],grid,cell:PxuiCellDesc{col:v.cell.col,row:v.cell.row,col_span:v.cell.col_span,row_span:v.cell.row_span},z:v.z,reserved_z:0,role:v.role as u32,a11y:v.a11y,anchor,has_at,at,scroll_step:v.scroll_step,look:look(v.look),inset:v.inset,mark:mark(v.mark),bar:bar(v.bar),state:state(v.state),transition}
}

impl Ui {
    /// Declares a node under the current parent, keyed by `label`. The descriptor is built on the stack and
    /// crosses by reference; only a node with a grid touches the library's track table.
    #[inline]
    pub fn node(&mut self,label:&str,node:&Node)->Result<PxuiBox,i32> {
        let mut store:[PxuiTrackDesc;2*MAX_TRACKS];
        let grid=match &node.grid {
            Some(g)=>{ store=[PxuiTrackDesc::default();2*MAX_TRACKS]; encode_grid(g,&mut store).ok_or(-1)? }
            None=>PxuiGridDesc{has_grid:0,reserved:[0;7],cols:Slice{ptr:core::ptr::null(),len:0},rows:Slice{ptr:core::ptr::null(),len:0}},
        };
        self.node_desc(label,&encode(node,grid))
    }

    /// Declares a node using an explicit key in the current parent scope.
    #[inline]
    pub fn node_keyed(&mut self,key:i64,node:&Node)->Result<PxuiBox,i32> {
        let mut store:[PxuiTrackDesc;2*MAX_TRACKS];
        let grid=match &node.grid {
            Some(g)=>{ store=[PxuiTrackDesc::default();2*MAX_TRACKS]; encode_grid(g,&mut store).ok_or(-1)? }
            None=>PxuiGridDesc{has_grid:0,reserved:[0;7],cols:Slice{ptr:core::ptr::null(),len:0},rows:Slice{ptr:core::ptr::null(),len:0}},
        };
        let mut out=PxuiBox::default();
        let rc=unsafe{sys::pxui_ui_node_keyed(self.0.as_ptr(),key,&encode(node,grid),&mut out)};
        if rc==0 {Ok(out)} else {Err(rc)}
    }
}
