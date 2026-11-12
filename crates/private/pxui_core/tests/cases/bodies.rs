//! Widget bodies: a theme names a body per widget kind, behaviour (value, input, accessibility) stays the engine's, and what
//! behaviour needs of the structure it reads from the body's parts. Proof bodies: a checkbox for the toggle, a round 24-point
//! thumb for the slider, a taller track for progress. The default bodies' bytes are `bodies_default_identical` (pxui_gallery).
use crate::support::*;
use input::{Event, Key};
use pxui_core::a11y::{state, Role};
use pxui_core::ui::*;
use std::cell::Cell;

/// A toggle as a checkbox: a 12-point box at the row's start is the hit area, the label after it.
fn checkbox_body(ui: &mut Ui, c: &ToggleConfig, label: &str) {
    let t = ui.theme();
    let (look, boxes) = (*t.toggle.pick(c.state), t.checkbox);
    ui.set_look(&c.row, look, c.state);
    ui.node_mut(&c.row).inset = Edges { l: 24.0, ..Edges::ZERO };
    ui.text(&c.row, shown(label));
    let b = ui.part(Part::HIT, Node { at: Some((6.0, 6.0)).into(), w: Size::Px(12.0), h: Size::Px(12.0), ..Node::DEFAULT });
    ui.style(&b, &boxes, State { hovered: c.state.hovered, checked: c.on, ..State::default() });
}

/// A slider with a round 24-point thumb on a 4-point rail.
fn round_body(ui: &mut Ui, c: &SliderConfig, _label: &str) {
    let t = ui.theme();
    let (rail, fill, thumb) = (t.separator.idle, t.mark.idle, t.slider);
    ui.part(Part::RAIL, Node { h: Size::Px(4.0), look: rail, ..Node::DEFAULT });
    ui.node("fill", Node { w: Size::Pct(c.fraction), h: Size::Px(4.0), look: fill, ..Node::DEFAULT });
    let lane = ui.node("lane", Node { h: Size::Px(24.0), axis: Axis::Row, ..Node::DEFAULT });
    let inside = ui.enter(&lane);
    ui.node("before", Node { h: Size::Px(1.0), grow: c.fraction, ..Node::DEFAULT });
    let th = ui.part(Part::THUMB, Node { w: Size::Px(24.0), h: Size::Px(24.0), look: Look { radius: [12.0; 4], ..Look::NONE }, ..Node::DEFAULT });
    ui.style(&th, &thumb, c.state);
    ui.node("after", Node { h: Size::Px(1.0), grow: 1.0 - c.fraction, ..Node::DEFAULT });
    ui.leave(inside);
}

/// A progress row with an 8-point track and a half-width bar.
fn tall_body(ui: &mut Ui, c: &ProgressConfig, label: &str) {
    let t = ui.theme();
    let (track, bar) = (t.progress.idle, t.mark.idle);
    ui.text(&c.row, shown(label));
    let tr = ui.part(Part::TRACK, Node { flags: CLIP, h: Size::Px(8.0), look: track, ..Node::DEFAULT });
    let inside = ui.enter(&tr);
    match c.value {
        Some(v) => {
            ui.node("fill", Node { w: Size::Pct(v), h: Size::Grow, look: bar, ..Node::DEFAULT });
        }
        None => {
            ui.part(Part::BAR, Node { w: Size::Pct(0.5), h: Size::Grow, look: bar, ..Node::DEFAULT });
        }
    }
    ui.leave(inside);
}

fn frame(t: f64, events: Vec<Event>) -> input::frame::Frame {
    frame_at(vec![], 1.0, t, events)
}

fn tree_node(ui: &Ui, role: Role, name: &str) -> pxui_core::a11y::A11yNode {
    let tree = ui.output().a11y;
    *tree.nodes.iter().find(|n| n.role == role && tree.str(n.name) == name).unwrap_or_else(|| panic!("no {role:?} {name}"))
}

fn mid(r: pxui_core::geom::Rect, dx: f32) -> (i64, i64) {
    ((r.x + dx) as i64, (r.y + r.h / 2.0) as i64)
}

/// A UI whose toggle body is the checkbox when `checkbox` (registered by name, picked by the theme), the default otherwise.
fn toggle_ui(checkbox: bool) -> Ui {
    let mut ui = new_ui(None);
    ui.set_a11y(true);
    if checkbox {
        let id = ui.register_toggle_body("checkbox", checkbox_body);
        let mut theme = *ui.theme();
        theme.body.toggle = id;
        ui.set_theme(theme);
    }
    ui
}

