//! A settled frame allocates nothing: a small widget panel built through the wrapper, with a signal read per interactive box, a
//! box with a hit callback, a canvas that fills more than one span and every output accessor,
//! between two counted points. The counting allocator is `mem_sys`, installed for the test binary by the `use`: linked statically
//! (`--features static`) it counts the library's allocations too; linked dynamically it counts the wrapper's own.
use crate::{Node, Ui};
use pxui_types::{geom::{Rect, Rgba}, ui::{BoxOpts, HitFn, CLIP, SCROLL}};
use std::{rc::Rc, sync::atomic::{AtomicUsize, Ordering}};

/// How many times the canvas callback ran and how many shapes it stored (a span holds 256: it refills)
static CANVAS_RUNS: AtomicUsize = AtomicUsize::new(0);
static CANVAS_SHAPES: AtomicUsize = AtomicUsize::new(0);
const SHAPES: usize = 300;
use mem_sys as _;
use pxui_types::{Axis, FrameIn, Size, layout::{Cell, Grid, Track}};

struct State { clicks: u32, night: bool, check: bool, volume: f32, mode: usize, seen: u64, hit: HitFn }

fn panel(ui: &mut Ui, s: &mut State) {
    ui.panel(pxui_types::ui::PanelOpts::default(), "settings", &mut |ui: &mut Ui| {
        ui.label("Display");
        if ui.button("Apply") { s.clicks += 1; }
        s.night = ui.toggle("Night light", s.night);
        s.check = ui.checkbox("Check", s.check);
        s.volume = ui.slider("Volume", s.volume, 0.0..=1.0);
        s.mode = ui.radio("Mode", &["Auto", "Light", "Dark"], s.mode);
        let _ = ui.text_field("Name", "abc");
        let row = ui.node("row", Node { h: Size::Px(24.0), axis: Axis::Row, ..Node::DEFAULT });
        ui.within(&row, &mut |ui: &mut Ui| {
            // a keyed node of a grid, as a custom widget declares it
            const COLS: [Track; 2] = [Track::Fr(1.0), Track::Px(40.0)];
            let cell = ui.node_keyed(7, Node { grid: Some(Grid { cols: &COLS, rows: &[] }), cell: Cell::at(0, 0), ..Node::DEFAULT });
            let signal = ui.signal(&cell);
            s.clicks += signal.clicked as u32;
            ui.text(&cell, "custom");
        });
        let signal = ui.signal(&row);
        s.clicks += signal.keys.len() as u32;
        engine_shaped(ui, s);
    });
}

// The engine-shaped calls: queries, a box with its scroll, label and text state, a canvas whose shapes land in a span, a
// list, a table and a popup.
fn engine_shaped(ui: &mut Ui, s: &mut State) {
    s.seen += (ui.font_size() + ui.row_height()) as u64 + ui.frame_index() + ui.captured() as u64;
    let scroller = ui.box_fmt(format_args!("scroller"), BoxOpts::default().flags(SCROLL | CLIP).w(Size::Px(60.0)).h(Size::Px(40.0)).at((4.0, 4.0)));
    s.seen += ui.scroll_offset(&scroller) as u64 + ui.local_rect(&scroller).w as u64 + ui.cached_label(&scroller, 0, "cached").len() as u64;
    ui.set_text_state(&scroller, None);
    s.seen += ui.text_state(&scroller).is_some() as u64 + ui.hovered_exactly(&scroller) as u64 + ui.drop_target(&scroller).is_some() as u64;
    let canvas = ui.box_("canvas", BoxOpts::default().w(Size::Px(80.0)).h(Size::Px(30.0)));
    let origin = s.clicks as f32;
    ui.canvas(&canvas, move |p, _| {
        CANVAS_RUNS.fetch_add(1, Ordering::Relaxed);
        for i in 0..SHAPES { p.fill(i as f32 * 0.25, 0.0, 1.0, 1.0, Rgba { r: 5, g: 5, b: 5, a: 255 }); }
        CANVAS_SHAPES.fetch_add(SHAPES, Ordering::Relaxed);
        p.fill(origin, 0.0, 8.0, 8.0, Rgba { r: 9, g: 9, b: 9, a: 255 });
        p.line_w((0.0, 0.0), (20.0, 9.0), 1.5, Rgba { r: 1, g: 2, b: 3, a: 255 });
        p.circle((30.0, 10.0), 4.0, Some(Rgba { r: 7, g: 7, b: 7, a: 255 }), None);
        p.text((2.0, 12.0), "span");
        p.fill(40.0, 0.0, 8.0, 8.0, Rgba { r: 9, g: 9, b: 9, a: 255 });
    });
    let hit = ui.box_("hit", BoxOpts { hit: Some(Rc::clone(&s.hit)), ..BoxOpts::default().w(Size::Px(20.0)).h(Size::Px(20.0)) });
    s.seen += hit.box_key as u64 & 1;
    s.seen += ui.list_view("items", 3, false, &mut [false; 3], |row, emit| emit(["one", "two", "three"][row])).is_some() as u64;
    s.seen += ui.table_str((0.0, 0.0), 120.0, 60.0, &["Name", "Size"], 3, |_, column| if column == 0 { "row" } else { "1" }, "table").is_some() as u64;
    s.seen += ui.popup(None, None, true, false, (10.0, 10.0), 80.0, 30.0, "popup", &mut |ui: &mut Ui| ui.label("popped")) as u64;
}

