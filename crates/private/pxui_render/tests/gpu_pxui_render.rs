//! Offscreen painter cases on Metal (macOS) or fatal-validation Vulkan (Windows): one golden scene
//! with every instance kind, analytic spot checks, the one-draw-per-layer rule and a zero-allocation
//! warm frame. `RAYS_UPDATE_FIXTURES=1` rewrites the golden.
use mem_sys as _; // the counting global allocator
use ogpu_core::backend::{self as be, ColorAttachment, Device, Queue, RenderEncoder, RenderTarget, Texture};
use ogpu_core::error::{self, Error};
use ogpu_core::pipeline::{ColorFormat, DepthFormat};
use ogpu_core::render_pass::{Load, Store};
use ogpu_core::types::{Extent, Origin, TextureDescriptor, TextureFormat, TextureUsage};
use pxui_core::draw::{fit, nine_slice, Fit};
use pxui_render::{flags, ramp, AtlasKind, Break, Instance, Painter, View};
#[path = "../../../../tools/t/png.rs"]
mod png;

const W: u32 = 128;
const H: u32 = 96;
const SCALE: u32 = 2;
const BG: [u8; 4] = [204, 204, 204, 255];

fn ok<T>(result: Result<T, Error>) -> T { result.unwrap_or_else(|e| panic!("{}", error::to_string(&e))) }

/// A device on the platform backend, or None (reported as a skip) where neither exists.
fn open() -> Option<Device> {
    #[cfg(any(windows, target_os = "linux"))]
    ogpu_core::impl_::select(ogpu_core::impl_::Impl {
        backend: ogpu_vulkan::impl_::BACKEND,
        create_driver: || ogpu_vulkan::backend::create_with_validation((true, true)),
        live_handles: ogpu_vulkan::impl_::live_handles,
    });
    #[cfg(target_os = "macos")]
    ogpu_core::impl_::select(ogpu_metal::impl_::implementation());
    if !cfg!(any(windows, target_os = "linux", target_os = "macos")) {
        println!("RAYS_TEST_SKIP: no native OGPU implementation on this platform");
        assert!(std::env::var("RAYS_ALLOW_SKIP").as_deref() == Ok("1"), "RAYS_ALLOW_SKIP=1 explicitly permits skipping");
        return None;
    }
    Some(ok(be::create_device(ok(ogpu_core::impl_::create_driver()))))
}

fn texture(device: &Device, width: u32, height: u32, usage: &[TextureUsage]) -> Texture {
    ok(be::create_texture(device, TextureDescriptor {
        label: Some("pxui_render.test".into()), width: width as i64, height: height as i64, depth: 1, mip_levels: 1,
        sample_count: 1, format: TextureFormat::Rgba8Unorm, usage: usage.to_vec(),
    }))
}

fn target(device: &Device) -> Texture {
    texture(device, W * SCALE, H * SCALE, &[TextureUsage::RenderAttachment, TextureUsage::TextureCopySrc, TextureUsage::TextureBinding])
}

/// Open a clearing pass on `target`, run `body`, submit and wait.
fn pass(queue: &Queue, target: &Texture, body: &mut dyn FnMut(&RenderEncoder)) { pass_with(queue, target, Load::Clear, body) }

fn pass_with(queue: &Queue, target: &Texture, load: Load, body: &mut dyn FnMut(&RenderEncoder)) {
    let commands = ok(be::begin_commands(queue));
    let clear = (BG[0] as f64 / 255.0, BG[1] as f64 / 255.0, BG[2] as f64 / 255.0, 1.0);
    let encoder = ok(be::render_encoder(&commands, &RenderTarget {
        colors: vec![ColorAttachment { texture: target.clone(), resolve: None, load, store: Store::Store, clear }],
        depth: None,
    }));
    body(&encoder);
    ok(be::end_render(&encoder));
    let receipt = ok(be::commit(&commands));
    ok(be::complete_through(queue, receipt.epoch));
}

fn painter(device: &Device) -> Painter {
    ok(Painter::new(device, ColorFormat::Rgba8Unorm, DepthFormat::NoDepth, 1, (64, 64), (256, 32)))
}

/// Synthetic atlas content: an 8x8 ring glyph in the mask atlas at (16, 16), a 4x4 checker at (0, 0)
/// of the colour atlas.
fn upload_atlases(painter: &mut Painter, glyph: &[u8], checker: &[u8]) {
    ok(painter.upload(AtlasKind::Mask, 16, 16, 8, 8, glyph, 8));
    ok(painter.upload(AtlasKind::Color, 0, 0, 4, 4, checker, 16));
    let mut row = [0u8; 1024];
    ramp(&[(0.0, RED), (0.5, GREEN), (1.0, BLUE)], &mut row);
    ok(painter.upload(AtlasKind::Color, 0, 8, 256, 1, &row, 1024));
}

/// Views the scene names: 1 fades and shifts, 2 turns a little about its rect's centre.
fn set_scene_views(painter: &mut Painter) {
    let faded = View { offset: [8.0, 0.0], opacity: 0.5, ..View::IDENTITY };
    ok(painter.set_view(1, faded));
    ok(painter.set_view(2, View::rotation(0.3, [117.0, 88.0])));
}

fn atlas_bytes() -> (Vec<u8>, Vec<u8>) {
    let glyph = (0..64).map(|i| {
        let (x, y) = ((i % 8) as f32 - 3.5, (i / 8) as f32 - 3.5);
        let d = (x * x + y * y).sqrt();
        if (1.5..3.5).contains(&d) { 255 } else { 0 }
    }).collect();
    let checker = (0..16).flat_map(|i| if (i % 4 + i / 4) % 2 == 0 { [255, 255, 255, 255] } else { [20, 40, 160, 255] }).collect();
    (glyph, checker)
}

const RED: [u8; 4] = [220, 40, 40, 255];
const BLUE: [u8; 4] = [40, 80, 220, 255];
const GREEN: [u8; 4] = [30, 170, 70, 255];
const INK: [u8; 4] = [30, 30, 40, 255];

/// Every kind in a 128x96-point scene.
fn scene() -> Vec<Instance> {
    let tau = std::f32::consts::TAU;
    vec![
        Instance::rect([4.0, 4.0, 28.0, 20.0], RED),
        Instance::rect([32.0, 4.0, 60.0, 20.0], BLUE).radii([0.0, 4.0, 8.0, 2.0]),
        Instance::rect([64.0, 4.0, 92.0, 20.0], [250, 210, 60, 255]).radii([4.0; 4]).border(2.0, INK),
        Instance::rect([96.0, 4.0, 124.0, 20.0], RED).linear([0.0, 0.5], [1.0, 0.5], BLUE),
        Instance::rect([4.0, 24.0, 28.0, 48.0], GREEN).radial([0.5, 0.5], [0.5, 0.5], [200, 40, 200, 255]),
        Instance::rect([32.0, 24.0, 56.0, 48.0], RED).radii([12.0; 4]).conic([0.5, 0.5], 0.0, BLUE),
        Instance::shadow([64.0, 26.0, 84.0, 42.0], [4.0; 4], [0, 0, 0, 160], [2.0, 2.0], 3.0, 0.0, false),
        Instance::rect([64.0, 26.0, 84.0, 42.0], [255, 255, 255, 255]).radii([4.0; 4]),
        Instance::rect([96.0, 26.0, 120.0, 44.0], [255, 255, 255, 255]).radii([6.0; 4]),
        Instance::shadow([96.0, 26.0, 120.0, 44.0], [6.0; 4], [0, 0, 0, 200], [2.0, 2.0], 2.0, 1.0, true),
        Instance::line([4.0, 54.0], [40.0, 60.0], 2.0, INK),
        Instance::line([4.0, 64.0], [40.0, 64.0], 2.0, BLUE).dash(4.0, 2.0, 0.0).round_caps(),
        Instance::arc([56.0, 62.0], 8.0, 0.0, 0.7 * tau, 2.0, RED),
        Instance::arc([56.0, 62.0], 4.0, 0.0, -0.5 * tau, 1.0, BLUE).dash(2.0, 1.5, 0.0),
        Instance::curve([72.0, 68.0], [82.0, 48.0], [92.0, 68.0], 1.5, GREEN),
        Instance::mask([100.0, 52.0, 116.0, 68.0], [16.0 / 64.0, 16.0 / 64.0, 24.0 / 64.0, 24.0 / 64.0], INK),
        Instance::image([4.0, 72.0, 20.0, 88.0], [0.0, 0.0, 4.0 / 256.0, 4.0 / 32.0], [255, 255, 255, 255]),
        Instance::rect([24.0, 72.0, 56.0, 92.0], GREEN).clip([30, 76, 50, 88], 4.0),
        Instance::rect([60.0, 72.0, 80.0, 92.0], BLUE).view(1),
        Instance::rect([92.0, 72.0, 106.0, 92.0], [255, 255, 255, 255]).border(3.0, INK).sides(flags::BOTTOM),
        Instance::rect([108.0, 72.0, 126.0, 80.0], [0; 4]).linear([0.0, 0.5], [1.0, 0.5], [0; 4]).ramp(8),
        Instance::rect([108.0, 84.0, 126.0, 92.0], BLUE).view(2),
    ]
}