/// The toggle's role, name, actions and the states a user can see: what a body must not change.
fn toggle_facts(ui: &Ui) -> (Role, u32, u32) {
    let n = tree_node(ui, Role::Toggle, "Animate");
    (n.role, n.actions, n.states & (state::CHECKABLE | state::CHECKED | state::FOCUSED))
}

/// Click, keyboard and accessibility state are the default body's; only the structure and where the press lands differ.
pub fn toggle_checkbox_body() {
    let mut runs = Vec::new();
    for checkbox in [false, true] {
        let mut ui = toggle_ui(checkbox);
        let on = Cell::new(false);
        let mut build = |ui: &mut Ui| ui.panel(PanelOpts::default(), "panel", &mut |ui| on.set(ui.toggle("Animate", on.get())));
        let mut t = 0.0;
        let mut step = |ui: &mut Ui, e: Vec<Event>| {
            t += 0.1;
            ui.frame(&frame(t, e), &mut build);
        };
        for _ in 0..2 {
            step(&mut ui, vec![]);
        }
        let row = tree_node(&ui, Role::Toggle, "Animate").rect;
        let (switch, boxed) = (mid(row, row.w - 20.0), mid(row, 12.0));
        let (hit, miss) = if checkbox { (boxed, switch) } else { (switch, boxed) };
        let mut click = |ui: &mut Ui, p: (i64, i64)| {
            for e in [vec![move_to(p)], vec![press(p)], vec![release(p)], vec![]] {
                step(ui, e);
            }
        };
        click(&mut ui, miss);
        check(!on.get(), &format!("checkbox {checkbox}: a press outside the body's hit area toggled"));
        click(&mut ui, hit);
        check(on.get(), &format!("checkbox {checkbox}: a press on the body's hit area did not toggle"));
        let after_click = toggle_facts(&ui);
        check(after_click.2 & state::CHECKED != 0, "the tree does not show it checked");
        step(&mut ui, vec![Event::KeyPressed(Key::Tab)]);
        step(&mut ui, vec![Event::KeyPressed(Key::Space)]);
        step(&mut ui, vec![]);
        check(!on.get(), &format!("checkbox {checkbox}: Space did not toggle it back"));
        let after_key = toggle_facts(&ui);
        // warm frames allocate nothing, with the override too
        mem_sys::local_reset();
        let before = mem_sys::local_alloc_count();
        for _ in 0..20 {
            step(&mut ui, vec![]);
        }
        check(mem_sys::local_alloc_count() == before, &format!("checkbox {checkbox}: warm frames allocated"));
        let instances = ui.output().draw[0].instances.to_vec();
        runs.push((after_click, after_key, instances));
        ui.destroy();
    }
    check(runs[0].0 == runs[1].0 && runs[0].1 == runs[1].1, &format!("accessibility facts differ: {:?} {:?}", (runs[0].0, runs[0].1), (runs[1].0, runs[1].1)));
    check(runs[0].2 != runs[1].2, "the checkbox body drew the same as the switch");
}

/// A 24-point thumb: a press at the rail's left end is 0 and at its right end 1, and a drag lands where the thumb's centre goes.
pub fn slider_custom_thumb() {
    let mut ui = new_ui(None);
    ui.set_a11y(true);
    let id = ui.register_slider_body("round", round_body);
    let mut theme = *ui.theme();
    theme.body.slider = id;
    ui.set_theme(theme);
    let v = Cell::new(0.5f32);
    let mut build = |ui: &mut Ui| ui.panel(PanelOpts::default(), "panel", &mut |ui| v.set(ui.slider("Level", v.get(), 0.0..=1.0)));
    let mut t = 0.0;
    let mut step = |ui: &mut Ui, e: Vec<Event>| {
        t += 0.1;
        ui.frame(&frame(t, e), &mut build);
    };
    for _ in 0..3 {
        step(&mut ui, vec![]);
    }
    let r = tree_node(&ui, Role::Slider, "Level").rect;
    let at = |f: f32| mid(r, 12.0 + f * (r.w - 24.0));
    let mut drag = |ui: &mut Ui, from: f32, to: f32| {
        for e in [vec![move_to(at(from))], vec![press(at(from))], vec![move_to(at(to))], vec![release(at(to))], vec![]] {
            step(ui, e);
        }
    };
    drag(&mut ui, 0.0, 0.0);
    check(v.get() < 0.002, &format!("a press at the left end of the rail: {}", v.get()));
    drag(&mut ui, 0.0, 1.0);
    check(v.get() > 0.998, &format!("a drag to the right end: {}", v.get()));
    drag(&mut ui, 1.0, 0.75);
    check((v.get() - 0.75).abs() < 0.002, &format!("a drag to three quarters: {}", v.get()));
    // the thumb is the body's: 24 points wide where the value puts it
    let thumb = ui.part_rect(key_of(key_of(0x2c1b3c6d, "panel"), "Level"), Part::THUMB).expect("the thumb part");
    check(thumb.w == 24.0 && thumb.h == 24.0, &format!("the thumb part is not the body's: {thumb:?}"));
    mem_sys::local_reset();
    let before = mem_sys::local_alloc_count();
    for _ in 0..20 {
        step(&mut ui, vec![]);
    }
    check(mem_sys::local_alloc_count() == before, "warm slider frames allocated");
    ui.destroy();
}

