use crate::support::{frame_at, new_ui};
use input::{Event, Touch, TouchPhase};
use pxui_core::ui::*;

fn touch(phase: TouchPhase, point: (f64, f64), time: f64) -> Event {
    finger(phase, point, time, 1)
}
fn finger(phase: TouchPhase, point: (f64, f64), time: f64, id: u64) -> Event {
    Event::Touch(Touch {
        device: u64::MAX,
        finger: id,
        point,
        time,
        pressure: 1.0,
        phase,
    })
}
struct Scene {
    ui: Ui,
    frame: input::frame::Frame,
    scroll: bool,
    nested: bool,
    modal: bool,
    replace: bool,
    row: Option<Handle>,
}
impl Scene {
    fn new(scroll: bool, nested: bool) -> Self {
        let mut frame = frame_at(vec![], 2.0, 0.0, Vec::with_capacity(32));
        frame.input.events.clear();
        Self {
            ui: new_ui(None),
            frame,
            scroll,
            nested,
            modal: false,
            replace: false,
            row: None,
        }
    }
    fn step(&mut self, time: f64, events: &[Event]) -> (Handle, Handle, Signal) {
        self.frame.input.t = time;
        self.frame.input.events.clear();
        self.frame.input.events.extend_from_slice(events);
        let (scroll, nested, modal, replace) = (self.scroll, self.nested, self.modal, self.replace);
        let mut out = None;
        let mut row_handle = None;
        let mut modal_open = modal;
        self.ui.frame(&self.frame, &mut |ui| {
            let root = ui.box_(
                "touch-scroll",
                BoxOpts::default()
                    .at((0.0, 0.0))
                    .w(Size::Px(120.0))
                    .h(Size::Px(120.0))
                    .flags(Flags::CLIP | if scroll { Flags::SCROLL } else { Flags::NONE }),
            );
            let saved = ui.enter(&root);
            let inner = if nested {
                ui.box_(
                    "sideways",
                    BoxOpts::default()
                        .w(Size::Px(120.0))
                        .h(Size::Px(400.0))
                        .flags(Flags::CLIP | Flags::SCROLL_X),
                )
            } else {
                root
            };
            let inner_saved = if nested { Some(ui.enter(&inner)) } else { None };
            let row = ui.box_(
                if replace {
                    "replacement-row"
                } else {
                    "touch-row"
                },
                BoxOpts::default()
                    .w(Size::Px(if nested { 2000.0 } else { 120.0 }))
                    .h(Size::Px(if nested { 400.0 } else { 4000.0 }))
                    .flags(Flags::CLICKABLE),
            );
            row_handle = Some(row);
            let signal = ui.signal(&row);
            if let Some(saved) = inner_saved {
                ui.leave(saved);
            }
            ui.leave(saved);
            if modal {
                modal_open = ui.modal("touch-blocker", &pxui_core::ui::Overlay::DEFAULT.w(80.0).name("touch-blocker"), &mut |ui| ui.label("modal", pxui_core::ui::Label::DEFAULT));
            }
            out = Some((root, inner, signal));
        });
        self.row = row_handle;
        self.modal = modal_open;
        out.unwrap()
    }
}

