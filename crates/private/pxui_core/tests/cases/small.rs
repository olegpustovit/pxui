//! Ports of lib/pxui ,  and .
use crate::support::*;
use input::{Event, Key, MouseButton, ScrollPhase};
use pxui_core::ui::*;
use input::frame;
use std::cell::Cell;
use std::rc::Rc;

// ------------------------------------------------------------------ test_table

fn table_frame(count: i64, events: Vec<Event>) -> frame::Frame {
    let mut f = frame_at(Vec::new(), 1.0, count as f64 / 60.0, events);
    f.input.size = (360, 240);
    f
}

pub fn window_pxui_table() {
    let mut ui = new_ui(None);
    let cells = Rc::new(Cell::new(0i64));
    let first = Rc::new(Cell::new(i64::MAX));
    let clicked = Rc::new(Cell::new(None::<i64>));
    let mut build = {
        let (cells, first, clicked) = (cells.clone(), first.clone(), clicked.clone());
        move |ui: &mut Ui| {
            let (cells, first) = (cells.clone(), first.clone());
            let headers = ["index", "value"];
            let cell = move |row: i64, column: i64| {
                cells.set(cells.get() + 1);
                first.set(first.get().min(row));
                (row + column).to_string()
            };
            let c = ui.table_view("table", &headers, &mut [], 1_000_000 as usize, &mut None, &pxui_core::ui::TableView::text(pxui_core::ui::Rect::new(0.0, 0.0, 300.0, 168.0)), &mut |ui, r, c| ui.item_text(&cell(r as i64, c as i64))).row.map(|r| r as i64);
            clicked.set(c);
        }
    };
    ui.frame(&table_frame(0, vec![]), &mut build);
    cells.set(0);
    ui.frame(&table_frame(1, vec![]), &mut build);
    assert!(cells.get() > 0 && cells.get() <= 20);
    ui.frame(&table_frame(2, vec![press((30, 36)), release((30, 36))]), &mut build);
    assert!(clicked.get() == Some(0));
    cells.set(0);
    first.set(i64::MAX);
    ui.frame(&table_frame(3, vec![move_to((30, 100)), Event::Scrolled((0.0, -10000.0))]), &mut build);
    assert!(cells.get() >= 12 && cells.get() <= 20 && first.get() == 10000);
    cells.set(0);
    ui.frame(&table_frame(4, vec![]), &mut build);
    check(
        cells.get() >= 12 && cells.get() <= 20 && first.get() == 10000,
        &format!("scrolled table cells={} first={}", cells.get(), first.get()),
    );
    ui.frame(&table_frame(5, vec![press((30, 36)), release((30, 36))]), &mut build);
    assert!(clicked.get() == Some(10000));
    for count in 6..=15 {
        cells.set(0);
        first.set(i64::MAX);
        ui.frame(&table_frame(count, vec![move_to((30, 100)), Event::Scrolled((0.0, -3.0))]), &mut build);
        assert!(cells.get() >= 12 && cells.get() <= 20 && first.get() == 10000 + (count - 5) * 3);
    }
    cells.set(0);
    first.set(i64::MAX);
    ui.frame(
        &table_frame(
            16,
            vec![
                Event::TrackpadScrolled { delta: (0.0, -72.0), phase: ScrollPhase::Moved, time: 16.0 / 60.0 },
                Event::Scrolled((0.0, -3.0)),
            ],
        ),
        &mut build,
    );
    assert!(cells.get() >= 12 && cells.get() <= 20 && first.get() == 10033);
    ui.frame(
        &table_frame(17, vec![Event::TrackpadScrolled { delta: (0.0, -72.0), phase: ScrollPhase::Moved, time: 17.0 / 60.0 }]),
        &mut build,
    );
    ui.frame(
        &table_frame(18, vec![Event::TrackpadScrolled { delta: (0.0, 0.0), phase: ScrollPhase::Lifted, time: 18.0 / 60.0 }]),
        &mut build,
    );
    cells.set(0);
    first.set(i64::MAX);
    ui.frame(&table_frame(19, vec![]), &mut build);
    assert!(cells.get() >= 12 && cells.get() <= 20 && first.get() > 10036);
    ui.destroy();
}

// ------------------------------------------------------------------ test_keyboard

fn keyboard_frame(keys: Vec<Key>, events: Vec<Event>) -> frame::Frame {
    let mut input = input::at_time(1.0);
    input.frame = 0;
    input.tick = 0;
    input.size = (500, 360);
    input.pointer = (-10.0, -10.0);
    input.keys = keys;
    input.events = events;
    frame::Frame { input, pixel_scale: (1.0, 1.0), fps: 60.0, mouse_delta: (0.0, 0.0) }
}

#[derive(Clone)]
struct Model {
    clicks: i64,
    enabled: bool,
    text: String,
}

fn controls(ui: &mut Ui, model: &mut Model) {
    let m = model.clone();
    let mut out = m.clone();
    ui.panel("controls", &pxui_core::ui::Overlay::DEFAULT.w_px(400.0).name("controls"), &mut |ui| {
        out.clicks = m.clicks + if ui.button("Run") { 1 } else { 0 };
        out.enabled = ui.toggle("Enabled", m.enabled);
        out.text = ui.text_field("Text", &m.text).unwrap_or_else(|| m.text.clone());
    });
    *model = out;
}

