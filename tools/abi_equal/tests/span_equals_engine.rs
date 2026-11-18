//! The same drawing through the wrapper (shapes stored into spans of the layer's storage) and through the engine's own
//! painter yields byte-identical instances, in the same order, text included.
use input::{frame, Event, Platform};
use pxui_core::ui::{Node, Ui as Engine};
use pxui::{Fill, Label, Stroke};
use pxui::FrameIn;
use pxui_types::{geom::{Rect, Rgba}, node::Dir, shape::Shapes, theme::Look, Size};

const SIZE: (f32, f32) = (300.0, 200.0);

// The same drawing on both painters: the engine's through its `Shapes` calls, the wrapper's through its own `Rect` + `Fill`/`Stroke`
// shapes, each line of one the twin of the same line of the other; text through the label path on both. (A clip changed mid-drawing
// across a span refill is `paint::tests::a_clip_set_mid_drawing_restamps_every_refilled_span`.)
const INK: Rgba = Rgba { r: 20, g: 120, b: 220, a: 255 };
const WASH: Rgba = Rgba { r: 240, g: 200, b: 60, a: 128 };
const CAPTION: Label = Label::CAPTION.caps(true);

fn engine_art(p: &mut pxui_core::ui::Paint) {
    let (ink, wash) = (INK, WASH);
    p.fill(2.0, 3.0, 40.0, 10.5, ink);
    p.fill_fractional(3.25, 15.5, 20.5, 0.5, wash);
    p.stroke(10.0, 10.0, 80.0, 30.0, wash);
    p.stroke_w(12.0, 12.0, 70.0, 20.0, 3.0, ink);
    p.frame(20.0, 20.0, 50.0, 40.0, wash);
    p.frame_w(22.0, 22.0, 46.0, 36.0, 2.0, ink);
    p.line((0.0, 0.0), (100.0, 100.0), ink);
    p.line_w((5.0, 90.0), (95.0, 90.0), 2.0, wash);
    p.line_w((5.5, 80.0), (60.0, 120.5), 1.5, ink);
    p.circle((150.0, 50.0), 12.0, Some(ink), Some((1.0, wash)));
    p.circle((190.0, 50.0), 9.0, None, Some((2.0, ink)));
    p.grid(110.0, 90.0, 80.0, 60.0, (1.5, 2.5), 8.0, 2.0, wash);
    p.chevron((250.0, 20.0), Dir::Down, 1.5, ink);
    p.brackets(200.0, 100.0, 60.0, 40.0, 3.0, 8.0, 1.0, wash);
    p.dashed((10.0, 150.0), (200.0, 150.0), ink);
    p.dashed_w((10.0, 155.0), (200.0, 155.0), 2.0, wash);
    p.dashed_rect(10.0, 160.0, 90.0, 30.0, 1.0, wash);
    p.triangle((5.0, 5.0), (5.0, 15.0), (14.0, 10.0), ink);
    p.curve((20.0, 5.0), (25.0, 15.0), (30.0, 5.0), 1.5, wash);
    p.cubic([(40.0, 5.0), (44.0, 13.0), (48.0, -3.0), (52.0, 5.0)], 1.5, ink);
    p.flag((270.0, 150.0), true, true);
    p.flag((270.0, 180.0), false, false);
    // text between shapes: it must land after what was drawn before it and before what follows
    p.label((8.0, 120.0), Label::BODY.fields(), 0.0, "shapes then text");
    p.fill(150.0, 150.0, 30.0, 12.0, ink);
    p.label((150.0, 170.0), CAPTION.fields(), 0.0, "caption");
    // more shapes than one span holds
    for i in 0..1300 {
        p.fill((i % 50) as f32 * 5.0, (i / 50) as f32 * 4.0, 3.0, 3.0, if i % 2 == 0 { ink } else { wash });
    }
    p.line((0.0, 199.0), (299.0, 199.0), ink);
    p.fill(1.0, 1.0, 9.0, 9.0, ink);
}

