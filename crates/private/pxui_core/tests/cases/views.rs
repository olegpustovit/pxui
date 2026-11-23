//! Hybrid views (layer 2): cached builds, retained runs, damage, view offsets and virtual lists.
use crate::support::*;
use input::Event;
use pxui_core::geom::Rgba;
use pxui_core::ui::*;
use std::cell::Cell;

fn at(t: f64, events: Vec<Event>) -> input::frame::Frame {
    frame_at(vec![], 2.0, t, events)
}

fn chip(ui: &mut Ui, key: &str, fill: Rgba, flags: Flags) -> Handle {
    ui.node_ref(key, &Node::DEFAULT.flags(flags).w(Size::Px(40.0)).h(Size::Px(20.0)).look(Look::NONE.fill(fill)))
}

const RED: Rgba = Rgba::rgb(200, 0, 0);
const BLUE: Rgba = Rgba::rgb(0, 0, 200);

/// The P5 exit criterion: bumping one view's revision runs only that view's build.
pub fn window_pxui_view_rebuilds_one() {
    let mut ui = new_ui(None);
    let (a_runs, b_runs, rev_a) = (Cell::new(0), Cell::new(0), Cell::new(1u64));
    let mut build = |ui: &mut Ui| {
        ui.view("a", rev_a.get(), Size::Px(100.0), Size::Px(50.0), &mut |ui| {
            a_runs.set(a_runs.get() + 1);
            chip(ui, "x", RED, Flags::NONE);
        });
        ui.view("b", 1, Size::Px(100.0), Size::Px(50.0), &mut |ui| {
            b_runs.set(b_runs.get() + 1);
            chip(ui, "y", BLUE, Flags::NONE);
        });
    };
    ui.frame(&at(0.0, vec![]), &mut build);
    let (instances, revision) = (ui.output().draw[0].instances.clone(), ui.draw_revision());
    ui.frame(&at(0.1, vec![]), &mut build);
    check(a_runs.get() == 1 && b_runs.get() == 1, "a clean view rebuilt");
    check(ui.built_nodes() == 3, &format!("a frame of two kept views declared {} nodes", ui.built_nodes()));
    check(ui.output().draw[0].instances == instances && ui.draw_revision() == revision, "kept views changed the output");
    check(ui.output().damage.bounds().is_none(), "an idle frame has damage");
    rev_a.set(2);
    ui.frame(&at(0.2, vec![]), &mut build);
    check(a_runs.get() == 2 && b_runs.get() == 1, &format!("a revision bump ran builds a {} b {}", a_runs.get(), b_runs.get()));
    // the kept view's node lives on undeclared
    let y = ui.box_of_key(key_of(ui.box_of_key(key_of(0x2c1b3c6d, "b")).map_or(0, |b| b.box_key), "y"));
    check(y.is_some_and(|y| ui.live(&y)), "a kept view's node was pruned");
    // the view stops being declared: its nodes go with it
    ui.frame(&at(0.3, vec![]), &mut |ui| {
        ui.view("a", 2, Size::Px(100.0), Size::Px(50.0), &mut |ui| {
            chip(ui, "x", RED, Flags::NONE);
        });
    });
    check(!y.is_some_and(|y| ui.live(&y)), "a dropped view's node lived on");
    ui.destroy();
}

/// A layer that shrinks before the host drew it keeps its dirty ranges inside its instances: the painter
/// refuses a range past the layer's length (an app's graph pane built six frames, shrinking, before one draw).
pub fn window_pxui_shrunk_layer_clips_dirty() {
    let mut ui = new_ui(None);
    let (count, rev) = (Cell::new(6), Cell::new(1u64));
    let mut build = |ui: &mut Ui| {
        ui.view("a", rev.get(), Size::Px(100.0), Size::Px(200.0), &mut |ui| {
            for i in 0..count.get() { chip(ui, &format!("c{i}"), RED, Flags::NONE); }
        });
    };
    ui.frame(&at(0.0, vec![]), &mut build);
    (count.set(1), rev.set(2));
    ui.frame(&at(0.1, vec![]), &mut build);
    for layer in ui.output().draw.iter() {
        check(layer.dirty.iter().all(|&(a, b)| a <= b && b as usize <= layer.instances.len()), &format!("dirty {:?} of a layer of {} instances", layer.dirty, layer.instances.len()));
    }
    ui.destroy();
}

/// A dirty parent rebuilds; the clean view inside it is still kept.
pub fn window_pxui_view_nested() {
    let mut ui = new_ui(None);
    let (outer, inner, rev) = (Cell::new(0), Cell::new(0), Cell::new(1u64));
    let mut build = |ui: &mut Ui| {
        ui.view("outer", rev.get(), Size::Px(200.0), Size::Px(100.0), &mut |ui| {
            outer.set(outer.get() + 1);
            chip(ui, "o", RED, Flags::NONE);
            ui.view("inner", 1, Size::Px(100.0), Size::Px(50.0), &mut |ui| {
                inner.set(inner.get() + 1);
                chip(ui, "i", BLUE, Flags::NONE);
            });
        });
    };
    ui.frame(&at(0.0, vec![]), &mut build);
    let instances = ui.output().draw[0].instances.clone();
    rev.set(2);
    ui.frame(&at(0.1, vec![]), &mut build);
    check(outer.get() == 2 && inner.get() == 1, "the clean inner view rebuilt with its parent");
    check(ui.output().draw[0].instances == instances, "a parent rebuild changed the kept child's output");
    ui.destroy();
}

/// Hover and click reach nodes of a kept view and rebuild only that view.
pub fn window_pxui_view_input() {
    let mut ui = new_ui(None);
    let (a_runs, b_runs) = (Cell::new(0), Cell::new(0));
    let (hovered, clicked) = (Cell::new(false), Cell::new(false));
    let mut build = |ui: &mut Ui| {
        ui.view("a", 1, Size::Px(100.0), Size::Px(50.0), &mut |ui| {
            a_runs.set(a_runs.get() + 1);
            let h = chip(ui, "x", RED, Flags::CLICKABLE);
            let s = ui.signal_owned(&h);
            hovered.set(s.hovered);
            clicked.set(clicked.get() || s.clicked);
        });
        ui.view("b", 1, Size::Px(100.0), Size::Px(50.0), &mut |ui| {
            b_runs.set(b_runs.get() + 1);
            chip(ui, "y", BLUE, Flags::CLICKABLE);
        });
    };
    ui.frame(&at(0.0, vec![move_to((300, 230))]), &mut build);
    ui.frame(&at(0.1, vec![]), &mut build);
    check((a_runs.get(), b_runs.get()) == (1, 1), "warm frames rebuilt");
    ui.frame(&at(0.2, vec![move_to((10, 10))]), &mut build);
    check(a_runs.get() == 2 && b_runs.get() == 1 && hovered.get(), "hover entering a kept view's node");
    ui.frame(&at(0.3, vec![move_to((12, 11))]), &mut build);
    check(a_runs.get() == 2, "motion inside one node rebuilt its view");
    ui.frame(&at(0.4, vec![press((12, 11)), release((12, 11))]), &mut build);
    check(a_runs.get() == 3 && b_runs.get() == 1 && clicked.get(), "a click in a kept view");
    ui.frame(&at(0.5, vec![move_to((300, 230))]), &mut build);
    check(a_runs.get() == 4 && b_runs.get() == 1 && !hovered.get(), "hover leaving a kept view's node");
    ui.destroy();
}

/// Offset and opacity change one table entry: no instance rewritten, damage reported, hit testing
/// and `rect` follow.
pub fn window_pxui_view_offset() {
    let mut ui = new_ui(None);
    let (runs, view, node) = (Cell::new(0), Cell::new(None), Cell::new(None));
    let mut build = |ui: &mut Ui| {
        view.set(Some(ui.view("a", 1, Size::Px(100.0), Size::Px(50.0), &mut |ui| {
            runs.set(runs.get() + 1);
            node.set(Some(chip(ui, "x", RED, Flags::CLICKABLE)));
        })));
    };
    ui.frame(&at(0.0, vec![move_to((300, 230))]), &mut build);
    ui.frame(&at(0.1, vec![]), &mut build);
    let (v, n) = (view.get().unwrap(), node.get().unwrap());
    let (instances, revision) = (ui.output().draw[0].instances.clone(), ui.draw_revision());
    ui.view_offset(&v, 30.0, 10.0);
    check(ui.output().draw[0].instances == instances, "an offset rewrote instances");
    check(ui.draw_revision() == revision + 1, "an offset kept the revision");
    let entry = ui.output().views.iter().find(|(_, e)| e.offset == [30.0, 10.0]);
    check(entry.is_some(), "no view entry carries the offset");
    check(ui.output().draw[0].instances.iter().any(|i| Some(i.view) == entry.map(|e| e.0)), "the view's instances do not name its entry");
    check(ui.output().damage.bounds() == Some(Rect::new(0.0, 0.0, 130.0, 60.0)), &format!("offset damage {:?}", ui.output().damage.bounds()));
    check(ui.rect(&n) == Rect::new(30.0, 10.0, 40.0, 20.0), &format!("rect after an offset {:?}", ui.rect(&n)));
    ui.view_opacity(&v, 0.5);
    check(ui.output().views.iter().any(|(_, e)| e.opacity == 0.5) && ui.output().draw[0].instances == instances, "opacity");
    // the old place no longer hits; the new one does, and the frame keeps the offset
    ui.frame(&at(0.2, vec![move_to((5, 5))]), &mut build);
    check(!ui.signal_owned(&n).hovered, "the node hit at its old place");
    ui.frame(&at(0.3, vec![move_to((35, 15))]), &mut build);
    check(ui.signal_owned(&n).hovered && runs.get() == 2, "the node missed at its offset place");
    check(ui.output().views.iter().any(|(_, e)| e.offset == [30.0, 10.0]), "a frame dropped the offset");
    ui.destroy();
}

/// Two thousand views (one per graph card) each keep a table entry: an offset, opacity and clip
/// change rewrites no instance and rebuilds no view.
pub fn window_pxui_view_table_2000() {
    let mut ui = new_ui(None);
    let views = std::cell::RefCell::new(Vec::new());
    let mut build = |ui: &mut Ui| {
        let mut v = views.borrow_mut();
        v.clear();
        for k in 0..2000 {
            v.push(ui.view(&format!("card{k}"), 1, Size::Px(20.0), Size::Px(10.0), &mut |ui| {
                chip(ui, "x", RED, Flags::CLICKABLE);
            }));
        }
    };
    ui.frame(&at(0.0, vec![]), &mut build);
    ui.frame(&at(0.1, vec![]), &mut build);
    let instances = ui.output().draw[0].instances.clone();
    let last = views.borrow()[1999];
    ui.view_offset(&last, 7.0, 3.0);
    ui.view_opacity(&last, 0.25);
    ui.view_clip(&last, Some(Rect::new(0.0, 0.0, 5.0, 5.0)));
    let entry = ui.output().views.iter().find(|(_, e)| e.offset == [7.0, 3.0]).copied();
    check(entry.is_some_and(|(i, e)| i >= 2000 && e.opacity == 0.25 && e.clip[2] < 1.0e6), &format!("view 2000's entry {entry:?}"));
    check(ui.output().draw[0].instances == instances, "a table change rewrote instances");
    check(ui.output().draw[0].instances.iter().any(|i| Some(i.view) == entry.map(|e| e.0)), "the view's instances do not name its entry");
    ui.frame(&at(0.2, vec![]), &mut build);
    check(ui.rebuilt_views() == 0 && ui.output().views.iter().filter(|(i, _)| *i != 0).count() >= 2000, "a moved view rebuilt or lost its entry");
    ui.destroy();
}

