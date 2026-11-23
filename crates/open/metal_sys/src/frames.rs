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
        full_present: i32,
        verify_present: u64,
    ) -> *mut c_void;
    fn rays_metal_frames_begin(host: *mut c_void, width: u32, height: u32) -> i32;
    fn rays_metal_frames_can_draw(host: *mut c_void) -> i32;
    fn rays_metal_frames_serial(host: *mut c_void) -> u64;
    fn rays_metal_frames_completed(host: *mut c_void) -> u64;
    fn rays_metal_frames_commands(host: *mut c_void) -> *mut c_void;
    fn rays_metal_frames_pass(host: *mut c_void, red: f64, green: f64, blue: f64, rects: *const [u32; 4], count: u32, partial: *mut u32) -> *mut c_void;
    fn rays_metal_frames_scissor(host: *mut c_void, rect: *const [u32; 4]);
    fn rays_metal_frames_cancel(host: *mut c_void);
    fn rays_metal_frames_end_pass(host: *mut c_void) -> *mut c_void;
    fn rays_metal_frames_resume_pass(host: *mut c_void) -> *mut c_void;
    fn rays_metal_frames_acquire(host: *mut c_void, wait_ns: *mut u64, history_slot: *mut i32) -> i32;
    fn rays_metal_frames_submit(host: *mut c_void, rects: *const [u32; 4], count: u32) -> i32;
    fn rays_metal_frames_full_present(host: *mut c_void, on: i32) -> i32;
    fn rays_metal_frames_verified(host: *mut c_void, failed: *mut i32) -> u64;
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
    pub fn new(device: &Device, layer: Token, full_present: bool, verify_present: u64) -> Result<Self, String> {
        let pointer = layer
            .borrow_raw(layer.owner_id(), layer.generation())
            .ok_or("Window layer is unavailable")?;
        let native =
            backend::native_leased(device).map_err(|error| ogpu_core::error::to_string(&error))?;
        let raw =
            unsafe {
                rays_metal_frames_create(
                    pointer.as_ptr(), native.mtl_device, native.queue, i32::from(full_present), verify_present,
                )
            };
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
    /// Opens the retained canvas pass. Returns its encoder and the number of damage rects to draw.
    /// Each rect is `[x, y, width, height]` in drawable pixels. All are filled in one draw and the
    /// first is set as the scissor. An empty list or invalid canvas clears the whole target and returns zero rects.
    pub fn pass(&mut self, color: [u8; 3], damage: &[[u32; 4]]) -> Result<(*mut c_void, usize), String> {
        let mut partial = 0;
        let pass = unsafe {
            rays_metal_frames_pass(
                self.raw,
                color[0] as f64 / 255.0,
                color[1] as f64 / 255.0,
                color[2] as f64 / 255.0,
                damage.as_ptr(), damage.len().min(9) as u32, &mut partial,
            )
        };
        if pass.is_null() {
            Err("Could not open Metal render pass".into())
        } else {
            Ok((pass, partial as usize))
        }
    }
    /// Ends the active render pass and borrows its canvas texture for the rest of the frame.
    pub fn end_pass(&mut self) -> Option<core::ptr::NonNull<c_void>> {
        core::ptr::NonNull::new(unsafe { rays_metal_frames_end_pass(self.raw) })
    }
    /// Starts another render pass on this frame's canvas, loading pixels from the last pass.
    pub fn resume_pass(&mut self) -> Option<core::ptr::NonNull<c_void>> {
        core::ptr::NonNull::new(unsafe { rays_metal_frames_resume_pass(self.raw) })
    }

    /// Sets the scissor to the next damage rect from `pass` for another draw.
    pub fn scissor(&mut self, rect: &[u32; 4]) {
        unsafe { rays_metal_frames_scissor(self.raw, rect); }
    }
    /// Discards an unsubmitted frame and requires a full redraw on the next pass.
    pub fn cancel(&mut self) {
        unsafe { rays_metal_frames_cancel(self.raw); }
    }

    /// Ends canvas encoding before waiting for a drawable. A missed drawable still permits a canvas commit.
    /// A history slot identifies a retained drawable. Its reset flag invalidates the whole history.
    pub fn acquire(&mut self) -> Result<(std::time::Duration, Option<(usize, bool)>), String> {
        let mut wait_ns = 0;
        let mut slot = -1;
        match unsafe { rays_metal_frames_acquire(self.raw, &mut wait_ns, &mut slot) } {
            outcome @ 0..=2 => Ok((
                std::time::Duration::from_nanos(wait_ns),
                (0..4).contains(&slot).then_some((slot as usize, outcome == 2)),
            )),
            _ => Err("Could not acquire Metal drawable".into()),
        }
    }
    /// Commits the frame; an empty damage list requests a full copy.
    pub fn submit(&mut self, damage: &[[u32; 4]]) -> Result<bool, String> {
        match unsafe { rays_metal_frames_submit(self.raw, damage.as_ptr(), damage.len().min(9) as u32) } {
            1 => Ok(true),
            0 => Ok(false),
            _ => Err("Could not submit Metal frame".into()),
        }
    }

    /// Switches presentation mode between frames.
    pub fn set_full_present(&mut self, full: bool) -> bool {
        unsafe { rays_metal_frames_full_present(self.raw, i32::from(full)) == 1 }
    }

    /// Returns completed pixel comparisons and whether partial presentation was disabled by a mismatch.
    pub fn present_verification(&self) -> (u64, bool) {
        let mut failed = 0;
        let verified = unsafe { rays_metal_frames_verified(self.raw, &mut failed) };
        (verified, failed != 0)
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