fn render_scene(device: &Device, queue: &Queue) -> Vec<u8> {
    let mut painter = painter(device);
    let (glyph, checker) = atlas_bytes();
    upload_atlases(&mut painter, &glyph, &checker);
    set_scene_views(&mut painter);
    let target = target(device);
    let instances = scene();
    painter.begin_frame();
    pass(queue, &target, &mut |encoder| ok(painter.draw(encoder, [W * SCALE, H * SCALE], SCALE as f32, &instances, &[])));
    assert_eq!(painter.draw_calls(), 1, "a layer without breaks is one draw call");
    let pixels = ok(be::read_texture(&target, (W * SCALE * 4) as i64));
    ok(be::destroy_texture(&target));
    pixels
}

fn finish(device: Device, queue: Queue) {
    ok(be::destroy_queue(&queue));
    ok(be::destroy_device(&device));
}

/// The pixel under logical point (x, y).
fn at(pixels: &[u8], x: f32, y: f32) -> [u8; 4] {
    let (px, py) = ((x * SCALE as f32) as usize, (y * SCALE as f32) as usize);
    let i = (py * (W * SCALE) as usize + px) * 4;
    pixels[i..i + 4].try_into().unwrap()
}

fn near(a: [u8; 4], b: [u8; 4], tolerance: i32) -> bool { a.iter().zip(b).all(|(x, y)| (*x as i32 - y as i32).abs() <= tolerance) }

/// Metal handles live on the process main thread, which libtest never gives a test: this target is
/// `harness = false` and speaks just enough of libtest's protocol (`--list`, names to filter).
fn main() {
    let cases: [(&str, fn()); 20] = [
        ("gpu_scene_golden_and_spot_checks", gpu_scene_golden_and_spot_checks),
        ("gpu_one_draw_per_layer", gpu_one_draw_per_layer),
        ("gpu_warm_frame_allocates_nothing", gpu_warm_frame_allocates_nothing),
        ("gpu_view_scale_and_rotation", gpu_view_scale_and_rotation),
        ("gpu_per_side_borders", gpu_per_side_borders),
        ("gpu_ramp_gradient", gpu_ramp_gradient),
        ("gpu_retained_layer_writes_only_dirty_ranges", gpu_retained_layer_writes_only_dirty_ranges),
        ("gpu_view_table_past_255", gpu_view_table_past_255),
        ("gpu_grid_instance_equals_dots", gpu_grid_instance_equals_dots),
        ("gpu_blur_translucent_source", gpu_blur_translucent_source),
        ("gpu_backdrop_blur", gpu_backdrop_blur),
        ("gpu_smooth_image", gpu_smooth_image),
        ("gpu_backdrop_blur_panel", gpu_backdrop_blur_panel),
        ("fit_modes_place_and_crop", fit_modes_place_and_crop),
        ("nine_slice_corners_stay_and_centre_stretches", nine_slice_corners_stay_and_centre_stretches),
        ("shared_device_matches_kit_panel", shared_device_matches_kit_panel),
        ("shared_no_submit", shared_no_submit),
        ("shared_teardown", shared_teardown),
        ("shared_frame_serials", shared_frame_serials),
        ("shared_backdrop_blur", shared_backdrop_blur),
    ];
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--ignored") { return; }
    if args.iter().any(|a| a == "--list") {
        for (name, _) in cases { println!("{name}: test"); }
        return;
    }
    let filters: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    for (name, case) in cases {
        if !filters.is_empty() && !filters.iter().any(|f| *f == name) { continue; }
        if std::panic::catch_unwind(case).is_err() { std::process::exit(101); }
        println!("test {name} ... ok");
    }
}

fn gpu_scene_golden_and_spot_checks() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let pixels = render_scene(&device, &queue);
    finish(device, queue);

    assert_eq!(at(&pixels, 16.0, 12.0), RED, "solid fill centre");
    assert_eq!(at(&pixels, 46.0, 12.0), BLUE, "rounded rect centre");
    assert_eq!(at(&pixels, 59.6, 4.4), BG, "pixel inside the 4-point top-right corner is outside the shape");
    assert_eq!(at(&pixels, 32.2, 4.2), BLUE, "square top-left corner is filled");
    assert_eq!(at(&pixels, 78.0, 12.0), [250, 210, 60, 255], "bordered rect fill");
    assert_eq!(at(&pixels, 78.0, 4.6), INK, "inside border");
    assert_eq!(at(&pixels, 29.5, 80.0), BG, "clip excludes the rect left of it");
    assert_eq!(at(&pixels, 40.0, 82.0), GREEN, "inside the clip");
    assert_eq!(at(&pixels, 30.6, 76.4), BG, "rounded clip corner excludes");
    assert_eq!(at(&pixels, 64.0, 82.0), BG, "view offset moved the rect right");
    assert!(near(at(&pixels, 86.0, 82.0), [122, 142, 212, 255], 2), "view opacity halves the rect: {:?}", at(&pixels, 86.0, 82.0));
    assert!(near(at(&pixels, 108.0, 60.0), BG, 0) && near(at(&pixels, 101.0, 60.0), BG, 0), "glyph hole and margin are background");
    assert_eq!(at(&pixels, 104.6, 60.0), INK, "glyph ring is ink");
    assert_eq!(at(&pixels, 99.0, 90.0), INK, "bottom-only border band");
    assert_eq!(at(&pixels, 99.0, 80.0), [255, 255, 255, 255], "no border above the band");
    assert!(near(at(&pixels, 117.0, 76.0), GREEN, 6), "3-stop ramp middle is the middle stop: {:?}", at(&pixels, 117.0, 76.0));
    assert_eq!(at(&pixels, 117.0, 88.0), BLUE, "rotated view keeps its centre");
    assert_eq!(at(&pixels, 5.0, 73.0), [255, 255, 255, 255], "checker texel 0");
    assert_eq!(at(&pixels, 9.0, 73.0), [20, 40, 160, 255], "checker texel 1");

    let golden = format!("{}/../../../fixtures/pxui_render/scene_{SCALE}x.png", env!("CARGO_MANIFEST_DIR"));
    if std::env::var("RAYS_UPDATE_FIXTURES").as_deref() == Ok("1") {
        std::fs::create_dir_all(std::path::Path::new(&golden).parent().unwrap()).unwrap();
        std::fs::write(&golden, png::encode(W * SCALE, H * SCALE, &pixels)).unwrap();
    }
    let (w, h, expected) = png::decode(&std::fs::read(&golden).expect("golden; refresh with RAYS_UPDATE_FIXTURES=1"));
    assert_eq!((w, h), (W * SCALE, H * SCALE), "golden size");
    let (mut different, mut first, mut max) = (0, None, 0);
    for (i, (a, b)) in pixels.chunks(4).zip(expected.chunks(4)).enumerate() {
        max = max.max(a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap());
        // designed limit (specification/pxui.md): 2 levels (measured max: Metal 0, Vulkan 1) absorb transcendental differences on edges.
        if !near(a.try_into().unwrap(), b.try_into().unwrap(), 2) {
            different += 1;
            first.get_or_insert((i as u32 % w, i as u32 / w));
        }
    }
    println!("pxui_render golden max per-channel difference: {max}");
    assert_eq!(different, 0, "{different} pixels differ from {golden}, first at {first:?}");
}