#[test]
fn a_settled_frame_through_the_wrapper_allocates_nothing() {
    mem_sys::local_reset();
    let counted = mem_sys::local_alloc_count();
    drop(std::hint::black_box(Box::new(1u64)));
    assert!(mem_sys::local_alloc_count() > counted, "the counting allocator is installed");
    let mut ui = Ui::new().expect("library");
    let hit: HitFn = Rc::new(|rect: Rect| rect);
    let mut state = State {
        clicks: 0, night: false, check: false, volume: 0.5, mode: 0, seen: 0, hit,
    };
    let input = FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() };
    // `revision` is the page view's when there is one: a changing one rebuilds the whole page (every canvas callback runs and
    // refills its spans); without a view the retained frame is left alone
    let frame = |ui: &mut Ui, state: &mut State, revision: Option<u64>| {
        ui.frame(&input, &mut |ui: &mut Ui| match revision {
            Some(revision) => { ui.view("page", revision, Size::Grow, Size::Grow, &mut |ui: &mut Ui| panel(ui, state)); }
            None => panel(ui, state),
        }).expect("frame");
        // the stat accessors read the cached output: no rebuild, no allocation
        state.seen += ui.draw_revision() + ui.built_nodes() as u64 + ui.rebuilt_views() as u64 + ui.arranged_nodes() as u64 + ui.repaint_at().map_or(0, |_| 1);
        let output = ui.output();
        state.seen += output.a11y.nodes.len as u64 + output.platform.has_clipboard as u64;
    };
    for (name, forced) in [("settled", false), ("rebuilt", true)] {
        for round in 0..8 { frame(&mut ui, &mut state, forced.then_some(100 + round)); }
        let (runs, shapes) = (CANVAS_RUNS.load(Ordering::Relaxed), CANVAS_SHAPES.load(Ordering::Relaxed));
        let mut worst = 0;
        for round in 0..4 {
            let before = mem_sys::local_alloc_count();
            frame(&mut ui, &mut state, forced.then_some(200 + round));
            worst = worst.max(mem_sys::local_alloc_count() - before);
        }
        let measured = (CANVAS_RUNS.load(Ordering::Relaxed) - runs, CANVAS_SHAPES.load(Ordering::Relaxed) - shapes);
        if forced {
            assert_eq!(measured.0, 4, "the canvas ran in each measured frame");
            assert_eq!(measured.1, 4 * SHAPES, "and stored its shapes (more than a span holds, so it refilled)");
        }
        assert_eq!(worst, 0, "a {name} frame allocates nothing");
    }
    assert!(state.seen > 0);
}
