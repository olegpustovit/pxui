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

fn image(raw: pxui_types::Image, handles: &mut Handles) -> Option<CoreImage> {
    if !valid(raw) { return None; }
    Some(CoreImage { handle: handles.get(raw), width: i64::from(raw.width), height: i64::from(raw.height) })
}

pub(crate) fn valid(raw: pxui_types::Image) -> bool { raw.valid() }

struct Handle { value: Rc<dyn std::any::Any>, used: u64, bytes: u64 }
/// One `Ui`'s native-image identities (`Shared::images`): bounded to `HANDLES_KEPT` entries and `HANDLES_BYTES` of estimated texels; a miss
/// that overflows either evicts the least recently used by a scan of at most `HANDLES_KEPT` entries.
pub(crate) struct Handles { entries: std::collections::HashMap<TextureKey, Handle>, clock: u64, bytes: u64 }
const HANDLES_KEPT: usize = 1024;
const HANDLES_BYTES: u64 = 128 * 1024 * 1024;
impl Handles {
    pub(crate) fn new() -> Self { Self { entries: std::collections::HashMap::with_capacity(HANDLES_KEPT), clock:0, bytes:0 } }
    pub(crate) fn get(&mut self, raw: pxui_types::Image) -> Rc<dyn std::any::Any> {
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
    pub(crate) fn remove(&mut self,key:&TextureKey) {
        if let Some(handle)=self.entries.remove(key) { self.bytes-=handle.bytes; }
    }
}
// Stable identities avoid extra segments and wrappers when a warmed gallery scrolls back into view.

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
    /// The 1.70 image calls validate everything before they touch the native-image table. `pxui_ui_image_node`: a short record (read
    /// after its `size`), an invalid descriptor, an unknown fit or size kind and a NaN size are refused with PXUI_ERR_ARGUMENT and
    /// register nothing; a valid call is keyed by position, not by its name (`##`/`###` in a name are ordinary text).
    /// `pxui_paint_image_checked` under a live painter: a short record, an invalid descriptor, an unknown fit and a NaN rectangle are
    /// refused and register nothing, a valid call registers one native image; with a null painter it is refused.
    #[test]
    fn image_node_and_checked_paint_refuse_bad_input_before_registering_anything() {
        use pxui_types::{ffi::PxuiSizeDesc, PxuiBox, Str, PXUI_ERR_ARGUMENT};
        extern "C" fn build(_: *mut core::ffi::c_void, raw: *mut core::ffi::c_void) {
            let ui: *mut crate::PxuiUi = raw.cast();
            let name = |t: &'static str| Str { ptr: t.as_ptr(), len: t.len() };
            let px = PxuiSizeDesc { kind: PxuiSizeDesc::PX, value: 20.0, relative: None };
            let good = pxui_types::Image { mtl_texture: std::ptr::dangling_mut(), width: 8, height: 4, format: 1, ..Default::default() };
            let images = || unsafe { crate::get(ui) }.unwrap().shared().images.entries.len();
            let mut out = PxuiBox::default();
            for (image, fit, w) in [
                (pxui_types::Image { size: 8, ..good }, 1, px),
                (pxui_types::Image { width: 0, ..good }, 1, px),
                (good, 9, px),
                (good, 1, PxuiSizeDesc { kind: 99, ..px }),
                (good, 1, PxuiSizeDesc { value: f32::NAN, ..px }),
            ] {
                assert_eq!(unsafe { super::pxui_ui_image_node(ui, name("Cover"), &image, fit, w, px, &mut out) }, PXUI_ERR_ARGUMENT);
                assert_eq!(images(), 0, "a refused call registers no native image");
            }
            assert_eq!(unsafe { super::pxui_ui_image_node(ui, name("a##b###c"), &good, 1, px, px, &mut out) }, 0);
            let first = out.box_key;
            assert_eq!(unsafe { super::pxui_ui_image_node(ui, name("c"), &good, 1, px, px, &mut out) }, 0);
            assert_ne!(out.box_key, first, "the second image is the second \\0image");
            assert_ne!(first, pxui_types::ui::key_of(pxui_types::ui::ROOT_KEY, "c"), "`###c` in a name is not a key");
            assert_eq!(images(), 1);
            // a painter callback on that node exercises the checked paint call under a live painter
            extern "C" fn paint(user: *mut core::ffi::c_void, raw: *mut crate::paint::PxuiPaint, rect: pxui_types::geom::Rect, _: *const pxui_types::ffi::PxuiPaintInfo) {
                let _ = user;
                // the painter's own `Ui` state (the build's borrowed handle is gone by paint time)
                let images = || unsafe { (*(*raw).shared).images.entries.len() };
                let white = pxui_types::geom::Rgba::rgb(255, 255, 255);
                let good = pxui_types::Image { mtl_texture: 0x10 as *mut _, width: 8, height: 4, format: 1, ..Default::default() };
                let before = images();
                for (image, fit, x) in [(pxui_types::Image { size: 8, ..good }, 1, rect.x), (pxui_types::Image { width: 0, ..good }, 1, rect.x), (good, 9, rect.x), (good, 1, f32::NAN)] {
                    let rc = unsafe { super::pxui_paint_image_checked(raw, &image, x, rect.y, 4.0, 4.0, fit, white) };
                    PAINTED.with(|p| p.borrow_mut().push((rc, images() - before)));
                }
                let rc = unsafe { super::pxui_paint_image_checked(raw, &good, rect.x, rect.y, 4.0, 4.0, 1, white) };
                PAINTED.with(|p| p.borrow_mut().push((rc, images() - before)));
            }
            assert_eq!(unsafe { crate::paint::pxui_ui_draw(ui, &out, 0, Some(paint), core::ptr::null_mut(), None) }, 0);
        }
        thread_local! { static PAINTED: std::cell::RefCell<Vec<(i32, usize)>> = const { std::cell::RefCell::new(Vec::new()) }; }
        let mut ui = unsafe { crate::pxui_ui_create(&crate::PxuiUiDesc::default()) };
        let mut frame = crate::FrameOut::default();
        assert_eq!(unsafe { crate::pxui_frame(ui, &crate::FrameIn { width: 320.0, height: 240.0, ..Default::default() }, Some(build), (&mut ui as *mut *mut crate::PxuiUi).cast(), &mut frame) }, 0);
        let painted = PAINTED.with(|p| p.borrow().clone());
        assert_eq!(painted, [(PXUI_ERR_ARGUMENT, 0), (PXUI_ERR_ARGUMENT, 0), (PXUI_ERR_ARGUMENT, 0), (PXUI_ERR_ARGUMENT, 0), (0, 1)],
            "a short record, an invalid descriptor, an unknown fit and a NaN rectangle are refused and register nothing; a valid call registers one");
        // the checked paint call outside a painter, and with bad input, is refused the same way
        let good = pxui_types::Image { mtl_texture: std::ptr::dangling_mut(), width: 8, height: 4, format: 1, ..Default::default() };
        assert_eq!(unsafe { super::pxui_paint_image_checked(core::ptr::null_mut(), &good, 0.0, 0.0, 4.0, 4.0, 1, pxui_types::geom::Rgba::rgb(255, 255, 255)) }, PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { crate::pxui_ui_destroy(ui) }, 0);
    }
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

export!(build pub unsafe fn pxui_ui_image(ui: *mut crate::PxuiUi, label: Str, raw: *const pxui_types::Image, fit_code: u32, width: PxuiSizeDesc, height: PxuiSizeDesc, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { crate::get(ui) }) else { return -1; };
    if raw.is_null() || out.is_null() { return unsafe { crate::fail(ui, "invalid image descriptor") }; }
    let (Some(label), Some(image), Some(fit), Some(width), Some(height)) = (unsafe { crate::state::checked_label(label) }, image(unsafe { *raw }, &mut state.shared().images), fit(fit_code), size(width), size(height))
        else { return unsafe { crate::fail(ui, "invalid image descriptor") }; };
    let handle = unsafe { &mut *state.engine }.image_labeled(label, &image, fit, width, height);
    unsafe { *out = to_abi_box(handle); }
    0
});

