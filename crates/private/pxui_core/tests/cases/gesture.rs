//! UIKit-style touch gestures as timestamped traces: every assertion is a frame of the trace.
use crate::support::{frame_at, new_ui};
use input::{Event, Touch, TouchPhase::*};
use pxui_core::ui::gesture::*;
use pxui_core::ui::*;

fn touch(phase: input::TouchPhase, point: (f64, f64), time: f64) -> Event {
    Event::Touch(Touch { device: u64::MAX, finger: 1, point, time, pressure: 1.0, phase })
}

const ROWS: [&str; 10] = ["row-0", "row-1", "row-2", "row-3", "row-4", "row-5", "row-6", "row-7", "row-8", "row-9"];

struct Scene {
    ui: Ui,
    frame: input::frame::Frame,
    rows: usize,
    row: GestureCfg,
    nav: GestureCfg,
    list: GestureCfg,
    commands: [u32; 3],
    dismiss: Option<(f32, f32)>,
    list_box: Option<UiBox>,
}
struct Seen {
    dismiss: DismissState,
    row: GestureState,
    nav: GestureState,
    list: GestureState,
    clicked: bool,
    offset: f32,
    position: f32,
}
impl Scene {
    fn new() -> Self {
        let mut frame = frame_at(vec![], 2.0, 0.0, Vec::with_capacity(32));
        frame.input.events.clear();
        let none = GestureCfg::default();
        let mut scene = Self { ui: new_ui(None), frame, rows: 10, row: none, nav: none, list: none, commands: [0; 3], dismiss: None, list_box: None };
        scene.step(0.0, &[]);
        scene
    }
    fn step(&mut self, time: f64, events: &[Event]) -> Seen {
        self.frame.input.t = time;
        self.frame.input.events.clear();
        self.frame.input.events.extend_from_slice(events);
        let (rows, row_cfg, nav_cfg, list_cfg, dismiss, commands) = (self.rows, self.row, self.nav, self.list, self.dismiss, std::mem::take(&mut self.commands));
        let mut seen = None;
        let mut list_box = None;
        self.ui.frame(&self.frame, &mut |ui| {
            let nav = ui.box_("nav", BoxOpts::default().at((0.0, 0.0)).w(Size::Px(200.0)).h(Size::Px(200.0)).flags(CLIP));
            let nav_state = ui.gesture(&nav, Some(nav_cfg), commands[0]);
            let dismissed = dismiss.map_or_else(DismissState::default, |(fraction, velocity)| ui.dismiss(&nav, fraction, velocity));
            let saved = ui.enter(&nav);
            let list = ui.box_("list", BoxOpts::default().w(Size::Px(200.0)).h(Size::Px(200.0)).flags(CLIP | SCROLL));
            ui.gesture(&list, Some(list_cfg), commands[2]);
            list_box = Some(list);
            let inner = ui.enter(&list);
            let mut first = None;
            for i in 0..rows {
                let row = ui.box_(ROWS[i], BoxOpts::default().w(Size::Px(200.0)).h(Size::Px(50.0)).flags(CLICKABLE));
                if i == 0 {
                    first = Some((ui.gesture(&row, Some(row_cfg), commands[1]), ui.signal_state(&row).clicked));
                }
            }
            ui.leave(inner);
            ui.leave(saved);
            let (row, clicked) = first.unwrap();
            seen = Some((row, nav_state, clicked, dismissed));
        });
        // scroll motion is applied by layout, after the build: read it once the frame is done
        let (row, nav, clicked, dismiss) = seen.unwrap();
        let list = list_box.unwrap();
        self.list_box = Some(list);
        Seen { dismiss, row, nav, list: self.ui.gesture(&list, None, 0), clicked, offset: self.ui.scroll_offset(&list), position: self.ui.scroll_position(&list) }
    }
}

fn long_press_highlights_after_delay_fires_once_and_swallows_the_tap() {
    let mut s = Scene::new();
    s.row = GestureCfg { kinds: LONG_PRESS, ..Default::default() };
    s.step(0.0, &[]);
    let seen = s.step(1.0, &[touch(Began, (100.0, 25.0), 1.0)]);
    assert_eq!(seen.row.flags, 0, "no highlight at touch down");
    assert_eq!(s.ui.repaint_at(), Some(1.1), "the highlight asks for its frame");
    assert_eq!(s.step(1.05, &[]).row.flags, 0);
    // movement inside the 10 pt slop keeps the contact pending
    assert_eq!(s.step(1.08, &[touch(Moved, (106.0, 29.0), 1.08)]).row.flags, 0);
    let seen = s.step(1.1, &[]);
    assert_eq!(seen.row.flags, HIGHLIGHTED);
    assert_eq!(s.ui.repaint_at(), Some(1.5));
    assert_eq!(s.step(1.3, &[]).row.flags, HIGHLIGHTED);
    let seen = s.step(1.5, &[]);
    assert_eq!(seen.row.flags, LONG_PRESSED, "fires at 0.5 s and drops the highlight");
    assert!(s.ui.repaint_at().is_none(), "nothing more is pending");
    assert_eq!(s.step(1.6, &[]).row.flags, 0, "the signal is a one-frame pulse");
    let seen = s.step(1.7, &[touch(Moved, (100.0, 200.0), 1.7), touch(Ended, (100.0, 200.0), 1.7)]);
    assert!(!seen.clicked && seen.offset == 0.0, "a long press is not a tap and does not scroll");
    // a quick tap flashes the highlight and clicks
    s.step(2.0, &[touch(Began, (100.0, 25.0), 2.0)]);
    let seen = s.step(2.04, &[touch(Ended, (100.0, 25.0), 2.04)]);
    assert!(seen.clicked && seen.row.flags == HIGHLIGHTED);
    assert!(s.ui.repaint_at().is_none_or(|t| t > 2.5), "a released contact leaves no timer");
    // leaving the slop cancels both timers and scrolls
    s.step(3.0, &[touch(Began, (100.0, 25.0), 3.0)]);
    s.step(3.12, &[touch(Moved, (100.0, 40.0), 3.12)]);
    assert_eq!(s.step(3.13, &[]).row.flags, 0, "highlight cleared once the finger moved");
    assert_eq!(s.step(3.7, &[]).row.flags, 0, "no long press after a drag");
    s.ui.destroy();
}