/// `busy_step` sweeps the bar a body built as `Part::BAR` along its `Part::TRACK`, and nothing else (the default body's too).
pub fn busy_step_custom_body() {
    busy_step_with(true);
    busy_step_with(false);
}

fn busy_step_with(custom: bool) {
    let mut ui = new_ui(None);
    ui.set_a11y(true);
    let id = ui.register_progress_body("tall", tall_body);
    let mut theme = *ui.theme();
    theme.body.progress = if custom { id } else { BodyId::DEFAULT };
    ui.set_theme(theme);
    let handles = std::cell::RefCell::new(Vec::new());
    let build = |ui: &mut Ui| {
        ui.view("busy", 1, Size::Px(240.0), Size::Px(100.0), &mut |ui| {
            let mut h = handles.borrow_mut();
            h.clear();
            h.push(ui.progress("Busy", None));
            h.push(ui.progress("Done", Some(0.5)));
        });
    };
    for t in 0..3 {
        ui.frame(&frame(t as f64 * 0.1, vec![]), &mut |ui| build(ui));
    }
    let (busy, done) = (handles.borrow()[0], handles.borrow()[1]);
    let tree = ui.output().a11y;
    let find = |name: &str| *tree.nodes.iter().find(|n| n.role == Role::Progress && tree.str(n.name) == name).expect(name);
    check(find("Busy").states & state::BUSY != 0, "the custom track is not busy in the tree");
    check(find("Done").value == pxui_core::a11y::Value::Number { now: 50.0, min: 0.0, max: 100.0, step: 1.0 }, "the custom track's number");
    let bar = |ui: &Ui| ui.part_rect(busy.box_key, Part::BAR).expect("the bar part");
    let track = ui.part_rect(busy.box_key, Part::TRACK).expect("the track part");
    check(track.h == if custom { 8.0 } else { 4.0 }, &format!("the track is the body's: {track:?}"));
    ui.frame(&frame(0.4, vec![]), &mut |ui| build(ui));
    check(ui.built_nodes() <= 2, "a kept frame built nodes");
    check(ui.busy_step(&busy, 1.0), "a custom body's bar did not move");
    let first = bar(&ui);
    check(ui.busy_step(&busy, 1.5), "the bar stopped");
    let second = bar(&ui);
    check(first.x != second.x && (first.w - track.w * 0.3).abs() < 0.01, &format!("the bar did not sweep: {first:?} {second:?}"));
    check(!ui.busy_step(&done, 1.0), "a determinate bar moved");
    mem_sys::local_reset();
    let before = mem_sys::local_alloc_count();
    for i in 0..60 {
        ui.busy_step(&busy, 2.0 + i as f64 / 60.0);
    }
    check(mem_sys::local_alloc_count() == before, "animated steps allocated");
    ui.destroy();
}

