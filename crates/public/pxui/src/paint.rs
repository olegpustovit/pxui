use core::{ffi::c_void, marker::PhantomData, ptr::NonNull};
use pxui_types::{geom::{Rect, Rgba}, theme::Tokens, Dir, PxuiBox, Str, TextMetrics, TextStyle};

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{PxuiPaint as RawPaint, pxui_paint_brackets, pxui_paint_cap, pxui_paint_cap_tracking,
        pxui_paint_cap_width, pxui_paint_caret_x, pxui_paint_chevron, pxui_paint_circle, pxui_paint_clip,
        pxui_paint_clip_radius, pxui_paint_dashed, pxui_paint_dashed_rect, pxui_paint_dashed_width,
        pxui_paint_density, pxui_paint_fill, pxui_paint_flag, pxui_paint_font_size, pxui_paint_frame,
        pxui_paint_frame_width, pxui_paint_grid, pxui_paint_icon, pxui_paint_input_region,
        pxui_paint_label_size, pxui_paint_line, pxui_paint_line_width, pxui_paint_ratio,
        pxui_paint_rect, pxui_paint_row_height, pxui_paint_rtl, pxui_paint_selection, pxui_paint_stroke,
        pxui_paint_stroke_width, pxui_paint_text, pxui_paint_text_color, pxui_paint_text_metrics, pxui_paint_text_top,
        pxui_paint_text_size, pxui_paint_text_styled, pxui_paint_text_width, pxui_paint_text_with,
        pxui_paint_theme, pxui_ui_draw};
}