fn row_swipe_axis_lock_offset_snap_and_close() {
    for rtl in [false, true] {
        let sign = if rtl { 1.0f32 } else { -1.0 };
        let mut s = Scene::new();
        if rtl {
            s.ui.set_dir(Direction::Rtl);
        }
        let dir = if rtl { -1.0 } else { 1.0 }; // finger x travel toward the actions is -dir
        s.row = GestureCfg { kinds: ROW_SWIPE, reveal: 80.0, ..Default::default() };
        s.step(0.0, &[]);
        let x = |d: f64| 150.0 - dir * d;
        // a tap on a swipe row is still a tap
        s.step(0.5, &[touch(Began, (x(0.0), 25.0), 0.5)]);
        assert!(s.step(0.52, &[touch(Ended, (x(1.0), 25.0), 0.52)]).clicked);
        s.step(1.0, &[touch(Began, (x(0.0), 25.0), 1.0)]);
        // 7 pt: still deciding
        let seen = s.step(1.016, &[touch(Moved, (x(7.0), 26.0), 1.016)]);
        assert_eq!((seen.row.flags, seen.row.swipe), (0, 0.0));
        // 8 pt locks horizontal: the row follows the finger from the touch-down point
        let seen = s.step(1.032, &[touch(Moved, (x(9.0), 26.0), 1.032)]);
        assert_eq!((seen.row.flags, seen.row.swipe), (SWIPING, sign * 9.0), "rtl {rtl}");
        let seen = s.step(1.048, &[touch(Moved, (x(30.0), 26.0), 1.048)]);
        assert_eq!(seen.row.swipe, sign * 30.0);
        // past the actions it stretches by the rubber band
        let seen = s.step(1.064, &[touch(Moved, (x(120.0), 26.0), 1.064)]);
        let stretch = (1.0 - 1.0 / (40.0 * 0.55 / 80.0 + 1.0)) * 80.0;
        assert!((seen.row.swipe.abs() - (80.0 + stretch as f32)).abs() < 0.01);
        let seen = s.step(1.08, &[touch(Moved, (x(30.0), 26.0), 1.08)]);
        assert_eq!(seen.row.swipe, sign * 30.0);
        assert_eq!(seen.offset, 0.0, "the scroll view did not move");
        // a slow release before half way snaps closed, frame by frame
        let seen = s.step(1.5, &[touch(Ended, (x(30.0), 26.0), 1.5)]);
        assert_eq!(seen.row.flags, 0);
        let mut last = seen.row.swipe.abs();
        for frame in 1..=40 {
            let seen = s.step(1.5 + f64::from(frame) / 60.0, &[]);
            assert!(seen.row.swipe.abs() <= last + 1e-4, "monotonic close");
            last = seen.row.swipe.abs();
        }
        assert_eq!(last, 0.0);
        assert!(s.ui.repaint_at().is_none(), "a settled swipe idles");
        // a release past half way opens and rests at the action width
        s.step(3.0, &[touch(Began, (x(0.0), 25.0), 3.0)]);
        s.step(3.016, &[touch(Moved, (x(10.0), 25.0), 3.016)]);
        s.step(3.032, &[touch(Moved, (x(50.0), 25.0), 3.032)]);
        s.step(3.5, &[touch(Ended, (x(50.0), 25.0), 3.5)]);
        let seen = s.step(3.6, &[]);
        assert_eq!(seen.row.flags, SWIPE_OPEN | SWIPE_OPEN_TRAILING);
        let seen = s.step(4.5, &[]);
        assert_eq!((seen.row.flags, seen.row.swipe), (SWIPE_OPEN | SWIPE_OPEN_TRAILING, sign * 80.0));
        assert!(s.ui.repaint_at().is_none());
        // a flick back closes an open row even from the open side of half way
        s.step(5.0, &[touch(Began, (x(0.0), 25.0), 5.0)]);
        s.step(5.016, &[touch(Moved, (x(-2.0), 25.0), 5.016)]);
        s.step(5.032, &[touch(Moved, (x(-14.0), 25.0), 5.032)]);
        let seen = s.step(5.048, &[touch(Ended, (x(-14.0), 25.0), 5.048)]);
        assert!(seen.row.swipe.abs() > 65.9 && seen.row.flags & SWIPING == 0);
        s.step(5.049, &[]);
        assert_eq!(s.step(6.0, &[]).row.swipe, 0.0);
        // a flick toward the actions opens a row dragged only 20 pt
        s.step(7.0, &[touch(Began, (x(0.0), 25.0), 7.0)]);
        s.step(7.016, &[touch(Moved, (x(10.0), 25.0), 7.016)]);
        s.step(7.032, &[touch(Moved, (x(20.0), 25.0), 7.032)]);
        s.step(7.04, &[touch(Ended, (x(20.0), 25.0), 7.04)]);
        assert_eq!(s.step(8.0, &[]).row.flags, SWIPE_OPEN | SWIPE_OPEN_TRAILING);
        // the app closes it
        s.commands[1] = CLOSE_SWIPE;
        assert_eq!(s.step(8.1, &[]).row.flags & SWIPE_OPEN, 0);
        assert_eq!(s.step(9.0, &[]).row.swipe, 0.0);
        // away from the actions with the row closed is not a swipe; vertical goes to the scroll view
        s.step(10.0, &[touch(Began, (x(0.0), 25.0), 10.0)]);
        let seen = s.step(10.016, &[touch(Moved, (x(-12.0), 25.0), 10.016)]);
        assert_eq!((seen.row.flags, seen.row.swipe), (0, 0.0));
        s.step(10.02, &[touch(Ended, (x(-12.0), 25.0), 10.02)]);
        s.step(11.0, &[touch(Began, (x(0.0), 100.0), 11.0)]);
        s.step(11.016, &[touch(Moved, (x(2.0), 60.0), 11.016)]);
        let seen = s.step(11.032, &[touch(Moved, (x(2.0), 20.0), 11.032)]);
        assert!(seen.offset > 0.0 && seen.row.flags & SWIPING == 0 && seen.row.swipe == 0.0);
        s.ui.destroy();
    }
}