/// A kept view that draws an image and then text keeps its break back to the atlas when its layer
/// is placed again around it (the text must not sample the image).
pub fn window_pxui_view_kept_atlas_break() {
    // a colour glyph after the image needs the break back to the colour atlas
    let mut ui = new_ui(None);
    // the 190 MB system emoji collection is over `MAX_FONT_BYTES`: the system provider resolves it instead
    if !ui.text_mut().enable_system_fonts() { return println!("note: skipping, no system fonts"); }
    let image = pxui_core::ui::Image { handle: std::rc::Rc::new(()), width: 8, height: 8 };
    let rev = Cell::new(1u64);
    let mut build = |ui: &mut Ui| {
        let img = image.clone();
        ui.label(&format!("before {}", rev.get()), pxui_core::ui::Label::DEFAULT);
        ui.view("v", 1, Size::Px(100.0), Size::Px(40.0), &mut |ui| {
            let h = ui.node("pic".into(), Node::DEFAULT.w(Size::Px(8.0)).h(Size::Px(8.0)));
            let img = img.clone();
            ui.canvas(&h, move |paint, r| paint.image(r.x, r.y, r.w, r.h, &img));
            ui.label("\u{1f600}", pxui_core::ui::Label::DEFAULT);
        });
    };
    for t in 0..2 {
        ui.frame(&at(t as f64 * 0.1, vec![]), &mut build);
    }
    let atlas_after_image = |ui: &Ui| {
        let d = &ui.output().draw[0];
        // the emoji's instance follows the image's directly: the atlas comes back right after it
        let k = d.breaks.iter().position(|b| b.image != pxui_core::draw::ATLAS);
        k.is_some_and(|k| d.breaks.get(k + 1).is_some_and(|b| b.image == pxui_core::draw::ATLAS && b.at_instance <= d.breaks[k].at_instance + 1))
    };
    check(atlas_after_image(&ui), "the fresh view has no break back to the atlas");
    // the label before it grows: the layer is placed again with the view kept
    rev.set(1234567);
    ui.frame(&at(0.3, vec![]), &mut build);
    check(ui.rebuilt_views() == 0, "the view rebuilt");
    check(atlas_after_image(&ui), "the kept view lost its break back to the atlas");
    ui.destroy();
}

/// Damage: none for an idle frame; one node's rectangle for a one-node change; old and new for a
/// patch that moves it.
pub fn window_pxui_view_damage() {
    let mut ui = new_ui(None);
    let (fill, rev, node) = (Cell::new(RED), Cell::new(1u64), Cell::new(None));
    let mut build = |ui: &mut Ui| {
        ui.view("v", rev.get(), Size::Px(100.0), Size::Px(60.0), &mut |ui| {
            chip(ui, "first", BLUE, Flags::NONE);
            node.set(Some(chip(ui, "second", fill.get(), Flags::NONE)));
        });
        chip(ui, "root", BLUE, Flags::NONE);
    };
    ui.frame(&at(0.0, vec![]), &mut build);
    ui.frame(&at(0.1, vec![]), &mut build);
    check(ui.output().damage.bounds().is_none(), "an idle frame has damage");
    for i in 0..ui.output().draw.len() {
        ui.drawn(i);
    }
    (fill.set(Rgba::rgb(0, 160, 0)), rev.set(2));
    ui.frame(&at(0.2, vec![]), &mut build);
    check(ui.output().damage.bounds() == Some(Rect::new(0.0, 20.0, 40.0, 20.0)), &format!("one node's damage {:?}", ui.output().damage.bounds()));
    check(ui.output().draw[0].dirty.len() == 1 && ui.output().draw[0].dirty[0].1 - ui.output().draw[0].dirty[0].0 == 1, &format!("dirty {:?}", ui.output().draw[0].dirty));
    let n = node.get().unwrap();
    check(ui.patch_fill(&n, BLUE) && ui.output().damage.bounds() == Some(Rect::new(0.0, 20.0, 40.0, 20.0)), "patch damage");
    check(ui.patch_offset(&n, 5.0, 0.0) && ui.output().damage.bounds() == Some(Rect::new(0.0, 20.0, 45.0, 20.0)), &format!("move damage {:?}", ui.output().damage.bounds()));
    let mut shadow = Look::NONE.fill(BLUE).shadows([Shadow::NONE.color(Rgba::BLACK).blur(4.0).spread(2.0).offset(3.0, 1.0), Shadow::NONE]);
    ui.drawn(0);
    assert!(ui.patch_look(&n,&shadow));
    assert_eq!(ui.output().damage.bounds(),Some(Rect::new(0.0,12.0,56.0,37.0)));
    ui.drawn(0);
    shadow.shadows[0].blur=8.0;
    assert!(ui.patch_look(&n,&shadow));
    assert_eq!(ui.output().damage.bounds(),Some(Rect::new(0.0,6.0,62.0,49.0)),"changed shadow parameters must damage the larger new cover");
    ui.destroy();
}

fn window_pxui_full_damage_uploads_changed_instances() {
    let mut ui = new_ui(None);
    let mut theme = Theme::default();
    theme.tokens.reduced_motion = true;
    let mut build = |ui: &mut Ui| {
        for key in ["first", "second", "third"] {
            chip(ui, key, ui.theme().tokens.panel, Flags::NONE);
        }
    };
    ui.set_theme(theme);
    ui.frame(&at(0.0, vec![]), &mut build);
    ui.drawn(0);
    theme.tokens.panel = BLUE;
    ui.set_theme(theme);
    ui.frame(&at(1.0, vec![]), &mut build);
    let out = ui.output();
    assert_eq!(out.damage.bounds(), Some(Rect::new(0.0, 0.0, 320.0, 240.0)));
    assert_eq!(out.draw[0].dirty, [(0, 3)]);
    assert!(out.draw[0].instances.iter().all(|i| i.color == [0, 0, 200, 255]));
    ui.drawn(0);
    ui.frame(&at(2.0, vec![]), &mut build);
    assert!(ui.output().damage.bounds().is_none());
    assert!(ui.output().draw[0].dirty.is_empty());
    ui.destroy();
}

fn window_pxui_full_damage_uploads_fresh_and_relocated_kept_ranges() {
    let mut ui = new_ui(None);
    let count = Cell::new(1);
    let ink = Cell::new(RED);
    let mut build = |ui: &mut Ui| {
        for index in 0..count.get() {
            ui.node((index as i64).into(), Node::DEFAULT
                .w(Size::Px(10.0)).h(Size::Px(8.0)).at(index as f32 * 10.0, 0.0)
                .look(Look::NONE.fill(ink.get())));
        }
        ui.view("kept", 0, Size::Px(100.0), Size::Px(50.0), &mut |ui| {
            chip(ui, "kept-content", BLUE, Flags::NONE);
            ui.retain_view_on_resize();
        });
    };
    ui.frame(&at(0.0, vec![]), &mut build);
    let mut uploaded = ui.output().draw[0].instances.clone();
    for (step, fresh) in [1, 16].into_iter().enumerate() {
        ui.drawn(0);
        count.set(fresh);
        ink.set(if step == 0 { BLUE } else { RED });
        let mut frame = at(step as f64 + 1.0, vec![]);
        frame.input.size.0 += step as i64 + 1;
        ui.frame(&frame, &mut build);
        assert_eq!(ui.rebuilt_views(), 0, "the unchanged view stays built");
        let out = ui.output();
        assert_eq!(out.damage.bounds(), Some(Rect::new(0.0, 0.0, frame.input.size.0 as f32, 240.0)));
        let layer = &out.draw[0];
        uploaded.resize(layer.instances.len(), pxui_types::draw::Instance::DEFAULT);
        for &(start, end) in &layer.dirty {
            let range = start as usize..end as usize;
            uploaded[range.clone()].copy_from_slice(&layer.instances[range]);
        }
        assert_eq!(uploaded, layer.instances);
    }
    ui.destroy();
}

/// A kit panel in a view: kept and rebuilt output are the same instances.
pub fn window_pxui_view_kit_cached() {
    let runs = Cell::new(0);
    let panel = |ui: &mut Ui, rev: u64| {
        ui.view("kit", rev, Size::Px(310.0), Size::Px(220.0), &mut |ui| {
            runs.set(runs.get() + 1);
            ui.panel("panel", &pxui_core::ui::Overlay::DEFAULT.name("panel"), &mut |ui| {
                ui.label("PXUI", pxui_core::ui::Label::DEFAULT);
                ui.toggle("Animate", true);
                ui.text_field("Caption", "Functional UI");
                ui.button("Quit");
            });
        });
    };
    let (mut kept, mut fresh) = (new_ui(None), new_ui(None));
    for t in 0..4 {
        kept.frame(&at(t as f64, vec![]), &mut |ui| panel(ui, 1));
        fresh.frame(&at(t as f64, vec![]), &mut |ui| panel(ui, t));
    }
    check(runs.get() == 1 + 4, &format!("the kit panel view ran {} builds", runs.get()));
    check(kept.output().draw[0].instances == fresh.output().draw[0].instances, "kept and rebuilt kit panels differ");
    check(kept.output().views == fresh.output().views, "kept and rebuilt view entries differ");
    kept.destroy();
    fresh.destroy();
}

/// 100,000 rows build only the rows in view; a row keeps its state while it scrolls.
pub fn window_pxui_list() {
    let mut ui = new_ui(None);
    let (built, cells) = (Cell::new(0), Cell::new(None));
    let mut build = |ui: &mut Ui| {
        built.set(0);
        ui.list("rows", 100_000, 24.0, &mut |ui, i| {
            built.set(built.get() + 1);
            let cell = ui.node("cell".into(), Node::DEFAULT.h(Size::Px(24.0)));
            if i == 5 {
                cells.set(Some(cell));
            }
        });
    };
    ui.frame(&at(0.0, vec![move_to((100, 100))]), &mut build);
    ui.frame(&at(0.1, vec![]), &mut build);
    // 240 points of 24-point rows: 10 visible, one partly, and 4 of overscan past each edge
    check(built.get() <= 10 + 1 + 2 * 4, &format!("{} rows built", built.get()));
    check(ui.built_nodes() <= 3 + 2 * built.get(), &format!("{} nodes for {} rows", ui.built_nodes(), built.get()));
    let cell = cells.get().unwrap();
    ui.set_state(&cell, 42);
    ui.frame(&at(0.2, vec![Event::Scrolled((0.0, -3.0))]), &mut build);
    check(built.get() <= 10 + 1 + 2 * 4, &format!("{} rows built after scrolling", built.get()));
    let again = cells.get().unwrap();
    check(again.box_slot == cell.box_slot && ui.live(&cell) && ui.state(&again, 0) == 42, "a row lost its state scrolling");
    check(ui.rect(&again).y == 5.0 * 24.0 - 72.0, &format!("row 5 at {:?} after three rows of scroll", ui.rect(&again)));
    ui.destroy();
}

/// A forgotten revision: verify mode re-runs the clean view and names it.
pub fn window_pxui_view_verify() {
    let fill = Cell::new(RED);
    let mut ui = new_ui(None);
    let mut build = |ui: &mut Ui| {
        ui.view("stale-view", 1, Size::Px(100.0), Size::Px(50.0), &mut |ui| {
            chip(ui, "x", fill.get(), Flags::NONE);
        });
    };
    ui.frame(&at(0.0, vec![]), &mut build);
    ui.verify_views(true);
    ui.frame(&at(0.1, vec![]), &mut build);
    fill.set(BLUE);
    let message = verify_failure(&mut ui, 0.2, &mut build);
    check(message.contains("stale-view"), &format!("verify did not name the stale view: {message:?}"));
}

/// The panic message of a frame that is expected to fail verification ("" when it passed).
fn verify_failure(ui: &mut Ui, t: f64, build: &mut dyn FnMut(&mut Ui)) -> String {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { ui.frame(&at(t, vec![]), build); }));
    result.err().and_then(|e| e.downcast_ref::<String>().cloned()).unwrap_or_default()
}