fn touch_taps_scroll_slop_cancellation_and_direct_controls() {
    use TouchPhase::*;
    let mut scene = Scene::new(true, false);
    scene.step(0.0, &[]);
    let (_, _, signal) = scene.step(0.01, &[touch(Began, (50.0, 80.0), 0.01)]);
    assert!(!signal.pressed && !signal.held && !signal.clicked);
    let (_, _, signal) = scene.step(0.02, &[touch(Ended, (51.0, 81.0), 0.02)]);
    assert!(signal.clicked && signal.released);
    scene.step(0.03, &[touch(Began, (50.0, 80.0), 0.03)]);
    let (root, _, signal) = scene.step(0.04, &[touch(Moved, (50.0, 73.0), 0.04)]);
    assert_eq!(scene.ui.scroll_offset(&root), 0.0);
    assert!(!signal.held && !signal.pressed);
    let (root, _, signal) = scene.step(0.05, &[touch(Moved, (50.0, 60.0), 0.05)]);
    assert_eq!(scene.ui.scroll_offset(&root), 20.0);
    assert!(!signal.clicked && !signal.pressed && !signal.held);
    let (_, _, signal) = scene.step(0.06, &[touch(Cancelled, (50.0, 60.0), 0.06)]);
    assert!(!signal.clicked && !signal.released);
    let (root, _, _) = scene.step(1.0, &[]);
    assert_eq!(
        scene.ui.scroll_offset(&root),
        20.0,
        "cancellation must not fling"
    );
    let mut direct = Scene::new(false, false);
    direct.step(0.0, &[]);
    let (_, _, signal) = direct.step(0.1, &[touch(Began, (50.0, 50.0), 0.1)]);
    assert!(signal.pressed && signal.held);
    let routed = direct.ui.input(Some(direct.row.unwrap().box_key));
    assert_eq!(routed.input.events.len(), 1);
    assert!(
        matches!(routed.input.events[0], Event::Touch(_)),
        "nested viewports must not receive duplicate synthetic pointer actions"
    );

    let (_, _, signal) = direct.step(0.2, &[touch(Moved, (70.0, 50.0), 0.2)]);
    assert!(signal.dragging && signal.held);
    assert_eq!(signal.drag, (20.0, 0.0));
    let (_, _, signal) = direct.step(0.3, &[touch(Cancelled, (70.0, 50.0), 0.3)]);
    assert!(!signal.clicked && !signal.released && !signal.held);
    direct.step(0.4, &[touch(Began, (50.0, 50.0), 0.4)]);
    direct.step(
        0.5,
        &[Event::PointerPressed(
            input::MouseButton::LeftButton,
            (70.0, 50.0),
        )],
    );
    let (_, _, signal) = direct.step(0.6, &[touch(Ended, (50.0, 50.0), 0.6)]);
    assert!(
        !signal.clicked && signal.held,
        "a finger release must not release the new mouse capture"
    );
    let (_, _, signal) = direct.step(
        0.7,
        &[Event::PointerReleased(
            input::MouseButton::LeftButton,
            (70.0, 50.0),
        )],
    );
    assert!(signal.clicked && signal.released);
    direct.ui.destroy();
    let mut popup = Scene::new(true, false);
    popup.modal = true;
    popup.step(0.0, &[]);
    popup.step(0.01, &[touch(Began, (1.0, 1.0), 0.01)]);
    assert!(
        !popup.modal,
        "a finger press outside must dismiss the kit popup"
    );
    let (_, _, signal) = popup.step(0.02, &[touch(Ended, (1.0, 1.0), 0.02)]);
    assert!(
        !signal.clicked,
        "dismissing a popup must not tap through it"
    );
    popup.ui.destroy();
    scene.ui.destroy();
}

fn touch_nested_axis_lock_multitouch_and_modal_cancellation() {
    use TouchPhase::*;
    for horizontal in [false, true] {
        let mut scene = Scene::new(true, true);
        scene.step(0.0, &[]);
        scene.step(0.01, &[touch(Began, (80.0, 80.0), 0.01)]);
        let point = if horizontal {
            (60.0, 78.0)
        } else {
            (78.0, 60.0)
        };
        let (root, inner, signal) = scene.step(0.02, &[touch(Moved, point, 0.02)]);
        assert!(!signal.clicked);
        assert_eq!(
            scene.ui.scroll_offset(&root),
            if horizontal { 0.0 } else { 20.0 }
        );
        assert_eq!(
            scene.ui.scroll_offset_x(&inner),
            if horizontal { 20.0 } else { 0.0 }
        );
        let (root, inner, _) = scene.step(0.03, &[touch(Moved, (50.0, 50.0), 0.03)]);
        assert_eq!(
            scene.ui.scroll_offset(&root),
            if horizontal { 0.0 } else { 30.0 }
        );
        assert_eq!(
            scene.ui.scroll_offset_x(&inner),
            if horizontal { 30.0 } else { 0.0 }
        );
        scene.step(0.04, &[Event::FocusLost]);
        scene.ui.destroy();
    }
    let mut scene = Scene::new(true, false);
    scene.step(0.0, &[]);
    scene.step(0.01, &[touch(Began, (50.0, 50.0), 0.01)]);
    scene.step(0.02, &[finger(Began, (60.0, 50.0), 0.02, 2)]);
    let (_, _, signal) = scene.step(0.03, &[touch(Ended, (50.0, 50.0), 0.03)]);
    assert!(!signal.clicked);
    let (_, _, signal) = scene.step(0.04, &[finger(Ended, (60.0, 50.0), 0.04, 2)]);
    assert!(!signal.clicked);
    scene.step(0.05, &[touch(Began, (50.0, 50.0), 0.05)]);
    let (_, _, signal) = scene.step(0.06, &[touch(Ended, (50.0, 50.0), 0.06)]);
    assert!(
        signal.clicked,
        "a new single finger must work after all fingers lift"
    );
    scene.step(0.07, &[touch(Began, (50.0, 50.0), 0.07)]);
    scene.modal = true;
    scene.step(0.08, &[]);
    scene.modal = false;
    scene.step(0.09, &[]);
    let (_, _, signal) = scene.step(0.1, &[touch(Ended, (50.0, 50.0), 0.1)]);
    assert!(!signal.clicked);
    scene.ui.destroy();
    let mut scene = Scene::new(true, false);
    scene.step(0.0, &[]);
    scene.step(0.01, &[touch(Began, (50.0, 50.0), 0.01)]);
    scene.replace = true;
    scene.step(0.02, &[]);
    let (_, _, signal) = scene.step(0.03, &[touch(Ended, (50.0, 50.0), 0.03)]);
    assert!(
        !signal.clicked,
        "a replacement node must not inherit the old pending tap"
    );
    scene.ui.destroy();
}

