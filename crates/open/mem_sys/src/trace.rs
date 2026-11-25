//! Opt-in process-wide malloc stack capture (`trace` feature, Apple malloc_logger).
//! Zero cost in the default build: the C hook is not compiled and this module is absent.
//!
//! Capture stores raw return addresses only (allocation-free). Symbolisation runs at
//! [`report`] time: `dladdr` for image/load base, then `atos -p <pid>` against the live
//! process slides for demangled names and `file:line`. Allocator and trace_log frames are
//! skipped when naming a site.

use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::fmt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

const DEPTH: usize = 64;

#[repr(C)]
struct Raw {
    count: u64,
    bytes: u64,
    n: u32,
    main: u8,
    kind: u8,
    _pad: [u8; 2],
    pc: [usize; DEPTH],
}

unsafe extern "C" {
    fn mem_sys_trace_start();
    fn mem_sys_trace_stop();
    fn mem_sys_trace_resume();
    fn mem_sys_trace_clear();
    fn mem_sys_trace_capturing() -> i32;
    fn mem_sys_trace_dropped() -> u64;
    fn mem_sys_trace_snapshot(out: *mut Raw, max: i32) -> i32;
    fn mem_sys_trace_slot_count() -> i32;
}

static STARTED: AtomicBool = AtomicBool::new(false);

/// Whether this build compiled the malloc-logger hook (the `trace` feature on Apple).
pub const ENABLED: bool = cfg!(all(feature = "trace", any(target_os = "macos", target_os = "ios")));

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bucket {
    /// Rust allocator in the main binary (Rays + statically linked open tier).
    RustApp,
    /// Rust allocator inside `libpxui`.
    LibPxui,
    /// ObjC / C++ / system / driver malloc (no `__rust_alloc` on the stack).
    Native,
}

impl fmt::Display for Bucket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Bucket::RustApp => "rust-app",
            Bucket::LibPxui => "libpxui",
            Bucket::Native => "native",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub address: usize,
    pub load_base: usize,
    pub function: String,
    pub image: String,
    /// `file:line` when resolved; empty otherwise.
    pub location: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub count: u64,
    pub bytes: u64,
    pub bucket: Bucket,
    pub main_thread: bool,
    pub frames: Vec<Frame>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub rust_app: u64,
    pub libpxui: u64,
    pub native: u64,
    pub bytes_rust_app: u64,
    pub bytes_libpxui: u64,
    pub bytes_native: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub sites: Vec<Site>,
    pub totals: Totals,
    pub dropped: u64,
}

/// Install the malloc logger (once), clear the table, and start capturing.
pub fn start() {
    if !ENABLED {
        return;
    }
    unsafe { mem_sys_trace_start() };
    STARTED.store(true, Ordering::Relaxed);
}

/// Stop capturing. The logger stays installed until process exit; further [`start`]s clear and resume.
pub fn stop() {
    if !ENABLED {
        return;
    }
    unsafe { mem_sys_trace_stop() };
}

/// Resume capturing without clearing aggregated sites. No-op until the first [`start`].
pub fn resume() {
    if !ENABLED || !STARTED.load(Ordering::Relaxed) {
        return;
    }
    unsafe { mem_sys_trace_resume() };
}

/// Clear aggregated sites without changing the capture flag.
pub fn clear() {
    if !ENABLED {
        return;
    }
    unsafe { mem_sys_trace_clear() };
}

pub fn capturing() -> bool {
    ENABLED && unsafe { mem_sys_trace_capturing() != 0 }
}

/// Symbolise and classify captured sites, sorted by count descending. Stops capture first.
/// `top` keeps that many sites (0 = all).
pub fn report(top: usize) -> Report {
    let (mut sites, totals, dropped) = collect();
    sites.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| b.bytes.cmp(&a.bytes)));
    if top > 0 && sites.len() > top {
        sites.truncate(top);
    }
    enrich_atos(&mut sites);
    Report { sites, totals, dropped }
}

/// Counts only (dladdr for classification, no `atos`). Stops capture first.
pub fn totals() -> Totals {
    collect().1
}