pub fn window_pxui_keyboard() {
    let mut ui = new_ui(Some(11));
    let mut model = Model { clicks: 0, enabled: false, text: "old".to_owned() };
    fn step(ui: &mut Ui, model: &mut Model, keys: Vec<Key>, events: Vec<Event>) {
        ui.frame(&keyboard_frame(keys, events), &mut |ui| controls(ui, model));
    }
    step(&mut ui, &mut model, vec![], vec![]);
    step(
        &mut ui,
        &mut model,
        vec![],
        vec![key_down(Key::Shift), key_down(Key::Tab), Event::KeyReleased(Key::Shift), Event::TextInput("!".into())],
    );
    check(model.text == "old!", "same-frame Shift release changed backwards Tab traversal");
    step(&mut ui, &mut model, vec![], vec![key_down(Key::Tab), key_down(Key::Enter)]);
    check(
        model.clicks == 1 && ui.input(None).input.events.is_empty(),
        "Tab/Enter did not activate the first control or leaked into the host",
    );
    step(&mut ui, &mut model, vec![], vec![key_down(Key::Tab), key_down(Key::Space)]);
    check(model.enabled, "Space did not toggle the focused control");
    // Rebuilding fewer controls prunes focus and keeps hidden controls inert.
    ui.frame(&keyboard_frame(vec![], vec![]), &mut |ui| { ui.panel("controls", &pxui_core::ui::Overlay::DEFAULT.w_px(400.0).name("controls"), &mut |_| {}); });
    check(!ui.text_input_focused(), "disappearing focused controls kept focus");
    step(&mut ui, &mut model, vec![], vec![key_down(Key::Space)]);
    check(model.enabled, "a control that had disappeared kept its keyboard activation");
    step(&mut ui, &mut model, vec![Key::Shift], vec![key_down(Key::Tab), Event::TextInput("!".into())]);
    check(model.text == "old!!", "Shift-Tab did not enter the last text field");
    step(
        &mut ui,
        &mut model,
        vec![],
        vec![key_down(Key::Meta), key_down(kc('a')), Event::KeyReleased(Key::Meta), Event::TextInput("new".into())],
    );
    check(model.text == "new", "same-frame Command release broke text select-all");
    step(
        &mut ui,
        &mut model,
        vec![],
        vec![key_down(Key::Shift), key_down(Key::ArrowLeft), Event::KeyReleased(Key::Shift), key_down(Key::Backspace)],
    );
    check(model.text == "ne", "same-frame Shift release broke text selection");
    step(&mut ui, &mut model, vec![], vec![key_down(Key::Tab), key_down(Key::Enter)]);
    check(model.clicks == 2, "Tab did not wrap to the first control");
    step(&mut ui, &mut model, vec![], vec![key_down(Key::Escape)]);
    check(!ui.text_input_focused(), "Escape did not return control keys to the host");
    step(&mut ui, &mut model, vec![], vec![key_down(Key::Tab)]);
    step(&mut ui, &mut model, vec![], vec![Event::FocusLost, key_down(Key::Enter)]);
    check(model.clicks == 2 && !ui.text_input_focused(), "focus loss did not cancel keyboard activation");
    const ITEMS: [MenuItem; 2] = [MenuItem::new("Disabled").disabled(true), MenuItem::new("Enabled")];
    let mut menu = Chosen::default();
    let mut background = false;
    let mut popup = |ui: &mut Ui| {
        menu = ui.menu("menu".into(), pxui_types::geom::Point::new(100.0, 100.0), &ITEMS, &Menu::DEFAULT);
        ui.panel("background", &pxui_core::ui::Overlay::DEFAULT.name("background"), &mut |ui| background = ui.button("Background"));
    };
    ui.frame(&keyboard_frame(vec![], vec![]), &mut popup);
    ui.frame(&keyboard_frame(vec![], vec![key_down(Key::Tab), key_down(Key::Enter)]), &mut popup);
    check(menu.chosen == Some(1) && !background, "modal traversal reached a disabled row or escaped to the background");
    ui.frame(&keyboard_frame(vec![], vec![Event::FocusLost]), &mut |_| {});
    let picked = std::cell::RefCell::new(None);
    let mut picker = |ui: &mut Ui| {
        ui.modal("picker", &pxui_core::ui::Overlay::DEFAULT.name("picker"), &mut |ui| {
            let rows = |_: &str| vec![("One".to_owned(), String::new()), ("Two".to_owned(), String::new())];
            *picked.borrow_mut() = Some({ let r = ui.picker("Search", "Search", "", &pxui_core::ui::PickerOpts::default(), &mut |q: &str, out: &mut pxui_core::ui::PickRows| for (a, b) in rows(q) { out.push(&a, &b) }); (r.query.to_owned(), r.pick) });
        });
    };
    ui.frame(&keyboard_frame(vec![], vec![]), &mut picker);
    *picked.borrow_mut() = None;
    ui.frame(&keyboard_frame(vec![], vec![key_down(Key::Tab), key_down(Key::Enter)]), &mut picker);
    check(*picked.borrow() == Some((String::new(), Pick::Pick(0))), "picker stole focus from its keyboard-activated row");
    ui.destroy();
}

// ------------------------------------------------------------------ test_input

