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
    list: crate::ListState,
    table: crate::TableState,
}

fn panel(ui: &mut Ui, s: &mut State) {
    ui.panel("settings", &crate::Overlay::DEFAULT, &mut |ui: &mut Ui| {
        ui.label("Display", crate::Label::DEFAULT);
        if ui.button("button", "Apply", &crate::Button::DEFAULT).clicked { s.clicks += 1; }
        s.night = ui.toggle("night", "Night light", s.night, &crate::Toggle::DEFAULT).changed != s.night;
        s.check = s.check != ui.checkbox("check", "Check", s.check, &crate::Checkbox::DEFAULT).changed;
        s.volume = ui.slider("volume", "Volume", s.volume, 0.0..=1.0, &crate::Slider::DEFAULT).value;
        s.mode = ui.radio("mode", &[crate::Segment::new("Auto"), crate::Segment::new("Light"), crate::Segment::new("Dark")], Some(s.mode), &crate::Radio::DEFAULT.name("Mode")).chosen.unwrap_or(s.mode);
        let _ = ui.text_field("Name", "abc", &crate::TextField::DEFAULT);
        let row = ui.node("row", Node::DEFAULT.h(Size::Px(24.0)).axis(Axis::Row));
        ui.within(&row, &mut |ui: &mut Ui| {
            // a keyed node of a grid, as a custom widget declares it
            const COLS: [Track; 2] = [Track::Fr(1.0), Track::Px(40.0)];
            let cell = ui.node(7i64, Node::DEFAULT.grid(Grid::new(&COLS, &[])).cell(Cell::at(0, 0)));
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
    let scroller = ui.node("scroller", Node::DEFAULT.flags(Flags::SCROLL | Flags::CLIP).w(Size::Px(60.0)).h(Size::Px(40.0)).at(4.0, 4.0));
    s.seen += ui.scroll(&scroller).settled as u64 + ui.local_rect(&scroller).w as u64 + ui.cached_label(&scroller, 0, "cached").len() as u64;
    ui.set_text_state(&scroller, None);
    s.seen += ui.text_state(&scroller).is_some() as u64 + ui.hovered(&scroller, false) as u64 + ui.drop_target(&scroller).is_some() as u64;
    let canvas = ui.node("canvas", Node::DEFAULT.w(Size::Px(80.0)).h(Size::Px(30.0)));
    let origin = s.clicks as f32;
    ui.canvas(&canvas, move |p, _| {
        CANVAS_RUNS.fetch_add(1, Ordering::Relaxed);
        for i in 0..SHAPES { p.fill(Rect::new(i as f32 * 0.25, 0.0, 1.0, 1.0), Fill::new(Rgba::rgb(5, 5, 5))); }
        CANVAS_SHAPES.fetch_add(SHAPES, Ordering::Relaxed);
        p.fill(Rect::new(origin, 0.0, 8.0, 8.0), Fill::new(Rgba::rgb(9, 9, 9)));
        p.line((0.0, 0.0), (20.0, 9.0), Stroke::new(Rgba::rgb(1, 2, 3)).width(1.5));
        p.circle((30.0, 10.0), 4.0, Some(Fill::new(Rgba::rgb(7, 7, 7))), None);
        p.text(crate::Point::new(2.0, 12.0), Label::BODY, 0.0, "span");
        p.fill(Rect::new(40.0, 0.0, 8.0, 8.0), Fill::new(Rgba::rgb(9, 9, 9)));
    });
    s.seen += ui.selection_list("items", 3, &|i| i as i64, &mut s.list, &crate::SelectionList::DEFAULT, &mut |ui, part, _| if let crate::ui::SectionPart::Row(row) = part { ui.item_text(["one", "two", "three"][row]) }).activated.is_some() as u64;
    const COLUMNS: [crate::Column; 2] = [crate::Column::new("Name").sortable(true).share(1.0), crate::Column::new("Size").w_px(40.0)];
    s.seen += ui.table("table", &COLUMNS, 3, &|i| i as i64, &mut s.table, &crate::Table::DEFAULT, &mut |ui, _, column, _| ui.item_text(if column == 0 { "row" } else { "1" })).rows.activated.is_some() as u64;
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
        clicks: 0, night: false, check: false, volume: 0.5, mode: 0, seen: 0, list: crate::ListState::new(), table: crate::TableState::new(),
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

/// Pointer moves that change no widget state (hover over an inert label) allocate nothing once warm.
#[test]
fn pointer_moves_over_a_static_ui_allocate_nothing() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    let mut state = State {
        clicks: 0, night: false, check: false, volume: 0.5, mode: 0, seen: 0, list: crate::ListState::new(), table: crate::TableState::new(),
    };
    let build = |ui: &mut Ui, state: &mut State| {
        ui.frame(&FrameIn::new(320.0, 240.0), &mut |ui| panel(ui, state)).expect("frame");
    };
    for _ in 0..4 {
        build(&mut ui, &mut state);
    }
    let mut worst = 0u64;
    for i in 0..8 {
        let event = crate::Event::pointer(crate::Event::POINTER_MOVED, 0, 40.0 + i as f64, 20.0);
        mem_sys::local_reset();
        ui.frame(&{
            let mut frame = FrameIn::new(320.0, 240.0).time(0.1 * i as f64);
            frame.events = core::slice::from_ref(&event);
            frame
        }, &mut |ui| panel(ui, &mut state)).expect("frame");
        worst = worst.max(mem_sys::local_alloc_count());
    }
    assert_eq!(worst, 0, "pointer moves over a static UI allocate nothing");
    assert!(state.seen > 0 || state.clicks == 0);
}

/// Key presses that no focused control consumes allocate nothing once warm.
#[test]
fn key_presses_over_a_static_ui_allocate_nothing() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    let mut state = State {
        clicks: 0, night: false, check: false, volume: 0.5, mode: 0, seen: 0, list: crate::ListState::new(), table: crate::TableState::new(),
    };
    for _ in 0..4 {
        ui.frame(&FrameIn::new(320.0, 240.0), &mut |ui| panel(ui, &mut state)).expect("frame");
    }
    let keys = [
        crate::KeyCode::ArrowUp, crate::KeyCode::ArrowDown, crate::KeyCode::ArrowLeft, crate::KeyCode::ArrowRight,
        crate::KeyCode::Char('a'), crate::KeyCode::Char('b'),
    ];
    // First press of each code may grow spare key/modifier lists; warm them before counting.
    for (i, code) in keys.into_iter().enumerate() {
        let warm = crate::Event::key(crate::Event::KEY_PRESSED, code.wire());
        ui.frame(&{
            let mut frame = FrameIn::new(320.0, 240.0).time(0.01 * i as f64);
            frame.events = core::slice::from_ref(&warm);
            frame
        }, &mut |ui| panel(ui, &mut state)).expect("frame");
    }
    let mut worst = 0u64;
    for (i, code) in keys.into_iter().enumerate() {
        let event = crate::Event::key(crate::Event::KEY_PRESSED, code.wire());
        mem_sys::local_reset();
        ui.frame(&{
            let mut frame = FrameIn::new(320.0, 240.0).time(1.0 + 0.1 * i as f64);
            frame.events = core::slice::from_ref(&event);
            frame
        }, &mut |ui| panel(ui, &mut state)).expect("frame");
        worst = worst.max(mem_sys::local_alloc_count());
    }
    assert_eq!(worst, 0, "key presses over a static UI allocate nothing");
}