fn collect() -> (Vec<Site>, Totals, u64) {
    if !ENABLED || !STARTED.load(Ordering::Relaxed) {
        return (Vec::new(), Totals::default(), 0);
    }
    let max = unsafe { mem_sys_trace_slot_count() }.max(0) as usize;
    let dropped = unsafe { mem_sys_trace_dropped() };
    let mut raw = Vec::with_capacity(max);
    raw.resize_with(max, || Raw {
        count: 0,
        bytes: 0,
        n: 0,
        main: 0,
        kind: 0,
        _pad: [0; 2],
        pc: [0; DEPTH],
    });
    let n = unsafe { mem_sys_trace_snapshot(raw.as_mut_ptr(), max as i32) }.max(0) as usize;
    raw.truncate(n);

    let mut cache: HashMap<usize, Frame> = HashMap::new();
    let mut sites = Vec::with_capacity(n);
    let mut totals = Totals::default();
    for entry in &raw {
        let depth = (entry.n as usize).min(DEPTH);
        let mut frames = Vec::with_capacity(depth);
        for &pc in entry.pc.iter().take(depth) {
            if pc == 0 {
                continue;
            }
            let frame = cache.entry(pc).or_insert_with(|| resolve_dladdr(pc)).clone();
            frames.push(frame);
        }
        let bucket = classify(&frames);
        match bucket {
            Bucket::RustApp => {
                totals.rust_app += entry.count;
                totals.bytes_rust_app += entry.bytes;
            }
            Bucket::LibPxui => {
                totals.libpxui += entry.count;
                totals.bytes_libpxui += entry.bytes;
            }
            Bucket::Native => {
                totals.native += entry.count;
                totals.bytes_native += entry.bytes;
            }
        }
        sites.push(Site {
            count: entry.count,
            bytes: entry.bytes,
            bucket,
            main_thread: entry.main != 0,
            frames,
        });
    }
    (sites, totals, dropped)
}

/// Format the top sites for a failure message or the RAYS_ZERO_ALLOC log.
pub fn format_sites(sites: &[Site], limit: usize) -> String {
    let mut out = String::new();
    for (i, site) in sites.iter().take(limit).enumerate() {
        let head = site
            .frames
            .iter()
            .find(|f| is_app_frame(f))
            .or_else(|| site.frames.iter().find(|f| !is_noise_frame(f)))
            .or_else(|| site.frames.first());
        let via = site
            .frames
            .iter()
            .find(|f| is_alloc_frame(f))
            .map(|f| f.function.as_str())
            .unwrap_or("?");
        let (func, loc) = match head {
            Some(f) => (
                f.function.as_str(),
                if f.location.is_empty() {
                    f.image.as_str()
                } else {
                    f.location.as_str()
                },
            ),
            None => ("?", "?"),
        };
        out.push_str(&format!(
            "  #{:<2} {:>6}× {:<10} {} ({}) via {}\n",
            i + 1,
            site.count,
            site.bucket,
            func,
            loc,
            via
        ));
    }
    out
}

fn classify(frames: &[Frame]) -> Bucket {
    match frames.iter().position(is_alloc_frame) {
        Some(i) => {
            let image = frames.get(i).map(|f| f.image.as_str()).unwrap_or("");
            if image.contains("libpxui") {
                Bucket::LibPxui
            } else {
                Bucket::RustApp
            }
        }
        None => Bucket::Native,
    }
}

fn is_alloc_frame(f: &Frame) -> bool {
    let n = f.function.as_str();
    // `__rust_alloc` / `__rdl_*` when the default shim is on the stack; with our counting
    // `GlobalAlloc` the unwind often lands on `CountingAllocator::alloc` instead (opt-level 1).
    n.contains("__rust_alloc")
        || n.contains("__rust_realloc")
        || n.contains("__rdl_alloc")
        || n.contains("__rdl_realloc")
        || n.contains("CountingAllocator")
        || n.contains("GlobalAlloc5alloc")
        || n.contains("GlobalAlloc8realloc")
        || n.contains("GlobalAlloc12alloc_zeroed")
}

fn is_trace_frame(f: &Frame) -> bool {
    let n = f.function.as_str();
    n == "trace_log"
        || n.starts_with("mem_sys_trace_")
        || n == "capture_stack"
        || n == "unwind_fill"
        || n.contains("_Unwind_")
        || n.contains("malloc_logger")
}

fn is_noise_frame(f: &Frame) -> bool {
    is_alloc_frame(f)
        || is_trace_frame(f)
        || {
            let n = f.function.as_str();
            // Demangled forms may be `<alloc::sync::Arc<…>>::…` — match the crate path inside.
            n.contains("alloc::")
                || n.contains("hashbrown::")
                || n.starts_with("std::")
                || n.starts_with("core::")
                || n.contains("core::")
                || n.starts_with("_malloc")
                || n.starts_with("malloc")
                || n.starts_with("calloc")
                || n.starts_with("realloc")
                || n == "?"
                || n.starts_with("0x")
        }
}

fn is_app_frame(f: &Frame) -> bool {
    if is_noise_frame(f) {
        return false;
    }
    let img = f.image.as_str();
    // mem_sys itself is the tracer; never pick it as the site head.
    if img.contains("mem_sys") {
        return false;
    }
    img.contains("rays")
        || img.contains("libpxui")
        || img.contains("gpu_editor")
        || img.contains("ogpu_")
        || (!img.contains("/usr/") && !img.contains("/System/") && !img.is_empty() && img != "?")
}

