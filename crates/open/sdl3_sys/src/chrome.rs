//! Main-thread AppKit chrome over a live framework window layer.
use native_layer_token_sys::portable::Token;
use std::{ffi::c_void, io, marker::PhantomData, rc::Rc};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub top_inset: f64,
    pub content_width: f64,
    pub content_height: f64,
    pub resizable: u32,
    pub zoom_enabled: u32,
    pub full_size_content: u32,
    pub full_screen_primary: u32,
}

pub struct Chrome {
    layer: Token,
    _main: PhantomData<Rc<()>>,
}
impl Chrome {
    pub fn open(layer: Token, minimum: (f64, f64)) -> io::Result<Self> {
        if !minimum.0.is_finite() || !minimum.1.is_finite() || minimum.0 <= 0.0 || minimum.1 <= 0.0
        {
            return Err(unavailable());
        }
        let raw = layer
            .borrow_raw(layer.owner_id(), layer.generation())
            .ok_or_else(unavailable)?;
        if unsafe { rays_sdl3_chrome_configure(raw.as_ptr(), minimum.0, minimum.1) } == 0 {
            return Err(unavailable());
        }
        Ok(Self {
            layer,
            _main: PhantomData,
        })
    }
    pub fn snapshot(&self) -> io::Result<Snapshot> {
        let raw = self
            .layer
            .borrow_raw(self.layer.owner_id(), self.layer.generation())
            .ok_or_else(unavailable)?;
        let mut state = Snapshot::default();
        if unsafe { rays_sdl3_chrome_read(raw.as_ptr(), &mut state) } == 0
            || !state.top_inset.is_finite()
            || state.top_inset < 0.0
        {
            return Err(unavailable());
        }
        Ok(state)
    }
}
fn unavailable() -> io::Error {
    io::Error::other("Window chrome needs a live main-thread AppKit view")
}
unsafe extern "C" {
    fn rays_sdl3_chrome_configure(
        layer: *mut c_void,
        minimum_width: f64,
        minimum_height: f64,
    ) -> i32;
    fn rays_sdl3_chrome_read(layer: *mut c_void, state: *mut Snapshot) -> i32;
}