// ABI 1.77: an image keyed by `key` (a label or id label under the current parent, as `pxui_ui_node` keys it) and named `name` for
// accessibility only (never parsed for a key). Checked as `pxui_ui_image_node`, the key as a node label.
export!(build pub unsafe fn pxui_ui_image_keyed(ui: *mut crate::PxuiUi, key: Str, name: Str, raw: *const pxui_types::Image, fit_code: u32, width: PxuiSizeDesc, height: PxuiSizeDesc, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { crate::get(ui) }) else { return crate::PXUI_ERR_ARGUMENT; };
    if raw.is_null() || out.is_null() { return unsafe { crate::fail(ui, "invalid image descriptor") }; }
    let (Some(record), Some(key), Some(name), Some(fit), Some(width), Some(height)) = (read_desc!(raw, pxui_types::Image), unsafe { crate::state::checked_key(key) }, unsafe { checked(name) }, fit(fit_code), size(width), size(height))
        else { return unsafe { crate::fail(ui, "invalid image arguments") }; };
    let Some(image) = image(record, &mut state.shared().images) else { return unsafe { crate::fail(ui, "invalid image descriptor") }; };
    let handle = unsafe { &mut *state.engine }.image_keyed(key, name, &image, fit, width, height);
    unsafe { *out = to_abi_box(handle); }
    0
});