fn edge_back_progress_commit_cancel_and_direction() {
    for rtl in [false, true] {
        let dir = if rtl { -1.0 } else { 1.0 };
        let mut s = Scene::new();
        if rtl {
            s.ui.set_dir(Direction::Rtl);
        }
        s.nav = GestureCfg { kinds: EDGE_BACK, ..Default::default() };
        s.step(0.0, &[]);
        let x = |d: f64| if rtl { 200.0 - 5.0 - d } else { 5.0 + d };
        // inside the 20 pt band: progress follows the finger over the 200 pt screen
        s.step(1.0, &[touch(Began, (x(0.0), 100.0), 1.0)]);
        assert_eq!(s.step(1.016, &[touch(Moved, (x(6.0), 100.0), 1.016)]).nav.flags, 0, "undecided before 8 pt");
        let seen = s.step(1.032, &[touch(Moved, (x(40.0), 100.0), 1.032)]);
        assert_eq!((seen.nav.flags, seen.nav.back), (BACK_ACTIVE, 0.2), "rtl {rtl} {dir}");
        assert_eq!(s.step(1.048, &[touch(Moved, (x(100.0), 100.0), 1.048)]).nav.back, 0.5);
        assert_eq!(s.step(1.064, &[touch(Moved, (x(400.0), 100.0), 1.064)]).nav.back, 1.0);
        assert_eq!(s.step(1.08, &[touch(Moved, (x(60.0), 100.0), 1.08)]).nav.back, 0.3);
        // a slow release short of half way cancels: progress settles to 0, then one pulse
        s.step(1.6, &[touch(Ended, (x(60.0), 100.0), 1.6)]);
        let mut pulses = 0;
        let mut last = 1.0;
        for frame in 1..=60 {
            let seen = s.step(1.6 + f64::from(frame) / 60.0, &[]);
            if seen.nav.flags & BACK_CANCEL != 0 {
                pulses += 1;
                assert_eq!((seen.nav.back, seen.nav.flags & BACK_ACTIVE), (0.0, 0));
            } else if seen.nav.flags & BACK_ACTIVE != 0 {
                assert!(seen.nav.back <= last && seen.nav.flags & BACK_COMMIT == 0);
                last = seen.nav.back;
            }
        }
        assert_eq!(pulses, 1);
        assert!(s.ui.repaint_at().is_none());
        // past half way commits
        s.step(4.0, &[touch(Began, (x(0.0), 100.0), 4.0)]);
        s.step(4.016, &[touch(Moved, (x(20.0), 100.0), 4.016)]);
        s.step(4.5, &[touch(Moved, (x(130.0), 100.0), 4.5)]);
        s.step(5.0, &[touch(Ended, (x(130.0), 100.0), 5.0)]);
        let mut commit = None;
        for frame in 1..=60 {
            let seen = s.step(5.0 + f64::from(frame) / 60.0, &[]);
            if seen.nav.flags & BACK_COMMIT != 0 {
                commit = Some(frame);
                assert_eq!(seen.nav.back, 0.0, "the pulse frame already shows the popped screen");
            }
        }
        assert!(commit.is_some_and(|f| (10..=30).contains(&f)), "{commit:?}");
        // a fast flick commits from 20 pt; a fast flick back cancels from 150
        s.step(7.0, &[touch(Began, (x(0.0), 100.0), 7.0)]);
        s.step(7.016, &[touch(Moved, (x(10.0), 100.0), 7.016)]);
        s.step(7.032, &[touch(Moved, (x(30.0), 100.0), 7.032)]);
        s.step(7.04, &[touch(Ended, (x(30.0), 100.0), 7.04)]);
        let committed = (1..=60).any(|f| s.step(7.04 + f64::from(f) / 60.0, &[]).nav.flags & BACK_COMMIT != 0);
        assert!(committed, "a 1.2 kpt/s flick commits");
        s.step(9.0, &[touch(Began, (x(0.0), 100.0), 9.0)]);
        s.step(9.016, &[touch(Moved, (x(20.0), 100.0), 9.016)]);
        s.step(9.5, &[touch(Moved, (x(150.0), 100.0), 9.5)]);
        s.step(9.516, &[touch(Moved, (x(130.0), 100.0), 9.516)]);
        s.step(9.524, &[touch(Ended, (x(130.0), 100.0), 9.524)]);
        let cancelled = (1..=60).any(|f| s.step(9.524 + f64::from(f) / 60.0, &[]).nav.flags & BACK_CANCEL != 0);
        assert!(cancelled, "a flick back cancels");
        // outside the band, outward or vertical starts are not a back; the cancel path returns
        for (start, to) in [((x(40.0), 100.0), (x(80.0), 100.0)), ((x(0.0), 100.0), (x(-30.0), 100.0)), ((x(0.0), 120.0), (x(4.0), 60.0))] {
            s.step(12.0, &[touch(Began, start, 12.0)]);
            let seen = s.step(12.016, &[touch(Moved, to, 12.016)]);
            assert_eq!(seen.nav.flags & BACK_ACTIVE, 0, "{start:?} {to:?}");
            s.step(12.5, &[touch(Cancelled, to, 12.5)]);
        }
        s.step(13.0, &[touch(Began, (x(0.0), 100.0), 13.0)]);
        s.step(13.016, &[touch(Moved, (x(50.0), 100.0), 13.016)]);
        s.step(13.02, &[touch(Cancelled, (x(50.0), 100.0), 13.02)]);
        let cancelled = (1..=60).any(|f| s.step(13.02 + f64::from(f) / 60.0, &[]).nav.flags & BACK_CANCEL != 0);
        assert!(cancelled, "a cancelled touch returns, never commits");
        s.ui.destroy();
    }
}

