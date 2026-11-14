//! Retained device scratch for synchronous private-texture transfers.
use super::{Error, Handle};
use std::{
    ffi::{c_char, c_void},
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
};

unsafe extern "C" {
    fn rays_metal_uploads_create(
        device: *mut super::NativeHandle,
        out: *mut *mut c_void,
        message: *mut *const c_char,
    ) -> i32;
    fn rays_metal_uploads_destroy(uploads: *mut c_void);
    fn rays_metal_texture_transfer(
        uploads: *mut c_void,
        write: u8,
        texture: *mut super::NativeHandle,
        bytes: *mut u8,
        length: usize,
        row: usize,
        image: usize,
        x: u64,
        y: u64,
        z: u64,
        width: u64,
        height: u64,
        depth: u64,
        mip: u64,
        slice: u64,
        message: *mut *const c_char,
    ) -> i32;
}

pub struct Uploads {
    raw: NonNull<c_void>,
    _thread: PhantomData<Rc<()>>,
}
impl Uploads {
    pub fn new(device: &Handle) -> Result<Self, Error> {
        let mut raw = std::ptr::null_mut();
        let mut message = std::ptr::null();
        let status =
            unsafe { rays_metal_uploads_create(device.as_native_ptr()?, &mut raw, &mut message) };
        if status != 0 {
            return Err(unsafe { Error::from_native(status, message) });
        }
        Ok(Self {
            raw: NonNull::new(raw).ok_or(Error::InvalidHandle)?,
            _thread: PhantomData,
        })
    }
    pub fn write(
        &mut self,
        texture: &Handle,
        layout: &ogpu_core::validation::TextureRead,
        bytes: &[u8],
    ) -> Result<(), Error> {
        self.transfer(
            texture,
            layout,
            bytes.as_ptr().cast_mut(),
            bytes.len(),
            true,
        )
    }
    pub fn read(
        &mut self,
        texture: &Handle,
        layout: &ogpu_core::validation::TextureRead,
        bytes: &mut [u8],
    ) -> Result<(), Error> {
        self.transfer(texture, layout, bytes.as_mut_ptr(), bytes.len(), false)
    }
    fn transfer(
        &mut self,
        texture: &Handle,
        layout: &ogpu_core::validation::TextureRead,
        bytes: *mut u8,
        length: usize,
        write: bool,
    ) -> Result<(), Error> {
        let mut message = std::ptr::null();
        let r = layout.region();
        let status = unsafe {
            rays_metal_texture_transfer(
                self.raw.as_ptr(),
                u8::from(write),
                texture.as_native_ptr()?,
                bytes,
                length,
                layout.bytes_per_row(),
                layout.bytes_per_image(),
                r.x as u64,
                r.y as u64,
                r.z as u64,
                r.width as u64,
                r.height as u64,
                r.depth as u64,
                layout.mip_level() as u64,
                layout.slice() as u64,
                &mut message,
            )
        };
        if status != 0 {
            return Err(unsafe { Error::from_native(status, message) });
        }
        Ok(())
    }
}
impl Drop for Uploads {
    fn drop(&mut self) {
        unsafe {
            rays_metal_uploads_destroy(self.raw.as_ptr());
        }
    }
}
