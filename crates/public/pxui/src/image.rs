//! Host-owned GPU images in the widget tree and in a painter.
//!
//! One rule for a descriptor PXUI cannot sample (`Image::valid` is false: a zero size, no native handle, a depth format, a short
//! `size`): the call draws nothing and returns `Error::InvalidInput`, in `Ui::image` and `Paint::image` alike. An `Image` is host data
//! that can go bad between frames, so it is never a panic. The check is the library's (`pxui_ui_image_node`,
//! `pxui_paint_image_checked`).
use crate::node::size;
use crate::sys::PxuiPaint as RawPaint;
use crate::{as_str, Error, Handle, Paint, Ui};
use pxui_types::{draw::Fit, ffi::PxuiSizeDesc, geom::{Rect, Rgba}, Image, PxuiBox, Size, Str};

library!(lib {
    fn pxui_ui_image_node(ui: *mut PxuiUi, name: Str, image: *const Image, fit: u32, width: PxuiSizeDesc, height: PxuiSizeDesc, out: *mut PxuiBox) -> i32;
    fn pxui_paint_image_checked(paint: *mut RawPaint, image: *const Image, x: f32, y: f32, width: f32, height: f32, fit: u32, tint: Rgba) -> i32;
});

#[inline]
fn fit(value: Fit) -> u32 {
    match value { Fit::Fill => 0, Fit::Contain => 1, Fit::Cover => 2, Fit::None => 3 }
}

/// How `Ui::image` places a picture: its box's width and height and how the image fits inside it.
///
/// A `Copy` value made with const builders: `const COVER: ImageStyle = ImageStyle::DEFAULT.fit(Fit::Cover).w_px(40.0).h_px(40.0);`.
#[derive(Clone, Copy, Debug)]
pub struct ImageStyle {
    pub(crate) fit: Fit,
    pub(crate) w: Size,
    pub(crate) h: Size,
}

impl ImageStyle {
    /// The image contained (aspect kept, letterboxed) in a box that grows into its parent's free space on both axes.
    pub const DEFAULT: ImageStyle = ImageStyle { fit: Fit::Contain, w: Size::Grow, h: Size::Grow };

    /// The same style with the image placed as CSS `object-fit` places it: `Fill` stretches, `Contain` and `Cover` keep the aspect
    /// ratio, `None` keeps the pixel size.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{draw::Fit, ImageStyle};
    /// const COVER: ImageStyle = ImageStyle::DEFAULT.fit(Fit::Cover);
    /// # let _ = COVER;
    /// ```
    #[inline] pub const fn fit(mut self, fit: Fit) -> ImageStyle { self.fit = fit; self }
    /// The same style with the box `size` wide.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::Size, ImageStyle};
    /// const WIDE: ImageStyle = ImageStyle::DEFAULT.w(Size::Pct(50.0));
    /// # let _ = WIDE;
    /// ```
    #[inline] pub const fn w(mut self, size: Size) -> ImageStyle { self.w = size; self }
    /// The same style with the box `size` tall.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::{ui::Size, ImageStyle};
    /// const TALL: ImageStyle = ImageStyle::DEFAULT.h(Size::Pct(50.0));
    /// # let _ = TALL;
    /// ```
    #[inline] pub const fn h(mut self, size: Size) -> ImageStyle { self.h = size; self }
    /// The same style with the box `points` wide: `w(Size::Px(points))`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const THUMB: pxui::ImageStyle = pxui::ImageStyle::DEFAULT.w_px(40.0).h_px(40.0);
    /// # let _ = THUMB;
    /// ```
    #[inline] pub const fn w_px(self, points: f32) -> ImageStyle { self.w(Size::Px(points)) }
    /// The same style with the box `points` tall: `h(Size::Px(points))`.
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const BANNER: pxui::ImageStyle = pxui::ImageStyle::DEFAULT.h_px(120.0);
    /// # let _ = BANNER;
    /// ```
    #[inline] pub const fn h_px(self, points: f32) -> ImageStyle { self.h(Size::Px(points)) }
}