fn touch_coast_normal_decay_catch_rubber_band_and_warm_allocations() {
    use TouchPhase::*;
    for rate in [60.0, 120.0] {
        let mut scene = Scene::new(true, false);
        scene.step(0.0, &[]);
        scene.step(0.0, &[touch(Began, (50.0, 100.0), 0.0)]);
        scene.step(0.016, &[touch(Moved, (50.0, 90.0), 0.016)]);
        scene.step(0.032, &[touch(Moved, (50.0, 80.0), 0.032)]);
        scene.step(0.04, &[touch(Ended, (50.0, 80.0), 0.04)]);
        let mut offset = 0.0;
        for frame in 1..=rate as usize {
            let (root, _, signal) = scene.step(0.04 + frame as f64 / rate, &[]);
            offset = scene.ui.scroll_offset(&root);
            assert!(!signal.clicked);
        }
        let tau = -1.0 / (1000.0 * 0.998f64.ln());
        let expected = 20.0 + 625.0 * tau * (1.0 - (-1.0 / tau).exp());
        assert!(
            (f64::from(offset) - expected).abs() <= 0.5,
            "normal decay at {rate} Hz: {offset}, expected {expected}"
        );
        let (root, _, _) = scene.step(1.05, &[touch(Began, (50.0, 60.0), 1.05)]);
        let caught = scene.ui.scroll_offset(&root);
        let (root, _, signal) = scene.step(1.06, &[touch(Ended, (50.0, 60.0), 1.06)]);
        assert!(
            !signal.clicked,
            "touch-down while coasting must not tap a row"
        );
        assert_eq!(scene.ui.scroll_offset(&root), caught);
        let (root, _, _) = scene.step(2.0, &[]);
        assert_eq!(scene.ui.scroll_offset(&root), caught);
        scene.ui.set_scroll_offset(&root, 0.0);
        scene.step(3.0, &[touch(Began, (50.0, 50.0), 3.0)]);
        let (root, _, _) = scene.step(3.02, &[touch(Moved, (50.0, 110.0), 3.02)]);
        let expected = -0.55 * 60.0 / (1.0 + 0.55 * 60.0 / 120.0);
        let held = scene.ui.scroll_position(&root);
        assert!((f64::from(held) - expected).abs() < 0.001);
        let (root, _, _) = scene.step(4.0, &[]);
        assert_eq!(scene.ui.scroll_position(&root), held);
        assert!(
            scene.ui.repaint_at().is_none(),
            "a stationary held stretch must idle"
        );
        scene.step(4.01, &[touch(Cancelled, (50.0, 110.0), 4.01)]);
        for frame in 1..=240 {
            scene.step(4.01 + frame as f64 / rate, &[]);
        }
        for frame in 0..182 {
            let time = 10.0 + frame as f64 / rate;
            let event = match frame % 4 {
                0 => touch(Began, (50.0, 80.0), time),
                1 => touch(Moved, (50.0, 60.0), time),
                2 => touch(Cancelled, (50.0, 60.0), time),
                _ => Event::EnvironmentChanged,
            };
            mem_sys::local_reset();
            let (_, _, signal) = scene.step(time, &[event]);
            assert!(!signal.clicked);
            if frame >= 82 {
                assert_eq!(mem_sys::local_alloc_count(), 0, "warm finger frame {frame}");
            }
        }
        scene.ui.destroy();
    }
}

