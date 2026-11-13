use std::rc::Rc;
use ogpu_core::backend::{self, Device, NativeTexture, Texture};
use pxui_core::ui::{Fit, Image as CoreImage};
use pxui_types::{ffi::PxuiSizeDesc, geom::Rgba, PxuiBox, Str};
use crate::{node::size, state::{checked, finite, to_abi_box}};

#[derive(Clone, Copy)]
struct ForeignImage(pxui_types::Image);

fn fit(value: u32) -> Option<Fit> {
    Some(match value { 0 => Fit::Fill, 1 => Fit::Contain, 2 => Fit::Cover, 3 => Fit::None, _ => return None })
}

fn image(raw: pxui_types::Image) -> Option<CoreImage> {
    if !valid(raw) { return None; }
    Some(CoreImage { handle: handle_of(raw), width: i64::from(raw.width), height: i64::from(raw.height) })
}

pub(crate) fn valid(raw: pxui_types::Image) -> bool {
    raw.size >= core::mem::size_of::<pxui_types::Image>() as u32 && raw.width != 0 && raw.height != 0
        && (!raw.vk_image_view.is_null() || !raw.mtl_texture.is_null()) && raw.format < pxui_types::Image::FORMAT_DEPTH32F
}

struct Handle { value: Rc<dyn std::any::Any>, used: u64, bytes: u64 }
struct Handles { entries: std::collections::HashMap<TextureKey, Handle>, clock: u64, bytes: u64 }
const HANDLES_KEPT: usize = 1024;
const HANDLES_BYTES: u64 = 128 * 1024 * 1024;
impl Handles {
    fn new() -> Self { Self { entries: std::collections::HashMap::with_capacity(HANDLES_KEPT), clock:0, bytes:0 } }
    fn get(&mut self, raw: pxui_types::Image) -> Rc<dyn std::any::Any> {
        let key=image_key(&raw);
        self.clock=self.clock.wrapping_add(1);
        if let Some(handle)=self.entries.get_mut(&key) { handle.used=self.clock; return Rc::clone(&handle.value); }
        let texel=match raw.format { 0=>1, 4=>8, 5=>16, _=>4 };
        let bytes=u64::from(raw.width).saturating_mul(u64::from(raw.height)).saturating_mul(texel);
        // One image larger than the working-set budget may stand alone; no pixel memory is allocated here.
        while !self.entries.is_empty() && (self.entries.len()>=HANDLES_KEPT || self.bytes.saturating_add(bytes)>HANDLES_BYTES) {
            let key=*self.entries.iter().min_by_key(|(_,handle)|handle.used).unwrap().0;
            self.remove(&key);
        }
        let value: Rc<dyn std::any::Any>=Rc::new(ForeignImage(raw));
        self.entries.insert(key,Handle { value:Rc::clone(&value), used:self.clock, bytes });
        self.bytes=self.bytes.saturating_add(bytes);
        value
    }
    fn remove(&mut self,key:&TextureKey) {
        if let Some(handle)=self.entries.remove(key) { self.bytes-=handle.bytes; }
    }
}
// Stable identities avoid extra segments and wrappers when a warmed gallery scrolls back into view.
thread_local! { static HANDLES: std::cell::RefCell<Handles> = std::cell::RefCell::new(Handles::new()); }
fn handle_of(raw: pxui_types::Image) -> Rc<dyn std::any::Any> { HANDLES.with(|handles|handles.borrow_mut().get(raw)) }
pub(crate) fn forget(raw: pxui_types::Image) { HANDLES.with(|handles|handles.borrow_mut().remove(&image_key(&raw))); }

/// A native image's identity includes both handles, extent and format.
pub(crate) type TextureKey = (usize, usize, u32, u32, u32);

pub(crate) fn image_key(raw: &pxui_types::Image) -> TextureKey {
    (raw.vk_image_view as usize, raw.mtl_texture as usize, raw.width, raw.height, raw.format)
}

