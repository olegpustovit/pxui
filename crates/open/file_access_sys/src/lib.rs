//! A chosen path with its original platform access capability. File I/O stays on the caller's worker.
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSelection {
    path: PathBuf,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    url: Option<std::sync::Arc<apple::Url>>,
}
impl FileSelection {
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// None for a plain path; Some creates a bookmark from the exact URL delivered by the picker.
    /// May contact the file provider. Call on the file worker, never while building a UI frame.
    pub fn bookmark(&self) -> std::io::Result<Option<Vec<u8>>> {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        if let Some(url) = &self.url {
            return url.bookmark().map(Some);
        }
        Ok(None)
    }
}
impl From<PathBuf> for FileSelection {
    fn from(path: PathBuf) -> Self {
        Self {
            path,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            url: None,
        }
    }
}
impl From<String> for FileSelection {
    fn from(path: String) -> Self {
        PathBuf::from(path).into()
    }
}
impl From<&str> for FileSelection {
    fn from(path: &str) -> Self {
        PathBuf::from(path).into()
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple;

/// Takes one retained NSArray<NSURL *> from a native picker, including ownership on failure.
/// # Safety
/// `raw` must be a live +1 retained NSArray containing the original picker NSURL objects.
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub unsafe fn take_apple_selection(
    raw: *mut std::ffi::c_void,
) -> std::io::Result<Vec<FileSelection>> {
    unsafe { apple::take(raw) }
}

#[cfg(all(feature = "test-util", any(target_os = "ios", target_os = "macos")))]
pub mod test_util {
    use super::FileSelection;
    use std::{
        ffi::{CString, c_char, c_void},
        io,
        path::Path,
    };
    unsafe extern "C" {
        fn pxui_selection_test_batch(path: *const c_char, identity: *mut usize) -> *mut c_void;
        fn pxui_selection_test_status(calls: *mut u32, starts: *mut u32, stops: *mut u32) -> usize;
    }
    pub fn selection(path: &Path) -> io::Result<(FileSelection, usize)> {
        let path = CString::new(path.as_os_str().as_encoded_bytes()).map_err(io::Error::other)?;
        let mut identity = 0;
        let raw = unsafe { pxui_selection_test_batch(path.as_ptr(), &mut identity) };
        let mut files = unsafe { super::take_apple_selection(raw) }?;
        Ok((files.remove(0), identity))
    }
    pub fn status() -> (usize, u32, u32, u32) {
        let (mut calls, mut starts, mut stops) = (0, 0, 0);
        let identity = unsafe { pxui_selection_test_status(&mut calls, &mut starts, &mut stops) };
        (identity, calls, starts, stops)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(feature = "test-util", any(target_os = "ios", target_os = "macos")))]
    #[test]
    fn original_url_survives_callback_batch_and_creates_bookmarks_on_the_worker() {
        let path = std::env::temp_dir().join(format!("pxui-selection-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        let (selected, identity) = test_util::selection(&path).unwrap();
        assert_eq!(selected.path(), path);
        let clone = selected.clone();
        let data = std::thread::spawn(move || selected.bookmark().unwrap().unwrap())
            .join()
            .unwrap();
        assert!(!data.is_empty());
        let (seen, calls, starts, stops) = test_util::status();
        assert_eq!(
            seen, identity,
            "bookmark creation must use the original selected NSURL"
        );
        assert_eq!(calls, 1);
        assert_eq!(starts, stops);
        assert_eq!(clone.path(), path);
        drop(clone);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn plain_paths_do_not_claim_native_permissions_and_cloning_keeps_both_paths() {
        let selected = FileSelection::from("/tmp/music");
        assert!(selected.bookmark().unwrap().is_none());
        let clone = selected.clone();
        drop(selected);
        assert_eq!(clone.path(), Path::new("/tmp/music"));
    }
}
