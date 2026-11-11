use ogpu_core::backend::{self as gpu_backend, Device, Queue, Texture};
use ogpu_core::pipeline::{ColorFormat, DepthFormat};
use ogpu_core::types::{Extent, Origin, TextureFormat};
use pxui_core::ui::Ui;
use pxui_render::{AtlasKind, Break, Painter, View};
use pxui_core::text::AtlasKind as TextKind;

use crate::state::{fail, get, PxuiUi};
use crate::{PXUI_ERR_ARGUMENT, PXUI_ERR_GPU, PXUI_ERR_STATE, PxuiTargetDesc};
use core::ffi::c_void;

pub struct PxuiRenderer {
    painter: Painter,
    queue: Queue,
    device: Device,
    textures: std::collections::HashMap<usize, Texture>,
    engine: *mut Ui,
    pub(crate) shared: *mut crate::state::Shared,
    format: TextureFormat,
    samples: i64,
    /// the last frame passed to `prepare`, and the last `completed` the app reported
    frame_serial: u64,
    completed: u64,
    atlas_initialized: bool,
}

impl PxuiRenderer {
    /// Record `message` as the owning `Ui`'s last error and return `code`.
    fn fail(&mut self, code: i32, message: &str) -> i32 {
        unsafe { let e=&mut (*self.shared).error; e.clear(); e.push_str(message); }
        code
    }
    fn fail_gpu(&mut self, error: ogpu_core::error::Error) -> i32 { self.fail(PXUI_ERR_GPU, &ogpu_core::error::to_string(&error)) }
}

struct GpuLease { token:*mut c_void, release:Option<extern "C" fn(*mut c_void)>, active:bool }
impl Drop for GpuLease {
    fn drop(&mut self) {
        if self.active {
            if let Some(release)=self.release { release(self.token); }
        }
    }
}

/// A renderer on the app's device, or why it could not be made. The lease on the app's device is released on every refusal
/// until the adopted driver owns it.
unsafe fn make(ui: &mut PxuiUi, gpu: &pxui_types::Gpu, target: PxuiTargetDesc) -> Result<PxuiRenderer, String> {
    let mut lease=GpuLease { token:gpu.owner_token, release:gpu.release_owner, active:true };
    if gpu.size < core::mem::size_of::<pxui_types::Gpu>() as u32 { return Err("invalid gpu descriptor".into()); }
    if target.size < core::mem::size_of::<PxuiTargetDesc>() as u32 || target.samples == 0 { return Err("invalid target descriptor".into()); }
    let (format,color) = match target.format { 1=>(TextureFormat::Rgba8Unorm,ColorFormat::Rgba8Unorm), 2=>(TextureFormat::Bgra8Unorm,ColorFormat::Bgra8Unorm), _=>return Err("unknown target format".into()) };
    let native=ogpu_core::backend::NativeDevice { size:gpu.size, backend:gpu.backend, instance:gpu.instance, physical_device:gpu.physical_device, device:gpu.device, queue:gpu.queue, queue_family:gpu.queue_family, api_version:gpu.api_version, features:gpu.features, get_instance_proc_addr:gpu.get_instance_proc_addr, pipeline_cache:gpu.pipeline_cache, mtl_device:gpu.mtl_device, wl_display:gpu.wl_display, wl_surface:gpu.wl_surface, metal_layer:gpu.metal_layer, hwnd:gpu.hwnd, owner_token:gpu.owner_token, release_owner:gpu.release_owner };
    lease.active=false;
    // This copy of OGPU (the library's own) has not been told which backend it drives: the app selected one in its copy.
    #[cfg(target_os = "macos")]
    ogpu_core::impl_::select(ogpu_metal::impl_::implementation());
    #[cfg(any(windows,target_os = "linux"))]
    ogpu_core::impl_::select(ogpu_core::impl_::Impl { backend: ogpu_vulkan::impl_::BACKEND, create_driver: ogpu_vulkan::backend::create, live_handles: ogpu_vulkan::impl_::live_handles });
    #[cfg(target_os = "macos")]
    let driver=ogpu_metal::adopt(native);
    #[cfg(any(windows,target_os = "linux"))]
    let driver=ogpu_vulkan::adopt(native);
    let describe=|e: ogpu_core::error::Error| ogpu_core::error::to_string(&e);
    let device=gpu_backend::create_device(driver.map_err(describe)?).map_err(describe)?;
    let queue=match gpu_backend::create_queue(&device) { Ok(q)=>q, Err(e)=>{ let _=gpu_backend::destroy_device(&device); return Err(describe(e)); } };
    let (mask,color_size)=unsafe { let text=(&mut *ui.engine).text_mut(); (text.atlas_size(TextKind::Mask),text.atlas_size(TextKind::Color)) };
    if (target.mask_width!=0 && target.mask_width!=mask.0) || (target.mask_height!=0 && target.mask_height!=mask.1) || (target.color_width!=0 && target.color_width!=color_size.0) || (target.color_height!=0 && target.color_height!=color_size.1) {
        let _=gpu_backend::destroy_queue(&queue); let _=gpu_backend::destroy_device(&device); return Err("the target's atlas sizes differ from the Ui's".into());
    }
    let painter=match Painter::new(&device,color,DepthFormat::NoDepth,i64::from(target.samples),mask,color_size) {
        Ok(p)=>p,
        Err(e)=>{ let _=gpu_backend::destroy_queue(&queue); let _=gpu_backend::destroy_device(&device); return Err(describe(e)); }
    };
    Ok(PxuiRenderer { painter, queue, device, textures:std::collections::HashMap::new(), engine:ui.engine, shared:ui.shared, format, samples:i64::from(target.samples), frame_serial:0, completed:0, atlas_initialized:false })
}

