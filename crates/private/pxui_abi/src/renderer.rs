//! Renderer status refuses UI frame callbacks and another renderer's split callback on the same UI.
//! Shared::current being null fences engine ownership; in_frame alone is insufficient. Foreign-
//! image/blur-source wrappers use full native handles, extent and format identity and explicitly
//! destroy retired wrappers.
//!
use ogpu_core::backend::{self as gpu_backend, Device, Queue, Texture};
use ogpu_core::pipeline::{ColorFormat, DepthFormat};
use ogpu_core::types::{Extent, Origin, TextureFormat};
use pxui_core::ui::{BlurOut, Ui, expand_blur_damage};
use pxui_render::{AtlasKind, BLUR_CACHE_SLOTS, Blurred, Break, Instance, Painter, View};
use pxui_core::text::AtlasKind as TextKind;

use crate::state::{fail, fail_code, get, Handle, PxuiUi};
use crate::{PXUI_ERR_ARGUMENT, PXUI_ERR_GPU, PXUI_ERR_STATE, PxuiTargetDesc};
use core::ffi::c_void;

struct CachedTexture { texture: Texture, used: u64, image: Option<std::rc::Weak<dyn std::any::Any>> }
impl Drop for CachedTexture {
    fn drop(&mut self) { let _=gpu_backend::destroy_texture(&self.texture); }
}

struct CachedBlur {
    view: BlurOut,
    result: Blurred,
    extent: [u32; 2],
    scale: f32,
    written: u64,
}