/// A theme file picks a registered body by name, and `default` the built-in one.
pub fn theme_file_selects_body() {
    let mut ui = new_ui(None);
    let (toggle, slider) = (ui.register_toggle_body("checkbox", checkbox_body), ui.register_slider_body("round", round_body));
    check(toggle == BodyId(1) && slider == BodyId(1), "ids are numbered from 1 per kind");
    check(ui.register_toggle_body("checkbox", checkbox_body) == toggle, "a name registered again keeps its id");
    check(ui.register_toggle_body("second", checkbox_body) == BodyId(2), "the next name is the next id");
    let base = Theme::default();
    let t = ui.parse_theme("toggle.body = checkbox # the box\nslider.body = round\nprogress.body = default\n", &base).expect("a theme with bodies");
    check(t.body == Bodies { toggle, slider, ..Bodies::default() }, &format!("{:?}", t.body));
    check(Theme::parse("toggle.body = default\n", &t).map(|t| t.body.toggle) == Ok(BodyId::DEFAULT), "default needs no registry");
    // keys the file leaves alone keep the base's body, and a token key keeps it too
    check(ui.parse_theme("tokens.density = 0.9\n", &t).map(|t| t.body) == Ok(t.body), "a token key dropped the bodies");
    // the body is a theme field: the theme differs, so a scope under it rebuilds and draws the body
    check(t != base, "a theme with a body equals the default");
    let draw = |theme: &Theme| {
        let mut ui = new_ui(None);
        ui.register_toggle_body("checkbox", checkbox_body);
        for k in 0..2 {
            ui.frame(&frame(k as f64 * 0.1, vec![]), &mut |ui| ui.themed(theme, &mut |ui| ui.panel(PanelOpts::default(), "panel", &mut |ui| { ui.toggle("Animate", true); })));
        }
        let drawn = ui.output().draw[0].instances.to_vec();
        ui.destroy();
        drawn
    };
    check(draw(&t) != draw(&base), "a scoped theme's body did not draw");
    // the file's text is not written back: a written theme has no body keys, and parses to the base's bodies
    check(Theme::parse(&t.write(), &base).map(|t| t.body) == Ok(Bodies::default()), "a written theme names bodies");
    ui.destroy();
}

/// A name nobody registered, for the wrong kind or for a kind with no bodies, is a `ThemeError` with its line; nothing is applied.
pub fn unknown_body_is_error() {
    let mut ui = new_ui(None);
    ui.register_toggle_body("checkbox", checkbox_body);
    let base = Theme::default();
    let errors = ui.parse_theme("tokens.density = 0.9\ntoggle.body = nosuch\nprogress.body = checkbox\nslider.body = checkbox\nbutton.body = checkbox\n", &base).expect_err("unknown names parsed");
    let lines: Vec<usize> = errors.iter().map(|e| e.line).collect();
    check(lines == [2, 3, 4, 5], &format!("error lines {lines:?}: {errors:?}"));
    check(errors[0].message.contains("nosuch") && errors[0].message.contains("toggle"), &format!("{:?}", errors[0]));
    check(Theme::parse("toggle.body = checkbox\n", &base).is_err(), "Theme::parse knows no registry");
    check(ui.parse_theme("toggle.body = \n", &base).is_err(), "an empty name parsed");
    // an id from another Ui draws the default body, with no panic
    let mut theme = base;
    theme.body.toggle = BodyId(7);
    ui.set_theme(theme);
    ui.frame(&frame(0.0, vec![]), &mut |ui| ui.panel(PanelOpts::default(), "panel", &mut |ui| { ui.toggle("Animate", true); }));
    ui.destroy();
}

/// A progress body that draws a ring and builds no track still has its accessible name, number and busy state (on the row).
fn ring_body(ui: &mut Ui, c: &ProgressConfig, label: &str) {
    ui.text(&c.row, shown(label));
    ui.node("ring", Node { w: Size::Px(16.0), h: Size::Px(16.0), look: Look { border: Look::NONE.fill, border_w: 2.0, radius: [8.0; 4], ..Look::NONE }, ..Node::DEFAULT });
}

pub fn progress_without_track_keeps_accessibility() {
    let mut ui = new_ui(None);
    ui.set_a11y(true);
    let mut theme = *ui.theme();
    theme.body.progress = ui.register_progress_body("ring", ring_body);
    ui.set_theme(theme);
    for t in 0..2 {
        ui.frame(&frame(t as f64 * 0.1, vec![]), &mut |ui| ui.panel(PanelOpts::default(), "panel", &mut |ui| {
            ui.progress("Busy", None);
            ui.progress("Done", Some(0.25));
        }));
    }
    let tree = ui.output().a11y;
    let find = |name: &str| *tree.nodes.iter().find(|n| n.role == Role::Progress && tree.str(n.name) == name).unwrap_or_else(|| panic!("no progress {name}"));
    check(find("Busy").states & state::BUSY != 0, "no busy state without a track");
    check(find("Done").value == pxui_core::a11y::Value::Number { now: 25.0, min: 0.0, max: 100.0, step: 1.0 }, "no number without a track");
    ui.destroy();
}