fn gpu_one_draw_per_layer() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let mut painter = painter(&device);
    let target = target(&device);
    let external = texture(&device, 4, 4, &[TextureUsage::TextureBinding, TextureUsage::TextureCopyDst]);
    ok(be::write_texture(&external, None, Origin { x: 0, y: 0, z: 0 }, Extent { width: 4, height: 4, depth: 1 }, 16, &[255; 64]));
    // A scroll list: every row clipped differently, rows of three kinds.
    let rows: Vec<Instance> = (0..300).map(|i| {
        let y = (i % 40) as f32 * 2.0;
        let clip = [0, (i % 7) as i16 * 4, 128, 96 - (i % 5) as i16 * 4];
        match i % 3 {
            0 => Instance::rect([2.0, y, 60.0, y + 2.0], RED).clip(clip, 2.0),
            1 => Instance::mask([62.0, y, 70.0, y + 2.0], [0.0, 0.0, 0.1, 0.1], INK).clip(clip, 0.0),
            _ => Instance::line([72.0, y], [120.0, y + 1.0], 1.0, BLUE).clip(clip, 0.0),
        }
    }).collect();
    painter.begin_frame();
    pass(&queue, &target, &mut |encoder| ok(painter.draw(encoder, [W * SCALE, H * SCALE], SCALE as f32, &rows, &[])));
    assert_eq!(painter.draw_calls(), 1, "clips do not split a layer");
    let breaks = [Break { at_instance: 100, texture: Some(&external) }, Break { at_instance: 200, texture: None }];
    painter.begin_frame();
    pass(&queue, &target, &mut |encoder| ok(painter.draw(encoder, [W * SCALE, H * SCALE], SCALE as f32, &rows, &breaks)));
    assert_eq!(painter.draw_calls(), 4, "two breaks make three draws");
    let bad = [Break { at_instance: 200, texture: None }, Break { at_instance: 100, texture: None }];
    pass(&queue, &target, &mut |encoder| assert!(painter.draw(encoder, [W * SCALE, H * SCALE], SCALE as f32, &rows, &bad).is_err(), "unordered breaks are refused"));
    assert!(painter.set_view(0, View::IDENTITY).is_err(), "view 0 stays the identity");
    for texture in [&target, &external] { ok(be::destroy_texture(texture)); }
    drop(painter);
    finish(device, queue);
}

fn gpu_warm_frame_allocates_nothing() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let mut painter = painter(&device);
    set_scene_views(&mut painter);
    let (glyph, checker) = atlas_bytes();
    let target = target(&device);
    let instances = scene();
    let mut allocations = u64::MAX;
    for frame in 0..4 {
        painter.begin_frame();
        let commands = ok(be::begin_commands(&queue));
        let encoder = ok(be::render_encoder(&commands, &RenderTarget {
            colors: vec![ColorAttachment { texture: target.clone(), resolve: None, load: Load::Clear, store: Store::Store, clear: (0.0, 0.0, 0.0, 1.0) }],
            depth: None,
        }));
        mem_sys::local_reset();
        upload_atlases(&mut painter, &glyph, &checker);
        ok(painter.draw(&encoder, [W * SCALE, H * SCALE], SCALE as f32, &instances, &[]));
        if frame == 3 { allocations = mem_sys::local_alloc_count(); }
        ok(be::end_render(&encoder));
        ok(be::complete_through(&queue, ok(be::commit(&commands)).epoch));
    }
    assert_eq!(allocations, 0, "a warm upload + draw allocates nothing");
    ok(be::destroy_texture(&target));
    drop(painter);
    finish(device, queue);
}

fn gpu_view_scale_and_rotation() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let mut painter = painter(&device);
    // View 1: halved about its centre (40, 30). View 2: turned a quarter about the centre (100, 54).
    ok(painter.set_view(1, View { scale: 0.5, pivot: [40.0, 30.0], ..View::IDENTITY }));
    ok(painter.set_view(2, View::rotation(std::f32::consts::FRAC_PI_2, [100.0, 54.0])));
    let target = target(&device);
    // The scaled rect's left edge lands at 30.125, a quarter pixel inside the pixel centre at 30.25:
    // coverage 0.75 only if the anti-aliasing width follows the scale.
    let instances = [Instance::rect([20.25, 20.0, 59.75, 40.0], BLUE).view(1), Instance::rect([80.0, 50.0, 120.0, 58.0], RED).view(2)];
    painter.begin_frame();
    pass(&queue, &target, &mut |encoder| ok(painter.draw(encoder, [W * SCALE, H * SCALE], SCALE as f32, &instances, &[])));
    let pixels = ok(be::read_texture(&target, (W * SCALE * 4) as i64));
    ok(be::destroy_texture(&target));
    drop(painter);
    finish(device, queue);
    assert_eq!(at(&pixels, 40.0, 30.0), BLUE, "scaled rect centre");
    assert_eq!(at(&pixels, 32.0, 30.0), BLUE, "inside the halved rect");
    assert_eq!(at(&pixels, 28.0, 30.0), BG, "left of the halved rect (inside the unscaled one)");
    assert_eq!(at(&pixels, 40.0, 23.0), BG, "above the halved rect");
    let edge = at(&pixels, 30.3, 30.0);
    assert!(near(edge, [81, 111, 216, 255], 6), "scaled edge coverage 0.75, not 1: {edge:?}");
    assert_eq!(at(&pixels, 100.0, 40.0), RED, "rotated rect is now tall");
    assert_eq!(at(&pixels, 100.0, 70.0), RED, "rotated rect's far end");
    assert_eq!(at(&pixels, 85.0, 54.0), BG, "the old horizontal arm is gone");
    assert_eq!(at(&pixels, 107.0, 54.0), BG, "right of the rotated rect (width 8, half-width 4)");
}

fn gpu_per_side_borders() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let mut painter = painter(&device);
    let target = target(&device);
    let white = [255, 255, 255, 255];
    let (sides, n) = pxui_core::draw::borders([60.0, 8.0, 92.0, 32.0], [0.0; 4], [0.0, 4.0, 0.0, 2.0], [[0; 4], RED, [0; 4], BLUE]);
    assert_eq!(n, 1, "one instance for every side");
    // four widths and four colours: the corners split on the mitre, no seam
    let (green, gold) = ([0, 160, 0, 255], [230, 180, 0, 255]);
    let (four, m) = pxui_core::draw::borders([8.0, 48.0, 48.0, 80.0], [0.0; 4], [1.0, 2.0, 3.0, 4.0], [RED, green, BLUE, gold]);
    let mut instances = vec![
        Instance::rect([8.0, 8.0, 40.0, 32.0], white).border(4.0, INK).sides(flags::BOTTOM),
        Instance::rect([60.0, 8.0, 92.0, 32.0], white),
        Instance::rect([8.0, 48.0, 48.0, 80.0], white),
    ];
    instances.extend_from_slice(&sides[..n]);
    instances.extend_from_slice(&four[..m]);
    painter.begin_frame();
    pass(&queue, &target, &mut |encoder| ok(painter.draw(encoder, [W * SCALE, H * SCALE], SCALE as f32, &instances, &[])));
    let pixels = ok(be::read_texture(&target, (W * SCALE * 4) as i64));
    ok(be::destroy_texture(&target));
    drop(painter);
    finish(device, queue);
    assert_eq!(at(&pixels, 24.0, 30.0), INK, "bottom border band");
    assert_eq!(at(&pixels, 24.0, 26.0), white, "above the band is fill");
    assert_eq!(at(&pixels, 24.0, 9.0), white, "no top border");
    assert_eq!(at(&pixels, 9.0, 20.0), white, "no left border");
    assert_eq!(at(&pixels, 39.0, 20.0), white, "no right border");
    assert_eq!(at(&pixels, 61.0, 20.0), BLUE, "left side 2 wide, own colour");
    assert_eq!(at(&pixels, 63.0, 20.0), white, "left side is only 2 wide");
    assert_eq!(at(&pixels, 90.0, 20.0), RED, "right side 4 wide, own colour");
    assert_eq!(at(&pixels, 76.0, 9.0), white, "no top border from the helper either");
    for (x, y, colour, what) in [(28.0, 48.25, RED, "top"), (47.0, 64.0, green, "right"), (28.0, 79.0, BLUE, "bottom"), (9.0, 64.0, gold, "left"),
        (8.25, 48.25, gold, "top-left corner: the wider left side's"), (11.75, 48.25, RED, "top-left past the mitre: top"), (47.75, 79.75, BLUE, "bottom-right corner: the wider bottom's")] {
        assert_eq!(at(&pixels, x, y), colour, "four borders: {what}");
    }
    // no seam: every pixel centre well inside the band shows a border colour
    for y in 0..32 {
        for x in 0..40 {
            let (px, py) = (8.25 + x as f32, 48.25 + y as f32);
            let band = py < 49.0 || px > 46.0 || py > 77.0 || px < 12.0;
            if band {
                let c = at(&pixels, px, py);
                assert!([RED, green, BLUE, gold].contains(&c), "a seam at ({px}, {py}): {c:?}");
            }
        }
    }
}

