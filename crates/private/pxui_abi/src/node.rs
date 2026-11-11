use crate::state::{PxuiUi, Shared, core_box, fail, get, text, to_abi_box};
use pxui_core::ui::Ui;
use core::mem::transmute;
use std::rc::Rc;
use pxui_types::{a11y::Role, layout::{Align, Anchor, Cell, Grid, Justify, MAX_TRACKS, Placement, Side, Track}, theme::{Curve, Font, Look, Ring, Shadow, State}, Axis, Bar, Dir, Mark, Node, Size, geom::Rect, ffi::{NodeDesc, PxuiFontDesc, PxuiLookDesc, PxuiShadowDesc, PxuiStateDesc, PxuiTrackDesc}, PxuiBox};

fn align(v:u32)->Option<Align>{Some(match v {0=>Align::Start,1=>Align::Center,2=>Align::End,3=>Align::Stretch,4=>Align::Baseline,_=>return None})}
pub(crate) fn size(v:pxui_types::ffi::PxuiSizeDesc)->Option<Size>{if !v.value.is_finite(){return None;} Some(match v.kind {0=>Size::Px(v.value),1=>Size::Pct(v.value),2=>Size::Rel(v.relative?),3=>Size::Grow,4=>Size::Fit,_=>return None})}
fn font(v:PxuiFontDesc)->Font { Font { family:v.family, weight:v.weight, size:v.size, tracking:v.tracking, caps:v.caps!=0 } }
fn shadow(v:PxuiShadowDesc)->Shadow { Shadow { color:v.color, offset:(v.offset[0],v.offset[1]), blur:v.blur, spread:v.spread, inner:v.inner!=0 } }
pub(crate) fn look(v:PxuiLookDesc)->Look { Look {
    fill:v.fill, gradient:(v.has_gradient!=0).then_some(v.gradient), angle:v.angle,
    border:v.border, border_w:v.border_w, sides:v.sides as u8, radius:v.radius,
    shadows:[shadow(v.shadows[0]),shadow(v.shadows[1])], text:v.text, font:font(v.font), opacity:v.opacity,
    ring:Ring { color:v.ring.color, width:v.ring.width, sides:v.ring.sides as u8 },
    selection:v.selection, caret:v.caret, dashed:v.dashed!=0,
} }
pub(crate) fn state(v:PxuiStateDesc)->State { State { hovered:v.hovered!=0, pressed:v.pressed!=0, focused:v.focused!=0, disabled:v.disabled!=0, checked:v.checked!=0 } }
fn track(v:PxuiTrackDesc)->Option<Track>{if !v.value.is_finite(){return None;} Some(match v.kind {0=>Track::Px(v.value),1=>Track::Fr(v.value),2=>Track::Fit,_=>return None})}
unsafe fn tracks<'a>(slice:pxui_types::ffi::Slice<PxuiTrackDesc>)->Option<&'a [PxuiTrackDesc]> {
    if slice.len>64 || (slice.len!=0 && slice.ptr.is_null()) { return None; }
    Some(if slice.len==0 { &[] } else { unsafe { core::slice::from_raw_parts(slice.ptr,slice.len) } })
}
fn decode(desc: &NodeDesc, shared: &mut Shared) -> Option<(Node, Option<Rc<[Track]>>)> {
    if desc.size < core::mem::size_of::<NodeDesc>() as u32 { return None; }
    if desc.wrap>1 || desc.grid.has_grid>1 || desc.anchor.has_anchor>1 || desc.anchor.flip>1 || desc.has_at>1
        || desc.look.has_gradient>1 || desc.look.dashed>1 || desc.mark.rising>1 || desc.bar.has_bar>1 || desc.bar.whole>1
        || [desc.state.hovered,desc.state.pressed,desc.state.focused,desc.state.disabled,desc.state.checked].iter().any(|v|*v>1)
    { return None; }
    let axis=match desc.axis {0=>Axis::Row,1=>Axis::Column,2=>Axis::Stack,_=>return None};
    let justify=match desc.justify {0=>Justify::Start,1=>Justify::Center,2=>Justify::End,3=>Justify::SpaceBetween,4=>Justify::SpaceAround,5=>Justify::SpaceEvenly,_=>return None};
    let align_self=if desc.align_self==u32::MAX {None} else {Some(align(desc.align_self)?)};
    let role=if desc.role<=Role::Separator as u32 { unsafe { transmute::<u8,Role>(desc.role as u8) } } else { return None };
    // The engine's Grid borrows 'static track slices, so a grid node's tracks live in `shared` while its box does. A list equal
    // to one already stored is shared, so a settled frame allocates nothing; a changed list allocates one slice.
    let (grid,grid_storage)=if desc.grid.has_grid==0 { (None,None) } else {
        let cols=unsafe{tracks(desc.grid.cols)}?; let rows=unsafe{tracks(desc.grid.rows)}?;
        let mut buffer=[Track::Fit;2*MAX_TRACKS];
        for (out,t) in buffer.iter_mut().zip(cols.iter().chain(rows)) { *out=track(*t)?; }
        let flat=&buffer[..cols.len()+rows.len()];
        let stored=match shared.grid_tracks.values().find(|stored|stored[..]==*flat) { Some(stored)=>Rc::clone(stored), None=>Rc::from(flat) };
        let all: &'static [Track]=unsafe{transmute::<&[Track],&'static [Track]>(&stored[..])};
        let split=cols.len();
        (Some(Grid{cols:&all[..split],rows:&all[split..]}),Some(stored))
    };
    let anchor=if desc.anchor.has_anchor==0 {None} else {
        let side=match desc.anchor.side {0=>Side::Below,1=>Side::Above,2=>Side::Right,3=>Side::Left,_=>return None};
        Some(Anchor{rect:desc.anchor.rect,placement:Placement{side,align:align(desc.anchor.align)?,gap:desc.anchor.gap,margin:desc.anchor.margin,flip:desc.anchor.flip!=0}})
    };
    let mark=match desc.mark.kind {0=>Mark::None,1=>Mark::Chevron(match desc.mark.direction {0=>Dir::Down,1=>Dir::Up,2=>Dir::Left,3=>Dir::Right,_=>return None}),2=>Mark::Line{width:desc.mark.width,rising:desc.mark.rising!=0},_=>return None};
    let bar=if desc.bar.has_bar==0 {None} else {Some(Bar{color:desc.bar.color,radius:desc.bar.radius,width:desc.bar.width,gap:desc.bar.gap,margin:desc.bar.margin,whole:desc.bar.whole!=0})};
    let transition=match desc.transition.kind {0=>None,1=>Some(Curve::Bezier{duration:desc.transition.duration,points:desc.transition.points}),2=>Some(Curve::Spring{omega:desc.transition.omega}),_=>return None};
    Some((Node{flags:desc.flags,w:size(desc.w)?,h:size(desc.h)?,min_w:desc.min_w,min_h:desc.min_h,max_w:desc.max_w,max_h:desc.max_h,grow:desc.grow,shrink:desc.shrink,aspect:desc.aspect,axis,padding:desc.padding,margin:desc.margin,gap:desc.gap,row_gap:desc.row_gap,justify,align:align(desc.align)?,align_self,wrap:desc.wrap!=0,grid,cell:Cell{col:desc.cell.col,row:desc.cell.row,col_span:desc.cell.col_span,row_span:desc.cell.row_span},z:desc.z,anchor,role,a11y:desc.a11y,at:(desc.has_at!=0).then_some((desc.at[0],desc.at[1])),scroll_step:desc.scroll_step,look:look(desc.look),inset:desc.inset,mark,bar,state:state(desc.state),transition},grid_storage))
}

