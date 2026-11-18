//! The one error of the wrapper's fallible calls.

use pxui_types::{PXUI_ERR_ARGUMENT, PXUI_ERR_GPU, PXUI_ERR_PANIC, PXUI_ERR_STALE, PXUI_ERR_STATE};

/// Why a fallible UI or host call was refused.
///
/// `Ui::last_error` gives the library's own message when the operation was tied to a Ui.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// An argument the engine rejects: out of range, non-finite, oversized or malformed (`PXUI_ERR_ARGUMENT`).
    InvalidInput,
    /// A GPU adoption or recording failure (`PXUI_ERR_GPU`).
    Gpu,
    /// The call is not allowed now: inside a frame callback, on a borrowed handle, or without its precondition (`PXUI_ERR_STATE`).
    State,
    /// A panic was caught inside the library (`PXUI_ERR_PANIC`).
    Panic,
    /// A handle names something that has gone (`PXUI_ERR_STALE`).
    Stale,
    /// The wrapper could not reserve the memory a result needs.
    Allocation,
    /// The library and wrapper use different ABI major versions.
    Version,
    /// A native host operation failed. The static message identifies the operation.
    Host(&'static str),
    /// The engine failed in a way no other variant names, including a status code this version does not know.
    Engine,
}

impl Error {
    /// The error for an ABI status code (`0`, success, is not an error and maps to `Engine`).
    ///
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// assert_eq!(pxui::Error::from_code(pxui_types::PXUI_ERR_STALE), pxui::Error::Stale);
    /// ```
    pub fn from_code(code: i32) -> Error {
        match code {
            PXUI_ERR_ARGUMENT => Error::InvalidInput,
            PXUI_ERR_GPU => Error::Gpu,
            PXUI_ERR_STATE => Error::State,
            PXUI_ERR_PANIC => Error::Panic,
            PXUI_ERR_STALE => Error::Stale,
            _ => Error::Engine,
        }
    }
}

#[cfg(test)]
impl Error {
    /// The ABI status code of this error; wrapper-raised variants report a generic refusal.
    pub(crate) fn code(self) -> i32 {
        match self {
            Error::InvalidInput => PXUI_ERR_ARGUMENT,
            Error::Gpu => PXUI_ERR_GPU,
            Error::State => PXUI_ERR_STATE,
            Error::Panic => PXUI_ERR_PANIC,
            Error::Stale => PXUI_ERR_STALE,
            Error::Allocation | Error::Version | Error::Host(_) | Error::Engine => PXUI_ERR_ARGUMENT,
        }
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Error::InvalidInput => "invalid input",
            Error::Gpu => "GPU failure",
            Error::State => "not allowed in the current state",
            Error::Panic => "panic inside the library",
            Error::Stale => "stale handle",
            Error::Allocation => "allocation failed",
            Error::Version => "incompatible pxui ABI",
            Error::Host(operation) => operation,
            Error::Engine => "engine failure",
        })
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_abi_status_code_maps_to_its_variant() {
        assert_eq!(Error::from_code(PXUI_ERR_ARGUMENT), Error::InvalidInput);
        assert_eq!(Error::from_code(PXUI_ERR_GPU), Error::Gpu);
        assert_eq!(Error::from_code(PXUI_ERR_STATE), Error::State);
        assert_eq!(Error::from_code(PXUI_ERR_PANIC), Error::Panic);
        assert_eq!(Error::from_code(PXUI_ERR_STALE), Error::Stale);
        assert_eq!(Error::from_code(-99), Error::Engine);
        for code in [PXUI_ERR_ARGUMENT, PXUI_ERR_GPU, PXUI_ERR_STATE, PXUI_ERR_PANIC, PXUI_ERR_STALE] {
            assert_eq!(Error::from_code(code).code(), code);
        }
        assert!(Error::State.to_string().contains("state"));
    }
}