fn gpu_ramp_gradient() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let mut painter = ok(Painter::new(&device, ColorFormat::Rgba8Unorm, DepthFormat::NoDepth, 1, (64, 64), (256, 4)));
    let target = target(&device);
    let external = texture(&device, 4, 4, &[TextureUsage::TextureBinding, TextureUsage::TextureCopyDst]);
    ok(be::write_texture(&external, None, Origin { x: 0, y: 0, z: 0 }, Extent { width: 4, height: 4, depth: 1 }, 16, &[255; 64]));
    let mut row = [0u8; 1024];
    ramp(&[(0.0, RED), (0.5, GREEN), (1.0, BLUE)], &mut row);
    assert_eq!((&row[..4], &row[1020..]), (&RED[..], &BLUE[..]), "ends are the end stops");
    ok(painter.upload(AtlasKind::Color, 0, 2, 256, 1, &row, 1024));
    let bar = |y: f32| Instance::rect([0.0, y, 100.0, y + 20.0], [0; 4]).linear([0.0, 0.5], [1.0, 0.5], [0; 4]).ramp(2);
    let instances = [bar(0.0), bar(40.0)];
    painter.begin_frame();
    // Row 40 is drawn after a break onto an external texture: ramps still read the atlas.
    let breaks = [Break { at_instance: 1, texture: Some(&external) }];
    pass(&queue, &target, &mut |encoder| ok(painter.draw(encoder, [W * SCALE, H * SCALE], SCALE as f32, &instances, &breaks)));
    let pixels = ok(be::read_texture(&target, (W * SCALE * 4) as i64));
    for texture in [&target, &external] { ok(be::destroy_texture(texture)); }
    drop(painter);
    finish(device, queue);
    for y in [10.0, 50.0] {
        assert!(near(at(&pixels, 50.0, y), GREEN, 2), "middle pixel is the middle stop: {:?}", at(&pixels, 50.0, y));
        assert!(near(at(&pixels, 0.5, y), RED, 8) && near(at(&pixels, 99.5, y), BLUE, 8), "ends");
    }
    assert_eq!(at(&pixels, 110.0, 10.0), BG, "outside the rect");
}

/// View 2,000's entry (offset, opacity, clip) applies exactly as view 1's would: the table is not
/// capped at 256 entries.
fn gpu_view_table_past_255() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let mut painter = painter(&device);
    let target = target(&device);
    let size = [W * SCALE, H * SCALE];
    let entry = View { offset: [9.0, 4.0], opacity: 0.5, clip: [0.0, 0.0, 30.0, 40.0], ..View::IDENTITY };
    let shoot = |painter: &mut Painter, view: u16| {
        let rect = [Instance::rect([4.0, 4.0, 60.0, 50.0], RED).view(view)];
        painter.begin_frame();
        pass(&queue, &target, &mut |encoder| ok(painter.draw_retained(encoder, size, SCALE as f32, view as u32 % 3, &rect, &[], &[])));
        ok(be::read_texture(&target, (W * SCALE * 4) as i64))
    };
    ok(painter.set_view(1, entry));
    ok(painter.set_view(2000, entry));
    let (low, high) = (shoot(&mut painter, 1), shoot(&mut painter, 2000));
    assert!(low == high, "view 2000 drew differently from view 1");
    assert!(low != shoot(&mut painter, 0), "the entry changed nothing");
}

/// One `kind::GRID` instance draws the pixels of one rect instance per dot (fractional origin and
/// spacing, a clip cutting dots, view 1's offset), to one level on a few edge texels.
fn gpu_grid_instance_equals_dots() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let mut painter = painter(&device);
    ok(painter.set_view(1, View { offset: [0.5, 0.25], ..View::IDENTITY }));
    let target = target(&device);
    let size = [W * SCALE, H * SCALE];
    for (view, origin, spacing, dot) in [(0u16, [3.37f32, 5.11], 7.3f32, 2.0f32), (0, [0.0, 0.0], 24.0, 3.0), (1, [1.5, 2.25], 9.0, 1.5)] {
        let (region, clip) = ([10.0f32, 8.0, 150.0, 110.0], [12i16, 9, 140, 100]);
        let first = |lo: f32, o: f32| ((lo - o - dot) / spacing).floor() as i64 + 1;
        let last = |hi: f32, o: f32| ((hi - o) / spacing).ceil() as i64 - 1;
        let mut dots = Vec::new();
        for j in first(region[1], origin[1])..=last(region[3], origin[1]) {
            let dy = origin[1] + j as f32 * spacing;
            for i in first(region[0], origin[0])..=last(region[2], origin[0]) {
                let dx = origin[0] + i as f32 * spacing;
                dots.push(Instance { clip, ..Instance::rect([dx, dy, dx + dot, dy + dot], RED).view(view) });
            }
        }
        let grid = [Instance { clip, ..Instance::grid(region, origin, spacing, dot, RED).view(view) }];
        let shoot = |painter: &mut Painter, instances: &[Instance]| {
            painter.begin_frame();
            pass(&queue, &target, &mut |encoder| ok(painter.draw(encoder, size, SCALE as f32, instances, &[])));
            ok(be::read_texture(&target, (W * SCALE * 4) as i64))
        };
        let (a, b) = (shoot(&mut painter, &dots), shoot(&mut painter, &grid));
        // the quads differ (one per dot or one for all), so a pixel centre's interpolated position differs
        // in its last bits (about 1e-3 px): an anti-aliased edge texel near a rounding boundary lands one
        // level apart; inner and outer texels are equal
        let worst = a.iter().zip(&b).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap_or(0);
        let apart = a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count();
        println!("grid spacing {spacing}: {} dots, {apart} pixels a level apart (largest difference {worst})", dots.len());
        assert!(dots.len() > 20 && worst <= 1, "spacing {spacing}: largest channel difference {worst}, {apart} pixels differ");
    }
}