// Makes a renderer for the owner `Ui` (never a callback's borrowed handle) outside a frame; null, with the reason in
// `pxui_last_error`, when it cannot be made.
export!(pub unsafe fn pxui_renderer_create(ui:*mut PxuiUi, gpu:*const pxui_types::Gpu, target:*const PxuiTargetDesc) -> *mut PxuiRenderer {
    let Some(state)=(unsafe { get(ui) }) else { return core::ptr::null_mut(); };
    // a borrowed callback handle's engine is only live for its frame: a renderer needs the owner, outside a frame
    if !state.owner || state.shared().in_frame || gpu.is_null() || target.is_null() {
        unsafe { fail(ui, "a renderer needs the owner Ui outside a frame, a gpu and a target"); }
        return core::ptr::null_mut();
    }
    match unsafe { make(state, &*gpu, *target) } {
        Ok(renderer)=>{ state.shared().renderers += 1; Box::into_raw(Box::new(renderer)) }
        Err(message)=>{ unsafe { fail(ui, &message); } core::ptr::null_mut() }
    }
});

export!(pub unsafe fn pxui_renderer_prepare(renderer:*mut PxuiRenderer, commands:*const pxui_types::Commands, frame:u64, completed:u64) -> i32 {
    if renderer.is_null() { return PXUI_ERR_ARGUMENT; }
    let renderer=unsafe { &mut *renderer };
    if commands.is_null() || unsafe { (*commands).size } < core::mem::size_of::<pxui_types::Commands>() as u32 { return renderer.fail(PXUI_ERR_ARGUMENT, "invalid commands descriptor"); }
    let commands=unsafe { *commands };
    // The painter's instance ring and the glyph atlas reuse what frame `frame - FRAMES` used: that frame must be done.
    if frame <= renderer.frame_serial { return renderer.fail(PXUI_ERR_ARGUMENT, "frame serials must increase"); }
    if completed >= frame || completed < renderer.completed || frame - completed > pxui_render::FRAMES as u64 {
        return renderer.fail(PXUI_ERR_ARGUMENT, "completed must not go back and must be at least frame - 3");
    }
    let native=ogpu_core::backend::NativeCommands { size:core::mem::size_of::<ogpu_core::backend::NativeCommands>() as u32, vk_command_buffer:commands.vk_command_buffer, mtl_command_buffer:commands.mtl_command_buffer, frame_serial:frame, inside_pass:0 };
    let commands=match gpu_backend::adopt_commands(&renderer.queue,native) { Ok(c)=>c, Err(e)=>return renderer.fail_gpu(e) };
    if let Err(e)=gpu_backend::retire(&renderer.queue,completed) { return renderer.fail_gpu(e); }
    renderer.completed=completed;
    let ui=unsafe { &mut *renderer.engine };
    let mut failed=None;
    if !renderer.atlas_initialized {
        for (kind,atlas,texel) in [(TextKind::Mask,AtlasKind::Mask,1i64),(TextKind::Color,AtlasKind::Color,4i64)] {
            let (w,h)=ui.text_mut().atlas_size(kind);
            let bytes=ui.text_mut().atlas_pixels(kind);
            if let Err(error)=gpu_backend::write_texture_on(&commands,renderer.painter.atlas(atlas),None,Origin{x:0,y:0,z:0},Extent{width:w as i64,height:h as i64,depth:1},(w as i64)*texel,bytes) {
                failed=Some(error);
                break;
            }
        }
        if failed.is_none() {
            ui.text_mut().drain_uploads(&mut |_,_,_,_,_,_,_| {});
            renderer.atlas_initialized=true;
        }
    }
    if let Some(e)=failed { return renderer.fail_gpu(e); }
    ui.text_mut().drain_uploads(&mut |kind,x,y,w,h,bytes,stride| {
        let atlas=if kind==TextKind::Mask { AtlasKind::Mask } else { AtlasKind::Color };
        let result=gpu_backend::write_texture_on(&commands,renderer.painter.atlas(atlas),None,Origin{x:x as i64,y:y as i64,z:0},Extent{width:w as i64,height:h as i64,depth:1},stride as i64,bytes);
        if let Err(error)=result { if failed.is_none() { failed=Some(error); } }
    });
    if let Some(e)=failed { renderer.atlas_initialized=false; return renderer.fail_gpu(e); }
    renderer.frame_serial=frame;
    renderer.painter.begin_frame();
    0
});

