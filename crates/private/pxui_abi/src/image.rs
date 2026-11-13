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
    if raw.size < core::mem::size_of::<pxui_types::Image>() as u32 || raw.width == 0 || raw.height == 0
        || (raw.vk_image_view.is_null() && raw.mtl_texture.is_null()) || raw.format >= pxui_types::Image::FORMAT_DEPTH32F
    { return None; }
    Some(CoreImage { handle: handle_of(raw), width: i64::from(raw.width), height: i64::from(raw.height) })
}

// The engine tells images apart by their handle's identity and draws consecutive uses of one image as one segment, so an app texture
// named in several calls (or frames) gets the same handle each time: made anew per call, four uses of one texture cost four draws.
// A texture is the same one while its native handle, size and format are; the last few are kept (the oldest goes first).
thread_local! { static HANDLES: std::cell::RefCell<Vec<(pxui_types::Image, Rc<dyn std::any::Any>)>> = const { std::cell::RefCell::new(Vec::new()) }; }
const HANDLES_KEPT: usize = 64;

fn handle_of(raw: pxui_types::Image) -> Rc<dyn std::any::Any> {
    let same = |a: &pxui_types::Image| a.vk_image_view == raw.vk_image_view && a.mtl_texture == raw.mtl_texture && a.width == raw.width && a.height == raw.height && a.format == raw.format;
    HANDLES.with(|handles| {
        let mut handles = handles.borrow_mut();
        if let Some((_, handle)) = handles.iter().find(|(image, _)| same(image)) { return Rc::clone(handle); }
        if handles.len() == HANDLES_KEPT { handles.remove(0); }
        let handle: Rc<dyn std::any::Any> = Rc::new(ForeignImage(raw));
        handles.push((raw, Rc::clone(&handle)));
        handle
    })
}

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
