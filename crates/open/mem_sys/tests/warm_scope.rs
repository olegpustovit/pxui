// mem_sys counts only with its `counting` feature; a plain `cargo t -p mem_sys` installs the counting allocator here, as an app's
// test or performance build does.
#[cfg(not(feature = "counting"))]
#[global_allocator]
static ALLOCATOR: mem_sys::CountingAllocator = mem_sys::CountingAllocator;

// One test: reports are process-wide, so concurrent scopes in sibling tests would mix.
#[test]
fn warm_scope_reports_allocations_only() {
    mem_sys::local_reset();
    let scope = mem_sys::warm_scope();
    drop(scope);
    assert_eq!(mem_sys::local_alloc_count(), 0);
    assert!(mem_sys::take_reports().is_empty());

    let scope = mem_sys::warm_scope();
    let mut v = Vec::new();
    v.push(1u64);
    drop(scope);
    let reports = mem_sys::take_reports();
    assert_eq!(reports.len(), 1);
    assert!(reports[0].contains("warm_scope_reports_allocations_only"), "{}", reports[0]);
    assert_eq!(v.len(), 1);
}