export!(pub unsafe fn pxui_renderer_draw(renderer:*mut PxuiRenderer, pass:*const pxui_types::Pass) -> i32 {
    if renderer.is_null() { return PXUI_ERR_ARGUMENT; }
    let renderer=unsafe { &mut *renderer };
    if pass.is_null() { return renderer.fail(PXUI_ERR_ARGUMENT, "pass is null"); }
    let pass=unsafe { *pass };
    if pass.size < core::mem::size_of::<pxui_types::Pass>() as u32 || pass.width_px==0 || pass.height_px==0 || !pass.scale.is_finite() || pass.scale<=0.0 { return renderer.fail(PXUI_ERR_ARGUMENT, "invalid pass descriptor"); }
    if renderer.frame_serial==0 { return renderer.fail(PXUI_ERR_STATE, "draw needs a prepared frame"); }
    let commands_native=ogpu_core::backend::NativeCommands { size:core::mem::size_of::<ogpu_core::backend::NativeCommands>() as u32, vk_command_buffer:pass.vk_command_buffer, mtl_command_buffer:pass.mtl_command_buffer, frame_serial:renderer.frame_serial, inside_pass:1 };
    let native_pass=ogpu_core::backend::NativePass { size:core::mem::size_of::<ogpu_core::backend::NativePass>() as u32, vk_command_buffer:pass.vk_command_buffer, mtl_render_encoder:pass.mtl_render_encoder, width_px:pass.width_px, height_px:pass.height_px, scale:pass.scale };
    let commands=match gpu_backend::adopt_commands(&renderer.queue,commands_native) { Ok(c)=>c, Err(e)=>return renderer.fail_gpu(e) };
    let encoder=match gpu_backend::adopt_render_encoder(&commands,native_pass,renderer.format,renderer.samples) { Ok(e)=>e, Err(e)=>return renderer.fail_gpu(e) };
    let ui=unsafe { &mut *renderer.engine };
    for (index,v) in ui.output().views {
        if let Err(e)=renderer.painter.set_view(*index,View { offset:v.offset,opacity:v.opacity,clip:v.clip,scale:v.scale,transform:v.transform,pivot:v.pivot }) { return renderer.fail_gpu(e); }
    }
    let mut used_textures=std::collections::HashSet::new();
    for index in 0..ui.output().draw.len() {
        let layer=&ui.output().draw[index];
        // Adopt foreign textures before building `Break` references. Keeping those
        // references alive while inserting another texture would borrow the map
        // immutably across a mutation.
        for b in &layer.breaks {
            if b.image==pxui_core::draw::ATLAS { continue; }
            let Some(image)=layer.images.get(b.image as usize) else { continue; };
            if image.handle.downcast_ref::<Texture>().is_some() { continue; }
            let Some(key)=crate::image::texture_key(image) else { continue; };
            used_textures.insert(key);
            if !renderer.textures.contains_key(&key) {
                let texture=match crate::image::adopt_texture(&renderer.device,image) { Ok(texture)=>texture, Err(())=>return renderer.fail(PXUI_ERR_GPU, "an app image could not be wrapped") };
                renderer.textures.insert(key,texture);
            }
        }
        let mut breaks=Vec::with_capacity(layer.breaks.len());
        for b in &layer.breaks {
            let texture=if b.image==pxui_core::draw::ATLAS { None } else if let Some(image)=layer.images.get(b.image as usize) {
                if let Some(texture)=image.handle.downcast_ref::<Texture>() { Some(texture) }
                else { crate::image::texture_key(image).and_then(|key|renderer.textures.get(&key)) }
            } else { None };
            breaks.push(Break { at_instance:b.at_instance, texture });
        }
        if let Err(e)=renderer.painter.draw_retained(&encoder,[pass.width_px,pass.height_px],pass.scale,layer.id,&layer.instances,&layer.dirty,&breaks) { drop(breaks); return renderer.fail_gpu(e); }
        ui.drawn(index);
    }
    renderer.textures.retain(|key,_|used_textures.contains(key));
    // ends only this copy's view of the pass; the app's encoder must `invalidate_state` before drawing more
    if let Err(e)=gpu_backend::end_render(&encoder) { return renderer.fail_gpu(e); }
    0
});