fn pull_to_refresh_threshold_hold_and_end() {
    for rows in [10, 2] {
        let mut s = Scene::new();
        s.rows = rows;
        s.list = GestureCfg { kinds: REFRESH, threshold: 60.0, ..Default::default() };
        s.step(0.0, &[]);
        // short pull: springs back, never refreshes
        s.step(1.0, &[touch(Began, (100.0, 100.0), 1.0)]);
        s.step(1.016, &[touch(Moved, (100.0, 112.0), 1.016)]);
        let seen = s.step(1.032, &[touch(Moved, (100.0, 160.0), 1.032)]);
        let shown = |raw: f64| 0.55 * raw / (1.0 + 0.55 * raw / 200.0);
        assert!((f64::from(seen.list.pull) - shown(60.0)).abs() < 0.01, "pull {}", seen.list.pull);
        assert!(seen.list.pull < 60.0 && seen.list.flags == 0);
        s.step(1.04, &[touch(Ended, (100.0, 160.0), 1.04)]);
        let mut fired = false;
        for frame in 1..=60 {
            let seen = s.step(1.04 + f64::from(frame) / 60.0, &[]);
            fired |= seen.list.flags & (REFRESH_FIRED | REFRESHING) != 0;
        }
        assert!(!fired && s.ui.repaint_at().is_none());
        // a pull past the threshold fires once on release and holds at the threshold
        s.step(3.0, &[touch(Began, (100.0, 20.0), 3.0)]);
        s.step(3.016, &[touch(Moved, (100.0, 32.0), 3.016)]);
        let seen = s.step(3.032, &[touch(Moved, (100.0, 180.0), 3.032)]);
        assert!(seen.list.pull > 60.0 && seen.list.flags & REFRESHING == 0, "pulling, not yet released");
        s.step(3.5, &[touch(Ended, (100.0, 180.0), 3.5)]);
        let mut fires = 0;
        let mut previous = f32::INFINITY;
        for frame in 1..=90 {
            let seen = s.step(3.5 + f64::from(frame) / 60.0, &[]);
            fires += usize::from(seen.list.flags & REFRESH_FIRED != 0);
            assert!(seen.list.flags & REFRESHING != 0 || frame == 0);
            assert!(seen.list.pull <= previous + 1e-3 && seen.list.pull >= 60.0 - 1e-3, "monotonic settle onto the hold");
            previous = seen.list.pull;
        }
        assert_eq!((fires, previous), (1, 60.0), "rows {rows}");
        assert!(s.ui.repaint_at().is_none(), "held refresh idles");
        // the app ends it: the content returns
        s.commands[2] = END_REFRESH;
        let seen = s.step(10.0, &[]);
        assert_eq!(seen.list.flags & REFRESHING, 0);
        let seen = (1..=60).map(|f| s.step(10.0 + f64::from(f) / 60.0, &[])).last().unwrap();
        assert_eq!((seen.list.pull, seen.position), (0.0, 0.0));
        s.ui.destroy();
    }
}

fn scroll_physics_trace_rubber_band_spring_back_and_catch() {
    let mut s = Scene::new();
    s.step(0.0, &[]);
    // UIScrollView's rubber band: x past the edge shows as (1 - 1/(x * 0.55 / d + 1)) * d, d = 200
    for (i, pull) in [20.0f64, 60.0, 150.0, 400.0].into_iter().enumerate() {
        let t = 1.0 + i as f64 * 10.0;
        s.step(t, &[touch(Began, (100.0, 20.0), t)]);
        s.step(t + 0.016, &[touch(Moved, (100.0, 32.0), t + 0.016)]);
        let seen = s.step(t + 0.032, &[touch(Moved, (100.0, 32.0 + pull), t + 0.032)]);
        let expected = (1.0 - 1.0 / ((pull + 12.0) * 0.55 / 200.0 + 1.0)) * 200.0;
        assert!((f64::from(-seen.position) - expected).abs() < 0.01, "{pull}: {} vs {expected}", seen.position);
        s.step(t + 0.04, &[touch(Cancelled, (100.0, 32.0 + pull), t + 0.04)]);
        // release: the stretch decays as exp(-t/0.084) on the frame clock, whatever the rate
        let start = f64::from(-s.step(t + 0.05, &[]).position);
        for frame in 1..=12 {
            let dt = f64::from(frame) * 0.016;
            let seen = s.step(t + 0.05 + dt, &[]);
            let want = start * (-dt / 0.084).exp();
            assert!((f64::from(-seen.position) - want).abs() < 0.3 + 0.002 * start, "frame {frame}: {} vs {want}", seen.position);
        }
        s.step(t + 5.0, &[]);
    }
    // a flick: v(t) = v0 * 0.998^(1000 t), position integrated, frame by frame at 120 Hz
    s.step(100.0, &[touch(Began, (100.0, 150.0), 100.0)]);
    s.step(100.016, &[touch(Moved, (100.0, 140.0), 100.016)]);
    s.step(100.032, &[touch(Moved, (100.0, 130.0), 100.032)]);
    s.step(100.04, &[touch(Ended, (100.0, 130.0), 100.04)]);
    let tau = -1.0 / (1000.0 * 0.998f64.ln());
    for frame in 1..=240 {
        let dt = f64::from(frame) / 120.0;
        let seen = s.step(100.04 + dt, &[]);
        let want = (20.0 + 625.0 * tau * (1.0 - (-dt / tau).exp())).min(300.0);
        assert!((f64::from(seen.offset) - want).abs() <= 0.5, "frame {frame}: {} vs {want}", seen.offset);
        assert!(!seen.clicked);
    }
    s.ui.destroy();
}