fn gpu_retained_layer_writes_only_dirty_ranges() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let mut painter = painter(&device);
    set_scene_views(&mut painter);
    let (glyph, checker) = atlas_bytes();
    upload_atlases(&mut painter, &glyph, &checker);
    let target = target(&device);
    let external = texture(&device, 4, 4, &[TextureUsage::TextureBinding, TextureUsage::TextureCopyDst]);
    ok(be::write_texture(&external, None, Origin { x: 0, y: 0, z: 0 }, Extent { width: 4, height: 4, depth: 1 }, 16, &[255; 64]));
    // A break at instance 14 (not a multiple of 4) puts the image on the external texture.
    let breaks = [Break { at_instance: 14, texture: Some(&external) }];
    let size = [W * SCALE, H * SCALE];
    let mut instances = scene();
    let count = instances.len() as u64;
    let plain = |painter: &mut Painter, instances: &[Instance]| {
        pass(&queue, &target, &mut |encoder| ok(painter.draw(encoder, size, SCALE as f32, instances, &breaks)));
        ok(be::read_texture(&target, (W * SCALE * 4) as i64))
    };
    // (change, expected bytes written): frame 0 fills all three copies, the edit is replayed into the
    // next two frames' copies, then the layer is quiet again. Two far-apart edits copy only themselves
    // (each copy keeps a list of ranges), not the span between them.
    let frames: [(&[usize], u64); 11] = [(&[], 3 * count * 64), (&[], 0), (&[3], 64), (&[], 64), (&[], 64), (&[], 0), (&[7], 64), (&[], 64), (&[], 64), (&[1, 17], 128), (&[], 128)];
    for (frame, (change, expected)) in frames.into_iter().enumerate() {
        for &i in change { instances[i] = Instance::rect([4.0 + frame as f32, 76.0, 12.0 + frame as f32, 90.0], [frame as u8 * 20, 90, 10, 255]).view(1); }
        let dirty: Vec<(u32, u32)> = change.iter().map(|&i| (i as u32, i as u32 + 1)).collect();
        painter.begin_frame();
        let mut allocations = 0;
        pass(&queue, &target, &mut |encoder| {
            mem_sys::local_reset();
            ok(painter.draw_retained(encoder, size, SCALE as f32, 0, &instances, &dirty, &breaks));
            allocations = mem_sys::local_alloc_count();
        });
        assert_eq!(painter.retained_bytes(), expected, "frame {frame}: bytes written");
        if frame >= 2 { assert_eq!(allocations, 0, "frame {frame}: warm retained draw allocates nothing"); }
        let retained = ok(be::read_texture(&target, (W * SCALE * 4) as i64));
        painter.begin_frame();
        assert!(retained == plain(&mut painter, &instances), "frame {frame}: pixels equal plain draw");
    }
    // Growth past capacity and a shrink (a shrink after an in-place change leaves the copies not yet
    // written a pending range past the new length; it is clamped, not read past the instances): pixels still equal plain draw.
    let rows = |n: usize| (0..n).map(|i| Instance::rect([(i % 60) as f32, (i / 60) as f32 * 2.0, (i % 60) as f32 + 1.5, (i / 60) as f32 * 2.0 + 1.0], [(i * 7) as u8, 100, 200, 255])).collect::<Vec<_>>();
    for (frame, n) in [300, 700, 650, 90, 90, 90, 90].into_iter().enumerate() {
        let list = rows(n);
        painter.begin_frame();
        pass(&queue, &target, &mut |encoder| ok(painter.draw_retained(encoder, size, SCALE as f32, 1, &list, &[], &[])));
        let retained = ok(be::read_texture(&target, (W * SCALE * 4) as i64));
        painter.begin_frame();
        assert!(retained == plain(&mut painter, &list), "layer resize {frame}: pixels equal plain draw");
    }
    pass(&queue, &target, &mut |encoder| assert!(painter.draw_retained(encoder, size, SCALE as f32, 0, &instances, &[(0, 99)], &[]).is_err(), "dirty range past the end is refused"));
    for texture in [&target, &external] { ok(be::destroy_texture(texture)); }
    drop(painter);
    finish(device, queue);
}

/// Red channel along the middle row of a `w`-texel-wide RGBA texture readback.
fn middle_row(pixels: &[u8], w: usize) -> Vec<i32> {
    let y = pixels.len() / 4 / w / 2;
    (0..w).map(|x| pixels[(y * w + x) * 4] as i32).collect()
}

/// A black/white edge at texel `edge` blurred with `sigma` texels: monotonic, 10-90% width 2.56 sigma, far field untouched.
fn assert_ramp(row: &[i32], edge: usize, sigma: f32, what: &str) {
    assert!(row.windows(2).all(|p| p[0] <= p[1]), "{what}: monotonic {row:?}");
    assert_eq!((row[0], row[row.len() - 1]), (0, 255), "{what}: far field unchanged");
    let first = |level: i32| row.iter().position(|v| *v >= level).unwrap() as f32;
    let width = first(230) - first(26);
    assert!((width - 2.56 * sigma).abs() <= 1.5, "{what}: 10-90% width {width}, expected {}", 2.56 * sigma);
    assert!((row[edge - 1] - 102).abs() <= 14 && (row[edge] - 153).abs() <= 14, "{what}: the two texels at the edge straddle 50%: {} {}", row[edge - 1], row[edge]);
}

/// A picture with a hard texel edge (black | white, 2x1 texels) scaled across the target: with
/// `flags::SMOOTH` the edge is a smooth monotonic ramp (no texel tie for a GPU to break), without
/// it a hard step; drawn 1:1 on whole pixels (a 4x4 checker into 4x4 pixels) SMOOTH returns the
/// texels exactly.
fn gpu_smooth_image() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let mut painter = painter(&device);
    let edge = texture(&device, 2, 1, &[TextureUsage::TextureBinding, TextureUsage::TextureCopyDst]);
    ok(be::write_texture(&edge, None, Origin { x: 0, y: 0, z: 0 }, Extent { width: 2, height: 1, depth: 1 }, 8, &[0, 0, 0, 255, 255, 255, 255, 255]));
    let shown = target(&device);
    let size = [W * SCALE, H * SCALE];
    let row = |painter: &mut Painter, flags: u8| {
        let image = [Instance { flags, ..Instance::image([0.0, 0.0, W as f32, H as f32], [0.0, 0.0, 1.0, 1.0], [255; 4]) }];
        painter.begin_frame();
        pass(&queue, &shown, &mut |encoder| ok(painter.draw(encoder, size, SCALE as f32, &image, &[Break { at_instance: 0, texture: Some(&edge) }])));
        middle_row(&ok(be::read_texture(&shown, (size[0] * 4) as i64)), size[0] as usize)
    };
    let (w, smooth, hard) = (size[0] as usize, row(&mut painter, flags::SMOOTH), row(&mut painter, 0));
    assert!(smooth.windows(2).all(|p| p[0] <= p[1]), "smooth: monotonic {smooth:?}");
    // texel centres at a quarter and three quarters: clamped outside, linear between
    assert!(smooth[..w / 4 - 1].iter().all(|v| *v <= 2) && smooth[3 * w / 4 + 1..].iter().all(|v| *v >= 253), "smooth: flat beyond the texel centres {smooth:?}");
    for x in (w / 4 + 2..3 * w / 4 - 2).step_by(7) {
        let expected = 255.0 * ((x as f32 + 0.5) / w as f32 * 2.0 - 0.5);
        assert!((smooth[x] as f32 - expected).abs() <= 2.0, "smooth: pixel {x} is {} not {expected}", smooth[x]);
    }
    assert!(hard[..w / 2].iter().all(|v| *v == 0) && hard[w / 2..].iter().all(|v| *v == 255), "nearest: a hard step {hard:?}");
    // 1:1 on whole pixels: the checker's texels exactly, either filter
    let checker = texture(&device, 4, 4, &[TextureUsage::TextureBinding, TextureUsage::TextureCopyDst]);
    let texels: Vec<u8> = (0..16).flat_map(|i| if (i % 4 + i / 4) % 2 == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }).collect();
    ok(be::write_texture(&checker, None, Origin { x: 0, y: 0, z: 0 }, Extent { width: 4, height: 4, depth: 1 }, 16, &texels));
    let one = |painter: &mut Painter, flags: u8| {
        let s = SCALE as f32;
        let image = [Instance { flags, ..Instance::image([10.0, 10.0, 10.0 + 4.0 / s, 10.0 + 4.0 / s], [0.0, 0.0, 1.0, 1.0], [255; 4]) }];
        painter.begin_frame();
        pass(&queue, &shown, &mut |encoder| ok(painter.draw(encoder, size, s, &image, &[Break { at_instance: 0, texture: Some(&checker) }])));
        let px = ok(be::read_texture(&shown, (size[0] * 4) as i64));
        (0..16).map(|i| at(&px, 10.0 + ((i % 4) as f32 + 0.5) / s, 10.0 + ((i / 4) as f32 + 0.5) / s)).collect::<Vec<_>>()
    };
    let (a, b) = (one(&mut painter, flags::SMOOTH), one(&mut painter, 0));
    assert_eq!(a, b, "1:1 on whole pixels, linear equals nearest");
    assert!(a.iter().zip(texels.chunks(4)).all(|(p, t)| p[..] == *t), "1:1 shows the texels: {a:?}");
    for texture in [&edge, &checker, &shown] { ok(be::destroy_texture(texture)); }
    drop(painter);
    finish(device, queue);
}

