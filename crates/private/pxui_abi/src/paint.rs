#![allow(clippy::all)]

use core::ffi::c_void;
use pxui_core::ui::Paint as EnginePaint;
use crate::state::{Shared, finite};
use crate::text::{metrics, style};
use pxui_types::{geom::{Rect, Rgba}, PxuiBox, Str, TextMetrics, TextStyle, ffi::ThemeTokens};

#[repr(C)]
pub struct PxuiPaint { paint: *mut EnginePaint, pub(crate) shared: *mut Shared }

pub type PaintBuild = Option<extern "C" fn(*mut c_void, *mut PxuiPaint, Rect)>;
pub type PaintRelease = Option<extern "C" fn(*mut c_void)>;
pub type SpanBuild = Option<extern "C" fn(*mut c_void, f32, f32)>;

pub(crate) struct CallbackOwner { pub(crate) user: *mut c_void, pub(crate) release: PaintRelease }
impl Drop for CallbackOwner { fn drop(&mut self) { if let Some(release) = self.release { release(self.user); } } }

pub(crate) unsafe fn value<'a>(paint: *mut PxuiPaint) -> Option<&'a mut EnginePaint> {
    if paint.is_null() { None } else { unsafe { (*paint).paint.as_mut() } }
}

unsafe fn string(s: Str) -> Option<&'static str> { unsafe { crate::text(s) } }

export!(bare pub unsafe fn pxui_ui_draw(ui: *mut crate::PxuiUi, box_: *const PxuiBox, over: u8, build: PaintBuild, user: *mut c_void, release: PaintRelease) -> i32 {
    let owner = CallbackOwner { user, release };
    // `owner` releases the callback on every early return, so a poisoned or invalid call still frees the caller's closure.
    let Some(state) = (unsafe { crate::get(ui) }) else { return crate::PXUI_ERR_ARGUMENT };
    if state.shared().poisoned { return crate::PXUI_ERR_PANIC; }
    if box_.is_null() || over > 1 { return unsafe { crate::fail(ui, "invalid paint callback descriptor") }; }
    let Some(b) = crate::core_box(unsafe { *box_ }) else { return unsafe { crate::fail(ui, "invalid paint box") }; };
    let shared = state.shared;
    let callback = move |paint: &mut EnginePaint, rect: pxui_core::ui::Rect| {
        let _owner = &owner;
        if let Some(build) = build {
            let mut borrowed = PxuiPaint { paint, shared };
            build(user, &mut borrowed, Rect { x: rect.x, y: rect.y, w: rect.w, h: rect.h });
        }
    };
    let engine = unsafe { &mut *state.engine };
    if over == 0 { engine.draw(&b, callback); } else { engine.draw_over(&b, callback); }
    0
});

export!(pub unsafe fn pxui_paint_font_size(p: *mut PxuiPaint) -> i64 { unsafe { value(p) }.map_or(0, |p| p.font_size) });
export!(pub unsafe fn pxui_paint_row_height(p: *mut PxuiPaint) -> i64 { unsafe { value(p) }.map_or(0, |p| p.row_height) });
export!(pub unsafe fn pxui_paint_density(p: *mut PxuiPaint) -> i64 { unsafe { value(p) }.map_or(0, |p| p.density) });
export!(pub unsafe fn pxui_paint_clip(p: *mut PxuiPaint) -> Rect { unsafe { value(p) }.map_or(Rect::default(), |p| Rect { x:p.clip_rect.x, y:p.clip_rect.y, w:p.clip_rect.w, h:p.clip_rect.h }) });
export!(pub unsafe fn pxui_paint_clip_radius(p: *mut PxuiPaint) -> f32 { unsafe { value(p) }.map_or(0.0, |p| p.clip_radius) });
export!(pub unsafe fn pxui_paint_text_top(p: *mut PxuiPaint, has_size: u8, size: i64, y: f32, h: f32) -> f32 { if has_size > 1 || !finite(&[y, h]) { return 0.0; } unsafe { value(p) }.map_or(0.0, |p| p.text_top((has_size != 0).then_some(size), y, h)) });
export!(pub unsafe fn pxui_paint_label_size(p: *mut PxuiPaint) -> i64 { unsafe { value(p) }.map_or(0, |p| p.label_size()) });
export!(pub unsafe fn pxui_paint_cap_tracking(p: *mut PxuiPaint) -> f32 { unsafe { value(p) }.map_or(0.0, |p| p.cap_tracking()) });
export!(pub unsafe fn pxui_paint_rtl(p: *mut PxuiPaint) -> u8 { unsafe { value(p) }.map_or(0, |p| p.rtl as u8) });
export!(pub unsafe fn pxui_paint_theme(p: *mut PxuiPaint) -> ThemeTokens {
    let Some(p) = (unsafe { value(p) }) else { return unsafe { core::mem::zeroed() }; };
    crate::theme::tokens(p.theme)
});