/// Verify mode with a look transition running in a view: the build that started it is compared as
/// it painted (no false report while the fade runs, and the output still moves frame by frame). A
/// look the build declares unlike the kept build's is named: under a running fade, as a stale state
/// that starts one, and after a fade came to rest.
pub fn window_pxui_view_verify_transition() {
    const GREEN: Rgba = Rgba::rgb(0, 200, 0);
    let (hot, fill, rev) = (Cell::new(false), Cell::new(BLUE), Cell::new(1u64));
    let mut build = |ui: &mut Ui| {
        ui.view("fade", rev.get(), Size::Px(100.0), Size::Px(50.0), &mut |ui| {
            let n = chip(ui, "x", RED, Flags::NONE);
            let look = Look::NONE.fill(if hot.get() { fill.get() } else { RED });
            ui.set_look(&n, look, State::default().hovered(hot.get()));
        });
    };
    let fresh = || {
        let mut ui = new_ui(None);
        let mut theme = Theme::default();
        theme.tokens.motion.normal = 0.2;
        ui.set_theme(theme);
        ui.verify_views(true);
        ui
    };
    let blue = |ui: &Ui| ui.output().draw[0].instances.iter().filter(|i| i.color[1] == 0 && i.color[2] > 0).map(|i| i.color[2]).max().unwrap_or(0);
    let hover = |ui: &mut Ui, build: &mut dyn FnMut(&mut Ui)| {
        ui.frame(&at(0.0, vec![]), build);
        ui.frame(&at(0.1, vec![]), build);
        (hot.set(true), rev.set(2));
        ui.frame(&at(1.0, vec![]), build);
    };
    let mut ui = fresh();
    hover(&mut ui, &mut build);
    check(ui.transitions() == 1, &format!("{} transitions after the hover", ui.transitions()));
    let mut seen = vec![blue(&ui)];
    for k in 1..=3 {
        let message = verify_failure(&mut ui, 1.0 + 0.04 * k as f64, &mut build);
        check(message.is_empty(), &format!("a running fade was reported: {message:?}"));
        seen.push(blue(&ui));
    }
    check(seen.windows(2).all(|w| w[0] < w[1]), &format!("the fill did not move toward the hover look: {seen:?}"));
    fill.set(GREEN);
    let message = verify_failure(&mut ui, 1.16, &mut build);
    check(message.contains("\"fade\""), &format!("a stale look under a running fade was not named: {message:?}"));
    // at rest, a stale fill is still checked: finishing the fade is no patch that skips the view
    (hot.set(false), fill.set(BLUE), rev.set(1));
    let mut ui = fresh();
    hover(&mut ui, &mut build);
    for k in 1..=8 {
        let message = verify_failure(&mut ui, 1.0 + 0.05 * k as f64, &mut build);
        check(message.is_empty(), &format!("a fade to rest was reported: {message:?}"));
    }
    check(ui.transitions() == 0 && blue(&ui) == BLUE.b, "the fade did not come to rest on the hover look");
    fill.set(GREEN);
    let message = verify_failure(&mut ui, 1.5, &mut build);
    check(message.contains("\"fade\""), &format!("a stale fill after a fade was not named: {message:?}"));
    // a stale state starts a fade whose first frame still shows the kept look
    (hot.set(false), fill.set(BLUE), rev.set(1));
    let mut ui = fresh();
    hover(&mut ui, &mut build);
    for k in 1..=8 {
        ui.frame(&at(1.0 + 0.05 * k as f64, vec![]), &mut build);
    }
    hot.set(false);
    let message = verify_failure(&mut ui, 1.5, &mut build);
    check(message.contains("\"fade\""), &format!("a stale state starting a fade was not named: {message:?}"));
}

/// Verify mode with a hover fade inside a clean view: the node, in a view rebuilt every frame (a reorderable list row's), reads its
/// hover signal and declares the hovered look, so the pointer reaching it (moving on, or there as the view first shows) starts a
/// fade while the enclosing page view is kept and checked. No frame is reported.
pub fn window_pxui_view_verify_hover_fade() {
    let frame = Cell::new(0u64);
    let build = |ui: &mut Ui| {
        frame.set(frame.get() + 1);
        ui.view("page", 1, Size::Px(300.0), Size::Px(100.0), &mut |ui| {
            chip(ui, "title", BLUE, Flags::NONE);
            ui.view("rows", frame.get(), Size::Px(200.0), Size::Px(60.0), &mut |ui| {
                let n = chip(ui, "row", RED, Flags::CLICKABLE);
                let hovered = ui.signal(&n).hovered;
                ui.set_look(&n, Look::NONE.fill(if hovered { BLUE } else { RED }), State::default().hovered(hovered));
            });
        });
    };
    for already in [false, true] {
        let mut ui = new_ui(None);
        let mut theme = Theme::default();
        theme.tokens.motion.normal = 0.2;
        ui.set_theme(theme);
        ui.verify_views(true);
        let mut build = build;
        let over = vec![Event::PointerMoved((10.0, 30.0))];
        let mut faded = false;
        ui.frame(&at(0.0, if already { over.clone() } else { vec![] }), &mut build);
        for k in 1..=12 {
            let events = if k == 2 && !already { over.clone() } else { vec![] };
            let t = 0.03 * k as f64;
            // a reported fade panics in verify mode, failing the case
            ui.frame(&at(t, events), &mut build);
            faded |= ui.transitions() > 0;
        }
        check(faded, &format!("no hover fade ran (pointer there first: {already})"));
    }
}

/// A view rebuilt while its child views are kept starts where its first content does: the room a compaction left ahead of it
/// stays outside its run, whether fresh pieces end against the child or the view's first piece is empty. Taking it in made every
/// compaction and rebuild grow the view's run (and the layer).
pub fn window_pxui_rebuilt_view_keeps_compaction_room_outside_its_run() {
    let frame = Cell::new(0u64);
    let mut build = |ui: &mut Ui| {
        let n = frame.get();
        frame.set(n + 1);
        // fresh content ahead of the views: growing past its room while the parent is kept compacts the layer (room goes ahead of
        // the parent's run); the parent rebuilds five frames later, in place
        for i in 0..(if (n / 10) % 2 == 0 { 1 } else { 41 }) {
            chip(ui, &format!("lead{i}"), RED, Flags::NONE);
        }
        // a clipped view (its own entry in the view table), as a panel's part is
        let parent = ui.view("parent", (n + 5) / 10, Size::Px(200.0), Size::Px(100.0), &mut |ui| {
            chip(ui, "head", BLUE, Flags::NONE);
            for k in 0..3 {
                ui.view(&format!("child{k}"), 1, Size::Px(120.0), Size::Px(20.0), &mut |ui| {
                    for i in 0..3 { chip(ui, &format!("c{i}"), RED, Flags::NONE); }
                });
                chip(ui, &format!("sep{k}"), BLUE, Flags::NONE);
            }
        });
        ui.view_clip(&parent, Some(pxui_core::ui::Rect::new(0.0, 0.0, 400.0, 300.0)));
        // a view that starts with a child view: its first piece is empty, at the start of the room before that child
        ui.view("bare", (n + 5) / 10, Size::Px(200.0), Size::Px(40.0), &mut |ui| {
            for k in 0..2 {
                ui.view(&format!("key{k}"), 1, Size::Px(40.0), Size::Px(20.0), &mut |ui| { chip(ui, "glyph", RED, Flags::NONE); });
            }
        });
    };
    let mut ui = new_ui(None);
    ui.verify_views(false);
    let (mut largest, before) = (0, ui.layer_rewrites().1);
    for t in 0..200 {
        ui.frame(&at(t as f64 * 0.016, vec![]), &mut build);
        largest = largest.max(ui.output().draw[0].instances.len());
    }
    // once compacted the room stays ahead of the views: no compaction per rebuild, and the layer keeps its size
    let compactions = ui.layer_rewrites().1 - before;
    check((1..=2).contains(&compactions), &format!("{compactions} compactions in 20 rebuilds"));
    check(largest < 120, &format!("compaction room accumulated in a rebuilt view's run: {largest} instances"));
    ui.destroy();
}

/// Verify mode across a layer change: a clean view that its parent moves into a raised layer and
/// back is checked against the run it kept in its old layer, not the same range of the new one; a
/// stale fill there is still named.
pub fn window_pxui_view_verify_layer() {
    const BASE: [&str; 4] = ["b0", "b1", "b2", "b3"];
    let (front, fill) = (Cell::new(true), Cell::new(RED));
    let mut build = |ui: &mut Ui| {
        for key in BASE {
            chip(ui, key, BLUE, Flags::NONE);
        }
        let holder = ui.box_("holder", BoxOpts::default().at((0.0, 100.0)).w(Size::Px(200.0)).h(Size::Px(100.0)));
        if front.get() {
            ui.to_front(0, &holder);
        }
        ui.within(&holder, &mut |ui| {
            ui.view("moved", 1, Size::Px(100.0), Size::Px(50.0), &mut |ui| {
                chip(ui, "x", fill.get(), Flags::NONE);
            });
        });
    };
    let mut ui = new_ui(None);
    ui.verify_views(true);
    ui.frame(&at(0.0, vec![]), &mut build);
    ui.frame(&at(0.1, vec![]), &mut build);
    check(ui.output().draw.len() == 2, &format!("{} layers with the holder raised", ui.output().draw.len()));
    for (k, raised) in [false, true, false].into_iter().enumerate() {
        front.set(raised);
        let message = verify_failure(&mut ui, 0.2 + 0.1 * k as f64, &mut build);
        check(message.is_empty(), &format!("a view moved to another layer was reported: {message:?}"));
    }
    check(ui.output().draw.len() == 1, &format!("{} layers with the holder lowered", ui.output().draw.len()));
    (front.set(true), fill.set(BLUE));
    let message = verify_failure(&mut ui, 0.6, &mut build);
    check(message.contains("\"moved\""), &format!("a stale view moved to another layer was not named: {message:?}"));
}

/// Allocations and CPU per frame for ten views of 50 kit widgets: all kept, one rebuilt, all
/// rebuilt (docs/pxui-baseline.md).
pub fn window_pxui_view_costs() {
    const NAMES: [&str; 10] = ["v0", "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8", "v9"];
    let revisions = std::cell::RefCell::new([1u64; 10]);
    let build = |ui: &mut Ui| {
        let revisions = *revisions.borrow();
        for (v, rev) in revisions.iter().enumerate() {
            ui.view(NAMES[v], *rev, Size::Px(300.0), Size::Px(25.0 * 24.0), &mut |ui| {
                for w in 0..25 {
                    ui.label("LABEL", pxui_core::ui::Label::DEFAULT);
                    ui.button(if w % 2 == 0 { "Even" } else { "Odd" });
                }
            });
        }
    };
    let mut ui = new_ui(None);
    let mut t = 0.0;
    let mut measure = |ui: &mut Ui, bump: usize, name: &str, zero: bool| {
        let (n, mut ns) = (100u64, 0u128);
        for _ in 0..10 {
            t += 1.0 / 60.0;
            revisions.borrow_mut().iter_mut().take(bump).for_each(|r| *r += 1);
            ui.frame(&at(t, vec![]), &mut |ui| build(ui));
        }
        mem_sys::local_reset();
        let before = mem_sys::local_alloc_count();
        for _ in 0..n {
            t += 1.0 / 60.0;
            revisions.borrow_mut().iter_mut().take(bump).for_each(|r| *r += 1);
            let start = std::time::Instant::now();
            ui.frame(&at(t, vec![]), &mut |ui| build(ui));
            ns += start.elapsed().as_nanos();
        }
        let allocs = mem_sys::local_alloc_count() - before;
        eprintln!("PXUI_VIEWS {name}: {:.1} us per frame, {:.1} allocs per frame, {} nodes built", ns as f64 / n as f64 / 1e3, allocs as f64 / n as f64, ui.built_nodes());
        if zero {
            check(allocs == 0, &format!("{name}: {allocs} allocations in {n} warm frames"));
        }
    };
    measure(&mut ui, 0, "all kept", true);
    measure(&mut ui, 1, "one of 50 widgets rebuilt", true);
    measure(&mut ui, 10, "all 500 widgets rebuilt", false);
    ui.destroy();
}

/// Allocations over `f`'s frames (the measuring thread's).
fn allocs(f: &mut dyn FnMut()) -> u64 {
    mem_sys::local_reset();
    let before = mem_sys::local_alloc_count();
    f();
    mem_sys::local_alloc_count() - before
}