impl Ui {
    /// Adds a host-owned GPU image as a leaf widget placed by `style`, framed by the theme's image look; `name` is only its accessible
    /// name.
    ///
    /// The `Image` is a descriptor the host keeps alive until the frame has been drawn; the library never copies or frees the texture.
    /// Returns the widget's node. `name` is display text only: never parsed for a key (`##` and `###` are ordinary characters).
    /// Allocates: never on a warm frame.
    /// Identity: no key, as `label`: every image is the n-th image among its siblings, whatever its name; images hold no state.
    /// Panics: never; refusals are the errors below.
    /// Errors: `Error::InvalidInput` (nothing added) for an invalid image (`Image::valid`), a size the library refuses or a name past
    /// 64 KiB; `Error::State` outside a frame.
    ///
    /// ```
    /// use pxui::{draw::Fit, Image, ImageStyle};
    /// const COVER: ImageStyle = ImageStyle::DEFAULT.fit(Fit::Contain).w_px(40.0).h_px(40.0);
    /// let image = Image { mtl_texture: std::ptr::dangling_mut(), width: 8, height: 4, format: Image::FORMAT_RGBA8, ..Default::default() };
    /// let ui = pxui::doc::frames(2, &mut |ui| {
    ///     let a = ui.image("Cover", &image, &COVER).unwrap();
    ///     let b = ui.image("Cover", &image, &COVER).unwrap();
    ///     assert_ne!(a.box_key, b.box_key, "two images, two nodes");
    ///     assert_eq!(ui.image("Gone", &Image::default(), &COVER).err(), Some(pxui::Error::InvalidInput));
    /// });
    /// assert!(ui.a11y_nodes().iter().any(|n| ui.a11y_name_of(n) == "Cover"), "the name is the accessible name");
    /// ```
    #[track_caller]
    pub fn image(&mut self, name: &str, image: &Image, style: &ImageStyle) -> Result<Handle, Error> {
        let mut out = Handle::default();
        let rc = unsafe { lib::pxui_ui_image_node(self.0.as_ptr(), as_str(name), image, fit(style.fit), size(style.w), size(style.h), &mut out) };
        if rc == 0 { Ok(out) } else { Err(Error::from_code(rc)) }
    }
}

impl Paint<'_> {
    /// Paints a host-owned GPU image into `rect` (logical points) as CSS `object-fit` places it, its texels multiplied by `tint`.
    ///
    /// `Fit::Cover` and `Fit::Contain` keep the aspect ratio; `Fit::None` keeps the pixel size; opaque white leaves the texels
    /// unchanged. A scaled image samples linearly.
    /// Allocates: never on a warm frame.
    /// Panics: never; refusals are the errors below.
    /// Errors: `Error::InvalidInput` (nothing drawn) for an invalid image (`Image::valid`) or a `rect` that is not finite.
    ///
    /// ```
    /// use pxui::{draw::Fit, geom::Rgba, Image};
    /// let image = Image { mtl_texture: std::ptr::dangling_mut(), width: 8, height: 4, format: Image::FORMAT_RGBA8, ..Default::default() };
    /// pxui::doc::frames(1, &mut |ui| {
    ///     let node = ui.node("art".into(), pxui::Node::DEFAULT.w_px(40.0).h_px(40.0));
    ///     ui.canvas(&node, move |paint, rect| {
    ///         paint.image(rect, &image, Fit::Cover, Rgba::rgb(255, 255, 255)).unwrap();
    ///         assert!(paint.image(rect, &Image::default(), Fit::Cover, Rgba::rgb(255, 255, 255)).is_err());
    ///     });
    /// });
    /// ```
    #[inline]
    pub fn image(&mut self, rect: Rect, image: &Image, fit_mode: Fit, tint: Rgba) -> Result<(), Error> {
        let rc = unsafe { lib::pxui_paint_image_checked(self.raw(), image, rect.x, rect.y, rect.w, rect.h, fit(fit_mode), tint) };
        if rc == 0 { Ok(()) } else { Err(Error::from_code(rc)) }
    }
}

#[cfg(test)]
mod tests {
    use crate::{draw::Fit, geom::{Rect, Rgba}, Fill, FrameIn, Image, ImageStyle, Stroke, Ui};