export!(pub unsafe fn pxui_paint_fill(p:*mut PxuiPaint,x:f32,y:f32,w:f32,h:f32,c:Rgba) { if finite(&[x,y,w,h]) { if let Some(p)=unsafe{value(p)} { p.fill(x,y,w,h,c); } } });
export!(pub unsafe fn pxui_paint_stroke_width(p:*mut PxuiPaint,x:f32,y:f32,w:f32,h:f32,width:f32,c:Rgba) { if finite(&[x,y,w,h,width]) { if let Some(p)=unsafe{value(p)} { p.stroke_w(x,y,w,h,width,c); } } });
export!(pub unsafe fn pxui_paint_stroke(p:*mut PxuiPaint,x:f32,y:f32,w:f32,h:f32,c:Rgba) { if finite(&[x,y,w,h]) { if let Some(p)=unsafe{value(p)} { p.stroke(x,y,w,h,c); } } });
export!(pub unsafe fn pxui_paint_frame_width(p:*mut PxuiPaint,x:f32,y:f32,w:f32,h:f32,width:f32,c:Rgba) { if finite(&[x,y,w,h,width]) { if let Some(p)=unsafe{value(p)} { p.frame_w(x,y,w,h,width,c); } } });
export!(pub unsafe fn pxui_paint_frame(p:*mut PxuiPaint,x:f32,y:f32,w:f32,h:f32,c:Rgba) { if finite(&[x,y,w,h]) { if let Some(p)=unsafe{value(p)} { p.frame(x,y,w,h,c); } } });
export!(pub unsafe fn pxui_paint_rect(p:*mut PxuiPaint,x:f32,y:f32,w:f32,h:f32,fill:Rgba,has_fill:u8,stroke:Rgba,has_stroke:u8) { if finite(&[x,y,w,h])&&has_fill<=1&&has_stroke<=1 { if let Some(p)=unsafe{value(p)} { p.rect(x,y,w,h,(has_fill!=0).then_some(fill),(has_stroke!=0).then_some(stroke)); } } });
export!(pub unsafe fn pxui_paint_line_width(p:*mut PxuiPaint,x0:f32,y0:f32,x1:f32,y1:f32,width:f32,c:Rgba) { if finite(&[x0,y0,x1,y1,width]) { if let Some(p)=unsafe{value(p)} { p.line_w((x0,y0),(x1,y1),width,c); } } });
export!(pub unsafe fn pxui_paint_line(p:*mut PxuiPaint,x0:f32,y0:f32,x1:f32,y1:f32,c:Rgba) { if finite(&[x0,y0,x1,y1]) { if let Some(p)=unsafe{value(p)} { p.line((x0,y0),(x1,y1),c); } } });
export!(pub unsafe fn pxui_paint_circle(p:*mut PxuiPaint,x:f32,y:f32,r:f32,fill:Rgba,has_fill:u8,stroke:Rgba,has_stroke:u8) { if finite(&[x,y,r])&&has_fill<=1&&has_stroke<=1 { if let Some(p)=unsafe{value(p)} { p.circle((x,y),r,(has_fill!=0).then_some(fill),(has_stroke!=0).then_some(stroke)); } } });
export!(pub unsafe fn pxui_paint_grid(p:*mut PxuiPaint,x:f32,y:f32,w:f32,h:f32,ox:f32,oy:f32,spacing:f32,dot:f32,c:Rgba) { if finite(&[x,y,w,h,ox,oy,spacing,dot]) { if let Some(p)=unsafe{value(p)} { p.grid(x,y,w,h,(ox,oy),spacing,dot,c); } } });
export!(pub unsafe fn pxui_paint_icon(p:*mut PxuiPaint,x:f32,y:f32,font:u16,size:f32,codepoint:u32,c:Rgba)->f32 { if !finite(&[x,y,size])||size<=0.0{return 0.0} let (Some(p),Some(ch))=(unsafe{value(p)},char::from_u32(codepoint)) else{return 0.0}; p.icon((x,y),font,size,ch,c) });

