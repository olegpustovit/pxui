use ogpu_core::backend::{self as gpu_backend, Device, Queue, Texture};
use ogpu_core::pipeline::{ColorFormat, DepthFormat};
use ogpu_core::types::{Extent, Origin, TextureFormat};
use pxui_core::ui::Ui;
use pxui_render::{AtlasKind, Break, Instance, Painter, View};
use pxui_core::text::AtlasKind as TextKind;

use crate::state::{fail, get, PxuiUi};
use crate::{PXUI_ERR_ARGUMENT, PXUI_ERR_GPU, PXUI_ERR_STATE, PxuiTargetDesc};
use core::ffi::c_void;

pub struct PxuiRenderer {
    painter: Painter,
    queue: Queue,
    device: Device,
    /// wrapped app textures by native handle, with the serial of the frame that last drew them
    textures: std::collections::HashMap<usize, (Texture, u64)>,
    engine: *mut Ui,
    pub(crate) shared: *mut crate::state::Shared,
    format: TextureFormat,
    samples: i64,
    /// the last frame passed to `prepare`, and the last `completed` the app reported
    frame_serial: u64,
    completed: u64,
    atlas_initialized: bool,
    /// the app's command buffer and pass as adopted by the last prepare and draw, pointed at the next ones (`readopt_*`) so a
    /// settled frame allocates nothing; adopted anew when the driver cannot
    prepared: Option<gpu_backend::Commands>,
    drawn: Option<(gpu_backend::Commands, gpu_backend::RenderEncoder)>,
}

/// `native` in the commands kept from the last frame when the driver can point them there, or adopted anew.
fn adopt_again(queue: &Queue, kept: Option<gpu_backend::Commands>, native: ogpu_core::backend::NativeCommands) -> Result<gpu_backend::Commands, ogpu_core::error::Error> {
    match kept {
        Some(commands) if gpu_backend::readopt_commands(&commands, native)? => Ok(commands),
        _ => gpu_backend::adopt_commands(queue, native),
    }
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
    // In a static build this is the app's own copy: its selection (a mock, say) stays.
    if !ogpu_core::impl_::is_selected() {
        #[cfg(target_os = "macos")]
        ogpu_core::impl_::select(ogpu_metal::impl_::implementation());
        #[cfg(any(windows,target_os = "linux"))]
        ogpu_core::impl_::select(ogpu_core::impl_::Impl { backend: ogpu_vulkan::impl_::BACKEND, create_driver: ogpu_vulkan::backend::create, live_handles: ogpu_vulkan::impl_::live_handles });
    }
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
    Ok(PxuiRenderer { painter, queue, device, textures:std::collections::HashMap::new(), engine:ui.engine, shared:ui.shared, format, samples:i64::from(target.samples), frame_serial:0, completed:0, atlas_initialized:false, prepared:None, drawn:None })
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
    let commands=match adopt_again(&renderer.queue,renderer.prepared.take(),native) { Ok(c)=>c, Err(e)=>return renderer.fail_gpu(e) };
    if let Err(e)=gpu_backend::retire(&renderer.queue,completed) { return renderer.fail_gpu(e); }
    renderer.completed=completed;
    // the renderer holds the engine until it returns: a callback it runs (the pass's split) finds no usable `PxuiUi` handle
    let _current=crate::state::Current::set(renderer.shared, core::ptr::null_mut());
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
    // a wrapped texture no frame in flight can still sample (it was last drawn more than 3 frames ago) goes
    renderer.textures.retain(|_,(_,used)| frame-*used<=pxui_render::FRAMES as u64);
    renderer.painter.begin_frame();
    gpu_backend::settle_commands(&commands);
    renderer.prepared=Some(commands);
    0
});

/// What the blur recording callback of one split reaches.
struct Recording<'a> { painter: &'a mut Painter, renderer_device: &'a Device, queue: &'a Queue, serial: u64, native: [*mut c_void; 2], rect: [u32; 4], sigma: f32, saturation: f32, blurred: Option<Texture>, error: Option<ogpu_core::error::Error> }