fn input_frame(buttons: Vec<MouseButton>, keys: Vec<Key>, events: Vec<Event>) -> frame::Frame {
    let mut input = input::at_time(0.0);
    input.frame = 0;
    input.tick = 0;
    input.size = (320, 240);
    input.pointer = (10.0, 10.0);
    input.keys = keys;
    input.buttons = buttons;
    input.events = events;
    frame::Frame { input, pixel_scale: (1.0, 1.0), fps: 60.0, mouse_delta: (0.0, 0.0) }
}

fn plain(events: Vec<Event>) -> frame::Frame {
    input_frame(vec![], vec![], events)
}

pub fn window_pxui_input() {
    let mut ui = new_ui(Some(11));
    let body = |ui: &mut Ui| -> (i64, Signal, Signal) {
        let view = ui.box_("view", BoxOpts::default().flags(Flags::CLICKABLE).at((0.0, 0.0)).w(Size::Px(160.0)).h(Size::Px(240.0)));
        let saved = ui.enter(&view);
        let control = ui.box_("control", BoxOpts::default().flags(Flags::CLICKABLE | Flags::FOCUSABLE).at((30.0, 30.0)).w(Size::Px(50.0)).h(Size::Px(50.0)));
        ui.leave(saved);
        let graph = ui.box_("graph", BoxOpts::default().flags(Flags::CLICKABLE | Flags::SCROLL).at((160.0, 0.0)).w(Size::Px(160.0)).h(Size::Px(240.0)));
        (key_of_box(&view), ui.signal_owned(&control), ui.signal_owned(&graph))
    };
    let step = |ui: &mut Ui, popup: bool, f: frame::Frame| -> (frame::Frame, Signal, Signal) {
        let mut out = None;
        ui.frame(&f, &mut |ui| {
            if popup {
                ui.modal("modal", &pxui_core::ui::Overlay::DEFAULT.name("modal"), &mut |ui| ui.label("Popup", pxui_core::ui::Label::DEFAULT));
            }
            out = Some(body(ui));
        });
        let (owner, control, graph) = out.unwrap();
        (ui.input(Some(owner)), control, graph)
    };
    step(&mut ui, false, plain(vec![]));
    let drag = vec![
        Event::PointerPressed(MouseButton::RightButton, (10.0, 10.0)),
        Event::PointerMoved((220.0, 20.0)),
        Event::PointerReleased(MouseButton::RightButton, (220.0, 20.0)),
    ];
    let (inp, _, graph) = step(&mut ui, false, plain(drag.clone()));
    check(inp.input.events == drag && !graph.pressed, "viewport capture did not survive a pane crossing/release");
    let drag = vec![
        Event::PointerPressed(MouseButton::MiddleButton, (200.0, 10.0)),
        Event::PointerMoved((10.0, 10.0)),
        Event::PointerReleased(MouseButton::MiddleButton, (10.0, 10.0)),
    ];
    let (inp, _, graph) = step(&mut ui, false, plain(drag));
    check(
        inp.input.events == vec![Event::PointerCancelled(MouseButton::MiddleButton)] && graph.pressed,
        "graph-owned drag reached the viewport",
    );
    let click = vec![press((40, 40)), release((40, 40)), key_down(Key::Enter)];
    let (inp, control, _) = step(&mut ui, false, input_frame(vec![], vec![Key::Shift], click.clone()));
    check(
        control.clicked && inp.input.events == vec![Event::PointerCancelled(MouseButton::LeftButton)] && inp.input.keys.is_empty(),
        "child widget input escaped to its viewport root",
    );
    let mut events = click.clone();
    events.push(Event::Scrolled((0.0, 1.0)));
    let (inp, control, _) = step(&mut ui, true, plain(events));
    check(
        !control.clicked
            && inp.input.buttons.is_empty()
            && inp.mouse_delta == (0.0, 0.0)
            && inp.input.events == vec![Event::PointerCancelled(MouseButton::LeftButton)],
        "new popup did not shield this frame's underlying controls",
    );
    let mut with_escape = vec![key_down(Key::Escape)];
    with_escape.extend(click.clone());
    let (inp, control, _) = step(&mut ui, true, plain(with_escape));
    check(
        !control.clicked && inp.input.events == vec![Event::PointerCancelled(MouseButton::LeftButton)],
        "popup dismissal leaked its remaining press/release events",
    );
    step(&mut ui, false, plain(vec![]));
    step(
        &mut ui,
        false,
        input_frame(vec![MouseButton::RightButton], vec![], vec![Event::PointerPressed(MouseButton::RightButton, (10.0, 10.0))]),
    );
    let (inp, _, _) = step(&mut ui, true, input_frame(vec![MouseButton::RightButton], vec![], vec![]));
    check(
        inp.input.events.contains(&Event::PointerCancelled(MouseButton::RightButton)),
        "popup opening did not cancel the viewport's captured gesture",
    );
    step(&mut ui, false, plain(vec![Event::FocusLost]));
    let (inp, _, _) = step(&mut ui, false, plain(vec![Event::FocusLost, Event::PointerCancelled(MouseButton::RightButton)]));
    check(
        inp.input.events == vec![Event::FocusLost, Event::PointerCancelled(MouseButton::RightButton)],
        "cancellation events were consumed",
    );
    let only_view = |ui: &mut Ui| -> i64 {
        let b = ui.box_("only-view", BoxOpts::default().flags(Flags::CLICKABLE).at((0.0, 0.0)).w(Size::Px(160.0)).h(Size::Px(240.0)));
        key_of_box(&b)
    };
    ui.frame(&plain(vec![]), &mut |ui| {
        only_view(ui);
    });
    let unowned_click = vec![press((200, 10)), release((200, 10))];
    let mut owner = 0;
    ui.frame(&plain(unowned_click.clone()), &mut |ui| owner = only_view(ui));
    check(
        ui.input(Some(owner)).input.events == vec![Event::PointerCancelled(MouseButton::LeftButton)],
        "unowned pointer events escaped to an explicit viewport owner",
    );
    check(ui.input(None).input.events == unowned_click, "default input lost unowned pointer events");
    let modal_y = |ui: &mut Ui, label: &str| -> f64 {
        let mut row = None;
        ui.frame(&plain(vec![]), &mut |ui| {
            ui.modal(label, &pxui_core::ui::Overlay::DEFAULT.name(label), &mut |ui| row = Some(ui.box_("row", BoxOpts::default().h(Size::Px(24.0)))));
        });
        f64::from(ui.rect(&row.expect("modal open")).y)
    };
    modal_y(&mut ui, "height-original");
    let centred = modal_y(&mut ui, "height-original");
    ui.frame(&plain(vec![]), &mut |_| {});
    ui.frame(&plain(vec![]), &mut |_| {});
    check(modal_y(&mut ui, "height-original") == centred, "closed modal lost its remembered height");
    for index in 0..64 {
        let label = format!("height-{}", index);
        modal_y(&mut ui, &label);
        modal_y(&mut ui, &label);
    }
    check(modal_y(&mut ui, "height-original") != centred, "dynamic modal keys did not evict old cached heights");
    let probe = |ui: &mut Ui| -> Vec<Key> {
        let b = ui.box_("press-keys", BoxOpts::default().flags(Flags::CLICKABLE).w(Size::Px(100.0)).h(Size::Px(100.0)));
        ui.press_keys(&b)
    };
    let mut seen = Vec::new();
    ui.frame(&plain(vec![]), &mut |ui| seen = probe(ui));
    let mut pressed = Vec::new();
    ui.frame(
        &input_frame(vec![MouseButton::LeftButton], vec![Key::Shift], vec![key_down(Key::Shift), press((10, 10))]),
        &mut |ui| pressed = probe(ui),
    );
    let mut held = Vec::new();
    ui.frame(
        &input_frame(vec![MouseButton::LeftButton], vec![], vec![Event::KeyReleased(Key::Shift), move_to((20, 10))]),
        &mut |ui| held = probe(ui),
    );
    let mut released = Vec::new();
    ui.frame(&plain(vec![release((20, 10))]), &mut |ui| released = probe(ui));
    check(pressed == vec![Key::Shift] && held == pressed && released == pressed, "captured press keys changed during motion or release");
    // capture_unclaimed: a blocking (non-clickable) child under the ancestor takes the press away
    // from it only where the child is clickable
    let unclaimed = |ui: &mut Ui| -> (Signal, Signal) {
        let outer = ui.box_("outer", BoxOpts::default().flags(Flags::CAPTURE_UNCLAIMED | Flags::BLOCKING).at((0.0, 0.0)).w(Size::Px(200.0)).h(Size::Px(200.0)));
        let saved = ui.enter(&outer);
        let taker = ui.box_("taker", BoxOpts::default().flags(Flags::CLICKABLE).at((100.0, 0.0)).w(Size::Px(50.0)).h(Size::Px(50.0)));
        ui.leave(saved);
        let saved = ui.enter(&outer);
        ui.box_("plain", BoxOpts::default().flags(Flags::SCROLL).at((0.0, 100.0)).w(Size::Px(50.0)).h(Size::Px(50.0)));
        ui.leave(saved);
        (ui.signal_owned(&outer), ui.signal_owned(&taker))
    };
    ui.frame(&plain(vec![]), &mut |ui| {
        unclaimed(ui);
    });
    let mut r = None;
    ui.frame(
        &input_frame(vec![MouseButton::LeftButton], vec![], vec![press((110, 10)), move_to((120, 10))]),
        &mut |ui| r = Some(unclaimed(ui)),
    );
    let (outer, taker) = r.take().unwrap();
    check(taker.pressed && !outer.pressed && !outer.held, "ancestor captured a consumed press");
    ui.frame(&plain(vec![release((120, 10))]), &mut |ui| {
        unclaimed(ui);
    });
    ui.frame(
        &input_frame(vec![MouseButton::LeftButton], vec![], vec![press((10, 110)), move_to((30, 90))]),
        &mut |ui| r = Some(unclaimed(ui)),
    );
    let (outer, _) = r.take().unwrap();
    check(
        outer.pressed && outer.held && outer.dragging && outer.drag == (20.0, -20.0) && outer.press_point == (10.0, 110.0),
        "ancestor did not capture an unclaimed press",
    );
    ui.frame(&plain(vec![release((60, 90))]), &mut |ui| r = Some(unclaimed(ui)));
    let (outer, _) = r.take().unwrap();
    check(outer.released && !outer.held && outer.release_point == (60.0, 90.0), "unclaimed capture did not release");
    let _ = seen;
    ui.destroy();
}