fn row_swipe_both_sides_open_full_swipe_commit_close_and_mirror() {
    for rtl in [false, true] {
        let dir = if rtl { -1.0f64 } else { 1.0 };
        let shift = |travel: f64| (-dir * travel) as f32; // physical shift for finger travel toward the trailing side
        let mut s = Scene::new();
        if rtl {
            s.ui.set_dir(Direction::Rtl);
        }
        s.row = GestureCfg { kinds: ROW_SWIPE, reveal: 80.0, leading: 60.0, full: 120.0, ..Default::default() };
        s.step(0.0, &[]);
        let x = |d: f64| 150.0 - dir * d;
        let open = |s: &mut Scene, t: f64, travel: f64, end_at: f64| {
            s.step(t, &[touch(Began, (x(0.0), 25.0), t)]);
            s.step(t + 0.016, &[touch(Moved, (x(travel.signum() * 10.0), 25.0), t + 0.016)]);
            s.step(t + 0.032, &[touch(Moved, (x(travel), 25.0), t + 0.032)]);
            s.step(end_at, &[touch(Ended, (x(travel), 25.0), end_at)])
        };
        // leading: axis lock, follows the finger, rubber band past its own 60 pt width
        s.step(1.0, &[touch(Began, (x(0.0), 25.0), 1.0)]);
        let seen = s.step(1.016, &[touch(Moved, (x(-7.0), 26.0), 1.016)]);
        assert_eq!((seen.row.flags, seen.row.swipe), (0, 0.0));
        let seen = s.step(1.032, &[touch(Moved, (x(-9.0), 26.0), 1.032)]);
        assert_eq!((seen.row.flags, seen.row.swipe), (SWIPING, shift(-9.0)), "rtl {rtl}");
        let seen = s.step(1.048, &[touch(Moved, (x(-100.0), 26.0), 1.048)]);
        let stretch = (1.0 - 1.0 / (40.0 * 0.55 / 60.0 + 1.0)) * 60.0;
        assert!((seen.row.swipe.abs() - (60.0 + stretch as f32)).abs() < 0.01 && seen.row.swipe * shift(-1.0) > 0.0);
        s.step(1.064, &[touch(Moved, (x(-40.0), 26.0), 1.064)]);
        // released past half way (and under the full distance) it rests open on the leading side
        let seen = s.step(1.5, &[touch(Ended, (x(-40.0), 26.0), 1.5)]);
        assert_eq!(seen.row.flags & (SWIPE_COMMIT_LEADING | SWIPE_COMMIT_TRAILING), 0);
        s.step(1.6, &[]);
        let seen = s.step(2.5, &[]);
        assert_eq!((seen.row.flags, seen.row.swipe), (SWIPE_OPEN | SWIPE_OPEN_LEADING, shift(-60.0)));
        assert!(s.ui.repaint_at().is_none());
        s.commands[1] = CLOSE_SWIPE;
        s.step(2.6, &[]);
        let seen = s.step(3.5, &[]);
        assert_eq!((seen.row.flags, seen.row.swipe), (0, 0.0), "close");
        // trailing keeps its own width
        let seen = open(&mut s, 4.0, 50.0, 4.5);
        assert_eq!(seen.row.flags & SWIPE_COMMIT_TRAILING, 0);
        s.step(4.6, &[]);
        let seen = s.step(5.5, &[]);
        assert_eq!((seen.row.flags, seen.row.swipe), (SWIPE_OPEN | SWIPE_OPEN_TRAILING, shift(80.0)));
        s.commands[1] = CLOSE_SWIPE;
        s.step(5.6, &[]);
        assert_eq!(s.step(6.5, &[]).row.swipe, 0.0);
        // a full swipe commits that side for one frame and the row settles closed
        for (i, (travel, bit)) in [(125.0, SWIPE_COMMIT_TRAILING), (-125.0, SWIPE_COMMIT_LEADING)].into_iter().enumerate() {
            let t = 10.0 + 10.0 * i as f64;
            let seen = open(&mut s, t, travel, t + 0.5);
            assert_eq!(seen.row.flags & !SWIPING, bit, "commit pulse on the release frame (rtl {rtl}, {travel})");
            let seen = s.step(t + 0.6, &[]);
            assert_eq!(seen.row.flags & (SWIPE_OPEN | SWIPE_COMMIT_LEADING | SWIPE_COMMIT_TRAILING), 0, "the pulse lasts one frame");
            assert_eq!(s.step(t + 2.0, &[]).row.swipe, 0.0);
            assert!(s.ui.repaint_at().is_none());
        }
        // just under the distance opens instead of committing
        let seen = open(&mut s, 30.0, 110.0, 30.5);
        assert_eq!(seen.row.flags & (SWIPE_COMMIT_LEADING | SWIPE_COMMIT_TRAILING), 0);
        assert_eq!(s.step(32.0, &[]).row.flags, SWIPE_OPEN | SWIPE_OPEN_TRAILING);
        // dragging an open trailing row across closed into the leading side
        s.step(33.0, &[touch(Began, (x(0.0), 25.0), 33.0)]);
        s.step(33.016, &[touch(Moved, (x(-10.0), 25.0), 33.016)]);
        let seen = s.step(33.032, &[touch(Moved, (x(-100.0), 25.0), 33.032)]);
        assert!(seen.row.swipe * shift(-1.0) > 0.0, "leading side revealed");
        // a cancelled touch never commits
        s.step(33.1, &[touch(Cancelled, (x(-100.0), 25.0), 33.1)]);
        let seen = s.step(33.2, &[]);
        assert_eq!(seen.row.flags & (SWIPE_COMMIT_LEADING | SWIPE_COMMIT_TRAILING), 0);
        // a side with no actions is not a swipe
        s.row = GestureCfg { kinds: ROW_SWIPE, reveal: 80.0, ..Default::default() };
        s.commands[1] = CLOSE_SWIPE;
        s.step(34.0, &[]);
        s.step(36.0, &[]);
        s.step(37.0, &[touch(Began, (x(0.0), 25.0), 37.0)]);
        let seen = s.step(37.016, &[touch(Moved, (x(-12.0), 25.0), 37.016)]);
        assert_eq!((seen.row.flags, seen.row.swipe), (0, 0.0));
        s.ui.destroy();
    }
}