#[cfg(not(feature = "static"))]
mod sys {
    use core::ffi::c_void;
    use pxui_types::{geom::{Rect, Rgba}, ffi::ThemeTokens, PxuiBox, Str, TextMetrics, TextStyle};
    #[repr(C)] pub struct RawPaint { _private: [u8; 0] }
    pub type PaintBuild = Option<extern "C" fn(*mut c_void, *mut RawPaint, Rect)>;
    pub type PaintRelease = Option<extern "C" fn(*mut c_void)>;
    pub type SpanBuild = Option<extern "C" fn(*mut c_void, f32, f32)>;
    #[link(name = "pxui")]
    unsafe extern "C" {
        pub fn pxui_ui_draw(ui:*mut c_void,box_:*const PxuiBox,over:u8,build:PaintBuild,user:*mut c_void,release:PaintRelease)->i32;
        pub fn pxui_paint_font_size(p:*mut RawPaint)->i64; pub fn pxui_paint_row_height(p:*mut RawPaint)->i64; pub fn pxui_paint_density(p:*mut RawPaint)->i64;
        pub fn pxui_paint_clip(p:*mut RawPaint)->Rect; pub fn pxui_paint_clip_radius(p:*mut RawPaint)->f32;
        pub fn pxui_paint_text_top(p:*mut RawPaint,has_size:u8,size:i64,y:f32,h:f32)->f32; pub fn pxui_paint_label_size(p:*mut RawPaint)->i64;
        pub fn pxui_paint_cap_tracking(p:*mut RawPaint)->f32; pub fn pxui_paint_theme(p:*mut RawPaint)->ThemeTokens;
        pub fn pxui_paint_rtl(p:*mut RawPaint)->u8;
        pub fn pxui_paint_fill(p:*mut RawPaint,x:f32,y:f32,w:f32,h:f32,c:Rgba); pub fn pxui_paint_stroke_width(p:*mut RawPaint,x:f32,y:f32,w:f32,h:f32,width:f32,c:Rgba);
        pub fn pxui_paint_stroke(p:*mut RawPaint,x:f32,y:f32,w:f32,h:f32,c:Rgba); pub fn pxui_paint_frame_width(p:*mut RawPaint,x:f32,y:f32,w:f32,h:f32,width:f32,c:Rgba);
        pub fn pxui_paint_frame(p:*mut RawPaint,x:f32,y:f32,w:f32,h:f32,c:Rgba); pub fn pxui_paint_rect(p:*mut RawPaint,x:f32,y:f32,w:f32,h:f32,fill:Rgba,has_fill:u8,stroke:Rgba,has_stroke:u8);
        pub fn pxui_paint_line_width(p:*mut RawPaint,x0:f32,y0:f32,x1:f32,y1:f32,width:f32,c:Rgba); pub fn pxui_paint_line(p:*mut RawPaint,x0:f32,y0:f32,x1:f32,y1:f32,c:Rgba);
        pub fn pxui_paint_circle(p:*mut RawPaint,x:f32,y:f32,r:f32,fill:Rgba,has_fill:u8,stroke:Rgba,has_stroke:u8);
        pub fn pxui_paint_grid(p:*mut RawPaint,x:f32,y:f32,w:f32,h:f32,ox:f32,oy:f32,spacing:f32,dot:f32,c:Rgba);
        pub fn pxui_paint_icon(p:*mut RawPaint,x:f32,y:f32,font:u16,size:f32,codepoint:u32,c:Rgba)->f32;
        pub fn pxui_paint_text(p:*mut RawPaint,x:f32,y:f32,s:Str); pub fn pxui_paint_text_color(p:*mut RawPaint,x:f32,y:f32,c:Rgba,s:Str);
        pub fn pxui_paint_text_size(p:*mut RawPaint,x:f32,y:f32,size:i64,c:Rgba,s:Str);
        pub fn pxui_paint_text_with(p:*mut RawPaint,x:f32,y:f32,has_size:u8,size:i64,tracking:f32,has_color:u8,color:Rgba,s:Str);
        pub fn pxui_paint_text_styled(p:*mut RawPaint,x:f32,y:f32,style:*const TextStyle,width:f32,has_color:u8,color:Rgba,s:Str)->TextMetrics;
        pub fn pxui_paint_text_width(p:*mut RawPaint,has_size:u8,size:i64,s:Str)->f32;
        pub fn pxui_paint_text_metrics(p:*mut RawPaint,style:*const TextStyle,width:f32,s:Str)->TextMetrics;
        pub fn pxui_paint_caret_x(p:*mut RawPaint,has_size:u8,size:i64,s:Str,index:usize)->f32;
        pub fn pxui_paint_selection(p:*mut RawPaint,has_size:u8,size:i64,s:Str,a:usize,b:usize,build:SpanBuild,user:*mut c_void)->i32;
        pub fn pxui_paint_cap(p:*mut RawPaint,x:f32,y:f32,has_color:u8,color:Rgba,s:Str); pub fn pxui_paint_cap_width(p:*mut RawPaint,s:Str)->f32;
        pub fn pxui_paint_chevron(p:*mut RawPaint,x:f32,y:f32,direction:u32,c:Rgba);
        pub fn pxui_paint_brackets(p:*mut RawPaint,x:f32,y:f32,w:f32,h:f32,offset:f32,length:f32,width:f32,c:Rgba);
        pub fn pxui_paint_dashed_width(p:*mut RawPaint,x0:f32,y0:f32,x1:f32,y1:f32,width:f32,c:Rgba);
        pub fn pxui_paint_dashed(p:*mut RawPaint,x0:f32,y0:f32,x1:f32,y1:f32,c:Rgba); pub fn pxui_paint_dashed_rect(p:*mut RawPaint,x:f32,y:f32,w:f32,h:f32,c:Rgba);
        pub fn pxui_paint_flag(p:*mut RawPaint,x:f32,y:f32,round:u8,on:u8); pub fn pxui_paint_ratio(p:*mut RawPaint,x:f32,y:f32,a:i64,b:i64);
        pub fn pxui_paint_input_region(p:*mut RawPaint,cursor:f32,x:f32,y:f32,w:f32,h:f32);
    }
}

pub struct Paint<'a> {
    raw: NonNull<sys::RawPaint>, _borrow: PhantomData<&'a mut sys::RawPaint>,
    pub theme: Tokens, pub font_size: i64, pub row_height: i64, pub density: i64,
    pub clip_rect: Rect, pub clip_radius: f32, pub rtl: bool,
}

