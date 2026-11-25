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

#[test]
fn parse_atos_line_extracts_function_and_file() {
    #[cfg(all(feature = "trace", any(target_os = "macos", target_os = "ios")))]
    {
        use mem_sys::trace::parse_atos_line;
        let (f, l) = parse_atos_line("allocate_here_for_trace (in gpu_editor) (/tmp/trace.rs:12)");
        assert_eq!(f.as_deref(), Some("allocate_here_for_trace"));
        assert_eq!(l.as_deref(), Some("/tmp/trace.rs:12"));
        assert!(parse_atos_line("0x1000").0.is_none());
    }
}

/// Named site the report must surface (not `trace_log`).
#[cfg(all(feature = "trace", any(target_os = "macos", target_os = "ios")))]
#[inline(never)]
fn allocate_here_for_trace() {
    let mut v = Vec::new();
    v.reserve(16 * 1024);
    v.extend_from_slice(&[1u8; 64]);
    std::hint::black_box(v);
}

#[test]
fn known_alloc_site_is_named_in_report() {
    if !mem_sys::TRACE {
        return;
    }
    #[cfg(all(feature = "trace", any(target_os = "macos", target_os = "ios")))]
    {
        use mem_sys::trace;
        trace::clear();
        trace::start();
        allocate_here_for_trace();
        trace::stop();
        let report = trace::report(64);
        let text = trace::format_sites(&report.sites, 32);
        let named = report.sites.iter().any(|s| {
            s.frames.iter().any(|f| f.function.contains("allocate_here_for_trace"))
        }) || text.contains("allocate_here_for_trace");
        assert!(
            named,
            "expected allocate_here_for_trace in symbolised report, got:\n{text}\nsites={report:?}"
        );
        assert!(
            !text.lines().take(5).all(|l| l.contains("trace_log")),
            "top sites still headed by trace_log:\n{text}"
        );
    }
}
