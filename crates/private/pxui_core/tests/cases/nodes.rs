//! Retained nodes (layer 1): stable handles, direct patches between builds, and the frame arena.
use crate::support::*;
use pxui_core::geom::color;
use pxui_core::ui::*;
use std::cell::Cell;
use std::rc::Rc;

fn frame(ui: &mut Ui, t: f64, f: &mut dyn FnMut(&mut Ui)) {
    ui.frame(&frame_at(vec![], 2.0, t, vec![]), f);
}

fn chip(ui: &mut Ui, key: &str, text: &str, fill: color::T) -> Handle {
    let look = Look { fill, text: color::black(), ..Look::NONE };
    let h = ui.node(key, Node { w: Size::Px(80.0), h: Size::Px(20.0), look, inset: Edges { l: 4.0, ..Edges::default() }, ..Node::DEFAULT });
    ui.text(&h, text);
    h
}

fn has_color(ui: &Ui, c: color::T) -> bool {
    ui.output().draw[0].instances.iter().any(|i| i.color == [c.r as u8, c.g as u8, c.b as u8, c.a as u8])
}

pub fn window_pxui_current_build_box_rejects_forged_index() {
    let mut ui = new_ui(None);
    let (mut valid, mut forged) = (false, false);
    frame(&mut ui, 0.0, &mut |ui| {
        let b = ui.node("handle", Node::DEFAULT);
        valid = ui.is_current_build_box(&b);
        forged = ui.is_current_build_box(&Handle { index: b.index + 1, ..b });
    });
    check(valid && !forged, "current-build check accepted a forged index or rejected its own handle");
    ui.destroy();
}

pub fn window_pxui_node_handles() {
    let mut ui = new_ui(None);
    let (mut first, mut again) = (None, None);
    frame(&mut ui, 0.0, &mut |ui| first = Some(chip(ui, "a", "one", color::white())));
    frame(&mut ui, 0.1, &mut |ui| again = Some(chip(ui, "a", "one", color::white())));
    let (a, again) = (first.unwrap(), again.unwrap());
    check(a.box_slot == again.box_slot && a.generation == again.generation && ui.live(&a), "a node's handle changed across frames");
    // the owner stops declaring it: the handle is stale, and a new node in its slot is not reached
    frame(&mut ui, 0.2, &mut |_| {});
    check(!ui.live(&a) && !ui.patch_text(&a, "x") && ui.rect(&a).w == 0.0, "a stale handle reached its old slot");
    let mut b = None;
    frame(&mut ui, 0.3, &mut |ui| b = Some(chip(ui, "b", "two", color::white())));
    let b = b.unwrap();
    check(b.box_slot == a.box_slot && !ui.live(&a) && ui.live(&b), "the slot was not reused under a new generation");
    check(!ui.patch_fill(&a, color::rgb(1, 2, 3)) && !has_color(&ui, color::rgb(1, 2, 3)), "a stale patch reached the new node");
    check(!ui.signal(&a).hovered, "a stale handle has a signal");
    ui.destroy();
}

pub fn window_pxui_node_patch() {
    let mut ui = new_ui(None);
    let builds = Cell::new(0);
    let (red, blue) = (color::rgb(200, 0, 0), color::rgb(0, 0, 200));
    let handles = Cell::new(None);
    let mut build = |ui: &mut Ui| {
        builds.set(builds.get() + 1);
        handles.set(Some((chip(ui, "clock", "12:00", red), chip(ui, "after", "x", color::white()))));
    };
    frame(&mut ui, 0.0, &mut build);
    frame(&mut ui, 0.1, &mut build);
    let (clock, after) = handles.get().unwrap();
    let (built, revision, count) = (builds.get(), ui.draw_revision(), ui.output().draw[0].instances.len());
    check(ui.patch_fill(&clock, blue), "patch_fill");
    check(has_color(&ui, blue) && !has_color(&ui, red), "patch_fill did not rewrite the fill instance");
    check(ui.output().draw[0].instances.len() == count, "an in-place patch changed the instance count");
    // a longer text shifts the later node's range; patching that node still hits its own instances
    check(ui.patch_text(&clock, "12:00:01"), "patch_text");
    let drawn = |ui: &Ui| ui.output().draw[0].instances.iter().filter(|i| **i != pxui_core::draw::Instance::DEFAULT).count();
    check(drawn(&ui) == count + 3, "patch_text did not add the new glyphs");
    // the growth left as much room again: the next growths are written in place, no tail shift
    let shifts = ui.layer_rewrites().0;
    check(shifts == 1, "a growing patch did not shift the tail once");
    let len = ui.output().draw[0].instances.len();
    for (k, t) in ["12:00:012", "12:00:0123", "12:00:01234"].iter().enumerate() {
        check(ui.patch_text(&clock, t) && drawn(&ui) == count + 4 + k, "a growth into the room lost glyphs");
    }
    check(ui.layer_rewrites().0 == shifts && ui.output().draw[0].instances.len() == len, "a growth within the room shifted the layer");
    check(ui.patch_text(&clock, "12:00:01"), "patch_text");
    let green = color::rgb(0, 160, 0);
    check(ui.patch_fill(&after, green) && has_color(&ui, green) && has_color(&ui, blue), "a shifted range missed its node");
    check(builds.get() == built && ui.draw_revision() > revision, "a patch ran the build or kept the revision");
    check(!ui.wants_frame(&[], 10.0), "a pure patch asked for a frame");
    // the owner rebuilds the node: the build's value wins over the patch
    frame(&mut ui, 0.2, &mut build);
    check(has_color(&ui, red) && !has_color(&ui, blue) && ui.output().draw[0].instances.len() == count, "the patch outlived the node's next build");
    ui.destroy();
}