// a vertical drag on the 200x200 nav node (an inner 10-row list scrolls inside it) from `y0` to `y1`, lifting after `lift` seconds
fn drag(s: &mut Scene, at: f64, x: f64, y0: f64, y1: f64, lift: f64) -> (Seen, Seen) {
    s.step(at, &[touch(Began, (x, y0), at)]);
    s.step(at + 0.016, &[touch(Moved, (x, y0 + (y1 - y0).signum() * 12.0), at + 0.016)]);
    let seen = s.step(at + 0.032, &[touch(Moved, (x, y1), at + 0.032)]);
    (seen, s.step(at + lift, &[touch(Ended, (x, y1), at + lift)]))
}
fn settle(s: &mut Scene, at: f64) -> Seen {
    (1..=60).map(|f| s.step(at + f64::from(f) / 60.0, &[])).last().unwrap()
}

fn dismiss_scene() -> Scene {
    let mut s = Scene::new();
    s.dismiss = Some((0.0, 0.0));
    s.step(0.0, &[]);
    s
}

fn dismiss_commits_by_distance_or_velocity_and_cancels_otherwise() {
    let mut s = dismiss_scene();
    // the list is at its top, so it hands the downward drag over; the offset follows the finger less the 12 pt lock slop
    let (seen, lifted) = drag(&mut s, 1.0, 100.0, 20.0, 100.0, 1.0);
    assert!((seen.dismiss.offset - 68.0).abs() < 1e-3 && seen.dismiss.flags == DISMISSING, "{:?}", seen.dismiss);
    assert_eq!(seen.position, 0.0, "the list did not scroll");
    // 68 of 200 is past a quarter: commit pulse once, then it settles to the height
    assert_eq!(lifted.dismiss.flags & DISMISS_COMMIT, DISMISS_COMMIT, "{:?}", lifted.dismiss);
    assert_eq!(s.step(2.02, &[]).dismiss.flags & (DISMISS_COMMIT | DISMISS_CANCEL), 0, "one frame");
    let seen = settle(&mut s, 2.02);
    assert_eq!((seen.dismiss.offset, seen.dismiss.flags), (200.0, 0));
    assert!(s.ui.repaint_at().is_none());
    // the app closes after the commit (the node goes away); a fresh node starts at rest
    s.ui.destroy();
    s = dismiss_scene();
    // a short slow drag springs back
    let at = 5.0;
    let lifted = drag(&mut s, at, 100.0, 20.0, 50.0, 1.0).1;
    assert_eq!(lifted.dismiss.flags & DISMISS_CANCEL, DISMISS_CANCEL);
    assert_eq!((settle(&mut s, at + 1.0).dismiss.offset, s.ui.repaint_at()), (0.0, None));
    // a short drag released at speed commits: 30 pt in 16 ms is far past 600
    let at = 10.0;
    s.step(at, &[touch(Began, (100.0, 20.0), at)]);
    s.step(at + 0.016, &[touch(Moved, (100.0, 32.0), at + 0.016)]);
    s.step(at + 0.032, &[touch(Moved, (100.0, 40.0), at + 0.032)]);
    s.step(at + 0.048, &[touch(Moved, (100.0, 60.0), at + 0.048)]);
    let seen = s.step(at + 0.05, &[touch(Ended, (100.0, 62.0), at + 0.05)]);
    assert!(seen.dismiss.offset < 50.0);
    assert_eq!(seen.dismiss.flags & DISMISS_COMMIT, DISMISS_COMMIT);
    assert_eq!(settle(&mut s, at + 0.06).dismiss.offset, 200.0);
    // a configured fraction: 0.5 needs 100 points
    s.ui.destroy();
    s = dismiss_scene();
    s.dismiss = Some((0.5, 0.0));
    let at = 20.0;
    let lifted = drag(&mut s, at, 100.0, 20.0, 90.0, 1.0).1;
    assert_eq!(lifted.dismiss.flags & DISMISS_CANCEL, DISMISS_CANCEL);
    settle(&mut s, at + 1.0);
    // upward past 0 rubber-bands at 0.55; a drag back down and up again stays continuous
    s.dismiss = Some((0.0, 0.0));
    let at = 30.0;
    s.step(at, &[touch(Began, (100.0, 100.0), at)]);
    s.step(at + 0.016, &[touch(Moved, (100.0, 112.0), at + 0.016)]);
    s.step(at + 0.032, &[touch(Moved, (100.0, 150.0), at + 0.032)]);
    let seen = s.step(at + 0.048, &[touch(Moved, (100.0, 52.0), at + 0.048)]);
    let raw = -60.0_f64;
    let want = -(0.55 * -raw / (1.0 + 0.55 * -raw / 200.0));
    assert!((f64::from(seen.dismiss.offset) - want).abs() < 0.01, "{} vs {want}", seen.dismiss.offset);
    // a cancel never commits
    let seen = s.step(at + 0.06, &[touch(Cancelled, (100.0, 52.0), at + 0.06)]);
    assert_eq!(seen.dismiss.flags & (DISMISS_COMMIT | DISMISS_CANCEL), DISMISS_CANCEL);
    assert_eq!(settle(&mut s, at + 0.06).dismiss.offset, 0.0);
    s.ui.destroy();
}

