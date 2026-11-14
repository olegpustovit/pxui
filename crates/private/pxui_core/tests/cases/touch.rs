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
                    .flags(CLIP | if scroll { SCROLL } else { 0 }),
            );
            let saved = ui.enter(&root);
            let inner = if nested {
                ui.box_(
                    "sideways",
                    BoxOpts::default()
                        .w(Size::Px(120.0))
                        .h(Size::Px(400.0))
                        .flags(CLIP | SCROLL_X),
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
                    .flags(CLICKABLE),
            );
            row_handle = Some(row);
            let signal = ui.signal_state(&row);
            if let Some(saved) = inner_saved {
                ui.leave(saved);
            }
            ui.leave(saved);
            if modal {
                modal_open = ui.modal(Some(80.0), "touch-blocker", &mut |ui| ui.label("modal"));
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
        "touch_coast_normal_decay_catch_rubber_band_and_warm_allocations",
        touch_coast_normal_decay_catch_rubber_band_and_warm_allocations,
    ),
];
