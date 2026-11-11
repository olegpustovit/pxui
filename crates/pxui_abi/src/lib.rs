#[unsafe(no_mangle)]
pub extern "C" fn pxui_abi_version() -> u32 {
    1 << 16
}
