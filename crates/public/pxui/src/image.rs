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
    /// Adds a host-owned GPU image as a widget of `width` by `height`, drawn inside its box as `fit_mode` places it; its `label` is the accessible name.
    ///
    /// The `Image` is a descriptor the host keeps alive until the frame has been drawn; the library never copies or frees the texture.
    /// Returns the widget's box.
    /// Allocates: never on a warm frame.
    /// Identity: `label` keys the widget under its parent; the same label twice in one parent collides.
    /// Panics: when the image is invalid (zero width or height, no native texture handle, an unknown format, a short `size`), or outside a frame.
    ///
    /// ```
    /// use pxui::{draw::Fit, Image, Size};
    /// let image = Image { mtl_texture: std::ptr::dangling_mut(), width: 8, height: 4, format: Image::FORMAT_RGBA8, ..Default::default() };
    /// let ui = pxui::doc::frames(2, &mut |ui| {
    ///     ui.image("Cover", &image, Fit::Contain, Size::Px(40.0), Size::Px(40.0));
    /// });
    /// assert_eq!(ui.frame_index(), 2);
    /// ```
    #[track_caller]
    pub fn image(&mut self, label: &str, image: &Image, fit_mode: Fit, width: Size, height: Size) -> PxuiBox {
        let mut out = PxuiBox::default();
        check(self, unsafe { lib::pxui_ui_image(self.0.as_ptr(), as_str(label), image, fit(fit_mode), size(width), size(height), &mut out) });
        out
    }
}

impl Paint<'_> {
    /// Paints a host-owned GPU image into the logical-point rectangle `x`, `y`, `width`, `height`, stretched to fill it, untinted.
    ///
    /// An invalid image (see `Ui::image`) or a non-finite coordinate draws nothing and does not panic.
    /// Allocates: never on a warm frame.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::Image;
    /// let image = Image { mtl_texture: std::ptr::dangling_mut(), width: 8, height: 4, format: Image::FORMAT_RGBA8, ..Default::default() };
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("art".into(), pxui::Node::DEFAULT);
    ///     ui.canvas(&node, move |paint, rect| paint.image(rect.x, rect.y, 40.0, 40.0, &image));
    /// });
    /// ```
    #[inline]
    pub fn image(&mut self, x: f32, y: f32, width: f32, height: f32, image: &Image) {
        self.image_fit(x, y, width, height, image, Fit::Fill, Rgba { r: 255, g: 255, b: 255, a: 255 })
    }

    /// Paints a host-owned GPU image placed in the logical-point rectangle `x`, `y`, `width`, `height` as CSS `object-fit` places it, texels multiplied by `tint`.
    ///
    /// `Fit::Cover` and `Fit::Contain` keep the aspect ratio; `Fit::None` keeps the pixel size. Opaque white leaves the texels unchanged.
    /// An invalid image (see `Ui::image`) or a non-finite coordinate draws nothing and does not panic.
    /// Allocates: never on a warm frame.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{draw::Fit, geom::Rgba, Image};
    /// let image = Image { mtl_texture: std::ptr::dangling_mut(), width: 8, height: 4, format: Image::FORMAT_RGBA8, ..Default::default() };
    /// let white = Rgba { r: 255, g: 255, b: 255, a: 255 };
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("art".into(), pxui::Node::DEFAULT);
    ///     ui.canvas(&node, move |paint, rect| paint.image_fit(rect.x, rect.y, 40.0, 20.0, &image, Fit::Cover, white));
    /// });
    /// ```
    #[inline]
    pub fn image_fit(&mut self, x: f32, y: f32, width: f32, height: f32, image: &Image, fit_mode: Fit, tint: Rgba) {
        unsafe { lib::pxui_paint_image(self.raw(), image, x, y, width, height, fit(fit_mode), tint) }
    }
}