/// Click, keyboard, returned value and accessibility facts of the widget `(role, name)` with a theme's body installed and with the
/// built-in one; warm frames allocate nothing either way, and the two draw different bytes. `build` reads and writes the cell that
/// stands for the widget's value. Returns nothing: it fails a `check`.
fn swap(kind: &str, install: &dyn Fn(&mut Ui), role: Role, name: &str, key: Key, build: &dyn Fn(&mut Ui, &Cell<i64>)) {
    let mut runs = Vec::new();
    for custom in [false, true] {
        let mut ui = new_ui(None);
        ui.set_a11y(true);
        if custom {
            install(&mut ui);
        }
        let value = Cell::new(0);
        let mut b = |ui: &mut Ui| ui.panel(PanelOpts::default(), "panel", &mut |ui| build(ui, &value));
        let mut t = 0.0;
        let mut step = |ui: &mut Ui, e: Vec<Event>| {
            t += 0.1;
            ui.frame(&frame(t, e), &mut b);
        };
        for _ in 0..3 {
            step(&mut ui, vec![]);
        }
        let facts = |ui: &Ui| {
            let n = tree_node(ui, role, name);
            (n.role, n.actions, n.states & (state::CHECKABLE | state::CHECKED))
        };
        let p = mid(tree_node(&ui, role, name).rect, 4.0);
        for e in [vec![move_to(p)], vec![press(p)], vec![release(p)], vec![]] {
            step(&mut ui, e);
        }
        let clicked = (value.get(), facts(&ui));
        step(&mut ui, vec![Event::KeyPressed(Key::Tab)]);
        step(&mut ui, vec![Event::KeyPressed(key)]);
        step(&mut ui, vec![]);
        let keyed = (value.get(), facts(&ui));
        mem_sys::local_reset();
        let before = mem_sys::local_alloc_count();
        for _ in 0..20 {
            step(&mut ui, vec![]);
        }
        check(mem_sys::local_alloc_count() == before, &format!("{kind} custom {custom}: warm frames allocated"));
        runs.push((clicked, keyed, ui.output().draw[0].instances.to_vec()));
        ui.destroy();
    }
    check(runs[0].0 .0 != 0, &format!("{kind}: the click did nothing"));
    check(runs[0].0 == runs[1].0 && runs[0].1 == runs[1].1, &format!("{kind}: behaviour differs: {:?} {:?}", (runs[0].0, runs[0].1), (runs[1].0, runs[1].1)));
    check(runs[0].2 != runs[1].2, &format!("{kind}: the body drew the same bytes"));
}

fn pick(ui: &mut Ui, f: &dyn Fn(&mut Theme, BodyId), register: &dyn Fn(&mut Ui) -> BodyId) {
    let id = register(ui);
    let mut theme = *ui.theme();
    f(&mut theme, id);
    ui.set_theme(theme);
}

/// A pill: a fixed 90 x 20 box with a dot inside, in the caller's look.
fn pill_button(ui: &mut Ui, c: &ButtonConfig, states: &States, _text: &str) {
    let look = *states.pick(c.state);
    let rh = ui.row_height() as f32;
    ui.node_mut(&c.line).h = Size::Px(rh);
    let n = ui.node_mut(&c.row);
    (n.w, n.h) = (Size::Px(90.0), Size::Px(20.0));
    ui.set_look(&c.row, Look { radius: [10.0; 4], ..look }, c.state);
    ui.node("dot", Node { w: Size::Px(8.0), h: Size::Px(8.0), margin: Edges::all(6.0), look: Look { fill: look.text, ..Look::NONE }, ..Node::DEFAULT });
}

/// Press, Space, the returned `true`, the role and the name are the default button's; the box is the body's.
pub fn button_pill_body() {
    let hits = |ui: &mut Ui, v: &Cell<i64>| {
        if ui.button("Go") {
            v.set(v.get() + 1);
        }
        if ui.button_styled("Bare", true, false, true, None) {
            v.set(v.get() + 100);
        }
    };
    swap("button", &|ui| pick(ui, &|t, id| t.body.button = id, &|ui| ui.register_button_body("pill", pill_button)), Role::Button, "Go", Key::Space, &hits);
    // the geometry is the body's: its box is 90 wide, not the text's
    let mut ui = new_ui(None);
    pick(&mut ui, &|t, id| t.body.button = id, &|ui| ui.register_button_body("pill", pill_button));
    for k in 0..2 {
        ui.frame(&frame(k as f64 * 0.1, vec![]), &mut |ui| ui.panel(PanelOpts::default(), "panel", &mut |ui| { ui.button("Go"); }));
    }
    let w = ui.box_of_key(key_of(key_of(0x2c1b3c6d, "panel"), "Go")).map(|b| ui.rect(&b).w);
    check(w == Some(90.0), &format!("the button's box is not the body's: {w:?}"));
    ui.destroy();
}

