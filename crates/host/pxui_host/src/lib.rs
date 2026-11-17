//! Framework-owned window/GPU integration. UI calls use the public pxui API; platform backends stay here.
//!
//! Ownership is per surface: one `Window` (an SDL window, routed its own events by `pxui_sdl`), one
//! `Ui`, one `Platform`, one `Graphics` and, on macOS, one `Accessibility` serve one surface; a
//! multi-window app makes one set per window and never shares a `Platform` or `Graphics` between them.
//!
//! Threading: every type here, the windows and all GPU work belong to the initial thread (SDL's and
//! AppKit/UIKit's main thread); none is `Send`. Workers hand decoded pixels to the main thread, which
//! calls `Graphics::upload_image`; `pxui_sdl::wake` is the only call another thread may make.
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod events;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use events::Events;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod platform;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use platform::Platform;
#[cfg(target_os = "macos")]
pub use pxui_sdl::Chrome;
#[cfg(target_os = "macos")]
mod accessibility;
#[cfg(target_os = "macos")]
pub use accessibility::Accessibility;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use pxui_sdl::Window;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod graphics;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod images;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use graphics::Graphics;

#[cfg(test)]
mod window_tests;
#[cfg(test)]
fn main() { window_tests::main(); }