fn resolve_dladdr(pc: usize) -> Frame {
    #[repr(C)]
    struct DlInfo {
        fname: *const c_char,
        fbase: *mut u8,
        sname: *const c_char,
        saddr: *mut u8,
    }
    unsafe extern "C" {
        fn dladdr(addr: *const u8, info: *mut DlInfo) -> i32;
    }
    let mut info = DlInfo {
        fname: std::ptr::null(),
        fbase: std::ptr::null_mut(),
        sname: std::ptr::null(),
        saddr: std::ptr::null_mut(),
    };
    // Return addresses point one past the call; step back for the call site when resolving.
    let addr = pc.saturating_sub(1);
    let ok = unsafe { dladdr(addr as *const u8, &mut info) } != 0;
    let function = if ok && !info.sname.is_null() {
        unsafe { CStr::from_ptr(info.sname) }.to_string_lossy().into_owned()
    } else {
        "?".into()
    };
    let image = if ok && !info.fname.is_null() {
        unsafe { CStr::from_ptr(info.fname) }.to_string_lossy().into_owned()
    } else {
        "?".into()
    };
    let load_base = if ok { info.fbase as usize } else { 0 };
    Frame {
        address: pc,
        load_base,
        function,
        image,
        location: String::new(),
    }
}

/// Resolve demangled names and file:line via `atos -p` against this process's loaded slides.
fn enrich_atos(sites: &mut [Site]) {
    let mut order: Vec<usize> = Vec::new();
    let mut index: HashMap<usize, usize> = HashMap::new();
    for site in sites.iter() {
        for frame in &site.frames {
            if frame.address == 0 {
                continue;
            }
            index.entry(frame.address).or_insert_with(|| {
                let i = order.len();
                order.push(frame.address);
                i
            });
        }
    }
    if order.is_empty() {
        return;
    }
    let pid = std::process::id().to_string();
    let mut resolved: Vec<(Option<String>, Option<String>)> = vec![(None, None); order.len()];
    for (chunk_i, chunk) in order.chunks(64).enumerate() {
        let mut cmd = Command::new("atos");
        cmd.arg("-p").arg(&pid);
        for &pc in chunk {
            // Call-site address (return address minus one).
            cmd.arg(format!("{:#x}", pc.saturating_sub(1)));
        }
        let Ok(output) = cmd.output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        for (line_i, line) in text.lines().enumerate() {
            let at = chunk_i * 64 + line_i;
            if at >= resolved.len() {
                break;
            }
            resolved[at] = parse_atos_line(line);
        }
    }
    for site in sites.iter_mut() {
        for frame in &mut site.frames {
            let Some(&i) = index.get(&frame.address) else {
                continue;
            };
            let (func, loc) = &resolved[i];
            if let Some(name) = func {
                if !name.is_empty() && !name.starts_with("0x") {
                    frame.function = name.clone();
                }
            }
            if let Some(path) = loc {
                if !path.is_empty() {
                    frame.location = path.clone();
                }
            }
            // atos may leave Rust v0 linkage names; demangle for the report.
            frame.function = demangle(&frame.function);
        }
    }
}

/// Demangle Rust v0 / Itanium names via `c++filt` (Apple's c++filt understands `_R…`).
fn demangle(name: &str) -> String {
    if !(name.starts_with("_R") || name.starts_with("_ZN") || name.starts_with("__Z")) {
        return name.to_owned();
    }
    let Ok(output) = Command::new("c++filt").arg("-n").arg(name).output() else {
        return name.to_owned();
    };
    if !output.status.success() {
        return name.to_owned();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let demangled = text.trim();
    if demangled.is_empty() || demangled == name {
        name.to_owned()
    } else {
        demangled.to_owned()
    }
}

/// `function (in image) (file:line)` or `function (in image)` or bare `0x…`.
pub fn parse_atos_line(line: &str) -> (Option<String>, Option<String>) {
    let line = line.trim();
    if line.is_empty() || line == "-" || line.starts_with("0x") || line.starts_with("0X") {
        return (None, None);
    }
    let func = line
        .split(" (in ")
        .next()
        .unwrap_or(line)
        .trim()
        .to_owned();
    let location = line.rfind('(').and_then(|start| {
        let inside = line[start + 1..].trim_end_matches(')');
        if inside.starts_with("in ") {
            return None;
        }
        if inside.contains(':') {
            Some(inside.to_owned())
        } else {
            None
        }
    });
    (Some(func), location)
}