/// Exact work counters for ten 50-widget views: a kept frame declares and lays out the root and the
/// ten view boxes only, a one-view rebuild adds that view's 75 nodes; the accessibility tree stays
/// empty (nothing built) until asked for, then a one-view rebuild updates it without allocating.
pub fn window_pxui_budget_counts() {
    const NAMES: [&str; 10] = ["v0", "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8", "v9"];
    let rev = Cell::new(1u64);
    let build = |ui: &mut Ui| {
        for (v, name) in NAMES.iter().enumerate() {
            ui.view(name, if v == 0 { rev.get() } else { 1 }, Size::Px(300.0), Size::Px(25.0 * 24.0), &mut |ui| {
                for w in 0..25 {
                    ui.label("LABEL", pxui_core::ui::Label::DEFAULT);
                    ui.button(if w % 2 == 0 { "Even" } else { "Odd" });
                }
            });
        }
    };
    let mut ui = new_ui(None);
    let mut t = 0.0;
    let mut step = |ui: &mut Ui, bump: bool| {
        t += 1.0 / 60.0;
        if bump {
            rev.set(rev.get() + 1);
        }
        ui.frame(&at(t, vec![]), &mut |ui| build(ui));
    };
    for _ in 0..3 {
        step(&mut ui, false);
    }
    step(&mut ui, false);
    check(ui.built_nodes() == 11 && ui.arranged_nodes() == 11 && ui.rebuilt_views() == 0, &format!("kept frame: {} built, {} arranged, {} views", ui.built_nodes(), ui.arranged_nodes(), ui.rebuilt_views()));
    step(&mut ui, true);
    check(ui.built_nodes() == 86 && ui.arranged_nodes() == 86 && ui.rebuilt_views() == 1, &format!("one view: {} built, {} arranged, {} views", ui.built_nodes(), ui.arranged_nodes(), ui.rebuilt_views()));
    check(ui.output().a11y.nodes.is_empty() && ui.output().a11y.revision == 0, "the accessibility tree was built with nobody asking");
    ui.set_a11y(true);
    for _ in 0..3 {
        step(&mut ui, true);
    }
    check(ui.output().a11y.nodes.len() == 500, &format!("{} accessible nodes", ui.output().a11y.nodes.len()));
    let n = allocs(&mut || {
        for _ in 0..20 {
            step(&mut ui, true);
        }
    });
    check(n == 0, &format!("{n} allocations in 20 one-view rebuilds with the tree on"));
    // a one-view rebuild computes that view's 50 nodes; the nine kept views keep their runs
    check(ui.a11y_written() == 50, &format!("a one-view rebuild computed {} accessible nodes", ui.a11y_written()));
    let (nodes, strings, revision) = (ui.output().a11y.nodes.clone(), ui.output().a11y.strings.clone(), ui.output().a11y.revision);
    step(&mut ui, true);
    check(ui.output().a11y.revision == revision, "an unchanged rebuild bumped the revision");
    // the incremental tree is the tree a full walk builds
    ui.set_a11y(false);
    ui.set_a11y(true);
    step(&mut ui, false);
    check(ui.a11y_written() == 500 && ui.output().a11y.nodes == nodes && ui.output().a11y.strings == strings, "a full walk differs from the incremental tree");
    ui.destroy();
}

/// A cached frame costs per view, not per node: twenty kept views of 10 nodes and of 200 nodes each
/// take within 1.5x of each other (best of interleaved rounds, so load hits both alike).
pub fn window_pxui_budget_cached_per_view() {
    let sizes = [10usize, 200];
    let mut uis: Vec<Ui> = sizes.iter().map(|_| new_ui(None)).collect();
    let mut best = [f64::INFINITY; 2];
    let mut t = 0.0;
    for round in 0..8 {
        for (k, ui) in uis.iter_mut().enumerate() {
            let mut ns = 0u128;
            for f in 0..30 {
                t += 1.0 / 60.0;
                let start = std::time::Instant::now();
                ui.frame(&at(t, vec![]), &mut |ui| {
                    for v in 0..20 {
                        ui.view(&format!("v{v}"), 1, Size::Px(300.0), Size::Px(100.0), &mut |ui| {
                            for i in 0..sizes[k] {
                                ui.node_keyed(int_key(7, i as i64), Node::DEFAULT.w(Size::Px(4.0)).h(Size::Px(4.0)).look(Look::NONE.fill(RED)));
                            }
                        });
                    }
                });
                if round > 0 && f > 0 {
                    ns += start.elapsed().as_nanos();
                }
            }
            if round > 0 {
                best[k] = best[k].min(ns as f64 / 29.0);
            }
        }
    }
    for ui in &mut uis {
        ui.destroy();
    }
    eprintln!("PXUI_BUDGET cached frame of 20 views: {:.0} ns at 10 nodes per view, {:.0} ns at 200", best[0], best[1]);
    check(best[1] < best[0] * 1.5, &format!("a cached frame grew with the nodes per view: {:.0} -> {:.0} ns", best[0], best[1]));
}

/// The idle-600 pattern: a list of kept row views, 700 frames with no input (the layouts of their
/// text are never asked for, so age alone would drop them), then every row rebuilds: list-owned
/// layouts survived, so the return allocates nothing.
pub fn window_pxui_list_text_survives_idle() {
    let names: Vec<String> = (0..64).map(|i| format!("Row title number {i}")).collect();
    let rev = Cell::new(1u64);
    let mut ui = new_ui(None);
    let mut list = |ui: &mut Ui| {
        ui.list("rows", 10_000, 24.0, &mut |ui, i| {
            ui.view("row", rev.get(), Size::Grow, Size::Px(24.0), &mut |ui| {
                let row = ui.node("name".into(), Node::DEFAULT.w(Size::Grow).h(Size::Px(24.0)));
                ui.text(&row, &names[i % 64]);
            });
        });
    };
    ui.frame(&at(0.0, vec![move_to((100, 100))]), &mut list);
    for k in 2..4 {
        rev.set(k);
        ui.frame(&at(k as f64, vec![]), &mut list);
    }
    let idle: Vec<_> = (0..700).map(|t| at(10.0 + t as f64, vec![])).collect();
    for f in &idle {
        ui.frame(f, &mut list);
    }
    rev.set(9);
    let after = at(2000.0, vec![]);
    let n = allocs(&mut || { ui.frame(&after, &mut list); });
    check(n == 0, &format!("{n} allocations rebuilding the rows after 700 idle frames"));
    ui.destroy();
}

/// A new view record has room for a list row's nodes and hits: a view that grows from one node to 25 (8 of
/// them clickable) while a sibling shrinks by as much allocates nothing on its warm frames. Without the room
/// its node, kept-node and hit buffers grow as it does.
pub fn window_pxui_new_view_grows_within_its_room() {
    let grow = Cell::new(1usize);
    let mut ui = new_ui(None);
    let frame = |ui: &mut Ui, warm: bool| {
        let n = grow.get();
        // the first frame declares as many nodes and hits as any later one, outside the views
        if !warm {
            for i in 0..33 {
                ui.node_keyed(int_key(9, i), Node::DEFAULT.flags(Flags::CLICKABLE).w(Size::Px(4.0)).h(Size::Px(4.0)));
            }
        }
        for (name, count, first) in [("grow", n, 0), ("spare", 33 - n, n)] {
            ui.view(name, n as u64, Size::Px(300.0), Size::Px(200.0), &mut |ui| {
                for i in first..first + count {
                    let flags = if i < 8 || i >= 25 { Flags::CLICKABLE } else { Flags::NONE };
                    ui.node_keyed(int_key(7, i as i64), Node::DEFAULT.flags(flags).w(Size::Px(4.0)).h(Size::Px(4.0)));
                }
            });
        }
    };
    let mut t = 0.0;
    for warm in [false, true, true] {
        t += 1.0 / 60.0;
        ui.frame(&at(t, vec![]), &mut |ui| frame(ui, warm));
    }
    let n = allocs(&mut || {
        while grow.get() < 25 {
            grow.set(grow.get() + 1);
            t += 1.0 / 60.0;
            ui.frame(&at(t, vec![]), &mut |ui| frame(ui, true));
        }
    });
    check(ui.rebuilt_views() == 2, &format!("{} views rebuilt on the last frame", ui.rebuilt_views()));
    check(n == 0, &format!("{n} allocations while a new view grew to 25 nodes and 8 hits"));
    ui.destroy();
}

/// A kept view's text outside any list: 700 frames with no input keep the view (its text is never
/// laid out, so age alone would drop it), then the view rebuilds with the same text: no re-shape.
pub fn window_pxui_kept_view_text_survives_idle() {
    let rev = Cell::new(1u64);
    let mut ui = new_ui(None);
    let mut header = |ui: &mut Ui| {
        ui.view("header", rev.get(), Size::Grow, Size::Px(48.0), &mut |ui| {
            let title = ui.node("title".into(), Node::DEFAULT.w(Size::Grow).h(Size::Px(24.0)));
            ui.text(&title, "Songs");
            let hint = ui.node("hint".into(), Node::DEFAULT.w(Size::Grow).h(Size::Px(24.0)));
            ui.text(&hint, "Click a header to sort. Double-click or Return plays.");
        });
    };
    ui.frame(&at(0.0, vec![move_to((100, 100))]), &mut header);
    for k in 2..4 {
        rev.set(k);
        ui.frame(&at(k as f64, vec![]), &mut header);
    }
    let idle: Vec<_> = (0..700).map(|t| at(10.0 + t as f64, vec![])).collect();
    for f in &idle {
        ui.frame(f, &mut header);
    }
    rev.set(9);
    let after = at(2000.0, vec![]);
    let layouts = ui.text_mut().layout_count();
    let n = allocs(&mut || { ui.frame(&after, &mut header); });
    check(n == 0, &format!("{n} allocations rebuilding the header after 700 idle frames"));
    check(ui.text_mut().layout_count() == layouts, "the header's text was re-shaped after 700 idle frames");
    ui.destroy();
}

/// The worst frame of the fling workload (`window_pxui_fling_shaping_stays_under_its_ceiling`): Rust allocations on the UI
/// thread while a fling brings rows the text cache has never seen into view faster than the prepare queue's worker delivers
/// them. The one named exception of the warm-allocation contract (first-time shaping, pxui/AGENTS.md "Contracts"). This
/// ceiling may only go down: lower it when a change measures less, never raise it.
const FLING_CEILING: u64 = 116;

/// A list of `count` song rows: each a view row holding a plain text node and a one-line ellipsized label, every row's text
/// its own (no row repeats another's), so a row the list has not shown before is first-time shaping.
fn song_rows(ui: &mut Ui, names: &[String], second: &[String], measured: bool) {
    let mut row = |ui: &mut Ui, i: usize| {
        ui.view("row", 1, Size::Grow, Size::Px(24.0), &mut |ui| {
            let title = ui.node("title".into(), Node::DEFAULT.w(Size::Grow).h(Size::Px(24.0)));
            ui.text(&title, &names[i]);
            ui.label(&second[i], Label::BODY.line_height(20.0).lines(1).ellipsis().w(Size::Px(120.0)));
        });
    };
    if measured {
        ui.list_measured("songs", names.len(), 24.0, &mut row);
    } else {
        ui.list("songs", names.len(), 24.0, &mut row);
    }
}

/// Allocations and UI-thread layouts of each of `frames`, waiting between frames until the prepare queue's worker has
/// delivered what the frame requested (a deterministic worker: the measurement does not depend on its speed).
fn queue_frames(ui: &mut Ui, frames: &[input::frame::Frame], list: &mut dyn FnMut(&mut Ui)) -> Vec<(u64, u64)> {
    let mut out = Vec::with_capacity(frames.len());
    for f in frames {
        let layouts = ui.text_mut().layout_count();
        let n = allocs(&mut || { ui.frame(f, &mut *list); });
        out.push((n, ui.text_mut().layout_count() - layouts));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while !ui.text_mut().queue_delivered() {
            check(std::time::Instant::now() < deadline, "the prepare queue delivered nothing for a second");
            std::thread::yield_now();
        }
    }
    out
}

fn song_names(count: usize) -> (Vec<String>, Vec<String>) {
    ((0..count).map(|i| format!("Song title {i:05}")).collect(), (0..count).map(|i| format!("{:02}:{:02} Artist {i:05}", i % 60, i % 7)).collect())
}

