// Opt-in malloc stack capture. Compiles always; the body runs only with `--features trace` on Apple.
#[cfg(not(feature = "counting"))]
#[global_allocator]
static ALLOCATOR: mem_sys::CountingAllocator = mem_sys::CountingAllocator;

#[test]
fn malloc_trace_captures_rust_alloc_when_enabled() {
    if !mem_sys::TRACE {
        return;
    }
    #[cfg(all(feature = "trace", any(target_os = "macos", target_os = "ios")))]
    {
        use mem_sys::trace::{self, Bucket};
        trace::clear();
        trace::start();
        let mut v = Vec::new();
        v.reserve(4096);
        v.push(1u8);
        trace::stop();
        let report = trace::report(64);
        assert!(
            report.totals.rust_app + report.totals.libpxui + report.totals.native > 0,
            "expected malloc events while capturing: {report:?}"
        );
        assert!(
            report.totals.rust_app > 0 || report.sites.iter().any(|s| s.bucket == Bucket::RustApp),
            "Vec reserve should classify as rust-app: totals={:?} top={}",
            report.totals,
            trace::format_sites(&report.sites, 5)
        );
        std::hint::black_box(&v);
    }
}
