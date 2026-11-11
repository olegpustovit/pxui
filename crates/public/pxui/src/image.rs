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

fn size(value: Size) -> PxuiSizeDesc {
    match value {
        Size::Px(value) => PxuiSizeDesc { kind: 0, value, relative: None },
        Size::Pct(value) => PxuiSizeDesc { kind: 1, value, relative: None },
        Size::Rel(relative) => PxuiSizeDesc { kind: 2, value: 0.0, relative: Some(relative) },
        Size::Grow => PxuiSizeDesc { kind: 3, value: 0.0, relative: None },
        Size::Fit => PxuiSizeDesc { kind: 4, value: 0.0, relative: None },
    }
}

fn fit(value: Fit) -> u32 {
    match value { Fit::Fill => 0, Fit::Contain => 1, Fit::Cover => 2, Fit::None => 3 }
}

fn as_str(value: &str) -> Str { Str { ptr: value.as_ptr(), len: value.len() } }

impl Ui {
    /// Adds a host-owned GPU image as an accessible image widget.
    pub fn image(&mut self, label: &str, image: &Image, fit_mode: Fit, width: Size, height: Size) -> Result<PxuiBox, i32> {
        let mut out = PxuiBox::default();
        let rc = unsafe { sys::pxui_ui_image(self.0.as_ptr(), as_str(label), image, fit(fit_mode), size(width), size(height), &mut out) };
        if rc == 0 { Ok(out) } else { Err(rc) }
    }
}

impl Paint<'_> {
    /// Paints a host-owned GPU image into the given logical-point rectangle.
    pub fn image(&mut self, image: &Image, x: f32, y: f32, width: f32, height: f32, fit_mode: Fit, tint: Rgba) {
        unsafe { sys::pxui_paint_image(self.image_raw().cast(), image, x, y, width, height, fit(fit_mode), tint) }
    }
}