fn dismiss_hands_over_to_an_inner_scroll_only_at_its_top_and_ignores_horizontal() {
    let mut s = Scene::new();
    s.dismiss = Some((0.0, 0.0));
    s.step(0.0, &[]);
    // scroll the list down by dragging up: the drag scrolls and the sheet does not move
    let seen = drag(&mut s, 1.0, 100.0, 150.0, 40.0, 0.04).0;
    assert_eq!(seen.dismiss.offset, 0.0);
    settle(&mut s, 1.04);
    let offset = s.step(3.0, &[]).offset;
    assert!(offset > 50.0, "list scrolled to {offset}");
    // not at top: a downward drag scrolls the list back instead of moving the sheet
    let (seen, lifted) = drag(&mut s, 4.0, 100.0, 20.0, 50.0, 1.0);
    assert_eq!(seen.dismiss, DismissState { flags: 0, offset: 0.0 });
    assert!(seen.position < offset);
    assert_eq!(lifted.dismiss.flags & (DISMISS_COMMIT | DISMISS_CANCEL), 0);
    // scroll all the way to the top, then the same drag moves the sheet
    let list = s.list_box.unwrap();
    s.ui.set_scroll_offset(&list, 0.0);
    s.step(7.0, &[]);
    let seen = drag(&mut s, 8.0, 100.0, 20.0, 80.0, 1.0).0;
    assert!(seen.dismiss.offset > 40.0 && seen.position == 0.0, "{:?} {}", seen.dismiss, seen.position);
    settle(&mut s, 9.0);
    // a horizontal drag moves nothing
    s.step(13.0, &[touch(Began, (100.0, 100.0), 13.0)]);
    s.step(13.016, &[touch(Moved, (112.0, 102.0), 13.016)]);
    let seen = s.step(13.032, &[touch(Moved, (180.0, 104.0), 13.032)]);
    assert_eq!(seen.dismiss, DismissState::default());
    s.step(13.04, &[touch(Ended, (180.0, 104.0), 13.04)]);
    s.ui.destroy();
}

fn gestures_allocate_nothing_when_warm() {
    let mut s = Scene::new();
    s.row = GestureCfg { kinds: LONG_PRESS | ROW_SWIPE, reveal: 80.0, leading: 60.0, full: 120.0, ..Default::default() };
    s.nav = GestureCfg { kinds: EDGE_BACK, ..Default::default() };
    s.list = GestureCfg { kinds: REFRESH, threshold: 60.0, ..Default::default() };
    s.dismiss = Some((0.0, 0.0));
    for round in 0..4 {
        let t = f64::from(round) * 30.0;
        mem_sys::local_reset();
        let mut at = t;
        let frames = |s: &mut Scene, from: f64, to: f64, events: &[Event]| {
            s.step(from, events);
            let mut now = from;
            while now + 1.0 / 60.0 < to {
                now += 1.0 / 60.0;
                s.step(now, &[]);
            }
            now
        };
        // long press, swipe open and close, back commit and cancel, refresh and end
        at = frames(&mut s, at + 1.0, at + 2.0, &[touch(Began, (100.0, 25.0), at + 1.0)]);
        at = frames(&mut s, at + 0.1, at + 0.2, &[touch(Ended, (100.0, 25.0), at + 0.1)]);
        let begin = at + 1.0;
        s.step(begin, &[touch(Began, (150.0, 25.0), begin)]);
        s.step(begin + 0.016, &[touch(Moved, (140.0, 25.0), begin + 0.016)]);
        s.step(begin + 0.032, &[touch(Moved, (80.0, 25.0), begin + 0.032)]);
        at = frames(&mut s, begin + 0.04, begin + 1.0, &[touch(Ended, (80.0, 25.0), begin + 0.04)]);
        s.commands[1] = CLOSE_SWIPE;
        at = frames(&mut s, at + 0.1, at + 1.0, &[]);
        // full swipes commit on each side
        for end in [20.0, 280.0] {
            let begin = at + 1.0;
            s.step(begin, &[touch(Began, (150.0, 25.0), begin)]);
            s.step(begin + 0.016, &[touch(Moved, (150.0 + (end - 150.0) * 0.1, 25.0), begin + 0.016)]);
            s.step(begin + 0.032, &[touch(Moved, (end, 25.0), begin + 0.032)]);
            at = frames(&mut s, begin + 0.5, begin + 1.5, &[touch(Ended, (end, 25.0), begin + 0.5)]);
        }
        let begin = at + 1.0;
        s.step(begin, &[touch(Began, (5.0, 100.0), begin)]);
        s.step(begin + 0.016, &[touch(Moved, (40.0, 100.0), begin + 0.016)]);
        s.step(begin + 0.032, &[touch(Moved, (150.0, 100.0), begin + 0.032)]);
        at = frames(&mut s, begin + 0.04, begin + 1.0, &[touch(Ended, (150.0, 100.0), begin + 0.04)]);
        let begin = at + 1.0;
        s.step(begin, &[touch(Began, (100.0, 20.0), begin)]);
        s.step(begin + 0.016, &[touch(Moved, (100.0, 40.0), begin + 0.016)]);
        s.step(begin + 0.032, &[touch(Moved, (100.0, 180.0), begin + 0.032)]);
        at = frames(&mut s, begin + 0.04, begin + 2.0, &[touch(Ended, (100.0, 180.0), begin + 0.04)]);
        s.commands[2] = END_REFRESH;
        at = frames(&mut s, at + 0.1, at + 1.5, &[]);
        // dismiss: commit and spring back (the dismiss node also wraps the refresh list, so the drag starts at its top)
        for end in [90.0, 50.0] {
            let begin = at + 1.0;
            s.step(begin, &[touch(Began, (100.0, 20.0), begin)]);
            s.step(begin + 0.016, &[touch(Moved, (100.0, 32.0), begin + 0.016)]);
            s.step(begin + 0.032, &[touch(Moved, (100.0, end), begin + 0.032)]);
            at = frames(&mut s, begin + 0.5, begin + 1.5, &[touch(Ended, (100.0, end), begin + 0.5)]);
        }
        if round >= 2 {
            assert_eq!(mem_sys::local_alloc_count(), 0, "warm gesture round {round}");
        }
    }
    s.ui.destroy();
}

pub const CASES: &[(&str, fn())] = &[
    ("gesture_long_press_highlights_after_delay_fires_once_and_swallows_the_tap", long_press_highlights_after_delay_fires_once_and_swallows_the_tap),
    ("gesture_row_swipe_axis_lock_offset_snap_and_close", row_swipe_axis_lock_offset_snap_and_close),
    ("gesture_row_swipe_both_sides_open_full_swipe_commit_close_and_mirror", row_swipe_both_sides_open_full_swipe_commit_close_and_mirror),
    ("gesture_edge_back_progress_commit_cancel_and_direction", edge_back_progress_commit_cancel_and_direction),
    ("gesture_pull_to_refresh_threshold_hold_and_end", pull_to_refresh_threshold_hold_and_end),
    ("gesture_dismiss_commits_by_distance_or_velocity_and_cancels_otherwise", dismiss_commits_by_distance_or_velocity_and_cancels_otherwise),
    ("gesture_dismiss_hands_over_to_an_inner_scroll_only_at_its_top_and_ignores_horizontal", dismiss_hands_over_to_an_inner_scroll_only_at_its_top_and_ignores_horizontal),
    ("gestures_allocate_nothing_when_warm", gestures_allocate_nothing_when_warm),
    ("gesture_touch_drag_node_takes_a_held_press_and_drag_without_scrolling", touch_drag_node_takes_a_held_press_and_drag_without_scrolling),
    ("gesture_scroll_physics_trace_rubber_band_spring_back_and_catch", scroll_physics_trace_rubber_band_spring_back_and_catch),
];