export!(pub unsafe fn pxui_paint_text(p:*mut PxuiPaint,x:f32,y:f32,s:Str) { if finite(&[x,y]) { if let (Some(p),Some(s))=(unsafe{value(p)},unsafe{string(s)}) { p.text((x,y),s); } } });
export!(pub unsafe fn pxui_paint_text_color(p:*mut PxuiPaint,x:f32,y:f32,c:Rgba,s:Str) { if finite(&[x,y]) { if let (Some(p),Some(s))=(unsafe{value(p)},unsafe{string(s)}) { p.text_c((x,y),c,s); } } });
export!(pub unsafe fn pxui_paint_text_size(p:*mut PxuiPaint,x:f32,y:f32,size:i64,c:Rgba,s:Str) { if finite(&[x,y]) { if let (Some(p),Some(s))=(unsafe{value(p)},unsafe{string(s)}) { p.text_s((x,y),size,c,s); } } });
export!(pub unsafe fn pxui_paint_text_with(p:*mut PxuiPaint,x:f32,y:f32,has_size:u8,size:i64,tracking:f32,has_color:u8,color:Rgba,s:Str) { if !finite(&[x,y,tracking])||has_size>1||has_color>1{return} if let (Some(p),Some(s))=(unsafe{value(p)},unsafe{string(s)}) { p.text_with((x,y),(has_size!=0).then_some(size),tracking,(has_color!=0).then_some(color),s); } });
export!(pub unsafe fn pxui_paint_text_styled(p:*mut PxuiPaint,x:f32,y:f32,style_:*const TextStyle,width:f32,has_color:u8,color:Rgba,s:Str)->TextMetrics { if !finite(&[x,y,width])||has_color>1{return TextMetrics::default()} let (Some(p),false)=(unsafe{value(p)},style_.is_null()) else{return TextMetrics::default()}; let Some(s)=(unsafe{string(s)}) else{return TextMetrics::default()}; let Some(style)=style(unsafe{*style_}) else{return TextMetrics::default()}; metrics(p.text_styled((x,y),&style,width,(has_color!=0).then_some(color),s)) });
export!(pub unsafe fn pxui_paint_text_width(p:*mut PxuiPaint,has_size:u8,size:i64,s:Str)->f32 { if has_size>1{return 0.0} let (Some(p),Some(s))=(unsafe{value(p)},unsafe{string(s)}) else{return 0.0}; p.text_width((has_size!=0).then_some(size),s) });
export!(pub unsafe fn pxui_paint_text_metrics(p:*mut PxuiPaint,style_:*const TextStyle,width:f32,s:Str)->TextMetrics { if !width.is_finite(){return TextMetrics::default()} let (Some(p),false)=(unsafe{value(p)},style_.is_null()) else{return TextMetrics::default()}; let Some(s)=(unsafe{string(s)}) else{return TextMetrics::default()}; let Some(style)=style(unsafe{*style_}) else{return TextMetrics::default()}; metrics(p.text_metrics(&style,width,s)) });
export!(pub unsafe fn pxui_paint_caret_x(p:*mut PxuiPaint,has_size:u8,size:i64,s:Str,index:usize)->f32 { if has_size>1{return 0.0} let (Some(p),Some(s))=(unsafe{value(p)},unsafe{string(s)}) else{return 0.0}; if !s.is_char_boundary(index){return 0.0} p.caret_x((has_size!=0).then_some(size),s,index) });
export!(pub unsafe fn pxui_paint_selection(p:*mut PxuiPaint,has_size:u8,size:i64,s:Str,a:usize,b:usize,build:SpanBuild,user:*mut c_void)->i32 { if has_size>1{return -1} let (Some(p),Some(s))=(unsafe{value(p)},unsafe{string(s)}) else{return -1}; if !s.is_char_boundary(a)||!s.is_char_boundary(b){return -1} if let Some(build)=build {p.selection((has_size!=0).then_some(size),s,a,b,&mut |x,w|build(user,x,w));} 0 });
export!(pub unsafe fn pxui_paint_cap(p:*mut PxuiPaint,x:f32,y:f32,has_color:u8,color:Rgba,s:Str) { if !finite(&[x,y])||has_color>1{return} if let (Some(p),Some(s))=(unsafe{value(p)},unsafe{string(s)}) { p.cap((x,y),(has_color!=0).then_some(color),s); } });
export!(pub unsafe fn pxui_paint_cap_width(p:*mut PxuiPaint,s:Str)->f32 { let (Some(p),Some(s))=(unsafe{value(p)},unsafe{string(s)}) else{return 0.0}; p.cap_width(s) });
export!(pub unsafe fn pxui_paint_chevron(p:*mut PxuiPaint,x:f32,y:f32,direction:u32,c:Rgba) { if !finite(&[x,y]) { return; } let Some(p)=(unsafe{value(p)}) else{return}; let d=match direction{0=>pxui_core::ui::Dir::Down,1=>pxui_core::ui::Dir::Up,2=>pxui_core::ui::Dir::Left,3=>pxui_core::ui::Dir::Right,_=>return}; p.chevron((x,y),d,c); });
export!(pub unsafe fn pxui_paint_brackets(p:*mut PxuiPaint,x:f32,y:f32,w:f32,h:f32,offset:f32,length:f32,width:f32,c:Rgba) { if finite(&[x,y,w,h,offset,length,width]) { if let Some(p)=unsafe{value(p)} { p.brackets(x,y,w,h,offset,length,width,c); } } });
export!(pub unsafe fn pxui_paint_dashed_width(p:*mut PxuiPaint,x0:f32,y0:f32,x1:f32,y1:f32,width:f32,c:Rgba) { if finite(&[x0,y0,x1,y1,width]) { if let Some(p)=unsafe{value(p)} { p.dashed_w((x0,y0),(x1,y1),width,c); } } });
export!(pub unsafe fn pxui_paint_dashed(p:*mut PxuiPaint,x0:f32,y0:f32,x1:f32,y1:f32,c:Rgba) { if finite(&[x0,y0,x1,y1]) { if let Some(p)=unsafe{value(p)} { p.dashed((x0,y0),(x1,y1),c); } } });
export!(pub unsafe fn pxui_paint_dashed_rect(p:*mut PxuiPaint,x:f32,y:f32,w:f32,h:f32,c:Rgba) { if finite(&[x,y,w,h]) { if let Some(p)=unsafe{value(p)} { p.dashed_rect(x,y,w,h,c); } } });
export!(pub unsafe fn pxui_paint_flag(p:*mut PxuiPaint,x:f32,y:f32,round:u8,on:u8) { if finite(&[x,y]) && round<=1 && on<=1 { if let Some(p)=unsafe{value(p)} { p.flag((x,y),round!=0,on!=0); } } });
export!(pub unsafe fn pxui_paint_ratio(p:*mut PxuiPaint,x:f32,y:f32,a:i64,b:i64) { if finite(&[x,y]) { if let Some(p)=unsafe{value(p)} { p.ratio((x,y),a,b); } } });
export!(pub unsafe fn pxui_paint_input_region(p:*mut PxuiPaint,cursor:f32,x:f32,y:f32,w:f32,h:f32) { if finite(&[cursor,x,y,w,h]) { if let Some(p)=unsafe{value(p)} { p.input_region(cursor,x,y,w,h); } } });