/// The prepare queue keeps a scrolling list's first-time shaping off the UI thread: scrolled two rows a frame down through
/// rows it has never shown, then back up through other unseen rows, every row a view whose text is new, a frame shapes and
/// allocates nothing; the rows past the viewport were requested (deferred), prepared on the worker and installed between
/// frames. Without a worker attached the same scroll shapes every row on the UI thread. A measured list queues its rows below the
/// viewport: scrolling it down shapes nothing either.
pub fn window_pxui_prepare_queue_scrolls_unseen_rows_without_shaping() {
    let (names, second) = song_names(10_000);
    // one worker for every Ui of the case, as a host with several windows attaches one
    let worker = pxui_core::text::Worker::start().unwrap();
    for queued in [true, false] {
        let mut ui = new_ui(None);
        let mut list = |ui: &mut Ui| song_rows(ui, &names, &second, false);
        if queued {
            ui.set_text_worker(Some(&worker));
        }
        // the warm-up scrolls the top rows as the measurement scrolls: every character the rows use is rasterized and the
        // retained storage (slots, view records, text buffers) reaches the size this scroll keeps
        let warm: Vec<_> = (0..60).map(|t| at(t as f64 * 0.05, vec![Event::Scrolled((0.0, -2.0))])).collect();
        queue_frames(&mut ui, &warm, &mut list);
        // far down (a jump, not measured), then two rows a frame down and later up, through rows never shown
        queue_frames(&mut ui, &[at(3.0, vec![Event::Scrolled((0.0, -4000.0))]), at(3.1, vec![])], &mut list);
        let down: Vec<_> = (0..60).map(|t| at(4.0 + t as f64 * 0.1, vec![Event::Scrolled((0.0, -2.0))])).collect();
        queue_frames(&mut ui, &down[..2], &mut list);
        let measured_down = queue_frames(&mut ui, &down[2..], &mut list);
        queue_frames(&mut ui, &[at(20.0, vec![Event::Scrolled((0.0, -2000.0))]), at(20.1, vec![])], &mut list);
        let up: Vec<_> = (0..60).map(|t| at(21.0 + t as f64 * 0.1, vec![Event::Scrolled((0.0, 2.0))])).collect();
        queue_frames(&mut ui, &up[..2], &mut list);
        let measured_up = queue_frames(&mut ui, &up[2..], &mut list);
        let (allocated, shaped) = measured_down.iter().chain(&measured_up).fold((0, 0), |(a, l), &(n, k)| (a + n, l + k));
        let (deferred, installed) = ui.text_mut().queue_counts();
        eprintln!("prepare queue {queued}: {allocated} allocations, {shaped} UI-thread layouts over 116 scrolled frames; {deferred} deferred, {installed} installed");
        if queued {
            check(allocated == 0 && shaped == 0, &format!("{allocated} allocations and {shaped} layouts scrolling unseen rows with the prepare queue"));
            check(installed > 0, "nothing was prepared on the worker");
        } else {
            check(shaped > 0 && allocated > 0, "the scroll without the queue did not shape: the workload does not exercise the queue");
        }
        ui.destroy();
    }
    // a measured list queues the rows below the viewport: scrolling down through unseen rows shapes nothing
    let mut ui = new_ui(None);
    let mut list = |ui: &mut Ui| song_rows(ui, &names, &second, true);
    ui.set_text_worker(Some(&worker));
    let warm: Vec<_> = (0..60).map(|t| at(t as f64 * 0.05, vec![Event::Scrolled((0.0, -2.0))])).collect();
    queue_frames(&mut ui, &warm, &mut list);
    queue_frames(&mut ui, &[at(3.0, vec![Event::Scrolled((0.0, -4000.0))]), at(3.1, vec![])], &mut list);
    let down: Vec<_> = (0..60).map(|t| at(4.0 + t as f64 * 0.1, vec![Event::Scrolled((0.0, -2.0))])).collect();
    queue_frames(&mut ui, &down[..2], &mut list);
    let (allocated, shaped) = queue_frames(&mut ui, &down[2..], &mut list).iter().fold((0, 0), |(a, l), &(n, k)| (a + n, l + k));
    eprintln!("measured list, prepare queue: {allocated} allocations, {shaped} UI-thread layouts over 58 scrolled frames");
    check(allocated == 0 && shaped == 0, &format!("{allocated} allocations and {shaped} layouts scrolling a measured list's unseen rows"));
    ui.destroy();
}

/// The fling workload and its ceiling: six rows a frame (a fast fling: rows cross the four-row overscan in under a frame)
/// through rows never shown. Rows the worker delivered in time draw from the cache; the rest shape on the UI thread, the
/// named exception. A frame allocates only when it shaped on the UI thread, and the worst frame stays under
/// `FLING_CEILING`; the queue shapes fewer rows on the UI thread than no queue.
pub fn window_pxui_fling_shaping_stays_under_its_ceiling() {
    let (names, second) = song_names(10_000);
    let worker = pxui_core::text::Worker::start().unwrap();
    let mut worst = [0u64; 2];
    let mut layouts = [0u64; 2];
    for (k, queued) in [true, false].into_iter().enumerate() {
        let mut ui = new_ui(None);
        let mut list = |ui: &mut Ui| song_rows(ui, &names, &second, false);
        if queued {
            ui.set_text_worker(Some(&worker));
        }
        // the warm-up flings the top rows as the measurement does (glyphs rasterized, retained storage at its size)
        let warm: Vec<_> = (0..60).map(|t| at(t as f64 * 0.05, vec![Event::Scrolled((0.0, -6.0))])).collect();
        queue_frames(&mut ui, &warm, &mut list);
        queue_frames(&mut ui, &[at(3.0, vec![Event::Scrolled((0.0, -4000.0))]), at(3.1, vec![])], &mut list);
        let fling: Vec<_> = (0..62).map(|t| at(4.0 + t as f64 * 0.1, vec![Event::Scrolled((0.0, -6.0))])).collect();
        queue_frames(&mut ui, &fling[..2], &mut list);
        let measured = queue_frames(&mut ui, &fling[2..], &mut list);
        for &(n, shaped) in &measured {
            check(n == 0 || shaped > 0, &format!("a fling frame allocated {n} times without shaping: only first-time shaping may allocate"));
        }
        worst[k] = measured.iter().map(|m| m.0).max().unwrap_or(0);
        layouts[k] = measured.iter().map(|m| m.1).sum();
        let total: u64 = measured.iter().map(|m| m.0).sum();
        eprintln!("fling, prepare queue {queued}: worst frame {} allocations, {total} over 60 frames, {} UI-thread layouts", worst[k], layouts[k]);
        ui.destroy();
    }
    check(worst[0] <= FLING_CEILING, &format!("the fling's worst frame allocated {}, over its ceiling {FLING_CEILING}", worst[0]));
    check(layouts[0] < layouts[1], &format!("the prepare queue shaped {} layouts on the UI thread, no fewer than {} without it", layouts[0], layouts[1]));
}

/// A label drawn at a width of at least its natural one (a numeric cell wider than its column's measure) shows the layout
/// `prewarm_text(text, style, 0)` prepared: no layout on the UI thread, no allocation; at a narrower width it lays out its own.
pub fn window_pxui_prewarmed_natural_text_is_the_layout_drawn_at_a_wider_width() {
    let worker = pxui_core::text::Worker::start().unwrap();
    let mut ui = new_ui(None);
    ui.set_text_worker(Some(&worker));
    let label = Label::BODY.line_height(20.0).lines(1).ellipsis();
    let resolved = ui.resolve(label.fields()).expect("the label resolves");
    // the engine's own style of the label (`pxui_abi` reads a resolved `TextStyle` the same way)
    let style = pxui_text::Style {
        font: resolved.font, size: resolved.size_points, weight: resolved.weight, italic: resolved.italic != 0, line_height: resolved.line_height,
        wrap: match resolved.wrap { 0 => pxui_text::Wrap::None, 1 => pxui_text::Wrap::Word, 2 => pxui_text::Wrap::Glyph, _ => pxui_text::Wrap::WordSpaces },
        align: pxui_text::Align::Left, max_lines: resolved.max_lines, ellipsis: pxui_text::Ellipsis::End, tracking: resolved.tracking, features: resolved.features,
    };
    check(resolved.align == 0 && resolved.ellipsis == 1, "the label is left-aligned and ellipsized at its end");
    let texts: Vec<String> = (0..40).map(|i| format!("{:05} plays", 12345 + i * 1111 % 80000)).collect();
    let shown = Cell::new(0usize);
    let wide = Cell::new(200.0f32);
    let mut draw = |ui: &mut Ui| {
        ui.label(&texts[shown.get()], label.w(Size::Px(wide.get())));
    };
    // every digit rasterized, then prepared texts through the queue until its retained storage is at size
    queue_frames(&mut ui, &[at(0.0, vec![]), at(0.1, vec![])], &mut draw);
    let natural = ui.text_mut().layout(pxui_text::content_key(&style, 0.0, &texts[0], &[]), &texts[0], &style, 0.0).w;
    check(natural > 0.0 && natural < 100.0, "the sample text fits its column");
    let mut t = 0.2;
    let mut show = |ui: &mut Ui, draw: &mut dyn FnMut(&mut Ui), i: usize, width: f32| {
        check(ui.prewarm_text(&texts[i], &style, 0.0), "prewarm refused");
        t += 0.1;
        queue_frames(ui, &[at(t, vec![])], draw);
        shown.set(i);
        wide.set(width);
        t += 0.1;
        queue_frames(ui, &[at(t, vec![])], draw)[0]
    };
    for i in 1..24 {
        show(&mut ui, &mut draw, i, 200.0);
    }
    // wider than the natural width, and exactly the natural width: the prepared layout, nothing shaped or allocated
    for (i, width) in [(24, 200.0), (25, natural.ceil() + 0.5), (26, 1000.0)] {
        let (n, layouts) = show(&mut ui, &mut draw, i, width);
        check(layouts == 0 && n == 0, &format!("{} at width {width}: {layouts} UI-thread layouts and {n} allocations after prewarm", texts[i]));
    }
    // narrower than the natural width: its own cut layout, shaped on the UI thread
    let (_, layouts) = show(&mut ui, &mut draw, 27, natural * 0.5);
    check(layouts > 0, "a width under the natural one must lay out its own layout");
    ui.destroy();
}

/// No allocation once warm: a virtual list scrolled a row per frame, and a hover entering and leaving
/// a kept view's button with the theme's transition running.
pub fn window_pxui_budget_zero_alloc() {
    const WORDS: [&str; 4] = ["alpha", "beta", "gamma", "delta"];
    let mut ui = new_ui(None);
    ui.set_a11y(true);
    let mut list = |ui: &mut Ui| {
        ui.list("rows", 10_000, 24.0, &mut |ui, i| {
            ui.view("row", 0, Size::Grow, Size::Px(24.0), &mut |ui| {
                let row = ui.node("name".into(), Node::DEFAULT.w(Size::Grow).h(Size::Px(24.0)).role(pxui_core::a11y::Role::Label));
                ui.text(&row, WORDS[i % 4]);
                ui.a11y_name(&row, WORDS[i % 4]);
                ui.a11y_value(&row, WORDS[(i + 1) % 4], (0, 0));
            });
        });
    };
    ui.frame(&at(0.0, vec![move_to((100, 100))]), &mut list);
    // the frames are made up front: an event list allocates
    let frames: Vec<_> = (1..=600).map(|t| at(t as f64, vec![Event::Scrolled((0.0, -1.0))])).collect();
    for f in &frames[..30] {
        ui.frame(f, &mut list);
    }
    let mut largest = 0;
    let n = allocs(&mut || {
        for f in &frames[30..] {
            ui.frame(f, &mut list);
            largest = largest.max(ui.output().draw[0].instances.len());
        }
    });
    check(n == 0, &format!("{n} allocations scrolling a list 570 rows"));
    check(largest < 1024, &format!("retired row slots accumulated: {largest} instances"));
    ui.destroy();

    let mut ui = new_ui(None);
    let mut theme = Theme::default();
    theme.tokens.motion.normal = 0.2;
    ui.set_theme(theme);
    let mut build = |ui: &mut Ui| {
        ui.view("v", 1, Size::Px(200.0), Size::Px(100.0), &mut |ui| {
            ui.panel("p", &pxui_core::ui::Overlay::DEFAULT.name("p"), &mut |ui| {
                ui.button("Go");
            });
        });
    };
    // in and out of the button (its ground is (17, 14) to (49, 34) in the panel at (12, 12)), then
    // eight steps of the transition, twice
    let mut frames = Vec::new();
    for (k, p) in [(24, 24), (300, 230), (24, 24), (300, 230)].into_iter().enumerate() {
        frames.push(at(k as f64, vec![move_to(p)]));
        frames.extend((1..=8).map(|i| at(k as f64 + 0.04 * i as f64, vec![])));
    }
    for f in &frames[..18] {
        ui.frame(f, &mut build);
    }
    let n = allocs(&mut || {
        for f in &frames[18..] {
            ui.frame(f, &mut build);
        }
    });
    check(n == 0, &format!("{n} allocations hovering in and out with a transition"));
    ui.destroy();
}