pub struct PxuiRenderer {
    painter: Painter,
    queue: Queue,
    device: Device,
    /// wrapped app textures by native handle, with the serial of the frame that last drew them
    textures: std::collections::HashMap<crate::image::TextureKey, CachedTexture>,
    blur_sources: std::collections::HashMap<crate::image::TextureKey, CachedTexture>,
    engine: *mut Ui,
    pub(crate) shared: *mut crate::state::Shared,
    format: TextureFormat,
    samples: i64,
    /// the last frame passed to `prepare`, and the last `completed` the app reported
    frame_serial: u64,
    completed: u64,
    /// The draw revision whose view table the painter has copied.
    view_revision: Option<i64>,
    atlas_initialized: [bool;4],
    /// the app's command buffer and pass as adopted by the last prepare and draw, pointed at the next ones (`readopt_*`) so a
    /// settled frame allocates nothing; adopted anew when the driver cannot
    prepared: Option<gpu_backend::Commands>,
    drawn: Option<(gpu_backend::Commands, gpu_backend::RenderEncoder)>,
    split_points: Vec<u32>,
    no_cull: bool,
    breaks: Vec<Break>,
    blur_commands: Option<gpu_backend::Commands>,
    blur_reuse: bool,
    blur_slots: usize,
    blur_cache: [Option<CachedBlur>; BLUR_CACHE_SLOTS],
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
impl GpuLease {
    fn new(gpu: &pxui_types::Gpu) -> Self {
        Self { token: gpu.owner_token, release: gpu.release_owner, active: true }
    }
}

impl Drop for GpuLease {
    fn drop(&mut self) {
        if self.active {
            if let Some(release)=self.release { release(self.token); }
        }
    }
}

/// A renderer on the app's device, or why it could not be made. The lease on the app's device is released on every refusal
/// until the adopted driver owns it.
unsafe fn make(ui: &mut PxuiUi, gpu: pxui_types::Gpu, target: PxuiTargetDesc, mut lease: GpuLease) -> Result<PxuiRenderer, (i32, String)> {
    if target.samples == 0 || target.reserved_tail & !PxuiTargetDesc::BACKDROP_UI_DAMAGE != 0 { return Err((PXUI_ERR_ARGUMENT, "invalid target descriptor".into())); }
    let (format,color) = match target.format { 1=>(TextureFormat::Rgba8Unorm,ColorFormat::Rgba8Unorm), 2=>(TextureFormat::Bgra8Unorm,ColorFormat::Bgra8Unorm), _=>return Err((PXUI_ERR_ARGUMENT, "unknown target format".into())) };
    let native=ogpu_core::backend::NativeDevice { size:gpu.size, backend:gpu.backend, instance:gpu.instance, physical_device:gpu.physical_device, device:gpu.device, queue:gpu.queue, queue_family:gpu.queue_family, api_version:gpu.api_version, features:gpu.features, get_instance_proc_addr:gpu.get_instance_proc_addr, pipeline_cache:gpu.pipeline_cache, mtl_device:gpu.mtl_device, wl_display:gpu.wl_display, wl_surface:gpu.wl_surface, metal_layer:gpu.metal_layer, hwnd:gpu.hwnd, owner_token:gpu.owner_token, release_owner:gpu.release_owner };
    // This copy of OGPU (the library's own) has not been told which backend it drives: the app selected one in its copy.
    // In a static build this is the app's own copy: its selection (a mock, say) stays.
    if !ogpu_core::impl_::is_selected() {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        ogpu_core::impl_::select(ogpu_metal::impl_::implementation());
        #[cfg(any(windows,target_os = "linux"))]
        ogpu_core::impl_::select(ogpu_core::impl_::Impl { backend: ogpu_vulkan::impl_::BACKEND, create_driver: ogpu_vulkan::backend::create, live_handles: ogpu_vulkan::impl_::live_handles });
    }
    lease.active=false;
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    let driver=ogpu_metal::adopt(native);
    #[cfg(any(windows,target_os = "linux"))]
    let driver=ogpu_vulkan::adopt(native);
    let describe = |error: ogpu_core::error::Error| {
        let status = match error.kind {
            ogpu_core::error::ErrorKind::InvalidArgument => PXUI_ERR_ARGUMENT,
            _ => PXUI_ERR_GPU,
        };
        (status, ogpu_core::error::to_string(&error))
    };
    let device=gpu_backend::create_device(driver.map_err(describe)?).map_err(describe)?;
    let queue=match gpu_backend::create_queue(&device) { Ok(q)=>q, Err(e)=>{ let _=gpu_backend::destroy_device(&device); return Err(describe(e)); } };
    let (mask,color_size)=unsafe { let text=(&mut *ui.engine).text_mut(); (text.atlas_size(TextKind::Mask),text.atlas_size(TextKind::Color)) };
    if (target.mask_width!=0 && target.mask_width!=mask.0) || (target.mask_height!=0 && target.mask_height!=mask.1) || (target.color_width!=0 && target.color_width!=color_size.0) || (target.color_height!=0 && target.color_height!=color_size.1) {
        let _=gpu_backend::destroy_queue(&queue); let _=gpu_backend::destroy_device(&device); return Err((PXUI_ERR_ARGUMENT, "the target's atlas sizes differ from the Ui's".into()));
    }
    let painter=match Painter::new(&device,color,DepthFormat::NoDepth,i64::from(target.samples),mask,color_size) {
        Ok(p)=>p,
        Err(e)=>{ let _=gpu_backend::destroy_queue(&queue); let _=gpu_backend::destroy_device(&device); return Err(describe(e)); }
    };
    let blur_reuse=target.reserved_tail & PxuiTargetDesc::BACKDROP_UI_DAMAGE != 0
        && std::env::var_os("PXUI_NO_BLUR_REUSE").as_deref() != Some(std::ffi::OsStr::new("1"));
    Ok(PxuiRenderer { painter, queue, device, textures:std::collections::HashMap::new(), blur_sources:std::collections::HashMap::new(), engine:ui.engine, shared:ui.shared, format, samples:i64::from(target.samples), frame_serial:0, view_revision:None, completed:0, atlas_initialized:[false;4], prepared:None, drawn:None, split_points:Vec::new(), breaks:Vec::new(), blur_commands:None, no_cull:std::env::var_os("PXUI_NO_CULL").as_deref()==Some(std::ffi::OsStr::new("1")), blur_reuse, blur_slots:0, blur_cache:Default::default() })
}

export!(bare pub unsafe fn pxui_renderer_create(ui:*mut PxuiUi, gpu:*const pxui_types::Gpu, target:*const PxuiTargetDesc, out:*mut *mut PxuiRenderer) -> i32 {
    let shared = ui.shared();
    unsafe { crate::state::note_call(shared); }
    // A complete GPU record transfers its lease before the Ui/target checks. The legacy call keeps its original order.
    export!(@run payload => { unsafe { crate::state::panicked(shared, payload, false); } crate::PXUI_ERR_PANIC }, {
        if out.is_null() { return unsafe { fail(ui, "null renderer output") }; }
        let Some(gpu) = (if gpu.is_null() { None } else { read_desc!(gpu, pxui_types::Gpu) }) else {
            return unsafe { fail(ui, "invalid gpu descriptor") };
        };
        let lease = GpuLease::new(&gpu);
        if !ui.usable(shared) { return unsafe { crate::state::refuse(shared) }; }
        let state = match unsafe { renderer_owner(ui, !target.is_null()) } {
            Ok(state) => state,
            Err(code) => return code,
        };
        let Some(target) = read_desc!(target, PxuiTargetDesc) else { return unsafe { fail(ui, "invalid target descriptor") }; };
        match unsafe { create_renderer(ui, state, gpu, target, lease) } {
            Ok(renderer) => { unsafe { out.write(renderer); } 0 }
            Err(code) => code,
        }
    })
});

unsafe fn renderer_owner<'a>(ui:*mut PxuiUi, descriptors:bool) -> Result<&'a mut PxuiUi, i32> {
    let Some(state)=(unsafe { get(ui) }) else { return Err(PXUI_ERR_ARGUMENT); };
    // a borrowed callback handle's engine is only live for its frame: a renderer needs the owner, outside a frame
    if !state.owner || state.shared().in_frame || !descriptors {
        let code = if !state.owner || state.shared().in_frame { PXUI_ERR_STATE } else { PXUI_ERR_ARGUMENT };
        return Err(unsafe { fail_code(ui, "a renderer needs the owner Ui outside a frame, a gpu and a target", code) });
    }
    Ok(state)
}

unsafe fn create_renderer(ui:*mut PxuiUi, state:&mut PxuiUi, gpu:pxui_types::Gpu, target:PxuiTargetDesc, lease:GpuLease) -> Result<*mut PxuiRenderer, i32> {
    match unsafe { make(state, gpu, target, lease) } {
        Ok(renderer)=>{ state.shared().renderers += 1; Ok(Box::into_raw(Box::new(renderer))) }
        Err((code, message))=>Err(unsafe { fail_code(ui, &message, code) }),
    }
}

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
    // the renderer holds the engine until it returns: a callback it runs (the pass's split) finds no usable or building `PxuiUi` handle
    let _current=crate::state::NoCurrent::set(renderer.shared);
    let ui=unsafe { &mut *renderer.engine };
    let mut failed=None;
    let mut copied=[false;4];
    for (kind,atlas) in [(TextKind::Mask,AtlasKind::Mask),(TextKind::Color,AtlasKind::Color),(TextKind::Mask2,AtlasKind::Mask2),(TextKind::Color2,AtlasKind::Color2)] {
        let index=kind as usize;
        if !ui.text_mut().atlas_active(kind) || renderer.atlas_initialized[index] { continue; }
        let (w,h)=ui.text_mut().atlas_size(kind);
        if let Err(error)=renderer.painter.prepare_atlas(atlas,(w,h)) {
            renderer.atlas_initialized=[false;4];
            return renderer.fail_gpu(error);
        }
        let texture=renderer.painter.atlas(atlas).expect("prepared atlas");
        let bytes=ui.text_mut().atlas_pixels(kind);
        let texel=if kind.is_mask() { 1 } else { 4 };
        if let Err(error)=gpu_backend::write_texture_on(&commands,texture,None,Origin{x:0,y:0,z:0},Extent{width:w as i64,height:h as i64,depth:1},(w as i64)*texel,bytes) {
            renderer.atlas_initialized=[false;4];
            return renderer.fail_gpu(error);
        }
        renderer.atlas_initialized[index]=true;
        copied[index]=true;
    }
    ui.text_mut().drain_uploads(&mut |kind,x,y,w,h,bytes,stride| {
        if copied[kind as usize] { return; }
        let atlas=match kind { TextKind::Mask=>AtlasKind::Mask,TextKind::Color=>AtlasKind::Color,TextKind::Mask2=>AtlasKind::Mask2,TextKind::Color2=>AtlasKind::Color2 };
        let texture=renderer.painter.atlas(atlas).expect("active atlas was prepared");
        let result=gpu_backend::write_texture_on(&commands,texture,None,Origin{x:x as i64,y:y as i64,z:0},Extent{width:w as i64,height:h as i64,depth:1},stride as i64,bytes);
        if let Err(error)=result { if failed.is_none() { failed=Some(error); } }
    });
    if let Some(e)=failed { renderer.atlas_initialized=[false;4]; return renderer.fail_gpu(e); }
    renderer.frame_serial=frame;
    // Keep live image identities; release abandoned wrappers only after their actual GPU completion.
    renderer.textures.retain(|_,entry| entry.used>completed || entry.image.as_ref().is_some_and(|image|image.strong_count()!=0));
    // A cached backdrop can skip many splits. Keep its sole source wrapper warm for the next miss.
    if renderer.blur_sources.len()!=1 || !renderer.blur_cache.iter().any(Option::is_some) {
        renderer.blur_sources.retain(|_,entry| entry.used>completed || frame-entry.used<=pxui_render::FRAMES as u64);
    }
    renderer.painter.begin_frame();
    if renderer.blur_reuse {
        renderer.blur_slots=renderer.painter.prepare_blur_cache(ui.output().blurs.len());
    }
    gpu_backend::settle_commands(&commands);
    renderer.prepared=Some(commands);
    0
});

