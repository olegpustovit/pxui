use super::FileSelection;
use std::{
    ffi::{CStr, c_char, c_void},
    io,
    path::PathBuf,
    ptr::NonNull,
    sync::Arc,
};

unsafe extern "C" {
    fn pxui_selection_count(batch: *mut c_void) -> usize;
    fn pxui_selection_url(batch: *mut c_void, index: usize) -> *mut c_void;
    fn pxui_selection_path(url: *mut c_void) -> *mut c_char;
    fn pxui_selection_bookmark(
        url: *mut c_void,
        data: *mut *mut u8,
        len: *mut usize,
        error: *mut *mut c_char,
    ) -> bool;
    fn pxui_selection_release(value: *mut c_void);
    fn pxui_selection_free(value: *mut c_void);
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Url(NonNull<c_void>);
// NSURL is immutable and thread safe. Each bookmark operation balances its own security scope.
unsafe impl Send for Url {}
unsafe impl Sync for Url {}
impl Drop for Url {
    fn drop(&mut self) {
        unsafe { pxui_selection_release(self.0.as_ptr()) };
    }
}
impl Url {
    pub(super) fn bookmark(&self) -> io::Result<Vec<u8>> {
        let (mut data, mut len, mut error) = (std::ptr::null_mut(), 0, std::ptr::null_mut());
        if !unsafe { pxui_selection_bookmark(self.0.as_ptr(), &mut data, &mut len, &mut error) } {
            let message = if error.is_null() {
                "Could not bookmark the selected URL".to_owned()
            } else {
                unsafe { CStr::from_ptr(error) }
                    .to_string_lossy()
                    .into_owned()
            };
            unsafe { pxui_selection_free(error.cast()) };
            return Err(io::Error::other(message));
        }
        let bytes = unsafe { std::slice::from_raw_parts(data, len) }.to_vec();
        unsafe { pxui_selection_free(data.cast()) };
        Ok(bytes)
    }
}

pub(super) unsafe fn take(raw: *mut c_void) -> io::Result<Vec<FileSelection>> {
    let batch =
        Url(NonNull::new(raw).ok_or_else(|| io::Error::other("Missing picker URL capability"))?);
    let count = unsafe { pxui_selection_count(batch.0.as_ptr()) };
    if count == 0 || count > 4096 {
        return Err(io::Error::other("Invalid picker URL count"));
    }
    let mut selected = Vec::with_capacity(count);
    let mut bytes = 0usize;
    for index in 0..count {
        let raw = unsafe { pxui_selection_url(batch.0.as_ptr(), index) };
        let url = Url(NonNull::new(raw)
            .ok_or_else(|| io::Error::other("Picker returned an invalid file URL"))?);
        let path = unsafe { pxui_selection_path(url.0.as_ptr()) };
        if path.is_null() {
            return Err(io::Error::other(
                "Picker URL has no valid absolute UTF-8 path",
            ));
        }
        let decoded = unsafe { CStr::from_ptr(path) }
            .to_str()
            .map(PathBuf::from)
            .map_err(io::Error::other);
        unsafe { pxui_selection_free(path.cast()) };
        let path = decoded?;
        bytes = bytes.saturating_add(path.as_os_str().len());
        if !path.is_absolute() || bytes > 1024 * 1024 {
            return Err(io::Error::other("Picker paths exceed their memory limit"));
        }
        selected.push(FileSelection {
            path,
            url: Some(Arc::new(url)),
        });
    }
    Ok(selected)
}
