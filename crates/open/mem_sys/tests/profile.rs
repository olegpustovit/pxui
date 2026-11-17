#![cfg(any(target_os = "macos", target_os = "ios"))]
// mem_sys counts only with its `counting` feature; a plain `cargo t -p mem_sys` installs the counting allocator here.
#[cfg(not(feature = "counting"))]
#[global_allocator]
static ALLOCATOR: mem_sys::CountingAllocator = mem_sys::CountingAllocator;
use mem_sys::profile::{FrameInterval, thread_cpu_time_ns, ui_thread_priority};

#[test]
fn ui_priority_rejects_worker_threads() {
    std::thread::spawn(|| {
        assert_eq!(
            ui_thread_priority().unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
    })
    .join()
    .unwrap();
}

#[test]
fn native_frame_intervals_and_thread_cpu_clock_allocate_no_warm_rust_storage() {
    drop(FrameInterval::begin(0));
    let mut previous = thread_cpu_time_ns().unwrap();
    mem_sys::local_reset();
    for frame in 1..101 {
        let current = thread_cpu_time_ns().unwrap();
        assert!(current >= previous);
        FrameInterval::begin(frame)
            .finish((current - previous) / 1000, (current - previous) / 1000);
        drop(FrameInterval::begin(frame));
        previous = current;
    }
    assert_eq!(mem_sys::local_alloc_count(), 0);
}