// The host forgets an evicted native resource before destroying/reusing its handle.
export!(pub unsafe fn pxui_renderer_forget_image(renderer:*mut PxuiRenderer, raw:*const pxui_types::Image, completed:u64) -> i32 {
    if renderer.is_null() { return PXUI_ERR_ARGUMENT; }
    let renderer=unsafe { &mut *renderer };
    let Some(raw)=read_desc!(raw, pxui_types::Image) else { return renderer.fail(PXUI_ERR_ARGUMENT,"invalid image descriptor") };
    if !crate::image::valid(raw) { return renderer.fail(PXUI_ERR_ARGUMENT,"invalid image descriptor"); }
    if completed<renderer.completed { return renderer.fail(PXUI_ERR_ARGUMENT,"completed must not go back"); }
    let key=crate::image::image_key(&raw);
    if renderer.textures.get(&key).into_iter().chain(renderer.blur_sources.get(&key)).any(|entry|entry.used>completed) {
        return renderer.fail(PXUI_ERR_STATE,"image still has an incomplete GPU frame");
    }
    if let Err(error)=gpu_backend::retire(&renderer.queue,completed) { return renderer.fail_gpu(error); }
    renderer.completed=completed;
    let released:Result<(),ogpu_core::error::Error>=(|| {
        for cache in [&mut renderer.textures,&mut renderer.blur_sources] {
            if let Some(entry)=cache.get(&key) { gpu_backend::destroy_texture(&entry.texture)?; }
            cache.remove(&key);
        }
        Ok(())
    })();
    if let Err(error)=released { return renderer.fail_gpu(error); }
    unsafe { &mut *renderer.shared }.images.remove(&crate::image::image_key(&raw));
    0
});