/// Declares the node, remembering a grid's tracks under its key for as long as its box lives. Only grid nodes
/// touch the track table.
fn declare(shared: &mut Shared, engine: &mut Ui, desc: &NodeDesc, make: &mut dyn FnMut(&mut Ui, Node) -> pxui_core::ui::UiBox) -> Option<PxuiBox> {
    let (node, grid_storage) = decode(desc, shared)?;
    let handle = make(engine, node);
    if let Some(tracks) = grid_storage {
        if !shared.grid_tracks.get(&handle.box_key).is_some_and(|stored| Rc::ptr_eq(stored, &tracks)) {
            if let Some(old) = shared.grid_tracks.insert(handle.box_key, tracks) { shared.grid_retired.push(old); }
        }
    }
    Some(to_abi_box(handle))
}

export!(pub unsafe fn pxui_ui_node(ui:*mut PxuiUi,label:pxui_types::Str,desc:*const NodeDesc,out:*mut PxuiBox)->i32 {
    let Some(state)=(unsafe{get(ui)}) else{return crate::PXUI_ERR_ARGUMENT};
    if desc.is_null()||out.is_null(){return unsafe{fail(ui,"null node descriptor")};}
    let Some(label)=(unsafe{text(label)}) else{return unsafe{fail(ui,"invalid node label")};};
    let Some(handle)=declare(state.shared(),unsafe{&mut *state.engine},unsafe{&*desc},&mut |engine,node|engine.node(label,node)) else{return unsafe{fail(ui,"invalid node descriptor")};};
    unsafe{*out=handle;}
    0
});