/// A scroll view with a `TOUCH_DRAG` strip over its right edge: returns the strip's signal and the list's scroll offset.
fn strip_step(ui: &mut Ui, frame: &mut input::frame::Frame, time: f64, events: &[Event], claim: bool) -> (Signal, f32) {
    frame.input.t = time;
    frame.input.events.clear();
    frame.input.events.extend_from_slice(events);
    let mut out = None;
    ui.frame(frame, &mut |ui| {
        let list = ui.box_("strip-list", BoxOpts::default().at((0.0, 0.0)).w(Size::Px(100.0)).h(Size::Px(200.0)).flags(CLIP | SCROLL));
        let inner = ui.enter(&list);
        for name in ROWS {
            let row = ui.box_(name, BoxOpts::default().w(Size::Px(100.0)).h(Size::Px(50.0)).flags(CLICKABLE));
            // rows under the strip take swipes: without the claim they would hold the touch pending
            ui.gesture(&row, Some(GestureCfg { kinds: ROW_SWIPE, reveal: 60.0, ..Default::default() }), 0);
        }
        ui.leave(inner);
        let strip = ui.box_("strip", BoxOpts::default().at((80.0, 0.0)).w(Size::Px(20.0)).h(Size::Px(200.0)).flags(CLICKABLE));
        ui.gesture(&strip, Some(GestureCfg { kinds: if claim { TOUCH_DRAG } else { 0 }, ..Default::default() }), 0);
        out = Some((ui.signal_state(&strip), list));
    });
    let (signal, list) = out.unwrap();
    (signal, ui.scroll_offset(&list))
}

fn touch_drag_node_takes_a_held_press_and_drag_without_scrolling() {
    let mut frame = frame_at(vec![], 2.0, 0.0, Vec::with_capacity(32));
    let mut ui = new_ui(None);
    strip_step(&mut ui, &mut frame, 0.0, &[], true);
    let (s, _) = strip_step(&mut ui, &mut frame, 0.1, &[touch(Began, (90.0, 20.0), 0.1)], true);
    assert!(s.pressed && s.held && s.pointer == (90.0, 20.0), "press on touch-down, no slop wait");
    // frame by frame down the strip, then past its edge: capture keeps tracking, the list never scrolls
    for (i, p) in [(92.0, 50.0), (95.0, 120.0), (200.0, 190.0)].into_iter().enumerate() {
        let t = 0.116 + 0.016 * i as f64;
        let (s, off) = strip_step(&mut ui, &mut frame, t, &[touch(Moved, p, t)], true);
        assert!(s.held && s.dragging && !s.pressed && s.pointer == (p.0 as f32, p.1 as f32), "frame {i}");
        assert_eq!(off, 0.0, "the vertical drag does not scroll the list");
    }
    let (s, off) = strip_step(&mut ui, &mut frame, 0.2, &[touch(Ended, (200.0, 190.0), 0.2)], true);
    assert!(s.released && !s.held && off == 0.0);
    let (_, off) = strip_step(&mut ui, &mut frame, 0.4, &[], true);
    assert_eq!(off, 0.0, "no fling after the lift");
    // a tap clicks like a mouse
    strip_step(&mut ui, &mut frame, 1.0, &[touch(Began, (90.0, 20.0), 1.0)], true);
    let (s, _) = strip_step(&mut ui, &mut frame, 1.03, &[touch(Ended, (90.0, 20.0), 1.03)], true);
    assert!(s.clicked);
    // without the kind the touch waits on the row swipe under the strip and is never held
    strip_step(&mut ui, &mut frame, 1.9, &[], false);
    let (s, _) = strip_step(&mut ui, &mut frame, 2.0, &[touch(Began, (90.0, 150.0), 2.0)], false);
    let (s2, _) = strip_step(&mut ui, &mut frame, 2.016, &[touch(Moved, (90.0, 145.0), 2.016)], false);
    assert!(!s.held && !s2.held, "unclaimed touch is arbitrated {} {} {}", s.held, s2.held, s.pressed);
    strip_step(&mut ui, &mut frame, 2.05, &[touch(Ended, (90.0, 145.0), 2.05)], false);
    // mouse behaviour is the same press/drag path
    strip_step(&mut ui, &mut frame, 5.0, &[Event::PointerMoved((90.0, 20.0)), Event::PointerPressed(input::MouseButton::LeftButton, (90.0, 20.0))], true);
    let (s, _) = strip_step(&mut ui, &mut frame, 5.016, &[Event::PointerMoved((90.0, 120.0))], true);
    assert!(s.held && s.dragging && s.pointer == (90.0, 120.0));
    // warm allocations
    for round in 0..4 {
        let t = 10.0 + f64::from(round) * 5.0;
        mem_sys::local_reset();
        strip_step(&mut ui, &mut frame, t, &[touch(Began, (90.0, 20.0), t)], true);
        strip_step(&mut ui, &mut frame, t + 0.016, &[touch(Moved, (90.0, 90.0), t + 0.016)], true);
        strip_step(&mut ui, &mut frame, t + 0.032, &[touch(Ended, (90.0, 90.0), t + 0.032)], true);
        if round >= 2 {
            assert_eq!(mem_sys::local_alloc_count(), 0, "warm touch drag round {round}");
        }
    }
    ui.destroy();
}