// Allocate blur scratch storage between frames, before recording commands for the resized target.
export!(pub unsafe fn pxui_renderer_resize(renderer:*mut PxuiRenderer, width:u32, height:u32) -> i32 {
    if renderer.is_null() { return PXUI_ERR_ARGUMENT; }
    let renderer=unsafe { &mut *renderer };
    if width==0 || height==0 { return renderer.fail(PXUI_ERR_ARGUMENT, "renderer extent must be nonzero"); }
    // the device's own limit: the blur targets are half this extent, so a larger one is refused here, never allocated
    let maximum=ogpu_core::backend::capabilities(&renderer.device).limits.max_texture_dimension_2d;
    if i64::from(width)>maximum || i64::from(height)>maximum { return renderer.fail(PXUI_ERR_ARGUMENT, "renderer extent exceeds the device's maximum texture dimension"); }
    renderer.blur_cache=Default::default();
    renderer.blur_slots=0;
    match renderer.painter.prepare_blur([width,height]) { Ok(())=>0, Err(e)=>renderer.fail_gpu(e) }
});

/// What the blur recording callback of one split reaches.
/// The pointers borrow from the split call that builds it and outlive neither it nor the callback.
struct Recording { painter: *mut Painter, renderer_device: *const Device, queue: *const Queue, commands: *mut Option<gpu_backend::Commands>, sources: *mut std::collections::HashMap<crate::image::TextureKey, CachedTexture>, serial: u64, native: [*mut c_void; 2], rect: [u32; 4], sigma: f32, saturation: f32, slot: Option<usize>, blurred: Option<Blurred>, error: Option<ogpu_core::error::Error> }

/// The app has ended its pass: record the blur of `rect` of the target it drew so far into its command buffer.
extern "C" fn record_blur(user: *mut c_void, target: *const pxui_types::Image) -> i32 {
    let rec = unsafe { &mut *user.cast::<Recording>() };
    if target.is_null() || unsafe { (*target).size } < core::mem::size_of::<pxui_types::Image>() as u32 { return PXUI_ERR_ARGUMENT; }
    let t = unsafe { *target };
    let result = (|| {
        let entry=match unsafe { &mut *rec.sources }.entry(crate::image::image_key(&t)) {
            std::collections::hash_map::Entry::Occupied(entry)=>entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry)=>{
                let texture=gpu_backend::wrap_texture(unsafe { &*rec.renderer_device }, ogpu_core::backend::NativeTexture { size: core::mem::size_of::<ogpu_core::backend::NativeTexture>() as u32, vk_image_view: t.vk_image_view, mtl_texture: t.mtl_texture, width: t.width, height: t.height, format: t.format })?;
                entry.insert(CachedTexture { texture, used:rec.serial, image:None })
            }
        };
        entry.used=rec.serial;
        let native = ogpu_core::backend::NativeCommands { size: core::mem::size_of::<ogpu_core::backend::NativeCommands>() as u32, vk_command_buffer: rec.native[0], mtl_command_buffer: rec.native[1], frame_serial: rec.serial, inside_pass: 0 };
        let commands=adopt_again(unsafe { &*rec.queue },unsafe { &mut *rec.commands }.take(),native)?;
        let blurred=unsafe { &mut *rec.painter }.blur_in_slot(&commands,&entry.texture,rec.rect,rec.sigma,rec.saturation,rec.slot);
        gpu_backend::settle_commands(&commands);
        rec.blurred=Some(blurred?);
        unsafe { *rec.commands=Some(commands); }
        Ok(())
    })();
    match result { Ok(()) => 0, Err(e) => { rec.error = Some(e); PXUI_ERR_GPU } }
}

// A retained solid/text run can inherit a texture break without sampling that image.
fn samples_image(instances:&[Instance],breaks:&[pxui_core::draw::Break],index:usize)->bool {
    let start=(breaks[index].at_instance as usize).min(instances.len());
    let end=breaks.get(index+1).map_or(instances.len(),|b|(b.at_instance as usize).min(instances.len()));
    instances[start..end].iter().any(|instance|instance.kind==pxui_render::kind::IMAGE)
}
#[cfg(test)]
mod image_binding_tests {
    #[test]
    fn inherited_and_trailing_breaks_without_image_instances_do_not_sample_a_resource() {
        let mut instances=[pxui_render::Instance::DEFAULT;6];
        instances[3].kind=pxui_render::kind::IMAGE;
        let breaks=[0,2,4,6].map(|at_instance|pxui_core::draw::Break{at_instance,image:0});
        assert!(!super::samples_image(&instances,&breaks,0));
        assert!(super::samples_image(&instances,&breaks,1));
        assert!(!super::samples_image(&instances,&breaks,2));
        assert!(!super::samples_image(&instances,&breaks,3));
    }
}

