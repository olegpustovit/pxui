//! A settled frame allocates nothing: a small widget panel built through the wrapper in static mode, with a signal read
//! per interactive box and every output accessor, between two counted points. The library's counting allocator is
//! `mem_sys`, installed for the test binary by the `use`.
use crate::{Node, Ui};
use mem_sys as _;
use pxui_types::{Axis, FrameIn, Size, layout::{Cell, Grid, Track}};

struct State { clicks: u32, night: bool, check: bool, volume: f32, mode: usize, seen: u64 }

fn panel(ui: &mut Ui, s: &mut State) {
    ui.panel("settings", |ui| {
        ui.label("Display");
        if ui.button("Apply") { s.clicks += 1; }
        s.night = ui.toggle("Night light", s.night);
        s.check = ui.checkbox("Check", s.check).unwrap_or(s.check);
        s.volume = ui.slider("Volume", s.volume, 0.0..=1.0).unwrap_or(s.volume);
        s.mode = ui.radio("Mode", &["Auto", "Light", "Dark"], s.mode).unwrap_or(s.mode);
        let _ = ui.text_field("Name", "abc");
        let row = ui.node("row", &Node { h: Size::Px(24.0), axis: Axis::Row, ..Node::DEFAULT }).unwrap();
        ui.within(&row, |ui| {
            // a keyed node of a grid, as a custom widget declares it
            const COLS: [Track; 2] = [Track::Fr(1.0), Track::Px(40.0)];
            let cell = ui.node_keyed(7, &Node { grid: Some(Grid { cols: &COLS, rows: &[] }), cell: Cell::at(0, 0), ..Node::DEFAULT }).unwrap();
            let signal = ui.signal(&cell).unwrap();
            s.clicks += signal.clicked as u32;
            ui.text(&cell, "custom").unwrap();
        });
        let signal = ui.signal(&row).unwrap();
        s.clicks += signal.keys.len() as u32;
    });
}

#[test]
fn a_settled_frame_through_the_wrapper_allocates_nothing() {
    mem_sys::local_reset();
    let counted = mem_sys::local_alloc_count();
    drop(std::hint::black_box(Box::new(1u64)));
    assert!(mem_sys::local_alloc_count() > counted, "the counting allocator is installed");
    let mut ui = Ui::new().expect("library");
    let mut state = State { clicks: 0, night: false, check: false, volume: 0.5, mode: 0, seen: 0 };
    let input = FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() };
    let frame = |ui: &mut Ui, state: &mut State| {
        ui.frame(&input, |ui| panel(ui, state)).expect("frame");
        // the stat accessors read the cached output: no rebuild, no allocation
        state.seen += ui.draw_revision() + ui.built_nodes() + ui.rebuilt_views() + ui.arranged_nodes() + ui.repaint_at().map_or(0, |_| 1);
        let output = ui.output();
        state.seen += output.a11y.nodes.len as u64 + output.platform.has_clipboard as u64;
    };
    for _ in 0..8 { frame(&mut ui, &mut state); }
    let mut worst = 0;
    for _ in 0..4 {
        let before = mem_sys::local_alloc_count();
        frame(&mut ui, &mut state);
        worst = worst.max(mem_sys::local_alloc_count() - before);
    }
    assert_eq!(worst, 0, "a settled frame allocates nothing");
    assert!(state.seen > 0);
}