/// The app has ended its pass: record the blur of `rect` of the target it drew so far into its command buffer.
extern "C" fn record_blur(user: *mut c_void, target: *const pxui_types::Image) -> i32 {
    let rec = unsafe { &mut *user.cast::<Recording>() };
    if target.is_null() || unsafe { (*target).size } < core::mem::size_of::<pxui_types::Image>() as u32 { return PXUI_ERR_ARGUMENT; }
    let t = unsafe { *target };
    let result = (|| {
        let texture = gpu_backend::wrap_texture(rec.renderer_device, ogpu_core::backend::NativeTexture { size: core::mem::size_of::<ogpu_core::backend::NativeTexture>() as u32, vk_image_view: t.vk_image_view, mtl_texture: t.mtl_texture, width: t.width, height: t.height, format: t.format })?;
        let native = ogpu_core::backend::NativeCommands { size: core::mem::size_of::<ogpu_core::backend::NativeCommands>() as u32, vk_command_buffer: rec.native[0], mtl_command_buffer: rec.native[1], frame_serial: rec.serial, inside_pass: 0 };
        let commands = gpu_backend::adopt_commands(rec.queue, native)?;
        let blurred = rec.painter.blur_in_with_saturation(&commands, &texture, rec.rect, rec.sigma, rec.saturation)?.clone();
        rec.blurred = Some(blurred);
        let _ = gpu_backend::destroy_texture(&texture);
        Ok(())
    })();
    match result { Ok(()) => 0, Err(e) => { rec.error = Some(e); PXUI_ERR_GPU } }
}

