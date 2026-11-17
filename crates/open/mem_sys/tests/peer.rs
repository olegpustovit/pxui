// mem_sys counts only with its `counting` feature; a plain `cargo t -p mem_sys` installs the counting allocator here, as an app's
// test or performance build does.
#[cfg(not(feature = "counting"))]
#[global_allocator]
static ALLOCATOR: mem_sys::CountingAllocator = mem_sys::CountingAllocator;

use mem_sys::*;

// A peer copy's numbers join this copy's (this binary's one case registers the process's one peer).
// Bounds, not equalities: the test harness's threads allocate too.
#[test]
fn a_peer_copy_adds_to_the_counters() {
    fn fake(which: u32) -> u64 { match which { COUNT_LOCAL_RESET => 0, COUNT_COUNTING => 1, _ => 1000 + u64::from(which) } }
    set_peer(fake);
    local_reset();
    assert_eq!(local_alloc_count(), own(COUNT_LOCAL_ALLOCS) + 1004);
    let before = own(COUNT_BYTES);
    assert!(alloc_bytes_total() >= before + 1002);
    let before = own(COUNT_ALLOCS);
    assert!(alloc_count() >= before + 1001);
}