/// A translucent source blurs premultiplied: opaque red beside clear keeps its red where it fades
/// (straight-alpha averaging darkened it towards the clear texels' black), at both downsamples.
fn gpu_blur_translucent_source() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let mut painter = painter(&device);
    let (w, h) = (64u32, 16u32);
    let source = texture(&device, w, h, &[TextureUsage::TextureBinding, TextureUsage::TextureCopyDst, TextureUsage::TextureCopySrc]);
    let pixels: Vec<u8> = (0..w * h).flat_map(|i| if i % w < w / 2 { [255, 0, 0, 255] } else { [0, 0, 0, 0] }).collect();
    ok(be::write_texture(&source, None, Origin { x: 0, y: 0, z: 0 }, Extent { width: w as i64, height: h as i64, depth: 1 }, (w * 4) as i64, &pixels));
    let sink = target(&device);
    let back = ok(be::read_texture(&source, (w * 4) as i64));
    assert_eq!((&back[..4], &back[(w as usize - 1) * 4..][..4]), (&[255u8, 0, 0, 255][..], &[0u8; 4][..]), "the source");
    for sigma in [3.0, 8.0] {
        painter.begin_frame();
        let blurred = ok(painter.blur(&queue, &source, [0, 0, w, h], sigma)).clone();
        pass(&queue, &sink, &mut |_| {}); // a pass after the blur's on the queue: its work is done
        let bw = blurred_width(&blurred);
        let out = ok(be::read_texture(&blurred, (bw * 4) as i64));
        let row = &out[(bw as usize * (out.len() / 4 / bw as usize / 2)) * 4..][..bw as usize * 4];
        let fading: Vec<&[u8]> = row.chunks(4).filter(|t| t[3] > 20 && t[3] < 235).collect();
        assert!(!fading.is_empty(), "sigma {sigma}: no texel fades: {:?}", row.chunks(4).collect::<Vec<_>>());
        assert!(fading.iter().all(|t| t[0] >= 250 && t[1] <= 2 && t[2] <= 2), "sigma {sigma}: a fading texel is not red: {fading:?}");
    }
    for texture in [&source, &sink] { ok(be::destroy_texture(texture)); }
    drop(painter);
    finish(device, queue);
}

fn gpu_backdrop_blur() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let mut painter = painter(&device);
    let source = target(&device);
    let size = [W * SCALE, H * SCALE];
    let edge = [Instance::rect([0.0, 0.0, 64.0, 96.0], [0, 0, 0, 255]), Instance::rect([64.0, 0.0, 128.0, 96.0], [255, 255, 255, 255])];
    painter.begin_frame();
    pass(&queue, &source, &mut |encoder| ok(painter.draw(encoder, size, SCALE as f32, &edge, &[])));
    // Whole source, sigma 4 px: halved (sigma 2 texels), edge at texel 64 of 128.
    let blurred = ok(painter.blur(&queue, &source, [0, 0, size[0], size[1]], 4.0)).clone();
    // The blurred texture drawn as an image through a break, tinted white, into a fresh target.
    let shown = target(&device);
    let image = [Instance::image([0.0, 0.0, 128.0, 96.0], [0.0, 0.0, 1.0, 1.0], [255, 255, 255, 255])];
    pass(&queue, &shown, &mut |encoder| ok(painter.draw(encoder, size, SCALE as f32, &image, &[Break { at_instance: 0, texture: Some(&blurred) }])));
    let direct = ok(be::read_texture(&blurred, (blurred_width(&blurred) * 4) as i64));
    let shown_pixels = ok(be::read_texture(&shown, (size[0] * 4) as i64));
    assert_eq!(blurred_width(&blurred), 128);
    let row = middle_row(&direct, 128);
    assert_ramp(&row, 64, 2.0, "x2 downsample");
    let (top, bottom) = (&direct[(10 * 128 + 60) * 4..][..32], &direct[(80 * 128 + 60) * 4..][..32]);
    assert!(top.iter().zip(bottom).all(|(a, b)| a.abs_diff(*b) <= 1), "an edge blurred horizontally is the same on every row");
    assert_eq!(at(&shown_pixels, 20.0, 20.0)[0] as i32, row[20], "drawn image shows the blurred texel");
    assert_eq!(at(&shown_pixels, 65.5, 48.0)[..3], [row[65] as u8; 3], "drawn image shows the blurred texel next to the edge");

    // Right three quarters, sigma 8 px: the scratch targets are replaced (x4 downsample, 48 texels, edge at 16).
    let blurred = ok(painter.blur(&queue, &source, [64, 0, size[0], size[1]], 8.0)).clone();
    painter.begin_frame();
    pass(&queue, &shown, &mut |_| {});
    assert_eq!(blurred_width(&blurred), 48);
    let row = middle_row(&ok(be::read_texture(&blurred, 48 * 4)), 48);
    assert_ramp(&row, 16, 2.0, "x4 downsample");
    assert!(painter.blur(&queue, &source, [0, 0, size[0] + 1, size[1]], 4.0).is_err(), "region outside the source is refused");
    for texture in [&source, &shown] { ok(be::destroy_texture(texture)); }
    drop(painter);
    finish(device, queue);
}

/// The host's backdrop-blur sequence for one view: draw the backdrop, end the pass, blur the region
/// under the view, reopen the pass on the same target (`Load::Load`), draw the blur as an image
/// clipped to the view's rounded rect. The tint halves the blurred colour so it differs from the backdrop.
fn gpu_backdrop_blur_panel() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let mut painter = painter(&device);
    let target = target(&device);
    let size = [W * SCALE, H * SCALE];
    let panel = [40.0, 24.0, 88.0, 72.0];
    let region = panel.map(|v| (v * SCALE as f32) as u32);
    let edge = [Instance::rect([0.0, 0.0, 64.0, 96.0], [0, 0, 0, 255]), Instance::rect([64.0, 0.0, 128.0, 96.0], [255, 255, 255, 255])];
    painter.begin_frame();
    pass(&queue, &target, &mut |encoder| ok(painter.draw(encoder, size, SCALE as f32, &edge, &[])));
    let blurred = ok(painter.blur(&queue, &target, region, 8.0)).clone();
    let glass = [Instance::image(panel, [0.0, 0.0, 1.0, 1.0], [128, 128, 128, 255]).radii([12.0; 4])];
    pass_with(&queue, &target, Load::Load, &mut |encoder| ok(painter.draw(encoder, size, SCALE as f32, &glass, &[Break { at_instance: 0, texture: Some(&blurred) }])));
    let pixels = ok(be::read_texture(&target, (size[0] * 4) as i64));
    let red = |x: f32, y: f32| at(&pixels, x, y)[0] as i32;
    assert_eq!((red(20.0, 48.0), red(110.0, 48.0)), (0, 255), "outside the view the backdrop is untouched");
    assert_eq!((red(87.0, 25.0), red(41.0, 25.0)), (255, 0), "outside the rounded corner the backdrop shows");
    assert!(red(45.0, 48.0) <= 4 && (red(82.0, 48.0) - 128).abs() <= 4, "far from the edge the blur is the tinted backdrop: {} {}", red(45.0, 48.0), red(82.0, 48.0));
    let (left, right) = (red(63.5, 48.0), red(64.5, 48.0));
    assert!((50..=78).contains(&left) && (50..=78).contains(&right), "the hard edge is a soft ramp through half tint: {left} {right}");
    assert!(red(58.0, 48.0) > 4 && red(70.0, 48.0) < 124, "the ramp spreads across the edge");
    ok(be::destroy_texture(&target));
    drop(painter);
    finish(device, queue);
}