pub(crate) fn texture_key(image: &CoreImage) -> Option<TextureKey> {
    let raw = image.handle.downcast_ref::<ForeignImage>()?.0;
    Some(image_key(&raw))
}

pub(crate) fn adopt_texture(device: &Device, image: &CoreImage) -> Result<Texture, ()> {
    let raw = image.handle.downcast_ref::<ForeignImage>().ok_or(())?.0;
    backend::wrap_texture(device, NativeTexture {
        size: core::mem::size_of::<NativeTexture>() as u32,
        vk_image_view: raw.vk_image_view,
        mtl_texture: raw.mtl_texture,
        width: raw.width,
        height: raw.height,
        format: raw.format,
    }).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    #[test]
    fn image_identities_reuse_large_working_sets_and_bound_metadata_residency() {
        let mut cache=super::Handles::new();
        let raw=|id|pxui_types::Image { mtl_texture:id as *mut _,width:16,height:16,format:1,..Default::default() };
        let handles:Vec<_>=(1..=768).map(|id|cache.get(raw(id))).collect();
        mem_sys::local_reset();
        for _ in 0..10 { for (index,handle) in handles.iter().enumerate() { assert!(std::rc::Rc::ptr_eq(handle,&cache.get(raw(index+1)))); } }
        assert_eq!(mem_sys::local_alloc_count(),0);
        for id in 769..=2048 { cache.get(raw(id)); }
        assert_eq!(cache.entries.len(),super::HANDLES_KEPT);
        for id in 3000..4000 { cache.get(pxui_types::Image { width:512,height:512,..raw(id) }); }
        assert!(cache.bytes<=super::HANDLES_BYTES);
        assert_eq!(cache.entries.len(),128);
        let image=raw(4000); let handle=cache.get(image); let weak=std::rc::Rc::downgrade(&handle); drop(handle);
        cache.remove(&super::image_key(&image));
        assert_eq!(weak.strong_count(),0);
    }
    #[test]
    fn texture_keys_include_both_native_handles_extent_and_format() {
        let base=pxui_types::Image { width:16,height:32,format:1,..Default::default() };
        for changed in [
            pxui_types::Image { width:32,..base },pxui_types::Image { height:16,..base },
            pxui_types::Image { format:2,..base },
            pxui_types::Image { mtl_texture:std::ptr::dangling_mut(),..base },
            pxui_types::Image { vk_image_view:std::ptr::dangling_mut(),..base },
        ] { assert_ne!(super::image_key(&base),super::image_key(&changed)); }
    }
}

export!(pub unsafe fn pxui_ui_image(ui: *mut crate::PxuiUi, label: Str, raw: *const pxui_types::Image, fit_code: u32, width: PxuiSizeDesc, height: PxuiSizeDesc, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { crate::get(ui) }) else { return -1; };
    if raw.is_null() || out.is_null() { return unsafe { crate::fail(ui, "invalid image descriptor") }; }
    let (Some(label), Some(image), Some(fit), Some(width), Some(height)) = (unsafe { checked(label) }, image(unsafe { *raw }), fit(fit_code), size(width), size(height))
        else { return unsafe { crate::fail(ui, "invalid image descriptor") }; };
    let handle = unsafe { &mut *state.engine }.image(label, &image, fit, width, height);
    unsafe { *out = to_abi_box(handle); }
    0
});

export!(pub unsafe fn pxui_paint_image(paint: *mut crate::paint::PxuiPaint, raw: *const pxui_types::Image, x: f32, y: f32, width: f32, height: f32, fit_code: u32, tint: Rgba) {
    if raw.is_null() || !finite(&[x, y, width, height]) { return; }
    let (Some(image), Some(fit), Some(paint)) = (image(unsafe { *raw }), fit(fit_code), unsafe { crate::paint::value(paint) }) else { return; };
    paint.image_fit(x, y, width, height, &image, fit, tint);
});