// Draws layers `pass.first_layer..` (`pass.layer_count` of them, 0 for all) of the last frame into the app's open pass. A layer
// with a backdrop blur asks the app to end the pass (`pass.split`), records the blur and goes on in the pass the app reopens.
export!(pub unsafe fn pxui_renderer_draw(renderer:*mut PxuiRenderer, pass:*const pxui_types::Pass) -> i32 {
    unsafe { draw(renderer, pass, &[]) }
});

export!(pub unsafe fn pxui_renderer_draw_damage(renderer:*mut PxuiRenderer, pass:*const pxui_types::Pass, x:u32, y:u32, width:u32, height:u32) -> i32 {
    unsafe { draw(renderer, pass, &[[x,y,width,height]]) }
});

export!(pub unsafe fn pxui_renderer_draw_damage_rects(renderer:*mut PxuiRenderer, pass:*const pxui_types::Pass, rects:*const u32, count:u32) -> i32 {
    if renderer.is_null() { return PXUI_ERR_ARGUMENT; }
    if !(2..=8).contains(&count) || rects.is_null() || !rects.is_aligned() {
        return unsafe { &mut *renderer }.fail(PXUI_ERR_ARGUMENT, "expected two to eight damage rectangles");
    }
    // SAFETY: the ABI caller provides `count` contiguous four-word records until this call returns.
    unsafe { draw(renderer, pass, core::slice::from_raw_parts(rects.cast::<[u32;4]>(), count as usize)) }
});