fn key_of_box(b: &UiBox) -> i64 {
    key(b)
}

/// Label typography through pxui_text: wrap, ellipsis with a line limit, alignment and a second
/// (icon) font addressed by code point.
pub fn window_pxui_typography() {
    use pxui_core::text::{Align, Style, Wrap};
    let mut ui = new_ui(None);
    let icons = ui.text_mut().add_font(include_bytes!("../../../../../assets/fonts/Pragmasevka-Regular.ttf")).unwrap() as u16;
    let body = Style { size: 13.0, ..Style::default() };
    let text = "one two three four five six";
    let mut metrics = Vec::new();
    ui.frame(&frame_at(vec![], 2.0, 0.0, vec![]), &mut |ui| {
        metrics.push(ui.text_metrics(&Style { wrap: Wrap::Word, ..body }, 60.0, text));
        metrics.push(ui.text_metrics(&Style { wrap: Wrap::Word, max_lines: 1, ellipsis: pxui_text::Ellipsis::End, ..body }, 60.0, text));
        let b = ui.box_("typography", BoxOpts::default().w(Size::Px(200.0)).h(Size::Px(40.0)).at((0.0, 0.0)));
        ui.draw(&b, move |paint, _| {
            paint.text_styled((0.0, 0.0), &Style { align: Align::Right, ..body }, 200.0, None, "end");
            paint.text_styled((0.0, 20.0), &Style { font: icons, ..body }, 0.0, None, "\u{2192}");
        });
    });
    let (wrapped, clipped) = (metrics[0], metrics[1]);
    assert!(wrapped.lines > 1 && wrapped.w <= 60.0, "wrap: {wrapped:?}");
    assert!(clipped.lines == 1 && clipped.w <= 60.0, "ellipsis on one line: {clipped:?}");
    let glyphs: Vec<_> = ui.output().draw[0].instances.iter().filter(|i| i.kind == pxui_core::draw::kind::MASK).collect();
    assert!(glyphs.len() == 4, "three right-aligned glyphs and one icon: {}", glyphs.len());
    assert!(glyphs[0].rect[0] > 150.0, "right alignment: first glyph at {}", glyphs[0].rect[0]);
    assert!(glyphs[3].rect[1] >= 20.0, "the icon sits on its own line");
    ui.destroy();
}