export!(pub unsafe fn pxui_ui_node_keyed(ui:*mut PxuiUi,key:i64,desc:*const NodeDesc,out:*mut PxuiBox)->i32 {
    let Some(state)=(unsafe{get(ui)}) else{return crate::PXUI_ERR_ARGUMENT};
    if desc.is_null()||out.is_null(){return unsafe{fail(ui,"null keyed node descriptor")};}
    let Some(handle)=declare(state.shared(),unsafe{&mut *state.engine},unsafe{&*desc},&mut |engine,node|engine.node_keyed(key,node)) else{return unsafe{fail(ui,"invalid keyed node descriptor")};};
    unsafe{*out=handle;}
    0
});

/// Ends a frame: frees replaced grid tracks and those of boxes that are gone, and builds the output once.
pub(crate) fn finish_frame(shared: &mut Shared, engine: &Ui) {
    shared.grid_retired.clear();
    if !shared.grid_tracks.is_empty() { shared.grid_tracks.retain(|key, _| engine.box_of_key(*key).is_some()); }
    crate::ui::build_output(shared, engine, true);
}

export!(pub unsafe fn pxui_ui_rect(ui:*mut PxuiUi,box_:*const PxuiBox,out:*mut Rect)->i32 {
    let Some(state)=(unsafe{get(ui)}) else{return crate::PXUI_ERR_ARGUMENT};
    if box_.is_null()||out.is_null(){return unsafe{fail(ui,"null rect argument")};}
    let Some(b)=core_box(unsafe{*box_}) else{return unsafe{fail(ui,"invalid rect box")};};
    unsafe{*out=(&*state.engine).rect(&b);}
    0
});

export!(pub unsafe fn pxui_ui_box_of_key(ui:*mut PxuiUi,key:i64,out:*mut PxuiBox)->u8 {
    let Some(state)=(unsafe{get(ui)}) else{return 0};
    if out.is_null(){return 0;}
    let handle=unsafe{(&*state.engine).box_of_key(key)};
    let Some(handle)=handle else{return 0};
    unsafe{*out=to_abi_box(handle);}
    1
});

export!(pub unsafe fn pxui_ui_text(ui:*mut PxuiUi,box_:*const PxuiBox,value:pxui_types::Str)->i32 {
    let Some(state)=(unsafe{get(ui)}) else{return crate::PXUI_ERR_ARGUMENT};
    if box_.is_null(){return unsafe{fail(ui,"null text box")};}
    let Some(value)=(unsafe{text(value)}) else{return unsafe{fail(ui,"invalid text")};};
    let Some(b)=core_box(unsafe{*box_}) else{return unsafe{fail(ui,"invalid text box")};};
    unsafe{(&mut *state.engine).text(&b,value);}
    0
});