impl<'a> Paint<'a> {
    unsafe fn from_raw(raw:*mut sys::RawPaint)->Option<Self>{NonNull::new(raw).map(|raw|Self{
        theme:unsafe{sys::pxui_paint_theme(raw.as_ptr())}.into(),font_size:unsafe{sys::pxui_paint_font_size(raw.as_ptr())},row_height:unsafe{sys::pxui_paint_row_height(raw.as_ptr())},
        density:unsafe{sys::pxui_paint_density(raw.as_ptr())},clip_rect:unsafe{sys::pxui_paint_clip(raw.as_ptr())},clip_radius:unsafe{sys::pxui_paint_clip_radius(raw.as_ptr())},rtl:unsafe{sys::pxui_paint_rtl(raw.as_ptr())}!=0,
        raw,_borrow:PhantomData,
    })}
    pub(crate) fn raw(&mut self)->*mut sys::RawPaint{self.raw.as_ptr()}
    pub(crate) fn image_raw(&mut self)->*mut c_void{self.raw.as_ptr().cast()}
    pub fn font_size(&self)->i64{unsafe{sys::pxui_paint_font_size(self.raw.as_ptr())}}
    pub fn row_height(&self)->i64{unsafe{sys::pxui_paint_row_height(self.raw.as_ptr())}}
    pub fn density(&self)->i64{unsafe{sys::pxui_paint_density(self.raw.as_ptr())}}
    pub fn clip_rect(&self)->Rect{unsafe{sys::pxui_paint_clip(self.raw.as_ptr())}}
    pub fn clip_radius(&self)->f32{unsafe{sys::pxui_paint_clip_radius(self.raw.as_ptr())}}
    pub fn text_top(&self,size:Option<i64>,y:f32,h:f32)->f32{unsafe{sys::pxui_paint_text_top(self.raw.as_ptr(),size.is_some() as u8,size.unwrap_or_default(),y,h)}}
    pub fn label_size(&self)->i64{unsafe{sys::pxui_paint_label_size(self.raw.as_ptr())}}
    pub fn cap_tracking(&self)->f32{unsafe{sys::pxui_paint_cap_tracking(self.raw.as_ptr())}}
    pub fn fill(&mut self,x:f32,y:f32,w:f32,h:f32,c:Rgba){unsafe{sys::pxui_paint_fill(self.raw(),x,y,w,h,c)}}
    pub fn stroke_w(&mut self,x:f32,y:f32,w:f32,h:f32,width:f32,c:Rgba){unsafe{sys::pxui_paint_stroke_width(self.raw(),x,y,w,h,width,c)}}
    pub fn stroke(&mut self,x:f32,y:f32,w:f32,h:f32,c:Rgba){unsafe{sys::pxui_paint_stroke(self.raw(),x,y,w,h,c)}}
    pub fn frame_w(&mut self,x:f32,y:f32,w:f32,h:f32,width:f32,c:Rgba){unsafe{sys::pxui_paint_frame_width(self.raw(),x,y,w,h,width,c)}}
    pub fn frame(&mut self,x:f32,y:f32,w:f32,h:f32,c:Rgba){unsafe{sys::pxui_paint_frame(self.raw(),x,y,w,h,c)}}
    pub fn rect(&mut self,x:f32,y:f32,w:f32,h:f32,fill:Option<Rgba>,stroke:Option<Rgba>){unsafe{sys::pxui_paint_rect(self.raw(),x,y,w,h,fill.unwrap_or_else(transparent),fill.is_some() as u8,stroke.unwrap_or_else(transparent),stroke.is_some() as u8)}}
    pub fn line_w(&mut self,a:(f32,f32),b:(f32,f32),width:f32,c:Rgba){unsafe{sys::pxui_paint_line_width(self.raw(),a.0,a.1,b.0,b.1,width,c)}}
    pub fn line(&mut self,a:(f32,f32),b:(f32,f32),c:Rgba){unsafe{sys::pxui_paint_line(self.raw(),a.0,a.1,b.0,b.1,c)}}
    pub fn circle(&mut self,at:(f32,f32),r:f32,fill:Option<Rgba>,stroke:Option<Rgba>){unsafe{sys::pxui_paint_circle(self.raw(),at.0,at.1,r,fill.unwrap_or_else(transparent),fill.is_some() as u8,stroke.unwrap_or_else(transparent),stroke.is_some() as u8)}}
    pub fn grid(&mut self,x:f32,y:f32,w:f32,h:f32,origin:(f32,f32),spacing:f32,dot:f32,c:Rgba){unsafe{sys::pxui_paint_grid(self.raw(),x,y,w,h,origin.0,origin.1,spacing,dot,c)}}
    pub fn icon(&mut self,at:(f32,f32),font:u16,size:f32,ch:char,c:Rgba)->f32{unsafe{sys::pxui_paint_icon(self.raw(),at.0,at.1,font,size,ch as u32,c)}}
    pub fn text(&mut self,at:(f32,f32),s:&str){unsafe{sys::pxui_paint_text(self.raw(),at.0,at.1,as_str(s))}}
    pub fn text_c(&mut self,at:(f32,f32),c:Rgba,s:&str){unsafe{sys::pxui_paint_text_color(self.raw(),at.0,at.1,c,as_str(s))}}
    pub fn text_s(&mut self,at:(f32,f32),size:i64,c:Rgba,s:&str){unsafe{sys::pxui_paint_text_size(self.raw(),at.0,at.1,size,c,as_str(s))}}
    pub fn text_with(&mut self,at:(f32,f32),size:Option<i64>,tracking:f32,color:Option<Rgba>,s:&str){unsafe{sys::pxui_paint_text_with(self.raw(),at.0,at.1,size.is_some() as u8,size.unwrap_or_default(),tracking,color.is_some() as u8,color.unwrap_or_else(transparent),as_str(s))}}
    pub fn text_styled(&mut self,at:(f32,f32),style:&TextStyle,width:f32,color:Option<Rgba>,s:&str)->TextMetrics{unsafe{sys::pxui_paint_text_styled(self.raw(),at.0,at.1,style,width,color.is_some() as u8,color.unwrap_or_else(transparent),as_str(s))}}
    pub fn text_width(&mut self,size:Option<i64>,s:&str)->f32{unsafe{sys::pxui_paint_text_width(self.raw(),size.is_some() as u8,size.unwrap_or_default(),as_str(s))}}
    pub fn text_metrics(&mut self,style:&TextStyle,width:f32,s:&str)->TextMetrics{unsafe{sys::pxui_paint_text_metrics(self.raw(),style,width,as_str(s))}}
    pub fn caret_x(&mut self,size:Option<i64>,s:&str,index:usize)->f32{unsafe{sys::pxui_paint_caret_x(self.raw(),size.is_some() as u8,size.unwrap_or_default(),as_str(s),index)}}
    pub fn selection<F:FnMut(f32,f32)>(&mut self,size:Option<i64>,s:&str,a:usize,b:usize,mut f:F)->Result<(),i32>{extern "C" fn call<F:FnMut(f32,f32)>(u:*mut c_void,x:f32,w:f32){unsafe{(&mut *u.cast::<F>())(x,w)}}let rc=unsafe{sys::pxui_paint_selection(self.raw(),size.is_some() as u8,size.unwrap_or_default(),as_str(s),a,b,Some(call::<F>),(&mut f as *mut F).cast::<c_void>())};if rc==0{Ok(())}else{Err(rc)}}
    pub fn cap(&mut self,at:(f32,f32),color:Option<Rgba>,s:&str){unsafe{sys::pxui_paint_cap(self.raw(),at.0,at.1,color.is_some() as u8,color.unwrap_or_else(transparent),as_str(s))}}
    pub fn cap_width(&mut self,s:&str)->f32{unsafe{sys::pxui_paint_cap_width(self.raw(),as_str(s))}}
    pub fn chevron(&mut self,at:(f32,f32),direction:Dir,c:Rgba){let d=match direction{Dir::Down=>0,Dir::Up=>1,Dir::Left=>2,Dir::Right=>3};unsafe{sys::pxui_paint_chevron(self.raw(),at.0,at.1,d,c)}}
    pub fn brackets(&mut self,x:f32,y:f32,w:f32,h:f32,offset:f32,length:f32,width:f32,c:Rgba){unsafe{sys::pxui_paint_brackets(self.raw(),x,y,w,h,offset,length,width,c)}}
    pub fn dashed_w(&mut self,a:(f32,f32),b:(f32,f32),width:f32,c:Rgba){unsafe{sys::pxui_paint_dashed_width(self.raw(),a.0,a.1,b.0,b.1,width,c)}}
    pub fn dashed(&mut self,a:(f32,f32),b:(f32,f32),c:Rgba){unsafe{sys::pxui_paint_dashed(self.raw(),a.0,a.1,b.0,b.1,c)}}
    pub fn dashed_rect(&mut self,x:f32,y:f32,w:f32,h:f32,c:Rgba){unsafe{sys::pxui_paint_dashed_rect(self.raw(),x,y,w,h,c)}}
    pub fn flag(&mut self,at:(f32,f32),round:bool,on:bool){unsafe{sys::pxui_paint_flag(self.raw(),at.0,at.1,round as u8,on as u8)}}
    pub fn ratio(&mut self,at:(f32,f32),a:i64,b:i64){unsafe{sys::pxui_paint_ratio(self.raw(),at.0,at.1,a,b)}}
    pub fn input_region(&mut self,cursor:f32,x:f32,y:f32,w:f32,h:f32){unsafe{sys::pxui_paint_input_region(self.raw(),cursor,x,y,w,h)}}
}