/// Dirty ranges stay sorted and disjoint; past the cap the closest two merge, never all of them.
fn dirty_ranges_merge_closest() {
    let mut r = Vec::new();
    for (a, b) in [(10, 11), (0, 1), (11, 12), (100, 101), (50, 51), (1000, 1001), (52, 53)] {
        pxui_core::draw::add_range(&mut r, a, b, 4);
    }
    // (11, 12) touches (10, 11); past four ranges the closest pair merges: (0, 1) with (10, 12), then 50 with 52
    assert_eq!(r, [(0, 12), (50, 53), (100, 101), (1000, 1001)]);
}

/// Fuzzy ranking: word starts and runs beat scattered bytes; no allocation.
fn fuzzy_score_ranks() {
    mem_sys::local_reset();
    let a = fuzzy_score("tr", "Transform");
    let b = fuzzy_score("tr", "scatter");
    let c = fuzzy_score("sc", "point/scale");
    let d = fuzzy_score("sc", "describe");
    let e = fuzzy_score("ps", "PointScale");
    check(mem_sys::local_alloc_count() == 0, "fuzzy_score allocated");
    check(a > b && c > d && e > fuzzy_score("ps", "perhaps"), &format!("{a:?} {b:?} {c:?} {d:?} {e:?}"));
    check(fuzzy_score("xyz", "Transform").is_none() && fuzzy_match("TRF", "transform") && !fuzzy_match("ft", "transform"), "matching");
    // the later word start is taken only when the rest still matches after it
    check(fuzzy_score("ab", "xab_a").is_some(), "lookahead broke a match");
}

/// Text rows are kept least recently used first out, up to `set_text_panes`.
fn text_rows_lru() {
    let mut ui = new_ui(None);
    ui.set_text_panes(2);
    let texts = ["a\nb", "c\nd", "e\nf"];
    let first = ui.text_rows(None, texts[0]);
    ui.text_rows(None, texts[1]);
    // reading the first again keeps it over the second when a third arrives
    check(Rc::ptr_eq(&first, &ui.text_rows(None, texts[0])), "a kept text walked again");
    ui.text_rows(None, texts[2]);
    check(Rc::ptr_eq(&first, &ui.text_rows(None, texts[0])), "the most recently used text was dropped");
    ui.destroy();
}

