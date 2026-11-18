//! A picker and an inspector head through the wrapper and the library allocate what the engine's own calls do in the same settled
//! frames, plus one: the owned copy of the head's title (the engine hands back a `String`; the library lends its text across the ABI and
//! the wrapper's owned `Head` copies it). The engine's own allocations in these two widgets (keys, the title, the action list) are
//! counted here, not hidden: the equality below shows the ABI adds nothing to them.
use input::{frame, Event, Platform};
use mem_sys as _;
use pxui_core::ui::{HeadAction, Overlay, PickRows, PickerOpts, Ui as Engine};
use pxui::FrameIn;
use std::rc::Rc;

fn fill(_: &str, rows: &mut PickRows) {
    for (label, detail) in [("one", "1"), ("two", "2")] { rows.push(label, detail); }
}

fn engine_allocations() -> u64 {
    let mut ui = Engine::create(Platform::Apple, None, pxui_core::text::Text::new(&[pxui_core::text::PRAGMASEVKA]));
    let actions = [HeadAction { caption: "Run".into(), keycap: "R".into(), active: false, usable: true }];
    let rename: Rc<dyn Fn(&str) -> bool> = Rc::new(|name: &str| !name.is_empty());
    let mut worst = 0;
    for count in 0..12 {
        let mut input = input::at_time(count as f64 / 60.0);
        input.size = (320, 240);
        input.pointer = (-100.0, -100.0);
        input.events = vec![Event::PointerMoved((-100.0, -100.0))];
        let frame = frame::Frame { input, pixel_scale: (1.0, 1.0), fps: 60.0, mouse_delta: (0.0, 0.0) };
        let mut measured = 0;
        ui.frame(&frame, &mut |ui| ui.panel("panel", &Overlay::DEFAULT, &mut |ui| {
            let before = mem_sys::local_alloc_count();
            let _ = ui.picker("filter", "filter", "", &PickerOpts::default(), &mut fill);
            let _ = ui.inspector_header("head", Some("kind"), None, Some("1"), Some(Rc::clone(&rename)), &actions, Some("reset"), true, "Title", "detail");
            measured = mem_sys::local_alloc_count() - before;
        }));
        if count >= 8 { worst = worst.max(measured); }
    }
    worst
}

fn wrapper_allocations() -> u64 {
    let mut ui = pxui::Ui::new(&pxui::UiDesc::new(pxui::Platform::Apple).fonts(&[pxui::FontData(pxui_core::text::PRAGMASEVKA)])).expect("pxui Ui");
    let actions = [pxui::ui::HeadAction { caption: "Run".into(), keycap: "R".into(), active: false, usable: true }];
    let rename: Rc<dyn Fn(&str) -> bool> = Rc::new(|name: &str| !name.is_empty());
    let mut worst = 0;
    for count in 0..12 {
        let input = FrameIn::new(320.0, 240.0).scale(1.0).time(count as f64 / 60.0);
        let mut measured = 0;
        ui.frame(&input, &mut |ui: &mut pxui::Ui| ui.panel("panel", &pxui::Overlay::DEFAULT, &mut |ui: &mut pxui::Ui| {
            let before = mem_sys::local_alloc_count();
            let _ = ui.picker("filter", "filter", "", &pxui::PickerOpts::DEFAULT, &mut fill);
            let _ = ui.inspector_header("head", Some("kind"), None, Some("1"), Some(Rc::clone(&rename)), &actions, Some("reset"), true, "Title", "detail");
            measured = mem_sys::local_alloc_count() - before;
        })).expect("frame");
        if count >= 8 { worst = worst.max(measured); }
    }
    worst
}

#[test]
fn a_picker_and_a_head_allocate_what_the_engine_does() {
    mem_sys::local_reset();
    let counted = mem_sys::local_alloc_count();
    drop(std::hint::black_box(Box::new(1u64)));
    assert!(mem_sys::local_alloc_count() > counted, "the counting allocator is installed");
    let (engine, wrapper) = (engine_allocations(), wrapper_allocations());
    println!("settled picker + head: engine {engine} allocations, wrapper {wrapper}");
    assert_eq!(wrapper, engine + 1, "the library and the wrapper add only the owned copy of the title to the engine's own");
}