    /// An image's name is its accessible name verbatim (`##` and `###` are ordinary text) and never its identity: renaming an image
    /// keeps its node; images are keyed by position among their siblings.
    #[test]
    fn an_image_name_is_verbatim_accessibility_and_not_identity() {
        let image = Image { mtl_texture: std::ptr::dangling_mut(), width: 8, height: 4, format: Image::FORMAT_RGBA8, ..Default::default() };
        let mut ui = Ui::new().unwrap();
        ui.set_a11y(true);
        let mut keys = [0i64; 2];
        for (frame, name) in ["a##b###c", "renamed"].into_iter().enumerate() {
            ui.frame(&FrameIn { width: 320.0, height: 240.0, time: frame as f64, ..FrameIn::default() }, &mut |ui| {
                keys[frame] = ui.image(name, &image, &ImageStyle::DEFAULT.w_px(20.0).h_px(20.0)).unwrap().box_key;
            }).unwrap();
            assert!(ui.a11y_nodes().iter().any(|n| ui.a11y_name_of(n) == name), "{name} is the accessible name");
        }
        assert_eq!(keys[0], keys[1], "renaming keeps the node");
    }

    /// Warm frames that add image widgets and paint images and every shape with `Fill`/`Stroke` values allocate nothing, in the
    /// wrapper or the library; a refused image is an error, not an allocation.
    #[test]
    fn warm_image_widgets_painted_images_and_shapes_allocate_nothing() {
        mem_sys::set_peer(crate::library_counter);
        const THUMB: ImageStyle = ImageStyle::DEFAULT.fit(Fit::Cover).w_px(40.0).h_px(40.0);
        let image = Image { mtl_texture: std::ptr::dangling_mut(), width: 8, height: 4, format: Image::FORMAT_RGBA8, ..Default::default() };
        let ink = Rgba::rgb(10, 20, 30);
        let mut ui = Ui::new().unwrap();
        for frame in 0..12 {
            if frame == 8 { mem_sys::local_reset(); }
            ui.frame(&FrameIn { width: 320.0, height: 240.0, time: frame as f64 / 60.0, ..FrameIn::default() }, &mut |ui| {
                for name in ["One", "Two", "Three"] { ui.image(name, &image, &THUMB).unwrap(); }
                assert!(ui.image("Bad", &Image::default(), &THUMB).is_err());
                let node = ui.node("canvas".into(), crate::Node::DEFAULT.w_px(200.0).h_px(100.0));
                ui.canvas(&node, move |p, r| {
                    p.image(r, &image, Fit::Contain, Rgba::rgb(255, 255, 255)).unwrap();
                    assert!(p.image(r, &Image::default(), Fit::Contain, ink).is_err());
                    p.fill(Rect::new(r.x, r.y, 10.0, 10.0), Fill::new(ink));
                    p.fill(Rect::new(r.x + 0.25, r.y, 10.0, 0.5), Fill::new(ink).fractional());
                    p.stroke(r, Stroke::new(ink).width(2.0));
                    p.frame(r, Stroke::new(ink));
                    p.brackets(r, 2.0, 6.0, Stroke::new(ink));
                    p.dashed_rect(r, Stroke::new(ink).width(1.5));
                    p.line((r.x, r.y), (r.x + 20.0, r.y + 9.0), Stroke::new(ink).width(1.5));
                    p.dashed((r.x, r.y + 50.0), (r.x + 80.0, r.y + 50.0), Stroke::new(ink));
                    p.triangle((r.x, r.y), (r.x, r.y + 10.0), (r.x + 9.0, r.y + 5.0), Fill::new(ink));
                    p.curve((r.x, r.y), (r.x + 5.0, r.y + 10.0), (r.x + 10.0, r.y), Stroke::new(ink).width(1.5));
                    p.cubic([(r.x, r.y), (r.x + 4.0, r.y + 8.0), (r.x + 8.0, r.y - 8.0), (r.x + 12.0, r.y)], Stroke::new(ink));
                    p.circle((r.x + 30.0, r.y + 30.0), 6.0, Some(Fill::new(ink)), Some(Stroke::new(ink).width(2.0)));
                    p.grid(r, (r.x, r.y), 8.0, 1.0, Fill::new(ink));
                    p.chevron((r.x + 60.0, r.y + 10.0), crate::ui::Dir::Down, Stroke::new(ink));
                    p.flag((r.x + 80.0, r.y + 10.0), true, true);
                });
            }).unwrap();
        }
        assert_eq!(mem_sys::local_alloc_count(), 0, "warm image and shape frames allocate nothing");
    }
}