fn window_pxui_focused_text_field_warm_ime() {
    let mut ui = new_ui(Some(11));
    let mut build = |ui: &mut Ui| {
        ui.panel("editor", &pxui_core::ui::Overlay::DEFAULT.w_px(400.0).name("editor"), &mut |ui| {
            assert!(ui.text_field("Text", "café track 10").is_none());
            // Force arena growth after the editor captures its text ranges.
            for i in 0..128 {
                let b = ui.node_keyed(100 + i, Node::DEFAULT);
                let payload = [i; 32];
                ui.canvas(&b, move |_, _| { std::hint::black_box(payload); });
            }
        });
    };
    ui.frame(&keyboard_frame(vec![], vec![]), &mut build);
    ui.frame(&keyboard_frame(vec![], vec![key_down(Key::Tab)]), &mut build);
    check(ui.text_input_focused(), "Tab did not focus the text field");
    ui.frame(&keyboard_frame(vec![], vec![
        key_down(Key::Shift), key_down(Key::ArrowLeft), Event::KeyReleased(Key::Shift),
        Event::TextEditing { text: "é".into(), start: 0, length: 1 },
    ]), &mut build);
    let mut frame = keyboard_frame(vec![], vec![]);
    mem_sys::local_reset();
    for tick in 0..80 {
        frame.input.t = 1.0 + tick as f64 / 60.0;
        frame.input.size.0 = 500 + tick % 2;
        let before = mem_sys::local_alloc_count();
        ui.frame(&frame, &mut build);
        let allocations = mem_sys::local_alloc_count() - before;
        if tick >= 10 { check(allocations == 0, "focused text field allocated on a warm frame"); }
        check(ui.text_input_focused(), "warm rebuild lost text focus");
        let ((_, _, w, h), caret) = ui.output().platform.ime.expect("focused field must publish IME geometry");
        check(w > 0 && h > 0 && caret >= 0 && caret <= w, "invalid IME caret geometry");
    }
    // Selection and composition must survive the borrowed paint path; committing replaces the selection.
    let mut committed = None;
    ui.frame(&keyboard_frame(vec![], vec![Event::TextInput("é".into())]), &mut |ui| {
        ui.panel("editor", &pxui_core::ui::Overlay::DEFAULT.w_px(400.0).name("editor"), &mut |ui| {
            committed = ui.text_field("Text", "café track 10");
        });
    });
    check(committed.as_deref() == Some("café track 1é"), "IME commit lost the selection");
    ui.destroy();
}

fn window_pxui_keyboard_signal_reuses_routing_storage() {
    let mut ui = new_ui(Some(11));
    let button = Cell::new(None);
    let clicks = Cell::new(0);
    let mut build = |ui: &mut Ui| {
        let node = ui.node("button".into(), Node { flags: Flags::CLICKABLE | Flags::TAB_STOP, w: Size::Px(120.0), h: Size::Px(32.0), ..Node::DEFAULT });
        let state = ui.signal(&node);
        check(state.keys.is_empty(), "scalar signal copied key events");
        clicks.set(clicks.get() + i32::from(state.clicked));
        button.set(Some(node));
    };
    ui.frame(&keyboard_frame(vec![], vec![]), &mut build);
    ui.focus_box(&button.get().unwrap());
    let mut frame = keyboard_frame(vec![], vec![key_down(Key::Shift), key_down(Key::Space), Event::KeyReleased(Key::Space), Event::KeyReleased(Key::Shift)]);
    for tick in 0..60 {
        frame.input.t = 1.0 + tick as f64 / 60.0;
        mem_sys::local_reset();
        ui.frame(&frame, &mut build);
        if tick >= 5 { check(mem_sys::local_alloc_count() == 0, "warm numeric keyboard routing/state allocated"); }
        let events = ui.key_events_ref(&button.get().unwrap());
        check(events.len() == 4 && events[1].1.contains(&Key::Shift) && !events[2].1.contains(&Key::Space), "borrowed event modifiers changed");
    }
    check(clicks.get() == 60, "keyboard activation lost clicks");
    let owned = ui.signal_owned(&button.get().unwrap());
    check(owned.clicked && owned.keys.len() == 4, "legacy owned signal lost key events");
    let old = button.get().unwrap();
    ui.frame(&keyboard_frame(vec![], vec![]), &mut |_| {});
    check(ui.signal_owned(&old).keys.is_empty() && ui.key_events_ref(&old).is_empty(), "retired node retained routed input");
    ui.destroy();
}

pub const CASES: &[(&str, fn())] = &[
    ("menu_padding_name_and_the_current_choice", menu_padding_name_and_the_current_choice),
    ("line_box_glyphs_keep_their_half_leading", line_box_glyphs_keep_their_half_leading),
    ("paint_text_at_is_the_layout_top_and_measure_baseline_the_first_baseline", paint_text_at_is_the_layout_top_and_measure_baseline_the_first_baseline),
    ("paint_glyph_light_uses_each_text_size_and_density", paint_glyph_light_uses_each_text_size_and_density),
    ("line_box_fades_inherited_and_inline_colors", line_box_fades_inherited_and_inline_colors),
    ("window_pxui_keyboard_signal_reuses_routing_storage", window_pxui_keyboard_signal_reuses_routing_storage),
    ("window_pxui_focused_text_field_warm_ime", window_pxui_focused_text_field_warm_ime),
    ("text_rows_lru", text_rows_lru),
    ("fuzzy_score_ranks", fuzzy_score_ranks),
    ("dirty_ranges_merge_closest", dirty_ranges_merge_closest),
    ("window_pxui_typography", window_pxui_typography),
    ("window_pxui_table", window_pxui_table),
    ("window_pxui_keyboard", window_pxui_keyboard),
    ("window_pxui_input", window_pxui_input),
];

