//! Framework-owned window/GPU integration. UI calls use the public pxui API; platform backends stay here.
#[cfg(target_os = "macos")]
pub use pxui_sdl::Chrome;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use pxui_sdl::Window;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod graphics;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod images;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use graphics::Graphics;