/// Content-sized views: a `Fit` view kept is laid out at the size its last build measured; one that
/// rebuilds and grows or shrinks re-lays out its column in that frame, and the kept views after it
/// move through their table entries (not rebuilt, no instance written, hit where they now show).
/// A kept view whose own size changes (its parent shrank) is rebuilt within the frame: the frame's
/// output already fills the new size (the frame settles; no stale frame).
pub fn window_pxui_view_resize_settles() {
    let mut ui = new_ui(None);
    let (parent_h, runs) = (Cell::new(100.0f32), Cell::new(0));
    let ink = Rgba::rgb(1, 2, 3);
    let mut build = |ui: &mut Ui| {
        let col = ui.node("col".into(), Node::DEFAULT.w(Size::Px(200.0)).h(Size::Px(parent_h.get())));
        ui.within(&col, &mut |ui| {
            ui.view("v", 1, Size::Grow, Size::Grow, &mut |ui| {
                runs.set(runs.get() + 1);
                let h = ui.node("fill".into(), Node::DEFAULT.w(Size::Grow).h(Size::Grow));
                ui.canvas(&h, move |paint, r| paint.fill(r.x, r.y, r.w, r.h, ink));
            });
        });
    };
    ui.frame(&at(0.0, vec![]), &mut build);
    ui.frame(&at(0.1, vec![]), &mut build);
    let filled = |ui: &Ui| ui.output().draw[0].instances.iter().find(|i| i.color == [1, 2, 3, 255]).map(|i| i.rect[3] - i.rect[1]);
    check(filled(&ui) == Some(100.0) && runs.get() == 1, &format!("first build {:?}", filled(&ui)));
    parent_h.set(60.0);
    ui.frame(&at(0.2, vec![]), &mut build);
    check(filled(&ui) == Some(60.0) && runs.get() == 2, &format!("the resized view showed {:?} after {} builds", filled(&ui), runs.get()));
    check(!ui.wants_frame(&[], 10.0), "a settled frame still asks for another");
    ui.destroy();
}

pub fn window_pxui_view_fit() {
    let mut ui = new_ui(None);
    let (runs, rows_a) = ([Cell::new(0), Cell::new(0), Cell::new(0)], Cell::new(1usize));
    let (views, last) = (Cell::new([None; 3]), Cell::new(None));
    let mut build = |ui: &mut Ui| {
        let col = ui.node("col".into(), Node::DEFAULT.w(Size::Px(200.0)).h(Size::Px(400.0)));
        ui.within(&col, &mut |ui| {
            let mut vs = [None; 3];
            for (i, name) in ["a", "b", "c"].iter().enumerate() {
                let (count, rev) = if i == 0 { (rows_a.get(), rows_a.get() as u64) } else { (2, 1) };
                vs[i] = Some(ui.view(name, rev, Size::Grow, Size::Fit, &mut |ui| {
                    runs[i].set(runs[i].get() + 1);
                    for r in 0..count {
                        let h = chip(ui, ["r0", "r1", "r2"][r], RED, Flags::CLICKABLE);
                        if i == 2 {
                            last.set(Some(h));
                        }
                    }
                }));
            }
            views.set(vs);
        });
    };
    ui.frame(&at(0.0, vec![move_to((300, 300))]), &mut build);
    ui.frame(&at(0.1, vec![]), &mut build);
    let rect = |ui: &Ui, i: usize| ui.rect(&views.get()[i].unwrap());
    check(runs.iter().all(|r| r.get() == 1) && ui.rebuilt_views() == 0, &format!("a clean Fit view rebuilt {:?} {} {:?}", runs.iter().map(|r| r.get()).collect::<Vec<_>>(), ui.rebuilt_views(), rect(&ui, 0)));
    check(rect(&ui, 0).h == 20.0 && rect(&ui, 1) == Rect::new(0.0, 20.0, 200.0, 40.0) && rect(&ui, 2).y == 60.0, &format!("kept Fit views laid out at {:?} {:?} {:?}", rect(&ui, 0), rect(&ui, 1), rect(&ui, 2)));
    // the first growth past the slack after a's run compacts the layer once: grow and shrink
    rows_a.set(3);
    ui.frame(&at(0.15, vec![]), &mut build);
    rows_a.set(1);
    ui.frame(&at(0.17, vec![]), &mut build);
    for i in 0..ui.output().draw.len() {
        ui.drawn(i);
    }
    // a grows by two rows: b and c move down 40 in the same frame, kept
    rows_a.set(3);
    ui.frame(&at(0.2, vec![]), &mut build);
    check(runs.iter().map(|r| r.get()).collect::<Vec<_>>() == [4, 1, 1] && ui.rebuilt_views() == 1, "growing one view rebuilt another");
    check(rect(&ui, 0).h == 60.0 && rect(&ui, 1).y == 60.0 && rect(&ui, 2).y == 100.0, &format!("after growing {:?} {:?} {:?}", rect(&ui, 0), rect(&ui, 1), rect(&ui, 2)));
    check(ui.built_nodes() == 1 + 1 + 3 + 3, &format!("growing declared {} nodes", ui.built_nodes()));
    let written: usize = ui.output().draw.iter().flat_map(|l| l.dirty.iter()).map(|(a, b)| (b - a) as usize).sum();
    // a's rows only (fresh pieces end against the kept run after them, so a's start moves with its growth): b and c are not rewritten
    check(written <= 3, &format!("growing wrote {written} instances (a's three rows at most)"));
    check(ui.output().views.iter().any(|(_, e)| e.offset == [0.0, 40.0]), "the moved views ride no table offset");
    for i in 0..ui.output().draw.len() {
        ui.drawn(i);
    }
    // c's last row hits where it shows now; shrinking back moves it home
    let c = last.get().unwrap();
    check(ui.rect(&c) == Rect::new(0.0, 120.0, 40.0, 20.0), &format!("c's row at {:?}", ui.rect(&c)));
    ui.frame(&at(0.3, vec![move_to((10, 130))]), &mut build);
    check(ui.signal_owned(&c).hovered, "the moved view's row missed the pointer");
    ui.frame(&at(0.4, vec![move_to((10, 300))]), &mut build);
    rows_a.set(1);
    ui.frame(&at(0.5, vec![]), &mut build);
    check(rect(&ui, 2).y == 60.0 && ui.rebuilt_views() == 1, &format!("after shrinking c at {:?}, {} views rebuilt", rect(&ui, 2), ui.rebuilt_views()));
    // warm: growing and shrinking allocate nothing
    let mut t = 0.6;
    let n = allocs(&mut || {
        for k in 0..20 {
            rows_a.set(1 + 2 * (k % 2));
            t += 0.1;
            ui.frame(&at(t, vec![]), &mut build);
        }
    });
    check(n == 0, &format!("{n} allocations growing and shrinking a Fit view"));
    ui.destroy();
}

/// A `kept_drag` slider field in a kept view: its drag dirties no view (`DRAG_FREE`), the owner reads
/// it after the frame (`field_drag`) and patches the value and line (`patch_field`); what shows then is
/// what the field's own build of that drag shows (its release, at the same point), its trail (a
/// unit) beside it.
pub fn window_pxui_field_kept_drag() {
    use std::cell::RefCell;
    let mut ui = new_ui(None);
    let value = RefCell::new("0.25".to_owned());
    let (builds, key) = (Cell::new(0), Cell::new(0));
    let opts = |current: &str| ValueFieldOpts {
        fraction: current.parse::<f32>().ok(),
        kept_drag: true,
        trail: Some(("mm".to_owned(), pxui_core::geom::Rgba::WHITE)),
        ..ValueFieldOpts::default()
    };
    let mut build = |ui: &mut Ui| {
        let current = value.borrow().clone();
        ui.view("v", 1, Size::Px(200.0), Size::Px(40.0), &mut |ui| {
            builds.set(builds.get() + 1);
            let wrap = ui.box_("wrap", BoxOpts::default().w(Size::Px(200.0)).h(Size::Px(40.0)));
            key.set(pxui_core::ui_core::key_of(wrap.box_key, "field"));
            ui.within(&wrap, &mut |ui| if let Some(text) = ui.value_field((20.0, 10.0), 100.0, 20.0, &opts(&current), &|_| true, Some(&|f: f32| cfmt::g(f64::from(f), 3)), None, "field", &current).0 { *value.borrow_mut() = text.to_owned(); });
        });
    };
    let at = |x: i64| ((x, 20), (x as f64, 20.0));
    let mut t = 0.0;
    let mut go = |ui: &mut Ui, events: Vec<Event>, pointer: (f64, f64), held: bool| {
        t += 0.1;
        let mut fr = frame_at(vec![], 2.0, t, events);
        fr.input.pointer = pointer;
        fr.input.buttons = if held { vec![input::MouseButton::LeftButton] } else { vec![] };
        ui.frame(&fr, &mut build);
    };
    go(&mut ui, vec![], (0.0, 0.0), false);
    go(&mut ui, vec![], (0.0, 0.0), false);
    let (p, q) = at(40);
    go(&mut ui, vec![move_to(p), press(p)], q, true);
    let pressed = value.borrow().clone();
    let (p, q) = at(70);
    let before = builds.get();
    go(&mut ui, vec![move_to(p)], q, true);
    check(ui.rebuilt_views() == 0 && builds.get() == before, "a drag of a kept_drag field rebuilt its view");
    let field = ui.box_of_key(key.get()).expect("the field");
    let (fraction, dx) = ui.field_drag(&field).expect("the field's drag");
    check(dx == 30.0, &format!("the drag's points from the press: {dx}"));
    let text = cfmt::g(f64::from(fraction), 3);
    check(text != pressed && ui.patch_field(&field, &opts(&pressed), &text), "patch_field refused");
    let shown = |ui: &Ui| {
        let mut v: Vec<_> = ui.output().draw[0].instances.iter().filter(|i| **i != pxui_core::draw::Instance::DEFAULT).map(|i| (i.kind, i.rect.map(f32::to_bits), i.color)).collect();
        v.sort();
        v
    };
    let patched = shown(&ui);
    // the release at the same point: the field builds what it shows
    go(&mut ui, vec![release(p)], q, false);
    check(builds.get() == before + 1 && *value.borrow() == text, &format!("the release built {} times, value {}", builds.get() - before, value.borrow()));
    check(shown(&ui) == patched, "the patched field differs from its build");
    ui.destroy();
}

/// Reusing an empty buffer never gives its donor another node's previous text or accessibility value.
pub fn window_pxui_empty_buffers_preserve_names_and_values() {
    const LONG: &str = "a reusable buffer large enough for a row";
    let mut ui = new_ui(None);
    ui.set_a11y(true);
    let reuse = Cell::new(false);
    let mut build = |ui: &mut Ui| {
        let spare = ui.node("spare".into(), Node::DEFAULT.role(pxui_core::a11y::Role::Label).h(Size::Px(24.0)));
        let target = ui.node("target".into(), Node::DEFAULT.role(pxui_core::a11y::Role::Label).h(Size::Px(24.0)));
        if !reuse.get() {
            ui.text(&spare, LONG);
            ui.a11y_name(&spare, LONG);
            ui.a11y_value(&spare, LONG, (0, 0));
        } else {
            ui.text(&target, "a");
            ui.a11y_name(&target, "a");
            ui.a11y_value(&target, "a", (0, 0));
            ui.text(&target, LONG);
            ui.a11y_name(&target, LONG);
            ui.a11y_value(&target, LONG, (1, 2));
        }
    };
    let frame = at(0.0, vec![]);
    ui.frame(&frame, &mut build);
    reuse.set(true);
    ui.frame(&frame, &mut build);
    let tree = ui.output().a11y;
    let labels: Vec<_> = tree.nodes.iter().filter(|node| node.role == pxui_core::a11y::Role::Label).collect();
    check(labels.len() == 2 && tree.str(labels[0].name).is_empty() && tree.value_text(labels[0]).is_empty(), "a donor displayed stale contents");
    check(tree.str(labels[1].name) == LONG && tree.value_text(labels[1]) == LONG, "the destination lost its name or value");
    let count = allocs(&mut || { ui.frame(&frame, &mut build); });
    check(count == 0, &format!("reusing text buffers allocated {count} times"));
    ui.destroy();
}

pub fn window_pxui_more_kept_views_reuse_compaction_scratch() {
    let mut ui = new_ui(None);
    let mut phase = 0u64;
    let labels = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"];
    for frame in 0..30 {
        if frame == 10 { mem_sys::local_reset(); }
        ui.frame(&at(frame as f64 / 60.0, vec![]), &mut |ui| {
            ui.view("outer", frame, Size::Px(200.0), Size::Px(200.0), &mut |ui| {
                for (i, label) in labels.iter().enumerate() {
                    ui.view(label, if i < 3 { 1 } else { phase }, Size::Px(20.0), Size::Px(10.0), &mut |ui| {
                        ui.node("tile".into(), Node::DEFAULT.w(Size::Grow).h(Size::Grow).look(Look::NONE.fill(Rgba::rgb(9, 9, 9))));
                    });
                }
            });
        });
        if frame >= 10 { assert_eq!(mem_sys::local_alloc_count(), 0, "frame {frame} allocated"); }
        phase = (frame + 1).min(10);
        assert_eq!(ui.output().draw[0].instances.iter().filter(|i| i.color[0] == 9).count(), 12);
    }
    ui.destroy();
}