/// A line box places its first baseline its half-leading below its line's top: the glyphs of one line centred in a 30-point box sit
/// where they sit whatever the line height (a 12-point heading given its row's height as its line height no longer jumps up by half
/// the difference), and a box as tall as one 30-point line draws them there too.
fn line_box_glyphs_keep_their_half_leading() {
    let mut ui = new_ui(None);
    let frame = frame_at(vec![], 2.0, 0.0, vec![]);
    let glyphs = |ui: &mut Ui, line_height: f32, h: Size| {
        let mut rect = Rect::default();
        ui.frame(&frame, &mut |ui| {
            let node = ui.node("t".into(), Node { w: Size::Px(200.0), h, ..Node::DEFAULT });
            check(ui.text_line_box(&node, "Hgx", &pxui_text::Style { size: 12.0, line_height, ..Default::default() }, &[]), "a line box");
            rect = ui.rect(&node);
        });
        let rects: Vec<[f32; 4]> = ui.output().draw[0].instances.iter().filter(|i| i.kind == pxui_types::draw::kind::MASK).map(|i| i.rect).collect();
        (rect.h, rects)
    };
    let (_, natural) = glyphs(&mut ui, 30.0, Size::Px(30.0));
    check(natural.len() == 3, "three glyphs");
    let layouts = ui.text_layout_count();
    let (_, repeated) = glyphs(&mut ui, 30.0, Size::Px(30.0));
    check(repeated == natural && ui.text_layout_count() == layouts, "reusing the fixed line box changed its glyphs or reshaped it");
    // whole-point half-leadings land exactly; 16.8 floors 1.4 points of it, so it may sit up to a point higher
    for line_height in [12.0, 14.0, 20.0, 24.0, 16.8] {
        let (_, at) = glyphs(&mut ui, line_height, Size::Px(30.0));
        let close = at.len() == 3 && at.iter().zip(&natural).all(|(a, b)| a[0] == b[0] && (0.0..=1.0).contains(&(b[1] - a[1])) && a[3] - a[1] == b[3] - b[1]);
        check(close && (line_height == 16.8 || at == natural), &format!("line height {line_height} moved the glyphs: {at:?} vs {natural:?}"));
    }
    let (h, fit) = glyphs(&mut ui, 30.0, Size::Fit);
    check(h == 30.0 && fit == natural, &format!("a 30-point line's own box: {h} {fit:?} vs {natural:?}"));
    // the em box centred in the 30-point box: the cap height of the golden face around its middle
    let top = natural[0][1];
    check((9.0..=12.0).contains(&top), &format!("centred glyph top {top}"));
}

/// `Paint::text_styled`/`text_spans` (and so `label`, `chars`) take the layout's top: the first baseline sits `Metrics::baseline` below `at`, whatever the
/// line height, and a line box of the same style puts its glyphs on the same pixels as `text_spans` at the line's top.
fn paint_text_at_is_the_layout_top_and_measure_baseline_the_first_baseline() {
    let mut ui = new_ui(None);
    let frame = frame_at(vec![], 2.0, 0.0, vec![]);
    let mask = |ui: &Ui| -> Vec<[f32; 4]> { ui.output().draw[0].instances.iter().filter(|i| i.kind == pxui_types::draw::kind::MASK).map(|i| i.rect).collect() };
    let mut on_baseline = Vec::new();
    for line_height in [0.0, 12.0, 14.0, 20.0, 24.0, 16.8, 30.0] {
        let style = pxui_text::Style { size: 12.0, line_height, ..Default::default() };
        // `at = 40 - measure().baseline` puts the first baseline on y = 40 for every line height
        ui.frame(&frame, &mut |ui| {
            let b = ui.box_("measured", BoxOpts::default().w(Size::Px(200.0)).h(Size::Px(60.0)).at((0.0, 0.0)));
            ui.draw(&b, move |paint, _| {
                let baseline = paint.text_metrics(&style, 0.0, "Hgx").baseline;
                paint.text_styled((0.0, 40.0 - baseline), &style, 0.0, None, "Hgx");
            });
        });
        let glyphs = mask(&ui);
        check(glyphs.len() == 3, "three glyphs");
        on_baseline.push(glyphs);
    }
    for glyphs in &on_baseline[1..] {
        check(*glyphs == on_baseline[0], &format!("the first baseline moved with the line height: {glyphs:?} vs {:?}", on_baseline[0]));
    }
    // the same style in a line box as tall as its line, and drawn at that line's top, lands on the same pixels
    for line_height in [14.0, 20.0, 24.0, 30.0] {
        let style = pxui_text::Style { size: 12.0, line_height, ..Default::default() };
        let mut painted = Vec::new();
        for painter in 0..3 {
            ui.frame(&frame, &mut |ui| {
                let b = ui.box_("painted", BoxOpts::default().w(Size::Px(200.0)).h(Size::Px(line_height)).at((0.0, 0.0)));
                if painter == 0 {
                    ui.text_line_box(&b, "Hgx", &style, &[]);
                } else {
                    ui.draw(&b, move |paint, _| match painter {
                        1 => { paint.text_spans((0.0, 0.0), &style, 0.0, None, "Hgx", &[]); }
                        _ => { paint.text_styled((0.0, 0.0), &style, 0.0, None, "Hgx"); }
                    });
                }
            });
            painted.push(mask(&ui));
        }
        check(painted[0].len() == 3 && painted[0] == painted[1] && painted[0] == painted[2], &format!("line height {line_height}: line box {:?}, text_spans {:?}, text_styled {:?}", painted[0], painted[1], painted[2]));
    }
    ui.destroy();
}

