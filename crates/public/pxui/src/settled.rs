//! A settled frame allocates nothing: a small widget panel built through the wrapper, with a signal read per interactive box, a
//! canvas that fills more than one span and every output accessor,
//! between two counted points. The counting allocator is `mem_sys`, installed for the test binary by the `use`: linked statically
//! (`--features static`) it counts the library's allocations too; linked dynamically it counts the wrapper's own.
use crate::{Fill, Label, Node, Stroke, Ui};
use pxui_types::geom::{Rect, Rgba};
use std::sync::atomic::{AtomicUsize, Ordering};

/// How many times the canvas callback ran and how many shapes it stored (a span holds 256: it refills)
static CANVAS_RUNS: AtomicUsize = AtomicUsize::new(0);
static CANVAS_SHAPES: AtomicUsize = AtomicUsize::new(0);
const SHAPES: usize = 300;
use mem_sys as _;
use crate::FrameIn;
use pxui_types::{Axis, Size, layout::{Cell, Grid, Track}};
use pxui_types::Flags;

struct State {
    clicks: u32,
    night: bool,
    check: bool,
    volume: f32,
    mode: usize,
    seen: u64,
}

fn panel(ui: &mut Ui, s: &mut State) {
    ui.panel("settings", &crate::Overlay::DEFAULT, &mut |ui: &mut Ui| {
        ui.label("Display", crate::Label::DEFAULT);
        if ui.button("Apply") { s.clicks += 1; }
        s.night = ui.toggle("Night light", s.night);
        s.check = s.check != ui.checkbox("check", "Check", s.check, &crate::Checkbox::DEFAULT).changed;
        s.volume = ui.slider("volume", "Volume", s.volume, 0.0..=1.0, &crate::Slider::DEFAULT).value;
        s.mode = ui.radio("mode", &[crate::Segment::new("Auto"), crate::Segment::new("Light"), crate::Segment::new("Dark")], Some(s.mode), &crate::Radio::DEFAULT.name("Mode")).chosen.unwrap_or(s.mode);
        let _ = ui.text_field("Name", "abc", &crate::TextField::DEFAULT);
        let row = ui.node("row", Node { h: Size::Px(24.0), axis: Axis::Row, ..Node::DEFAULT });
        ui.within(&row, &mut |ui: &mut Ui| {
            // a keyed node of a grid, as a custom widget declares it
            const COLS: [Track; 2] = [Track::Fr(1.0), Track::Px(40.0)];
            let cell = ui.node(7i64, Node { grid: Some(Grid::new(&COLS, &[])).into(), cell: Cell::at(0, 0), ..Node::DEFAULT });
            let signal = ui.signal(&cell);
            s.clicks += signal.clicked as u32;
            ui.text(&cell, "custom");
        });
        s.clicks += ui.signal_keys(&row).len() as u32;
        engine_shaped(ui, s);
    });
}

// The engine-shaped calls: queries, a box with its scroll, label and text state, a canvas whose shapes land in a span, a
// list, a table and a popup.
fn engine_shaped(ui: &mut Ui, s: &mut State) {
    s.seen += (ui.font_size() + ui.row_height()) as u64 + ui.frame_index() + ui.captured() as u64;
    let scroller = ui.node("scroller", Node { at: Some((4.0, 4.0)).into(), ..Node::DEFAULT.flags(Flags::SCROLL | Flags::CLIP).w(Size::Px(60.0)).h(Size::Px(40.0)) });
    s.seen += ui.scroll(&scroller).settled as u64 + ui.local_rect(&scroller).w as u64 + ui.cached_label(&scroller, 0, "cached").len() as u64;
    ui.set_text_state(&scroller, None);
    s.seen += ui.text_state(&scroller).is_some() as u64 + ui.hovered(&scroller, false) as u64 + ui.drop_target(&scroller).is_some() as u64;
    let canvas = ui.node("canvas", Node::DEFAULT.w(Size::Px(80.0)).h(Size::Px(30.0)));
    let origin = s.clicks as f32;
    ui.canvas(&canvas, move |p, _| {
        CANVAS_RUNS.fetch_add(1, Ordering::Relaxed);
        for i in 0..SHAPES { p.fill(Rect::new(i as f32 * 0.25, 0.0, 1.0, 1.0), Fill::new(Rgba { r: 5, g: 5, b: 5, a: 255 })); }
        CANVAS_SHAPES.fetch_add(SHAPES, Ordering::Relaxed);
        p.fill(Rect::new(origin, 0.0, 8.0, 8.0), Fill::new(Rgba { r: 9, g: 9, b: 9, a: 255 }));
        p.line((0.0, 0.0), (20.0, 9.0), Stroke::new(Rgba { r: 1, g: 2, b: 3, a: 255 }).width(1.5));
        p.circle((30.0, 10.0), 4.0, Some(Fill::new(Rgba { r: 7, g: 7, b: 7, a: 255 })), None);
        p.text(crate::Point::new(2.0, 12.0), Label::BODY, 0.0, "span");
        p.fill(Rect::new(40.0, 0.0, 8.0, 8.0), Fill::new(Rgba { r: 9, g: 9, b: 9, a: 255 }));
    });
    s.seen += ui.selection_list("items", 3, &mut [0; 3], &crate::SelectionList::DEFAULT, &mut |ui, row| ui.item_text(["one", "two", "three"][row])).activated.is_some() as u64;
    s.seen += ui.table_view("table", &["Name", "Size"], &mut [], 3, &mut None, &crate::TableView::text(crate::ui::Rect::new(0.0, 0.0, 120.0, 60.0)), &mut |ui, _, column| ui.item_text(if column == 0 { "row" } else { "1" })).row.is_some() as u64;
    s.seen += ui.popup("popup", &crate::Overlay::DEFAULT.at(crate::Point::new(10.0, 10.0)).w_px(80.0).h_px(30.0).dismiss_initial(), &mut |ui: &mut Ui| { ui.label("popped", crate::Label::DEFAULT); }).open as u64;
}

#[test]
fn a_settled_frame_through_the_wrapper_allocates_nothing() {
    mem_sys::local_reset();
    let counted = mem_sys::local_alloc_count();
    drop(std::hint::black_box(Box::new(1u64)));
    assert!(mem_sys::local_alloc_count() > counted, "the counting allocator is installed");
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    let mut state = State {
        clicks: 0, night: false, check: false, volume: 0.5, mode: 0, seen: 0,
    };
    let input = FrameIn::new(320.0, 240.0);
    // `revision` is the page view's when there is one: a changing one rebuilds the whole page (every canvas callback runs and
    // refills its spans); without a view the retained frame is left alone
    let frame = |ui: &mut Ui, state: &mut State, revision: Option<u64>| {
        ui.frame(&input, &mut |ui: &mut Ui| match revision {
            Some(revision) => { ui.view_of("page", &revision, |ui: &mut Ui| panel(ui, state)); }
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