fn round_icon(ui: &mut Ui, c: &IconButtonConfig, _label: &str, _icon: &str) {
    let look = *ui.theme().icon_button.pick(c.state);
    let n = ui.node_mut(&c.node);
    (n.w, n.h) = (Size::Px(32.0), Size::Px(32.0));
    ui.set_look(&c.node, Look { radius: [16.0; 4], ..look }, c.state);
    ui.node("dot", Node { w: Size::Px(8.0), h: Size::Px(8.0), margin: Edges::all(12.0), look: Look { fill: look.text, ..Look::NONE }, ..Node::DEFAULT });
}

pub fn icon_button_round_body() {
    let hits = |ui: &mut Ui, v: &Cell<i64>| {
        if ui.icon_button("Add", "+") {
            v.set(v.get() + 1);
        }
    };
    swap("icon_button", &|ui| pick(ui, &|t, id| t.body.icon_button = id, &|ui| ui.register_icon_button_body("round", round_icon)), Role::Button, "Add", Key::Space, &hits);
}

/// A checkbox or radio row as a label and one 10-point mark that is filled when on.
fn mark_check(ui: &mut Ui, c: &CheckConfig, text: &str) {
    let look = ui.theme().toggle.idle;
    ui.set_look(&c.row, look, c.state);
    ui.node_mut(&c.row).inset = Edges { l: 20.0, ..Edges::ZERO };
    ui.text(&c.row, shown(text));
    let fill = if c.on { ui.theme().tokens.accent } else { Look::NONE.fill };
    ui.node("mark", Node { w: Size::Px(10.0), h: Size::Px(10.0), margin: Edges::all(2.0), look: Look { fill, ..Look::NONE }, ..Node::DEFAULT });
}

pub fn checkbox_mark_body() {
    let hits = |ui: &mut Ui, v: &Cell<i64>| v.set(ui.checkbox("Snap", v.get() != 0) as i64);
    swap("checkbox", &|ui| pick(ui, &|t, id| t.body.checkbox = id, &|ui| ui.register_checkbox_body("mark", mark_check)), Role::CheckBox, "Snap", Key::Space, &hits);
}

pub fn radio_mark_body() {
    let hits = |ui: &mut Ui, v: &Cell<i64>| v.set(ui.radio("Mode", &["A", "B", "C"], v.get() as usize) as i64);
    // a click on "B" chooses 1; Tab then Down moves on to 2
    swap("radio", &|ui| pick(ui, &|t, id| t.body.radio = id, &|ui| ui.register_radio_body("mark", mark_check)), Role::Radio, "B", Key::ArrowDown, &hits);
}

type Facts = Vec<(Role, String, pxui_core::a11y::Value, u32, u32)>;

/// What the accessibility tree says of everything built, and the layer bytes, after `build` has settled; warm frames allocate nothing.
fn settled(kind: &str, custom: &str, ui: &mut Ui, build: &dyn Fn(&mut Ui)) -> (Facts, String) {
    let mut t = 0.0;
    let mut step = |ui: &mut Ui| {
        t += 0.1;
        ui.frame(&frame(t, vec![]), &mut |ui| ui.panel(PanelOpts::default(), "panel", &mut |ui| build(ui)));
    };
    for _ in 0..3 {
        step(ui);
    }
    mem_sys::local_reset();
    let before = mem_sys::local_alloc_count();
    for _ in 0..20 {
        step(ui);
    }
    check(mem_sys::local_alloc_count() == before, &format!("{kind} {custom}: warm frames allocated"));
    let tree = ui.output().a11y;
    let facts = tree.nodes.iter().map(|n| (n.role, tree.str(n.name).to_owned(), n.value, n.states, n.actions)).collect();
    (facts, format!("{:?}", ui.output().draw[0].instances))
}

/// `settled` with the built-in body and with the one `install` puts in the theme: the facts are the same, the bytes are not.
fn swap_static(kind: &str, install: &dyn Fn(&mut Ui), build: &dyn Fn(&mut Ui)) {
    let mut runs = Vec::new();
    for custom in [false, true] {
        let mut ui = new_ui(None);
        ui.set_a11y(true);
        if custom {
            install(&mut ui);
        }
        runs.push(settled(kind, if custom { "custom" } else { "default" }, &mut ui, build));
        ui.destroy();
    }
    check(runs[0].0 == runs[1].0, &format!("{kind}: accessibility facts differ: {:?} {:?}", runs[0].0, runs[1].0));
    check(runs[0].0.len() >= 1, &format!("{kind}: nothing in the tree"));
    check(runs[0].1 != runs[1].1, &format!("{kind}: the body drew the same bytes"));
}

