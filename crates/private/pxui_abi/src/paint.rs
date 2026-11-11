#![allow(clippy::all)]

use core::ffi::c_void;
use pxui_core::ui::Paint as EnginePaint;
use crate::state::{Shared, finite};
use crate::text::{metrics, style};
use crate::{PXUI_ERR_ARGUMENT, PXUI_ERR_STATE};
use pxui_types::{draw::Instance, geom::{Rect, Rgba}, PxuiBox, Span, Str, TextMetrics, TextStyle, ffi::PxuiPaintInfo};

// A span asks for 1..=MAX_SPAN instances; the library gives at least MIN_SPAN so a drawing makes few calls.
const MAX_SPAN: usize = 1 << 20;
const MIN_SPAN: usize = 256;

/// The window of the layer's spare instance storage handed out by `pxui_paint_span` and not yet committed.
#[derive(Clone, Copy)]
struct Window { base: *mut Instance, base_len: usize, room: usize, open: bool }

#[repr(C)]
pub struct PxuiPaint { paint: *mut EnginePaint, pub(crate) shared: *mut Shared, window: Window }

pub type PaintBuild = Option<extern "C" fn(*mut c_void, *mut PxuiPaint, Rect)>;
pub type PaintRelease = Option<extern "C" fn(*mut c_void)>;
pub type SpanBuild = Option<extern "C" fn(*mut c_void, f32, f32)>;

pub(crate) struct CallbackOwner { pub(crate) user: *mut c_void, pub(crate) release: PaintRelease }
impl Drop for CallbackOwner { fn drop(&mut self) { if let Some(release) = self.release { release(self.user); } } }

// The engine's painter for a call that pushes instances itself: not while a span is open, since the engine would push
// behind the caller's uncommitted writes (the wrapper commits before every such call).
pub(crate) unsafe fn value<'a>(paint: *mut PxuiPaint) -> Option<&'a mut EnginePaint> {
    if paint.is_null() { return None; }
    if unsafe { (*paint).window.open } {
        unsafe { let error = &mut (*(*paint).shared).error; error.clear(); error.push_str("a paint span is open: commit it before this call"); }
        return None;
    }
    unsafe { (*paint).paint.as_mut() }
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
            let mut borrowed = PxuiPaint { paint, shared, window: Window { base: core::ptr::null_mut(), base_len: 0, room: 0, open: false } };
            build(user, &mut borrowed, Rect { x: rect.x, y: rect.y, w: rect.w, h: rect.h });
        }
    };
    let engine = unsafe { &mut *state.engine };
    if over == 0 { engine.draw(&b, callback); } else { engine.draw_over(&b, callback); }
    0
});


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
export!(pub unsafe fn pxui_paint_ratio(p:*mut PxuiPaint,x:f32,y:f32,a:i64,b:i64) { if finite(&[x,y]) { if let Some(p)=unsafe{value(p)} { p.ratio((x,y),a,b); } } });
export!(pub unsafe fn pxui_paint_input_region(p:*mut PxuiPaint,cursor:f32,x:f32,y:f32,w:f32,h:f32) { if finite(&[cursor,x,y,w,h]) { if let Some(p)=unsafe{value(p)} { p.input_region(cursor,x,y,w,h); } } });

// What a paint callback reads once: sizes, clip and theme tokens.
export!(pub unsafe fn pxui_paint_info(p: *mut PxuiPaint, out: *mut PxuiPaintInfo) -> i32 {
    if p.is_null() || out.is_null() || unsafe { (*out).size } < core::mem::size_of::<PxuiPaintInfo>() as u32 { return PXUI_ERR_ARGUMENT; }
    let Some(paint) = (unsafe { (*p).paint.as_ref() }) else { return PXUI_ERR_ARGUMENT; };
    unsafe {
        *out = PxuiPaintInfo {
            size: core::mem::size_of::<PxuiPaintInfo>() as u32, rtl: paint.rtl as u8, reserved: [0; 3], font_size: paint.font_size,
            row_height: paint.row_height, density: paint.density,
            clip: Rect { x: paint.clip_rect.x, y: paint.clip_rect.y, w: paint.clip_rect.w, h: paint.clip_rect.h }, clip_radius: paint.clip_radius,
            theme: paint.theme.into(),
        };
    }
    0
});

// Commits what the caller wrote into the previous span (`span.at` is where it stopped) and, when `at_least` is not zero, hands
// out a window with room for at least that many more instances, stamped with the painter's clip, clip radius and view.
// With `at_least == 0` the span comes back empty: the wrapper does that before every call that pushes instances itself.
export!(pub unsafe fn pxui_paint_span(p: *mut PxuiPaint, at_least: usize, span: *mut Span) -> i32 {
    if p.is_null() || span.is_null() || at_least > MAX_SPAN { return PXUI_ERR_ARGUMENT; }
    let handle = unsafe { &mut *p };
    let Some(paint) = (unsafe { handle.paint.as_mut() }) else { return PXUI_ERR_ARGUMENT; };
    if unsafe { (*span).size } < core::mem::size_of::<Span>() as u32 { return PXUI_ERR_ARGUMENT; }
    let (stamp_clip, stamp_radius, stamp_view) = paint.stamp();
    let instances = paint.instances_mut();
    let size = core::mem::size_of::<Instance>();
    if handle.window.open {
        handle.window.open = false;
        // The vector must be where it was handed out: no push or reallocation since, and the caller's pointer inside the window.
        let at = unsafe { (*span).at } as usize;
        let base = handle.window.base as usize;
        if instances.len() != handle.window.base_len || instances.as_mut_ptr().wrapping_add(instances.len()) as usize != base
            || at < base || (at - base) % size != 0 || (at - base) / size > handle.window.room {
            return PXUI_ERR_STATE;
        }
        unsafe { instances.set_len(instances.len() + (at - base) / size); }
    }
    let empty = Span { size: core::mem::size_of::<Span>() as u32, view: stamp_view, clip_radius_f16: stamp_radius, clip: stamp_clip, at: core::ptr::null_mut(), end: core::ptr::null_mut() };
    if at_least == 0 { unsafe { *span = empty; } return 0; }
    instances.reserve(at_least.max(MIN_SPAN));
    let room = instances.capacity() - instances.len();
    let base = instances.as_mut_ptr().wrapping_add(instances.len());
    handle.window = Window { base, base_len: instances.len(), room, open: true };
    unsafe { *span = Span { at: base, end: base.wrapping_add(room), ..empty }; }
    0
});