// ABI 1.70: an image with no key (the n-th image among its siblings, as a label is) named `name` for accessibility only; `name` is never
// parsed for a key. The record is read after its `size`, and every argument is checked before the image's native handle is registered:
// PXUI_ERR_ARGUMENT, nothing added and nothing registered, for a short or invalid descriptor (`Image::valid`), an unknown fit, a size or
// a name the library refuses.
export!(build pub unsafe fn pxui_ui_image_node(ui: *mut crate::PxuiUi, name: Str, raw: *const pxui_types::Image, fit_code: u32, width: PxuiSizeDesc, height: PxuiSizeDesc, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { crate::get(ui) }) else { return crate::PXUI_ERR_ARGUMENT; };
    if raw.is_null() || out.is_null() { return unsafe { crate::fail(ui, "invalid image descriptor") }; }
    let (Some(record), Some(name), Some(fit), Some(width), Some(height)) = (read_desc!(raw, pxui_types::Image), unsafe { checked(name) }, fit(fit_code), size(width), size(height))
        else { return unsafe { crate::fail(ui, "invalid image arguments") }; };
    let Some(image) = image(record, &mut state.shared().images) else { return unsafe { crate::fail(ui, "invalid image descriptor") }; };
    let handle = unsafe { &mut *state.engine }.image(name, &image, fit, width, height);
    unsafe { *out = to_abi_box(handle); }
    0
});

export!(pub unsafe fn pxui_paint_image(paint: *mut crate::paint::PxuiPaint, raw: *const pxui_types::Image, x: f32, y: f32, width: f32, height: f32, fit_code: u32, tint: Rgba) {
    if raw.is_null() || paint.is_null() || !finite(&[x, y, width, height]) { return; }
    let (Some(image), Some(fit), Some(paint)) = (unsafe { image(*raw, &mut (*(*paint).shared).images) }, fit(fit_code), unsafe { crate::paint::value(paint) }) else { return; };
    paint.image_fit(x, y, width, height, &image, fit, tint);
});

// ABI 1.70: `pxui_paint_image` that reports a refusal: PXUI_ERR_ARGUMENT (nothing drawn, nothing registered) for a null painter or
// descriptor, a short or invalid descriptor (`Image::valid`, read after its `size`), an unknown fit or a non-finite rectangle; 0 when
// drawn. `pxui_paint_image` is cut at 2.0.
export!(pub unsafe fn pxui_paint_image_checked(paint: *mut crate::paint::PxuiPaint, raw: *const pxui_types::Image, x: f32, y: f32, width: f32, height: f32, fit_code: u32, tint: Rgba) -> i32 {
    if raw.is_null() || paint.is_null() || !finite(&[x, y, width, height]) { return crate::PXUI_ERR_ARGUMENT; }
    let (Some(record), Some(fit)) = (read_desc!(raw, pxui_types::Image), fit(fit_code)) else { return crate::PXUI_ERR_ARGUMENT; };
    if !valid(record) { return crate::PXUI_ERR_ARGUMENT; }
    // the painter is checked (no span open) before the image's native handle is registered
    let Some(engine) = (unsafe { crate::paint::value(paint) }) else { return crate::PXUI_ERR_ARGUMENT; };
    let Some(shared) = (unsafe { (*paint).shared.as_mut() }) else { return crate::PXUI_ERR_ARGUMENT; };
    let Some(image) = image(record, &mut shared.images) else { return crate::PXUI_ERR_ARGUMENT; };
    engine.image_fit(x, y, width, height, &image, fit, tint);
    0
});