/// A patch made while building, on a node of a view kept inside a kept view (no view rebuilds that
/// frame), is written after the paint.
pub fn window_pxui_node_patch_nested_kept() {
    let mut ui = new_ui(None);
    let (red, blue) = (color::rgb(200, 0, 0), color::rgb(0, 0, 200));
    let handle = Cell::new(None);
    let build = |ui: &mut Ui, patch: bool| {
        ui.view("outer", 1, Size::Px(200.0), Size::Px(100.0), &mut |ui| {
            ui.view("inner", 1, Size::Px(100.0), Size::Px(40.0), &mut |ui| handle.set(Some(chip(ui, "c", "x", red))));
        });
        if patch {
            check(ui.patch_fill(&handle.get().unwrap(), blue), "patch_fill");
        }
    };
    frame(&mut ui, 0.0, &mut |ui| build(ui, false));
    frame(&mut ui, 0.1, &mut |ui| build(ui, false));
    frame(&mut ui, 0.2, &mut |ui| build(ui, true));
    check(ui.rebuilt_views() == 0, "a kept view rebuilt");
    check(has_color(&ui, blue) && !has_color(&ui, red), "a patch made while building missed a node of a nested kept view");
    ui.destroy();
}

thread_local! { static DROPS: Cell<usize> = const { Cell::new(0) }; }

