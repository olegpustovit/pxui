//! Host-owned GPU images in the widget tree and in a painter.
use crate::node::size;
use crate::sys::PxuiPaint as RawPaint;
use crate::{as_str, check, Paint, Ui};
use pxui_types::{draw::Fit, ffi::PxuiSizeDesc, geom::Rgba, Image, PxuiBox, Size, Str};

library!(lib {
    fn pxui_ui_image(ui: *mut PxuiUi, label: Str, image: *const Image, fit: u32, width: PxuiSizeDesc, height: PxuiSizeDesc, out: *mut PxuiBox) -> i32;
    fn pxui_paint_image(paint: *mut RawPaint, image: *const Image, x: f32, y: f32, width: f32, height: f32, fit: u32, tint: Rgba);
});

#[inline]
fn fit(value: Fit) -> u32 {
    match value { Fit::Fill => 0, Fit::Contain => 1, Fit::Cover => 2, Fit::None => 3 }
}

impl Ui {
    /// Adds a host-owned GPU image as an accessible image widget.
    #[track_caller]
    pub fn image(&mut self, label: &str, image: &Image, fit_mode: Fit, width: Size, height: Size) -> PxuiBox {
        let mut out = PxuiBox::default();
        check(self, unsafe { lib::pxui_ui_image(self.0.as_ptr(), as_str(label), image, fit(fit_mode), size(width), size(height), &mut out) });
        out
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
        unsafe { lib::pxui_paint_image(self.raw(), image, x, y, width, height, fit(fit_mode), tint) }
    }
}
