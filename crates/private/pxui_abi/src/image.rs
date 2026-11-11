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
    Some(CoreImage { handle: Rc::new(ForeignImage(raw)), width: i64::from(raw.width), height: i64::from(raw.height) })
}

pub(crate) fn texture_key(image: &CoreImage) -> Option<usize> {
    image.handle.downcast_ref::<ForeignImage>()?;
    Some(Rc::as_ptr(&image.handle) as *const () as usize)
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