struct Counted(#[allow(dead_code)] u64);
impl std::ops::Drop for Counted {
    fn drop(&mut self) {
        DROPS.with(|d| d.set(d.get() + 1));
    }
}

pub fn window_pxui_arena_closures() {
    let mut ui = new_ui(None);
    let ran = Rc::new(Cell::new(0));
    DROPS.with(|d| d.set(0));
    for t in 0..3 {
        let ran = ran.clone();
        frame(&mut ui, t as f64, &mut |ui| {
            let h = ui.node("c", Node { w: Size::Px(10.0), h: Size::Px(10.0), ..Node::DEFAULT });
            let (byte, word, wide, counted, ran) = (7u8, 9u64, 11u128, Counted(5), ran.clone());
            ui.canvas(&h, move |_, _| {
                let aligned = |p: usize, a: usize| p % a == 0;
                assert!(aligned(&word as *const u64 as usize, 8) && aligned(&wide as *const u128 as usize, align_of::<u128>()), "a capture is misaligned");
                assert!(byte == 7 && word == 9 && wide == 11 && counted.0 == 5, "a capture was corrupted");
                ran.set(ran.get() + 1);
            });
            let one = ui.node("u8", Node { w: Size::Px(1.0), h: Size::Px(1.0), ..Node::DEFAULT });
            ui.canvas_over(&one, move |_, _| assert!(byte == 7));
        });
        check(DROPS.with(|d| d.get()) == t + 1, "a canvas closure was not dropped exactly once after its frame");
    }
    check(ran.get() == 3, "a canvas closure did not run once per frame");
    ui.destroy();
    check(DROPS.with(|d| d.get()) == 3, "destroy dropped a closure twice");
}

/// The first frame of a new canvas node in a warm UI allocates nothing: the closure goes into the
/// frame arena and the node reuses a freed slot.
pub fn window_pxui_arena_first_frame() {
    let mut ui = new_ui(None);
    let kit = |ui: &mut Ui, extra: Option<&str>| {
        ui.panel(PanelOpts::default(), "panel", &mut |ui| {
            ui.label("PXUI");
            ui.button("Quit");
            if let Some(key) = extra {
                let h = ui.node(key, Node { h: Size::Px(10.0), ..Node::DEFAULT });
                let ink = color::rgb(1, 2, 3);
                ui.canvas(&h, move |paint, r| paint.fill(r.x, r.y, r.w, r.h, ink));
            }
        });
    };
    for i in 0..10 {
        frame(&mut ui, i as f64, &mut |ui| kit(ui, Some("old")));
    }
    for i in 10..20 {
        frame(&mut ui, i as f64, &mut |ui| kit(ui, None));
    }
    mem_sys::local_reset();
    let before = mem_sys::local_alloc_count();
    frame(&mut ui, 20.0, &mut |ui| kit(ui, Some("new")));
    let allocs = mem_sys::local_alloc_count() - before;
    check(has_color(&ui, color::rgb(1, 2, 3)), "the new canvas did not paint");
    check(allocs == 0, &format!("the first frame of a new canvas node allocated {allocs} times"));
    ui.destroy();
}

/// Repeated labels take distinct keys in declaration order, the same every frame, with or without an
/// explicitly keyed node between them: the n-th repeat keeps the n-th one's state.
pub fn window_pxui_node_duplicate_keys() {
    let mut ui = new_ui(None);
    let keys = |ui: &mut Ui, t: f64, mark: bool| {
        let out = std::cell::RefCell::new(Vec::new());
        frame(ui, t, &mut |ui| {
            for i in 0..6 {
                let h = ui.node("x", Node::DEFAULT);
                if i == 3 && mark {
                    ui.set_state(&h, 42);
                }
                out.borrow_mut().push((h.box_key, ui.state(&h, 0)));
                if i == 2 {
                    ui.node("y", Node::DEFAULT);
                }
            }
        });
        out.into_inner()
    };
    let first = keys(&mut ui, 0.0, true);
    let second = keys(&mut ui, 0.1, false);
    let mut distinct: Vec<i64> = first.iter().map(|k| k.0).collect();
    distinct.sort();
    distinct.dedup();
    check(distinct.len() == 6, &format!("repeated labels share keys: {first:?}"));
    check(first.iter().map(|k| k.0).eq(second.iter().map(|k| k.0)), &format!("keys moved between frames: {first:?} / {second:?}"));
    check(second.iter().map(|k| k.1).eq([0, 0, 0, 42, 0, 0]), &format!("the fourth repeat lost its state: {second:?}"));
    ui.destroy();
}

/// A canvas capture aligned to 32 bytes (a SIMD-aligned value) runs with its value intact.
pub fn window_pxui_arena_over_aligned() {
    #[derive(Clone, Copy)]
    #[repr(align(32))]
    struct Wide([u8; 4]);
    let mut ui = new_ui(None);
    let ink = Wide([9, 8, 7, 255]);
    for t in 0..3 {
        frame(&mut ui, t as f64, &mut |ui| {
            let h = ui.node("wide", Node { w: Size::Px(10.0), h: Size::Px(10.0), ..Node::DEFAULT });
            ui.canvas(&h, move |paint, r| {
                assert_eq!(&ink as *const Wide as usize % 32, 0, "a capture lost its alignment");
                paint.fill(r.x, r.y, r.w, r.h, color::rgb(ink.0[0] as i64, ink.0[1] as i64, ink.0[2] as i64));
            });
        });
    }
    check(has_color(&ui, color::rgb(9, 8, 7)), "the over-aligned canvas did not paint");
    ui.destroy();
}

/// `Paint::grid` is one instance however many dots it shows.
pub fn window_pxui_paint_grid_one_instance() {
    let mut ui = new_ui(None);
    frame(&mut ui, 0.0, &mut |ui| {
        let h = ui.node("ground", Node { w: Size::Px(600.0), h: Size::Px(400.0), ..Node::DEFAULT });
        ui.canvas(&h, |paint, r| paint.grid(r.x, r.y, r.w, r.h, (0.0, 0.0), 24.0, 2.0, color::rgb(9, 9, 9)));
    });
    let grids = ui.output().draw[0].instances.iter().filter(|i| i.kind == pxui_core::draw::kind::GRID).count();
    check(grids == 1, &format!("{grids} grid instances for 425 dots"));
    ui.destroy();
}

/// A patch that brings a colour glyph after an image, inside a kept view, is drawn the same frame:
/// the layer breaks back to the colour atlas before the glyph and to the image's texture state after.
pub fn window_pxui_node_patch_colour_glyph() {
    let Ok(emoji) = std::fs::read("/System/Library/Fonts/Apple Color Emoji.ttc") else { return println!("note: skipping, no colour emoji font") };
    let mut ui = new_ui(None);
    ui.text_mut().add_font(&emoji);
    let image = pxui_core::ui::Image { handle: Rc::new(()), width: 8, height: 8 };
    let label = Cell::new(None);
    let mut build = |ui: &mut Ui| {
        let img = image.clone();
        ui.view("v", 1, Size::Px(200.0), Size::Px(60.0), &mut |ui| {
            let h = ui.node("pic", Node { w: Size::Px(8.0), h: Size::Px(8.0), ..Node::DEFAULT });
            let img = img.clone();
            ui.canvas(&h, move |paint, r| paint.image(r.x, r.y, r.w, r.h, &img));
            label.set(Some(chip(ui, "label", "plain", color::white())));
        });
    };
    frame(&mut ui, 0.0, &mut build);
    frame(&mut ui, 0.1, &mut build);
    check(ui.patch_text(&label.get().unwrap(), "x\u{1f600}"), "the patch waited for a frame");
    let d = &ui.output().draw[0];
    let glyph = d.instances.iter().rposition(|i| i.kind == pxui_core::draw::kind::IMAGE).expect("the colour glyph");
    let state = d.breaks.iter().filter(|b| b.at_instance as usize <= glyph).last().map(|b| b.image);
    check(state == Some(pxui_core::draw::ATLAS), &format!("the colour glyph samples {state:?}, breaks {:?}", d.breaks));
    ui.destroy();
}

/// A `Mark::Line` node draws its line between its corners inset by half the width; `patch_rect`
/// (from `local_rect`) moves its ends and `patch_mark` turns it over, with no build.
pub fn window_pxui_node_line_patch() {
    let mut ui = new_ui(None);
    let builds = Cell::new(0);
    let handle = Cell::new(None);
    let mut build = |ui: &mut Ui| {
        builds.set(builds.get() + 1);
        let look = Look { text: color::rgb(0, 0, 200), ..Look::NONE };
        handle.set(Some(ui.node("line", Node { w: Size::Px(21.0), h: Size::Px(11.0), at: Some((10.0, 20.0)), look, mark: Mark::Line { width: 1.0, rising: false }, ..Node::DEFAULT })));
    };
    frame(&mut ui, 0.0, &mut build);
    frame(&mut ui, 0.1, &mut build);
    let line = |ui: &Ui| ui.output().draw[0].instances.iter().find(|i| i.kind == pxui_core::draw::kind::LINE).map(|i| i.rect);
    check(line(&ui) == Some([10.5, 20.5, 30.5, 30.5]), &format!("the line's ends: {:?}", line(&ui)));
    let h = handle.get().unwrap();
    let r = ui.local_rect(&h);
    check(ui.patch_rect(&h, pxui_core::geom::Rect::new(r.x + 2.0, r.y, r.w, r.h)) && ui.patch_mark(&h, Mark::Line { width: 1.0, rising: true }), "patch refused");
    check(line(&ui) == Some([12.5, 30.5, 32.5, 20.5]) && builds.get() == 2, &format!("the patched line's ends: {:?}", line(&ui)));
    ui.destroy();
}

pub const CASES: &[(&str, fn())] = &[
    ("window_pxui_current_build_box_rejects_forged_index", window_pxui_current_build_box_rejects_forged_index),
    ("window_pxui_node_patch_colour_glyph", window_pxui_node_patch_colour_glyph),
    ("window_pxui_paint_grid_one_instance", window_pxui_paint_grid_one_instance),
    ("window_pxui_node_line_patch", window_pxui_node_line_patch),
    ("window_pxui_arena_over_aligned", window_pxui_arena_over_aligned),
    ("window_pxui_node_duplicate_keys", window_pxui_node_duplicate_keys),
    ("window_pxui_node_handles", window_pxui_node_handles),
    ("window_pxui_node_patch", window_pxui_node_patch),
    ("window_pxui_node_patch_nested_kept", window_pxui_node_patch_nested_kept),
    ("window_pxui_arena_closures", window_pxui_arena_closures),
    ("window_pxui_arena_first_frame", window_pxui_arena_first_frame),
];
