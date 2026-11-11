//! The same drawing through the wrapper (shapes stored into spans of the layer's storage) and through the engine's own
//! painter yields byte-identical instances, in the same order, text included.
use input::{frame, Event, Platform};
use pxui_core::ui::{Node, Ui as Engine};
use pxui_types::{geom::Rgba, node::Dir, shape::Shapes, FrameIn, Size};

const SIZE: (f32, f32) = (300.0, 200.0);

// One body for both painters: they implement `Shapes`, and `text` has the same shape on both.
macro_rules! art {
    ($p:ident) => {{
        let ink = Rgba { r: 20, g: 120, b: 220, a: 255 };
        let wash = Rgba { r: 240, g: 200, b: 60, a: 128 };
        $p.fill(2.0, 3.0, 40.0, 10.5, ink);
        $p.stroke(10.0, 10.0, 80.0, 30.0, wash);
        $p.stroke_w(12.0, 12.0, 70.0, 20.0, 3.0, ink);
        $p.frame(20.0, 20.0, 50.0, 40.0, wash);
        $p.frame_w(22.0, 22.0, 46.0, 36.0, 2.0, ink);
        $p.rect(30.0, 30.0, 20.0, 20.0, Some(ink), Some(wash));
        $p.rect(30.0, 60.0, 20.0, 20.0, None, None);
        $p.line((0.0, 0.0), (100.0, 100.0), ink);
        $p.line_w((5.0, 90.0), (95.0, 90.0), 2.0, wash);
        $p.line_w((5.5, 80.0), (60.0, 120.5), 1.5, ink);
        $p.circle((150.0, 50.0), 12.0, Some(ink), Some(wash));
        $p.circle((190.0, 50.0), 9.0, None, Some(ink));
        $p.grid(110.0, 90.0, 80.0, 60.0, (1.5, 2.5), 8.0, 2.0, wash);
        $p.chevron((250.0, 20.0), Dir::Down, ink);
        $p.brackets(200.0, 100.0, 60.0, 40.0, 3.0, 8.0, 1.0, wash);
        $p.dashed((10.0, 150.0), (200.0, 150.0), ink);
        $p.dashed_rect(10.0, 160.0, 90.0, 30.0, wash);
        $p.flag((270.0, 150.0), true, true);
        $p.flag((270.0, 180.0), false, false);
        // text between shapes: it must land after what was drawn before it and before what follows
        $p.text((8.0, 120.0), "shapes then text");
        $p.fill(150.0, 150.0, 30.0, 12.0, ink);
        $p.cap((150.0, 170.0), None, "caption");
        // more shapes than one span holds
        for i in 0..700 {
            $p.fill((i % 50) as f32 * 5.0, (i / 50) as f32 * 4.0, 3.0, 3.0, if i % 2 == 0 { ink } else { wash });
        }
        $p.line((0.0, 199.0), (299.0, 199.0), ink);
    }};
}

fn engine_instances() -> Vec<pxui_types::draw::Instance> {
    let mut ui = Engine::create(Platform::Apple, None, pxui_core::text::Text::new(&[pxui_core::text::PRAGMASEVKA]));
    let mut input = input::at_time(0.0);
    input.size = (SIZE.0 as i64, SIZE.1 as i64);
    input.pointer = (-100.0, -100.0);
    input.events = vec![Event::PointerMoved((-100.0, -100.0))];
    let frame = frame::Frame { input, pixel_scale: (1.0, 1.0), fps: 60.0, mouse_delta: (0.0, 0.0) };
    ui.frame(&frame, &mut |ui| {
        let b = ui.node("canvas", Node { w: Size::Px(SIZE.0), h: Size::Px(SIZE.1), ..Node::DEFAULT });
        ui.canvas(&b, |p, _| art!(p));
    });
    ui.output().draw.iter().flat_map(|layer| layer.instances.iter().copied()).collect()
}

fn wrapper_instances() -> Vec<pxui_types::draw::Instance> {
    let mut ui = pxui::Ui::create(pxui::Platform::Apple, None, &[pxui_core::text::PRAGMASEVKA]);
    ui.frame(&FrameIn { width: SIZE.0, height: SIZE.1, scale: 1.0, ..FrameIn::default() }, &mut |ui: &mut pxui::Ui| {
        let b = ui.node("canvas", pxui::Node { w: Size::Px(SIZE.0), h: Size::Px(SIZE.1), ..pxui::Node::DEFAULT });
        ui.canvas(&b, |p, _| art!(p));
    }).expect("frame");
    // The wrapper's `Ui` is the library's `PxuiUi` pointer, a `#[repr(C)]` struct whose first field is the engine it built the
    // frame in: read the layers from there.
    let raw: *mut *mut Engine = unsafe { core::mem::transmute_copy(&ui) };
    let engine = unsafe { &**raw };
    engine.output().draw.iter().flat_map(|layer| layer.instances.iter().copied()).collect()
}

#[test]
fn span_equals_engine() {
    let (direct, wrapped) = (engine_instances(), wrapper_instances());
    assert!(direct.len() > 700, "the drawing crosses a span boundary: {} instances", direct.len());
    assert_eq!(wrapped.len(), direct.len(), "same number of instances");
    for (i, (a, b)) in wrapped.iter().zip(&direct).enumerate() {
        assert_eq!(a, b, "instance {i} differs");
    }
}
