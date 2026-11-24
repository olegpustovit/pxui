//! Reused frame context; native Metal handles never become Rust heap wrappers per frame.
use native_layer_token_sys::portable::Token;
use ogpu_core::backend::{self, Device};
use std::ffi::c_void;
use std::marker::PhantomData;
use std::rc::Rc;

unsafe extern "C" {
    fn rays_metal_frames_create(layer: *mut c_void, device: *mut c_void, queue: *mut c_void) -> *mut c_void;
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
    fn rays_metal_frames_acquire(host: *mut c_void, wait_ns: *mut u64) -> i32;
    fn rays_metal_frames_submit(host: *mut c_void) -> i32;
    fn rays_metal_frames_present(host: *mut c_void) -> *mut c_void;
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
        let raw = unsafe { rays_metal_frames_create(pointer.as_ptr(), native.mtl_device, native.queue) };
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
    /// Each rect is `[x, y, width, height]` in drawable pixels. The first is already filled and
    /// scissored. An empty list or invalid canvas clears the whole target and returns zero rects.
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

    /// Fills the next damage rect from `pass` and sets its scissor for another draw.
    pub fn scissor(&mut self, rect: &[u32; 4]) {
        unsafe { rays_metal_frames_scissor(self.raw, rect); }
    }
    /// Discards an unsubmitted frame and requires a full redraw on the next pass.
    pub fn cancel(&mut self) {
        unsafe { rays_metal_frames_cancel(self.raw); }
    }

    /// Ends canvas encoding before waiting for a drawable. A missed drawable still permits a canvas commit.
    pub fn acquire(&mut self) -> Result<(bool, std::time::Duration), String> {
        let mut wait_ns = 0;
        match unsafe { rays_metal_frames_acquire(self.raw, &mut wait_ns) } {
            status @ (0 | 1) => Ok((status == 1, std::time::Duration::from_nanos(wait_ns))),
            _ => Err("Could not acquire Metal drawable".into()),
        }
    }
    /// Copies the retained canvas into the acquired drawable and leaves the encoder open.
    pub fn present_pass(&mut self) -> Option<core::ptr::NonNull<c_void>> {
        core::ptr::NonNull::new(unsafe { rays_metal_frames_present(self.raw) })
    }

    /// Commits the frame and presents the drawable after its pass ends.
    pub fn submit(&mut self) -> Result<bool, String> {
        match unsafe { rays_metal_frames_submit(self.raw) } {
            1 => Ok(true),
            0 => Ok(false),
            _ => Err("Could not submit Metal frame".into()),
        }
    }

    #[cfg(feature = "test-util")]
    pub fn test_link_state(&self) -> (bool, bool) {
        unsafe extern "C" {
            fn rays_metal_frames_test_link_state(raw: *mut c_void, loan: *mut bool) -> bool;
        }
        let mut loan = false;
        let owned = unsafe { rays_metal_frames_test_link_state(self.raw, &mut loan) };
        (owned, loan)
    }

    /// While on, copy the presented drawable or the linked surface's canvas for `read_capture`.
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
