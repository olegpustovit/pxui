use pxui_types::{draw::Fit, ffi::PxuiSizeDesc, geom::Rgba, Image, PxuiBox, Size, Str};
use crate::{Paint, Ui};

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{pxui_paint_image, pxui_ui_image};
}

#[cfg(not(feature = "static"))]
mod sys {
    use pxui_types::{ffi::PxuiSizeDesc, geom::Rgba, Image, PxuiBox, Str};
    use crate::sys::PxuiUi;
    #[repr(C)] pub struct RawPaint { _private: [u8; 0] }
    #[link(name = "pxui")]
    unsafe extern "C" {
        pub fn pxui_ui_image(ui: *mut PxuiUi, label: Str, image: *const Image, fit: u32, width: PxuiSizeDesc, height: PxuiSizeDesc, out: *mut PxuiBox) -> i32;
        pub fn pxui_paint_image(paint: *mut RawPaint, image: *const Image, x: f32, y: f32, width: f32, height: f32, fit: u32, tint: Rgba);
    }
}

#[inline]
fn size(value: Size) -> PxuiSizeDesc {
    match value {
        Size::Px(value) => PxuiSizeDesc { kind: 0, value, relative: None },
        Size::Pct(value) => PxuiSizeDesc { kind: 1, value, relative: None },
        Size::Rel(relative) => PxuiSizeDesc { kind: 2, value: 0.0, relative: Some(relative) },
        Size::Grow => PxuiSizeDesc { kind: 3, value: 0.0, relative: None },
        Size::Fit => PxuiSizeDesc { kind: 4, value: 0.0, relative: None },
    }
}

#[inline]
fn fit(value: Fit) -> u32 {
    match value { Fit::Fill => 0, Fit::Contain => 1, Fit::Cover => 2, Fit::None => 3 }
}

#[inline]
fn as_str(value: &str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }

impl Ui {
    /// Adds a host-owned GPU image as an accessible image widget.
    #[inline]
    pub fn image(&mut self, label: &str, image: &Image, fit_mode: Fit, width: Size, height: Size) -> Result<PxuiBox, i32> {
        let mut out = PxuiBox::default();
        let rc = unsafe { sys::pxui_ui_image(self.0.as_ptr(), as_str(label), image, fit(fit_mode), size(width), size(height), &mut out) };
        if rc == 0 { Ok(out) } else { Err(rc) }
    }
}

impl Paint<'_> {
    /// Paints a host-owned GPU image into the given logical-point rectangle, stretched to fill it.
    #[inline]
    pub fn image(&mut self, x: f32, y: f32, width: f32, height: f32, image: &Image) {
        self.image_fit(x, y, width, height, image, Fit::Fill, Rgba { r: 255, g: 255, b: 255, a: 255 })
    }

    /// Paints a host-owned GPU image placed in the rectangle as CSS `object-fit` places it, texels multiplied by `tint`.
    #[inline]
    pub fn image_fit(&mut self, x: f32, y: f32, width: f32, height: f32, image: &Image, fit_mode: Fit, tint: Rgba) {
        unsafe { sys::pxui_paint_image(self.image_raw().cast(), image, x, y, width, height, fit(fit_mode), tint) }
    }
}