fn wrapper_art(p: &mut pxui::Paint<'_>) {
    let (ink, wash) = (Fill::new(INK), Fill::new(WASH));
    let (ink_line, wash_line) = (Stroke::new(INK), Stroke::new(WASH));
    p.fill(Rect::new(2.0, 3.0, 40.0, 10.5), ink);
    p.fill(Rect::new(3.25, 15.5, 20.5, 0.5), wash.fractional());
    p.stroke(Rect::new(10.0, 10.0, 80.0, 30.0), wash_line);
    p.stroke(Rect::new(12.0, 12.0, 70.0, 20.0), ink_line.width(3.0));
    p.frame(Rect::new(20.0, 20.0, 50.0, 40.0), wash_line);
    p.frame(Rect::new(22.0, 22.0, 46.0, 36.0), ink_line.width(2.0));
    p.line((0.0, 0.0), (100.0, 100.0), ink_line);
    p.line((5.0, 90.0), (95.0, 90.0), wash_line.width(2.0));
    p.line((5.5, 80.0), (60.0, 120.5), ink_line.width(1.5));
    p.circle((150.0, 50.0), 12.0, Some(ink), Some(wash_line));
    p.circle((190.0, 50.0), 9.0, None, Some(ink_line.width(2.0)));
    p.grid(Rect::new(110.0, 90.0, 80.0, 60.0), (1.5, 2.5), 8.0, 2.0, wash);
    p.chevron((250.0, 20.0), Dir::Down, ink_line.width(1.5));
    p.brackets(Rect::new(200.0, 100.0, 60.0, 40.0), 3.0, 8.0, wash_line);
    p.dashed((10.0, 150.0), (200.0, 150.0), ink_line);
    p.dashed((10.0, 155.0), (200.0, 155.0), wash_line.width(2.0));
    p.dashed_rect(Rect::new(10.0, 160.0, 90.0, 30.0), wash_line);
    p.triangle((5.0, 5.0), (5.0, 15.0), (14.0, 10.0), ink);
    p.curve((20.0, 5.0), (25.0, 15.0), (30.0, 5.0), wash_line.width(1.5));
    p.cubic([(40.0, 5.0), (44.0, 13.0), (48.0, -3.0), (52.0, 5.0)], ink_line.width(1.5));
    p.flag((270.0, 150.0), true, true);
    p.flag((270.0, 180.0), false, false);
    p.text(pxui::Point::new(8.0, 120.0), Label::BODY, 0.0, "shapes then text");
    p.fill(Rect::new(150.0, 150.0, 30.0, 12.0), ink);
    p.text(pxui::Point::new(150.0, 170.0), CAPTION, 0.0, "caption");
    for i in 0..1300 {
        p.fill(Rect::new((i % 50) as f32 * 5.0, (i / 50) as f32 * 4.0, 3.0, 3.0), if i % 2 == 0 { ink } else { wash });
    }
    p.line((0.0, 199.0), (299.0, 199.0), ink_line);
    p.fill(Rect::new(1.0, 1.0, 9.0, 9.0), ink);
}

fn engine_instances() -> Vec<pxui_types::draw::Instance> {
    let mut ui = Engine::create(Platform::Apple, None, pxui_core::text::Text::new(&[pxui_core::text::PRAGMASEVKA]));
    let mut input = input::at_time(0.0);
    input.size = (SIZE.0 as i64, SIZE.1 as i64);
    input.pointer = (-100.0, -100.0);
    input.events = vec![Event::PointerMoved((-100.0, -100.0))];
    let frame = frame::Frame { input, pixel_scale: (1.0, 1.0), fps: 60.0, mouse_delta: (0.0, 0.0) };
    ui.frame(&frame, &mut |ui| {
        let round = ui.node("round".into(), Node { flags: pxui_types::Flags::CLIP | pxui_types::Flags::ROUND_CLIP, w: Size::Px(SIZE.0), h: Size::Px(SIZE.1), look: Look { radius: [6.0; 4], ..Look::NONE }, ..Node::DEFAULT });
        ui.within(&round, &mut |ui| {
            let b = ui.node("canvas".into(), Node { w: Size::Px(SIZE.0), h: Size::Px(SIZE.1), ..Node::DEFAULT });
            ui.canvas(&b, |p, _| engine_art(p));
        });
    });
    ui.output().draw.iter().flat_map(|layer| layer.instances.iter().copied()).collect()
}

fn wrapper_instances() -> Vec<pxui_types::draw::Instance> {
    let mut ui = pxui::Ui::new(&pxui::UiDesc::new(pxui::Platform::Apple).fonts(&[pxui::FontData(pxui_core::text::PRAGMASEVKA)])).expect("pxui Ui");
    ui.frame(&FrameIn::new(SIZE.0, SIZE.1).scale(1.0), &mut |ui: &mut pxui::Ui| {
        let round = ui.node("round", pxui::Node { flags: pxui_types::Flags::CLIP | pxui_types::Flags::ROUND_CLIP, w: Size::Px(SIZE.0), h: Size::Px(SIZE.1), look: Look { radius: [6.0; 4], ..Look::NONE }, ..pxui::Node::DEFAULT });
        ui.within(&round, &mut |ui: &mut pxui::Ui| {
            let b = ui.node("canvas", pxui::Node { w: Size::Px(SIZE.0), h: Size::Px(SIZE.1), ..pxui::Node::DEFAULT });
            ui.canvas(&b, |p, _| wrapper_art(p));
        });
    }).expect("frame");
    // the engine the library built the frame in
    let engine = unsafe { &*ui.engine_ptr().cast::<Engine>() };
    engine.output().draw.iter().flat_map(|layer| layer.instances.iter().copied()).collect()
}

#[test]
fn span_equals_engine() {
    let (direct, wrapped) = (engine_instances(), wrapper_instances());
    assert!(direct.len() > 1300, "the drawing crosses span boundaries: {} instances", direct.len());
    assert!(direct.iter().any(|i| i.clip_radius != 0), "a rounded clip is stamped");
    assert_eq!(wrapped.len(), direct.len(), "same number of instances");
    for (i, (a, b)) in wrapped.iter().zip(&direct).enumerate() {
        assert_eq!(a, b, "instance {i} differs");
    }
}