unsafe fn draw(renderer:*mut PxuiRenderer, pass:*const pxui_types::Pass, scissors:&[[u32;4]]) -> i32 {
    const NEED_SPLIT: &str = "a layer with a backdrop blur needs the pass's split callback";
    if renderer.is_null() { return PXUI_ERR_ARGUMENT; }
    let renderer=unsafe { &mut *renderer };
    let Some(pass)=read_desc!(pass, pxui_types::Pass) else { return renderer.fail(PXUI_ERR_ARGUMENT, "invalid pass descriptor") };
    if pass.width_px==0 || pass.height_px==0 || !pass.scale.is_finite() || pass.scale<=0.0 { return renderer.fail(PXUI_ERR_ARGUMENT, "invalid pass descriptor"); }
    if scissors.iter().any(|&[x,y,w,h]| w==0 || h==0 || u64::from(x)+u64::from(w)>u64::from(pass.width_px) || u64::from(y)+u64::from(h)>u64::from(pass.height_px)) {
        return renderer.fail(PXUI_ERR_ARGUMENT, "scissor must be nonempty and inside the target");
    }
    if renderer.frame_serial==0 { return renderer.fail(PXUI_ERR_STATE, "draw needs a prepared frame"); }
    let size_px=[pass.width_px,pass.height_px];
    let commands_native=ogpu_core::backend::NativeCommands { size:core::mem::size_of::<ogpu_core::backend::NativeCommands>() as u32, vk_command_buffer:pass.vk_command_buffer, mtl_command_buffer:pass.mtl_command_buffer, frame_serial:renderer.frame_serial, inside_pass:1 };
    let native_pass=ogpu_core::backend::NativePass { size:core::mem::size_of::<ogpu_core::backend::NativePass>() as u32, vk_command_buffer:pass.vk_command_buffer, mtl_render_encoder:pass.mtl_render_encoder, width_px:pass.width_px, height_px:pass.height_px, scale:pass.scale };
    let (kept_commands,kept_encoder)=renderer.drawn.take().unzip();
    let mut commands=match adopt_again(&renderer.queue,kept_commands,commands_native) { Ok(c)=>c, Err(e)=>return renderer.fail_gpu(e) };
    let encoder=match kept_encoder.map(|e| gpu_backend::readopt_render_encoder(&commands,&e,native_pass).map(|again| again.then_some(e))) {
        Some(Ok(Some(encoder)))=>encoder,
        Some(Err(e))=>return renderer.fail_gpu(e),
        _=>match gpu_backend::adopt_render_encoder(&commands,native_pass,renderer.format,renderer.samples) { Ok(e)=>e, Err(e)=>return renderer.fail_gpu(e) },
    };
    // the renderer holds the engine until it returns: a callback it runs (the pass's split) finds no usable or building `PxuiUi` handle
    let _current=crate::state::NoCurrent::set(renderer.shared);
    let ui=unsafe { &mut *renderer.engine };
    let revision=ui.draw_revision();
    if renderer.view_revision!=Some(revision) {
        for (index,v) in ui.output().views {
            if let Err(e)=renderer.painter.set_view(*index,View { offset:v.offset,opacity:v.opacity,clip:v.clip,scale:v.scale,transform:v.transform,pivot:v.pivot }) { return renderer.fail_gpu(e); }
        }
        renderer.view_revision=Some(revision);
    }
    let damage=(scissors.len()>1).then(|| pxui_render::DamageClip::new(scissors));
    let damage=damage.as_ref();
    // Backdrop capture reads outside the scissor and can split the pass.
    let cull=!renderer.no_cull && !scissors.is_empty()
        && scissors!=[[0,0,pass.width_px,pass.height_px]] && ui.output().blurs.is_empty();
    let layers=ui.output().draw.len();
    let first=(pass.first_layer as usize).min(layers);
    let end=if pass.layer_count==0 { layers } else { (first+pass.layer_count as usize).min(layers) };
    let reuse = renderer.blur_slots != 0 && first == 0 && end == layers;
    if renderer.blur_reuse && !reuse {
        renderer.blur_cache=Default::default();
    } else if reuse && renderer.blur_cache.iter().any(Option::is_some) {
        let out=ui.output();
        let mut damage=*out.damage;
        let viewport=pxui_types::geom::Rect::new(0.0,0.0,size_px[0] as f32/pass.scale,size_px[1] as f32/pass.scale);
        // The incoming list can be rounded at the Ui density. Recheck at the target scale before
        // acknowledging any layer, including dependent blurs whose sources contain another blur.
        expand_blur_damage(&mut damage,out.blurs,viewport,pass.scale);
        for (slot, entry) in renderer.blur_cache.iter_mut().enumerate() {
            if entry.as_ref().is_some_and(|cached| {
                damage.is_full() || cached.extent != size_px || cached.scale != pass.scale
                    || out.blurs.get(slot) != Some(&cached.view)
                    || damage.rects().iter().any(|r| {
                        let overlap=r.intersect(cached.view.footprint(pass.scale));
                        overlap.w > 0.0 && overlap.h > 0.0
                    })
            }) {
                *entry=None;
            }
        }
    }
    // the pass the next layer draws into: the one the app opened, or the last one its split reopened
    let mut encoder=encoder;
    let blurred=|ui:&Ui,index:usize| ui.output().blurs.iter().any(|b| b.layer==index && (b.at_instance as usize)<ui.output().draw[index].instances.len());
    let mut index=first;
    while index<end {
        // A host drawing layers in order into one pass draws consecutive layers without a backdrop blur as one batch (one draw
        // call while their textures agree); a layer with a blur goes alone, since its pass splits.
        let batch_end=if blurred(ui,index) { index+1 } else { (index+1..end).find(|&i| blurred(ui,i)).unwrap_or(end) };
        let mut parts: [(u32,&[Instance],&[(u32,u32)]); pxui_types::MAX_LAYERS]=[(0,&[],&[]); pxui_types::MAX_LAYERS];
        renderer.breaks.clear();
        renderer.split_points.clear();
        // the texture the batch's last segment samples is the colour atlas
        let mut atlas_last=true;
        let mut offset=0u32;
        for (part,index) in (index..batch_end).enumerate() {
            let layer=&ui.output().draw[index];
            parts[part]=(layer.id,&layer.instances,&layer.dirty);
            // Cache foreign wrappers before building this layer's texture bindings.
            for (index,b) in layer.breaks.iter().enumerate() {
                if b.image==pxui_core::draw::ATLAS || !samples_image(&layer.instances,&layer.breaks,index) { continue; }
                let Some(image)=layer.images.get(b.image as usize) else { continue; };
                if image.handle.downcast_ref::<Texture>().is_some() { continue; }
                let Some(key)=crate::image::texture_key(image) else { continue; };
                match renderer.textures.get_mut(&key) {
                    Some(entry)=>entry.used=renderer.frame_serial,
                    None=>{
                        let texture=match crate::image::adopt_texture(&renderer.device,image) { Ok(texture)=>texture, Err(())=>return renderer.fail(PXUI_ERR_GPU, "an app image could not be wrapped") };
                        renderer.textures.insert(key,CachedTexture { texture, used:renderer.frame_serial, image:Some(std::rc::Rc::downgrade(&image.handle)) });
                    }
                }
            }
            renderer.split_points.extend(ui.output().blurs.iter().filter(|b| b.layer==index && (b.at_instance as usize)<layer.instances.len()).map(|b| b.at_instance));
            renderer.split_points.sort_unstable();
            renderer.split_points.dedup();
            renderer.breaks.reserve(layer.breaks.len()+renderer.split_points.len()+1);
            // a later layer starts on the colour atlas: it needs a break of its own only when the batch's last segment samples an image
            if offset>0 && !atlas_last && layer.breaks.first().map_or(true, |b| b.at_instance>0) { renderer.breaks.push(Break { at_instance:offset, texture:None }); atlas_last=true; }
            for (index,b) in layer.breaks.iter().enumerate() {
                let texture=if b.image==pxui_core::draw::ATLAS || !samples_image(&layer.instances,&layer.breaks,index) { None } else if let Some(image)=layer.images.get(b.image as usize) {
                    if let Some(texture)=image.handle.downcast_ref::<Texture>() { Some(texture.clone()) }
                    else { crate::image::texture_key(image).and_then(|key|renderer.textures.get(&key).map(|e|e.texture.clone())) }
                } else { None };
                if offset>0 && b.at_instance==0 && atlas_last && texture.is_none() { continue; }
                atlas_last=texture.is_none();
                renderer.breaks.push(Break { at_instance:offset+b.at_instance, texture });
            }
            offset+=layer.instances.len() as u32;
        }
        let parts=&parts[..batch_end-index];
        for &at in &renderer.split_points {
            let i=renderer.breaks.partition_point(|b| b.at_instance<at);
            if i==renderer.breaks.len() || renderer.breaks[i].at_instance!=at {
                let texture=i.checked_sub(1).and_then(|j| renderer.breaks[j].texture.clone());
                renderer.breaks.insert(i,Break { at_instance:at, texture });
            }
        }
        let breaks=&renderer.breaks;
        let mut missing_split=false;
        let result=if renderer.split_points.is_empty() {
            let selected=cull.then(|| (scissors, &ui.output().draw[index..batch_end]));
            renderer.painter.draw_retained_split(&encoder,size_px,pass.scale,parts,breaks,&[],selected,damage,&mut |_,_| Ok(None))
        } else {
            if pass.split.is_none() && !reuse {
                renderer.breaks.clear();
                return renderer.fail(PXUI_ERR_STATE, NEED_SPLIT);
            }
            let (queue,device,format,samples,serial)=(&renderer.queue,&renderer.device,renderer.format,renderer.samples,renderer.frame_serial);
            let (native_commands,scale)=([pass.vk_command_buffer,pass.mtl_command_buffer],pass.scale);
            let mut reopened: Option<ogpu_core::backend::RenderEncoder>=None;
            let current=encoder.clone();
            let outcome=renderer.painter.draw_retained_split(&current,size_px,scale,parts,breaks,&renderer.split_points,None,damage,&mut |painter,at| {
                let mut latest=None;
                for (slot,b) in ui.output().blurs.iter().enumerate().filter(|(_,b)|b.layer==index && b.at_instance==at) {
                    let [w,h]=size_px;
                    let (x0,y0)=((b.rect.x*scale).floor().max(0.0) as u32,(b.rect.y*scale).floor().max(0.0) as u32);
                    let (x1,y1)=(((b.rect.x+b.rect.w)*scale).ceil().clamp(0.0,w as f32) as u32,((b.rect.y+b.rect.h)*scale).ceil().clamp(0.0,h as f32) as u32);
                    if x0>=x1 || y0>=y1 || !(b.sigma>0.0) { continue; }
                    let open=latest.clone().or_else(||reopened.clone()).unwrap_or_else(||current.clone());
                    let cache_slot=(reuse && slot<renderer.blur_slots).then_some(slot);
                    let cached=cache_slot.and_then(|slot|renderer.blur_cache[slot].as_ref());
                    let (encoder,blurred)=if let Some(cached)=cached.filter(|entry|entry.written<=renderer.completed) {
                        (open,cached.result.clone())
                    } else {
                        // Preserve an in-flight result until the host reports completion. A split in
                        // the meantime paints through shared scratch, leaving the pending result intact.
                        let cache_slot=cache_slot.filter(|_|cached.is_none());
                        let Some(split)=pass.split else {
                            missing_split=true;
                            return Err(ogpu_core::error::make("Pxui.draw",ogpu_core::error::ErrorKind::InvalidState,NEED_SPLIT));
                        };
                        // this copy of the pass ends here; the app ends its own, records the blur and reopens one
                        gpu_backend::end_render(&open)?;
                        let mut rec=Recording { painter:&mut *painter, renderer_device:device, queue, commands:&mut renderer.blur_commands, sources:&mut renderer.blur_sources, serial, native:native_commands, rect:[x0,y0,x1,y1], sigma:b.sigma*scale, saturation:b.saturation, slot:cache_slot, blurred:None, error:None };
                        let mut next=pass;
                        let rc=split(pass.split_user,record_blur,(&mut rec as *mut Recording).cast(),&mut next);
                        if let Some(e)=rec.error.take() { return Err(e); }
                        if rc!=0 || rec.blurred.is_none() { return Err(ogpu_core::error::make("Pxui.draw",ogpu_core::error::ErrorKind::InvalidState,"the app's pass split failed")); }
                        let blurred=rec.blurred.take().expect("checked above");
                        let commands_native=ogpu_core::backend::NativeCommands { size:core::mem::size_of::<ogpu_core::backend::NativeCommands>() as u32, vk_command_buffer:next.vk_command_buffer, mtl_command_buffer:next.mtl_command_buffer, frame_serial:serial, inside_pass:1 };
                        let native_next=ogpu_core::backend::NativePass { size:core::mem::size_of::<ogpu_core::backend::NativePass>() as u32, vk_command_buffer:next.vk_command_buffer, mtl_render_encoder:next.mtl_render_encoder, width_px:next.width_px, height_px:next.height_px, scale:next.scale };
                        let resumed=adopt_again(queue,Some(commands.clone()),commands_native)?;
                        let encoder=if gpu_backend::readopt_render_encoder(&resumed,&open,native_next)? { open } else { gpu_backend::adopt_render_encoder(&resumed,native_next,format,samples)? };
                        commands=resumed;
                        if let Some(slot)=cache_slot {
                            renderer.blur_cache[slot]=Some(CachedBlur{view:*b,result:blurred.clone(),extent:size_px,scale,written:serial});
                        }
                        (encoder,blurred)
                    };
                    let rect=[x0,y0,x1,y1].map(|v|v as f32/scale);
                    let (alpha,radius)=((255.0*b.opacity.clamp(0.0,1.0)).round() as u8,b.radii.iter().fold(0.0f32,|a,r|a.max(*r)));
                    let clip=[b.rect.x.floor(),b.rect.y.floor(),(b.rect.x+b.rect.w).ceil(),(b.rect.y+b.rect.h).ceil()].map(|v|v as i16);
                    let image=Instance::image(rect,blurred.uv,[alpha;4]).clip(clip,radius);
                    painter.draw_clipped(&encoder,size_px,scale,&[image],&[Break { at_instance:0, texture:Some(blurred.texture.clone()) }],damage)?;
                    latest=Some(encoder);
                }
                if latest.is_some() { reopened=latest.clone(); }
                Ok(latest)
            });
            if let Some(next)=reopened { encoder=next; }
            outcome
        };
        renderer.breaks.clear();
        if missing_split { return renderer.fail(PXUI_ERR_STATE, NEED_SPLIT); }
        if let Err(e)=result { return renderer.fail_gpu(e); }
        for index in index..batch_end { ui.drawn(index); }
        unsafe { (*renderer.shared).draw_acknowledged = true; }
        index=batch_end;
    }
    // ends only this copy's view of the pass; the app's encoder must `invalidate_state` before drawing more
    if let Err(e)=gpu_backend::end_render(&encoder) { return renderer.fail_gpu(e); }
    gpu_backend::settle_commands(&commands);
    renderer.drawn=Some((commands,encoder));
    0
}