// Draws layers `pass.first_layer..` (`pass.layer_count` of them, 0 for all) of the last frame into the app's open pass. A layer
// with a backdrop blur asks the app to end the pass (`pass.split`), records the blur and goes on in the pass the app reopens.
export!(pub unsafe fn pxui_renderer_draw(renderer:*mut PxuiRenderer, pass:*const pxui_types::Pass) -> i32 {
    if renderer.is_null() { return PXUI_ERR_ARGUMENT; }
    let renderer=unsafe { &mut *renderer };
    let Some(pass)=read_desc!(pass, pxui_types::Pass) else { return renderer.fail(PXUI_ERR_ARGUMENT, "invalid pass descriptor") };
    if pass.width_px==0 || pass.height_px==0 || !pass.scale.is_finite() || pass.scale<=0.0 { return renderer.fail(PXUI_ERR_ARGUMENT, "invalid pass descriptor"); }
    if renderer.frame_serial==0 { return renderer.fail(PXUI_ERR_STATE, "draw needs a prepared frame"); }
    let size_px=[pass.width_px,pass.height_px];
    let commands_native=ogpu_core::backend::NativeCommands { size:core::mem::size_of::<ogpu_core::backend::NativeCommands>() as u32, vk_command_buffer:pass.vk_command_buffer, mtl_command_buffer:pass.mtl_command_buffer, frame_serial:renderer.frame_serial, inside_pass:1 };
    let native_pass=ogpu_core::backend::NativePass { size:core::mem::size_of::<ogpu_core::backend::NativePass>() as u32, vk_command_buffer:pass.vk_command_buffer, mtl_render_encoder:pass.mtl_render_encoder, width_px:pass.width_px, height_px:pass.height_px, scale:pass.scale };
    let (kept_commands,kept_encoder)=renderer.drawn.take().unzip();
    let commands=match adopt_again(&renderer.queue,kept_commands,commands_native) { Ok(c)=>c, Err(e)=>return renderer.fail_gpu(e) };
    let encoder=match kept_encoder.map(|e| gpu_backend::readopt_render_encoder(&commands,&e,native_pass).map(|again| again.then_some(e))) {
        Some(Ok(Some(encoder)))=>encoder,
        Some(Err(e))=>return renderer.fail_gpu(e),
        _=>match gpu_backend::adopt_render_encoder(&commands,native_pass,renderer.format,renderer.samples) { Ok(e)=>e, Err(e)=>return renderer.fail_gpu(e) },
    };
    // the renderer holds the engine until it returns: a callback it runs (the pass's split) finds no usable `PxuiUi` handle
    let _current=crate::state::Current::set(renderer.shared, core::ptr::null_mut());
    let ui=unsafe { &mut *renderer.engine };
    for (index,v) in ui.output().views {
        if let Err(e)=renderer.painter.set_view(*index,View { offset:v.offset,opacity:v.opacity,clip:v.clip,scale:v.scale,transform:v.transform,pivot:v.pivot }) { return renderer.fail_gpu(e); }
    }
    let layers=ui.output().draw.len();
    let first=(pass.first_layer as usize).min(layers);
    let end=if pass.layer_count==0 { layers } else { (first+pass.layer_count as usize).min(layers) };
    // the pass the next layer draws into: the one the app opened, or the last one its split reopened
    let mut encoder=encoder;
    for index in first..end {
        let layer=&ui.output().draw[index];
        // Adopt foreign textures before building `Break` references. Keeping those
        // references alive while inserting another texture would borrow the map
        // immutably across a mutation.
        for b in &layer.breaks {
            if b.image==pxui_core::draw::ATLAS { continue; }
            let Some(image)=layer.images.get(b.image as usize) else { continue; };
            if image.handle.downcast_ref::<Texture>().is_some() { continue; }
            let Some(key)=crate::image::texture_key(image) else { continue; };
            match renderer.textures.get_mut(&key) {
                Some(entry)=>entry.1=renderer.frame_serial,
                None=>{
                    let texture=match crate::image::adopt_texture(&renderer.device,image) { Ok(texture)=>texture, Err(())=>return renderer.fail(PXUI_ERR_GPU, "an app image could not be wrapped") };
                    renderer.textures.insert(key,(texture,renderer.frame_serial));
                }
            }
        }
        // the layer's breaks live on the stack (the heap past 64): a settled frame allocates nothing here
        let none=Break { at_instance:0, texture:None };
        let mut stack=[none;64];
        let mut heap: Vec<Break>=Vec::new();
        let slots: &mut [Break]=if layer.breaks.len()<=64 { &mut stack[..layer.breaks.len()] } else { heap.resize(layer.breaks.len(),none); &mut heap[..] };
        for (slot,b) in slots.iter_mut().zip(&layer.breaks) {
            let texture=if b.image==pxui_core::draw::ATLAS { None } else if let Some(image)=layer.images.get(b.image as usize) {
                if let Some(texture)=image.handle.downcast_ref::<Texture>() { Some(texture) }
                else { crate::image::texture_key(image).and_then(|key|renderer.textures.get(&key).map(|e|&e.0)) }
            } else { None };
            *slot=Break { at_instance:b.at_instance, texture };
        }
        let breaks: &[Break]=slots;
        let blurs: Vec<&pxui_core::ui::BlurOut>=ui.output().blurs.iter().filter(|b|b.layer==index && (b.at_instance as usize)<layer.instances.len()).collect();
        let result=if blurs.is_empty() {
            renderer.painter.draw_retained(&encoder,size_px,pass.scale,layer.id,&layer.instances,&layer.dirty,breaks)
        } else {
            let Some(split)=pass.split else { return renderer.fail(PXUI_ERR_STATE, "a layer with a backdrop blur needs the pass's split callback") };
            // a blur splits the layer where its view's run starts: a break there that keeps the texture in force
            // (a layer with a blur is the rare case: it may allocate)
            let mut breaks=breaks.to_vec();
            let mut splits: Vec<u32>=blurs.iter().map(|b|b.at_instance).collect();
            splits.sort_unstable();
            splits.dedup();
            for &at in &splits {
                if !breaks.iter().any(|b|b.at_instance==at) {
                    let i=breaks.partition_point(|b|b.at_instance<at);
                    breaks.insert(i,Break { at_instance:at, texture:i.checked_sub(1).and_then(|j|breaks[j].texture) });
                }
            }
            let (queue,device,format,samples,serial)=(&renderer.queue,&renderer.device,renderer.format,renderer.samples,renderer.frame_serial);
            let (native_commands,scale)=([pass.vk_command_buffer,pass.mtl_command_buffer],pass.scale);
            let mut reopened: Option<ogpu_core::backend::RenderEncoder>=None;
            let current=encoder.clone();
            let outcome=renderer.painter.draw_retained_split(&current,size_px,scale,layer.id,&layer.instances,&layer.dirty,&breaks,&splits,&mut |painter,at| {
                let mut latest=None;
                for b in blurs.iter().filter(|b|b.at_instance==at) {
                    let [w,h]=size_px;
                    let (x0,y0)=((b.rect.x*scale).floor().max(0.0) as u32,(b.rect.y*scale).floor().max(0.0) as u32);
                    let (x1,y1)=(((b.rect.x+b.rect.w)*scale).ceil().clamp(0.0,w as f32) as u32,((b.rect.y+b.rect.h)*scale).ceil().clamp(0.0,h as f32) as u32);
                    if x0>=x1 || y0>=y1 || !(b.sigma>0.0) { continue; }
                    // this copy of the pass ends here; the app ends its own, records the blur and reopens one
                    let open=latest.clone().or_else(||reopened.clone()).unwrap_or_else(||current.clone());
                    gpu_backend::end_render(&open)?;
                    let mut rec=Recording { painter, renderer_device:device, queue, serial, native:native_commands, rect:[x0,y0,x1,y1], sigma:b.sigma*scale, saturation:b.saturation, blurred:None, error:None };
                    let mut next=pass;
                    let rc=split(pass.split_user,record_blur,(&mut rec as *mut Recording).cast(),&mut next);
                    if let Some(e)=rec.error.take() { return Err(e); }
                    if rc!=0 || rec.blurred.is_none() { return Err(ogpu_core::error::make("Pxui.draw",ogpu_core::error::ErrorKind::InvalidState,"the app's pass split failed")); }
                    let blurred=rec.blurred.take();
                    let commands_native=ogpu_core::backend::NativeCommands { size:core::mem::size_of::<ogpu_core::backend::NativeCommands>() as u32, vk_command_buffer:next.vk_command_buffer, mtl_command_buffer:next.mtl_command_buffer, frame_serial:serial, inside_pass:1 };
                    let native_next=ogpu_core::backend::NativePass { size:core::mem::size_of::<ogpu_core::backend::NativePass>() as u32, vk_command_buffer:next.vk_command_buffer, mtl_render_encoder:next.mtl_render_encoder, width_px:next.width_px, height_px:next.height_px, scale:next.scale };
                    let commands=gpu_backend::adopt_commands(queue,commands_native)?;
                    let encoder=gpu_backend::adopt_render_encoder(&commands,native_next,format,samples)?;
                    let rect=[x0,y0,x1,y1].map(|v|v as f32/scale);
                    let (alpha,radius)=((255.0*b.opacity.clamp(0.0,1.0)).round() as u8,b.radii.iter().fold(0.0f32,|a,r|a.max(*r)));
                    let clip=[b.rect.x.floor(),b.rect.y.floor(),(b.rect.x+b.rect.w).ceil(),(b.rect.y+b.rect.h).ceil()].map(|v|v as i16);
                    let image=Instance::image(rect,[0.0,0.0,1.0,1.0],[alpha;4]).clip(clip,radius);
                    painter.draw(&encoder,size_px,scale,&[image],&[Break { at_instance:0, texture:blurred.as_ref() }])?;
                    latest=Some(encoder);
                }
                if latest.is_some() { reopened=latest.clone(); }
                Ok(latest)
            });
            if let Some(next)=reopened { encoder=next; }
            outcome
        };
        if let Err(e)=result { return renderer.fail_gpu(e); }
        ui.drawn(index);
    }
    // ends only this copy's view of the pass; the app's encoder must `invalidate_state` before drawing more
    if let Err(e)=gpu_backend::end_render(&encoder) { return renderer.fail_gpu(e); }
    gpu_backend::settle_commands(&commands);
    renderer.drawn=Some((commands,encoder));
    0
});

export!(bare pub unsafe fn pxui_renderer_destroy(renderer:*mut PxuiRenderer) {
    if renderer.is_null() { return; }
    let PxuiRenderer { painter, queue, device, textures, shared, frame_serial, prepared, drawn, .. } = *unsafe { Box::from_raw(renderer) };
    // a painter or texture that fails to go stays a child of the device, which then refuses to go: reported below
    drop((prepared, drawn));
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

// The draw calls this renderer's painter has issued so far, for tests that pin how many a frame takes.
export!(pub unsafe fn pxui_renderer_draw_calls(renderer:*mut PxuiRenderer) -> u64 {
    unsafe { renderer.as_ref() }.map_or(0, |r| r.painter.draw_calls())
});