fn blurred_width(texture: &Texture) -> u32 { be::texture_descriptor(texture).width as u32 }

// Pure image-placement maths (no GPU).

fn close(a: [f32; 4], b: [f32; 4]) -> bool { a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5) }

fn fit_modes_place_and_crop() {
    let (image, bounds, full) = ([200.0, 100.0], [0.0, 0.0, 100.0, 100.0], [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(fit(Fit::Fill, image, bounds, full), (bounds, full));
    let (rect, uv) = fit(Fit::Contain, image, bounds, full);
    assert!(close(rect, [0.0, 25.0, 100.0, 75.0]) && close(uv, full), "{rect:?} {uv:?}");
    let (rect, uv) = fit(Fit::Cover, image, bounds, full);
    assert!(close(rect, bounds) && close(uv, [0.25, 0.0, 0.75, 1.0]), "{rect:?} {uv:?}");
    let (rect, uv) = fit(Fit::None, image, bounds, full);
    assert!(close(rect, bounds) && close(uv, [0.25, 0.0, 0.75, 1.0]), "{rect:?} {uv:?}");
    let (rect, uv) = fit(Fit::None, [20.0, 10.0], bounds, full);
    assert!(close(rect, [40.0, 45.0, 60.0, 55.0]) && close(uv, full), "{rect:?} {uv:?}");
    // An atlas slot keeps its offset: the cover crop is taken inside the slot.
    let (_, uv) = fit(Fit::Cover, image, bounds, [0.5, 0.0, 1.0, 0.5]);
    assert!(close(uv, [0.625, 0.0, 0.875, 0.5]), "{uv:?}");
}

fn nine_slice_corners_stay_and_centre_stretches() {
    let tiles = nine_slice([0.0, 0.0, 100.0, 60.0], [0.0, 0.0, 1.0, 1.0], [30.0, 30.0], [10.0, 10.0, 10.0, 10.0], [255; 4]);
    let third = 1.0 / 3.0;
    assert!(close(tiles[0].rect, [0.0, 0.0, 10.0, 10.0]) && tiles[0].kind == pxui_core::draw::kind::IMAGE);
    assert!(close(tiles[4].rect, [10.0, 10.0, 90.0, 50.0]));
    assert!(close(tiles[8].rect, [90.0, 50.0, 100.0, 60.0]));
    let uv = |i: usize| [0, 1, 2, 3].map(|k| { let w = tiles[i].aux[k / 2]; (w >> (16 * (k % 2)) & 0xffff) as f32 / 65535.0 });
    assert!(close(uv(0), [0.0, 0.0, third, third]) && close(uv(4), [third, third, 2.0 * third, 2.0 * third]) && close(uv(8), [2.0 * third, 2.0 * third, 1.0, 1.0]));
    // Too small for its corners: they squeeze together instead of overlapping.
    let small = nine_slice([0.0, 0.0, 10.0, 60.0], [0.0, 0.0, 1.0, 1.0], [30.0, 30.0], [10.0, 10.0, 10.0, 10.0], [255; 4]);
    assert!(close(small[0].rect, [0.0, 0.0, 5.0, 10.0]) && close(small[2].rect, [5.0, 0.0, 10.0, 10.0]));
}

/// The kit panel built through the public ABI, with a label that changes between frames (new glyphs:
/// a staged atlas upload in each frame).
fn shared_ui(caption: &str) -> pxui::Ui {
    const GOLDEN_FONT: &[u8] = include_bytes!("../../../../assets/fonts/DepartureMono-Regular.otf");
    let mut ui = pxui::Ui::create(pxui::Platform::Apple, None, &[GOLDEN_FONT]);
    shared_frame_ui(&mut ui, caption);
    ui
}

fn shared_frame_ui(ui: &mut pxui::Ui, caption: &str) {
    let input = pxui_types::FrameIn { size: std::mem::size_of::<pxui_types::FrameIn>() as u32,
        width: 320.0, height: 300.0, scale: 2.0, time: 1.0 / 60.0, ..Default::default() };
    ui.frame(&input, &mut |ui: &mut pxui::Ui| ui.panel(pxui_types::ui::PanelOpts::default(), "panel", &mut |ui: &mut pxui::Ui| {
        ui.label("PXUI");
        ui.toggle("Animate", true);
        ui.toggle("Wire", false);
        ui.text_field("Caption", caption);
        ui.button("Quit");
    })).expect("build shared-device panel");
}

/// A renderer adopting `device` through one native lease, as an app hands its GPU to libpxui.
fn shared_renderer(device: &Device, ui: &pxui::Ui) -> pxui::Renderer {
    let native = ok(be::native_leased(device));
    let gpu = pxui_types::Gpu { size: native.size, backend: native.backend, instance: native.instance,
        physical_device: native.physical_device, device: native.device, queue: native.queue,
        queue_family: native.queue_family, api_version: native.api_version, features: native.features,
        get_instance_proc_addr: native.get_instance_proc_addr, pipeline_cache: native.pipeline_cache,
        mtl_device: native.mtl_device, wl_display: native.wl_display, wl_surface: native.wl_surface,
        metal_layer: native.metal_layer, hwnd: native.hwnd, owner_token: native.owner_token,
        release_owner: native.release_owner };
    pxui::Renderer::new(ui, gpu, pxui::TargetDesc { format: 1, samples: 1 }).expect("adopt app GPU")
}

fn public_commands(commands: &be::Commands) -> pxui_types::Commands {
    let native = ok(be::native_commands(commands));
    pxui_types::Commands { size: std::mem::size_of::<pxui_types::Commands>() as u32,
        vk_command_buffer: native.vk_command_buffer, mtl_command_buffer: native.mtl_command_buffer }
}

/// One app frame: PXUI prepares into the app's command buffer, draws into the app's pass, the app forgets
/// its cached pass state, ends the pass, submits once and waits.
fn shared_app_frame(queue: &Queue, target: &Texture, renderer: &mut pxui::Renderer, frame: u64, completed: u64) {
    let commands = ok(be::begin_commands(queue));
    renderer.prepare(&public_commands(&commands), frame, completed).expect("prepare shared frame");
    let clear = (228.0 / 255.0, 139.0 / 255.0, 161.0 / 255.0, 1.0);
    let encoder = ok(be::render_encoder(&commands, &RenderTarget { colors: vec![ColorAttachment {
        texture: target.clone(), resolve: None, load: Load::Clear, store: Store::Store, clear,
    }], depth: None }));
    let native_pass = ok(be::native_pass(&encoder));
    let public_pass = pxui_types::Pass { size: std::mem::size_of::<pxui_types::Pass>() as u32,
        vk_command_buffer: native_pass.vk_command_buffer, mtl_render_encoder: native_pass.mtl_render_encoder,
        width_px: native_pass.width_px, height_px: native_pass.height_px, scale: 2.0, // the app's density; a pass does not know it
        mtl_command_buffer: public_commands(&commands).mtl_command_buffer, ..pxui_types::Pass::default() };
    renderer.draw(&public_pass).expect("draw into app pass");
    ok(be::invalidate_state(&encoder));
    ok(be::end_render(&encoder));
    let submitted = ok(be::commit(&commands));
    ok(be::complete_through(queue, submitted.epoch));
}

/// Two shared frames (the second retires the first and stages new glyphs), the readback of the first
/// frame's pixels, the submits the whole run made, and teardown in the given order with the second
/// frame's upload never retired by a later prepare.
fn shared_panel(owner_first: bool) -> Option<(Vec<u8>, u64)> {
    let device = open()?;
    let queue = ok(be::create_queue(&device));
    let target = texture(&device, 640, 600, &[TextureUsage::RenderAttachment, TextureUsage::TextureCopySrc]);
    let mut ui = shared_ui("Functional UI");
    let submits = be::submits();
    let mut renderer = shared_renderer(&device, &mut ui);
    shared_app_frame(&queue, &target, &mut renderer, 1, 0);
    let submits = be::submits() - submits;
    let pixels = ok(be::read_texture(&target, 640 * 4));
    shared_frame_ui(&mut ui, "Shared device");
    shared_app_frame(&queue, &target, &mut renderer, 2, 1);

    ok(be::destroy_texture(&target));
    ok(be::destroy_queue(&queue));
    if owner_first {
        ok(be::destroy_device(&device)); // the lease keeps the native owner alive until the renderer goes
        drop(renderer);
    } else {
        drop(renderer);
        ok(be::destroy_device(&device));
    }
    assert_eq!(ui.last_error(), "", "renderer teardown reported an error");
    Some((pixels, submits))
}

fn shared_device_matches_kit_panel() {
    let Some((pixels, _)) = shared_panel(false) else { return };
    let golden = format!("{}/../../../fixtures/pxui_core/kit_panel_2x.png", env!("CARGO_MANIFEST_DIR"));
    let (w, h, expected) = png::decode(&std::fs::read(&golden).expect("kit panel golden"));
    assert_eq!((w, h), (640, 600));
    assert_eq!(pixels.len(), expected.len());
    assert!(pixels.chunks_exact(4).zip(expected.chunks_exact(4)).all(|(a, b)| near(a.try_into().unwrap(), b.try_into().unwrap(), 2)),
        "shared-device rendering diverged from kit_panel_2x.png");
}

fn shared_no_submit() {
    // Renderer creation, prepare and draw add no submit: the one in the frame is the app's own commit.
    let Some((_, submits)) = shared_panel(false) else { return };
    assert_eq!(submits, 1, "the library copy submitted work of its own");
}

fn shared_teardown() {
    let Some((first, _)) = shared_panel(true) else { return };
    let Some((second, _)) = shared_panel(false) else { return };
    assert_eq!(first, second, "both device teardown orders render identically");
}

/// A shared frame whose layer asks for a backdrop blur: the app ends its pass, the library records the blur into the
/// app's command buffer (making its scratch targets there, after the library adopted the buffer) and the app reopens
/// the pass. Those targets must be in the layout the frame reads them in when the app's buffer runs: the fatal
/// validation device fails the submit otherwise (UNASSIGNED-CoreValidation-DrawState-InvalidImageLayout).
fn shared_backdrop_blur() {
    use pxui::ui::{Look, Node, Size};
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let target = texture(&device, 640, 600, &[TextureUsage::RenderAttachment, TextureUsage::TextureCopySrc]);
    let mut ui = shared_ui("Functional UI");
    let glass = std::cell::Cell::new(None);
    let input = pxui_types::FrameIn { size: std::mem::size_of::<pxui_types::FrameIn>() as u32, width: 320.0, height: 300.0, scale: 2.0, time: 1.0 / 60.0, ..Default::default() };
    let build = |ui: &mut pxui::Ui| {
        ui.frame(&input, &mut |ui: &mut pxui::Ui| {
            glass.set(Some(ui.view("glass", 1, Size::Px(160.0), Size::Px(80.0), &mut |ui: &mut pxui::Ui| {
                ui.node("pane", Node { w: Size::Px(160.0), h: Size::Px(80.0), look: Look { fill: pxui::geom::color::rgba(0, 0, 0, 1), ..Look::NONE }, ..Node::DEFAULT });
            })));
        }).expect("build blur view");
    };
    build(&mut ui);
    let mut renderer = shared_renderer(&device, &ui);
    shared_app_frame(&queue, &target, &mut renderer, 1, 0);
    ui.view_offset(&glass.get().unwrap(), 80.0, 40.0);
    ui.view_backdrop_blur(&glass.get().unwrap(), 8.0);
    build(&mut ui);
    let commands = ok(be::begin_commands(&queue));
    renderer.prepare(&public_commands(&commands), 2, 1).expect("prepare blur frame");
    let colors = |load| RenderTarget { colors: vec![ColorAttachment { texture: target.clone(), resolve: None, load, store: Store::Store, clear: (1.0, 1.0, 1.0, 1.0) }], depth: None };
    let first = ok(be::render_encoder(&commands, &colors(Load::Clear)));
    let pass_of = |encoder: &RenderEncoder| {
        let native = ok(be::native_pass(encoder));
        pxui_types::Pass { size: std::mem::size_of::<pxui_types::Pass>() as u32, vk_command_buffer: native.vk_command_buffer, mtl_render_encoder: native.mtl_render_encoder,
            width_px: native.width_px, height_px: native.height_px, scale: 2.0, mtl_command_buffer: public_commands(&commands).mtl_command_buffer, ..pxui_types::Pass::default() }
    };
    let mut open_pass = first.clone();
    let mut splits = 0;
    let layers = ui.layers().count();
    for layer in 0..layers as u32 {
        let pass = pass_of(&open_pass);
        let mut reopened = None;
        renderer.draw_layer(&pass, layer, &mut |record| {
            splits += 1;
            ok(be::end_render(&open_pass));
            let native = ok(be::native_texture(&target));
            record(&pxui_types::Image { vk_image_view: native.vk_image_view, mtl_texture: native.mtl_texture, width: native.width, height: native.height, format: native.format, ..Default::default() })?;
            let next = ok(be::render_encoder(&commands, &colors(Load::Load)));
            let next_pass = pass_of(&next);
            reopened = Some(next);
            Ok(next_pass)
        }).unwrap_or_else(|code| panic!("draw the blurred layer: {code}: {}", ui.last_error()));
        if let Some(next) = reopened { open_pass = next; }
        ok(be::invalidate_state(&open_pass));
    }
    assert_eq!(splits, 1, "the blurred layer split the app's pass once");
    ok(be::end_render(&open_pass));
    let submitted = ok(be::commit(&commands));
    ok(be::complete_through(&queue, submitted.epoch));
    ok(be::destroy_texture(&target));
    ok(be::destroy_queue(&queue));
    drop(renderer);
    ok(be::destroy_device(&device));
    assert_eq!(ui.last_error(), "", "renderer teardown reported an error");
}

/// `prepare` refuses serials that do not increase and more frames in flight than the painter's ring holds,
/// before it records anything; the frames it accepted still render and tear down.
fn shared_frame_serials() {
    let Some(device) = open() else { return };
    let queue = ok(be::create_queue(&device));
    let target = texture(&device, 640, 600, &[TextureUsage::RenderAttachment, TextureUsage::TextureCopySrc]);
    let mut ui = shared_ui("Functional UI");
    let mut renderer = shared_renderer(&device, &mut ui);
    let frames = pxui_render::FRAMES as u64;
    shared_app_frame(&queue, &target, &mut renderer, 1, 0);
    let commands = ok(be::begin_commands(&queue));
    let native = public_commands(&commands);
    assert_eq!(renderer.prepare(&native, 1, 0), Err(-1), "a repeated frame serial");
    assert_eq!(renderer.prepare(&native, 2 + frames, 1), Err(-1), "frame - completed > FRAMES");
    assert_eq!(renderer.prepare(&native, 2, 2), Err(-1), "completed must precede the frame");
    ok(be::abandon(&commands));
    // the ring's limit itself: frame n with frame n - FRAMES the last one done
    shared_app_frame(&queue, &target, &mut renderer, 1 + frames, 1);
    let commands = ok(be::begin_commands(&queue));
    assert_eq!(renderer.prepare(&public_commands(&commands), 2 + frames, 0), Err(-1), "completed went back");
    ok(be::abandon(&commands));
    drop(renderer);
    assert!(ui.last_error().contains("completed"), "the refusal is the last error: {}", ui.last_error());
    ok(be::destroy_texture(&target));
    ok(be::destroy_queue(&queue));
    ok(be::destroy_device(&device));
}