export!(bare pub unsafe fn pxui_renderer_destroy(renderer:*mut PxuiRenderer) {
    if renderer.is_null() { return; }
    let PxuiRenderer { painter, queue, device, textures, blur_sources, shared, frame_serial, prepared, drawn, blur_commands, blur_cache, .. } = *unsafe { Box::from_raw(renderer) };
    // a painter or texture that fails to go stays a child of the device, which then refuses to go: reported below
    drop((prepared, drawn, blur_commands));
    drop(blur_cache);
    drop(painter);
    drop((textures,blur_sources));
    // Contract (pxui.h): the app has finished every frame it passed to prepare, so everything retires now.
    let result=gpu_backend::retire(&queue,frame_serial).and_then(|()| gpu_backend::destroy_queue(&queue)).and_then(|()| gpu_backend::destroy_device(&device));
    if let Err(e)=result { unsafe { let error=&mut (*shared).error; error.clear(); error.push_str(&ogpu_core::error::to_string(&e)); } }
    unsafe { crate::ui::renderer_released(shared); }
});

// The native handle of a glyph atlas texture (`kind` 0: the mask atlas, one byte per texel; 1: the colour atlas, RGBA8), for an
// app pass that draws text quads of `pxui_ui_label_quads`. The texture stays the renderer's: do not keep the handle past the frame.
export!(pub unsafe fn pxui_renderer_atlas(renderer:*mut PxuiRenderer, kind:u32, out:*mut pxui_types::Image) -> i32 {
    if renderer.is_null() { return PXUI_ERR_ARGUMENT; }
    let renderer=unsafe { &mut *renderer };
    if out.is_null() || unsafe { (*out).size } < core::mem::size_of::<pxui_types::Image>() as u32 { return renderer.fail(PXUI_ERR_ARGUMENT, "invalid image output"); }
    let (atlas,format)=match kind { 0=>(AtlasKind::Mask,pxui_types::Image::FORMAT_R8), 1=>(AtlasKind::Color,pxui_types::Image::FORMAT_RGBA8), 2=>(AtlasKind::Mask2,pxui_types::Image::FORMAT_R8), 3=>(AtlasKind::Color2,pxui_types::Image::FORMAT_RGBA8), _=>return renderer.fail(PXUI_ERR_ARGUMENT, "unknown atlas kind") };
    let Some(texture)=renderer.painter.atlas(atlas) else { return renderer.fail(PXUI_ERR_STATE,"atlas page has not been prepared") };
    let native=match gpu_backend::native_texture(texture) { Ok(native)=>native, Err(e)=>return renderer.fail_gpu(e) };
    unsafe { *out=pxui_types::Image { size:core::mem::size_of::<pxui_types::Image>() as u32, vk_image_view:native.vk_image_view, mtl_texture:native.mtl_texture, width:native.width, height:native.height, format }; }
    0
});

// The draw calls this renderer's painter has issued so far, for tests that pin how many a frame takes.
export!(pub unsafe fn pxui_renderer_draw_calls(renderer:*mut PxuiRenderer) -> u64 {
    unsafe { renderer.as_ref() }.map_or(0, |r| r.painter.draw_calls())
});