/// A newcomer view without a label match must not take the record that holds another label's
/// warmed node buffers (a relayout away and back, an app's compact/wide layouts), or that label regrows them.
pub fn window_pxui_view_newcomer_keeps_warm_buffers() {
    let names: Vec<String> = (0..300).map(|i| format!("n{i}")).collect();
    for big_first in [true, false] {
        let mut ui = new_ui(None);
        let mut run = |frame: usize, labels: &[(&str, usize)]| {
            ui.frame(&at(frame as f64 / 60.0, vec![]), &mut |ui| {
                for &(label, n) in labels {
                    ui.view(label, 1, Size::Px(100.0), Size::Px(50.0), &mut |ui| {
                        for name in &names[..n] { ui.node_ref(name, &Node::DEFAULT.w(Size::Px(4.0)).h(Size::Px(4.0))); }
                    });
                }
            });
        };
        let both = if big_first { [("big", 300), ("small", 2)] } else { [("small", 2), ("big", 300)] };
        run(0, &both);
        run(1, &both);
        run(2, &[("other", 2)]);
        run(3, &[("other", 2)]);
        run(4, &[("another", 2)]);
        mem_sys::local_reset();
        run(5, &both);
        assert_eq!(mem_sys::local_alloc_count(), 0, "big_first={big_first}: the returning view regrew its nodes");
        ui.destroy();
    }
}

/// Covering one transformed clip must not suppress damage outside that clip or instance uploads.
pub fn window_pxui_damage_containment_preserves_other_views() {
    let mut ui = new_ui(None);
    let (view, revision, fill) = (Cell::new(None), Cell::new(0), Cell::new(RED));
    let mut build = |ui: &mut Ui| {
        view.set(Some(ui.view("clipped", revision.get(), Size::Px(100.0), Size::Px(60.0), &mut |ui| {
            ui.node("background".into(), Node::DEFAULT.w(Size::Px(100.0)).h(Size::Px(60.0)).at(0.0, 0.0).look(Look::NONE.fill(fill.get())));
            ui.view("nested", revision.get(), Size::Px(100.0), Size::Px(60.0), &mut |ui| {
                ui.node("inside".into(), Node::DEFAULT.w(Size::Px(20.0)).h(Size::Px(10.0)).at(20.0, 20.0).look(Look::NONE.fill(fill.get())));
            });
        })));
        ui.node("outside".into(), Node::DEFAULT.w(Size::Px(20.0)).h(Size::Px(20.0)).at(150.0, 80.0).look(Look::NONE.fill(fill.get())));
    };
    ui.frame(&at(0.0, vec![]), &mut build);
    let view = view.get().unwrap();
    ui.view_clip(&view, Some(Rect::new(10.0, 10.0, 80.0, 40.0)));
    ui.view_transform(&view, 1.5, [1.0, 0.0, 0.0, 1.0], (0.0, 0.0));
    for tick in 1..24 {
        if tick == 4 { mem_sys::local_reset(); }
        revision.set(tick);
        let color = if tick % 2 == 0 { RED } else { BLUE };
        fill.set(color);
        ui.frame(&at(tick as f64 / 60.0, vec![]), &mut build);
        if tick > 1 { assert_eq!(ui.output().damage.bounds(), Some(Rect::new(10.0, 10.0, 160.0, 90.0))); }
        let layer = &ui.output().draw[0];
        let mut changed = 0;
        for (index, instance) in layer.instances.iter().enumerate() {
            if instance.color == [color.r, color.g, color.b, color.a] {
                changed += 1;
                assert!(layer.dirty.iter().any(|&(a, b)| a as usize <= index && index < b as usize), "damage containment must still upload every changed instance");
            }
        }
        assert_eq!(changed, 3);
        if tick >= 4 { assert_eq!(mem_sys::local_alloc_count(), 0); }
        for layer in 0..ui.output().draw.len() { ui.drawn(layer); }
    }
    ui.destroy();
}

/// Newly visible rows grow the fresh gap before a retained footer; storage stays tied to live rows.
pub fn window_pxui_streaming_rows_reuse_retained_gaps() {
    const ROWS: usize = 20;
    const TILES: usize = 64;
    for (step, fresh) in [(3, false), (19, false), (0, true)] {
        let mut ui = new_ui(None);
        #[cfg(target_os = "macos")]
        ui.set_cpu_clock(Some(|| mem_sys::profile::thread_cpu_time_ns().ok()));
        let mut frame = at(0.0, vec![]);
        frame.input.size = (800, 600);
        let mut compactions = 0;
        let mut paint_ns = 0;
        let mut largest = 0;
        let mut previous = Vec::with_capacity(2 * (ROWS * TILES + 1 + (ROWS + 1) * 32));
        for tick in 0..130 {
            let first = tick * step;
            frame.input.t = tick as f64 / 60.0;
            let before = ui.layer_rewrites().1;
            mem_sys::local_reset();
            let mut ends = [None; 2];
            ui.frame(&frame, &mut |ui| {
                let viewport = ui.node("viewport".into(), Node::DEFAULT.flags(Flags::CLIP).w(Size::Px(600.0)).h(Size::Px(400.0)));
                ui.within(&viewport, &mut |ui| {
                    for row in first..first + ROWS {
                        let holder = ui.node_keyed(int_key(viewport.box_key, row as i64), Node::DEFAULT.w(Size::Px(600.0)).h(Size::Px(20.0)).at(0.0, (row - first) as f32 * 20.0));
                        ui.within(&holder, &mut |ui| {
                            let view = ui.view("row", if fresh { tick as u64 } else { 1 }, Size::Grow, Size::Grow, &mut |ui| {
                                let node = ui.node("tiles".into(), Node::DEFAULT.w(Size::Grow).h(Size::Grow));
                                ui.canvas(&node, move |paint, rect| {
                                    for tile in 0..TILES { paint.fill(rect.x + tile as f32 * 8.0, rect.y, 8.0, 20.0, Rgba::rgb(20 + (row % 200) as u8, 10, 10)); }
                                });
                            });
                            if row == first { ends[0] = Some(view); }
                            if row == first + ROWS - 1 { ends[1] = Some(view); }
                        });
                    }
                });
                ui.view("footer", 1, Size::Px(600.0), Size::Px(20.0), &mut |ui| { chip(ui, "control", BLUE, Flags::NONE); });
            });
            if tick >= 30 {
                assert_eq!(mem_sys::local_alloc_count(), 0, "tick {tick}");
                compactions += ui.layer_rewrites().1 - before;
                if fresh { assert!(ui.output().draw[0].dirty.is_empty(), "unchanged fresh tiles must not upload"); }
                if let Some(phases) = ui.phase_cpu_times() { paint_ns += phases[4]; }
            }
            let layer = &ui.output().draw[0];
            largest = largest.max(layer.instances.len());
            assert!(layer.instances.len() <= 2 * (ROWS * TILES + 1 + (ROWS - step + 1) * 32));
            let mut count = 0;
            for instance in layer.instances.iter().filter(|i| i.color[1] == 10) {
                let row = first + count / TILES;
                assert_eq!(instance.color, [20 + (row % 200) as u8, 10, 10, 255]);
                count += 1;
            }
            assert_eq!(count, ROWS * TILES);
            assert_eq!(layer.instances.iter().filter(|i| i.color == [0, 0, 200, 255]).count(), 1);
            assert!(layer.dirty.iter().all(|&(a, b)| a < b && b as usize <= layer.instances.len()));
            if ui.layer_rewrites().1 != before {
                for (i, instance) in layer.instances.iter().enumerate() {
                    if previous.get(i) != Some(instance) {
                        assert!(layer.dirty.iter().any(|&(a, b)| (a as usize..b as usize).contains(&i)),
                            "compaction missed an upload at instance {i} on tick {tick}");
                    }
                }
            }
            previous.clear();
            previous.extend_from_slice(&layer.instances);
            assert_eq!(ui.rect(&ends[0].unwrap()), Rect::new(0.0, 0.0, 600.0, 20.0));
            assert_eq!(ui.rect(&ends[1].unwrap()), Rect::new(0.0, 380.0, 600.0, 20.0));
            ui.drawn(0);
        }
        println!("streaming retained gaps: step={step} fresh={fresh} frames=100 compactions={compactions} largest={largest} mean_paint_cpu_us={:.2}", paint_ns as f64 / 100_000.0);
        assert!(compactions <= 50, "step {step}: fresh batches compacted {compactions}/100 frames");
        ui.destroy();
    }
}

pub const CASES: &[(&str, fn())] = &[
    ("window_pxui_view_newcomer_keeps_warm_buffers", window_pxui_view_newcomer_keeps_warm_buffers),
    ("window_pxui_streaming_rows_reuse_retained_gaps", window_pxui_streaming_rows_reuse_retained_gaps),
    ("window_pxui_damage_containment_preserves_other_views", window_pxui_damage_containment_preserves_other_views),
    ("window_pxui_view_entrance_channels", window_pxui_view_entrance_channels),
    ("window_pxui_idle_loops_step_slower", window_pxui_idle_loops_step_slower),
    ("window_pxui_ambient_loops_share_ticks", window_pxui_ambient_loops_share_ticks),
    ("window_pxui_offscreen_transition_rests_without_frames", window_pxui_offscreen_transition_rests_without_frames),
    ("window_pxui_step_advances_transitions_without_a_build", window_pxui_step_advances_transitions_without_a_build),
    ("window_pxui_more_kept_views_reuse_compaction_scratch", window_pxui_more_kept_views_reuse_compaction_scratch),
    ("window_pxui_empty_buffers_preserve_names_and_values", window_pxui_empty_buffers_preserve_names_and_values),
    ("window_pxui_field_kept_drag", window_pxui_field_kept_drag),
    ("window_pxui_budget_counts", window_pxui_budget_counts),
    ("window_pxui_budget_cached_per_view", window_pxui_budget_cached_per_view),
    ("window_pxui_budget_zero_alloc", window_pxui_budget_zero_alloc),
    ("window_pxui_list_text_survives_idle", window_pxui_list_text_survives_idle),
    ("window_pxui_prepare_queue_scrolls_unseen_rows_without_shaping", window_pxui_prepare_queue_scrolls_unseen_rows_without_shaping),
    ("window_pxui_fling_shaping_stays_under_its_ceiling", window_pxui_fling_shaping_stays_under_its_ceiling),
    ("window_pxui_prewarmed_natural_text_is_the_layout_drawn_at_a_wider_width", window_pxui_prewarmed_natural_text_is_the_layout_drawn_at_a_wider_width),
    ("window_pxui_kept_view_text_survives_idle", window_pxui_kept_view_text_survives_idle),
    ("window_pxui_new_view_grows_within_its_room", window_pxui_new_view_grows_within_its_room),
    ("window_pxui_view_rebuilds_one", window_pxui_view_rebuilds_one),
    ("window_pxui_shrunk_layer_clips_dirty", window_pxui_shrunk_layer_clips_dirty),
    ("window_pxui_view_nested", window_pxui_view_nested),
    ("window_pxui_view_fit", window_pxui_view_fit),
    ("window_pxui_view_input", window_pxui_view_input),
    ("window_pxui_view_offset", window_pxui_view_offset),
    ("window_pxui_view_unchanged_controls_keep_table_storage", window_pxui_view_unchanged_controls_keep_table_storage),
    ("window_pxui_view_table_2000", window_pxui_view_table_2000),
    ("window_pxui_view_resize_settles", window_pxui_view_resize_settles),
    ("window_pxui_view_kept_atlas_break", window_pxui_view_kept_atlas_break),
    ("window_pxui_full_damage_uploads_changed_instances", window_pxui_full_damage_uploads_changed_instances),
    ("window_pxui_full_damage_uploads_fresh_and_relocated_kept_ranges", window_pxui_full_damage_uploads_fresh_and_relocated_kept_ranges),
    ("window_pxui_view_damage", window_pxui_view_damage),
    ("window_pxui_view_kit_cached", window_pxui_view_kit_cached),
    ("window_pxui_list", window_pxui_list),
    ("window_pxui_view_verify", window_pxui_view_verify),
    ("window_pxui_view_verify_transition", window_pxui_view_verify_transition),
    ("window_pxui_view_verify_hover_fade", window_pxui_view_verify_hover_fade),
    ("window_pxui_rebuilt_view_keeps_compaction_room_outside_its_run", window_pxui_rebuilt_view_keeps_compaction_room_outside_its_run),
    ("window_pxui_view_verify_layer", window_pxui_view_verify_layer),
    ("window_pxui_view_costs", window_pxui_view_costs),
];

