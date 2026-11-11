//! The mock implementation of `ogpu_core::impl_`.
use crate::backend_mock::{self, Control};
use ogpu_core::{backend::{Driver, Implementation as Impl, NativeDevice, NATIVE_MOCK}, error::{self, ErrorKind}};
use std::ffi::c_void;
use std::cell::RefCell;

thread_local! {
    // designed limit (specification/pxui.md): controls are kept for the thread's lifetime; they are a few counters each
    static CONTROLS: RefCell<Vec<Control>> = const { RefCell::new(Vec::new()) };
}

pub const BACKEND: &str = "mock";

pub fn create_driver() -> Box<dyn Driver> {
    let (driver, control) = backend_mock::create();
    CONTROLS.with(|controls| controls.borrow_mut().push(control));
    driver
}

pub fn adopt(mut native: NativeDevice) -> Result<Box<dyn Driver>, ogpu_core::error::Error> {
    const OP: &str = "Backend_mock.adopt";
    let token = native.owner_token;
    let release = native.release_owner;
    native.owner_token = std::ptr::null_mut();
    native.release_owner = None;
    let Some(release) = release.filter(|_| !token.is_null()) else {
        return Err(error::make(OP, ErrorKind::InvalidArgument, "native device owner lease is missing"));
    };
    if native.backend != NATIVE_MOCK || native.size < std::mem::size_of::<NativeDevice>() as u32 {
        release(token);
        return Err(error::make(OP, ErrorKind::InvalidArgument, "native mock device descriptor is invalid"));
    }
    let (driver, control) = backend_mock::create_with_lease(Some((token as *mut c_void, release)));
    CONTROLS.with(|controls| controls.borrow_mut().push(control));
    Ok(driver)
}

/// Native handles still alive, summed over every driver created on this thread.
pub fn live_handles() -> i64 {
    CONTROLS.with(|controls| {
        controls
            .borrow()
            .iter()
            .map(|control| {
                let (buffers, textures, pipelines, queues, surfaces) = backend_mock::live_counts(control);
                buffers + textures + pipelines + queues + surfaces
            })
            .sum()
    })
}

/// The record `ogpu_core::impl_::select` takes.
pub fn implementation() -> Impl {
    Impl {
        backend: BACKEND,
        create_driver,
        live_handles,
    }
}