/// Three big dots: `busy_step` turns the dots it finds and stops at the first missing one.
fn three_dots(ui: &mut Ui, c: &SpinnerConfig, _label: &str) {
    let n = ui.node_mut(&c.node);
    (n.w, n.h) = (Size::Px(24.0), Size::Px(8.0));
    n.axis = Axis::Row;
    for i in 0..3 {
        ui.part(Part::dot(i), Node { w: Size::Px(6.0), h: Size::Px(6.0), margin: Edges::all(1.0), look: Look { fill: ui_ink(), ..Look::NONE }, ..Node::DEFAULT });
    }
}

fn ui_ink() -> pxui_core::geom::Rgba {
    pxui_core::geom::color::white()
}

pub fn spinner_three_dots_body() {
    let install = |ui: &mut Ui| pick(ui, &|t, id| t.body.spinner = id, &|ui| ui.register_spinner_body("three", three_dots));
    swap_static("spinner", &install, &|ui| { ui.spinner("Loading"); });
    let mut ui = new_ui(None);
    install(&mut ui);
    let h = std::cell::Cell::new(None);
    for k in 0..3 {
        ui.frame(&frame(k as f64 * 0.1, vec![]), &mut |ui| ui.panel(PanelOpts::default(), "panel", &mut |ui| h.set(Some(ui.spinner("Loading")))));
    }
    let spinner = h.get().expect("the spinner");
    check(ui.part_rect(spinner.box_key, Part::dot(2)).is_some_and(|r| r.w == 6.0) && ui.part_rect(spinner.box_key, Part::dot(3)).is_none(), "the dots are the body's");
    check(ui.busy_step(&spinner, 0.5), "a custom spinner did not turn");
    mem_sys::local_reset();
    let before = mem_sys::local_alloc_count();
    for i in 0..60 {
        ui.busy_step(&spinner, 1.0 + i as f64 / 60.0);
    }
    check(mem_sys::local_alloc_count() == before, "animated steps allocated");
    ui.destroy();
}

/// A badge as an outlined box that is always 40 wide; the text is still on the node `patch_text` finds.
fn box_badge(ui: &mut Ui, c: &BadgeConfig, _label: &str, text: &str) {
    let n = ui.node_mut(&c.node);
    (n.w, n.h, n.look) = (Size::Px(40.0), Size::Px(20.0), Look { border: pxui_core::geom::color::white(), border_w: 1.0, ..Look::NONE });
    ui.text(&c.node, text);
}

pub fn badge_box_body() {
    let install = |ui: &mut Ui| pick(ui, &|t, id| t.body.badge = id, &|ui| ui.register_badge_body("box", box_badge));
    swap_static("badge", &install, &|ui| { ui.badge("Unread", "12"); });
    let mut ui = new_ui(None);
    install(&mut ui);
    let h = std::cell::Cell::new(None);
    for k in 0..3 {
        ui.frame(&frame(k as f64 * 0.1, vec![]), &mut |ui| ui.panel(PanelOpts::default(), "panel", &mut |ui| h.set(Some(ui.badge("Unread", "12")))));
    }
    check(ui.patch_text(&h.get().expect("the badge"), "13"), "patch_text missed the badge's node");
    ui.destroy();
}

/// A caption with a rule under it: the label's node holds a child.
fn ruled_label(ui: &mut Ui, c: &LabelConfig, text: &str) {
    let n = ui.node_mut(&c.node);
    n.h = Size::Px(30.0);
    ui.text(&c.node, shown(text));
    ui.node("rule", Node { h: Size::Px(1.0), margin: Edges { t: 20.0, ..Edges::ZERO }, look: Look { fill: pxui_core::geom::color::white(), ..Look::NONE }, ..Node::DEFAULT });
}

pub fn label_rule_body() {
    let install = |ui: &mut Ui| pick(ui, &|t, id| t.body.label = id, &|ui| ui.register_label_body("rule", ruled_label));
    swap_static("label", &install, &|ui| ui.label("Section"));
}

/// A footer that is just its end key, with no rule and no hints.
fn bare_footer(ui: &mut Ui, c: &FooterConfig, right: Option<&str>, _hints: &[(String, String)]) {
    ui.node_mut(&c.row).h = Size::Px(24.0);
    let end = ui.node("end", Node { w: Size::Px(60.0), h: Size::Px(24.0), ..Node::DEFAULT });
    ui.text(&end, right.unwrap_or(""));
}