fn paint_glyph_light_uses_each_text_size_and_density() {
    use pxui_types::draw::{flags, kind, Instance};
    let mut ui = Ui::create(input::Platform::Apple, None, crate::support::golden_text());
    let sizes = [14.0, 12.0, 14.0, 22.0, 12.0];
    for density in [1.0_f32, 2.0, 3.0] {
        ui.frame(&frame_at(vec![], f64::from(density), 0.0, vec![]), &mut |ui| {
            let b = ui.box_("ink", BoxOpts::default().w(Size::Px(200.0)).h(Size::Px(60.0)));
            ui.draw(&b, move |paint, _| {
                for (i, size) in sizes.into_iter().enumerate() {
                    let style = pxui_text::Style { size, ..Default::default() };
                    paint.text_styled((i as f32 * 30.0, 0.0), &style, 0.0, None, "H");
                }
            });
        });
        let extras = ui.output().draw[0].instances.iter().filter(|i| i.kind == kind::MASK && i.flags & flags::mask::LIGHT != 0).map(|i| i.extra);
        assert!(extras.eq(sizes.map(|size| Instance::light_extra(size * density, density))));
    }
    ui.destroy();
}

fn line_box_fades_inherited_and_inline_colors() {
    use pxui_core::geom::Rgba;
    use pxui_text::TextSpan;
    use pxui_types::draw::kind;
    let mut ui = new_ui(None);
    let style = pxui_text::Style { size: 14.0, line_height: 20.0, ..Default::default() };
    let span = TextSpan {
        start: 1,
        end: 2,
        flags: TextSpan::COLOR | TextSpan::BACKGROUND | TextSpan::UNDERLINE,
        color: Rgba::rgba(5, 6, 7, 201),
        background: Rgba::rgba(10, 20, 30, 100),
        underline: Rgba::rgba(50, 60, 70, 90),
        ..Default::default()
    };
    let mut build = |ui: &mut Ui| {
        let handle = ui.node_ref("line", &Node {
            w: Size::Px(100.0), h: Size::Px(20.0),
            look: Look { text: Rgba::rgb(200, 10, 20), opacity: 0.5, ..Look::NONE },
            ..Node::DEFAULT
        });
        assert!(ui.text_line_box(&handle, "AB", &style, &[span]));
    };
    let frame = frame_at(vec![], 2.0, 0.0, vec![]);
    ui.frame(&frame, &mut build);
    ui.frame(&frame, &mut build);
    mem_sys::local_reset();
    ui.frame(&frame, &mut build);
    assert_eq!(mem_sys::local_alloc_count(), 0);
    let drawn: Vec<_> = ui.output().draw[0].instances.iter()
        .map(|i| (i.kind, i.color)).collect();
    assert_eq!(drawn, [
        (kind::RECT, [10, 20, 30, 50]),
        (kind::MASK, [200, 10, 20, 128]),
        (kind::MASK, [5, 6, 7, 101]),
        (kind::RECT, [50, 60, 70, 45]),
    ]);
    ui.destroy();
}

/// `Menu::padding` moves the first row down and grows the sheet by twice the change; `Menu::name` names the sheet (none without, never the
/// key); a `MenuItem::selected` row reads CHECKED and SELECTED and the built-in row shows the theme's `checked` menu look.
fn menu_padding_name_and_the_current_choice() {
    use pxui_core::a11y::state as st;
    const ITEMS: [MenuItem; 2] = [MenuItem::new("4 GB"), MenuItem::new("8 GB").selected(true)];
    let mut ui = new_ui(None);
    ui.set_a11y(true);
    let seen = |ui: &mut Ui, menu: Menu<'static>| {
        for _ in 0..2 { ui.frame(&frame_at(vec![], 1.0, 0.0, vec![]), &mut |ui| { ui.menu("sizes".into(), pxui_types::geom::Point::new(20.0, 20.0), &ITEMS, &menu); }); }
        let tree = ui.output().a11y;
        let sheet = *tree.nodes.iter().find(|n| n.role == Role::Menu).expect("the sheet");
        let row = |name: &str| *tree.nodes.iter().find(|n| n.role == Role::MenuItem && tree.str(n.name) == name).expect("a row");
        (tree.str(sheet.name).to_owned(), sheet.rect, row("4 GB"), row("8 GB"))
    };
    let (name, sheet, plain, current) = seen(&mut ui, Menu::DEFAULT);
    check(name.is_empty(), &format!("an unnamed menu is named {name:?}"));
    check(plain.states & (st::CHECKED | st::SELECTED) == 0 && current.states & (st::CHECKED | st::SELECTED) == st::CHECKED | st::SELECTED, "the current choice's states");
    let b = ui.box_of_a11y(current.id).expect("the current row's node");
    let checked = ui.theme().menu.checked;
    check(ui.node_mut(&b).look == checked, "the built-in row shows the theme's checked menu look");
    let (named, roomy, first, _) = seen(&mut ui, Menu::DEFAULT.padding(10.0).name("Cache size"));
    check(named == "Cache size", &format!("the named menu is {named:?}"));
    check(first.rect.y - plain.rect.y == 4.0 && roomy.h - sheet.h == 8.0, &format!("padding 10: rows {} -> {}, sheet {} -> {}", plain.rect.y, first.rect.y, sheet.h, roomy.h));
}