export!(bare pub unsafe fn pxui_renderer_destroy(renderer:*mut PxuiRenderer) {
    if renderer.is_null() { return; }
    let PxuiRenderer { painter, queue, device, textures, shared, frame_serial, .. } = *unsafe { Box::from_raw(renderer) };
    // a painter or texture that fails to go stays a child of the device, which then refuses to go: reported below
    drop(painter);
    drop(textures);
    // Contract (pxui.h): the app has finished every frame it passed to prepare, so everything retires now.
    let result=gpu_backend::retire(&queue,frame_serial).and_then(|()| gpu_backend::destroy_queue(&queue)).and_then(|()| gpu_backend::destroy_device(&device));
    if let Err(e)=result { unsafe { let error=&mut (*shared).error; error.clear(); error.push_str(&ogpu_core::error::to_string(&e)); } }
    unsafe { (*shared).renderers -= 1; }
});

// The native handle of a glyph atlas texture (`kind` 0: the mask atlas, one byte per texel; 1: the colour atlas, RGBA8), for an
// app pass that draws text quads of `pxui_text_quads`. The texture stays the renderer's: do not keep the handle past the frame.
export!(pub unsafe fn pxui_renderer_atlas(renderer:*mut PxuiRenderer, kind:u32, out:*mut pxui_types::Image) -> i32 {
    if renderer.is_null() { return PXUI_ERR_ARGUMENT; }
    let renderer=unsafe { &mut *renderer };
    if out.is_null() || unsafe { (*out).size } < core::mem::size_of::<pxui_types::Image>() as u32 { return renderer.fail(PXUI_ERR_ARGUMENT, "invalid image output"); }
    let (atlas,format)=match kind { 0=>(AtlasKind::Mask,pxui_types::Image::FORMAT_R8), 1=>(AtlasKind::Color,pxui_types::Image::FORMAT_RGBA8), _=>return renderer.fail(PXUI_ERR_ARGUMENT, "unknown atlas kind") };
    let native=match gpu_backend::native_texture(renderer.painter.atlas(atlas)) { Ok(native)=>native, Err(e)=>return renderer.fail_gpu(e) };
    unsafe { *out=pxui_types::Image { size:core::mem::size_of::<pxui_types::Image>() as u32, vk_image_view:native.vk_image_view, mtl_texture:native.mtl_texture, width:native.width, height:native.height, format }; }
    0
});
