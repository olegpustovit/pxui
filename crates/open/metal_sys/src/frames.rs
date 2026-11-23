//! Reused frame context; native Metal handles never become Rust heap wrappers per frame.
use native_layer_token_sys::portable::Token;
use ogpu_core::backend::{self, Device};
use std::ffi::c_void;
use std::marker::PhantomData;
use std::rc::Rc;

unsafe extern "C" {
    fn rays_metal_frames_create(
        layer: *mut c_void,
        device: *mut c_void,
        queue: *mut c_void,
    ) -> *mut c_void;
    fn rays_metal_frames_begin(host: *mut c_void, width: u32, height: u32) -> i32;
    fn rays_metal_frames_can_draw(host: *mut c_void) -> i32;
    fn rays_metal_frames_serial(host: *mut c_void) -> u64;
    fn rays_metal_frames_completed(host: *mut c_void) -> u64;
    fn rays_metal_frames_commands(host: *mut c_void) -> *mut c_void;
    fn rays_metal_frames_pass(host: *mut c_void, red: f64, green: f64, blue: f64, x: u32, y: u32, width: u32, height: u32) -> *mut c_void;
    fn rays_metal_frames_submit(host: *mut c_void) -> i32;
    fn rays_metal_frames_capture(host: *mut c_void, on: i32) -> i32;
    fn rays_metal_frames_read(
        host: *mut c_void,
        out: *mut u8,
        length: usize,
        width: *mut u32,
        height: *mut u32,
    ) -> i32;
    fn rays_metal_frames_wait(host: *mut c_void) -> i32;
    fn rays_metal_frames_destroy(host: *mut c_void);
}

pub struct Frames {
    raw: *mut c_void,
    layer: Token,
    _thread: PhantomData<Rc<()>>,
}
impl Frames {
    pub fn new(device: &Device, layer: Token) -> Result<Self, String> {
        let pointer = layer
            .borrow_raw(layer.owner_id(), layer.generation())
            .ok_or("Window layer is unavailable")?;
        let native =
            backend::native_leased(device).map_err(|error| ogpu_core::error::to_string(&error))?;
        let raw =
            unsafe { rays_metal_frames_create(pointer.as_ptr(), native.mtl_device, native.queue) };
        if let Some(release) = native.release_owner {
            release(native.owner_token);
        }
        if raw.is_null() {
            return Err("Could not create main-thread Metal frame context".into());
        }
        Ok(Self {
            raw,
            layer,
            _thread: PhantomData,
        })
    }
    pub fn begin(&mut self, width: u32, height: u32) -> Result<bool, String> {
        if !self.layer.alive() {
            return Err("Window layer was invalidated".into());
        }
        match unsafe { rays_metal_frames_begin(self.raw, width, height) } {
            1 => Ok(true),
            0 => Ok(false),
            _ => Err("Metal frame acquisition or GPU execution failed".into()),
        }
    }
    pub fn serial(&self) -> u64 {
        unsafe { rays_metal_frames_serial(self.raw) }
    }
    pub fn can_draw(&self) -> bool {
        self.layer.alive() && unsafe { rays_metal_frames_can_draw(self.raw) } != 0
    }
    pub fn completed(&self) -> u64 {
        unsafe { rays_metal_frames_completed(self.raw) }
    }
    pub fn commands(&self) -> *mut c_void {
        unsafe { rays_metal_frames_commands(self.raw) }
    }
    /// Opens the frame's pass over the retained canvas. `damage` (`[x, y, width, height]` drawable pixels) redraws only
    /// that region: the pass keeps the last frame, its scissor is the damage and the damage is filled with `color`; the
    /// caller draws everything that intersects it. `None`, a first frame or a resize clears and redraws the whole canvas.
    pub fn pass(&mut self, color: [u8; 3], damage: Option<[u32; 4]>) -> Result<*mut c_void, String> {
        let [x, y, width, height] = damage.unwrap_or_default();
        let pass = unsafe {
            rays_metal_frames_pass(
                self.raw,
                color[0] as f64 / 255.0,
                color[1] as f64 / 255.0,
                color[2] as f64 / 255.0,
                x, y, width, height,
            )
        };
        if pass.is_null() {
            Err("Could not open Metal render pass".into())
        } else {
            Ok(pass)
        }
    }
    pub fn submit(&mut self) -> Result<bool, String> {
        match unsafe { rays_metal_frames_submit(self.raw) } {
            1 => Ok(true),
            0 => Ok(false),
            _ => Err("Could not submit Metal frame".into()),
        }
    }
    /// While on, each submitted frame is also copied for `read_capture` (drawables become blit-readable).
    pub fn capture(&mut self, on: bool) -> Result<(), String> {
        match unsafe { rays_metal_frames_capture(self.raw, i32::from(on)) } {
            1 => Ok(()),
            _ => Err("Could not change Metal frame capture between frames".into()),
        }
    }
    /// Waits for the last captured frame and returns its BGRA rows, or `None` when none was captured.
    pub fn read_capture(&mut self) -> Result<Option<(u32, u32, Vec<u8>)>, String> {
        let (mut width, mut height) = (0u32, 0u32);
        match unsafe { rays_metal_frames_read(self.raw, std::ptr::null_mut(), 0, &mut width, &mut height) } {
            0 => return Ok(None),
            1 => {}
            _ => return Err("Could not read the Metal frame capture".into()),
        }
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        match unsafe { rays_metal_frames_read(self.raw, pixels.as_mut_ptr(), pixels.len(), &mut width, &mut height) } {
            1 => Ok(Some((width, height, pixels))),
            _ => Err("Metal frame capture failed on the GPU".into()),
        }
    }
    pub fn wait(&mut self) -> Result<(), String> {
        if unsafe { rays_metal_frames_wait(self.raw) } == 1 {
            Ok(())
        } else {
            Err("Metal GPU execution failed".into())
        }
    }
}
impl Drop for Frames {
    fn drop(&mut self) {
        unsafe {
            rays_metal_frames_destroy(self.raw);
        }
    }
}
