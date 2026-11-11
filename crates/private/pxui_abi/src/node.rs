use crate::{get, text, PxuiUi};
use core::mem::transmute;
use pxui_core::ui::UiBox;
use pxui_types::{a11y::Role, layout::{Align, Anchor, Cell, Grid, Justify, Placement, Side, Track}, theme::{Curve, Font, Look, Ring, Shadow, State}, Axis, Bar, Dir, Mark, Node, Size, geom::{Rect, Rgba}, ffi::{NodeDesc, PxuiFontDesc, PxuiLookDesc, PxuiShadowDesc, PxuiStateDesc, PxuiTrackDesc}, PxuiBox};

fn color(c: Rgba) -> Rgba { c }
fn align(v:u32)->Option<Align>{Some(match v {0=>Align::Start,1=>Align::Center,2=>Align::End,3=>Align::Stretch,4=>Align::Baseline,_=>return None})}
fn size(v:pxui_types::ffi::PxuiSizeDesc)->Option<Size>{Some(match v.kind {0=>Size::Px(v.value),1=>Size::Pct(v.value),2=>Size::Rel(v.relative?),3=>Size::Grow,4=>Size::Fit,_=>return None})}
fn font(v:PxuiFontDesc)->Font { Font { family:v.family, weight:v.weight, size:v.size, tracking:v.tracking, caps:v.caps!=0 } }
fn shadow(v:PxuiShadowDesc)->Shadow { Shadow { color:color(v.color), offset:(v.offset[0],v.offset[1]), blur:v.blur, spread:v.spread, inner:v.inner!=0 } }
pub(crate) fn look(v:PxuiLookDesc)->Look { Look {
    fill:color(v.fill), gradient:(v.has_gradient!=0).then_some(color(v.gradient)), angle:v.angle,
    border:color(v.border), border_w:v.border_w, sides:v.sides as u8, radius:v.radius,
    shadows:[shadow(v.shadows[0]),shadow(v.shadows[1])], text:color(v.text), font:font(v.font), opacity:v.opacity,
    ring:Ring { color:color(v.ring.color), width:v.ring.width, sides:v.ring.sides as u8 },
    selection:color(v.selection), caret:color(v.caret), dashed:v.dashed!=0,
} }
pub(crate) fn state(v:PxuiStateDesc)->State { State { hovered:v.hovered!=0, pressed:v.pressed!=0, focused:v.focused!=0, disabled:v.disabled!=0, checked:v.checked!=0 } }
fn track(v:PxuiTrackDesc)->Option<Track>{Some(match v.kind {0=>Track::Px(v.value),1=>Track::Fr(v.value),2=>Track::Fit,_=>return None})}
unsafe fn tracks<'a>(slice:pxui_types::ffi::Slice<PxuiTrackDesc>)->Option<&'a [PxuiTrackDesc]> {
    if slice.len>64 || (slice.len!=0 && slice.ptr.is_null()) { return None; }
    Some(if slice.len==0 { &[] } else { unsafe { core::slice::from_raw_parts(slice.ptr,slice.len) } })
}
unsafe fn node(desc:NodeDesc)->Option<(Node,Option<(Box<[Track]>,usize)>)>{
    if desc.size < core::mem::size_of::<NodeDesc>() as u32 { return None; }
    if desc.wrap>1 || desc.grid.has_grid>1 || desc.anchor.has_anchor>1 || desc.anchor.flip>1 || desc.has_at>1
        || desc.look.has_gradient>1 || desc.look.dashed>1 || desc.mark.rising>1 || desc.bar.has_bar>1 || desc.bar.whole>1
        || [desc.state.hovered,desc.state.pressed,desc.state.focused,desc.state.disabled,desc.state.checked].iter().any(|v|*v>1)
    { return None; }
    let axis=match desc.axis {0=>Axis::Row,1=>Axis::Column,2=>Axis::Stack,_=>return None};
    let justify=match desc.justify {0=>Justify::Start,1=>Justify::Center,2=>Justify::End,3=>Justify::SpaceBetween,4=>Justify::SpaceAround,5=>Justify::SpaceEvenly,_=>return None};
    let align_self=if desc.align_self==u32::MAX {None} else {Some(align(desc.align_self)?)};
    let role=if desc.role<=Role::Separator as u32 { unsafe { transmute::<u8,Role>(desc.role as u8) } } else { return None };
    let (grid,grid_storage)=if desc.grid.has_grid==0 { (None,None) } else {
        let cols=unsafe{tracks(desc.grid.cols)}?; let rows=unsafe{tracks(desc.grid.rows)}?;
        let mut storage=Vec::with_capacity(cols.len()+rows.len());
        for t in cols.iter().chain(rows){storage.push(track(*t)?);}
        let split=cols.len();
        let storage=storage.into_boxed_slice();
        let all: &'static [Track]=unsafe{transmute::<&[Track],&'static [Track]>(storage.as_ref())};
        (Some(Grid{cols:&all[..split],rows:&all[split..]}),Some((storage,split)))
    };
    let anchor=if desc.anchor.has_anchor==0 {None} else {
        let side=match desc.anchor.side {0=>Side::Below,1=>Side::Above,2=>Side::Right,3=>Side::Left,_=>return None};
        Some(Anchor{rect:desc.anchor.rect,placement:Placement{side,align:align(desc.anchor.align)?,gap:desc.anchor.gap,margin:desc.anchor.margin,flip:desc.anchor.flip!=0}})
    };
    let mark=match desc.mark.kind {0=>Mark::None,1=>Mark::Chevron(match desc.mark.direction {0=>Dir::Down,1=>Dir::Up,2=>Dir::Left,3=>Dir::Right,_=>return None}),2=>Mark::Line{width:desc.mark.width,rising:desc.mark.rising!=0},_=>return None};
    let bar=if desc.bar.has_bar==0 {None} else {Some(Bar{color:color(desc.bar.color),radius:desc.bar.radius,width:desc.bar.width,gap:desc.bar.gap,margin:desc.bar.margin,whole:desc.bar.whole!=0})};
    let transition=match desc.transition.kind {0=>None,1=>Some(Curve::Bezier{duration:desc.transition.duration,points:desc.transition.points}),2=>Some(Curve::Spring{omega:desc.transition.omega}),_=>return None};
    Some((Node{flags:desc.flags,w:size(desc.w)?,h:size(desc.h)?,min_w:desc.min_w,min_h:desc.min_h,max_w:desc.max_w,max_h:desc.max_h,grow:desc.grow,shrink:desc.shrink,aspect:desc.aspect,axis,padding:desc.padding,margin:desc.margin,gap:desc.gap,row_gap:desc.row_gap,justify,align:align(desc.align)?,align_self,wrap:desc.wrap!=0,grid,cell:Cell{col:desc.cell.col,row:desc.cell.row,col_span:desc.cell.col_span,row_span:desc.cell.row_span},z:desc.z,anchor,role,a11y:desc.a11y,at:(desc.has_at!=0).then_some((desc.at[0],desc.at[1])),scroll_step:desc.scroll_step,look:look(desc.look),inset:desc.inset,mark,bar,state:state(desc.state),transition},grid_storage))
}
pub(crate) fn to_abi_box(b:UiBox)->PxuiBox { PxuiBox{index:b.index as u64,key:b.box_key,slot:b.box_slot as u64,generation:b.generation} }
pub(crate) fn from_abi_box(b:PxuiBox)->UiBox { UiBox{index:b.index as usize,box_key:b.key,box_slot:b.slot as usize,generation:b.generation} }

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_node(ui:*mut PxuiUi,label:pxui_types::Str,desc:*const NodeDesc,out:*mut PxuiBox)->i32 {
    let Some(ui)=(unsafe{get(ui)}) else{return -1};
    if desc.is_null()||out.is_null(){return unsafe{crate::fail(ui as *mut PxuiUi,"null node descriptor")};}
    let Some(label)=(unsafe{text(label)}) else{return unsafe{crate::fail(ui as *mut PxuiUi,"invalid node label")};};
    let desc=unsafe{*desc};
    let node=unsafe{node(desc)};
    let Some((node,grid_storage))=node else{return unsafe{crate::fail(ui as *mut PxuiUi,"invalid node descriptor")};};
    let handle=unsafe{(&mut *ui.engine).node(label,node)};
    let key=handle.box_key;
    let storage=unsafe { &mut *ui.grid_tracks };
    let has_grid=grid_storage.is_some();
    let versions=storage.entry(key).or_default();
    if let Some((tracks,_))=grid_storage { versions.push(tracks); }
    unsafe { (&mut *ui.grid_updated).insert(key,has_grid); }
    unsafe{*out=to_abi_box(handle);}
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_node_keyed(ui:*mut PxuiUi,key:i64,desc:*const NodeDesc,out:*mut PxuiBox)->i32 {
    let Some(ui)=(unsafe{get(ui)}) else{return -1};
    if desc.is_null()||out.is_null(){return unsafe{crate::fail(ui as *mut PxuiUi,"null keyed node descriptor")};}
    let desc=unsafe{*desc};
    let Some((node,grid_storage))=(unsafe{node(desc)}) else{return unsafe{crate::fail(ui as *mut PxuiUi,"invalid keyed node descriptor")};};
    let handle=unsafe{(&mut *ui.engine).node_keyed(key,node)};
    let storage=unsafe{&mut *ui.grid_tracks};
    let has_grid=grid_storage.is_some();
    let versions=storage.entry(handle.box_key).or_default();
    if let Some((tracks,_))=grid_storage { versions.push(tracks); }
    unsafe{(&mut *ui.grid_updated).insert(handle.box_key,has_grid);}
    unsafe{*out=to_abi_box(handle);}
    0
}

pub(crate) unsafe fn finish_frame(ui:*mut PxuiUi) {
    if let Some(ui)=unsafe{get(ui)} {
        let updated=unsafe{&mut *ui.grid_updated}; let tracks=unsafe{&mut *ui.grid_tracks};
        for (key,has_grid) in updated.drain() {
            if has_grid { if let Some(versions)=tracks.get_mut(&key) { if versions.len()>1 { let last=versions.pop().unwrap(); versions.clear(); versions.push(last); } } }
            else { tracks.remove(&key); }
        }
        let engine=unsafe{&*ui.engine};
        tracks.retain(|key,_|engine.box_of_key(*key).is_some());
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_rect(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut Rect)->i32 {
    let Some(ui)=(unsafe{get(ui)}) else{return -1};
    if box_.is_null()||out.is_null(){return unsafe{crate::fail(ui as *mut PxuiUi,"null rect argument")};}
    let b=from_abi_box(unsafe{*box_});
    unsafe{*out=(&*ui.engine).rect(&b);}
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_box_of_key(ui:*mut PxuiUi,key:i64,out:*mut PxuiBox)->u8 {
    let Some(ui)=(unsafe{get(ui)}) else{return 0};
    if out.is_null(){return 0;}
    let Some(handle)=(unsafe{(&*ui.engine).box_of_key(key)}) else{return 0};
    unsafe{*out=to_abi_box(handle);}
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_key_of(seed:i64,label:pxui_types::Str)->i64 {
    let Some(label)=(unsafe{text(label)}) else{return 0};
    pxui_core::ui_core::key_of(seed,label)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pxui_ui_text(ui:*mut PxuiUi,box_:*const PxuiBox,value:pxui_types::Str)->i32 {
    let Some(ui)=(unsafe{get(ui)}) else{return -1};
    if box_.is_null(){return unsafe{crate::fail(ui as *mut PxuiUi,"null text box")};}
    let Some(value)=(unsafe{text(value)}) else{return unsafe{crate::fail(ui as *mut PxuiUi,"invalid text")};};
    let b=from_abi_box(unsafe{*box_});
    unsafe{(&mut *ui.engine).text(&b,value);}
    0
}
