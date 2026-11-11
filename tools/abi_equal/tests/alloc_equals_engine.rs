//! A picker and an inspector head through the wrapper and the library allocate what the engine's own calls do in the same settled
//! frames, plus one: the owned copy of the head's title (the engine hands back a `String`; the library lends its text across the ABI and
//! the wrapper's owned `Head` copies it). The engine's own allocations in these two widgets (keys, the title, the action list) are
//! counted here, not hidden: the equality below shows the ABI adds nothing to them.
use input::{frame, Event, Platform};
use mem_sys as _;
use pxui_core::ui::{HeadAction, PanelOpts, PickerOpts, Ui as Engine};
use pxui_types::FrameIn;
use std::rc::Rc;

fn fill(_: &str, rows: &mut Vec<(String, String)>) {
    rows.truncate(2);
    for (at, (label, detail)) in [("one", "1"), ("two", "2")].into_iter().enumerate() {
        match rows.get_mut(at) {
            Some(row) => { row.0.clear(); row.0.push_str(label); row.1.clear(); row.1.push_str(detail); }
            None => rows.push((label.into(), detail.into())),
        }
    }
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
        ui.frame(&frame, &mut |ui| ui.panel(PanelOpts::default(), "panel", &mut |ui| {
            let before = mem_sys::local_alloc_count();
            let _ = ui.picker_filled(&PickerOpts::default(), "filter", "", &mut fill);
            let _ = ui.inspector_header("head", Some("kind"), None, Some("1"), Some(Rc::clone(&rename)), &actions, Some("reset"), true, "Title", "detail");
            measured = mem_sys::local_alloc_count() - before;
        }));
        if count >= 8 { worst = worst.max(measured); }
    }
    worst
}

fn wrapper_allocations() -> u64 {
    let mut ui = pxui::Ui::create(pxui::Platform::Apple, None, &[pxui_core::text::PRAGMASEVKA]);
    let actions = [pxui::ui::HeadAction { caption: "Run".into(), keycap: "R".into(), active: false, usable: true }];
    let rename: Rc<dyn Fn(&str) -> bool> = Rc::new(|name: &str| !name.is_empty());
    let mut worst = 0;
    for count in 0..12 {
        let input = FrameIn { width: 320.0, height: 240.0, time: count as f64 / 60.0, scale: 1.0, ..FrameIn::default() };
        let mut measured = 0;
        ui.frame(&input, &mut |ui: &mut pxui::Ui| ui.panel(pxui::ui::PanelOpts::default(), "panel", &mut |ui: &mut pxui::Ui| {
            let before = mem_sys::local_alloc_count();
            let _ = ui.picker_filled(&pxui::ui::PickerOpts::default(), "filter", "", &mut fill);
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