/// Identical setters keep the published table buffer, avoiding a recomposition and accessibility invalidation.
pub fn window_pxui_view_unchanged_controls_keep_table_storage() {
    let mut ui=new_ui(None);
    let view=Cell::new(None);
    let mut build=|ui:&mut Ui| {
        view.set(Some(ui.view("unchanged",1,Size::Px(100.0),Size::Px(80.0),&mut |ui| { chip(ui,"child",RED,Flags::CLICKABLE); })));
    };
    ui.frame(&at(0.0,vec![]),&mut build);
    let view=view.get().unwrap();
    let clip=Some(Rect::new(0.0,0.0,320.0,240.0));
    ui.view_opacity(&view,0.5);
    ui.view_scale(&view,0.8);
    ui.view_clip(&view,clip);
    let address=ui.output().views.as_ptr();
    let revision=ui.draw_revision();
    mem_sys::local_reset();
    for _ in 0..20 {
        ui.view_opacity(&view,0.5);
        assert_eq!(ui.output().views.as_ptr(),address);
        ui.view_scale(&view,0.8);
        assert_eq!(ui.output().views.as_ptr(),address);
        ui.view_clip(&view,clip);
        assert_eq!(ui.output().views.as_ptr(),address);
        assert_eq!(ui.draw_revision(),revision);
    }
    assert_eq!(mem_sys::local_alloc_count(),0);
    ui.destroy();
}


/// A transition steps without a build: the builder does not run, and the output is what a full frame at the same time shows.
pub fn window_pxui_step_advances_transitions_without_a_build() {
    let mut ui=new_ui(None);
    let (view,child,builds)=(Cell::new(None),Cell::new(None),Cell::new(0));
    let mut build=|ui:&mut Ui| {
        builds.set(builds.get()+1);
        view.set(Some(ui.view("stepped",1,Size::Px(100.0),Size::Px(80.0),&mut |ui| {child.set(Some(chip(ui,"child",RED,Flags::NONE)));})));
        chip(ui,"outside",BLUE,Flags::NONE);
    };
    assert!(!ui.step(0.0),"nothing to step before the first frame");
    ui.frame(&at(0.0,vec![]),&mut build);
    ui.frame(&at(0.1,vec![]),&mut build);
    let curve=Curve::Bezier {duration:1.0,points:[0.0,0.0,1.0,1.0]};
    assert!(ui.animate_keys(&child.get().unwrap(),&[Keyframe::opacity(0.0,0.0).curve(curve),Keyframe::opacity(1.0,1.0).curve(curve)],&Keyframes::DEFAULT));
    assert!(ui.animate_keys(&view.get().unwrap(),&[Keyframe::offset(0.0,0.0,0.0).curve(curve),Keyframe::offset(1.0,20.0,0.0).curve(curve)],&Keyframes::DEFAULT));
    ui.frame(&at(0.2,vec![]),&mut build);
    let (built,revision)=(builds.get(),ui.draw_revision());
    // warm the step's patch scratch once, then a step allocates nothing
    assert!(ui.step(0.3));
    mem_sys::local_reset();
    assert!(ui.step(0.6),"only transitions are pending");
    assert_eq!(mem_sys::local_alloc_count(),0,"a warm step allocated");
    assert_eq!(builds.get(),built,"a step ran the builder");
    assert!(ui.draw_revision()>revision && ui.output().damage.bounds().is_some(),"a step reports what changed");
    assert_eq!(ui.repaint_at(),Some(0.6),"the transitions still run");
    let stepped=(ui.output().draw[0].instances.clone(),ui.output().views.to_vec());
    assert!(!ui.step(0.5),"time runs forward only");
    ui.frame(&at(0.6,vec![]),&mut build);
    assert_eq!((ui.output().draw[0].instances.clone(),ui.output().views.to_vec()),stepped,"a full frame at the same time shows the same");
    ui.request_frame();
    assert!(!ui.step(0.7),"a frame request needs a build");
    ui.destroy();
}

/// A transition can finish while its row is scrolled out, with no display-rate requests.
pub fn window_pxui_offscreen_transition_rests_without_frames() {
    let mut ui = new_ui(None);
    let (scroll, row) = (Cell::new(None), Cell::new(None));
    let mut build = |ui: &mut Ui| {
        let pane = ui.node("pane".into(), Node::column().w(Size::Px(100.0)).h(Size::Px(50.0)).flags(Flags::SCROLL));
        scroll.set(Some(pane));
        ui.within(&pane, &mut |ui| {
            row.set(Some(ui.view("animated-row", 1, Size::Px(80.0), Size::Px(20.0), &mut |ui| {
                chip(ui, "color", RED, Flags::NONE);
            })));
            ui.node("spacer".into(), Node::column().h(Size::Px(200.0)));
        });
    };
    ui.frame(&at(0.0, vec![]), &mut build);
    let curve = Curve::Bezier { duration: 1.0, points: [0.0, 0.0, 1.0, 1.0] };
    assert!(ui.animate(&row.get().unwrap(), Target::Opacity(0.4), curve));
    ui.set_scroll_offset(&scroll.get().unwrap(), 100.0);
    ui.frame(&at(0.1, vec![]), &mut build);
    assert_eq!(ui.transitions(), 1);
    assert_eq!(ui.repaint_at(), None);
    ui.set_scroll_offset(&scroll.get().unwrap(), 0.0);
    ui.frame(&at(1.2, vec![]), &mut build);
    assert_eq!(ui.transitions(), 0);
    assert!(ui.output().views.iter().any(|(_, entry)| (entry.opacity - 0.4).abs() < 0.001));
    ui.destroy();
}

/// An endless loop asks for every display frame while input is recent, then steps at 30 fps; a counted run never slows.
pub fn window_pxui_idle_loops_step_slower() {
    let mut ui=new_ui(None);
    let view=Cell::new(None);
    let mut build=|ui:&mut Ui| {view.set(Some(ui.view("loop",1,Size::Px(100.0),Size::Px(80.0),&mut |ui| {chip(ui,"child",RED,Flags::CLICKABLE);})));};
    ui.frame(&at(0.0,vec![]),&mut build);
    let curve=Curve::Bezier {duration:0.45,points:[0.0,0.0,1.0,1.0]};
    let keys=[Keyframe::opacity(0.0, 0.4).curve(curve),Keyframe::opacity(0.45, 1.0).curve(curve)];
    assert!(ui.animate_keys(&view.get().unwrap(),&keys,&Keyframes::DEFAULT.repeat(Repeat::Forever).autoreverse()));
    ui.frame(&at(0.1,vec![Event::FocusGained, Event::PointerMoved((300.0, 180.0))]),&mut build);
    assert_eq!(ui.repaint_at(),Some(0.1),"input is recent");
    assert_eq!(ui.repaint_hz(), 0, "input raises an ambient loop to display rate");
    ui.frame(&at(0.9,vec![]),&mut build);
    assert_eq!(ui.repaint_at(),Some(0.9),"still within a second of input");
    ui.frame(&at(1.2,vec![]),&mut build);
    assert_eq!(ui.repaint_at(),Some(1.2+1.0/30.0),"idle: the loop steps at 30 fps");
    assert_eq!(ui.repaint_hz(), 30);
    ui.frame(&at(1.3, vec![Event::PointerMoved((10.0, 10.0))]), &mut build);
    assert_eq!(ui.repaint_at(), Some(1.3), "entering a hover target counts");
    ui.frame(&at(2.2, vec![Event::PointerMoved((11.0, 10.0))]), &mut build);
    ui.frame(&at(2.4, vec![Event::PointerMoved((12.0, 10.0))]), &mut build);
    assert_eq!(ui.repaint_at(), Some(2.4 + 1.0 / 30.0), "movement inside the same target doesn't extend the hold");
    assert!(ui.animate_keys(&view.get().unwrap(),&keys,&Keyframes::DEFAULT.repeat(Repeat::Count(3))));
    ui.frame(&at(2.5,vec![]),&mut build);
    assert_eq!(ui.repaint_at(),Some(2.5),"a counted run plays every frame");
    assert_eq!(ui.repaint_hz(), 0);
    ui.destroy();
}

/// Separate start times and cadences do not interleave repaint requests.
pub fn window_pxui_ambient_loops_share_ticks() {
    use pxui_types::ui::Rate;
    let mut ui = new_ui(None);
    let slow = Cell::new(None);
    let fast = Cell::new(None);
    let mut build = |ui: &mut Ui| {
        slow.set(Some(ui.view("slow", 1, Size::Px(100.0), Size::Px(40.0), &mut |ui| {
            chip(ui, "child", RED, Flags::NONE);
        })));
        fast.set(Some(ui.view("fast", 1, Size::Px(100.0), Size::Px(40.0), &mut |ui| {
            chip(ui, "child", BLUE, Flags::NONE);
        })));
    };
    ui.frame(&at(0.0, vec![]), &mut build);
    let curve = Curve::Bezier { duration: 1.0, points: [0.0, 0.0, 1.0, 1.0] };
    let keys = [Keyframe::opacity(0.0, 0.4).curve(curve), Keyframe::opacity(1.0, 1.0).curve(curve)];
    let ambient = Keyframes::DEFAULT.repeat(Repeat::Forever);
    assert!(ui.animate_keys(&slow.get().unwrap(), &keys, &ambient.rate(Rate::Ambient(20))));
    ui.frame(&at(0.01, vec![]), &mut build);
    let original = ui.output().views.to_vec();
    assert_eq!(ui.repaint_hz(), 20);
    assert_eq!(ui.repaint_at(), Some(0.01 + 0.05));
    ui.frame(&at(0.02, vec![Event::PointerMoved((0.0, 0.0))]), &mut build);
    assert_eq!(ui.output().views, original, "motion over the same target must not sample the loop early");
    assert_eq!(ui.repaint_at(), Some(0.01 + 0.05));
    assert!(ui.animate_keys(&fast.get().unwrap(), &keys, &ambient));
    ui.frame(&at(0.023, vec![]), &mut build);
    let tick = 0.01 + 1.0 / 30.0;
    assert_eq!(ui.repaint_at(), Some(tick));
    assert_eq!(ui.repaint_hz(), 30, "the fastest active loop controls the presentation rate");
    let initial = ui.output().views.to_vec();
    assert!(ui.step(tick));
    assert_eq!(ui.output().views.iter().zip(&initial).filter(|(a, b)| a.1.opacity != b.1.opacity).count(), 2);
    assert_eq!(ui.repaint_at(), Some(tick + 1.0 / 30.0));
    ui.destroy();
}

pub fn window_pxui_view_entrance_channels() {
    let mut ui=new_ui(None);
    let (view,child)= (Cell::new(None),Cell::new(None));
    let mut build=|ui:&mut Ui| {view.set(Some(ui.view("entrance",1,Size::Px(100.0),Size::Px(80.0),&mut |ui| {child.set(Some(chip(ui,"child",RED,Flags::CLICKABLE)));})));};
    ui.frame(&at(0.0,vec![]),&mut build);
    let original=ui.output().draw[0].instances.clone();
    let curve=Curve::Bezier {duration:0.24,points:[0.0,0.0,1.0,1.0]};
    let keys=[Keyframe::opacity(0.0, 0.0).curve(curve),Keyframe::offset(0.0, 32.0,10.0).curve(curve),
        Keyframe::opacity(0.24, 1.0).curve(curve),Keyframe::offset(0.24, 0.0,0.0).curve(curve)];
    assert!(ui.animate_keys(&view.get().unwrap(),&keys,&Keyframes::DEFAULT));
    for (time,x,y,alpha) in [(0.0,32.0,10.0,0.0),(0.12,16.0,5.0,0.5),(0.24,0.0,0.0,1.0)] {
        ui.frame(&at(time,vec![]),&mut build);
        let r=ui.rect(&child.get().unwrap());
        assert!((r.x-x).abs()<0.001 && (r.y-y).abs()<0.001);
        assert!(ui.output().views.iter().any(|(_,e)|(e.opacity-alpha).abs()<0.001));
        assert_eq!(ui.output().draw[0].instances,original);
    }
    assert_eq!(ui.transitions(),0);
    ui.destroy();
}