struct Draw<F>(F);
extern "C" fn draw<F:FnMut(&mut Paint<'_>,Rect)>(user:*mut c_void,raw:*mut sys::RawPaint,rect:Rect){
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{
        let Some(mut paint)=(unsafe{Paint::from_raw(raw)}) else{return};
        (unsafe{&mut *user.cast::<Draw<F>>()}.0)(&mut paint,rect);
    }));
    if let Err(panic)=result { PANIC.with(|slot| { let mut slot=slot.borrow_mut(); if slot.is_none(){*slot=Some(panic);} }); }
}
extern "C" fn release<F>(user:*mut c_void){unsafe{drop(Box::from_raw(user.cast::<Draw<F>>()))}}
thread_local!{static PANIC:std::cell::RefCell<Option<Box<dyn std::any::Any+Send>>>=const{std::cell::RefCell::new(None)};}
pub(crate) fn take_callback_panic()->Option<Box<dyn std::any::Any+Send>>{PANIC.with(|slot|slot.borrow_mut().take())}

impl crate::Ui {
    pub fn draw<F:for<'paint> FnMut(&mut Paint<'paint>,Rect)+'static>(&mut self,box_:&PxuiBox,callback:F)->Result<(),i32>{
        let user=Box::into_raw(Box::new(Draw::<F>(callback))).cast::<c_void>();
        let rc=unsafe{sys::pxui_ui_draw(self.0.as_ptr().cast(),box_,0,Some(draw::<F>),user,Some(release::<F>))};
        if rc==0{Ok(())}else{Err(rc)}
    }
    pub fn draw_over<F:for<'paint> FnMut(&mut Paint<'paint>,Rect)+'static>(&mut self,box_:&PxuiBox,callback:F)->Result<(),i32>{
        let user=Box::into_raw(Box::new(Draw::<F>(callback))).cast::<c_void>();
        let rc=unsafe{sys::pxui_ui_draw(self.0.as_ptr().cast(),box_,1,Some(draw::<F>),user,Some(release::<F>))};
        if rc==0{Ok(())}else{Err(rc)}
    }
}

fn as_str(s:&str)->Str{Str{ptr:s.as_ptr(),len:s.len()}}
fn transparent()->Rgba{Rgba{r:0,g:0,b:0,a:0}}
