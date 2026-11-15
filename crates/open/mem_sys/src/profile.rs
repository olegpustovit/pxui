//! Apple thread CPU time, initial-thread QoS and optional Instruments build intervals.
//! Scopes always close their signpost; warm Rust counters do not qualify native allocations.
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn thread_cpu_time_ns() -> std::io::Result<u64> {
    unsafe extern "C" {
        fn mem_sys_thread_cpu_ns() -> u64;
    }
    let value = unsafe { mem_sys_thread_cpu_ns() };
    if value == u64::MAX {
        Err(std::io::Error::other("Thread CPU clock is unavailable"))
    } else {
        Ok(value)
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn ui_thread_priority() -> std::io::Result<()> {
    unsafe extern "C" {
        fn mem_sys_ui_thread_priority() -> i32;
    }
    match unsafe { mem_sys_ui_thread_priority() } {
        0 => Ok(()),
        code => Err(std::io::Error::from_raw_os_error(code)),
    }
}

unsafe extern "C" {
    fn mem_sys_frame_interval_begin(frame: u64) -> u64;
    fn mem_sys_frame_interval_end(interval: u64, wall: u64, cpu: u64);
}

pub struct FrameInterval {
    id: u64,
    wall: u64,
    cpu: u64,
}
impl FrameInterval {
    pub fn begin(frame: u64) -> Self {
        Self {
            id: unsafe { mem_sys_frame_interval_begin(frame) },
            wall: 0,
            cpu: 0,
        }
    }
    pub fn finish(mut self, wall_us: u64, cpu_us: u64) {
        self.wall = wall_us;
        self.cpu = cpu_us;
    }
}
impl Drop for FrameInterval {
    fn drop(&mut self) {
        unsafe { mem_sys_frame_interval_end(self.id, self.wall, self.cpu) };
    }
}
