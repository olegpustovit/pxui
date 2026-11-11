//! OGPU's Vulkan implementation: the driver traits over the safe `vulkan` module (Windows and Linux).
pub mod reflect;
#[cfg(any(windows, target_os = "linux"))]
pub mod vulkan;
#[cfg(any(windows, target_os = "linux"))]
pub mod backend;
#[cfg(any(windows, target_os = "linux"))]
pub mod impl_;

/// `PxuiGpu.features` bits required to run PXUI's Vulkan shaders and pipelines on an adopted device.
///
/// The UI and blur SPIR-V use core storage buffers, sampled images, samplers and derivatives; they do
/// not use `Float16` or 8/16-bit storage. The rest is what OGPU records around them: dynamic rendering,
/// `vkCmdPipelineBarrier2`, timeline semaphores, push descriptors (with null descriptors for unbound
/// slots) and buffers created with `SHADER_DEVICE_ADDRESS` usage. `ogpu_vulkan` devices export the bits
/// they enabled; adoption also checks the entry points it calls are loaded.
pub const REQUIRED_FOR_UI: u64 =
    VK_DYNAMIC_RENDERING | VK_SYNCHRONIZATION2 | VK_TIMELINE_SEMAPHORE | VK_PUSH_DESCRIPTOR | VK_BUFFER_DEVICE_ADDRESS | VK_NULL_DESCRIPTOR;

pub const VK_DYNAMIC_RENDERING: u64 = 1 << 0;
pub const VK_SYNCHRONIZATION2: u64 = 1 << 1;
pub const VK_TIMELINE_SEMAPHORE: u64 = 1 << 2;
pub const VK_PUSH_DESCRIPTOR: u64 = 1 << 3;
/// `bufferDeviceAddress` (Vulkan 1.2 feature).
pub const VK_BUFFER_DEVICE_ADDRESS: u64 = 1 << 4;
/// `VK_EXT_robustness2` `nullDescriptor`.
pub const VK_NULL_DESCRIPTOR: u64 = 1 << 5;
/// Optional: `VK_EXT_memory_budget` is enabled; the adopted allocator then tracks the budget.
pub const VK_MEMORY_BUDGET: u64 = 1 << 6;

#[cfg(any(windows, target_os = "linux"))]
pub fn adopt(native: ogpu_core::backend::NativeDevice) -> Result<Box<dyn ogpu_core::backend::Driver>, ogpu_core::error::Error> {
    backend::adopt(native)
}

#[cfg(all(test, any(windows, target_os = "linux")))]
mod tests;