// Frame-by-frame traces of UIScrollView's physics: the rubber band (x past the edge shows as
// (1 - 1/(x * 0.55 / d + 1)) * d), the spring-back decay and the 0.998-per-millisecond coast.
fn touch_trace_rubber_band_spring_back_and_coast_per_frame() {
    use TouchPhase::*;
    let mut scene = Scene::new(true, false);
    scene.step(0.0, &[]);
    let d = 120.0;
    for (i, pull) in [20.0f64, 60.0, 150.0, 400.0].into_iter().enumerate() {
        let t = 1.0 + i as f64 * 10.0;
        scene.step(t, &[touch(Began, (50.0, 20.0), t)]);
        scene.step(t + 0.016, &[touch(Moved, (50.0, 32.0), t + 0.016)]);
        let (root, _, _) = scene.step(t + 0.032, &[touch(Moved, (50.0, 32.0 + pull), t + 0.032)]);
        let expected = (1.0 - 1.0 / ((pull + 12.0) * 0.55 / d + 1.0)) * d;
        let shown = f64::from(-scene.ui.scroll_position(&root));
        assert!((shown - expected).abs() < 0.01, "pull {pull}: {shown} vs {expected}");
        // the stretch decays as exp(-t/0.084) on the frame clock, at any frame rate
        scene.step(t + 0.04, &[touch(Cancelled, (50.0, 32.0 + pull), t + 0.04)]);
        let (root, _, _) = scene.step(t + 0.05, &[]);
        let start = f64::from(-scene.ui.scroll_position(&root));
        for frame in 1..=12 {
            let dt = f64::from(frame) * 0.016;
            let (root, _, _) = scene.step(t + 0.05 + dt, &[]);
            let want = start * (-dt / 0.084).exp();
            let got = f64::from(-scene.ui.scroll_position(&root));
            assert!((got - want).abs() < 0.3 + 0.002 * start, "pull {pull} frame {frame}: {got} vs {want}");
        }
        scene.step(t + 5.0, &[]);
    }
    // a flick: v(t) = v0 * 0.998^(1000 t), the position integrating it, frame by frame at 120 Hz
    scene.step(100.0, &[touch(Began, (50.0, 100.0), 100.0)]);
    scene.step(100.016, &[touch(Moved, (50.0, 90.0), 100.016)]);
    scene.step(100.032, &[touch(Moved, (50.0, 80.0), 100.032)]);
    scene.step(100.04, &[touch(Ended, (50.0, 80.0), 100.04)]);
    let tau = -1.0 / (1000.0 * 0.998f64.ln());
    for frame in 1..=240 {
        let dt = f64::from(frame) / 120.0;
        let (root, _, signal) = scene.step(100.04 + dt, &[]);
        let want = 20.0 + 625.0 * tau * (1.0 - (-dt / tau).exp());
        let got = f64::from(scene.ui.scroll_offset(&root));
        assert!((got - want).abs() <= 0.5, "frame {frame}: {got} vs {want}");
        assert!(!signal.clicked);
    }
    scene.ui.destroy();
}

pub const CASES: &[(&str, fn())] = &[
    (
        "touch_taps_scroll_slop_cancellation_and_direct_controls",
        touch_taps_scroll_slop_cancellation_and_direct_controls,
    ),
    (
        "touch_nested_axis_lock_multitouch_and_modal_cancellation",
        touch_nested_axis_lock_multitouch_and_modal_cancellation,
    ),
    (
        "touch_trace_rubber_band_spring_back_and_coast_per_frame",
        touch_trace_rubber_band_spring_back_and_coast_per_frame,
    ),
    (
        "touch_coast_normal_decay_catch_rubber_band_and_warm_allocations",
        touch_coast_normal_decay_catch_rubber_band_and_warm_allocations,
    ),
];