pub fn footer_bare_body() {
    let install = |ui: &mut Ui| pick(ui, &|t, id| t.body.footer = id, &|ui| ui.register_footer_body("bare", bare_footer));
    let hints = [("Esc".to_owned(), "close".to_owned())];
    swap_static("footer", &install, &|ui| ui.footer(Some("3 items"), &hints));
}

/// A grip with a 21-point press band: 10 points either side of its line.
fn wide_grip(ui: &mut Ui, c: &GripConfig) {
    ui.node_mut(&c.grip).w = Size::Px(1.0);
    ui.set_look(&c.grip, Look { fill: pxui_core::geom::color::white(), ..Look::NONE }, c.state);
    ui.hit_outset(&c.grip, Edges { l: 10.0, r: 10.0, ..Edges::ZERO });
}

/// Where a press takes the splitter's grip is the body's band (3 points either side by default, 10 in the custom body); the drag,
/// the cursor, the role and the returned ratio are the same.
pub fn grip_wide_band_body() {
    let mut runs = Vec::new();
    for custom in [false, true] {
        let mut ui = new_ui(None);
        ui.set_a11y(true);
        if custom {
            pick(&mut ui, &|t, id| t.body.grip = id, &|ui| ui.register_grip_body("wide", wide_grip));
        }
        let ratio = Cell::new(0.5f32);
        let mut build = |ui: &mut Ui| {
            let host = ui.node("host", Node { at: Some((0.0, 0.0)).into(), w: Size::Px(201.0), h: Size::Px(100.0), ..Node::DEFAULT });
            let saved = ui.enter(&host);
            let got = ui.split("split", Axis::Row, ratio.get(), 20.0, 30.0, &mut |ui| { ui.node("a", Node { h: Size::Grow, ..Node::DEFAULT }); }, &mut |ui| { ui.node("b", Node { h: Size::Grow, ..Node::DEFAULT }); });
            if let Some(r) = got {
                ratio.set(r);
            }
            ui.leave(saved);
        };
        let mut t = 0.0;
        let mut step = |ui: &mut Ui, e: Vec<Event>| {
            t += 0.1;
            ui.frame(&frame(t, e), &mut build);
        };
        for _ in 0..3 {
            step(&mut ui, vec![]);
        }
        // the line is at x = 100; a drag that starts `off` points from it moves the ratio, or does not
        let mut drag = |ui: &mut Ui, off: i64| {
            ratio.set(0.5);
            let (from, to) = ((100 - off, 50), (150, 50));
            for e in [vec![move_to(from)], vec![press(from)], vec![move_to(to)], vec![release(to)], vec![]] {
                step(ui, e);
            }
            ratio.get() != 0.5
        };
        let seen = [drag(&mut ui, 0), drag(&mut ui, 2), drag(&mut ui, 6), drag(&mut ui, 9), drag(&mut ui, 14)];
        check(seen == if custom { [true, true, true, true, false] } else { [true, true, false, false, false] }, &format!("custom {custom}: drags started from 0, 2, 6, 9 and 14 points off: {seen:?}"));
        mem_sys::local_reset();
        let before = mem_sys::local_alloc_count();
        for _ in 0..20 {
            step(&mut ui, vec![]);
        }
        check(mem_sys::local_alloc_count() == before, &format!("custom {custom}: warm frames allocated"));
        let tree = ui.output().a11y;
        let facts: Facts = tree.nodes.iter().map(|n| (n.role, tree.str(n.name).to_owned(), n.value, n.states, n.actions)).collect();
        runs.push(facts);
        ui.destroy();
    }
    check(runs[0] == runs[1] && runs[0].iter().any(|f| f.0 == Role::Splitter), &format!("accessibility facts differ: {:?} {:?}", runs[0], runs[1]));
}

pub const CASES: &[(&str, fn())] = &[
    ("toggle_checkbox_body", toggle_checkbox_body),
    ("slider_custom_thumb", slider_custom_thumb),
    ("busy_step_custom_body", busy_step_custom_body),
    ("progress_without_track_keeps_accessibility", progress_without_track_keeps_accessibility),
    ("theme_file_selects_body", theme_file_selects_body),
    ("unknown_body_is_error", unknown_body_is_error),
    ("button_pill_body", button_pill_body),
    ("icon_button_round_body", icon_button_round_body),
    ("checkbox_mark_body", checkbox_mark_body),
    ("radio_mark_body", radio_mark_body),
    ("spinner_three_dots_body", spinner_three_dots_body),
    ("badge_box_body", badge_box_body),
    ("label_rule_body", label_rule_body),
    ("footer_bare_body", footer_bare_body),
    ("grip_wide_band_body", grip_wide_band_body),
];
