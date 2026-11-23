//! The engine-shaped surface through the wrapper: queries, boxes, patches, scroll and text state, carries, view calls
//! and the loud failure of a caller bug.
use crate::{Fill, Stroke, Ui};
use crate::FrameIn;
use pxui_types::{geom::Rect, ui::{Cursor, UndoKind}, Mark, Node, Size};
use std::panic::{catch_unwind, AssertUnwindSafe};


fn input() -> FrameIn<'static> { FrameIn::new(320.0, 240.0) }

/// How far below a canvas draw's `at` (the layout's top) the same style's node text starts: node text is placed by its em box, so a
/// canvas that must land on the same pixels draws at the node's top plus this.
fn em_top_to_layout_top(points:f32)->f32 {
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let label=crate::Label::BODY.size(points);
    ui.frame(&input(),&mut |ui| {
        let b=ui.node("probe",crate::Node::DEFAULT.w_px(200.0).h_px(points));
        ui.text_spans(&b,"H",label,&[]).unwrap();
        ui.canvas(&b,move |paint,rect| {paint.text_spans(crate::Point::new(rect.x,rect.y),label,0.0,"H",&[]).unwrap();});
    }).unwrap();
    let ys:Vec<f32>=ui.layers().flat_map(|layer|layer.instances.iter().map(|i|i.rect[1])).collect();
    assert_eq!(ys.len(),2,"one node glyph and one canvas glyph");
    ys[0]-ys[1]
}

#[test]
fn prepared_text_moves_between_worker_and_ui_without_shaping_or_allocating_on_install() {
    use crate::{PreparedText,RetiredText,TextSpan};
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut worker=ui.text_service().fork().unwrap();
    let label=crate::Label::BODY;
    let style=ui.resolve(&label);
    let spans=[{ let mut record = TextSpan::new(0, 1); record.weight = 700; record }];
    let records=std::thread::spawn(move || {
        let (width,height,pixels)={ let mut px=Vec::new(); worker.rasterize_rgba("forked",&style,200.0,1.0,[255;4], &mut px).map(|(w,h)|(w,h,px)) }.unwrap();
        assert!(width>0 && height>0 && pixels.chunks_exact(4).any(|pixel|pixel[3]>0),"a fork retains standalone rasterization");
        (0..64).map(|index| {
            let text=format!("Track {index:05}");
            let record=worker.prepare(&text,&style,200.0,&spans).unwrap();
            (text,record)
        }).collect::<Vec<_>>()
    }).join().unwrap();
    let mut other=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let _=other.text_service().fork().unwrap();
    let mut records=records.into_iter();
    let (first_text,first)=records.next().unwrap();
    let rejected=other.text_service().install(first);
    let (code,first)=match rejected {Err(error)=>error,Ok(_)=>panic!("foreign fonts accepted")};
    assert_eq!(code,crate::Error::State);
    let records=std::iter::once((first_text,first)).chain(records).collect::<Vec<_>>();
    let mut retired:Vec<RetiredText>=Vec::with_capacity(records.len());
    let layouts=ui.text_layout_count();
    mem_sys::local_reset();
    for (text,record) in records {
        let expected=record.metrics();
        let storage=match ui.text_service().install(record) {Ok(storage)=>storage,Err((code,_))=>panic!("installation failed: {code}")};
        retired.push(storage);
        let actual=ui.text_service().measure_spans(&text,label,Some(200.0),&spans).unwrap();
        assert_eq!((actual.width,actual.height,actual.baseline,actual.lines),(expected.width,expected.height,expected.baseline,expected.lines));
    }
    assert_eq!(ui.text_layout_count(),layouts);
    assert_eq!(mem_sys::local_alloc_count(),0);
    ui.frame(&input(),&mut |ui| {
        let node=ui.node("prepared-label",crate::Node::DEFAULT.w_px(200.0).h_px(30.0));
        ui.text_spans(&node,"Track 00000",label,&spans).unwrap();
    }).unwrap();
    assert_eq!(ui.text_layout_count(),layouts,"ordinary node drawing must use the prepared content key");
    std::thread::spawn(move || drop(retired)).join().unwrap();
    let mut worker=ui.text_service().fork().unwrap();
    let stale:PreparedText=std::thread::spawn(move || worker.prepare("stale",&style,200.0,&[]).unwrap()).join().unwrap();
    assert!(ui.text_service().add_font(crate::pragmasevka()).is_ok());
    let (code,stale)=match ui.text_service().install(stale) {Err(error)=>error,Ok(_)=>panic!("stale fonts accepted")};
    assert_eq!(code,crate::Error::State);
    std::thread::spawn(move || drop(stale)).join().unwrap();
}

#[cfg(any(target_os="macos",target_os="ios"))]
#[test]
fn phase_cpu_timing_is_opt_in_and_zero_warm_allocation() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    assert_eq!(ui.phase_cpu_times(), Err(crate::Error::State));
    ui.set_timing(true);
    for round in 0..20 {
        mem_sys::local_reset();
        ui.frame(&input(), &mut |_| {
            let mut value = 1u64;
            for index in 1..5000 {
                value = std::hint::black_box(value.wrapping_mul(index));
            }
            std::hint::black_box(value);
        })
        .unwrap();
        let phases = ui.phase_cpu_times().unwrap();
        assert!(phases[1] > std::time::Duration::ZERO);
        if round > 2 {
            assert_eq!(mem_sys::local_alloc_count(), 0);
        }
    }
    ui.set_timing(false);
    assert_eq!(ui.phase_cpu_times(), Err(crate::Error::State));
    ui.frame(&input(), &mut |_| {}).unwrap();
    assert_eq!(ui.phase_cpu_times(), Err(crate::Error::State));
}

#[test]
fn text_layout_count_distinguishes_cold_content_from_retained_warm_frames_without_allocations() {
    use crate::ui::Node;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    assert_eq!(ui.text_layout_count(),0);
    let mut build=|ui:&mut Ui| {
        let node=ui.node("label",Node::DEFAULT.w_px(200.0).h_px(30.0));
        ui.text(&node,"Cache identity");
    };
    ui.frame(&input(),&mut build).unwrap();
    let first=ui.text_layout_count();
    assert!(first>0);
    for round in 0..20 {
        mem_sys::local_reset();
        ui.frame(&input(),&mut build).unwrap();
        assert_eq!(ui.text_layout_count(),first);
        if round>2 {assert_eq!(mem_sys::local_alloc_count(),0);}
    }
    ui.frame(&input(),&mut |ui| {
        let node=ui.node("label",Node::DEFAULT.w_px(200.0).h_px(30.0));
        ui.text(&node,"Replacement identity");
    }).unwrap();
    assert!(ui.text_layout_count()>first);
}

#[test]
fn hash_of_borrows_its_input_and_allocates_nothing() {
    for text in ["", "café", "a longer library identity with more than eight bytes"] {
        let value=(42u64,true,Some(text));
        let expected=crate::hash_of(&value);
        let owned=text.to_owned();
        assert_eq!(crate::hash_of(&owned),crate::hash_of(text));
        assert_eq!(owned,text,"the input stays borrowed");
        mem_sys::local_reset();
        for _ in 0..100 {assert_eq!(crate::hash_of(&value),expected);}
        assert_eq!(mem_sys::local_alloc_count(),0);
    }
}

#[test]
fn reveal_uses_fixed_measured_section_and_grid_geometry_without_warm_allocations() {
    use crate::ui::{Node, SectionList, SectionPart, Look};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    for kind in 0..4 {
        let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
        ui.set_dir(kind==3);
        let body=Cell::new(None);let count=Cell::new(10_000);let requested=Cell::new(None);
        let size=Cell::new(100.0);let calls=Cell::new(0);
        // each row's key as the build last declared it (cached views keep rows the build does not visit again)
        let keys:Vec<Cell<i64>>=(0..10_000).map(|_| Cell::new(0)).collect();
        let row_key=|index:usize| keys[index].get();
        let mut build=|ui:&mut Ui| {
            ui.id("reveal-page", |ui| { ui.column(Node::column().w_px(size.get()).h_px(size.get()), |ui| { ui.view_of("reveal-page", &(count.get() as u64), |ui| {
                let mut row=|ui:&mut Ui,index:usize| {
                    calls.set(calls.get()+1);
                    let h=if kind==1 || kind==2 {10.0+(index%3) as f32*20.0} else {20.0};
                    let row=ui.node(index as i64,Node::DEFAULT.w_px(25.0).h_px(h).look(Look::NONE.fill(crate::geom::Rgba::rgb(255, 0, 0))));
                    keys[index].set(row.box_key);
                };
                let section=SectionList::new(count.get()).row_h(20.0).lead(35.0).header(14.0).trail(11.0).overlay_scrollbar();
                let list=match kind {
                    0=>ui.section_list("rows",&section,&|i| i as i64,&mut |ui,part,_| if let SectionPart::Row(index)=part {row(ui,index)}).handle,
                    1=>ui.section_list("rows",&SectionList::new(count.get()).row_h(20.0).measured(),&|i| i as i64,&mut |ui,part,_| if let SectionPart::Row(index)=part {row(ui,index)}).handle,
                    2=>ui.section_list("rows",&section.measured(),&|i| i as i64,&mut |ui,part,_| if let SectionPart::Row(index)=part {row(ui,index)}).handle,
                    _=>ui.grid_list("rows",count.get(),&crate::GridList::DEFAULT.w_px(25.0).h_px(20.0),&mut row),
                };
                ui.set_node_layout(&list,&Node::DEFAULT.w(Size::Grow).h(Size::Grow).padding(crate::geom::Edges::all(6.0)));
                body.set(Some(list));
                if let Some(index)=requested.take() {ui.reveal(&list,index);}
            }); }); });
        };
        ui.frame(&input(),&mut build).unwrap();
        ui.frame(&input(),&mut build).unwrap();
        if kind==0 {
            for (index, expected) in [(1,1.0),(0,1.0),(4,61.0),(3,61.0),(0,35.0)] {
                ui.reveal(&body.get().unwrap(),index);
                ui.frame(&input(),&mut build).unwrap();
                assert_eq!(ui.scroll(&body.get().unwrap()).settled,expected,"minimum adjustment for row {index}");
            }
        }
        for round in 0..5 {
            mem_sys::local_reset();
            for index in [0,5_000,9_999,5_010,1,9_990,4,5_000,5_000] {
                calls.set(0);
                ui.reveal(&body.get().unwrap(),index);
                let allocations=mem_sys::local_alloc_count();
                ui.frame(&input(),&mut build).unwrap();
                let list=body.get().unwrap();
                let r=ui.rect(&ui.box_of_key(crate::Key::Resolved(row_key(index))).unwrap());let viewport=ui.rect(&list);
                let header=if kind==0 || kind==2 {14.0} else {0.0};
                assert!(r.y>=viewport.y+6.0+header-0.05 && r.y+r.h<=viewport.y+viewport.h-5.95,"kind {kind}, index {index}: {r:?} in {viewport:?}");
                assert!(calls.get()<100,"reveal must not build all 10,000 rows");
                let offset=ui.scroll(&list).settled;
                ui.reveal(&list,index);
                assert_eq!(ui.scroll(&list).settled,offset,"visible rows need no additional scrolling");
                if round>1 {assert_eq!(allocations,0,"the request itself allocates nothing");assert_eq!(mem_sys::local_alloc_count(),0,"warm reveal/build/draw kind {kind}");}
            }
        }
        for _ in 0..605 {ui.frame(&input(),&mut build).unwrap();}
        mem_sys::local_reset();
        ui.reveal(&body.get().unwrap(),1);
        ui.frame(&input(),&mut build).unwrap();
        assert!(ui.box_of_key(crate::Key::Resolved(row_key(1))).is_some(),"live cached-list geometry must survive cache expiry");
        assert_eq!(mem_sys::local_alloc_count(),0);
        let list=body.get().unwrap();let offset=ui.scroll(&list).settled;
        for index in [10_000,usize::MAX-1,usize::MAX] {ui.reveal(&list,index);}
        assert_eq!(ui.scroll(&list).settled,offset);
        // A request made after the builder must settle in the same frame with the new viewport.
        size.set(80.0);requested.set(Some(9_999));
        ui.frame(&input(),&mut build).unwrap();
        let list=body.get().unwrap();let r=ui.rect(&ui.box_of_key(crate::Key::Resolved(row_key(9_999))).unwrap_or_else(||panic!("resize kind {kind}, offset {}, request {:?}",ui.scroll(&list).settled,requested.get())));let v=ui.rect(&list);
        assert!(r.y>=v.y+5.95 && r.y+r.h<=v.y+v.h-5.95);
        count.set(0);ui.frame(&input(),&mut build).unwrap();
        assert_eq!(ui.scroll(&body.get().unwrap()).settled,0.0);
        ui.frame(&input(),&mut |_| {}).unwrap();
        ui.reveal(&list,5);
    }
}

#[test]
fn gestures_swipe_long_press_and_stale_handles_through_the_abi_without_warm_allocations() {
    use crate::{Event, GestureOpts, GestureState, SwipeActions, Touch, Str, ui::Size};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let state = Cell::new(GestureState::default());
    let node = Cell::new(None);
    let mut time = 0.0;
    let mut build = |ui: &mut Ui| {
        let row = ui.node("row", Node::DEFAULT.w(Size::Px(200.0)).h(Size::Px(50.0)).flags(pxui_types::Flags::CLICKABLE).at(0.0, 0.0));
        node.set(Some(row));
        state.set(ui.gesture(&row, &GestureOpts::DEFAULT.long_press().swipe_actions(SwipeActions::DEFAULT.leading(60.0).trailing(80.0).full(120.0))));
    };
    let touch = |ui: &mut Ui, phase: u32, x: f64, at: f64, build: &mut dyn FnMut(&mut Ui)| {
        let packet = Touch::new(1, 7, [x, 25.0], at, 1.0, phase);
        let event = { let mut record = Event::default(); record.kind = Event::TOUCH; record.text = { let mut record = Str::default(); record.ptr = (&packet as *const Touch).cast(); record.len = core::mem::size_of::<Touch>(); record }; record };
        ui.frame(&{
            let mut frame = input().time(at);
            frame.events = core::slice::from_ref(&event);
            frame
        }, build).unwrap();
    };
    ui.frame(&input().time(time), &mut build).unwrap();
    for round in 0..4 {
        time = f64::from(round) * 20.0 + 1.0;
        mem_sys::local_reset();
        touch(&mut ui, 0, 150.0, time, &mut build);
        touch(&mut ui, 1, 142.0, time + 0.016, &mut build);
        assert_eq!(state.get().swipe, -8.0);
        touch(&mut ui, 1, 100.0, time + 0.032, &mut build);
        assert_eq!((state.get().swiping, state.get().swipe), (true, -50.0));
        touch(&mut ui, 2, 100.0, time + 0.4, &mut build);
        ui.frame(&input().time(time + 0.5), &mut build).unwrap();
        assert!(state.get().swipe_open && !state.get().swiping);
        ui.frame(&input().time(time + 2.0), &mut build).unwrap();
        assert_eq!(state.get().swipe, -80.0);
        ui.close_swipe(&node.get().unwrap());
        ui.frame(&input().time(time + 3.0), &mut build).unwrap();
        ui.frame(&input().time(time + 4.0), &mut build).unwrap();
        assert_eq!(state.get().swipe, 0.0);
        // a held touch: highlight then long press come from frames alone
        touch(&mut ui, 0, 100.0, time + 5.0, &mut build);
        ui.frame(&input().time(time + 5.2), &mut build).unwrap();
        assert!(state.get().highlighted && !state.get().long_pressed);
        ui.frame(&input().time(time + 5.6), &mut build).unwrap();
        assert!(state.get().long_pressed && !state.get().highlighted);
        touch(&mut ui, 2, 100.0, time + 5.7, &mut build);
        // leading side opens; a full swipe commits each side for one frame and settles closed
        touch(&mut ui, 0, 100.0, time + 8.1, &mut build);
        touch(&mut ui, 1, 108.0, time + 8.116, &mut build);
        touch(&mut ui, 1, 150.0, time + 8.132, &mut build);
        assert_eq!((state.get().swiping, state.get().swipe), (true, 50.0));
        touch(&mut ui, 2, 150.0, time + 8.5, &mut build);
        ui.frame(&input().time(time + 10.0), &mut build).unwrap();
        let open = state.get();
        assert!(open.swipe_open && open.swipe_open_leading && !open.swipe_open_trailing && open.swipe == 60.0 && !open.swipe_commit_leading);
        ui.close_swipe(&node.get().unwrap());
        ui.frame(&input().time(time + 10.1), &mut build).unwrap();
        ui.frame(&input().time(time + 11.0), &mut build).unwrap();
        assert_eq!(state.get().swipe, 0.0);
        for (i, (end, trailing)) in [(20.0, true), (280.0, false)].into_iter().enumerate() {
            let t = time + 12.0 + 3.0 * i as f64;
            touch(&mut ui, 0, 150.0, t, &mut build);
            touch(&mut ui, 1, 150.0 + (end - 150.0) * 0.1, t + 0.016, &mut build);
            touch(&mut ui, 1, end, t + 0.032, &mut build);
            touch(&mut ui, 2, end, t + 0.5, &mut build);
            let done = state.get();
            assert_eq!((done.swipe_commit_trailing, done.swipe_commit_leading, done.swipe_open), (trailing, !trailing, false));
            ui.frame(&input().time(t + 0.6), &mut build).unwrap();
            assert!(!state.get().swipe_commit_trailing && !state.get().swipe_commit_leading);
            ui.frame(&input().time(t + 2.0), &mut build).unwrap();
            assert_eq!(state.get().swipe, 0.0);
        }
        if round > 1 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm gesture frames"); }
    }
    let stale = node.get().unwrap();
    ui.frame(&input().time(time), &mut |_| {}).unwrap();
    assert_eq!(ui.gesture_state(&stale), GestureState::default(), "a retired node reads as rest");
    ui.close_swipe(&stale);
}

#[test]
fn dismiss_drags_commits_and_springs_back_through_the_abi_without_warm_allocations() {
    use crate::{DismissOpts, Event, GestureOpts, GestureState, Touch, Str, ui::Size};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let state = Cell::new(GestureState::default());
    let mut build = |ui: &mut Ui| {
        let sheet = ui.node("sheet", Node::DEFAULT.w(Size::Px(200.0)).h(Size::Px(200.0)).flags(pxui_types::Flags::CLICKABLE).at(0.0, 0.0));
        state.set(ui.gesture(&sheet, &GestureOpts::DEFAULT.dismiss(DismissOpts::DEFAULT)));
    };
    let touch = |ui: &mut Ui, phase: u32, y: f64, at: f64, build: &mut dyn FnMut(&mut Ui)| {
        let packet = Touch::new(1, 7, [100.0, y], at, 1.0, phase);
        let event = { let mut record = Event::default(); record.kind = Event::TOUCH; record.text = { let mut record = Str::default(); record.ptr = (&packet as *const Touch).cast(); record.len = core::mem::size_of::<Touch>(); record }; record };
        ui.frame(&{
            let mut frame = input().time(at);
            frame.events = core::slice::from_ref(&event);
            frame
        }, build).unwrap();
    };
    ui.frame(&input().time(0.0), &mut build).unwrap();
    for round in 0..4 {
        let t = f64::from(round) * 20.0 + 1.0;
        mem_sys::local_reset();
        for (i, end) in [100.0, 50.0].into_iter().enumerate() {
            let t = t + 8.0 * i as f64;
            touch(&mut ui, 0, 20.0, t, &mut build);
            touch(&mut ui, 1, 32.0, t + 0.016, &mut build);
            touch(&mut ui, 1, end, t + 0.032, &mut build);
            assert_eq!(state.get().dismiss, end as f32 - 32.0);
            assert!(state.get().dismissing);
            touch(&mut ui, 2, end, t + 0.5, &mut build);
            let lifted = state.get();
            assert_eq!((lifted.dismiss_committed, lifted.dismiss_cancelled), (i == 0, i == 1));
            ui.frame(&input().time(t + 0.6), &mut build).unwrap();
            assert!(!state.get().dismiss_committed && !state.get().dismiss_cancelled);
            ui.frame(&input().time(t + 3.0), &mut build).unwrap();
            assert_eq!((state.get().dismiss, state.get().dismissing), (if i == 0 { 200.0 } else { 0.0 }, false));
            if i == 0 { ui.frame(&input().time(t + 4.0), &mut |_| {}).unwrap(); ui.frame(&input().time(t + 4.1), &mut build).unwrap(); }
        }
        if round > 1 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm dismiss frames"); }
    }
}

#[test]
fn scrub_previews_capture_commits_release_and_routes_keys_a11y_touch_without_warm_allocations() {
    use crate::{Event, SliderResponse, Touch, Str, ui::{Node, Role}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.set_a11y(true);
    let value = Cell::new(40.0);
    let range = Cell::new((10.0, 110.0));
    let result = Cell::new(None::<SliderResponse>);
    let popup = Cell::new(false);
    let present = Cell::new(true);
    let mut build = |ui: &mut Ui| {
        if popup.get() { ui.popup("scrub-popup", &crate::Overlay::DEFAULT.at(crate::Point::new(0.0, 0.0)).w_px(200.0).h_px(30.0), |_| {}); }
        if !present.get() { result.set(None); return; }
        let (lo, hi) = range.get();
        let scrub = ui.scrub("position", value.get(), lo..=hi, &crate::Scrubber::DEFAULT.step(5.0).name("Position"));
        ui.set_node_layout(&scrub.response.handle, &Node::DEFAULT.w_px(200.0).h_px(20.0));
        result.set(Some(scrub));
    };
    let pointer = |kind, x, code| Event::pointer(kind, code, x, 10.0);
    let key = |index: crate::KeyCode| Event::key(Event::KEY_PRESSED, index.wire());
    ui.frame(&input(), &mut build).unwrap();
    let mut identity = ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == "Position").unwrap().id;
    let mut time = 0.0;
    for round in 0..8 {
        mem_sys::local_reset();
        for (event, preview, commit, held) in [
            (pointer(Event::POINTER_PRESSED, 50.0, 0), Some(35.0), None, true),
            (pointer(Event::POINTER_MOVED, 250.0, 0), Some(110.0), None, true),
            (pointer(Event::POINTER_RELEASED, 150.0, 0), Some(85.0), Some(85.0), false),
            (pointer(Event::POINTER_PRESSED, 50.0, 1), None, None, false),
            (pointer(Event::POINTER_RELEASED, 50.0, 1), None, None, false),
            // a mouse scrub leaves the keys with the host; Tab focuses the scrubber
            (key(crate::KeyCode::Space), None, None, false),
            (key(crate::KeyCode::Tab), None, None, false),
            (key(crate::KeyCode::ArrowRight), Some(45.0), Some(45.0), false),
            (key(crate::KeyCode::ArrowLeft), Some(35.0), Some(35.0), false),
            (key(crate::KeyCode::ArrowUp), Some(45.0), Some(45.0), false),
            (key(crate::KeyCode::ArrowDown), Some(35.0), Some(35.0), false),
            (key(crate::KeyCode::PageUp), Some(90.0), Some(90.0), false),
            (key(crate::KeyCode::PageDown), Some(10.0), Some(10.0), false),
            (key(crate::KeyCode::Home), Some(10.0), Some(10.0), false),
            (key(crate::KeyCode::End), Some(110.0), Some(110.0), false),
            (pointer(Event::POINTER_PRESSED, 50.0, 0), Some(35.0), None, true),
            (pointer(Event::POINTER_CANCELLED, 50.0, 0), None, None, false),
            (pointer(Event::POINTER_RELEASED, 50.0, 0), None, None, false),
            (pointer(Event::POINTER_PRESSED, 50.0, 0), Some(35.0), None, true),
            (Event::new(Event::FOCUS_LOST), None, None, false),
            (pointer(Event::POINTER_RELEASED, 50.0, 0), None, None, false),
        ] {
            time += 0.01;
            ui.frame(&{
                let mut frame = input().time(time);
                frame.events = core::slice::from_ref(&event);
                frame
            }, &mut build).unwrap();
            let s = result.get().unwrap();
            assert_eq!((s.preview, s.commit, s.response.held), (preview, commit, held), "event {event:?}");
            assert_eq!(ui.rect(&s.response.handle), Rect::new(0.0, 0.0, 200.0, 20.0));
        }
        // The release point survives a later move in the same batch.
        let press = pointer(Event::POINTER_PRESSED, 50.0, 0);
        ui.frame(&{
            let mut frame = input().time(time);
            frame.events = core::slice::from_ref(&press);
            frame
        }, &mut build).unwrap();
        let events = [pointer(Event::POINTER_RELEASED, 150.0, 0), pointer(Event::POINTER_MOVED, 5.0, 0)];
        ui.frame(&{
            let mut frame = input().time(time);
            frame.events = &events[..2];
            frame
        }, &mut build).unwrap();
        assert_eq!(result.get().unwrap().commit, Some(85.0));
        ui.frame(&{
            let mut frame = input().time(time);
            frame.events = core::slice::from_ref(&press);
            frame
        }, &mut build).unwrap();
        let events = [pointer(Event::POINTER_RELEASED,150.0,0),pointer(Event::POINTER_PRESSED,20.0,0)];
        ui.frame(&{
            let mut frame = input().time(time);
            frame.events = &events[..2];
            frame
        }, &mut build).unwrap();
        let s = result.get().unwrap();
        assert_eq!((s.preview,s.commit,s.response.held),(Some(20.0),Some(85.0),true),"a new capture previews independently of the previous release");
        let cancel = pointer(Event::POINTER_CANCELLED,20.0,0);
        ui.frame(&{
            let mut frame = input().time(time);
            frame.events = core::slice::from_ref(&cancel);
            frame
        }, &mut build).unwrap();
        // (a key during a mouse hold is the host's: the press left no key focus)
        for accessible in [true] {
            ui.frame(&{
                let mut frame = input().time(time);
                frame.events = core::slice::from_ref(&press);
                frame
            }, &mut build).unwrap();
            let right=key(crate::KeyCode::ArrowRight);
            if accessible { ui.inject_a11y(&[crate::A11yAction {id:identity,action_code:4,payload:75.0,..Default::default()}]); }
            ui.frame(&{
                let mut frame = input().time(time);
                frame.events = if accessible { &[] } else { core::slice::from_ref(&right) };
                frame
            }, &mut build).unwrap();
            let s=result.get().unwrap();let expected=if accessible {75.0} else {40.0};
            assert_eq!((s.preview,s.commit,s.response.held),(Some(expected),Some(expected),false),"digital adjustment takes over a held pointer");
            let release=pointer(Event::POINTER_RELEASED,150.0,0);
            ui.frame(&{
                let mut frame = input().time(time);
                frame.events = core::slice::from_ref(&release);
                frame
            },&mut build).unwrap();
            assert!(result.get().unwrap().commit.is_none());
        }
        let events = [key(crate::KeyCode::Tab), key(crate::KeyCode::ArrowRight), key(crate::KeyCode::ArrowRight), key(crate::KeyCode::ArrowLeft)];
        ui.frame(&{
            let mut frame = input().time(time);
            frame.events = &events[..];
            frame
        }, &mut build).unwrap();
        assert_eq!(result.get().unwrap().commit, Some(45.0), "all routed presses retain their order");
        let events = [key(crate::KeyCode::Meta), key(crate::KeyCode::ArrowRight), Event::key(Event::KEY_RELEASED, crate::KeyCode::Meta.wire())];
        ui.frame(&{
            let mut frame = input().time(time);
            frame.events = &events[..];
            frame
        }, &mut build).unwrap();
        assert!(result.get().unwrap().commit.is_none(),"command chords remain available to the application");
        for (payload, expected) in [(75.0, 75.0), (-1.0, 10.0), (f64::MAX, 110.0)] {
            ui.inject_a11y(&[crate::A11yAction { id: identity, action_code: 4, payload, ..Default::default() }]);
            ui.frame(&input().time(time), &mut build).unwrap();
            assert_eq!(result.get().unwrap().commit, Some(expected));
            let node = ui.a11y_nodes().iter().find(|n| n.id == identity).unwrap();
            assert_eq!((node.value.now, node.value.min, node.value.max, node.value.step), (expected as f64, 10.0, 110.0, 5.0));
        }
        for (phase, x, preview, commit) in [(0,50.0,Some(35.0),None),(1,250.0,Some(110.0),None),(2,150.0,Some(85.0),Some(85.0)),(0,50.0,Some(35.0),None),(3,150.0,None,None)] {
            time += 0.01;
            let touch = Touch::new(1, 7, [x, 10.0], time, 1.0, phase);
            let event = { let mut record = Event::default(); record.kind = Event::TOUCH; record.text = { let mut record = Str::default(); record.ptr = (&touch as *const Touch).cast(); record.len = core::mem::size_of::<Touch>(); record }; record };
            ui.frame(&{
                let mut frame = input().time(time);
                frame.events = core::slice::from_ref(&event);
                frame
            }, &mut build).unwrap();
            let s = result.get().unwrap();
            assert_eq!((s.preview, s.commit), (preview, commit));
        }
        for (phase, finger, held) in [(0,7,true),(0,8,false),(2,7,false),(2,8,false)] {
            time += 0.01;
            let touch = Touch::new(1, finger, [50.0, 10.0], time, 1.0, phase);
            let event = { let mut record = Event::default(); record.kind = Event::TOUCH; record.text = { let mut record = Str::default(); record.ptr = (&touch as *const Touch).cast(); record.len = core::mem::size_of::<Touch>(); record }; record };
            ui.frame(&{
                let mut frame = input().time(time);
                frame.events = core::slice::from_ref(&event);
                frame
            }, &mut build).unwrap();
            let s = result.get().unwrap();
            assert_eq!(s.response.held,held);
            assert!(s.commit.is_none(),"multitouch cancels the original adjustment");
        }
        ui.frame(&{
            let mut frame = input().time(time);
            frame.events = core::slice::from_ref(&press);
            frame
        }, &mut build).unwrap();
        popup.set(true);
        ui.frame(&input().time(time), &mut build).unwrap();
        assert!(!result.get().unwrap().response.held);
        let release = pointer(Event::POINTER_RELEASED,150.0,0);
        ui.frame(&{
            let mut frame = input().time(time);
            frame.events = core::slice::from_ref(&release);
            frame
        }, &mut build).unwrap();
        assert!(result.get().unwrap().commit.is_none(),"a modal cancels an outside scrub");
        ui.dismiss_popup(); popup.set(false);
        ui.frame(&input().time(time), &mut build).unwrap();
        ui.frame(&{
            let mut frame = input().time(time);
            frame.events = core::slice::from_ref(&press);
            frame
        }, &mut build).unwrap();
        present.set(false);
        ui.frame(&input().time(time), &mut build).unwrap();
        assert!(result.get().is_none());
        present.set(true);
        ui.frame(&{
            let mut frame = input().time(time);
            frame.events = core::slice::from_ref(&release);
            frame
        }, &mut build).unwrap();
        assert!(result.get().unwrap().commit.is_none(),"a retired owner cannot commit after reappearing");
        identity = ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == "Position").unwrap().id;
        if round > 1 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm pointer/keyboard/a11y/touch scrub frames"); }
    }
    let press = pointer(Event::POINTER_PRESSED, 50.0, 0);
    ui.frame(&{
        let mut frame = input().time(time);
        frame.events = core::slice::from_ref(&press);
        frame
    }, &mut build).unwrap();
    range.set((10.0, 10.0)); value.set(10.0);
    ui.frame(&input().time(time), &mut build).unwrap();
    assert!(!result.get().unwrap().response.held);
    assert!(ui.a11y_nodes().iter().find(|n| n.id == identity).unwrap().is_role(Role::Progress));
    range.set((10.0, 110.0)); value.set(40.0);
    let release = pointer(Event::POINTER_RELEASED, 150.0, 0);
    ui.frame(&{
        let mut frame = input().time(time);
        frame.events = core::slice::from_ref(&release);
        frame
    }, &mut build).unwrap();
    assert!(result.get().unwrap().commit.is_none(), "a read-only transition cancels capture");
    ui.set_dir(true);
    ui.frame(&input().time(time), &mut build).unwrap();
    let x = f64::from(ui.rect(&result.get().unwrap().response.handle).x);
    let press = pointer(Event::POINTER_PRESSED, x + 50.0, 0);
    let release = pointer(Event::POINTER_RELEASED, x + 150.0, 0);
    ui.frame(&{
        let mut frame = input().time(time);
        frame.events = core::slice::from_ref(&press);
        frame
    }, &mut build).unwrap();
    assert_eq!(result.get().unwrap().preview, Some(85.0));
    ui.frame(&{
        let mut frame = input().time(time);
        frame.events = core::slice::from_ref(&release);
        frame
    }, &mut build).unwrap();
    assert_eq!(result.get().unwrap().commit, Some(35.0));
    let keys = [key(crate::KeyCode::Tab), key(crate::KeyCode::ArrowRight)];
    ui.frame(&{
        let mut frame = input().time(time);
        frame.events = &keys[..];
        frame
    }, &mut build).unwrap();
    assert_eq!(result.get().unwrap().commit, Some(35.0));
}

#[test]
fn repeating_keyframes_keep_instances_and_builds_idle_when_hidden_and_stop_without_allocating() {
    use crate::{ui::{Curve, Keyframe, Keyframes, Look, Node, Repeat, Target, ViewTransform}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.verify_views(false); // the test counts kept views' rebuilds or allocations; verification rebuilds them
    ui.set_a11y(true);
    let view = Cell::new(None);
    let child = Cell::new(None);
    let builds = Cell::new(0);
    let mut build = |ui: &mut Ui| {
        view.set(Some({ let enclosing = ui.node("repeat", Node::column().w_px(20.0).h_px(10.0)); let mut view = enclosing; ui.within(&enclosing, &mut |ui| view = ui.view_of("repeat", &1, |ui| {
            builds.set(builds.get() + 1);
            let bar = ui.node("bar", Node::DEFAULT.w(Size::Grow).h(Size::Grow).role(crate::ui::Role::Image).look(Look::NONE.fill(crate::geom::Rgba::rgb(255, 0, 0))));
            ui.a11y_name(&bar, "Repeated bar");
            child.set(Some(bar));
        })); view }));
    };
    let curve = Curve::Bezier { duration: 99.0, points: [0.0, 0.0, 1.0, 1.0] };
    let keys = [Keyframe::scale(0.0, 1.0, 0.4).anchor(0.5, 1.0).curve(curve), Keyframe::scale(0.5, 1.0, 1.0).anchor(0.5, 1.0).curve(curve)];
    ui.frame(&input(), &mut build).unwrap();
    ui.frame(&input(), &mut build).unwrap();
    let view = view.get().unwrap();
    let child = child.get().unwrap();
    let identity = ui.a11y_nodes().iter().find(|node| ui.a11y_name_of(node) == "Repeated bar").unwrap().id;
    assert!(ui.box_of_a11y(u64::MAX).is_none());
    let instances = ui.layer(0).expect("base layer").instances.to_vec();
    let built = builds.get();
    for round in 0..3 {
        let start = round as f64 * 10.0;
        ui.frame(&input().time(start), &mut build).unwrap();
        mem_sys::local_reset();
        assert!(ui.animate_keys(&view, &keys, &Keyframes::DEFAULT.repeat(Repeat::Forever).autoreverse().offset(0.25)));
        let mut sample = start;
        for tick in 0..61 {
            let time = start + tick as f64 / 60.0;
            if time >= sample + 1.0 / 30.0 {
                sample = time;
            }
            ui.frame(&input().time(time), &mut build).unwrap();
            let phase = (sample - start + 0.25) % 1.0;
            let progress = if phase < 0.5 { phase * 2.0 } else { 2.0 - phase * 2.0 };
            let rect = ui.rect(&child);
            let resolved = ui.box_of_a11y(identity).unwrap();
            assert_eq!(resolved.box_key, child.box_key);
            assert_eq!(ui.rect(&resolved), rect);
            assert!((rect.h - (4.0 + 6.0 * progress as f32)).abs() < 0.001);
            assert!((rect.y + rect.h - 10.0).abs() < 0.001, "fixed bottom anchor");
            assert_eq!(ui.repaint_at(), Some(sample + 1.0 / 30.0));
            assert_eq!(builds.get(), built);
            assert_eq!(ui.layer(0).expect("base layer").instances, instances.as_slice());
        }
        ui.set_view(&view, &crate::ViewTransform::DEFAULT.clip(Some(Rect::new(0.0, 0.0, 0.0, 0.0))));
        ui.frame(&input().time(start + 2.0), &mut build).unwrap();
        assert!(!ui.wants_frame(&[], start + 2.0), "hidden loops must idle");
        // a transform that omits the clip keeps the one the view has
        ui.set_view(&view, &ViewTransform::DEFAULT.opacity(1.0));
        ui.frame(&input().time(start + 2.5), &mut build).unwrap();
        assert!(!ui.wants_frame(&[], start + 2.5), "an omitted clip keeps the existing one");
        ui.set_view(&view, &crate::ViewTransform::DEFAULT.clip(None));
        ui.frame(&input().time(start + 3.0), &mut build).unwrap();
        assert!(ui.wants_frame(&[], start + 3.0 + 1.0 / 30.0), "a shown loop runs again (30 fps: no recent input)");
        ui.set_view(&view, &ViewTransform::DEFAULT.opacity(0.0));
        ui.frame(&input().time(start + 3.0), &mut build).unwrap();
        assert!(!ui.wants_frame(&[], start + 3.0), "manually hidden loops must idle");
        ui.set_view(&view, &ViewTransform::DEFAULT.opacity(1.0));
        ui.frame(&input().time(start + 3.0), &mut build).unwrap();
        assert!(ui.wants_frame(&[], start + 3.0 + 1.0 / 30.0));
        // Finite cycles count both legs and settle at their authored first stop.
        assert!(ui.animate_keys(&view, &keys, &Keyframes::DEFAULT.repeat(Repeat::Count(2)).autoreverse()));
        for (dt, height, running) in [(0.5, 10.0, true), (1.0, 4.0, true), (1.5, 10.0, true), (2.0, 4.0, false)] {
            ui.frame(&input().time(start + 3.0 + dt), &mut build).unwrap();
            assert!((ui.rect(&child).h - height).abs() < 0.001);
            assert_eq!(ui.wants_frame(&[], start + 3.0 + dt), running);
        }
        assert!(ui.animate_keys(&view, &keys, &Keyframes::DEFAULT.repeat(Repeat::Count(2)).offset(0.25)));
        ui.frame(&input().time(start + 5.75), &mut build).unwrap();
        assert!((ui.rect(&child).h - 10.0).abs() < 0.001);
        assert!(!ui.wants_frame(&[], start + 5.75));
        assert!(ui.animate_keys(&view, &keys, &Keyframes::DEFAULT.repeat(Repeat::Forever).autoreverse()));
        assert!(ui.animate(&view, Target::Scale { x: 1.0, y: 0.4, anchor: (0.5, 1.0) }, curve.with_duration(0.0)));
        ui.frame(&input().time(start + 6.0), &mut build).unwrap();
        assert!(!ui.wants_frame(&[], start + 6.0), "retargeting stops a loop");
        if round > 0 { assert_eq!(mem_sys::local_alloc_count(), 0); }
    }
    assert!(ui.animate_keys(&view, &keys, &Keyframes::DEFAULT.repeat(Repeat::Forever).autoreverse().offset(0.25)));
    ui.frame(&input().time(27.0).reduced_motion(true), &mut build).unwrap();
    assert!((ui.rect(&child).h - 4.0).abs() < 0.001);
    assert!(!ui.wants_frame(&[], 28.0));
    assert!(ui.animate_keys(&view, &keys, &Keyframes::DEFAULT.repeat(Repeat::Forever).autoreverse()));
    ui.frame(&input().time(28.0).reduced_motion(true), &mut build).unwrap();
    assert!(!ui.wants_frame(&[], 29.0));
    ui.frame(&input().time(29.0), &mut |_| {}).unwrap();
    assert!(ui.box_of_a11y(identity).is_none());
    ui.frame(&input().time(30.0), &mut build).unwrap();
    assert!(ui.box_of_a11y(identity).is_none(), "recycled slots must not revive retired identities");
}

#[test]
fn shrinking_virtual_sections_select_the_clamped_rows_in_one_build_without_warm_allocations() {
    use std::cell::Cell;
    use crate::ui::{SectionList,SectionPart};
    mem_sys::set_peer(crate::library_counter);
    let count=Cell::new(10_000);let body=Cell::new(None);let first=Cell::new(None);let rows=Cell::new(0);let builds=Cell::new(0);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut build=|ui:&mut Ui| {
        builds.set(builds.get()+1);rows.set(0);first.set(None);
        body.set(Some(ui.section_list("shrinking",&SectionList::new(count.get()).row_h(20.0).lead(100.0).header(30.0).trail(40.0).overlay_scrollbar(),&|i| i as i64,&mut |_,part,_| {
            if let SectionPart::Row(index)=part {if first.get().is_none() {first.set(Some(index));}rows.set(rows.get()+1);}
        }).handle));
    };
    ui.frame(&input(),&mut build).unwrap();
    for cycle in 0..8 {
        mem_sys::local_reset();
        count.set(10_000);ui.set_scroll_offset(&body.get().unwrap(),199_900.0);ui.frame(&input(),&mut build).unwrap();
        assert!(first.get().unwrap()>9900);
        for next in [3,0] {
            count.set(next);let previous=builds.get();
            ui.frame(&input(),&mut build).unwrap();
            assert_eq!(builds.get()-previous,1,"shrink completes in the original build");
            assert_eq!(ui.scroll(&body.get().unwrap()).settled,0.0);
            assert_eq!(rows.get(),next,"the first shrink frame must not leave an empty viewport");
            assert_eq!(first.get(),if next>0 {Some(0)} else {None});
        }
        if cycle>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"warm shrink/grow virtual sections");}
    }
}

/// Rows carrying text spans (1 to 4 spans a row, so a row's span count differs from the row it replaces) scroll into view from far away, row
/// after row, and a warm frame allocates nothing: the spans of every styled node live in one arena of the `Ui` (compacted when it is full and
/// mostly dead) and in the retained layouts, never in a vector of their own.
#[test]
fn fresh_rows_carrying_text_spans_scroll_into_view_without_warm_allocations() {
    use std::cell::Cell;
    use crate::{ui::{SectionList, SectionPart}, Label, TextSpan};
    mem_sys::set_peer(crate::library_counter);
    const TEXTS: [&str; 4] = ["abcdefghijklmnopqrstuvwx", "the quick brown fox jumps", "0123456789 0123456789 01", "lorem ipsum dolor sit am"];
    let bold = { let mut record = TextSpan::default(); record.weight = 700; record };
    let spans: [TextSpan; 4] = std::array::from_fn(|i| { let mut record = bold; record.start = 3 * i as u32; record.end = 3 * i as u32 + 2; record });
    let (body, styled) = (Cell::new(None), Cell::new(0usize));
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut build = |ui: &mut Ui| {
        styled.set(0);
        body.set(Some(ui.section_list("spans", &SectionList::new(20_000).row_h(20.0).overlay_scrollbar(), &|i| i as i64, &mut |ui, part, _| {
            if let SectionPart::Row(index) = part {
                let node = ui.node(index as i64, crate::Node::DEFAULT.h_px(20.0));
                ui.text_spans(&node, TEXTS[index % 4], Label::BODY, &spans[..1 + index % 4]).unwrap();
                styled.set(styled.get() + 1);
            }
        }).handle));
    };
    ui.frame(&input(), &mut build).unwrap();
    for cycle in 0..40 {
        mem_sys::local_reset();
        for jump in 0..20 {
            // 19 rows past the last window (a prime step): every jump brings fresh rows, with other span counts than the rows they replace
            ui.set_scroll_offset(&body.get().unwrap(), 20.0 * ((cycle * 20 + jump) * 19 % 19_000) as f32);
            ui.frame(&input(), &mut build).unwrap();
            assert!(styled.get() >= 10, "the window shows rows with spans");
        }
        if cycle >= 30 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm fresh rows with text spans, cycle {cycle}"); }
    }
}

#[test]
fn section_list_scrolls_leading_content_pins_blocks_and_virtualizes_without_warm_allocations() {
    use std::cell::Cell;
    use crate::ui::{Node,SectionList,SectionPart,Role};
    mem_sys::set_peer(crate::library_counter);
    {
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap(); ui.set_a11y(true);
    let section=SectionList::new(10_000).row_h(20.0).lead(100.0).header(30.0).trail(40.0).overlay_scrollbar();
    let body=Cell::new(None); let lead=Cell::new(None); let header=Cell::new(None); let first=Cell::new(None);
    let rows=Cell::new(0usize); let frame_builds=Cell::new(0usize); let clicked=Cell::new(false); let sorted=Cell::new(false);
    let mut build=|ui:&mut Ui| {
        frame_builds.set(frame_builds.get()+1);
        rows.set(0); clicked.set(false); sorted.set(false); first.set(None);
        body.set(Some(ui.section_list("section",&section,&|i| i as i64,&mut |ui,part,_| {
            match part {
                SectionPart::Trailing=>{},
                SectionPart::Leading=>{
                    let clip=ui.node("heading-clip",Node::DEFAULT.flags(pxui_types::Flags::CLIP).w(Size::Grow).h_px(100.0));
                    ui.within(&clip,&mut |ui| {
                        ui.view_of("heading-cache",&0,|ui| {
                            let node=ui.node("title",Node::DEFAULT.role(Role::Label).w(Size::Grow).h_px(100.0));
                            ui.a11y_name(&node,"Page heading"); lead.set(Some(node));
                        });
                    });
                }
                SectionPart::Header=>{
                    let node=ui.node("sort",Node::DEFAULT.flags(pxui_types::Flags::CLICKABLE).role(Role::Button).w_px(50.0).h(Size::Grow));
                    ui.a11y_name(&node,"Sort"); header.set(Some(node)); sorted.set(ui.signal(&node).clicked);
                }
                SectionPart::Row(index)=>{
                    rows.set(rows.get()+1);
                    let node=ui.node("row",Node::DEFAULT.flags(pxui_types::Flags::CLICKABLE).role(Role::Button).w(Size::Grow).h(Size::Grow));
                    ui.a11y_name(&node,"Row");
                    if index==0 {first.set(Some(node));}
                    clicked.set(clicked.get() || ui.signal(&node).clicked);
                }
            }
        }).handle));
    };
    ui.frame(&input(),&mut build).unwrap();
    assert_eq!(ui.rect(&lead.get().unwrap()).y,0.0);
    assert_eq!(ui.rect(&header.get().unwrap()).y,100.0);
    assert_eq!(ui.rect(&first.get().unwrap()).y,130.0);
    assert_eq!(ui.rect(&first.get().unwrap()).w,320.0,"overlay thumb reserves no gutter");
    let id=ui.a11y_nodes().iter().find(|n|n.is_role(Role::List)).unwrap().id;
    for round in 0..8 {
        mem_sys::local_reset();
        for offset in [90.0,105.0,500.0,199_900.0,0.0] {
            let previous=ui.scroll(&body.get().unwrap()).settled;
            let previous_builds=frame_builds.get();
            ui.inject_a11y(&[crate::A11yAction {id,action_code:8,payload:(offset-previous) as f64,..Default::default()}]);
            ui.frame(&input(),&mut build).unwrap();
            assert_eq!(frame_builds.get()-previous_builds,1,"indexed heading return must not settle an extra frame build");
            assert!((ui.scroll(&body.get().unwrap()).settled-offset).abs()<0.001,"scroll offset");
            assert!((ui.rect(&lead.get().unwrap()).y+offset).abs()<0.001,"retained heading rect: {:?}",ui.rect(&lead.get().unwrap()));
            let heading=ui.a11y_nodes().iter().find(|n|ui.a11y_name_of(n)=="Page heading").unwrap();
            assert!((heading.rect.y+offset).abs()<0.001,"retained heading accessibility: {:?}",heading.rect);
            assert_eq!(heading.states & crate::A11yStates::VISIBLE.bits()!=0,offset<100.0,"offscreen headings are not visible to accessibility");
            assert!((ui.rect(&header.get().unwrap()).y-(100.0-offset).max(0.0)).abs()<0.001);
            assert!((1..30).contains(&rows.get()),"only viewport rows plus overscan: {}",rows.get());
        }
        if round>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"warm leading/sticky/virtual section scrolling");}
    }
    ui.set_scroll_offset(&body.get().unwrap(),105.0);
    ui.frame(&input(),&mut build).unwrap();
    assert_eq!(ui.rect(&first.get().unwrap()).y,25.0,"row partly under the pinned heading");
    for (x,y,sort,row) in [(200.0,27.0,false,false),(20.0,15.0,true,false),(200.0,35.0,false,true)] {
        for kind in [crate::Event::POINTER_PRESSED,crate::Event::POINTER_RELEASED] {
            let event=crate::Event::pointer(kind, 0, x, y);
            ui.frame(&{
                let mut frame = input();
                frame.events = core::slice::from_ref(&event);
                frame
            },&mut build).unwrap();
        }
        assert_eq!(sorted.get(),sort); assert_eq!(clicked.get(),row,"sticky padding blocks rows underneath");
    }
    }
}

#[test]
fn section_lists_validate_geometry_recover_callbacks_and_support_empty_unpinned_sections() {
    use crate::ui::{SectionList,SectionPart};
    let valid=SectionList::new(0).row_h(20.0).lead(30.0).trail(40.0);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let called=std::cell::Cell::new(false);
    for bad in [{ let mut record = valid; record.count = 1_000_001; record },valid.row_h(0.0),valid.row_h(f32::NAN),valid.lead(-1.0),valid.header(f32::INFINITY),valid.trail(-1.0),{ let mut record = valid.row_h(f32::MAX); record.count = 2; record }] {
        assert!(catch_unwind(AssertUnwindSafe(|| {let _=ui.frame(&input(),&mut |ui| {ui.section_list("bad",&bad,&|i| i as i64,&mut |_,_,_|called.set(true));});})).is_err());
        assert!(!called.get(),"invalid geometry reached the callback");
    }
    for fail in [SectionPart::Leading,SectionPart::Header,SectionPart::Row(0),SectionPart::Trailing] {
        let section={ let mut record = valid.header(20.0); record.count = 1; record };
        assert!(catch_unwind(AssertUnwindSafe(|| {let _=ui.frame(&input(),&mut |ui| {ui.section_list("panic",&section,&|i| i as i64,&mut |_,part,_| {if part==fail {panic!("section callback");}});});})).is_err());
        let mut calls=0;
        ui.frame(&input(),&mut |ui| {ui.section_list("empty",&valid,&|i| i as i64,&mut |_,part,_| {assert!(matches!(part,SectionPart::Leading|SectionPart::Trailing),"{part:?}");calls+=1;});}).unwrap();
        assert!(calls>=2,"empty sections retain the lead-in and the trailer and omit a zero heading and all rows");
    }
}

#[test]
fn measured_sections_validate_geometry_recover_callbacks_and_support_empty_unpinned_sections() {
    use crate::ui::{SectionList,SectionPart};
    let valid=SectionList::new(0).row_h(20.0).lead(30.0).trail(40.0);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let called=std::cell::Cell::new(false);
    for bad in [{ let mut record = valid; record.count = 1_000_001; record },valid.row_h(0.0),valid.row_h(f32::NAN),valid.lead(-1.0),valid.header(f32::INFINITY),valid.trail(-1.0),{ let mut record = valid.row_h(f32::MAX); record.count = 2; record }] {
        assert!(catch_unwind(AssertUnwindSafe(|| {let _=ui.frame(&input(),&mut |ui| {ui.section_list("bad",&bad.measured(),&|i| i as i64,&mut |_,_,_|called.set(true));});})).is_err());
        assert!(!called.get(),"invalid geometry reached the callback");
    }
    for fail in [SectionPart::Leading,SectionPart::Header,SectionPart::Row(0),SectionPart::Trailing] {
        let section={ let mut record = valid.header(20.0); record.count = 1; record };
        assert!(catch_unwind(AssertUnwindSafe(|| {let _=ui.frame(&input(),&mut |ui| {ui.section_list("panic",&section.measured(),&|i| i as i64,&mut |_,part,_| {if part==fail {panic!("section callback");}});});})).is_err());
        let mut calls=0;
        ui.frame(&input(),&mut |ui| {ui.section_list("empty",&valid.measured(),&|i| i as i64,&mut |_,part,_| {assert!(matches!(part,SectionPart::Leading|SectionPart::Trailing),"{part:?}");calls+=1;});}).unwrap();
        assert!(calls>=2,"empty sections retain the lead-in and the trailer and omit a zero heading and all rows");
    }
}

#[test]
fn rendering_density_matches_paint_updates_cached_views_and_allocates_nothing() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.verify_views(false); // the test counts kept views' rebuilds or allocations; verification rebuilds them
    assert_eq!(ui.density(),1.0);
    let builds=std::cell::Cell::new(0);
    for round in 0..6 {
        mem_sys::local_reset();
        let before=builds.get();
        for scale in [1.0,2.0,3.0,3.0] {
            ui.frame(&input().scale(scale),&mut |ui| {
                assert_eq!(ui.density(),scale as i64 as f32);
                ui.id("density", |ui| { ui.column(Node::column().w_px(40.0).h_px(40.0), |ui| { ui.view_of("density", &0, |ui| {
                    builds.set(builds.get()+1);
                    let density=ui.density();
                    let node=ui.node("paint",crate::Node::DEFAULT.w_px(40.0).h_px(40.0));
                    ui.canvas(&node,move |paint,_| {assert_eq!(paint.density(),density);});
                }); }); });
            }).unwrap();
            assert_eq!(ui.density(),scale as i64 as f32,"owner keeps the last frame's density");
        }
        assert_eq!(builds.get()-before,3,"density changes invalidate cached contents; equal density retains them");
        if round>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"cached density changes");}
    }
}

#[test]
fn analytic_curves_preserve_fractional_geometry_validate_and_reuse_warm_storage() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    for round in 0..6 {
        mem_sys::local_reset();
        for scale in [1.0,2.0] {
            ui.frame(&input().scale(scale),&mut |ui| {
                let node=ui.node("curves",crate::Node::DEFAULT.w_px(100.0).h_px(100.0));
                ui.canvas(&node,|paint,_| {
                    let color=crate::geom::Rgba::rgb(12, 34, 56);
                    paint.curve((1.25,2.75), (3.5,4.25), (5.75,6.5), Stroke::new(color).width(1.6));
                    paint.cubic([(7.0,7.0),(11.5,7.0),(12.5,17.0),(17.0,17.0)], Stroke::new(color).width(1.6));
                    for width in [0.0,-1.0,f32::NAN,f32::INFINITY,f32::MAX] {
                        paint.curve((0.0,0.0), (1.0,1.0), (2.0,2.0), Stroke::new(color).width(width));
                        paint.cubic([(0.0,0.0);4], Stroke::new(color).width(width));
                    }
                    paint.curve((f32::INFINITY,0.0), (1.0,1.0), (2.0,2.0), Stroke::new(color).width(1.0));
                    paint.cubic([(f32::NAN,0.0);4], Stroke::new(color).width(1.0));
                });
            }).unwrap();
            let mut instances=ui.layers().flat_map(|layer|layer.instances.iter());
            let first=instances.next().unwrap();
            assert_eq!(first.kind,crate::draw::kind::CURVE);
            assert_eq!(first.rect,[1.25,2.75,5.75,6.5]);
            assert_eq!(first.extra,[3.5f32.to_bits(),4.25f32.to_bits()]);
            assert_eq!(instances.count(),4,"one quadratic and four cubic pieces; invalid geometry emits nothing");
        }
        if round>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"analytic curve build/draw storage");}
    }
}

#[test]
fn soft_wrap_separator_widths_preserve_glyphs_and_cached_measurements() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let values=["one two three four","one   two three four","one two\nthree four","one\u{00a0}two three four","one אב two three four","\t one two three four"];
    for round in 0..6 {
        mem_sys::local_reset();
        ui.frame(&input(),&mut |ui| {
            for (index,value) in values.into_iter().enumerate() {
                let plain=crate::Label::BODY.size(12.0).line_height(20.0).wrap();
                let logical=crate::Label::BODY.size(12.0).line_height(20.0).wrap_spaces();
                let a=ui.text_service().measure(value,plain,Some(50.0)).unwrap();
                let b=ui.text_service().measure(value,logical,Some(50.0)).unwrap();
                assert_eq!((a.height,a.lines,a.baseline),(b.height,b.lines,b.baseline));
                assert!(b.width>=a.width && b.width.is_finite());
                if index==0 {
                    let space=ui.text_service().width(" ",plain,None).unwrap();
                    assert!((b.width-a.width-space).abs()<0.001,"the longest wrapped word retains its break space");
                }
                if round==0 {
                    let a={ let mut glyphs=Vec::new(); ui.text_service().quads(value,plain,Some(50.0),crate::Point::new(0.0, 0.0),1.0, &mut glyphs).map(|_|glyphs) }.unwrap();
                    let b={ let mut glyphs=Vec::new(); ui.text_service().quads(value,logical,Some(50.0),crate::Point::new(0.0, 0.0),1.0, &mut glyphs).map(|_|glyphs) }.unwrap();
                    assert_eq!(a,b,"separator metrics leave visible glyphs unchanged");
                }
                let plain=plain.lines(1);
                let logical=logical.lines(1);
                assert_eq!(ui.text_service().measure(value,plain,Some(50.0)).unwrap(),ui.text_service().measure(value,logical,Some(50.0)).unwrap(),"hidden paragraphs must not contribute trailing spaces");
            }
        }).unwrap();
        if round>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"both wrapping modes retain measurement storage");}
    }
}

#[test]
fn dismissed_popups_do_not_intercept_the_next_press_over_their_previous_bounds() {
    use std::cell::Cell;
    use pxui_types::Event;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let open = Cell::new(true);
    let pressed = Cell::new(false);
    let clicked = Cell::new(false);
    let mut build = |ui: &mut Ui| {
        if open.get() {
            ui.popup("popup", &crate::Overlay::DEFAULT.at(crate::Point::new(0.0, 0.0)).w_px(180.0).h_px(100.0), &mut |ui: &mut Ui| {
                ui.node("cover",crate::Node::DEFAULT.flags(pxui_types::Flags::CLICKABLE).w(Size::Grow).h_px(100.0));
            });
        }
        let underlying=ui.node("underlying",crate::Node::DEFAULT.flags(pxui_types::Flags::CLICKABLE).w_px(180.0).h_px(100.0));
        let signal=ui.signal(&underlying);
        pressed.set(signal.pressed); clicked.set(signal.clicked);
    };
    for round in 0..8 {
        mem_sys::local_reset();
        open.set(true);
        ui.frame(&input(),&mut build).unwrap();
        ui.dismiss_popup();
        open.set(false);
        for kind in [Event::POINTER_PRESSED,Event::POINTER_RELEASED] {
            let event=Event::pointer(kind, 0, 60.0, 50.0);
            ui.frame(&{
                let mut frame = input();
                frame.events = core::slice::from_ref(&event);
                frame
            },&mut build).unwrap();
            assert_eq!(pressed.get(),kind==Event::POINTER_PRESSED);
            assert_eq!(clicked.get(),kind==Event::POINTER_RELEASED);
        }
        if round>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"dismiss/next-press warm frames"); }
    }
}

#[test]
fn held_keys_include_focused_and_modal_modifiers_without_allocating_or_consuming_presses() {
    use std::cell::Cell;
    use pxui_types::{Event};
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let meta = crate::KeyCode::Meta;
    assert!(!ui.key_down(meta));
    let modal = Cell::new(false);
    let held = Cell::new(false);
    let comma = Cell::new(false);
    let mut contents = |ui: &mut Ui| {
        let field = ui.node("focused", crate::Node::DEFAULT.flags(pxui_types::Flags::FOCUSABLE).w_px(120.0).h_px(32.0));
        ui.focus_box(&field);
        assert_eq!(ui.key_down(meta), held.get());
        assert_eq!(ui.key_pressed(crate::KeyCode::Char(','), crate::Mods::ANY), comma.get());
        assert_eq!(ui.key_down(meta), held.get());
        assert_eq!(ui.key_pressed(crate::KeyCode::Char(','), crate::Mods::ANY), comma.get(), "reading held keys must not consume activation");
    };
    let mut build = |ui: &mut Ui| {
        if modal.get() { ui.popup("keys", &crate::Overlay::DEFAULT.at(crate::Point::new(10.0, 10.0)).w_px(180.0).h_px(40.0), &mut contents); }
        else { contents(ui); }
    };
    ui.frame(&input(), &mut build).unwrap();
    for round in 0..8 {
        mem_sys::local_reset();
        let mut trace=[0u64;14];
        let mut index=0;
        for popup in [false, true] {
            modal.set(popup);
            for (kind, code, down, pressed) in [
                (Event::KEY_PRESSED, meta.wire(), true, false),
                (Event::POINTER_MOVED, 0, true, false),
                (Event::KEY_PRESSED, ',' as u32, true, true),
                (Event::KEY_RELEASED, ',' as u32, true, false),
                (Event::KEY_RELEASED, meta.wire(), false, false),
                (Event::KEY_PRESSED, meta.wire(), true, false),
                (Event::FOCUS_LOST, 0, false, false),
            ] {
                held.set(down); comma.set(pressed);
                let event=Event::key(kind, code);
                ui.frame(&{
                    let mut frame = input().time(round as f64+0.01);
                    frame.events = core::slice::from_ref(&event);
                    frame
                },&mut build).unwrap();
                assert_eq!(ui.key_down(meta), down);
                trace[index]=mem_sys::local_alloc_count(); index+=1;
            }
        }
        if round>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"raw held-key queries with focus and a popup: {trace:?}"); }
    }
}

#[test]
fn modifier_at_press_queries_keep_event_order_focus_loss_and_warm_storage() {
    use pxui_types::{Event};
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    for round in 0..5 {
        mem_sys::local_reset();
        for (modifier, mods) in [(crate::KeyCode::Shift, crate::Mods::SHIFT), (crate::KeyCode::Ctrl, crate::Mods::CTRL), (crate::KeyCode::Alt, crate::Mods::ALT), (crate::KeyCode::Meta, crate::Mods::META)] {
            let key=|kind,code| Event::key(kind, code);
            for popup in [false,true] {
                for (events,expected) in [
                    ([key(Event::KEY_PRESSED,modifier.wire()),key(Event::KEY_PRESSED,b'k' as u32),key(Event::KEY_RELEASED,modifier.wire())],true),
                    ([key(Event::KEY_PRESSED,b'k' as u32),key(Event::KEY_PRESSED,modifier.wire()),key(Event::KEY_RELEASED,modifier.wire())],false),
                    ([key(Event::KEY_PRESSED,modifier.wire()),key(Event::FOCUS_LOST,0),key(Event::KEY_PRESSED,b'k' as u32)],false),
                    ([key(Event::KEY_PRESSED,modifier.wire()),key(Event::KEY_RELEASED,modifier.wire()),key(Event::KEY_PRESSED,b'k' as u32)],false),
                ] {
                    let mut contents=|ui:&mut Ui| {
                        let field=ui.node("focus",crate::Node::DEFAULT.flags(pxui_types::Flags::FOCUSABLE).w_px(100.0).h_px(24.0));
                        ui.focus_box(&field);
                        assert_eq!(ui.key_pressed(crate::KeyCode::Char('k'), mods),expected);
                        assert_eq!(ui.key_pressed(crate::KeyCode::Char('k'), mods),expected,"query consumes nothing");
                        assert!(ui.key_pressed(crate::KeyCode::Char('k'), crate::Mods::ANY));
                        assert!(!ui.key_down(modifier));
                    };
                    ui.frame(&{
                        let mut frame = input().time(round as f64);
                        frame.events = &events[..];
                        frame
                    },&mut |ui| {
                        if popup {ui.popup("chord", &crate::Overlay::DEFAULT.at(crate::Point::new(0.0, 0.0)).w_px(120.0).h_px(30.0).attached(), &mut contents);}
                        else {contents(ui);}
                    }).unwrap();
                    ui.dismiss_popup();
                }
                let press=key(Event::KEY_PRESSED,modifier.wire());
                ui.frame(&{
                    let mut frame = input();
                    frame.events = core::slice::from_ref(&press);
                    frame
                },&mut |_| {}).unwrap();
                let events=[key(Event::KEY_PRESSED,b'k' as u32),key(Event::KEY_RELEASED,modifier.wire())];
                ui.frame(&{
                    let mut frame = input();
                    frame.events = &events[..];
                    frame
                },&mut |ui| {
                    assert!(ui.key_pressed(crate::KeyCode::Char('k'), mods),"modifier can be held from the previous frame");
                    assert!(!ui.key_down(modifier));
                }).unwrap();
            }
        }
        if round>=2 {assert_eq!(mem_sys::local_alloc_count(),0,"focused and modal modifier-at-press queries");}
    }
}

#[test]
fn combined_modifiers_match_one_press_and_keep_character_case() {
    use crate::{Event, KeyCode, Mods};
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let edge = |kind, key: KeyCode| Event::key(kind, key.wire());
    let press = |key| edge(Event::KEY_PRESSED, key);
    let release = |key| edge(Event::KEY_RELEASED, key);
    let (key, upper) = (KeyCode::Char('k'), KeyCode::Char('K'));
    let (meta, shift, alt) = (KeyCode::Meta, KeyCode::Shift, KeyCode::Alt);
    let chord = Mods::META.with(Mods::SHIFT);
    let separate = [press(meta), press(key), release(key), release(meta),
        press(shift), press(key), release(key), release(shift)];
    let together = [press(meta), press(shift), press(alt), press(key), release(key),
        release(alt), release(shift), release(meta)];
    let unmodified = [press(meta), Event::new(Event::FOCUS_LOST),
        press(key), release(key)];
    for round in 0..5 {
        mem_sys::local_reset();
        for (events, expected) in [
            (&separate[..], (true, true, false, false)),
            (&together[..], (true, true, true, false)),
            (&unmodified[..], (false, false, false, true)),
        ] {
            crate::testing::frame(&mut ui, events, &mut |ui| {
                assert_eq!((ui.key_pressed(key, Mods::META), ui.key_pressed(key, Mods::SHIFT),
                    ui.key_pressed(key, chord), ui.key_pressed(key, Mods::NONE)), expected);
                assert!(ui.key_pressed(key, Mods::ANY));
                assert!(!ui.key_pressed(upper, Mods::ANY));
                assert!(!ui.key_down(meta) && !ui.key_down(shift) && !ui.key_down(key));
            });
        }
        if round >= 2 { assert_eq!(mem_sys::local_alloc_count(), 0); }
    }
}

#[test]
fn backdrop_saturation_validates_and_updates_without_warm_allocations() {
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let view = Cell::new(None);
    let mut build = |ui: &mut Ui| {
        view.set(Some({ let enclosing = ui.node("glass", Node::column().w_px(120.0).h_px(40.0)); let mut view = enclosing; ui.within(&enclosing, &mut |ui| view = ui.view_of("glass", &1, |ui| {
            ui.node("pane", crate::Node::DEFAULT.w_px(120.0).h_px(40.0).look(crate::ui::Look::NONE.fill(pxui_types::geom::Rgba::WHITE)));
        })); view }));
    };
    ui.frame(&input(), &mut build).unwrap();
    let view = view.get().unwrap();
    ui.set_view(&view, &crate::ViewTransform::DEFAULT.blur(8.0));
    for time in [1.0, 2.0] { ui.frame(&input().time(time), &mut build).unwrap(); }
    for saturation in [0.0, 0.9, 1.0, 1.5] {
        mem_sys::local_reset();
        ui.set_view(&view, &crate::ViewTransform::DEFAULT.saturation(saturation));
        ui.frame(&input().time(3.0), &mut build).unwrap();
        assert_eq!(mem_sys::local_alloc_count(), 0, "warm backdrop saturation update");
    }
    for invalid in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(catch_unwind(AssertUnwindSafe(|| ui.set_view(&view, &crate::ViewTransform::DEFAULT.saturation(invalid)))).is_err());
        assert!(ui.last_error().contains("saturation"));
    }
}

#[test]
fn scale_animation_retargets_keyframes_and_accessibility_allocate_nothing_when_warm() {
    use crate::{ui::{Curve, Keyframe, Keyframes, Look, Node, Role, Target}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    for nested in [false,true] {
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.verify_views(false); // the test counts kept views' rebuilds or allocations; verification rebuilds them
    ui.set_a11y(true);
    let (child, view, builds) = (Cell::new(None), Cell::new(None), Cell::new(0));
    let mut content = |ui: &mut Ui| {
        let v_box = ui.node("scale", Node::column().w_px(100.0).h_px(80.0));
        ui.set_at(&v_box, (100.0, 40.0));
        let mut v = v_box;
        ui.within(&v_box, &mut |ui| v = ui.view_of("scale", &1, |ui| {
            builds.set(builds.get() + 1);
            let b = ui.node("child", Node::DEFAULT.w_px(20.0).h_px(10.0).role(Role::Button).at(20.0, 10.0).look(Look::NONE.fill(pxui_types::geom::Rgba::rgb(9, 9, 9))));
            ui.a11y_name(&b, "Scaled");
            child.set(Some(b));
        }));
        view.set(Some(v));
    };
    let mut build=|ui:&mut Ui| {if nested {ui.view_of("outer", &0, &mut content);} else {content(ui);}};
    for time in [0.0, 0.1] { ui.frame(&input().time(time), &mut build).unwrap(); }
    let (view, child, built) = (view.get().unwrap(), child.get().unwrap(), builds.get());
    let instances = ui.layer(0).expect("base layer").instances.to_vec();
    let mut positions = [0.0; 16]; positions[1] = 0.5; positions[2] = 1.0;
    let mut values = positions; values[1] = 1.1;
    let spring = Curve::Table { duration: 0.2, count: 3, positions, values };
    let linear = Curve::Bezier { duration: 0.2, points: [0.0, 0.0, 1.0, 1.0] };
    let instant = Curve::Steps { duration: 0.0, count: 1 };
    let large = Target::Scale { x: 2.0, y: 0.5, anchor: (1.0, 1.0) };
    let normal = Target::Scale { x: 1.0, y: 1.0, anchor: (1.0, 1.0) };
    let shown = |ui: &Ui, expected: Rect| {
        let near = |r: Rect| [r.x - expected.x, r.y - expected.y, r.w - expected.w, r.h - expected.h].iter().all(|d| d.abs() < 0.005);
        assert!(near(ui.rect(&child)), "shown {:?}, expected {expected:?}", ui.rect(&child));
        let node = ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == "Scaled").unwrap();
        assert!(near(node.rect), "accessible {:?}, expected {expected:?}", node.rect);
    };
    for round in 0..3 {
        let base = 0.1 + round as f64 * 2.0;
        mem_sys::local_reset();
        assert!(ui.animate(&view, normal, instant));
        ui.frame(&input().time(base), &mut build).unwrap();
        assert!(ui.animate(&view, large, spring));
        ui.frame(&input().time(base + 0.1), &mut build).unwrap();
        shown(&ui, Rect::new(32.0, 88.5, 42.0, 4.5));
        let before = ui.rect(&child);
        assert!(ui.animate(&view, normal, linear));
        ui.frame(&input().time(base + 0.1), &mut build).unwrap();
        assert_eq!(ui.rect(&child), before, "retargeting cannot jump");
        ui.frame(&input().time(base + 0.2), &mut build).unwrap();
        shown(&ui, Rect::new(76.0, 69.25, 31.0, 7.25));
        ui.frame(&input().time(base + 0.3), &mut build).unwrap();
        shown(&ui, Rect::new(120.0, 50.0, 20.0, 10.0));
        let keys = [
            Keyframe::scale(0.2, 2.0, 0.5).anchor(1.0, 1.0).curve(spring.with_duration(99.0)),
            Keyframe::scale(0.4, 1.0, 1.0).anchor(1.0, 1.0).curve(Curve::Steps { duration: 99.0, count: 2 }),
        ];
        assert!(ui.animate_keys(&view, &keys, &Keyframes::DEFAULT));
        for (dt, expected) in [(0.1, Rect::new(32.0, 88.5, 42.0, 4.5)), (0.3, Rect::new(80.0, 67.5, 30.0, 7.5)), (0.4, Rect::new(120.0, 50.0, 20.0, 10.0))] {
            ui.frame(&input().time(base + 0.3 + dt), &mut build).unwrap();
            shown(&ui, expected);
        }
        let allocations = mem_sys::local_alloc_count();
        assert!(round == 0 || allocations == 0, "warm scale animations allocated {allocations} times");
        assert_eq!(builds.get(), built);
        assert!(!ui.wants_frame(&[], base + 1.9));
        assert_eq!(ui.layer(0).expect("base layer").instances, instances.as_slice(), "scale must only update the transform table");
    }
    let mut theme = crate::ThemeHandle::default();
    theme.reduced_motion(true);
    ui.set_theme(&theme).unwrap();
    assert!(ui.tokens().reduced_motion);
    ui.frame(&input().time(7.0), &mut build).unwrap();
    assert!(ui.animate(&view, large, spring));
    ui.frame(&input().time(7.0), &mut build).unwrap();
    shown(&ui, Rect::new(40.0, 85.0, 40.0, 5.0));
    assert!(!ui.wants_frame(&[], 7.1));
    }
}

#[test]
fn sampled_and_stepped_animations_keep_overshoot_and_allocate_nothing_when_warm() {
    use crate::{ui::{Curve, Keyframe, Keyframes, Look, Node, Target}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let table = Curve::Table {
        duration: 0.4, count: 4,
        positions: [0.0, 0.25, 0.5, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        values: [0.0, 1.2, 0.9, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    };
    let instant = Curve::Steps { duration: 0.0, count: 1 };
    let steps = Curve::Steps { duration: 0.2, count: 2 };
    let chip = Cell::new(None);
    let builds = Cell::new(0);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    ui.verify_views(false); // the test counts kept views' rebuilds or allocations; verification rebuilds them
    let mut build = |ui: &mut Ui| {
        ui.column(Node::column().w_px(250.0).h_px(50.0), |ui| { ui.view_of("animation", &1, |ui| {
            builds.set(builds.get() + 1);
            chip.set(Some(ui.node("chip", Node::DEFAULT.w_px(40.0).h_px(20.0).look(Look::NONE.fill(pxui_types::geom::Rgba::rgb(200, 40, 40))))));
        }); });
    };
    for time in [0.0, 0.1] { ui.frame(&input().time(time), &mut build).unwrap(); }
    let chip = chip.get().unwrap();
    let built = builds.get();
    let x = |ui: &Ui| ui.layer(0).expect("base layer").instances.iter().find(|i| i.color[0] == 200).unwrap().rect[0];
    for round in 0..3 {
        let base = 0.1 + round as f64 * 2.0;
        mem_sys::local_reset();
        assert!(ui.animate(&chip, Target::Offset(0.0, 0.0), instant));
        ui.frame(&input().time(base), &mut build).unwrap();
        assert!(ui.animate(&chip, Target::Offset(100.0, 0.0), table));
        for (dt, expected) in [(0.1, 120.0), (0.2, 90.0), (0.3, 95.0), (0.4, 100.0)] {
            ui.frame(&input().time(base + dt), &mut build).unwrap();
            assert!((x(&ui) - expected).abs() < 0.001, "table {dt}: {}", x(&ui));
            assert_eq!(ui.repaint_at().is_some(), dt < 0.4);
        }
        assert!(ui.animate(&chip, Target::Offset(0.0, 0.0), steps));
        for (dt, expected) in [(0.02, 100.0), (0.1, 50.0), (0.15, 50.0), (0.2, 0.0)] {
            ui.frame(&input().time(base + 0.4 + dt), &mut build).unwrap();
            assert!((x(&ui) - expected).abs() < 0.001, "steps {dt}: {}", x(&ui));
        }
        let keys = [
            Keyframe::offset(0.4, 100.0, 0.0).curve(table.with_duration(99.0)),
            Keyframe::offset(0.6, 0.0, 0.0).curve(steps.with_duration(99.0)),
        ];
        assert!(ui.animate_keys(&chip, &keys, &Keyframes::DEFAULT));
        for (dt, expected) in [(0.1, 120.0), (0.2, 90.0), (0.4, 100.0), (0.5, 50.0), (0.6, 0.0)] {
            ui.frame(&input().time(base + 0.6 + dt), &mut build).unwrap();
            assert!((x(&ui) - expected).abs() < 0.001, "keys {dt}: {}", x(&ui));
        }
        let allocations = mem_sys::local_alloc_count();
        assert!(round == 0 || allocations == 0, "warm animation allocated {allocations} times");
        assert_eq!(builds.get(), built);
        assert!(!ui.wants_frame(&[], base + 1.9));
    }
}

#[test]
fn queries_boxes_patches_and_text_state_have_the_engine_shape() {
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    assert_eq!(ui.font_size(), 13.0);
    ui.set_font_size(14.0);
    assert_eq!((ui.font_size(), ui.row_height()), (14.0, 25.0));
    // fractions are kept: the body size, its row, and the theme's body token
    ui.set_font_size(14.5);
    assert_eq!((ui.font_size(), ui.row_height()), (14.5, 25.5));
    ui.set_font_size(13.0);
    let mut seen = false;
    ui.frame(&input(), &mut |ui: &mut Ui| {
        assert!(!ui.captured() && !ui.text_input_focused() && !ui.verifying());
        assert_eq!(ui.passed_undo(), None::<UndoKind>);
        let b = ui.node("scroller", Node::DEFAULT.flags(pxui_types::Flags::CLICKABLE | pxui_types::Flags::SCROLL | pxui_types::Flags::CLIP).w(Size::Px(100.0)).h(Size::Px(60.0)).at(10.0, 20.0).scroll_step(12.0));
        assert_eq!(ui.scroll(&b).settled, 0.0);
        ui.set_scroll_offset(&b, 5.0);
        assert_eq!((ui.scroll(&b).settled, ui.scroll(&b).painted), (5.0, 5.0));
        assert_eq!(ui.text_state(&b), None);
        ui.set_text_state(&b, Some("kept".into()));
        assert_eq!(ui.text_state(&b), Some("kept"));
        assert_eq!(ui.cached_label(&b, 0, "label"), "label");
        ui.set_at(&b, (11.0, 21.0));
        let _ = ui.patch_rect(&b, Rect::new(0.0, 0.0, 1.0, 1.0));
        let _ = ui.patch_mark(&b, Mark::None);
        let _ = ui.local_rect(&b);
        assert!(!ui.hovered(&b, false) && !ui.hovered(&b, true));
        assert_eq!(ui.context_at(&b), None);
        assert!(ui.drop_target(&b).is_none());
        assert!(ui.press_keys(&b).is_empty());
        assert_eq!(ui.root_at(crate::Point { x: 1.0, y: 1.0 }, &[]), None);
        ui.carry(None, "kind", "value");
        let payload = ui.carrying().expect("a key carry is held at once");
        assert_eq!((payload.kind, payload.value), ("kind", "value"));
        ui.cancel_carry();
        assert_eq!(ui.carrying(), None);
        ui.request_cursor(Cursor::Text);
        ui.repaint_after(0.5);
        seen = true;
    }).expect("frame");
    assert!(seen);
    assert_eq!(ui.frame_index(), 1);
    assert_eq!(ui.expanded("nothing"), None);
    assert!(!ui.hover_delay("x"));
    assert!(ui.bytes().atlas >= 0 && ui.phase_times().len() == 7);
    assert_eq!(ui.rebuilt_named("no such view"), 0);
}

#[test]
fn a_caller_bug_panics_with_the_librarys_message_at_the_call() {
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    let stale = crate::Handle { index: 0, box_key: 1, box_slot: 99, generation: 7, ui: 0 };
    let err = catch_unwind(AssertUnwindSafe(|| { let _ = ui.scroll(&stale).settled; })).expect_err("a stale box is refused");
    let message = err.downcast_ref::<String>().expect("a formatted message");
    assert!(message.contains("stale box"), "{message}");
    // linked statically too, with nothing carried: the shortcut refuses the box as the export does
    let err = catch_unwind(AssertUnwindSafe(|| { let _ = ui.drop_target(&stale); })).expect_err("a stale drop target is refused");
    assert!(err.downcast_ref::<String>().is_some_and(|m| m.contains("stale box")));
    let err = catch_unwind(AssertUnwindSafe(|| ui.set_font_size(0.0))).expect_err("a font size of zero is refused");
    assert!(err.downcast_ref::<String>().is_some_and(|m| m.contains("font size")));
    assert_eq!(ui.font_size(), 13.0, "the refused call changed nothing");
}

#[test]
fn inspector_bar_reuses_large_hint_storage_on_warm_frames() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let hints = [("Esc", "close"); 17];
    let mut build = |ui: &mut Ui| crate::ext::inspector_bar(ui, &hints, i64::MIN);
    for _ in 0..4 { ui.frame(&input(), &mut build).unwrap(); }
    mem_sys::local_reset();
    for _ in 0..4 { ui.frame(&input(), &mut build).unwrap(); }
    assert_eq!(mem_sys::local_alloc_count(), 0);
}

#[test]
fn inspector_header_lends_the_title_without_allocating_on_warm_frames() {
    use pxui_types::ui::HeadAction;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let actions: [HeadAction; 17] = std::array::from_fn(|_| HeadAction { caption: "Run".into(), keycap: "R".into(), active: false, usable: true });
    let rename: std::rc::Rc<dyn Fn(&str) -> bool> = std::rc::Rc::new(|name| !name.is_empty());
    let mut build = |ui: &mut Ui| {
        let head = crate::ext::inspector_header(ui, "head", Some("Kind"), None, None, Some(rename.clone()), &actions, Some("Reset"), true, "Title", "Detail");
        assert_eq!((head.renamed, head.chosen, head.reset_pressed), ("Title", None, false));
    };
    for tick in 0..8 { ui.frame(&input().time(f64::from(tick) / 60.0), &mut build).unwrap(); }
    mem_sys::local_reset();
    for tick in 8..12 { ui.frame(&input().time(f64::from(tick) / 60.0), &mut build).unwrap(); }
    assert_eq!(mem_sys::local_alloc_count(), 0);
}

#[test]
fn inspector_header_lends_an_accepted_rename() {
    use crate::{Event, KeyCode, Str, testing};
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let valid: std::rc::Rc<dyn Fn(&str) -> bool> = std::rc::Rc::new(|title| title == "Renamed");
    let changed = std::cell::Cell::new(false);
    let mut build = |ui: &mut Ui| {
        let head = crate::ext::inspector_header(ui, "rename", None, None, None, Some(valid.clone()), &[], None, false, "Title", "");
        changed.set(changed.get() || head.renamed == "Renamed");
    };
    testing::frame(&mut ui, &[], &mut build);
    testing::click(&mut ui, 20.0, 20.0, &mut build);
    let input = { let mut record = Event::default(); record.kind = Event::TEXT_INPUT; record.text = Str::from("Renamed"); record };
    testing::frame(&mut ui, &[input], &mut build);
    testing::key(&mut ui, KeyCode::Enter, &mut build);
    assert!(changed.get(), "the borrowed response contains the committed title");
}

#[test]
fn kit_extras_build_and_report_through_the_engine_shapes() {
    use pxui_types::{theme::Curve, ui::{HeadAction, LineStep, Target}, Axis};
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    let hints = [("Esc", "close")];
    let actions: [HeadAction; 17] = std::array::from_fn(|_| HeadAction { caption: "Run".into(), keycap: "R".into(), active: false, usable: true });
    for round in 0..3 {
        ui.frame(&input(), &mut |ui: &mut Ui| {
            ui.panel("extras", &crate::Overlay::DEFAULT, &mut |ui: &mut Ui| {
                assert!(!ui.button("styled", "Styled", &crate::Button::DEFAULT.danger().align(pxui_types::layout::Justify::End)).clicked);
                let head = ui.inspector_head_for_test(&actions);
                assert_eq!(head.renamed, "Title");
                crate::ext::inspector_body(ui, &mut |ui: &mut Ui| {
                    assert!(crate::ext::inspector_section(ui, "section", true, None, "Section", &mut |ui: &mut Ui| { ui.label("inside", crate::Label::DEFAULT); }));
                    assert!(!crate::ext::inspector_toggle(ui, "toggle", "Toggle", false));
                });
                crate::ext::inspector_bar(ui, &hints, 3);
                let line = crate::ext::line_field(ui, "find", "abc", pxui_types::geom::Rect::new(0.0, 0.0, 120.0, 20.0), &crate::LineField::DEFAULT.placeholder("line"));
                let (text, step) = (line.changed, line.step);
                assert_eq!((text, step), (None, LineStep::None));
                let (grip, signal) = crate::ext::grip(ui, format_args!("grip-{}", 1), Axis::Row, None);
                assert!(!signal.clicked);
                assert_eq!(crate::ext::field_drag(ui, &grip), None);
                if round == 0 { assert!(ui.animate(&grip, Target::Opacity(0.5), Curve::Spring { omega: 30.0 }), "a new target starts moving"); }
                let mut ran = false;
                let open = ui.popup("pop", &crate::Overlay::DEFAULT.at(crate::Point::new(10.0, 10.0)).w_px(100.0).h_px(40.0).max_h_px(100.0).dismiss_initial(), &mut |ui: &mut Ui| { ran = true; ui.label("popped", crate::Label::DEFAULT); }).open;
                assert_eq!(open, ran, "the build runs exactly while the popup is open");
            });
        }).expect("frame");
    }
}

impl Ui {
    fn inspector_head_for_test(&mut self, actions: &[pxui_types::ui::HeadAction]) -> pxui_types::ui::Head<'_> {
        crate::ext::inspector_header(self, "head", Some("🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀"), None, Some("1"), Some(std::rc::Rc::new(|name: &str| !name.is_empty())), actions, Some("reset"), true, "Title", "detail")
    }
}

#[test]
fn a_theme_built_from_key_values_scopes_over_a_subtree_and_refuses_bad_keys() {
    use crate::{Error, ThemeHandle};
    let theme = ThemeHandle::parse("tokens.accent = #ff0000\nbutton.hover.fill = #00ff00").unwrap();
    for invalid in ["button.nonsense = 1", "tokens.accent = red", "toggle.body = two words"] {
        assert!(matches!(ThemeHandle::parse(invalid), Err(Error::InvalidInput)), "{invalid}");
    }
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    let mut inside = None;
    ui.frame(&input(), &mut |ui: &mut Ui| {
        let outer = ui.tokens().accent;
        ui.themed(&theme, &mut |ui: &mut Ui| {
            inside = Some(ui.tokens().accent);
            assert_eq!(ui.theme().button.hover.fill, crate::Rgba::rgb(0, 255, 0));
        });
        assert_eq!(ui.tokens().accent, outer, "the Ui's own theme is back");
    }).expect("frame");
    assert_eq!(inside, Some(pxui_types::geom::Rgba::rgb(255, 0, 0)));
}

#[test]
fn canvas_text_goes_through_the_uis_own_text_service() {
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    let style = crate::Label::BODY;
    let mut text = ui.text_service();
    let metrics = text.measure("abc", style, None).expect("measure");
    assert!(metrics.width > 0.0 && metrics.lines == 1);
    let glyphs = { let mut glyphs=Vec::new(); text.quads("abc",style,None,crate::Point::new(10.0,20.0), 1.0, &mut glyphs).map(|_|glyphs) }.expect("quads");
    assert_eq!(glyphs.len(), 3, "one quad per glyph");
    assert!(glyphs.iter().all(|g| g.rect[0] >= 10.0));
    let (w, h, pixels) = { let mut px=Vec::new(); text.rasterize_rgba("abc",style,None, 1.0, [255, 255, 255, 255], &mut px).map(|(w,h)|(w,h,px)) }.expect("rasterize");
    assert_eq!(pixels.len(), (w * h * 4) as usize);
    let bad = style.size(f32::NAN);
    assert!(text.measure("abc", bad, None).is_err(), "a NaN size is refused");
}

#[test]
fn second_atlas_pages_reach_public_quads_and_both_rasterizers() {
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let style=crate::Label::BODY.size(600.0);
    let resolved=ui.resolve(&style);
    let mut text=ui.text_service();
    let first={ let mut glyphs=Vec::new(); text.quads("A",style,None,crate::Point::new(0.0,0.0),2.0, &mut glyphs).map(|_|glyphs) }.unwrap();
    let glyphs={ let mut glyphs=Vec::new(); text.quads("ABCDEF",style,None,crate::Point::new(0.0,0.0),2.0, &mut glyphs).map(|_|glyphs) }.unwrap();
    assert_eq!(glyphs.len(),6);
    assert_eq!(glyphs[0],first[0],"page growth preserves the first public quad");
    assert!(glyphs.iter().any(|g|g.atlas==0) && glyphs.iter().any(|g|g.atlas==2));
    assert!(glyphs.iter().all(|g|g.atlas==0 || g.atlas==2));
    let (w,h,pixels)={ let mut px=Vec::new(); text.rasterize_rgba("ABCDEF",style,None,2.0,[17,33,65,255], &mut px).map(|(w,h)|(w,h,px)) }.unwrap();
    let mut shaper=crate::Text::new(&[crate::pragmasevka()]).unwrap();
    let (sw,sh,rgba)={ let mut px=Vec::new(); shaper.rasterize_rgba("ABCDEF",&resolved,0.0,2.0,[17,33,65,255], &mut px).map(|(w,h)|(w,h,px)) }.unwrap();
    assert_eq!((w,h),(sw,sh));
    assert_eq!(pixels,rgba,"standalone and UI mirrors sample both pages identically");
    for glyph in &glyphs {
        let [x,y,gw,gh]=glyph.rect;
        assert!((y.max(0.0) as usize..((y+gh).ceil() as usize).min(h as usize)).any(|row| {
            let start=(row*w as usize+x.max(0.0) as usize)*4;
            let end=(row*w as usize+((x+gw).ceil() as usize).min(w as usize))*4;
            pixels[start..end].chunks_exact(4).any(|p|p[3]>0)
        }),"each page-backed quad has raster coverage: {glyph:?}");
    }
    assert_eq!({ let mut glyphs=Vec::new(); text.quads("ABCDEF",style,None,crate::Point::new(0.0,0.0),2.0, &mut glyphs).map(|_|glyphs) }.unwrap(),glyphs);
}

#[test]
fn value_fields_take_their_options_and_text_edits_are_kept() {
    use crate::{ValueCallbacks, ValueField};
    use pxui_types::ui::TextEdit;
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    let opts = ValueField::DEFAULT.fraction(0.5).left(true).kept_drag(true).trail("px", pxui_types::geom::Rgba::rgb(1, 2, 3));
    let (accept, slide, scrub) = (|t: &str| t.parse::<f32>().is_ok(), |f: f32| format!("{f:.2}"), |origin: &str, dx: f32, _: bool| format!("{origin}{dx}"));
    let callbacks = ValueCallbacks::DEFAULT.valid(&accept).slide(&slide).scrub(&scrub);
    for _ in 0..2 {
        ui.frame(&input(), &mut |ui: &mut Ui| {
            ui.panel("fields", &crate::Overlay::DEFAULT, &mut |ui: &mut Ui| {
                let field = crate::ext::value_field(ui, "amount", "0.25", pxui_types::geom::Rect::new(10.0, 10.0, 120.0, 20.0), &opts, &callbacks);
                let (text, editing) = (field.changed, field.editing);
                assert_eq!((text, editing), (None, false));
                let edit = ui.load_text_edit(crate::Key::Resolved(7), "abc");
                assert_eq!((edit.text.as_str(), edit.caret, edit.anchor), ("abc", 3, 3));
                ui.remember(crate::Key::Resolved(7), &TextEdit { text: "abc".into(), caret: 1, anchor: 1 }, false);
                assert!(ui.text_undo_bytes() >= 3, "the snapshot is held");
            });
        }).expect("frame");
    }
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ui.frame(&input(), &mut |ui: &mut Ui| ui.remember(crate::Key::Resolved(7), &TextEdit { text: "é".into(), caret: 1, anchor: 0 }, false));
    })).expect_err("a caret inside a character is a caller bug");
    assert!(err.downcast_ref::<String>().is_some_and(|m| m.contains("caret and anchor")));
}

#[test]
fn a_shaper_of_its_own_measures_and_rasterizes_off_the_ui() {
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    let style = ui.resolve(&crate::Label::BODY);
    let mut text = crate::Text::new(&[crate::pragmasevka()]).expect("a shaper");
    let metrics = text.measure("abc", &style, 0.0).expect("measure");
    assert!(metrics.width > 0.0);
    // the same text measures the same in the Ui's own service: one font, two homes
    assert_eq!(ui.text_service().measure("abc", crate::Label::BODY, None).expect("service").width, metrics.width);
    let (w, h, pixels) = { let mut px=Vec::new(); text.rasterize_rgba("abc", &style, 0.0, 1.0, [255, 255, 255, 255], &mut px).map(|(w,h)|(w,h,px)) }.expect("rasterize");
    assert_eq!(pixels.len(), (w * h * 4) as usize);
    assert!(pixels.chunks(4).any(|p| p[3] != 0), "something was inked");
}

#[test]
fn a_stale_field_patches_nothing_and_whole_source_files_round_trip() {
    use pxui_types::ui::TextEdit;
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    let stale = crate::Handle { index: 0, box_key: 1, box_slot: 99, generation: 7, ui: 0 };
    // the engine answers a stale handle with false, so does the one export that replaced the wrapper's several calls
    assert!(!ui.patch_field(&stale, &crate::ValueField::DEFAULT, "1.5"));
    assert!(!ui.patch_text(&stale, "x"));
    // text far over the old 1 MiB limit crosses whole in both directions, and a refused call is a loud one
    let source = "(graph g)\n".repeat(300_000);
    assert!(source.len() > 1 << 21);
    ui.frame(&input(), &mut |ui: &mut Ui| {
        let edit: TextEdit = ui.load_text_edit(crate::Key::Resolved(9), &source);
        assert_eq!(edit.text.len(), source.len(), "the edit holds the whole text");
        let text = ui.text_area("area", &source, pxui_types::Node::DEFAULT.at(0.0, 0.0).w_px(100.0).h_px(100.0), &crate::TextArea::DEFAULT, &crate::AreaCallbacks::DEFAULT).changed;
        assert_eq!(text, None, "the whole text crossed and compared equal");
    }).expect("frame");
}


#[test]
fn text_area_borrows_its_text_and_allocates_nothing_on_warm_frames() {
    use crate::{AreaCallbacks, Splice, TextArea};
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    let source = "alpha\nbeta\ngamma";
    let plain = TextArea::DEFAULT;
    for tick in 0..12 {
        // warm-up and measured frames make the same call; the first 8 only warm the caches
        mem_sys::local_reset();
        let scope = (tick >= 8).then(mem_sys::warm_scope);
        ui.frame(&input().time(tick as f64 / 60.0), &mut |ui: &mut Ui| {
            ui.panel("borrowed", &crate::Overlay::DEFAULT, &mut |ui: &mut Ui| {
                let area = ui.text_area("area", source, pxui_types::Node::DEFAULT.at(0.0, 0.0).w_px(100.0).h_px(100.0), &plain, &AreaCallbacks::DEFAULT);
                assert_eq!((area.changed, area.submitted), (None, false));
            });
        }).expect("frame");
        let allocated = mem_sys::local_alloc_count();
        drop(scope);
        if tick >= 8 {
            assert_eq!(allocated, 0, "text_area on warm frame {tick}");
            assert!(mem_sys::take_reports().is_empty(), "warm frame {tick} allocated");
        }
    }
    // a splice into the focused area changes the text: it comes back borrowed
    let splices = [Splice::new(0, 5, "ALPHA")];
    let edit = TextArea::DEFAULT.select(0, 0).splices(&splices);
    let mut changed = None;
    for tick in 12..16 {
        ui.frame(&input().time(tick as f64 / 60.0), &mut |ui: &mut Ui| {
            ui.panel("borrowed", &crate::Overlay::DEFAULT, &mut |ui: &mut Ui| {
                changed = changed.take().or(ui.text_area("area", source, pxui_types::Node::DEFAULT.at(0.0, 0.0).w_px(100.0).h_px(100.0), &edit, &AreaCallbacks::DEFAULT).changed.map(str::to_owned));
            });
        }).expect("frame");
    }
    assert_eq!(changed.as_deref(), Some("ALPHA\nbeta\ngamma"));
}

#[test]
fn a_text_area_with_data_callbacks_and_a_language_and_a_value_field_with_callbacks_allocate_nothing_on_warm_frames() {
    use crate::{AreaCallbacks, AreaNote, Language, TextArea, ValueCallbacks, ValueField};
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    let source = "alpha\nbeta\ngamma";
    let notes = [AreaNote::new(1, "unused"), AreaNote::new(2, "shadowed").range(0, 3)];
    let chips = [(0, 5, pxui_types::geom::Rgba::rgb(90, 140, 220))];
    let (errors, spans) = ([2usize], [(6usize, 10usize)]);
    let area = TextArea::DEFAULT.notes(&notes).chips(&chips).errors(&errors).spans(&spans).wrap(true);
    // closures borrowing locals, a language with pairs and a comment, and a value field with valid and slide
    let clicks = std::cell::Cell::new(0usize);
    let click = |_at: usize, _modified: bool| clicks.set(clicks.get() + 1);
    let caret = |_at: usize| {};
    let callbacks = AreaCallbacks::DEFAULT.on_click(&click).on_caret(&caret).language(Language::DEFAULT.pairs(&[('(', ')'), ('[', ']')]).comment("//"));
    let accept = |t: &str| t.parse::<f32>().is_ok();
    let slide = |f: f32| format!("{f:.2}");
    let value_callbacks = ValueCallbacks::DEFAULT.valid(&accept).slide(&slide);
    for tick in 0..12 {
        mem_sys::local_reset();
        let scope = (tick >= 8).then(mem_sys::warm_scope);
        ui.frame(&input().time(tick as f64 / 60.0), &mut |ui: &mut Ui| {
            ui.panel("data", &crate::Overlay::DEFAULT, &mut |ui: &mut Ui| {
                let r = ui.text_area("area", source, pxui_types::Node::DEFAULT.at(0.0, 0.0).w_px(100.0).h_px(100.0), &area, &callbacks);
                assert_eq!((r.changed, r.submitted), (None, false));
                let v = crate::ext::value_field(ui, "amount", "0.25", pxui_types::geom::Rect::new(0.0, 110.0, 100.0, 20.0), &ValueField::DEFAULT.fraction(0.5), &value_callbacks);
                assert_eq!((v.changed, v.editing), (None, false));
            });
        }).expect("frame");
        let allocated = mem_sys::local_alloc_count();
        drop(scope);
        if tick >= 8 {
            assert_eq!(allocated, 0, "a text area with data, callbacks and a language, and a value field with callbacks, allocated on warm frame {tick}");
            assert!(mem_sys::take_reports().is_empty(), "warm frame {tick} allocated");
        }
    }
}

#[test]
fn tabular_digits_reach_shaping_and_keep_warm_layouts_allocation_free() {
    const FONT: &[u8]=include_bytes!("../../../../assets/fonts/Inter-Regular.ttf");
    mem_sys::set_peer(crate::library_counter);
    let mut text=crate::Text::new(&[FONT]).expect("shaper");
    let ui=Ui::new(&crate::UiDesc::new(crate::Platform::Apple).fonts(&[crate::FontData(FONT)])).expect("pxui Ui");
    let plain=ui.resolve(&crate::Label::BODY.size(20.0));
    let tabular=ui.resolve(&crate::Label::BODY.size(20.0).tabular());
    let proportional=text.measure("1111",&plain,0.0).unwrap();
    let tabular_one=text.measure("1111",&tabular,0.0).expect("tabular feature is accepted");
    let tabular_eight=text.measure("8888",&tabular,0.0).unwrap();
    assert!((proportional.width-tabular_one.width).abs()>1.0,"font fixture must exercise a real substitution");
    assert!((tabular_one.width-tabular_eight.width).abs()<1e-5,"equal digit counts have equal advances");
    assert_eq!(text.measure("1111",&plain,0.0).unwrap(),proportional,"feature changes have distinct cached layouts");
    text.measure("1111",&tabular,0.0).unwrap();
    for _ in 0..4 {
        mem_sys::local_reset();
        assert_eq!(text.measure("1111",&tabular,0.0).unwrap(),tabular_one);
        assert_eq!(text.measure("8888",&tabular,0.0).unwrap(),tabular_eight);
        assert_eq!(mem_sys::local_alloc_count(),0,"warm feature-bearing layouts allocate nothing across the library boundary");
    }
}


#[test]
fn canvas_feature_styles_have_distinct_warm_cache_entries() {
    const FONT: &[u8]=include_bytes!("../../../../assets/fonts/Inter-Regular.ttf");
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::new(crate::Platform::Apple).fonts(&[crate::FontData(FONT)])).expect("pxui Ui");
    let a=ui.text_service().measure("1111",crate::Label::BODY.size(20.0),None).unwrap();
    let b=ui.text_service().measure("1111",crate::Label::BODY.size(20.0).tabular(),None).unwrap();
    assert!((a.width-b.width).abs()>1.0);
    for frame in 0..7 {
        mem_sys::local_reset();
        ui.frame(&input().time(frame as f64/60.0),&mut |ui| {
            let bounds=ui.node("numbers",crate::Node::DEFAULT.w_px(240.0).h_px(80.0));
            ui.canvas(&bounds,move |paint,rect| {
                assert_eq!(paint.text(crate::Point::new(rect.x, rect.y), crate::Label::BODY.size(20.0), 0.0, "1111"),a);
                assert_eq!(paint.text(crate::Point::new(rect.x, rect.y+30.0), crate::Label::MONO.size(20.0), 0.0, "1111"),b);
            });
        }).unwrap();
        if frame>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"mixed feature styles must keep separate warm layout/glyph caches"); }
    }
}


#[test]
fn inline_spans_on_nodes_preserve_labels_opacity_and_zero_allocation_rebuilds() {
    use crate::TextSpan;
    use pxui_types::geom::Rgba;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.set_a11y(true);
    let text="ab CD ef";
    let label=crate::Label::BODY.size(20.0).line_height(24.0);
    let style=ui.resolve(&label);
    let span={ let mut record = TextSpan::new(3, 5); record.flags = TextSpan::COLOR|TextSpan::BACKGROUND|TextSpan::UNDERLINE; record.color = Rgba::rgba(255,0,0,255); record.background = Rgba::rgba(0,0,255,64); record.underline = Rgba::rgba(0,255,0,255); record };
    let mut shaper=crate::Text::new(&[crate::pragmasevka()]).unwrap();
    let measured=shaper.measure_spans(text,&style,200.0,&[span]).unwrap();
    assert_eq!(ui.text_service().measure_spans(text,label,Some(200.0),&[span]).unwrap(),measured);
    for frame in 0..10 {
        let plain=frame%3==0;
        let opacity=if frame%2==0 { 0.5 } else { 1.0 };
        mem_sys::local_reset();
        ui.frame(&input().time(frame as f64/60.0),&mut |ui| {
            let b=ui.node("label",crate::Node::DEFAULT.role(crate::ui::Role::Label).w_px(200.0).h_px(40.0).look(crate::ui::Look::NONE.text(Rgba::rgba(30,30,30,255)).opacity(opacity)));
            if plain { ui.text(&b,text); } else { ui.text_spans(&b,text,label,&[span]).unwrap(); }
        }).unwrap();
        if frame>=6 { assert_eq!(mem_sys::local_alloc_count(),0,"warm rich/plain rebuilds retain their span storage and glyph caches"); }
        let alpha=if opacity==0.5 { 128 } else { 255 };
        let has_red=ui.layers().any(|l|l.instances.iter().any(|i|i.color==[255,0,0,alpha]));
        assert_eq!(has_red,!plain,"plain text clears previous span colours");
        if !plain {
            assert!(ui.layers().any(|l|l.instances.iter().any(|i|i.color==[0,255,0,alpha])));
            let background=if opacity==0.5 { 32 } else { 64 };
            assert!(ui.layers().any(|l|l.instances.iter().any(|i|i.color==[0,0,255,background])));
        }
        assert!(ui.a11y_nodes().iter().any(|node|ui.a11y_name_of(node)==text),"styled nodes expose the complete original label");
    }
}

#[test]
fn inline_canvas_styles_keep_independent_caches_and_reject_invalid_ranges() {
    use crate::TextSpan;
    use pxui_types::geom::Rgba;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let label=crate::Label::BODY.size(20.0);
    let red={ let mut record = TextSpan::new(0, 2); record.flags = TextSpan::COLOR|TextSpan::UNDERLINE; record.color = Rgba::rgba(255,0,0,255); record.underline = Rgba::rgba(255,0,0,255); record };
    let green={ let mut record = red; record.color = Rgba::rgba(0,255,0,255); record.underline = Rgba::rgba(0,255,0,255); record };
    for bad in [{ let mut record = red; record.start = 1; record.end = 2; record },{ let mut record = red; record.end = 99; record },{ let mut record = red; record.flags = 8; record }] {
        assert!(ui.text_service().measure_spans("é!",label,None,&[bad]).is_err());
    }
    assert!(ui.text_service().measure_spans("abcdefghij",label,None,&[red;9]).is_err());
    for frame in 0..7 {
        mem_sys::local_reset();
        ui.frame(&input().time(frame as f64/60.0),&mut |ui| {
            let b=ui.node("canvas",crate::Node::DEFAULT.w_px(200.0).h_px(80.0));
            ui.canvas(&b,move |paint,rect| {
                let a=paint.text_spans(crate::Point::new(rect.x,rect.y),label,0.0,"ab cd",&[red]).unwrap();
                let b=paint.text_spans(crate::Point::new(rect.x,rect.y+30.0),label,0.0,"ab cd",&[green]).unwrap();
                assert_eq!(a,b,"decoration colour changes preserve text geometry");
            });
        }).unwrap();
        if frame>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"warm mixed spans keep separate shape/glyph caches"); }
    }
}


#[test]
fn inline_node_fractional_font_alignment_matches_canvas_text() {
    use crate::TextSpan;
    use pxui_types::geom::Rgba;
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut theme = crate::ThemeHandle::default();
    theme.set_color(crate::theme::ColorToken(0), Rgba::rgba(0, 0, 0, 255));
    ui.set_theme(&theme).unwrap();
    let label=crate::Label::BODY.size(20.25).color(crate::theme::ColorToken(0));
    let span={ let mut record = TextSpan::new(0, 2); record.flags = TextSpan::COLOR; record.color = Rgba::rgba(255,0,0,255); record };
    let shift=em_top_to_layout_top(20.25);
    ui.frame(&input(),&mut |ui| {
        let b=ui.node("fractional",crate::Node::DEFAULT.w_px(200.0).h_px(40.0).look(crate::ui::Look::NONE.text(Rgba::rgba(0,0,0,255))));
        ui.text_spans(&b,"ab cd",label,&[span]).unwrap();
        ui.canvas(&b,move |paint,rect| {
            let top=rect.y+((rect.h-20.25)/2.0).floor()+shift;
            paint.text_spans(crate::Point::new(rect.x,top),label,200.0,"ab cd",&[span]).unwrap();
        });
    }).unwrap();
    let instances:Vec<_>=ui.layers().flat_map(|layer|layer.instances.iter().copied()).collect();
    assert_eq!(instances.len(),8);
    for (node,canvas) in instances[..4].iter().zip(&instances[4..]) {
        assert_eq!(node.rect,canvas.rect,"fractional font points must not be rounded before node alignment");
        assert_eq!(node.aux,canvas.aux);
        assert_eq!(node.color,canvas.color);
    }
}


#[test]
fn inline_node_wrapped_alignment_matches_the_complete_canvas_block() {
    use crate::TextSpan;
    use pxui_types::geom::Rgba;
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut theme = crate::ThemeHandle::default();
    theme.set_color(crate::theme::ColorToken(0), Rgba::rgba(0, 0, 0, 255));
    ui.set_theme(&theme).unwrap();
    let label=crate::Label::BODY.size(20.0).line_height(24.0).wrap().color(crate::theme::ColorToken(0));
    let span={ let mut record = TextSpan::new(3, 5); record.flags = TextSpan::COLOR; record.color = Rgba::rgba(255,0,0,255); record };
    let metrics=ui.text_service().measure_spans("aa bb cc",label,Some(45.0),&[span]).unwrap();
    assert_eq!(metrics.lines,3);
    let shift=em_top_to_layout_top(20.0);
    ui.frame(&input(),&mut |ui| {
        let b=ui.node("wrapped",crate::Node::DEFAULT.w_px(45.0).h_px(80.0).look(crate::ui::Look::NONE.text(Rgba::rgba(0,0,0,255))));
        ui.text_spans(&b,"aa bb cc",label,&[span]).unwrap();
        ui.canvas(&b,move |paint,rect| {
            let block=20.0+(metrics.lines-1) as f32*24.0;
            let top=rect.y+((rect.h-block)/2.0).floor()+shift;
            paint.text_spans(crate::Point::new(rect.x,top),label,45.0,"aa bb cc",&[span]).unwrap();
        });
    }).unwrap();
    let instances:Vec<_>=ui.layers().flat_map(|layer|layer.instances.iter().copied()).collect();
    assert_eq!(instances.len(),12);
    for (node,canvas) in instances[..6].iter().zip(&instances[6..]) {
        assert_eq!(node.rect,canvas.rect,"node alignment includes every wrapped line");
        assert_eq!(node.aux,canvas.aux);
        assert_eq!(node.color,canvas.color);
    }
}

#[test]
fn ellipsis_modes_match_standalone_ui_measurement_quads_and_rasterization() {
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut shaper=crate::Text::new(&[crate::pragmasevka()]).unwrap();
    let source="ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut previous=None;
    let body=crate::Label::BODY.size(20.0);
    for (cut,label) in [(false,body),(true,body.ellipsis()),(true,body.ellipsis_middle()),(true,body.ellipsis_head())] {
        let style=ui.resolve(&label);
        let a=shaper.measure(source,&style,100.0).unwrap();
        assert_eq!(a,ui.text_service().measure(source,label,Some(100.0)).unwrap());
        let raster={ let mut px=Vec::new(); shaper.rasterize_rgba(source,&style,100.0,1.0,[0,0,0,255], &mut px).map(|(w,h)|(w,h,px)) }.unwrap();
        assert_eq!(raster,{ let mut px=Vec::new(); ui.text_service().rasterize_rgba(source,label,Some(100.0),1.0,[0,0,0,255], &mut px).map(|(w,h)|(w,h,px)) }.unwrap());
        let glyphs={ let mut glyphs=Vec::new(); ui.text_service().quads(source,label,Some(100.0),crate::Point::new(0.0, 0.0),1.0, &mut glyphs).map(|_|glyphs) }.unwrap();
        assert!(!glyphs.is_empty());
        if cut { assert!(a.width<=100.001 && glyphs.len()<26); }
        if let Some(previous)=previous { assert_ne!(raster,previous,"mode changes select distinct cached layouts"); }
        previous=Some(raster);
    }
    // a `Label` holds an `Ellipsis` and a `TextStyle` comes only from `resolve`: an unknown number exists only in a C record, refused
    // where the library reads it (pxui_abi `ellipsis_modes_preserve_existing_codes_and_refuse_unknown_values`)
}

#[test]
fn ellipsis_modes_on_nodes_and_canvas_preserve_labels_fit_modes_and_warm_storage() {
    use crate::TextSpan;
    use pxui_types::geom::Rgba;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.set_a11y(true);
    let source="ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let spans=[{ let mut record = TextSpan::new(0, 1); record.flags = TextSpan::COLOR; record.color = Rgba::rgba(255,0,0,255); record },
        { let mut record = TextSpan::new(25, 26); record.flags = TextSpan::COLOR; record.color = Rgba::rgba(0,0,255,255); record }];
    let shift=em_top_to_layout_top(20.0);
    let mut theme = crate::ThemeHandle::default();
    theme.set_color(crate::theme::ColorToken(0), Rgba::rgba(0, 0, 0, 255));
    ui.set_theme(&theme).unwrap();
    let body=crate::Label::BODY.size(20.0).color(crate::theme::ColorToken(0));
    let labels=[body.ellipsis(),body.ellipsis_middle(),body.ellipsis_head()];
    for frame in 0..15 {
        let label=labels[frame%3];
        mem_sys::local_reset();
        ui.frame(&input().time(frame as f64/60.0),&mut |ui| {
            let b=ui.node("ellipsis",crate::Node::DEFAULT.flags(pxui_types::Flags::FIT_TEXT).role(crate::ui::Role::Label).w_px(100.0).h_px(40.0).look(crate::ui::Look::NONE.text(Rgba::rgba(0,0,0,255))));
            ui.text_spans(&b,source,label,&spans).unwrap();
            ui.canvas(&b,move |paint,rect| {
                let top=rect.y+((rect.h-20.0)/2.0).floor()+shift;
                paint.text_spans(crate::Point::new(rect.x,top),label,100.0,source,&spans).unwrap();
            });
        }).unwrap();
        if frame>=9 { assert_eq!(mem_sys::local_alloc_count(),0,"mixed modes retain independent layout/glyph caches"); }
        let instances:Vec<_>=ui.layers().flat_map(|l|l.instances.iter().copied()).collect();
        let n=instances.len()/2;
        assert!(n>0 && instances.len()%2==0);
        for (node,canvas) in instances[..n].iter().zip(&instances[n..]) {
            assert_eq!((node.rect,node.aux,node.color),(canvas.rect,canvas.aux,canvas.color),"pxui_types::Flags::FIT_TEXT preserves an explicit head/middle mode");
        }
        let red=instances[..n].iter().any(|i|i.color==[255,0,0,255]);
        let blue=instances[..n].iter().any(|i|i.color==[0,0,255,255]);
        assert_eq!((red,blue),[(true,false),(true,true),(false,true)][frame%3],"end/middle/head keep the right endpoints");
        assert!(ui.a11y_nodes().iter().any(|node|ui.a11y_name_of(node)==source),"truncation keeps the complete accessible source");
    }
}

#[cfg(target_os="macos")]
#[test]
fn system_fonts_reach_standalone_and_ui_shapers_without_warm_allocations() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut text=crate::Text::new(&[crate::pragmasevka()]).unwrap();
    let label=crate::Label::BODY.size(20.0);
    let style=ui.resolve(&label);
    let old={ let mut px=Vec::new(); text.rasterize_rgba("漢字",&style,300.0,1.0,[0,0,0,255], &mut px).map(|(w,h)|(w,h,px)) }.unwrap();
    text.enable_system_fonts().unwrap();
    ui.text_service().enable_system_fonts().unwrap();
    let new={ let mut px=Vec::new(); text.rasterize_rgba("漢字",&style,300.0,1.0,[0,0,0,255], &mut px).map(|(w,h)|(w,h,px)) }.unwrap();
    assert_ne!(old,new,"enabling invalidates an earlier missing-font layout");
    let cases=["漢字","العربية","😀"];
    let mut measured=[pxui_types::TextMetrics::default();3];
    for (i,source) in cases.into_iter().enumerate() {
        measured[i]=text.measure(source,&style,300.0).unwrap();
        assert_eq!(measured[i],ui.text_service().measure(source,label,Some(300.0)).unwrap());
        let (w,h,rgba)={ let mut px=Vec::new(); text.rasterize_rgba(source,&style,300.0,1.0,[0,0,0,255], &mut px).map(|(w,h)|(w,h,px)) }.unwrap();
        assert!(w>0 && h>0 && rgba.chunks_exact(4).any(|p|p[3]>0),"native fallback rasterizes {source}");
    }
    for _ in 0..4 {
        mem_sys::local_reset();
        text.enable_system_fonts().unwrap();
        ui.text_service().enable_system_fonts().unwrap();
        for (i,source) in cases.into_iter().enumerate() {
            assert_eq!(text.measure(source,&style,300.0).unwrap(),measured[i]);
            assert_eq!(ui.text_service().measure(source,label,Some(300.0)).unwrap(),measured[i]);
        }
        assert_eq!(mem_sys::local_alloc_count(),0,"warm fallback measurement crosses both ABI paths without allocating");
    }
}

#[cfg(target_os="macos")]
#[test]
fn system_font_nodes_keep_labels_warm_storage_and_refuse_late_enable() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.text_service().enable_system_fonts().unwrap();
    ui.set_a11y(true);
    let source="漢字 العربية 😀";
    let label=crate::Label::BODY.size(20.0);
    for frame in 0..7 {
        mem_sys::local_reset();
        ui.frame(&input().time(frame as f64/60.0),&mut |ui| {
            let b=ui.node("native-fonts",crate::Node::DEFAULT.role(crate::ui::Role::Label).w_px(300.0).h_px(40.0));
            ui.text_spans(&b,source,label,&[]).unwrap();
            ui.canvas(&b,move |paint,rect| { paint.text(crate::Point::new(rect.x, rect.y+50.0), crate::Label::BODY.size(20.0), 300.0, source); });
        }).unwrap();
        if frame>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"mixed-script nodes and canvas reuse warm layouts and glyphs"); }
        assert!(ui.a11y_nodes().iter().any(|node|ui.a11y_name_of(node)==source));
        assert!(!ui.layers().all(|layer|layer.instances.is_empty()));
    }
    assert_eq!(ui.text_service().enable_system_fonts(),Err(crate::Error::State));
    assert!(ui.last_error().contains("before the first frame"));
}

#[test]
fn system_preferences_invalidate_kept_views_snap_running_motion_and_restore_authored_flags() {
    use std::cell::Cell;
    use crate::ui::Curve;
    use crate::ui::{Keyframe,Keyframes};
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.verify_views(false); // the test counts kept views' rebuilds or allocations; verification rebuilds them
    let scoped=crate::ThemeHandle::default();
    let handle=Cell::new(None);
    let child=Cell::new(None);
    let builds=Cell::new(0);
    let seen=Cell::new(crate::Appearance::Unknown);
    let mut build=|ui:&mut Ui| {
        handle.set(Some({ let enclosing = ui.node("system", Node::column().w_px(100.0).h_px(60.0)); let mut view = enclosing; ui.within(&enclosing, &mut |ui| view = ui.view_of("system", &1, |ui| {
            builds.set(builds.get()+1);
            seen.set(ui.system_appearance());
            child.set(Some(ui.node("child",crate::Node::DEFAULT.w_px(20.0).h_px(10.0).at(10.0,10.0))));
        })); view }));
        let reduced=ui.tokens().reduced_motion;
        ui.themed(&scoped,&mut |ui|assert_eq!(ui.tokens().reduced_motion,reduced,"a scoped theme cannot bypass OS motion policy"));
    };
    let ease=Curve::Bezier { duration:10.0,points:[0.0,0.0,1.0,1.0] };
    for cycle in 0..5 {
        let time=cycle as f64*20.0;
        ui.frame(&input().time(time).appearance(crate::Appearance::Light),&mut build).unwrap();
        ui.frame(&input().time(time+0.01).appearance(crate::Appearance::Light),&mut build).unwrap();
        let before=builds.get();
        let view=handle.get().unwrap();
        ui.animate_keys(&view,&[Keyframe::scale(5.0, 0.9, 0.9).curve(ease),
            Keyframe::scale(10.0, 0.8, 0.8).curve(ease)],&Keyframes::DEFAULT);
        ui.frame(&input().time(time+0.1).appearance(crate::Appearance::Light),&mut build).unwrap();
        mem_sys::local_reset();
        ui.frame(&input().time(time+0.2).reduced_motion(true).appearance(crate::Appearance::Dark),&mut build).unwrap();
        assert!(builds.get()>before);
        assert_eq!(seen.get(),crate::Appearance::Dark);
        assert!(ui.tokens().reduced_motion);
        assert!(ui.repaint_at().is_none(),"turning on reduced motion settles existing keyframes");
        let rect=ui.rect(&child.get().unwrap());
        assert!((rect.w-16.0).abs()<0.001 && (rect.h-8.0).abs()<0.001,"motion reaches the final keyframe: {rect:?}");
        if cycle>=2 { assert_eq!(mem_sys::local_alloc_count(),0,"warm preference changes retain caches and animation storage"); }
        ui.frame(&input().time(time+0.3).reduced_motion(true).appearance(crate::Appearance::Dark),&mut build).unwrap();
        let before=builds.get();
        ui.frame(&input().time(time+0.4).reduced_motion(true).appearance(crate::Appearance::Dark),&mut build).unwrap();
        assert_eq!(builds.get(),before,"unchanged OS inputs do not invalidate views");
        ui.frame(&input().time(time+0.5).appearance(crate::Appearance::Light),&mut build).unwrap();
        assert!(!ui.tokens().reduced_motion,"the app's original motion flag is restored");
    }
    let mut theme = crate::ThemeHandle::default();
    theme.reduced_motion(true);
    ui.set_theme(&theme).unwrap();
    ui.frame(&input().time(120.0),&mut |_|{}).unwrap();
    assert!(ui.tokens().reduced_motion,"an authored motion restriction is preserved");
    for (appearance,reduced_motion) in [(3,0),(u32::MAX,0),(0,2),(0,u32::MAX)] {
        assert!(ui.frame(&{
            let mut frame = input();
            frame.wire.appearance = appearance;
            frame.wire.reduced_motion = reduced_motion;
            frame
        },&mut |_|panic!("invalid inputs must not build")).is_err());
    }
}

#[test]
fn public_view_controls_retain_instances_geometry_and_hit_clips_without_warm_allocations() {
    use std::cell::Cell;
    use crate::ViewTransform;
    mem_sys::set_peer(crate::library_counter);
    let view=Cell::new(None);
    let child=Cell::new(None);
    let builds=Cell::new(0);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.verify_views(false); // the test counts kept views' rebuilds or allocations; verification rebuilds them
    let mut build=|ui:&mut Ui| {
        let enclosing = ui.node("controlled", Node::column().w_px(100.0).h_px(80.0));
        ui.set_at(&enclosing,(40.0,30.0));
        let v={ let mut view = enclosing; ui.within(&enclosing, &mut |ui| view = ui.view_of("controlled", &1, |ui| {
            builds.set(builds.get()+1);
            child.set(Some(ui.node("child",crate::Node::DEFAULT.flags(pxui_types::Flags::CLICKABLE).w_px(20.0).h_px(10.0).at(10.0,10.0).look(crate::ui::Look::NONE.fill(pxui_types::geom::Rgba::rgb(255,0,0))))));
        })); view };
        view.set(Some(v));
    };
    for time in [0.0,0.1] { ui.frame(&input().time(time),&mut build).unwrap(); }
    let (v,c,built)=(view.get().unwrap(),child.get().unwrap(),builds.get());
    let original=ui.layer(0).expect("base layer").instances.to_vec();
    let near=|a:Rect,b:Rect|[a.x-b.x,a.y-b.y,a.w-b.w,a.h-b.h].iter().all(|v|v.abs()<0.001);
    ui.set_view(&v,&ViewTransform::DEFAULT.about(1.0,[0.0,-1.0,1.0,0.0],(90.0,70.0)));
    ui.set_view(&v,&ViewTransform::DEFAULT.scale(1.0));
    assert!(near(ui.rect(&c),Rect::new(50.0,40.0,20.0,10.0)),"centre scale replaces an earlier manual rotation and fixed pivot");
    for cycle in 0..4 {
        mem_sys::local_reset();
        for (scale,expected) in [(2.0,Rect::new(10.0,10.0,40.0,20.0)),(1.0,Rect::new(50.0,40.0,20.0,10.0)),
            (0.0,Rect::new(90.0,70.0,0.0,0.0)),(-1.0,Rect::new(110.0,90.0,20.0,10.0))] {
            ui.set_view(&v,&ViewTransform::DEFAULT.opacity(0.5));
            ui.set_view(&v,&ViewTransform::DEFAULT.scale(scale));
            ui.set_view(&v,&crate::ViewTransform::DEFAULT.clip(Some(Rect::new(0.0,0.0,320.0,240.0))));
            ui.set_view(&v,&crate::ViewTransform::DEFAULT.clip(None));
            assert!(near(ui.rect(&c),expected),"scale {scale}: {:?}",ui.rect(&c));
            assert_eq!(ui.layer(0).expect("base layer").instances,original.as_slice(),"view table edits never rewrite instances");
        }
        ui.frame(&input().time(0.2+cycle as f64*0.1),&mut build).unwrap();
        assert_eq!(builds.get(),built);
        if cycle>0 { assert_eq!(mem_sys::local_alloc_count(),0); }
    }
    ui.set_view(&v,&ViewTransform::DEFAULT.scale(2.0));
    ui.set_view(&v,&crate::ViewTransform::DEFAULT.clip(Some(Rect::new(20.0,15.0,10.0,10.0))));
    for (time,x,hovered) in [(1.0,25.0,true),(1.1,15.0,false)] {
        let event=pxui_types::Event::pointer(pxui_types::Event::POINTER_MOVED, 0, x, 20.0);
        ui.frame(&{
            let mut frame = input().time(time);
            frame.events = core::slice::from_ref(&event);
            frame
        },&mut build).unwrap();
        assert_eq!(ui.signal(&c).hovered,hovered,"a view's window clip also clips hit testing");
    }
    ui.set_view(&v,&crate::ViewTransform::DEFAULT.clip(None));
    let event=pxui_types::Event::pointer(pxui_types::Event::POINTER_MOVED, 0, 15.0, 20.0);
    ui.frame(&{
        let mut frame = input().time(1.2);
        frame.events = core::slice::from_ref(&event);
        frame
    },&mut build).unwrap();
    assert!(ui.signal(&c).hovered);
    for opacity in [-1.0,1.01,f32::NAN,f32::INFINITY] { assert!(catch_unwind(AssertUnwindSafe(||ui.set_view(&v,&ViewTransform::DEFAULT.opacity(opacity)))).is_err()); }
    for scale in [f32::NAN,f32::INFINITY,f32::NEG_INFINITY] { assert!(catch_unwind(AssertUnwindSafe(||ui.set_view(&v,&ViewTransform::DEFAULT.scale(scale)))).is_err()); }
    for clip in [Rect::new(0.0,0.0,-1.0,2.0),Rect::new(f32::NAN,0.0,1.0,1.0),Rect::new(f32::MAX,0.0,f32::MAX,1.0)] {
        assert!(catch_unwind(AssertUnwindSafe(||ui.set_view(&v,&crate::ViewTransform::DEFAULT.clip(Some(clip))))).is_err());
    }
}

#[test]
fn public_measured_list_settles_virtual_rows_and_reuses_warm_storage() {
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let rows:[Cell<Option<crate::Handle>>;200]=std::array::from_fn(|_|Cell::new(None));
    let calls=Cell::new(0);
    let list=Cell::new(None);
    let count=Cell::new(200);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut build=|ui:&mut Ui| {
        let col=ui.node("column",crate::Node::DEFAULT.w_px(100.0).h_px(200.0));
        ui.within(&col,&mut |ui| {
            list.set(Some(ui.section_list("measured",&crate::ui::SectionList::new(count.get()).row_h(12.0).measured(),&|i| i as i64,&mut |ui,part,_| {
                let crate::ui::SectionPart::Row(i)=part else { return };
                assert!(i<count.get());
                calls.set(calls.get()+1);
                rows[i].set(Some(ui.node("row",crate::Node::DEFAULT.w_px(80.0).h_px(10.0+(i%3) as f32*10.0))));
            }).handle));
        });
    };
    ui.frame(&input(),&mut build).unwrap();
    for (i,top) in [0.0,10.0,30.0,60.0,70.0,90.0].into_iter().enumerate() {
        assert_eq!(ui.rect(&rows[i].get().unwrap()).y,top,"measured rows settle without a visible gap");
    }
    assert!(calls.get()<100,"the list builds visible/overscan rows, not all 200");
    for frame in 1..8 {
        calls.set(0);
        mem_sys::local_reset();
        ui.frame(&input().time(frame as f64/60.0),&mut build).unwrap();
        assert!(calls.get()>0 && calls.get()<40);
        if frame>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"warm measured rows keep their cache and callback storage"); }
    }
    ui.set_scroll_offset(&list.get().unwrap(),120.0);
    ui.frame(&input().time(1.0),&mut build).unwrap();
    assert!(ui.scroll(&list.get().unwrap()).settled>=120.0);
    for frame in 1..5 {
        mem_sys::local_reset();
        ui.frame(&input().time(1.0+frame as f64/60.0),&mut build).unwrap();
        if frame>=2 { assert_eq!(mem_sys::local_alloc_count(),0); }
    }
    count.set(0);
    calls.set(0);
    ui.frame(&input().time(2.0),&mut build).unwrap();
    assert_eq!(calls.get(),0);
    assert_eq!(ui.scroll(&list.get().unwrap()).settled,0.0);
}

#[test]
fn measured_list_validation_and_callback_panics_leave_frames_reusable() {
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    for (count,estimate) in [(1,0.0),(1,-1.0),(1,f32::NAN),(1,f32::INFINITY),(1_000_001,12.0),(10,f32::MAX)] {
        assert!(catch_unwind(AssertUnwindSafe(|| {
            ui.frame(&input(),&mut |ui| { ui.section_list("bad",&crate::ui::SectionList::new(count).row_h(estimate).measured(),&|i| i as i64,&mut |_,_,_|panic!("invalid arguments reached the row")); }).unwrap();
        })).is_err());
    }
    assert!(catch_unwind(AssertUnwindSafe(|| {
        ui.frame(&input(),&mut |ui| { ui.section_list("panic",&crate::ui::SectionList::new(1).row_h(20.0).measured(),&|i| i as i64,&mut |_,_,_|panic!("row callback")); }).unwrap();
    })).is_err());
    ui.frame(&input(),&mut |ui| { ui.section_list("recovered",&crate::ui::SectionList::new(1).row_h(20.0).measured(),&|i| i as i64,&mut |ui,_,_| { ui.label("Recovered", crate::Label::DEFAULT); }); }).unwrap();
}

#[test]
fn short_startup_input_keeps_capacity_for_the_first_background_resume_batch() {
    use pxui_types::Event;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut build = |ui: &mut Ui| { ui.label("Lifecycle", crate::Label::DEFAULT); };
    let first = Event::new(Event::FOCUS_GAINED);
    ui.frame(&{
        let mut frame = input();
        frame.events = core::slice::from_ref(&first);
        frame
    }, &mut build).unwrap();
    for frame in 0..82 {
        ui.frame(&input().time(frame as f64 / 60.0), &mut build).unwrap();
    }
    let events = [
        { let mut record = Event::default(); record.kind = Event::VISIBILITY_CHANGED; record.value = 0.0; record },
        Event::new(Event::FOCUS_LOST),
        Event::new(Event::FOCUS_GAINED),
        { let mut record = Event::default(); record.kind = Event::VISIBILITY_CHANGED; record.value = 1.0; record },
        Event::new(Event::ENVIRONMENT_CHANGED),
    ];
    mem_sys::local_reset();
    ui.frame(&{
        let mut frame = input().time(30.0);
        frame.events = &events[..];
        frame
    }, &mut build).unwrap();
    assert_eq!(mem_sys::local_alloc_count(), 0, "first resume must retain the normal input capacity");
    assert!(!ui.text_input_focused());
}

#[test]
fn focused_text_field_keeps_selection_and_ime_without_warm_allocations() {
    use pxui_types::{Event, Str};
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut build = |ui: &mut Ui| {
        assert!(ui.text_field("Caption", "café track 10", &crate::TextField::DEFAULT).changed.is_none());
    };
    ui.frame(&input(), &mut build).unwrap();
    let key = |kind, code: crate::KeyCode| Event::key(kind, code.wire());
    let focus = [key(Event::KEY_PRESSED, crate::KeyCode::Tab)];
    ui.frame(&{
        let mut frame = input().time(0.1);
        frame.events = &focus[..];
        frame
    }, &mut build).unwrap();
    assert!(ui.text_input_focused());
    let composition = "é";
    let select = [key(Event::KEY_PRESSED, crate::KeyCode::Shift), key(Event::KEY_PRESSED, crate::KeyCode::ArrowLeft), key(Event::KEY_RELEASED, crate::KeyCode::Shift),
        { let mut record = Event::default(); record.kind = Event::TEXT_EDITING; record.text = { let mut record = Str::default(); record.ptr = composition.as_ptr(); record.len = composition.len(); record }; record.y = 1.0; record }];
    ui.frame(&{
        let mut frame = input().time(0.2);
        frame.events = &select[..];
        frame
    }, &mut build).unwrap();
    for tick in 0..80 {
        mem_sys::local_reset();
        ui.frame(&{
            let mut frame = input().time(0.3 + tick as f64 / 60.0);
            frame.wire.width = 320.0 + (tick % 2) as f32;
            frame
        }, &mut build).unwrap();
        if tick >= 10 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm focused text field across the ABI"); }
        assert!(ui.text_input_focused());
        let crate::Ime { rect: crate::Rect { w, h, .. }, caret } = ui.ime().expect("focused field must publish IME geometry");
        assert!(w > 0.0 && h > 0.0 && caret >= 0.0 && caret <= w);
    }
    let commit = { let mut record = Event::default(); record.kind = Event::TEXT_INPUT; record.text = { let mut record = Str::default(); record.ptr = composition.as_ptr(); record.len = composition.len(); record }; record };
    let mut changed = false;
    ui.frame(&{
        let mut frame = input().time(2.0);
        frame.events = core::slice::from_ref(&commit);
        frame
    }, &mut |ui| {
        changed = ui.text_field("Caption", "café track 10", &crate::TextField::DEFAULT).changed == Some("café track 1é");
    }).unwrap();
    assert!(changed, "composition commit must replace the retained selection");
}

#[test]
fn custom_text_field_font_sizes_keep_glyphs_caret_hit_selection_and_ime_without_warm_allocations() {
    use crate::ui::{Node, Edges, Role};
    use crate::theme::{Font, Look, State};
    use crate::{Event, Part, Str};
    use std::cell::Cell;
    thread_local! {
        static SIZE: Cell<i64> = const { Cell::new(20) };
        static ROW: Cell<i64> = const { Cell::new(0) };
    }
    fn body(ui: &mut Ui, c: &crate::bodies::TextFieldConfig) { use crate::bodies::BodyConfig;
        ROW.set(c.row.box_key);
        let mut row = ui.node_layout(&c.row);
        row.h = Size::Px(59.0);
        ui.set_node_layout(&c.row, &row);
        ui.style(&c.row, &crate::theme::States::of(Look::NONE), State::default());
        let editor = ui.part(Part::EDITOR, Node::DEFAULT.at(18.0, 0.0).w_px(280.0).h_px(59.0).inset(Edges::ZERO.l(2.0))
            .look(Look::NONE.text(ui.theme().tokens.foreground).font(Font::BODY.size(SIZE.get() as f32))));
        if c.state.focused == 0 { ui.text(&editor, c.text_of(&c.value)); }
    }
    mem_sys::set_peer(crate::library_counter);
    for size in [20,-3,0] {
        for scale in [1.0,2.0] {
            SIZE.set(size);
            let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
            ui.set_a11y(true);
            ui.register_text_field_body("font-sized-editor",body).unwrap();
            let source = "text_field.body = font-sized-editor";
            ui.set_theme(&crate::ThemeHandle::parse(&source).unwrap()).unwrap();
            let value = "café track 10";
            let mut build = |ui: &mut Ui| assert!(ui.text_field("Styled",value,&crate::TextField::DEFAULT).changed.is_none());
            let frame = input().scale(scale);
            for tick in 0..3 { ui.frame(&frame.time(tick as f64/60.0),&mut build).unwrap(); }
            let glyphs = ui.layers().flat_map(|layer|layer.instances.iter()).filter(|quad|quad.kind==crate::draw::kind::MASK).map(|quad|(quad.rect,quad.aux)).collect::<Vec<_>>();
            assert!(!glyphs.is_empty());
            let points = if size==0 {ui.font_size()} else {Font::BODY.size(size as f32).points(ui.font_size())};
            let full_width = ui.text_service().width(value,crate::Label::BODY.size(points as f32),None).unwrap();
            let focus = Event::key(Event::KEY_PRESSED, crate::KeyCode::Tab.wire());
            ui.frame(&{
                let mut frame = frame.time(0.1);
                frame.events = core::slice::from_ref(&focus);
                frame
            },&mut build).unwrap();
            assert_eq!(ui.ime().unwrap().caret,(2.0+full_width).round(),"focused text uses the body's editor font size {size}");
            assert!(ui.layers().flat_map(|layer|layer.instances.iter()).filter(|quad|quad.kind==crate::draw::kind::MASK).map(|quad|(quad.rect,quad.aux)).eq(glyphs.iter().copied()),"focus preserves the exact glyph positions and atlas samples");
            let editor = ui.part_rect(&ui.box_of_key(crate::Key::Resolved(ROW.get())).unwrap(),Part::EDITOR).unwrap();
            let prefix = ui.text_service().width("café ",crate::Label::BODY.size(points as f32),None).unwrap();
            let x = editor.x+2.0+prefix;
            let y = editor.y+editor.h/2.0;
            if size==20 { SIZE.set(24); }
            for kind in [Event::POINTER_PRESSED,Event::POINTER_RELEASED] {
                let click = Event::pointer(kind, 0, x as f64, y as f64);
                ui.frame(&{
                    let mut frame = frame.time(0.2);
                    frame.events = core::slice::from_ref(&click);
                    frame
                },&mut build).unwrap();
            }
            let selected = ui.a11y_nodes().iter().find(|node|node.role==Role::TextField as u32).unwrap();
            assert_eq!(selected.selection,[5,5],"pointer hit testing uses the displayed UTF-8 text at size {size}");
            if size==20 {
                let width = ui.text_service().width("café ",crate::Label::BODY.size(24 as f32),None).unwrap();
                assert_eq!(ui.ime().unwrap().caret,(2.0+width).round(),"a body size change paints the new font immediately, after hit testing the previous frame");
            }
            let key = |kind,code: crate::KeyCode| Event::key(kind, code.wire());
            let composition = "é";
            let select = [key(Event::KEY_PRESSED,crate::KeyCode::Shift),key(Event::KEY_PRESSED,crate::KeyCode::ArrowLeft),key(Event::KEY_RELEASED,crate::KeyCode::Shift),{ let mut record = Event::default(); record.kind = Event::TEXT_EDITING; record.text = { let mut record = Str::default(); record.ptr = composition.as_ptr(); record.len = composition.len(); record }; record.y = 1.0; record }];
            ui.frame(&{
                let mut frame = frame.time(0.3);
                frame.events = &select[..];
                frame
            },&mut build).unwrap();
            let selected = ui.a11y_nodes().iter().find(|node|node.role==Role::TextField as u32).unwrap();
            assert_eq!(selected.selection,[5,4]);
            let keys = [Event::key(Event::KEY_PRESSED, crate::KeyCode::Alt.wire()),Event::key(Event::KEY_PRESSED, crate::KeyCode::F2.wire()),Event::key(Event::KEY_RELEASED, crate::KeyCode::F2.wire()),Event::key(Event::KEY_RELEASED, crate::KeyCode::Alt.wire())];
            for tick in 0..24 {
                mem_sys::local_reset();
                ui.frame(&{
                    let mut frame = frame.time(0.4+tick as f64/60.0);
                    frame.events = &keys[..];
                    frame
                },&mut build).unwrap();
                if tick>=8 {assert_eq!(mem_sys::local_alloc_count(),0,"warm custom body, focus, selection and composition");}
                assert!(ui.ime().is_some());
            }
            assert_eq!(ui.signal_keys(&ui.box_of_key(crate::Key::Resolved(ROW.get())).unwrap()).len(),keys.len(),"editing must restore the original key stream for later consumers");
            let commit = { let mut record = Event::default(); record.kind = Event::TEXT_INPUT; record.text = { let mut record = Str::default(); record.ptr = composition.as_ptr(); record.len = composition.len(); record }; record };
            let mut changed = false;
            ui.frame(&{
                let mut frame = frame.time(1.0);
                frame.events = core::slice::from_ref(&commit);
                frame
            },&mut |ui| {changed=ui.text_field("Styled",value,&crate::TextField::DEFAULT).changed==Some("caféétrack 10");}).unwrap();
            assert!(changed,"composition commit replaces exactly the selected UTF-8 range");
        }
    }
}

#[test]
fn numeric_signal_preserves_legacy_keys_without_warm_keyboard_allocations() {
    use std::cell::Cell;
    use pxui_types::{Event};
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let button = Cell::new(None);
    let clicks = Cell::new(0);
    let mut build = |ui: &mut Ui| {
        let node = ui.node("button", crate::Node::DEFAULT.flags(pxui_types::Flags::CLICKABLE | pxui_types::Flags::TAB_STOP).w_px(120.0).h_px(32.0));
        let state = ui.signal(&node);
        clicks.set(clicks.get() + i32::from(state.clicked));
        // the measured frame below reads the borrowed key slices; warm frames take the same path
        let _ = ui.signal_keys(&node).len() + ui.press_keys(&node).len();
        button.set(Some(node));
    };
    ui.frame(&input(), &mut build).unwrap();
    ui.focus_box(&button.get().unwrap());
    let key = |kind, code: crate::KeyCode| Event::key(kind, code.wire());
    let events = [key(Event::KEY_PRESSED, crate::KeyCode::Shift), key(Event::KEY_PRESSED, crate::KeyCode::Space), key(Event::KEY_RELEASED, crate::KeyCode::Space), key(Event::KEY_RELEASED, crate::KeyCode::Shift)];
    for tick in 0..60 {
        mem_sys::local_reset();
        ui.frame(&{
            let mut frame = input().time(1.0 + tick as f64 / 60.0);
            frame.events = &events[..];
            frame
        }, &mut build).unwrap();
        if tick >= 5 { assert_eq!(mem_sys::local_alloc_count(), 0, "numeric keyboard routing and scalar signal across the ABI"); }
    }
    // a warm frame whose box has key events: reading them through the borrowed slice allocates nothing
    let mut seen = 0;
    let scope = mem_sys::warm_scope();
    ui.frame(&{
        let mut frame = input().time(9.0);
        frame.events = &events[..];
        frame
    }, &mut |ui: &mut Ui| {
        let node = ui.node("button", crate::Node::DEFAULT.flags(pxui_types::Flags::CLICKABLE | pxui_types::Flags::TAB_STOP).w_px(120.0).h_px(32.0));
        seen = ui.signal_keys(&node).len() + ui.press_keys(&node).len() + usize::from(ui.signal(&node).clicked);
    }).unwrap();
    drop(scope);
    assert!(seen >= 4);
    assert!(mem_sys::take_reports().is_empty(), "warm frame with key events must not allocate");
    assert_eq!(clicks.get(), 60);
    let node = button.get().unwrap();
    assert!(ui.signal(&node).clicked);
    let owned = ui.signal_keys(&node);
    assert_eq!(owned.len(), 4);
    for (actual, expected) in owned.iter().zip(events) {
        assert_eq!((actual.kind, actual.code), (expected.kind, expected.code));
    }
    let text = "café";
    let events = [{ let mut record = Event::default(); record.kind = Event::TEXT_EDITING; record.x = 1.0; record.y = 2.0; record.text = { let mut record = pxui_types::Str::default(); record.ptr = text.as_ptr(); record.len = text.len(); record }; record },
        { let mut record = Event::default(); record.kind = Event::TEXT_INPUT; record.text = { let mut record = pxui_types::Str::default(); record.ptr = text.as_ptr(); record.len = text.len(); record }; record }];
    ui.frame(&{
        let mut frame = input().time(3.0);
        frame.events = &events[..];
        frame
    }, &mut build).unwrap();
    let owned = ui.signal_keys(&node);
    assert_eq!(owned.len(), 2);
    let str_of = |e: &Event| e.text.len;
    assert_eq!((owned[0].kind, owned[0].x, owned[0].y, str_of(&owned[0])), (Event::TEXT_EDITING, 1.0, 2.0, text.len()));
    assert_eq!((owned[1].kind, str_of(&owned[1])), (Event::TEXT_INPUT, text.len()));
    ui.frame(&input().time(3.1), &mut |_| {}).unwrap();
    assert!(ui.signal_keys(&node).is_empty() && !ui.signal(&node).clicked);
}

#[test]
fn borrowed_carry_queries_and_whole_warm_pointer_gestures_allocate_nothing() {
    use crate::{DropRef, PayloadRef};
    use pxui_types::Event;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut hover = false;
    let mut dropped = false;
    let mut deliveries = 0;
    let payload = PayloadRef { kind: "queue-occurrence", value: "18446744073709551615" };
    let mut build = |ui: &mut Ui| {
        let source = ui.node("carry-source", Node::DEFAULT.flags(pxui_types::Flags::CLICKABLE).w(Size::Px(100.0)).h(Size::Px(40.0)).at(0.0, 0.0));
        let target = ui.node("carry-target", Node::DEFAULT.flags(pxui_types::Flags::CLICKABLE).w(Size::Px(100.0)).h(Size::Px(40.0)).at(0.0, 100.0));
        if ui.signal(&source).held { ui.carry(Some(&source), payload.kind, payload.value); }
        if let Some(p) = ui.carrying() { assert_eq!(p, payload); }
        hover = false; dropped = false;
        if let Some(state) = ui.drop_target(&target) {
            match state {
                DropRef::Hover(p) => { assert_eq!(p, payload); hover = true; }
                DropRef::Dropped(p) => { assert_eq!(p, payload); dropped = true; deliveries += 1; }
            }
        }
        assert!(!ui.signal(&target).clicked);
    };
    let event = |kind, y| Event::pointer(kind, 0, 10.0, y);
    let mut tick = 0u32;
    for round in 0..24 {
        mem_sys::local_reset();
        for e in [None, Some(event(Event::POINTER_PRESSED, 10.0)), Some(event(Event::POINTER_MOVED, 120.0)), Some(event(Event::POINTER_RELEASED, 120.0)), None] {
            tick += 1;
            ui.frame(&{
                let mut frame = input().time(tick as f64 / 60.0);
                frame.events = e.as_slice();
                frame
            }, &mut build).unwrap();
        }
        if round > 3 { assert_eq!(mem_sys::local_alloc_count(), 0, "whole warm gesture including payload start, hover, release and retirement"); }
    }
    drop(build);
    assert!(!hover && !dropped, "a drop lasts one frame");
    assert_eq!(deliveries, 24);
    // Key carries reuse the same storage, including replacement and cancellation.
    for round in 0..12 {
        ui.frame(&input().time(10.0 + round as f64), &mut |_| {}).unwrap();
        mem_sys::local_reset();
        ui.carry(None, payload.kind, payload.value);
        assert_eq!(ui.carrying(), Some(payload));
        ui.carry(None, "kind", "café");
        assert_eq!(ui.carrying(), Some(PayloadRef { kind: "kind", value: "café" }));
        ui.cancel_carry();
        assert_eq!(ui.carrying(), None);
        if round > 1 { assert_eq!(mem_sys::local_alloc_count(), 0); }
    }
}

#[test]
fn transferred_pointer_carries_survive_virtualization_and_keep_warm_modified_gestures_allocation_free() {
    use crate::{DropRef, Node};
    use pxui_types::{Event};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let list = Cell::new(None);
    let source = Cell::new(None);
    let dropped = Cell::new(None);
    let visible = Cell::new(true);
    let mut build = |ui: &mut Ui| {
        if !visible.get() { return; }
        let body = ui.grid_list("virtual-carry", 10_000, &crate::GridList::DEFAULT.w_px(320.0).h_px(20.0), &mut |ui, index| {
            let row = ui.node("row", Node::DEFAULT.flags(pxui_types::Flags::CLICKABLE).w(Size::Grow).h(Size::Grow));
            if index == 0 {
                source.set(Some(row));
                if ui.signal(&row).held { ui.carry(Some(&row), "occurrence", "duplicate song"); }
            }
            if let Some(DropRef::Dropped(payload)) = ui.drop_target(&row) {
                assert_eq!((payload.kind, payload.value), ("occurrence", "duplicate song"));
                dropped.set(Some(index));
            }
        });
        list.set(Some(body));
        let unrelated = ui.node("unrelated", Node::DEFAULT.flags(pxui_types::Flags::CLICKABLE).at(400.0, 0.0).w_px(10.0).h_px(10.0));
        assert!(!ui.transfer_carry(&unrelated));
        ui.transfer_carry(&body);
        assert!(!ui.signal(&body).clicked, "finishing a carry must not click its capture owner");
    };
    ui.frame(&input(), &mut build).unwrap();
    let event = |kind, y| Event::pointer(kind, 0, 10.0, y);
    let shift = |kind| Event::key(kind, crate::KeyCode::Shift.wire());
    let mut time = 0.0;
    for round in 0..20 {
        dropped.set(None);
        mem_sys::local_reset();
        for events in [
            [shift(Event::KEY_PRESSED), event(Event::POINTER_PRESSED, 10.0)],
            [event(Event::POINTER_MOVED, 80.0), Event::default()],
            [Event::pointer(Event::SCROLLED, 0, 0.0, -30.0), Event::default()],
            [Event::default(), Event::default()],
            [event(Event::POINTER_RELEASED, 80.0), shift(Event::KEY_RELEASED)],
            [Event::pointer(Event::SCROLLED, 0, 0.0, 30.0), Event::default()],
            [Event::default(), Event::default()],
        ] {
            time += 0.02;
            // Zero-kind events are not wire events; omit the unused slots.
            let count = if events[0].kind == 0 { 0 } else { 1 + usize::from(events[1].kind != 0) };
            ui.frame(&{
                let mut frame = input().time(time);
                frame.events = &events[..count];
                frame
            }, &mut build).unwrap();
            if round == 0 && events[0].kind == Event::POINTER_RELEASED {
                assert_eq!(ui.press_keys(&list.get().unwrap()), &[crate::KeyCode::Shift.wire()], "capture transfer preserves the press modifiers through release");
            }
            if events[0].kind == Event::SCROLLED && events[0].y < 0.0 {
                assert!(!ui.transfer_carry(&source.get().unwrap()), "a virtualized child is stale");
                assert!(ui.carrying().is_some());
            }
        }
        assert_eq!(dropped.get(), Some(34));
        assert!(ui.carrying().is_none());
        if round > 4 { assert_eq!(mem_sys::local_alloc_count(), 0, "modified cross-page gesture across both allocator copies"); }
    }
    for cancel in [Some(event(Event::POINTER_CANCELLED, 0.0)), Some(event(Event::FOCUS_LOST, 0.0)), None] {
        for e in [event(Event::POINTER_PRESSED, 10.0), event(Event::POINTER_MOVED, 80.0)] {
            time += 0.02;
            ui.frame(&{
                let mut frame = input().time(time);
                frame.events = core::slice::from_ref(&e);
                frame
            }, &mut build).unwrap();
        }
        dropped.set(None);
        if let Some(e) = cancel {
            time += 0.02;
            ui.frame(&{
                let mut frame = input().time(time);
                frame.events = core::slice::from_ref(&e);
                frame
            }, &mut build).unwrap();
        } else { ui.cancel_carry(); }
        let e = event(Event::POINTER_RELEASED, 80.0);
        time += 0.02;
        ui.frame(&{
            let mut frame = input().time(time);
            frame.events = core::slice::from_ref(&e);
            frame
        }, &mut build).unwrap();
        assert!(dropped.get().is_none() && ui.carrying().is_none());
    }
    for e in [event(Event::POINTER_PRESSED, 10.0), event(Event::POINTER_MOVED, 80.0)] {
        time += 0.02;
        ui.frame(&{
            let mut frame = input().time(time);
            frame.events = core::slice::from_ref(&e);
            frame
        }, &mut build).unwrap();
    }
    visible.set(false);
    time += 0.02;
    ui.frame(&input().time(time), &mut build).unwrap();
    assert!(ui.carrying().is_none() && !ui.captured());
    assert_eq!(ui.built_nodes(), 1, "removing the container must remove its floating label in the same frame");
    assert!(ui.repaint_at().is_none());
    let body = list.get().unwrap();
    ui.carry(None, "key", "value");
    assert!(!ui.transfer_carry(&body), "key carries keep press-to-drop semantics");
    ui.cancel_carry();
    assert!(!ui.transfer_carry(&source.get().unwrap()), "no carry cannot acquire capture");
}

#[test]
fn fractional_fills_preserve_subpixel_quads_refuse_invalid_bounds_and_allocate_nothing_when_warm() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    for frame in 0..20 {
        mem_sys::local_reset();
        ui.frame(&input().time(frame as f64),&mut |ui| {
            let node=ui.node("fractional",crate::Node::DEFAULT.w_px(320.0).h_px(40.0));
            ui.canvas(&node,|paint,rect| {
                let white=pxui_types::geom::Rgba::WHITE;
                for i in 0..200 { paint.fill(Rect::new(rect.x+i as f32*1.5+0.25, rect.y+1.25, 0.5, 4.0), Fill::new(white).fractional()); }
                for (x,y,w,h) in [(f32::NAN,0.0,1.0,1.0),(0.0,f32::INFINITY,1.0,1.0),(f32::MAX,0.0,f32::MAX,1.0),(f32::MAX,0.0,1.0,1.0),(0.0,0.0,0.0,1.0),(0.0,0.0,1.0,-1.0)] {
                    paint.fill(Rect::new(x, y, w, h), Fill::new(white).fractional());
                }
            });
        }).unwrap();
        if frame>3 { assert_eq!(mem_sys::local_alloc_count(),0); }
        let mut quads=ui.layers().flat_map(|layer| layer.instances.iter());
        for i in 0..200 {
            let q=quads.next().unwrap();
            assert_eq!(q.rect,[i as f32*1.5+0.25,1.25,i as f32*1.5+0.75,5.25]);
            assert_eq!(q.flags,pxui_types::draw::flags::rect::AREA);
        }
        assert!(quads.next().is_none());
    }
}

#[test]
fn output_partitions_reuse_the_engine_dirty_capacity_after_unchanged_frames() {
    use crate::ui::{Node,Look};
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let names:Vec<&'static str>=(0..32).map(|index|&*format!("part-{index}").leak()).collect();
    for frame in 0..8 {
        ui.frame(&input().time(frame as f64),&mut |ui| {
            for (index,name) in names.iter().enumerate() {
                let phase=if frame<3 {frame.min(1)} else {frame-1};
                let changed=index%2==0 && phase%2==1;
                let revision=if index%2==0 {phase as u64} else {0};
                ui.id(*name, |ui| { ui.column(Node::column().w_px(20.0).h_px(4.0), |ui| { ui.view_of(*name, &revision, |ui| {
                    ui.node("quad",Node::DEFAULT.w_px(20.0).h_px(4.0).look(Look::NONE.fill(if changed {pxui_types::geom::Rgba::rgb(0, 0, 255)} else {pxui_types::geom::Rgba::WHITE})));
                }); }); });
            }
        }).unwrap();
        if frame<2 { ui.drawn(0); continue; }
        mem_sys::local_reset();
        let layer=ui.layer(0).expect("base layer");
        if frame>2 {
            assert_eq!(layer.dirty.len(),16);
            assert_eq!(mem_sys::local_alloc_count(),0,"output partition metadata must reuse capacity when no draw storage grows");
        }
        ui.drawn(0);
    }
}

#[test]
fn simultaneous_view_entrance_channels_move_cached_children_and_settle_without_allocating() {
    use crate::ui::{Keyframe, Keyframes};
    use crate::ui::{Curve, Node, Role};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.verify_views(false); // the test counts kept views' rebuilds or allocations; verification rebuilds them
    ui.set_a11y(true);
    let (view, child, builds) = (Cell::new(None), Cell::new(None), Cell::new(0));
    let mut build = |ui: &mut Ui| {
        let v={ let enclosing = ui.node("entrance", Node::column().w_px(100.0).h_px(80.0)); let mut view = enclosing; ui.within(&enclosing, &mut |ui| view = ui.view_of("entrance", &1, |ui| {
            builds.set(builds.get()+1);
            let b=ui.node("child",Node::DEFAULT.w_px(20.0).h_px(10.0).role(Role::Button).look(crate::ui::Look::NONE.fill(pxui_types::geom::Rgba::rgb(20,30,40))));
            ui.a11y_name(&b,"Moving child"); child.set(Some(b));
        })); view };
        ui.set_at(&v,(20.0,20.0)); view.set(Some(v));
    };
    ui.frame(&input(),&mut build).unwrap();
    let instances=ui.layer(0).expect("base layer").instances.to_vec();
    let curve=Curve::Bezier {duration:1.0,points:[0.0,0.0,1.0,1.0]};
    let keys=[
        Keyframe::opacity(0.0, 0.0).curve(curve),
        Keyframe::offset(0.0, 0.0,10.0).curve(curve),
        Keyframe::opacity(0.1, 0.0).curve(curve),
        Keyframe::opacity(0.34, 1.0).curve(curve),
        Keyframe::offset(0.34, 0.0,0.0).curve(curve),
    ];
    for round in 0..4 {
        let base=round as f64;
        ui.frame(&input().time(base),&mut build).unwrap();
        mem_sys::local_reset();
        assert!(ui.animate_keys(&view.get().unwrap(),&keys,&Keyframes::DEFAULT));
        for (dt,y) in [(0.0,30.0),(0.05,30.0),(0.22,25.0),(0.34,20.0),(0.5,20.0)] {
            ui.frame(&input().time(base+dt),&mut build).unwrap();
            assert_eq!(ui.now(),base+dt);
            let r=ui.rect(&child.get().unwrap());
            assert!((r.y-y).abs()<0.005,"entrance y {r:?} expected {y}");
            let node=ui.a11y_nodes().iter().find(|n|ui.a11y_name_of(n)=="Moving child").unwrap();
            assert!((node.rect.y-y).abs()<0.005);
            assert_eq!(ui.layer(0).expect("base layer").instances,instances.as_slice(),"entrance updates the view table");
        }
        assert_eq!(builds.get(),1,"animation must not rebuild cached contents");
        assert!(ui.repaint_at().is_none(),"settled entrance must stop drawing");
        if round>0 {assert_eq!(mem_sys::local_alloc_count(),0);}
    }
}

#[test]
fn recycled_view_records_reserve_short_name_storage_without_losing_diagnostics() {
    use crate::ui::Node;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let seed=|ui:&mut Ui,label:&'static str| {
        for index in 0..16 {
            let parent=ui.node(crate::ui::int_key(0x91b17,index),Node::DEFAULT.w_px(10.0).h_px(10.0));
            ui.within(&parent,&mut |ui| {ui.view_of(label, &1, |_| {});});
        }
    };
    for _ in 0..2 {ui.frame(&input(),&mut |ui| seed(ui,"x")).unwrap();}
    ui.frame(&input(),&mut |_| {}).unwrap();
    mem_sys::local_reset();
    ui.frame(&input(),&mut |ui| seed(ui,"longer-recycled-view-name")).unwrap();
    assert_eq!(mem_sys::local_alloc_count(),0,"recycled labels up to 32 bytes already have room");
    assert_eq!(ui.rebuilt_named("longer-recycled"),16,"preserve full diagnostic labels");
    assert_eq!(ui.rebuilt_named("x"),0);
}

#[test]
fn reopening_nested_view_kinds_reuses_their_node_and_hit_capacities() {
    use crate::ui::{Node, Role};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.set_a11y(true);
    let shown=Cell::new(true);
    let mut build=|ui:&mut Ui| {
        if shown.get() {
            ui.view_of("panel", &1, |ui| {
                for (label,count) in [("header",2),("information",10),("clock",1),("queue",24)] {
                    ui.id(label, |ui| { ui.column(Node::column().h_px(40.0), |ui| { ui.view_of(label, &1, |ui| {
                        for _index in 0..count {
                            let b=ui.node("item",Node::DEFAULT.flags(pxui_types::Flags::CLICKABLE).w_px(10.0).h_px(1.0).role(Role::Button));
                            ui.a11y_name(&b,"item");
                        }
                    }); }); });
                }
            });
        }
    };
    for round in 0..8 {
        mem_sys::local_reset();
        for (step,value) in [true,false,true,false].into_iter().enumerate() {
            shown.set(value);
            ui.frame(&input().time((round*4+step) as f64),&mut build).unwrap();
        }
        if round>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"reopening view kinds"); }
    }
}

#[test]
fn accessibility_action_batches_keep_order_without_warm_allocations() {
    use crate::ui::{Node, Role};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap(); ui.set_a11y(true);
    let value=Cell::new(None);
    let mut build=|ui:&mut Ui| {
        let slider=ui.node("slider",Node::DEFAULT.flags(pxui_types::Flags::FOCUSABLE).role(Role::Slider).w_px(200.0).h_px(20.0));
        ui.a11y_name(&slider,"Accessible value");
        assert_eq!(ui.a11y_number(&slider,0.0,0.0,100.0,1.0), Ok(true));
        value.set(ui.signal(&slider).a11y_value);
    };
    ui.frame(&input(),&mut build).unwrap();
    let id=ui.a11y_nodes().iter().find(|node|ui.a11y_name_of(node)=="Accessible value").unwrap().id;
    let mut batch=[crate::A11yAction {id,action_code:4,..Default::default()};65];
    for cycle in 0..8 {
        for (index,action) in batch.iter_mut().enumerate() {action.payload=if cycle%2==0 {index as f64} else {(64-index) as f64};}
        mem_sys::local_reset();
        ui.inject_a11y(&batch);
        ui.frame(&input().time(cycle as f64),&mut build).unwrap();
        assert_eq!(value.get(),Some(if cycle%2==0 {64.0} else {0.0}));
        if cycle>1 {assert_eq!(mem_sys::local_alloc_count(),0,"ordered accessibility chunks");}
    }
}

#[test]
fn line_box_text_fits_wraps_and_resizes_in_one_frame_without_warm_allocations() {
    use crate::ui::{Node, Role};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap(); ui.set_a11y(true);
    let style=crate::Label::BODY.size(12.0).line_height(20.0).wrap();
    let value="one two three four";
    let expected=[80.0,200.0].map(|width| ui.text_service().measure(value,style,Some(width)).unwrap().lines as f32*20.0);
    assert!(expected[0]>expected[1],"fixture must wrap");
    let handle=Cell::new(None);
    for round in 0..6 {
        mem_sys::local_reset();
        for (index,width) in [80.0,200.0].into_iter().enumerate() {
            for empty in [false,true] {
                ui.frame(&input().time(round as f64),&mut |ui| {
                    ui.id("wrapper", |ui| { ui.column(Node::column().w_px(width), |ui| { ui.view_of("wrapper", &((width as u64)*2+u64::from(empty)), |ui| {
                        let node=ui.node("line",Node::DEFAULT.role(Role::Label).w(Size::Grow).h(Size::Fit));
                        ui.text_line_box(&node,if empty {""} else {value},style,&[]).unwrap();
                        handle.set(Some(node));
                    }); }); });
                }).unwrap();
                let rect=ui.rect(&handle.get().unwrap());
                assert!((rect.w-width).abs()<0.001);
                assert!((rect.h-if empty {20.0} else {expected[index]}).abs()<0.001,"wrapped intrinsic height {rect:?}");
            }
        }
        if round>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"line-box fit/resize reuses layout and glyphs");}
    }
    ui.frame(&input(),&mut |ui| {
        let node=ui.node("checked",Node::DEFAULT.role(Role::Label).w_px(200.0).h_px(20.0));
        let ink = { let mut record = crate::TextSpan::new(0, 3); record.color = crate::geom::Rgba::rgb(17, 59, 137); record.flags = crate::TextSpan::COLOR; record };
        ui.text_line_box(&node,value,style,&[ink]).unwrap();
        for invalid in [
            style.line_height(0.0), style.line_height(f32::NAN), style.font(u16::MAX),
            style.size(f32::MAX).tracking(f32::MAX),
        ] {
            assert!(ui.text_line_box(&node,"wrong",invalid,&[]).is_err());
        }
        assert!(ui.text_line_box(&node,"é",style,&[crate::TextSpan::new(1, 2)]).is_err());
    }).unwrap();
    assert!(ui.a11y_nodes().iter().any(|node|ui.a11y_name_of(node)==value));
    assert!(ui.layers().flat_map(|layer| layer.instances.iter()).any(|quad| quad.color == [17, 59, 137, 255]));
}

#[test]
fn measured_section_rows_settle_positions_pin_heading_and_reuse_warm_storage() {
    use std::cell::{Cell,RefCell};
    use crate::ui::{Node,SectionList,SectionPart};
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let count=Cell::new(10_000);let taller=Cell::new(false);let body=Cell::new(None);let header=Cell::new(None);
    let positions=RefCell::new(Vec::with_capacity(100));
    let mut build=|ui:&mut Ui| {
        positions.borrow_mut().clear();
        body.set(Some(ui.section_list("measured",&SectionList::new(count.get()).row_h(20.0).lead(100.0).header(30.0).trail(40.0).overlay_scrollbar().measured(),&|i| i as i64,&mut |ui,part,cell| {
            let y=cell.y;
            match part {
                SectionPart::Leading=>assert_eq!(y,0.0),
                SectionPart::Trailing=>{},
                SectionPart::Header=>{assert_eq!(y,100.0);header.set(Some(ui.node("heading",Node::DEFAULT.w(Size::Grow).h(Size::Grow))));},
                SectionPart::Row(index)=>{
                    let h=if taller.get() {40.0} else {20.0};
                    let row=ui.node("content",Node::DEFAULT.w(Size::Grow).h_px(h));
                    positions.borrow_mut().push((index,y,row));
                }
            }
        }).handle));
    };
    ui.frame(&input(),&mut build).unwrap();
    for cycle in 0..10 {
        mem_sys::local_reset();
        for tall in [true,false] {
            taller.set(tall);count.set(10_000);ui.set_scroll_offset(&body.get().unwrap(),0.0);
            ui.frame(&input(),&mut build).unwrap();
            let h=if tall {40.0} else {20.0};
            assert!(positions.borrow().len()<30,"only visible rows and overscan are built");
            for &(index,y,row) in positions.borrow().iter() {
                assert_eq!(y,130.0+index as f32*h);
                assert_eq!(ui.rect(&row).y,y);assert_eq!(ui.rect(&row).h,h);
            }
            ui.set_scroll_offset(&body.get().unwrap(),100.0);ui.frame(&input(),&mut build).unwrap();
            assert_eq!(ui.rect(&header.get().unwrap()).y,0.0);
            count.set(2);ui.frame(&input(),&mut build).unwrap();
            assert_eq!(ui.scroll(&body.get().unwrap()).settled,(170.0+2.0*h-240.0).max(0.0));
            assert_eq!(positions.borrow().len(),2);
        }
        if cycle>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"warm measured height changes/scroll/shrink");}
    }
}

#[test]
fn filled_triangles_preserve_vertices_winding_validate_and_allocate_nothing_when_warm() {
    mem_sys::set_peer(crate::library_counter);let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    for round in 0..8 {
        mem_sys::local_reset();
        for scale in [1.0,2.0] {
            ui.frame(&input().scale(scale),&mut |ui| {
                let node=ui.node("triangles",crate::Node::DEFAULT.w_px(100.0).h_px(100.0));
                ui.canvas(&node,|paint,_| {
                    let color=crate::geom::Rgba::rgba(12, 34, 56, 128);
                    paint.triangle((1.25,2.75), (5.75,2.75), (3.5,6.5), Fill::new(color));
                    paint.triangle((1.25,2.75), (3.5,6.5), (5.75,2.75), Fill::new(color));
                    for points in [
                        [(0.0,0.0);3],[(0.0,0.0),(1.0,1.0),(2.0,2.0)],
                        [(f32::NAN,0.0),(1.0,0.0),(2.0,2.0)],[(0.0,0.0),(f32::INFINITY,1.0),(2.0,2.0)],
                        [(-f32::MAX,0.0),(f32::MAX,0.0),(0.0,1.0)],
                        [(0.0,0.0),(1e-30,0.0),(0.0,1.0)],[(0.0,0.0),(1e30,0.0),(0.0,1e30)],
                    ] {paint.triangle(points[0], points[1], points[2], Fill::new(color));}
                });
            }).unwrap();
            let mut instances=ui.layers().flat_map(|layer|layer.instances.iter());
            let first=instances.next().unwrap();assert_eq!(first.kind,crate::draw::kind::TRIANGLE);
            assert_eq!(first.rect,[1.25,2.75,5.75,2.75]);assert_eq!(first.extra,[3.5f32.to_bits(),6.5f32.to_bits()]);assert_eq!(first.color,[12,34,56,128]);
            let reverse=instances.next().unwrap();assert_eq!(reverse.rect,[1.25,2.75,3.5,6.5]);assert_eq!(reverse.extra,[5.75f32.to_bits(),2.75f32.to_bits()]);
            assert_eq!(instances.count(),0,"invalid or degenerate triangles emit no instances");
        }
        if round>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"warm triangle build/draw storage");}
    }
}

#[test]
fn accessible_text_values_preserve_utf8_selection_precedence_and_warm_storage() {
    mem_sys::set_peer(crate::library_counter);let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();ui.set_a11y(true);
    for round in 0..8 {
        mem_sys::local_reset();
        for value in ["Not kept offline","Kept offline","Café","", "ready\0now","🎵e\u{301}","👩‍💻"] {
            ui.frame(&input(),&mut |ui| {
                let button=ui.node("value",crate::Node::DEFAULT.w_px(100.0).h_px(30.0).role(crate::ui::Role::Button));
                ui.a11y_name(&button,"Offline");assert_eq!(ui.a11y_value(&button,value,(value.len() as u32,0)), Ok(true));
            }).unwrap();
            let node=ui.a11y_nodes().iter().find(|node|ui.a11y_name_of(node)=="Offline").unwrap();
            assert_eq!(ui.a11y_value_of(node),if value.is_empty() {None} else {Some(value)});assert_eq!(node.selection,[value.chars().count() as u32,0],"accessibility output uses character offsets, as native text widgets do");
        }
        if round>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"warm accessible value/selection changes");}
    }
    let mut original = None;
    ui.frame(&input(),&mut |ui| {
        let node=ui.node("number",crate::Node::DEFAULT.w_px(100.0).h_px(30.0).role(crate::ui::Role::Slider));
        assert_eq!(ui.a11y_value(&node,"one",(0,0)), Ok(true));
        assert_eq!(ui.a11y_number(&node,1.0,0.0,2.0,1.0), Ok(true));
        original = Some(node);
    }).unwrap();
    let node=ui.a11y_nodes().iter().find(|node|node.is_role(crate::ui::Role::Slider)).unwrap();assert_eq!(node.value.tag,2);assert_eq!(ui.a11y_value_of(node),Some("one"),"a number keeps its text as its words (ABI 1.126)");
    let retired = original.unwrap();
    ui.frame(&input(), &mut |_| {}).unwrap();
    assert_eq!(ui.a11y_value(&retired, "stale", (0, 0)), Ok(false));
    let name = format!("{}é", "a".repeat((64 << 10) - 1));
    ui.frame(&input(), &mut |ui| {
        let replacement = ui.node("replacement", Node::DEFAULT.flags(crate::Flags::FOCUSABLE).role(crate::Role::Button));
        assert_eq!(replacement.box_slot, retired.box_slot, "exercise an actually recycled slot");
        assert_ne!(replacement.generation, retired.generation);
        assert!(ui.a11y_name(&replacement, &name));
        assert_eq!(ui.a11y_value(&replacement, &name, (0, 0)), Err(crate::Error::InvalidInput));
        assert_eq!(ui.a11y_number(&replacement, f64::NAN, 0.0, 1.0, 0.0), Err(crate::Error::InvalidInput));
        assert!(!ui.a11y_name(&retired, "stale"));
        assert_eq!(ui.a11y_number(&retired, 1.0, 0.0, 2.0, 1.0), Ok(false));
        ui.set_role(&retired, crate::Role::Slider, crate::A11yStates::NONE);
        ui.focus_box(&replacement);
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ui.focus_box(&retired))).is_err());
        assert!(ui.last_error().contains("stale focus box"));
        assert!(ui.focused(&replacement));
    }).unwrap();
    let replacement = ui.a11y_nodes().iter().find(|node| node.is_role(crate::Role::Button)).unwrap();
    assert_eq!(ui.a11y_name_of(replacement), &name[..name.len() - 2]);
    for (value,selection,expected) in [("é",(1,0),[0,0]),("aé",(2,99),[1,2]),("🎵x",(3,1),[0,0])] {
        ui.frame(&input(),&mut |ui| {
            let node=ui.node("normalized",crate::Node::DEFAULT.role(crate::ui::Role::Button));
            assert_eq!(ui.a11y_value(&node,value,selection), Ok(true));
        }).unwrap();
        let node=ui.a11y_nodes().iter().find(|node|ui.a11y_value_of(node)==Some(value)).unwrap();
        assert_eq!(node.selection,expected,"offsets inside a character round down, past the end clamp, never panic");
    }
}

#[test]
fn checked_renderer_creation_releases_the_gpu_lease_on_refusal() {
    use crate::{Error, Gpu, Renderer, TargetDesc};
    use std::cell::Cell;
    thread_local! { static RELEASES: Cell<u32> = const { Cell::new(0) }; }
    extern "C" fn release(_: *mut core::ffi::c_void) {
        RELEASES.with(|count| count.set(count.get() + 1));
    }
    let lease = || Gpu { release_owner: Some(release), ..Default::default() };
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let target = TargetDesc { format: 1, samples: 1 };
    assert!(matches!(Renderer::new(&ui, lease(), TargetDesc { samples: 0, ..target }), Err(Error::InvalidInput)));
    assert!(ui.last_error().contains("invalid target descriptor"));
    assert_eq!(RELEASES.with(Cell::get), 1);
    assert!(matches!(Renderer::new(&ui, Gpu { size: 0, ..lease() }, target), Err(Error::InvalidInput)));
    assert!(ui.last_error().contains("invalid gpu descriptor"));
    assert_eq!(RELEASES.with(Cell::get), 2);
    ui.frame(&input(), &mut |ui| {
        assert!(matches!(Renderer::new(ui, lease(), target), Err(Error::State)));
        assert!(ui.last_error().contains("outside a frame"));
        assert_eq!(RELEASES.with(Cell::get), 3);
    }).unwrap();
}

#[test]
fn host_environment_invalidates_cached_layout_and_reuses_warm_geometry() {
    use std::cell::Cell;
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.verify_views(false); // the test counts kept views' rebuilds or allocations; verification rebuilds them
    mem_sys::set_peer(crate::library_counter);
    let builds=Cell::new(0);
    let seen=Cell::new(crate::Environment::default());
    let mut build=|ui:&mut Ui| {
        ui.view_of("environment", &1, |ui| {
            builds.set(builds.get()+1); seen.set(ui.environment()); ui.label("Environment", crate::Label::DEFAULT);
        });
    };
    for round in 0..100 {
        let environment=crate::Environment::new([47.0,0.0,34.0,0.0]).keyboard_animation([0.0,if round%2==0 {500.0} else {844.0},390.0,344.0],
            [0.0,844.0,390.0,344.0],1.0,0.25,7,round%2==0);
        mem_sys::local_reset();
        let previous=builds.get();
        ui.frame(&input().time(round as f64).environment(environment),&mut build).unwrap();
        assert!(builds.get()>previous,"geometry changes must rebuild a kept view");
        assert_eq!(seen.get(),environment); assert_eq!(ui.environment(),environment);
        let previous=builds.get();
        ui.frame(&input().time(round as f64).environment(environment),&mut build).unwrap();
        assert_eq!(builds.get(),previous,"unchanged geometry must keep cached content");
        if round>=10 {assert_eq!(mem_sys::local_alloc_count(),0,"warm environment updates allocated");}
    }
    let prior=ui.environment();
    let mut invalid=prior; invalid.safe_area[0]=f32::NAN;
    assert_eq!(ui.frame(&input().environment(invalid),&mut build).err(),Some(crate::Error::InvalidInput));
    assert_eq!(ui.environment(),prior);
}

#[test]
fn touch_gestures_drive_virtual_rows_catch_coasts_and_keep_warm_allocations_zero() {
    use crate::{Event, Str, Touch};
    use crate::ui::{Node, SectionList, SectionPart};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.set_a11y(true);
    let body = Cell::new(None);
    let clicked = Cell::new(None);
    let built = Cell::new(0);
    let mut build = |ui: &mut Ui| {
        built.set(0);
        ui.view_of("touch-page", &0, |ui| {
            body.set(Some(ui.section_list("touch-list", &SectionList::new(10_000).row_h(24.0).overlay_scrollbar(),&|i| i as i64, &mut |ui, part,_| {
                if let SectionPart::Row(index) = part {
                    built.set(built.get() + 1);
                    let row = ui.node("touch-row", Node::DEFAULT.flags(pxui_types::Flags::CLICKABLE).w(Size::Grow).h(Size::Grow));
                    ui.a11y_name(&row, "Song");
                    let signal = ui.signal(&row);
                    if signal.clicked { clicked.set(Some(index)); }
                    assert!(!signal.hovered, "finger input must not leave mouse hover on a row");
                }
            }).handle));
        });
    };
    let mut packet = Touch { device: u64::MAX, finger: 7, x: 50.0, y: 100.0, pressure: 1.0, ..Touch::default() };
    // Start outside the list so the initial host mouse position does not hover it.
    let outside = Event::pointer(Event::POINTER_MOVED, 0, -1.0, -1.0);
    ui.frame(&{
        let mut frame = input();
        frame.events = core::slice::from_ref(&outside);
        frame
    }, &mut build).unwrap();
    let send = |ui: &mut Ui, packet: &Touch, build: &mut dyn FnMut(&mut Ui)| {
        let event = { let mut record = Event::default(); record.kind = Event::TOUCH; record.text = { let mut record = Str::default(); record.ptr = (packet as *const Touch).cast(); record.len = core::mem::size_of::<Touch>(); record }; record };
        ui.frame(&{
            let mut frame = input().time(packet.time);
            frame.events = core::slice::from_ref(&event);
            frame
        }, build).unwrap();
    };
    send(&mut ui, &packet, &mut build);
    packet.phase = 2; packet.time = 0.01;
    send(&mut ui, &packet, &mut build);
    assert!(clicked.take().is_some(), "a touch tap did not activate a real virtual row");
    for (phase, y, time) in [(0,200.0,0.02),(1,-200.0,0.03),(1,-300.0,0.04),(3,-300.0,0.05)] {
        packet.phase = phase; packet.y = y; packet.time = time;
        send(&mut ui, &packet, &mut build);
    }
    assert!(ui.scroll(&body.get().unwrap()).settled >= 499.0, "retiring the original row cancelled its ancestor scroll capture");
    for frame in 0..182 {
        packet.time = 1.0 + frame as f64 / 120.0;
        packet.phase = (frame % 4) as u32;
        packet.y = if packet.phase == 0 { 100.0 } else { 60.0 };
        clicked.set(None);
        mem_sys::local_reset();
        send(&mut ui, &packet, &mut build);
        assert!(clicked.get().is_none(), "scroll/catch/cancel activated a virtual row");
        assert!(built.get() <= 10 + 1 + 2 * 4 + 1, "frame {frame} exceeded viewport rows plus overscan: {}", built.get());
        if frame >= 82 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm virtual finger frame {frame}"); }
    }
    assert!(ui.scroll(&body.get().unwrap()).settled > 100.0);
}

#[test]
fn keyboard_presentation_updates_cached_content_bounds_without_warm_allocations() {
    use crate::{Environment, Keyboard};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.verify_views(false); // the test counts kept views' rebuilds or allocations; verification rebuilds them
    let node = Cell::new(None);
    let builds = Cell::new(0);
    let expected = Cell::new([0.0, 47.0, 390.0, 763.0]);
    let mut build = |ui: &mut Ui| {
        ui.view_of("keyboard-content", &0, |ui| {
            builds.set(builds.get()+1);
            let rect = ui.content_rect();
            assert_eq!([rect.x, rect.y, rect.w, rect.h], expected.get());
            node.set(Some(ui.node("visible content", crate::Node::DEFAULT.at(rect.x, rect.y).w_px(rect.w).h_px(rect.h))));
        });
    };
    let environment = Environment::new([47.0,0.0,34.0,0.0]);
    for frame in 0..182 {
        let y = [810.0,750.0,600.0,508.0,600.0,750.0][frame%6];
        let flags = Keyboard::VALID | if matches!(frame%6,0|3) {0} else {Keyboard::ANIMATING};
        let keyboard = Keyboard::new(pxui_types::geom::Rect::new(0.0, y, 390.0, 844.0-y), flags);
        expected.set([0.0,47.0,390.0,y.min(810.0)-47.0]);
        mem_sys::local_reset();
        ui.frame(&FrameIn::new(390.0, 844.0).time(frame as f64/120.0).environment(environment).keyboard(keyboard), &mut build).unwrap();
        assert_eq!(ui.keyboard(), keyboard);
        let rect=ui.rect(&node.get().unwrap());
        assert_eq!([rect.x,rect.y,rect.w,rect.h],expected.get());
        assert_eq!(ui.repaint_at().is_some(),flags & Keyboard::ANIMATING != 0);
        let before=builds.get();
        ui.frame(&FrameIn::new(390.0, 844.0).time(frame as f64/120.0).environment(environment).keyboard(keyboard), &mut build).unwrap();
        assert_eq!(builds.get(),before,"unchanged keyboard geometry rebuilt a cached view");
        if frame>=82 {assert_eq!(mem_sys::local_alloc_count(),0,"warm keyboard frame {frame}");}
    }
    for (keyboard,height) in [
        (Keyboard::new(pxui_types::geom::Rect::new(100.0, 400.0, 200.0, 200.0), Keyboard::VALID),763.0),
        (Keyboard::new(pxui_types::geom::Rect::new(0.0, 900.0, 390.0, 100.0), Keyboard::VALID),763.0),
        (Keyboard::new(pxui_types::geom::Rect::new(0.0, -100.0, 390.0, 1000.0), Keyboard::VALID),0.0),
    ] {
        expected.set([0.0,47.0,390.0,height]);
        ui.frame(&FrameIn::new(390.0, 844.0).environment(environment).keyboard(keyboard),&mut build).unwrap();
    }
}

#[test]
fn text_field_refocus_reuses_its_warm_editor_storage() {
    use pxui_types::{Event};
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut build = |ui: &mut Ui| assert!(ui.text_field("Caption", "café track 10", &crate::TextField::DEFAULT).changed.is_none());
    let focus = [
        Event::key(Event::KEY_PRESSED, crate::KeyCode::Tab.wire()),
        Event::key(Event::KEY_RELEASED, crate::KeyCode::Tab.wire()),
    ];
    ui.frame(&input(), &mut build).unwrap();
    for frame in 0..182 {
        mem_sys::local_reset();
        let focusing = matches!(frame, 1 | 90);
        let focused = (1..40).contains(&frame) || (90..130).contains(&frame);
        if matches!(frame,40|130) { ui.unfocus(); }
        ui.frame(&{
            let mut frame = input().time(frame as f64/60.0);
            frame.events = if focusing { &focus[..] } else { &[] };
            frame
        }, &mut build).unwrap();
        assert_eq!(ui.text_input_focused(),focused);
        if frame>=82 {assert_eq!(mem_sys::local_alloc_count(),0,"warm field refocus frame {frame}");}
    }
}

#[test]
fn text_input_options_follow_the_focused_field_and_done_keeps_warm_storage() {
    use crate::{Event, TextField, TextTraits as Options};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let show_search=Cell::new(true);
    let done=Options::SEARCH.action(crate::ReturnKey::Done);
    let mut build=|ui:&mut Ui| {
        ui.view_of("input options", &(show_search.get() as u64), |ui| {
            if show_search.get() { assert!(ui.text_field("Search","café",&TextField::SEARCH).changed.is_none()); }
            assert!(ui.text_field("Name","café",&TextField::SEARCH.action(crate::ReturnKey::Done)).changed.is_none());
        });
    };
    ui.frame(&input(),&mut build).unwrap();
    for frame in 0..182 {
        let step=frame%6;
        let key = if matches!(step, 0 | 2) { crate::KeyCode::Tab } else { crate::KeyCode::Enter };
        let code = key.wire();
        let events=[Event::key(Event::KEY_PRESSED, code),Event::key(Event::KEY_RELEASED, code)];
        let active=matches!(step,0|2|4);
        mem_sys::local_reset();
        ui.frame(&{
            let mut frame = input().time(frame as f64);
            frame.events = &events[..if active {2} else {0}];
            frame
        },&mut build).unwrap();
        let expected=match step {0|1=>Options::SEARCH,2|3=>done,_=>Options::default()};
        assert_eq!(ui.text_traits(),expected,"frame {frame}");
        assert_eq!(ui.ime().is_some(),step<4,"Done must dismiss native input in the same frame");
        if step==4 {assert!(ui.key_pressed(key, crate::Mods::ANY),"raw key queries remain non-consuming");}
        if frame>=82 {assert_eq!(mem_sys::local_alloc_count(),0,"warm keyboard options frame {frame}");}
    }
    show_search.set(false);
    ui.frame(&input(),&mut build).unwrap();
    assert_eq!(ui.text_traits(),Options::default(),"an unfocused or omitted field cannot retain options");
}

#[test]
fn marked_text_replaces_the_selection_and_tracks_unicode_caret_without_warm_allocations() {
    use crate::{Event,Part,Str};
    fn body(ui:&mut Ui,c:&crate::bodies::TextFieldConfig) { use crate::bodies::BodyConfig;
        ui.style(&c.row,&crate::theme::States::of(crate::theme::Look::NONE),crate::theme::State::default());
        let editor=ui.part(Part::EDITOR,crate::Node::DEFAULT.w_px(240.0).h_px(30.0).look(crate::theme::Look::NONE.font(crate::theme::Font::BODY.size(18.0))));
        if c.state.focused==0 {ui.text(&editor,c.text_of(&c.value));}
    }
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.register_text_field_body("marked-editor",body).unwrap();
    let source="text_field.body = marked-editor";
    ui.set_theme(&crate::ThemeHandle::parse(&source).unwrap()).unwrap();
    let mut build=|ui:&mut Ui| {
        let field = ui.text_field("Marked", "café track", &crate::TextField::DEFAULT);
        assert!(field.changed.is_none());
        let response = field.response;
        if response.focused { ui.focus_box(&response.handle); }
    };
    ui.frame(&input(),&mut build).unwrap();
    let key=|kind,n: crate::KeyCode|Event::key(kind, n.wire());
    let focus=[key(Event::KEY_PRESSED,crate::KeyCode::Tab),key(Event::KEY_RELEASED,crate::KeyCode::Tab),key(Event::KEY_PRESSED,crate::KeyCode::Meta),
        Event::key(Event::KEY_PRESSED, 'a' as u32),key(Event::KEY_RELEASED,crate::KeyCode::Meta)];
    ui.frame(&{
        let mut frame = input();
        frame.events = &focus[..];
        frame
    },&mut build).unwrap();
    let value="éix";
    for (start,length) in [(f64::from(i32::MAX),f64::from(i32::MAX)),(1.0,1.0)] {
        let event={ let mut record = Event::default(); record.kind = Event::TEXT_EDITING; record.text = { let mut record = Str::default(); record.ptr = value.as_ptr(); record.len = value.len(); record }; record.x = start; record.y = length; record };
        ui.frame(&{
            let mut frame = input();
            frame.events = core::slice::from_ref(&event);
            frame
        },&mut build).unwrap();
        let prefix=if start>3.0 {value} else {"éi"};
        assert_eq!(ui.ime().unwrap().caret,(2.0+ui.text_service().width(prefix,crate::Label::BODY.size(18 as f32),None).unwrap()).round(),"marked selection uses Unicode scalar offsets");
    }
    for frame in 0..182 {
        mem_sys::local_reset();
        ui.frame(&input().time(frame as f64/120.0),&mut build).unwrap();
        assert_eq!(ui.layers().flat_map(|layer|layer.instances.iter()).filter(|quad|quad.kind==crate::draw::kind::MASK).count(),3,
            "preedit must replace the selected base text instead of appending to it");
        assert_eq!(ui.ime().unwrap().caret,(2.0+ui.text_service().width("éi",crate::Label::BODY.size(18 as f32),None).unwrap()).round());
        if frame>=82 {assert_eq!(mem_sys::local_alloc_count(),0,"warm marked selection frame {frame}");}
    }
    let commit=[{ let mut record = Event::default(); record.kind = Event::TEXT_INPUT; record.text = { let mut record = Str::default(); record.ptr = value.as_ptr(); record.len = value.len(); record }; record },key(Event::KEY_PRESSED,crate::KeyCode::Enter),key(Event::KEY_RELEASED,crate::KeyCode::Enter)];
    let mut changed=false;
    ui.frame(&{
        let mut frame = input();
        frame.events = &commit[..];
        frame
    },&mut |ui| {
        changed=ui.text_field("Marked","café track",&crate::TextField::DEFAULT.action(crate::ReturnKey::Done)).changed==Some(value);
    }).unwrap();
    assert!(changed,"commit replaces precisely the original UTF-8 selection");
    assert!(ui.ime().is_none());
    assert_eq!(ui.layers().flat_map(|layer|layer.instances.iter()).filter(|quad|quad.kind==crate::draw::kind::MASK).count(),3,"commit and Done paint the latest value in the same frame");
}

#[test]
fn keyless_text_measurement_retains_interleaved_values_styles_widths_and_spans() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut text = crate::Text::new(&[crate::pragmasevka()]).unwrap();
    let plain = crate::Label::BODY.size(14.0).line_height(20.0).wrap();
    let cases = [
        ("1111", plain, 0.0),
        ("88888", plain, 0.0),
        ("1111", plain.tabular(), 0.0),
        ("office café words", plain, 40.0),
        ("office café words", plain, 90.0),
        ("office café words", plain.weight(700).tracking(0.2 / 14.0), 40.0),
    ];
    let resolved = cases.map(|(_,label,_)| ui.resolve(&label));
    let expected = cases.map(|(value,label,width)| text.measure(value,&ui.resolve(&label),width).unwrap());
    assert_ne!(expected[0].width,expected[1].width);
    assert_ne!(expected[3].lines,expected[4].lines);
    let spans = [{ let mut record = crate::TextSpan::new(0, 6); record.weight = 700; record }];
    let rich = text.measure_spans(cases[3].0,&resolved[3],40.0,&spans).unwrap();
    for round in 0..6 {
        mem_sys::local_reset();
        ui.frame(&input(), &mut |ui| {
            for (index,(value,label,width)) in cases.into_iter().enumerate() {
                assert_eq!(text.measure(value,&resolved[index],width).unwrap(),expected[index]);
                assert_eq!(text.width(value,&resolved[index],width).unwrap(),expected[index].width);
                assert_eq!(ui.text_service().measure(value,label,Some(width)).unwrap(),expected[index]);
            }
            assert_eq!(text.measure_spans(cases[3].0,&resolved[3],40.0,&spans).unwrap(),rich);
            assert_eq!(ui.text_service().measure_spans(cases[3].0,plain,Some(40.0),&spans).unwrap(),rich);
        }).unwrap();
        if round>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"keyless layout identity must retain every interleaved variant"); }
    }
}

#[test]
fn pixel_fit_text_rounds_density_and_reuses_recycled_identity_style_and_span_layouts() {
    use crate::{TextSpan,ui::{Node,Look,Font,Role}};
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();ui.set_a11y(true);
    let mut text=crate::Text::new(&[crate::pragmasevka()]).unwrap();
    let span=[{ let mut record = TextSpan::new(0, 2); record.weight = 700; record }];
    let mut nodes=[None;9];
    let look=Look::NONE.text(crate::geom::Rgba::rgba(0,0,0,255));
    let values=["ab cd","écho","a slightly longer label",""];
    let styles=[crate::Label::BODY.size(14.25).line_height(20.0),crate::Label::BODY.size(16.5).line_height(22.0).tracking(0.15/16.5)];
    let widths=styles.map(|style|values.map(|value| text.measure(value,&ui.resolve(&style),0.0).unwrap().width));
    let span_width=text.measure_spans(values[0],&ui.resolve(&styles[0]),0.0,&span).unwrap().width;
    let plain_width=text.width("AB CD",&ui.resolve(&crate::Label::BODY.size(14.0)),0.0).unwrap();
    let mut serial=0;
    for round in 0..8 {
        for scale in [1.0,2.0,1.25,1.75] {
            for (variant,style) in styles.into_iter().enumerate() {
                mem_sys::local_reset();serial+=1;
                ui.frame(&input().scale(scale).time(serial as f64/60.0),&mut |ui| {
                    for (index,value) in values.into_iter().enumerate() {
                        let node=ui.node(index as i64+100+100*(round%2) as i64,Node::DEFAULT.flags(pxui_types::Flags::FIT_TEXT|pxui_types::Flags::FIT_TEXT_PIXEL).w(Size::Fit).h_px(24.0).role(Role::Label).look(look));
                        ui.text_line_box(&node,value,style,&[]).unwrap();nodes[index]=Some(node);
                    }
                    let node=ui.node("spans",Node::DEFAULT.flags(pxui_types::Flags::FIT_TEXT_PIXEL).w(Size::Fit).h_px(24.0).look(look));
                    ui.text_spans(&node,values[0],styles[0],&span).unwrap();nodes[4]=Some(node);
                    let node=ui.node("plain-caps",Node::DEFAULT.flags(pxui_types::Flags::FIT_TEXT_PIXEL).w(Size::Fit).h_px(24.0).role(Role::Label).look(look.font(Font::BODY.size(14.0).caps())));
                    ui.text(&node,values[0]);nodes[5]=Some(node);
                    let node=ui.node("limited",Node::DEFAULT.flags(pxui_types::Flags::FIT_TEXT_PIXEL).w(Size::Fit).max_w(10.25).h_px(24.0).look(look));
                    ui.text_line_box(&node,values[2],style,&[]).unwrap();nodes[6]=Some(node);
                    let node=ui.node("insets",Node::DEFAULT.flags(pxui_types::Flags::FIT_TEXT_PIXEL).w(Size::Fit).h_px(24.0).inset(crate::ui::Edges::default().l(0.2).r(0.3)).look(look));
                    ui.text_line_box(&node,values[0],style,&[]).unwrap();nodes[7]=Some(node);
                    let node=ui.node("plain-limited",Node::DEFAULT.flags(pxui_types::Flags::FIT_TEXT|pxui_types::Flags::FIT_TEXT_PIXEL).w(Size::Fit).max_w(10.25).h_px(24.0).look(Look::NONE.text(crate::geom::Rgba::rgba(255,0,0,255)).font(Font::BODY.size(14.0))));
                    ui.text(&node,values[0]);nodes[8]=Some(node);
                }).unwrap();
                let density=ui.density();
                let ceil=|value:f32|(value*density).ceil()/density;
                for index in 0..4 {assert_eq!(ui.rect(&nodes[index].unwrap()).w,ceil(widths[variant][index]));}
                assert_eq!(ui.rect(&nodes[4].unwrap()).w,ceil(span_width));
                assert_eq!(ui.rect(&nodes[5].unwrap()).w,ceil(plain_width));
                assert_eq!(ui.rect(&nodes[6].unwrap()).w,10.25,"explicit maximum still wins");
                assert_eq!(ui.rect(&nodes[7].unwrap()).w,ceil(widths[variant][0]+0.5));
                assert_eq!(ui.rect(&nodes[8].unwrap()).w,10.25);
                assert_eq!(ui.layers().flat_map(|layer|layer.instances.iter()).filter(|i|i.kind==crate::draw::kind::MASK && i.color==[255,0,0,255]).count(),1,"constrained plain text retains a shaped ellipsis");
                assert!(ui.a11y_nodes().iter().any(|node|ui.a11y_name_of(node)==values[2]),"full accessible labels survive fitting");
                if round>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"warm pixel-fit density/style/span and recycled identity frames");}
            }
        }
    }
}

#[test]
fn text_into_forms_match_and_allocate_nothing_with_capacity() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut shaper = crate::Text::new(&[crate::pragmasevka()]).unwrap();
    let label = crate::Label::BODY;
    let style = ui.resolve(&label);
    let color = [1, 2, 3, 255];
    let mut glyphs = Vec::new();
    let mut ui_pixels = Vec::new();
    let mut shaper_pixels = Vec::new();
    for _ in 0..2 {
        ui.text_service().quads("abc def", label, None, crate::Point::new(3.0, 4.0), 1.0, &mut glyphs).unwrap();
        ui.text_service().rasterize_rgba("abc def", label, None, 1.0, color, &mut ui_pixels).unwrap();
        shaper.rasterize_rgba("abc def", &style, 0.0, 1.0, color, &mut shaper_pixels).unwrap();
    }
    assert_eq!(glyphs, { let mut glyphs=Vec::new(); ui.text_service().quads("abc def", label, None, crate::Point::new(3.0, 4.0), 1.0, &mut glyphs).map(|_|glyphs) }.unwrap());
    let (w, h, old) = { let mut px=Vec::new(); ui.text_service().rasterize_rgba("abc def", label, None, 1.0, color, &mut px).map(|(w,h)|(w,h,px)) }.unwrap();
    assert_eq!(ui.text_service().rasterize_rgba("abc def", label, None, 1.0, color, &mut ui_pixels), Ok((w, h)));
    assert_eq!(ui_pixels, old);
    assert_eq!({ let mut px=Vec::new(); shaper.rasterize_rgba("abc def", &style, 0.0, 1.0, color, &mut px).map(|(w,h)|(w,h,px)) }.unwrap(), (w, h, shaper_pixels.clone()));
    let scope = mem_sys::warm_scope();
    mem_sys::local_reset();
    ui.text_service().quads("abc def", label, None, crate::Point::new(3.0, 4.0), 1.0, &mut glyphs).unwrap();
    ui.text_service().rasterize_rgba("abc def", label, None, 1.0, color, &mut ui_pixels).unwrap();
    shaper.rasterize_rgba("abc def", &style, 0.0, 1.0, color, &mut shaper_pixels).unwrap();
    assert_eq!(mem_sys::local_alloc_count(), 0);
    drop(scope);
    assert!(mem_sys::take_reports().is_empty());
}

#[test]
fn colour_tokens_follow_a_light_dark_theme_swap_without_app_rebuild_or_warm_allocations() {
    use std::{cell::Cell, rc::Rc};
    use pxui_types::{geom::Rgba, theme::ColorToken};
    mem_sys::set_peer(crate::library_counter);
    let c = |v: u8| Rgba::rgb(v, v, v);
    let mut light = crate::ThemeHandle::parse("tokens.panel = #f4f5f0").unwrap();
    let mut dark = crate::ThemeHandle::parse("tokens.dark = true\ntokens.panel = #101010").unwrap();
    light.set_color(ColorToken(0), c(240));
    light.set_color(ColorToken(1), c(200));
    dark.set_color(ColorToken(0), c(16));
    dark.set_color(ColorToken(1), c(60));
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.set_theme(&light).unwrap();
    // what the app reads while building (inside a retained view whose revision never changes), painting and in a scope
    let (built, painted, scoped) = (Cell::new(Rgba::default()), Rc::new(Cell::new(Rgba::default())), Cell::new(Rgba::default()));
    let mut build = |ui: &mut Ui| {
        ui.column(Node::column().w_px(100.0).h_px(60.0), |ui| { ui.view_of("tokens", &1, |ui| {
            built.set(ui.color(ColorToken(1)));
            let b = ui.node("canvas", crate::Node::DEFAULT.w_px(20.0).h_px(10.0));
            let painted = painted.clone();
            ui.canvas(&b, move |p, _| painted.set(p.color(ColorToken(0))));
        }); });
        ui.themed(&dark, &mut |ui| scoped.set(ui.color(ColorToken(0))));
    };
    for round in 0..4 {
        ui.frame(&input(), &mut build).unwrap();
        assert_eq!((ui.color(ColorToken(0)), built.get(), painted.get(), scoped.get()), (c(240), c(200), c(240), c(16)), "light theme: light entries; a dark scope reads dark");
        mem_sys::local_reset();
        ui.set_theme(&dark).unwrap();
        assert_eq!(ui.color(ColorToken(0)), c(16), "reads change at the swap, before any frame");
        ui.frame(&input(), &mut build).unwrap();
        assert_eq!((built.get(), painted.get()), (c(60), c(16)), "the unchanged-revision view and its canvas follow the dark theme");
        let mut sum = 0u32;
        for token in (0..(ColorToken::LIMIT as u16 + 8)).filter(|t| *t != ColorToken::ACCENT.0) { sum += ui.color(ColorToken(token)).r as u32; }
        assert_eq!(sum, 16 + 60, "unset and out-of-range tokens read transparent");
        assert_eq!(ui.color(ColorToken::ACCENT), ui.tokens().accent, "the accent token reads the theme's accent");
        ui.set_theme(&light).unwrap();
        if round >= 2 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm swaps, frames and reads allocate nothing"); }
    }
    assert_eq!(ui.color(ColorToken(65)), Rgba::default(), "past the table and the accent reads transparent");
}

#[test]
fn instant_theme_install_preserves_widget_motion_and_paints_the_new_look_immediately() {
    use crate::{Node, Rgba, ThemeHandle, theme::{ColorToken, Look, Tokens}};
    const INK: ColorToken = ColorToken(0);
    let mut tokens = Tokens::default();
    tokens.motion.normal = 0.2;
    tokens.motion.fast = 0.14;
    let mut light = ThemeHandle::from_tokens(tokens);
    light.set_color(INK, Rgba::rgb(240, 240, 240));
    tokens.dark = true;
    let mut dark = ThemeHandle::from_tokens(tokens);
    dark.set_color(INK, Rgba::rgb(16, 16, 16));
    dark.animate_swap(false);

    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.set_theme(&light).unwrap();
    let mut build = |ui: &mut Ui| {
        ui.view_of("tile", &0, |ui| {
            ui.node("fill", Node::column().w_px(30.0).h_px(30.0).look(Look::NONE.fill(Rgba::token(INK))));
        });
    };
    ui.frame(&input(), &mut build).unwrap();
    let rgb = |ui: &Ui| ui.layers().flat_map(|layer| layer.instances.iter())
        .find(|instance| instance.rect == [0.0, 0.0, 30.0, 30.0]).unwrap().color;
    assert_eq!(rgb(&ui), [240, 240, 240, 255]);
    ui.set_theme(&dark).unwrap();
    ui.frame(&input(), &mut build).unwrap();
    assert_eq!(rgb(&ui), [16, 16, 16, 255]);
    assert_eq!(ui.tokens().motion, tokens.motion);
}

#[test]
fn positioned_label_matches_the_node_line_box_and_reports_invalid_geometry() {
    use crate::{Label, Node, Rect, Rgba, Role, ThemeHandle, theme::{ColorToken, Look}};
    const INK: ColorToken = ColorToken(0);
    let mut theme = ThemeHandle::default();
    theme.set_color(INK, Rgba::rgb(17, 59, 137));
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.set_theme(&theme).unwrap();
    ui.set_a11y(true);
    let rect = Rect::new(7.0, 15.0, 70.0, 38.0);
    let text = "An accessible title";
    let style = Label::BODY.size(15.0).line_height(19.0).wrap_spaces().color(INK);
    for scale in [1.0, 2.0] {
        let mut instances = Vec::new();
        for positioned in [false, true] {
            ui.frame(&input().scale(scale), &mut |ui| {
                if positioned {
                    let invalid = Rect { w: -1.0, ..rect };
                    assert_eq!(ui.label_at("invalid".into(), invalid, text, style).unwrap_err(), crate::Error::InvalidInput);
                    assert!(ui.box_of_key("invalid").is_none());
                    ui.label_at("title".into(), rect, text, style).unwrap();
                } else {
                    let box_ = ui.node("title", Node::column().w_px(rect.w).h_px(rect.h).at(rect.x, rect.y)
                        .role(Role::Label).look(Look::NONE.text(Rgba::token(INK))));
                    ui.text_line_box(&box_, text, style, &[]).unwrap();
                }
            }).unwrap();
            assert_eq!(crate::testing::find(&ui, Role::Label, text), Some(rect));
            let actual = ui.layer(0).unwrap().instances;
            assert!(!actual.is_empty());
            if positioned { assert_eq!(actual, instances); } else { instances.extend_from_slice(actual); }
        }
    }
}

#[test]
fn positioned_label_redeclaration_clears_a_previous_layout_override() {
    use crate::{Label, Node, Rect, Role};
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.set_a11y(true);
    let rect = Rect::new(7.0, 15.0, 70.0, 38.0);
    let moved = Rect::new(17.0, 5.0, 90.0, 32.0);
    for frame in 0..4 {
        ui.frame(&input(), &mut |ui| {
            let label = ui.label_at("title".into(), rect, "Track", Label::BODY.line_height(19.0)).unwrap();
            if frame == 2 {
                ui.set_node_layout(&label, &Node::column().w_px(moved.w).h_px(moved.h).at(moved.x, moved.y));
            }
        }).unwrap();
        assert_eq!(crate::testing::find(&ui, Role::Label, "Track"), Some(if frame == 2 { moved } else { rect }));
    }
}

#[test]
fn instant_palette_reuse_matches_fresh_positioned_labels_and_rebuilds_canvases() {
    use crate::{Appearance, Fill, Label, Rect, Rgba, ThemeHandle, theme::ColorToken};
    use std::cell::Cell;

    let split_ink = |dark: bool| {
        let mut theme = ThemeHandle::parse(if dark { "tokens.dark = true\nlabel.text = #54ff0200" } else { "label.text = #54ff0200" }).unwrap();
        theme.animate_swap(false);
        theme.set_color(ColorToken(0), if dark { Rgba::rgb(60, 30, 90) } else { Rgba::rgb(10, 20, 30) });
        theme.set_color(ColorToken(1), if dark { Rgba::rgb(90, 30, 60) } else { Rgba::rgb(10, 20, 30) });
        theme.set_color(ColorToken(2), if dark { Rgba::rgb(52, 62, 72) } else { Rgba::rgb(22, 32, 42) });
        theme
    };
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.verify_views(false);
    let light = split_ink(false);
    let dark = split_ink(true);
    let label_builds = Cell::new(0);
    let canvas_builds = Cell::new(0);
    let mut build = |ui: &mut Ui| {
        ui.view_of("placed", &0, |ui| {
            label_builds.set(label_builds.get() + 1);
            for (index, text) in ["Snow", "River", "Hill"].into_iter().enumerate() {
                let style = Label::BODY.line_height(20.0);
                let style = if index < 2 { style.color(ColorToken(index as u16)) } else { style };
                ui.label_at((index as i64).into(), Rect::new(4.0, index as f32 * 20.0, 100.0, 20.0), text, style).unwrap();
            }
            ui.retain_palette_labels();
        });
        ui.view_of("canvas", &0, |ui| {
            canvas_builds.set(canvas_builds.get() + 1);
            let label = ui.label_at("placed".into(), Rect::new(140.0, 0.0, 40.0, 20.0), "View", Label::BODY.line_height(20.0)).unwrap();
            ui.canvas(&label, |paint, rect| paint.fill(rect, Fill::new(Rgba::token(ColorToken(0)))));
            ui.retain_palette_labels();
        });
    };
    let snapshot = |ui: &Ui| ui.layer(0).unwrap().instances.iter().filter(|instance| **instance != crate::draw::Instance::DEFAULT).copied().collect::<Vec<_>>();
    ui.set_theme(&light).unwrap();
    ui.frame(&input().appearance(Appearance::Light), &mut build).unwrap();
    let old = snapshot(&ui);
    let counts = (label_builds.get(), canvas_builds.get());
    ui.set_theme(&dark).unwrap();
    assert_eq!(snapshot(&ui), old, "changing the theme does not change an already published frame");
    ui.frame(&input().appearance(Appearance::Dark), &mut build).unwrap();
    assert_eq!((label_builds.get(), canvas_builds.get()), (counts.0, counts.1 + 1));
    let retained = snapshot(&ui);
    assert_ne!(old, retained);
    ui.verify_views(true);
    ui.frame(&input().appearance(Appearance::Dark), &mut build).unwrap();
    assert_eq!(snapshot(&ui), retained);

    ui.verify_views(false);
    let mut moved = split_ink(false);
    moved.density(2.0);
    let before = label_builds.get();
    ui.set_theme(&moved).unwrap();
    ui.frame(&input(), &mut build).unwrap();
    assert!(label_builds.get() > before, "a theme with different metrics rebuilds the view");
}

#[test]
fn instant_palette_replays_token_shapes_and_labels_in_both_directions() {
    use crate::{Appearance, Fill, Label, Node, Rect, Rgba, Stroke, ThemeHandle, theme::ColorToken};
    use std::cell::Cell;

    const FILL: ColorToken = ColorToken(0);
    const STROKE: ColorToken = ColorToken(1);
    let theme = |dark: bool| {
        let mut theme = ThemeHandle::parse(if dark { "tokens.dark = true" } else { "" }).unwrap();
        theme.animate_swap(false);
        theme.set_color(FILL, if dark { Rgba::rgb(70, 30, 15) } else { Rgba::rgb(20, 20, 20) });
        theme.set_color(STROKE, if dark { Rgba::rgb(15, 70, 30) } else { Rgba::rgb(20, 20, 20) });
        theme
    };
    let (light, dark) = (theme(false), theme(true));
    let builds = Cell::new(0);
    let build = |ui: &mut Ui, retain: bool| {
        ui.view_of("palette", &0, |ui| {
            if retain {
                ui.retain_palette_paint();
                builds.set(builds.get() + 1);
            }
            let surface = ui.node("rule", Node::column().w_px(180.0).h_px(60.0));
            ui.canvas(&surface, |paint, _| {
                paint.fill(Rect::new(1.25, 3.0, 40.0, 4.0), Fill::new(Rgba::token(STROKE)).fractional());
            });
            ui.within(&surface, &mut |ui| {
                let label = Label::BODY.line_height(18.0).color(FILL);
                ui.label_at("name".into(), Rect::new(2.0, 8.0, 70.0, 18.0), "Track", label).unwrap();
                ui.view_of("nested", &0, |ui| {
                    if retain { ui.retain_palette_paint(); }
                    let dot = ui.node("dot", Node::column().w_px(50.0).h_px(30.0));
                    ui.canvas(&dot, |paint, rect| {
                        paint.circle((rect.x + 17.0, rect.y + 16.0), 4.0,
                            Some(Fill::new(Rgba::token(FILL).alpha(153))),
                            Some(Stroke::new(Rgba::token(STROKE)).width(1.25)));
                    });
                });
            });
        });
    };
    let snapshot = |ui: &Ui| ui.layer(0).map_or_else(Vec::new, |layer|
        layer.instances.iter().filter(|i| **i != crate::draw::Instance::DEFAULT).copied().collect::<Vec<_>>());
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut fresh = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    fresh.verify_views(true);
    let mut previous = Vec::new();
    let mut first_light = Vec::new();
    for (theme, appearance) in [(&light, Appearance::Light), (&dark, Appearance::Dark), (&light, Appearance::Light)] {
        ui.verify_views(false);
        ui.set_theme(theme).unwrap();
        assert_eq!(snapshot(&ui), previous, "a swap cannot mutate published output before a frame");
        let before = builds.get();
        ui.frame(&input().appearance(appearance), &mut |ui| build(ui, true)).unwrap();
        let retained = snapshot(&ui);
        assert!(!retained.is_empty());
        if !previous.is_empty() {
            assert_ne!(retained, previous);
            assert_eq!(builds.get(), before, "canvas colors replay without building the view");
        }
        ui.verify_views(true);
        ui.frame(&input().appearance(appearance), &mut |ui| build(ui, true)).unwrap();
        assert_eq!(snapshot(&ui), retained, "retained paint matches a fresh verified build");

        fresh.set_theme(theme).unwrap();
        fresh.frame(&input().appearance(appearance), &mut |ui| build(ui, false)).unwrap();
        assert_eq!(snapshot(&fresh), retained, "symbolic and concrete painters produce the same output");
        if appearance == Appearance::Light {
            if first_light.is_empty() { first_light.clone_from(&retained); }
            else { assert_eq!(retained, first_light); }
        }
        previous = retained;
    }
}

#[test]
fn viewport_independent_view_keeps_its_local_layout_only_at_the_same_size_and_density() {
    use crate::{Label, Node, Rect, Role};
    use std::cell::Cell;

    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.verify_views(false);
    ui.set_a11y(true);
    let builds = Cell::new(0);
    let actual_width = Cell::new(100.0);
    let mut build = |ui: &mut Ui| {
        let x = ui.view_size().width - actual_width.get();
        let node = Node::column().w_px(actual_width.get()).h_px(50.0).at(x, 10.0);
        ui.column(node, |ui| {
            ui.view_of("transport", &0, |ui| {
                builds.set(builds.get() + 1);
                let _ = ui.label_at("title".into(), Rect::new(4.0, 6.0, 80.0, 20.0), "Playback", Label::BODY.line_height(20.0)).unwrap();
                ui.retain_view_on_resize();
            });
        });
    };
    ui.frame(&FrameIn::new(320.0, 240.0), &mut build).unwrap();
    let first = builds.get();
    ui.frame(&FrameIn::new(400.0, 280.0), &mut build).unwrap();
    assert_eq!(builds.get(), first);
    assert_eq!(crate::testing::find(&ui, Role::Label, "Playback"), Some(Rect::new(304.0, 16.0, 80.0, 20.0)));
    actual_width.set(120.0);
    ui.frame(&FrameIn::new(420.0, 300.0), &mut build).unwrap();
    assert!(builds.get() > first, "a change to the view's own size rebuilds");
    let before = builds.get();
    ui.frame(&FrameIn::new(420.0, 300.0).scale(2.0), &mut build).unwrap();
    assert!(builds.get() > before, "a density change rebuilds");
}

/// A handle's theme record (`PxuiUi.theme`, read by `Ui::theme` after the handle's first read) is the theme in effect for it: each
/// `themed` scope's handle sees its own, the outer handle reads its own again after scopes nested deeper than any before (the engine's
/// stack grew under it), and the owner's follows a theme swap between frames.
#[test]
fn a_handles_theme_record_is_its_scope_theme_through_deeper_scopes_and_swaps() {
    use crate::{Rgba, ThemeHandle};
    let fill = |n: u8| ThemeHandle::parse(&format!("button.idle.fill = #{n:02x}0000ff")).unwrap();
    let scopes: Vec<ThemeHandle> = (1..=12).map(fill).collect();
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.set_theme(&fill(0x80)).unwrap();
    assert_eq!(ui.theme().button.idle.fill, Rgba::rgb(0x80, 0, 0));
    fn nest(ui: &mut Ui, scopes: &[ThemeHandle], depth: usize) {
        if depth == scopes.len() { return; }
        ui.themed(&scopes[depth], &mut |ui| {
            let want = Rgba::rgb(depth as u8 + 1, 0, 0);
            assert_eq!(ui.theme().button.idle.fill, want);
            nest(ui, scopes, depth + 1);
            assert_eq!(ui.theme().button.idle.fill, want, "scope {depth} after deeper scopes");
        });
    }
    for depth in [2, scopes.len()] {
        ui.frame(&input(), &mut |ui| {
            let outer = ui.theme().button.idle.fill;
            nest(ui, &scopes[..depth], 0);
            assert_eq!(ui.theme().button.idle.fill, outer);
        }).unwrap();
    }
    ui.set_theme(&fill(0x40)).unwrap();
    assert_eq!(ui.theme().button.idle.fill, Rgba::rgb(0x40, 0, 0));
}

#[test]
fn authored_scope_palettes_inherit_and_invalidate_retained_views_without_warm_allocations() {
    use crate::{Rgba, ThemeHandle, theme::ColorToken};
    use std::cell::Cell;

    let token = ColorToken(0);
    let with_palette = |color| {
        let mut theme = ThemeHandle::parse("button.hover.fill = #54ff0000").unwrap();
        theme.set_color(token, color);
        theme
    };
    let root_ink = Rgba::rgb(10, 20, 30);
    let root = with_palette(root_ink);
    let (red, blue) = (Rgba::rgb(255, 0, 0), Rgba::rgb(0, 0, 255));
    let (left, right) = (with_palette(red), with_palette(blue));
    let inherited = ThemeHandle::default();
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let seen = Cell::new(Rgba::default());
    mem_sys::set_peer(crate::library_counter);
    for round in 0..6 {
        let (scope, ink) = if round % 2 == 0 { (&left, red) } else { (&right, blue) };
        mem_sys::local_reset();
        ui.set_theme(&root).unwrap();
        ui.frame(&input().reduced_motion(true), &mut |ui| {
            ui.themed(scope, &mut |ui| {
                assert_eq!(ui.theme().button.hover.fill, ink);
                assert!(ui.tokens().reduced_motion);
                ui.themed(&inherited, &mut |ui| {
                    ui.view_of("sample", &1, |ui| seen.set(ui.color(token)));
                });
            });
            assert_eq!(ui.color(token), root_ink);
        }).unwrap();
        assert_eq!(seen.get(), ink);
        if round >= 2 { assert_eq!(mem_sys::local_alloc_count(), 0); }
    }
}

thread_local! { static TOKEN_BODY_INK: std::cell::Cell<pxui_types::geom::Rgba> = const { std::cell::Cell::new(pxui_types::geom::Rgba::rgba(0, 0, 0, 0)) }; }

/// A registered icon button body that records the ink (its states' text colour) it was given.
fn token_body(_ui: &mut Ui, c: &crate::bodies::IconButtonConfig) {
    TOKEN_BODY_INK.with(|s| s.set(c.ink));
}

#[test]
fn const_looks_naming_colour_tokens_resolve_for_the_theme_in_effect_without_app_rebuild() {
    use std::cell::Cell;
    use pxui_types::{geom::Rgba, theme::{ColorToken, States}, Look};
    const PAPER: ColorToken = ColorToken(0);
    const INK: ColorToken = ColorToken(1);
    const ROW: Look = Look::NONE.fill(Rgba::token(PAPER)).text(Rgba::token(INK)).border(Rgba::rgb(1, 2, 3), 1.0);
    const PLATE: States = States::of(ROW);
    const KEY: crate::IconButton = crate::IconButton::sized(36.0, 36.0).look(&PLATE);
    mem_sys::set_peer(crate::library_counter);
    let c = |v: u8| Rgba::rgb(v, v, v);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.register_icon_button_body("token_body", token_body).unwrap();
    let mut light = crate::ThemeHandle::parse("tokens.panel = #f4f5f0\nicon_button.body = token_body").unwrap();
    let mut dark = crate::ThemeHandle::parse("tokens.dark = true\ntokens.panel = #101010\nicon_button.body = token_body").unwrap();
    light.set_color(PAPER, c(240));
    light.set_color(INK, c(30));
    dark.set_color(PAPER, c(16));
    dark.set_color(INK, c(220));
    ui.set_theme(&light).unwrap();
    let (node, pressed, button) = (Cell::new(Look::NONE), Cell::new(Look::NONE), Cell::new(Look::NONE));
    // a retained view whose revision never changes: the engine rebuilds it at the swap, the app changes nothing
    let mut build = |ui: &mut Ui| {
        ui.column(Node::column().w_px(200.0).h_px(100.0), |ui| { ui.view_of("tokens", &1, |ui| {
            let b = ui.node("plain", crate::Node::DEFAULT.w_px(20.0).h_px(10.0).look(ROW));
            ui.node_mut(&b, &mut |f| node.set(f.look));
            let p = ui.pressable("row", crate::Node::DEFAULT.w_px(20.0).h_px(10.0), &crate::Pressable::DEFAULT.look(&PLATE), |_, _| {});
            ui.node_mut(&p.handle, &mut |f| pressed.set(f.look));
            ui.icon_button("key", crate::Icon::glyph('>'), "Key", &KEY);
            let b = ui.button("button", "Button", &crate::Button::DEFAULT.look(&PLATE));
            ui.node_mut(&b.handle, &mut |f| button.set(f.look));
        }); });
    };
    for round in 0..4 {
        for (theme, paper, ink) in [(&light, c(240), c(30)), (&dark, c(16), c(220))] {
            mem_sys::local_reset();
            ui.set_theme(theme).unwrap();
            ui.frame(&input(), &mut build).unwrap();
            assert_eq!((node.get().fill, node.get().text, node.get().border), (paper, ink, Rgba::rgb(1, 2, 3)), "a node's const look");
            assert_eq!((pressed.get().fill, pressed.get().text), (paper, ink), "a pressable's const look");
            assert_eq!((button.get().fill, button.get().text), (paper, ink), "the built-in button body applies resolved states");
            assert_eq!(TOKEN_BODY_INK.with(|s| s.get()), ink, "a registered body is given resolved states and ink");
            if round >= 2 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm swaps and resolution allocate nothing"); }
        }
    }
}

/// U2b.7 (D19, N11): a token at a share of its strength (`Rgba::alpha`) resolves to its entry at that opacity, the 1.61 marker
/// opacity stays opaque, and `ColorToken::ACCENT` follows the theme's accent across a theme swap; warm swaps allocate nothing.
#[test]
fn token_alpha_and_the_accent_token_resolve_for_the_theme_in_effect() {
    use std::cell::Cell;
    use pxui_types::{geom::Rgba, theme::ColorToken, Look};
    const WASH: Look = Look::NONE.fill(Rgba::token(ColorToken(2)).alpha(153)).text(Rgba::token(ColorToken::ACCENT)).border(Rgba::token(ColorToken(2)), 1.0);
    assert_eq!((Rgba::token(ColorToken(2)).g, Rgba::token(ColorToken(2)).alpha(0x4b).g, Rgba::token(ColorToken(2)).alpha(0).g, Rgba::rgb(1, 2, 3).alpha(9).a), (255, 0x4b, 0, 9), "a token is opaque, every opacity is itself (0x4b included); a plain colour takes the alpha");
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let mut light = crate::ThemeHandle::parse("tokens.accent = #ff0000").unwrap();
    let mut dark = crate::ThemeHandle::parse("tokens.dark = true\ntokens.panel = #101010\ntokens.accent = #0000ff").unwrap();
    light.set_color(ColorToken(2), Rgba::rgb(200, 100, 0));
    dark.set_color(ColorToken(2), Rgba::rgb(0, 100, 200));
    let seen = Cell::new(Look::NONE);
    let mut build = |ui: &mut Ui| {
        let b = ui.node("wash", crate::Node::DEFAULT.w_px(20.0).h_px(10.0).look(WASH));
        ui.node_mut(&b, &mut |f| seen.set(f.look));
    };
    for round in 0..4 {
        for (theme, entry, accent) in [(&light, Rgba::rgb(200, 100, 0), Rgba::rgb(255, 0, 0)), (&dark, Rgba::rgb(0, 100, 200), Rgba::rgb(0, 0, 255))] {
            mem_sys::local_reset();
            ui.set_theme(theme).unwrap();
            ui.frame(&input(), &mut build).unwrap();
            let l = seen.get();
            assert_eq!((l.fill, l.text, l.border), ({ let mut record = entry; record.a = 153; record }, accent, entry), "60% of the entry, the accent, the marker opaque");
            if round >= 2 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm swaps allocate nothing"); }
        }
    }
}

thread_local! { static SWATCH_SEEN: std::cell::Cell<pxui_types::geom::Rgba> = const { std::cell::Cell::new(pxui_types::geom::Rgba::rgba(0, 0, 0, 0)) }; }

/// A registered context-menu row body that records the swatch it was given.
fn swatch_row(_ui: &mut Ui, c: &crate::bodies::MenuRowConfig) {
    if c.has_swatch != 0 { SWATCH_SEEN.with(|s| s.set(c.swatch)); }
}

#[test]
fn a_registered_menu_row_body_is_given_a_swatch_token_resolved_for_the_theme() {
    use pxui_types::{geom::Rgba, theme::ColorToken, ui::{Menu, MenuItem}};
    const ITEMS: [MenuItem; 1] = [MenuItem::new("Accent").swatch(Rgba::token(ColorToken(0)))];
    let c = |v: u8| Rgba::rgb(v, v, v);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.register_menu_row_body("swatch_row", swatch_row).unwrap();
    let mut light = crate::ThemeHandle::parse("tokens.panel = #f4f5f0\nmenu_row.body = swatch_row").unwrap();
    let mut dark = crate::ThemeHandle::parse("tokens.dark = true\ntokens.panel = #101010\nmenu_row.body = swatch_row").unwrap();
    light.set_color(ColorToken(0), c(240));
    dark.set_color(ColorToken(0), c(16));
    for (theme, want) in [(&light, c(240)), (&dark, c(16)), (&light, c(240))] {
        ui.set_theme(theme).unwrap();
        ui.frame(&input(), &mut |ui| { ui.menu("m", crate::Point::new(10.0, 10.0), &ITEMS, &Menu::DEFAULT); }).unwrap();
        assert_eq!(SWATCH_SEEN.with(|s| s.get()), want);
    }
}

/// `resolve` is what the label shapes with: `Label::BODY` is exactly the default C record, unset fields take the theme label's,
/// tracking becomes points, and an invalid style is refused by `text_line_box` without failing the `Ui`.
#[test]
fn resolve_gives_the_style_a_label_shapes_with() {
    let mut ui=Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    use pxui_types::ffi::PxuiTextStyle;
    let body=ui.resolve(&crate::Label::BODY).0;
    let plain=PxuiTextStyle::default();
    assert_eq!((body.font,body.weight,body.size_points.to_bits(),body.line_height.to_bits(),body.wrap,body.align,body.max_lines,body.ellipsis,body.features,body.tracking.to_bits()),
        (plain.font,plain.weight,plain.size_points.to_bits(),plain.line_height.to_bits(),plain.wrap,plain.align,plain.max_lines,plain.ellipsis,plain.features,plain.tracking.to_bits()));
    let tracked=(ui.resolve(&crate::Label::BODY.size(18.0).tracking(-0.04).weight(500).line_height(25.2).wrap_spaces().lines(2).ellipsis())).0;
    assert_eq!((tracked.size_points,tracked.tracking,tracked.weight,tracked.line_height,tracked.wrap,tracked.max_lines,tracked.ellipsis),
        (18.0,-0.04f32*18.0,500,25.2,(crate::Wrap::WordSpaces as u32),2,(crate::Ellipsis::End as u8)));
    let theme=ui.resolve(&crate::Label::DEFAULT).0;
    assert!(theme.size_points>0.0,"the theme label's size");
    ui.frame(&input(),&mut |ui| {
        let node=ui.node("n",crate::Node::DEFAULT);
        assert!(ui.text_line_box(&node,"x",crate::Label::BODY.size(-1.0),&[]).is_err());
        ui.text_line_box(&node,"x",crate::Label::BODY.line_height(20.0),&[]).unwrap();
    }).unwrap();
}

/// U1b.1: pick-one items keyed by id follow a reorder and never meet a positional item; a click chooses and a secondary press
/// reports its item; the slider commits on release with an equal preview; overlays report their roots and a dismissal; the menu
/// bar reports the flat index, the item id and a dismissal; the split reports its ratio while held and on release. Warm frames
/// of all of these allocate nothing.
#[test]
fn u1b1_results_follow_item_ids_report_phases_roots_and_menus_without_warm_allocations() {
    use crate::testing;
    use crate::{Event, Segment, ui::{MenuItem, Role}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    const A: Segment = Segment::new("Alpha").id(1);
    const B: Segment = Segment::new("Beta").id(2);
    const POSITIONAL: [Segment; 3] = [Segment::new("P"), Segment::new("Q"), Segment::new("R").id(1)];
    const FILE: [MenuItem; 2] = [MenuItem::new("Open").id(10), MenuItem::new("Save").id(11)];
    const MENUS: [crate::MenuBarMenu; 1] = [crate::MenuBarMenu::new("File", &FILE)];
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.set_a11y(true);
    let (flip, popup_open) = (Cell::new(false), Cell::new(true));
    let (pick, seg, slide) = (Cell::new(None::<crate::PickResponse>), Cell::new(None::<crate::PickResponse>), Cell::new(None::<crate::SliderResponse>));
    let (popup, panel, bar, split) = (Cell::new(None::<crate::OverlayResponse>), Cell::new(None::<crate::OverlayResponse>), Cell::new(None::<crate::Chosen>), Cell::new(None::<crate::SplitResponse>));
    let mut build = |ui: &mut Ui| {
        bar.set(Some(ui.menu_bar("bar", &MENUS, &crate::MenuBar::DEFAULT)));
        let items = if flip.get() { [B, A] } else { [A, B] };
        pick.set(Some(ui.radio("mode", &items, Some(0), &crate::Radio::DEFAULT.name("Mode"))));
        seg.set(Some(ui.segmented("seg", &POSITIONAL, None, &crate::Segmented::DEFAULT)));
        slide.set(Some(ui.slider("vol", "Volume", 0.5, 0.0..=1.0, &crate::Slider::DEFAULT)));
        split.set(Some(ui.split("split", 0.5, &crate::Split::DEFAULT, &mut |_| {}, &mut |_| {})));
        panel.set(Some(ui.panel("p", &crate::Overlay::DEFAULT.at(crate::Point::new(200.0, 150.0)).w_px(100.0), |_| {})));
        if popup_open.get() { popup.set(Some(ui.popup("pop", &crate::Overlay::DEFAULT.at(crate::Point::new(250.0, 10.0)).w_px(60.0).h_px(20.0), |_| {}))); }
    };
    let id_of = |ui: &Ui, name: &str| ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == name).map(|n| n.id).expect(name);
    for _ in 0..2 { testing::frame(&mut ui, &[], &mut build); }
    let (beta, q, r) = (id_of(&ui, "Beta"), id_of(&ui, "Q"), id_of(&ui, "R"));
    assert_ne!(q, r, "explicit id 1 and the item at index 1 are two identities");
    flip.set(true);
    testing::frame(&mut ui, &[], &mut build);
    assert_eq!(id_of(&ui, "Beta"), beta, "an item keyed by id keeps its node through a reorder");
    // overlays: the popup's and the panel's roots are live; Escape dismisses the popup with the empty handle
    let (open, root) = (popup.get().unwrap(), panel.get().unwrap().handle);
    assert!(open.open && open.handle.box_key != 0 && panel.get().unwrap().open);
    assert!(ui.rect(&open.handle).w > 0.0 && ui.rect(&root).w > 0.0);
    testing::key(&mut ui, crate::KeyCode::Escape, &mut build);
    assert_eq!((popup.get().unwrap().open, popup.get().unwrap().handle.box_key), (false, 0));
    popup_open.set(false);
    testing::frame(&mut ui, &[], &mut build);
    // a click on Beta (now first) keeps it; Alpha (second) is chosen and changes the choice
    let alpha = testing::find(&ui, Role::Radio, "Alpha").unwrap();
    testing::click(&mut ui, f64::from(alpha.x + 4.0), f64::from(alpha.y + 4.0), &mut build);
    let p = pick.get().unwrap();
    assert_eq!((p.chosen, p.response.changed, p.secondary), (Some(1), true, None));
    let at = |kind, x: f32, y: f32, code| Event::pointer(kind, code, f64::from(x), f64::from(y));
    testing::frame(&mut ui, &[at(Event::POINTER_PRESSED, alpha.x + 4.0, alpha.y + 4.0, 1)], &mut build);
    assert_eq!(pick.get().unwrap().secondary, Some(1), "a secondary press reports its item and never chooses");
    testing::frame(&mut ui, &[at(Event::POINTER_RELEASED, alpha.x + 4.0, alpha.y + 4.0, 1)], &mut build);
    // the slider: a held drag previews, the release commits with an equal preview, the next frame has neither
    let track = slide.get().unwrap().response.handle;
    let t = ui.rect(&track);
    let (x, y) = (t.x + t.w * 0.75, t.y + t.h / 2.0);
    testing::frame(&mut ui, &[at(Event::POINTER_PRESSED, x, y, 0)], &mut build);
    let s = slide.get().unwrap();
    assert!(s.preview.is_some() && s.commit.is_none() && s.response.held && Some(s.value) == s.preview);
    testing::frame(&mut ui, &[at(Event::POINTER_RELEASED, x, y, 0)], &mut build);
    let s = slide.get().unwrap();
    assert!(s.commit.is_some() && s.preview == s.commit && s.response.changed);
    testing::frame(&mut ui, &[], &mut build);
    assert_eq!((slide.get().unwrap().preview, slide.get().unwrap().commit), (None, None));
    // the split: no ratio at rest; a held grip and its release report one
    assert_eq!(split.get().unwrap().ratio, None);
    let grip = ui.rect(&split.get().unwrap().response.handle);
    let (gx, gy) = (grip.x + grip.w / 2.0, grip.y + grip.h / 2.0);
    testing::frame(&mut ui, &[at(Event::POINTER_PRESSED, gx, gy, 0)], &mut build);
    assert!(split.get().unwrap().ratio.is_some() && split.get().unwrap().response.held, "a held grip reports its ratio even before it moves");
    testing::frame(&mut ui, &[at(Event::POINTER_MOVED, gx + 20.0, gy, 0), at(Event::POINTER_RELEASED, gx + 20.0, gy, 0)], &mut build);
    assert!(split.get().unwrap().ratio.is_some_and(|r| r > 0.5), "the release frame reports the final ratio");
    testing::frame(&mut ui, &[], &mut build);
    assert_eq!(split.get().unwrap().ratio, None);
    // the menu bar: open File, pick Save (flat index 1, id 11); open again and a press outside is a dismissal
    let file = testing::find(&ui, Role::MenuItem, "File").unwrap();
    testing::click(&mut ui, f64::from(file.x + 4.0), f64::from(file.y + 4.0), &mut build);
    let save = testing::find(&ui, Role::MenuItem, "Save").unwrap();
    testing::click(&mut ui, f64::from(save.x + 4.0), f64::from(save.y + 4.0), &mut build);
    let c = bar.get().unwrap();
    assert_eq!((c.chosen, c.chosen_id, c.dismissed), (Some(1), Some(11), false));
    testing::click(&mut ui, f64::from(file.x + 4.0), f64::from(file.y + 4.0), &mut build);
    testing::frame(&mut ui, &[at(Event::POINTER_PRESSED, 310.0, 230.0, 0)], &mut build);
    let c = bar.get().unwrap();
    assert_eq!((c.chosen, c.dismissed), (None, true));
    // warm frames, including presses and moves, allocate nothing
    testing::frame(&mut ui, &[], &mut build);
    mem_sys::local_reset();
    for (kind, x, y) in [(Event::POINTER_MOVED, x, y), (Event::POINTER_PRESSED, x, y), (Event::POINTER_MOVED, x - 10.0, y), (Event::POINTER_RELEASED, x - 10.0, y), (Event::POINTER_MOVED, gx, gy)] {
        testing::frame(&mut ui, &[at(kind, x, y, 0)], &mut build);
    }
    assert_eq!(mem_sys::local_alloc_count(), 0, "warm U1b.1 result frames allocate nothing");
}

/// U1b.1: a tooltip's wait is keyed by its owner: one text on two boxes is two waits, a changed text keeps the owner's wait,
/// and the wait allocates nothing.
#[test]
fn a_tooltip_waits_by_its_owner_and_allocates_nothing_on_warm_frames() {
    use crate::{Event, ui::{Flags, Node, Role}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.set_a11y(true);
    let text = Cell::new("Tip");
    let mut build = |ui: &mut Ui| {
        for (key, x) in [("a", 0.0), ("b", 100.0)] {
            let b = ui.node(key, Node::DEFAULT.flags(Flags::CLICKABLE).role(Role::Button).at(x, 0.0).w(crate::ui::Size::Px(80.0)).h(crate::ui::Size::Px(30.0)));
            ui.tooltip(&b, text.get());
        }
    };
    let shown = |ui: &Ui, name: &str| ui.a11y_nodes().iter().any(|n| ui.a11y_name_of(n).contains(name) && n.role != Role::Button as u32);
    let mut time = 0.0;
    let mut rest = |ui: &mut Ui, x: f64, frames: usize, build: &mut dyn FnMut(&mut Ui)| for _ in 0..frames {
        time += 0.1;
        let moved = Event::pointer(Event::POINTER_MOVED, 0, x, 10.0);
        let mut frame = FrameIn::new(320.0, 240.0).time(time);
        frame.events = core::slice::from_ref(&moved);
        ui.frame(&frame, build).unwrap();
    };
    rest(&mut ui, 10.0, 2, &mut build);
    mem_sys::local_reset();
    rest(&mut ui, 10.0, 2, &mut build);
    assert_eq!(mem_sys::local_alloc_count(), 0, "a waiting tooltip copies no text");
    rest(&mut ui, 10.0, 3, &mut build);
    assert!(shown(&ui, "Tip"), "the owner's wait passed");
    text.set("Tip with enough words to wrap onto another line with spaces and UTF-8 café and more words still");
    rest(&mut ui, 10.0, 3, &mut build);
    assert!(shown(&ui, "café"));
    mem_sys::local_reset();
    rest(&mut ui, 10.0, 4, &mut build);
    assert_eq!(mem_sys::local_alloc_count(), 0, "a warm shown tooltip reuses its line and layout storage");
    text.set("Tip");
    rest(&mut ui, 110.0, 1, &mut build);
    assert!(!shown(&ui, "Tip"), "another owner with the same text waits again");
    rest(&mut ui, 110.0, 5, &mut build);
    text.set("Tip two");
    rest(&mut ui, 110.0, 1, &mut build);
    assert!(shown(&ui, "Tip two"), "a changed text keeps the owner's wait");
}

#[test]
fn a_text_area_description_restarts_its_hover_wait_at_a_new_token() {
    use crate::{AreaCallbacks, Event, Language, Node, Role, TextArea};
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.set_a11y(true);
    let start = std::cell::Cell::new(0);
    let description = std::cell::Cell::new("Token documentation");
    let describe = |_: &str, _: usize, out: &mut String| { out.push_str(description.get()); Some((start.get(), start.get() + 1)) };
    let callbacks = AreaCallbacks::DEFAULT.language(Language::DEFAULT.describe(&describe));
    let mut build = |ui: &mut Ui| {
        ui.text_area("area", "alpha beta", Node::DEFAULT.at(0.0, 0.0).w_px(160.0).h_px(80.0), &TextArea::DEFAULT, &callbacks);
    };
    let moved = Event::pointer(Event::POINTER_MOVED, 0, 46.0, 10.0);
    let mut rest = |ui: &mut Ui, tick: u32| {
        ui.frame(&input().time(f64::from(tick) * 0.15).events(core::slice::from_ref(&moved)), &mut build).unwrap();
    };
    let shown = |ui: &Ui| ui.a11y_nodes().iter().any(|n| n.is_role(Role::Tooltip) && ui.a11y_name_of(n) == description.get());
    for tick in 0..8 { rest(&mut ui, tick); }
    assert!(shown(&ui));
    mem_sys::local_reset();
    let scope = mem_sys::warm_scope();
    for tick in 8..12 { rest(&mut ui, tick); }
    drop(scope);
    let count = mem_sys::local_alloc_count();
    assert_eq!(count, 0, "a warm described token reuses its buffers: {:?}", mem_sys::take_reports());
    description.set("Updated description");
    rest(&mut ui, 12);
    assert!(shown(&ui), "description changes without restarting the token wait");
    start.set(6);
    rest(&mut ui, 13);
    assert!(!shown(&ui), "a new token starts its own wait");
    for tick in 14..21 { rest(&mut ui, tick); }
    assert!(shown(&ui));
}

#[test]
fn nested_text_area_descriptions_reuse_separate_buffers() {
    use crate::{AreaCallbacks, Event, Language, Node, TextArea};
    mem_sys::set_peer(crate::library_counter);
    let mut outer = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let inner = std::cell::RefCell::new(Ui::new(&crate::UiDesc::DEFAULT).unwrap());
    let tick = std::cell::Cell::new(0);
    let inner_calls = std::cell::Cell::new(0);
    let description = |_: &str, _: usize, out: &mut String| {
        inner_calls.set(inner_calls.get() + 1);
        out.push_str("Inner");
        Some((0, 1))
    };
    let inner_callbacks = AreaCallbacks::DEFAULT.language(Language::DEFAULT.describe(&description));
    let moved = Event::pointer(Event::POINTER_MOVED, 0, 46.0, 10.0);
    let region = Node::DEFAULT.at(0.0, 0.0).w_px(160.0).h_px(80.0);
    let frame = || input().time(f64::from(tick.get()) * 0.15).events(core::slice::from_ref(&moved));
    let outer_description = |_: &str, _: usize, out: &mut String| {
        inner.borrow_mut().frame(&frame(), &mut |ui| {
            ui.text_area("inner", "beta", region, &TextArea::DEFAULT, &inner_callbacks);
        }).unwrap();
        out.push_str("Outer description");
        Some((0, 1))
    };
    let outer_callbacks = AreaCallbacks::DEFAULT.language(Language::DEFAULT.describe(&outer_description));
    let mut build = |ui: &mut Ui| {
        ui.text_area("outer", "alpha", region, &TextArea::DEFAULT, &outer_callbacks);
    };
    for index in 0..12 {
        tick.set(index);
        mem_sys::local_reset();
        outer.frame(&frame(), &mut build).unwrap();
        if index >= 8 { assert_eq!(mem_sys::local_alloc_count(), 0, "nested descriptions on warm frame {index}"); }
    }
    assert!(inner_calls.get() >= 4, "the inner description actually ran");
}

/// U1b.2 keys: a resolved key is the key itself in creation calls and lookups (never hashed again); authored lookups resolve under
/// the active scope during a build and the root seed outside one; `root_at` keeps the innermost candidate whatever the order and
/// past one 32-key chunk; `remember` writes only the keyed field's history; the hover wait and lookups allocate nothing warm.
#[test]
fn u1b2_keys_resolve_once_and_lookups_keep_the_innermost_candidate_without_warm_allocations() {
    use crate::{Event, Key, Point, ui::{Flags, Node}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let keys = Cell::new((0i64, 0i64, 0i64));
    let lookups = Cell::new((false, false, false));
    let mut build = |ui: &mut Ui| {
        let outer = ui.node("outer", Node::DEFAULT.flags(Flags::CLICKABLE).at(0.0, 0.0).w(Size::Px(200.0)).h(Size::Px(100.0)));
        ui.within(&outer, &mut |ui: &mut Ui| {
            let inner = ui.node("inner", Node::DEFAULT.flags(Flags::CLICKABLE).w(Size::Px(50.0)).h(Size::Px(50.0)));
            // an authored lookup resolves under the active scope: "inner" here is the child just built
            lookups.set((ui.box_of_key("inner").is_some_and(|b| b.box_key == inner.box_key), lookups.get().1, lookups.get().2));
            keys.set((outer.box_key, inner.box_key, keys.get().2));
        });
        // a resolved key creates a node keyed by exactly that key, and finds it again
        let again = ui.node(Key::Resolved(0x5eed_5eed), Node::DEFAULT);
        keys.set((keys.get().0, keys.get().1, again.box_key));
        lookups.set((lookups.get().0, ui.box_of_key(Key::Resolved(0x5eed_5eed)).is_some(), ui.hover_delay(Key::Resolved(outer.box_key))));
    };
    for t in 0..3 { ui.frame(&input().time(t as f64), &mut build).unwrap(); }
    let (outer, inner, again) = keys.get();
    assert_eq!(again, 0x5eed_5eed, "a resolved key is never hashed again");
    assert!(lookups.get().0 && lookups.get().1, "authored and resolved lookups find their nodes during a build");
    // outside a build, an authored key resolves under the root seed
    assert_eq!(ui.box_of_key("outer").map(|b| b.box_key), Some(outer));
    // innermost wins whatever the order, also when the winners come from different 32-key chunks
    let mut many = [Key::Resolved(1); 40];
    many[3] = Key::Resolved(outer);
    many[37] = Key::Label("inner-missing");
    many[39] = Key::Resolved(inner);
    let at = Point { x: 10.0, y: 10.0 };
    assert_eq!(ui.root_at(at, &many), Some(Key::Resolved(inner)));
    assert_eq!(ui.root_at(at, &[Key::Resolved(inner), Key::Label("outer")]), Some(Key::Resolved(inner)));
    assert_eq!(ui.root_at(at, &[Key::Label("outer")]), Some(Key::Label("outer")), "the candidate comes back as the caller wrote it");
    assert_eq!(ui.root_at(Point { x: 150.0, y: 10.0 }, &many), Some(Key::Resolved(outer)));
    // the history of a field that is not the active editor is untouched by another key's snapshot
    ui.frame(&input().time(4.0), &mut |ui: &mut Ui| {
        let _ = ui.load_text_edit(Key::Resolved(11), "a");
        let before = ui.text_undo_bytes();
        ui.remember(Key::Resolved(12), &pxui_types::ui::TextEdit { text: "zzz".into(), caret: 0, anchor: 0 }, false);
        assert_eq!(ui.text_undo_bytes(), before, "a key with no history takes no snapshot");
        ui.remember(Key::Resolved(11), &pxui_types::ui::TextEdit { text: "abc".into(), caret: 0, anchor: 0 }, false);
        assert!(ui.text_undo_bytes() > before, "the active field's snapshot is held");
        // 11 is shelved when 13 loads; a snapshot keyed 11 goes to its shelved history and 13 stays the active editor
        let _ = ui.load_text_edit(Key::Resolved(13), "b");
        let shelved = ui.text_undo_bytes();
        ui.remember(Key::Resolved(11), &pxui_types::ui::TextEdit { text: "abcd".into(), caret: 0, anchor: 0 }, false);
        assert!(ui.text_undo_bytes() > shelved, "the shelved field's history takes the snapshot");
        assert_eq!(ui.load_text_edit(Key::Resolved(13), "b").text, "b", "the active editor did not change");
    }).unwrap();
    // warm: lookups, a resolved node and the hover wait allocate nothing in the wrapper or the library
    for t in 5..9 {
        let moved = Event::pointer(Event::POINTER_MOVED, 0, 150.0, 10.0);
        let mut frame = input().time(t as f64 * 0.1 + 5.0);
        frame.events = core::slice::from_ref(&moved);
        if t == 7 { mem_sys::local_reset(); }
        ui.frame(&frame, &mut build).unwrap();
        let _ = ui.root_at(at, &many);
    }
    assert_eq!(mem_sys::local_alloc_count(), 0, "warm lookups and the resolved hover wait allocate nothing");
}

/// The reserved resolved keys 0 and 1 (the key tables' marks, never a box key) are refused, never aliased to `Id(0)`/`Id(1)`: a creation
/// call and a text-keyed lookup (`expanded`, `hover_delay`) panic at the caller as for any invalid key, a key lookup finds nothing, and
/// the authored `Id(0)` and `Id(1)` keep their own nodes and state.
#[test]
fn reserved_resolved_keys_are_refused_and_never_alias_ids() {
    use crate::{Key, Point, ui::Node};
    let refused = |f: &mut dyn FnMut()| catch_unwind(AssertUnwindSafe(f)).is_err();
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.frame(&input(), &mut |ui: &mut Ui| {
        let mut labels = [[0u8; 17]; 2];
        let [zero, one] = &mut labels;
        crate::ext::inspector_section(ui, Key::Id(0).label(zero), true, None, "Zero", &mut |_| {});
        crate::ext::inspector_section(ui, Key::Id(1).label(one), false, None, "One", &mut |_| {});
        for reserved in [Key::Resolved(0), Key::Resolved(1)] {
            assert!(refused(&mut || { ui.node(reserved, Node::DEFAULT); }), "{reserved:?} is not created");
            assert!(refused(&mut || { ui.expanded(reserved); }), "{reserved:?} is not looked up as Id(0)/Id(1)");
            assert!(refused(&mut || { ui.hover_delay(reserved); }));
            assert!(ui.box_of_key(reserved).is_none());
        }
        assert_eq!((ui.expanded(Key::Id(0)), ui.expanded(Key::Id(1))), (Some(true), Some(false)), "the authored ids keep their state");
    }).unwrap();
    assert_eq!((ui.expanded(Key::Id(0)), ui.expanded(Key::Id(1))), (Some(true), Some(false)));
    assert_eq!(ui.root_at(Point { x: 1.0, y: 1.0 }, &[Key::Resolved(0), Key::Resolved(1)]), None);
}

/// U1b.3 units: a fractional body size is kept by creation and the setter (the 1.x C getters keep whole points; the C snapshot's
/// projection is checked in pxui_abi); a painter's text service measures what `Ui::text_service` measures in the same label; painting a
/// label with spans and measuring it in a canvas allocate nothing warm.
#[test]
fn u1b3_fractional_sizes_and_the_painters_text_service_without_warm_allocations() {
    use crate::{Label, Point, TextSpan};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT.font_size(14.5)).unwrap();
    assert_eq!((ui.font_size(), ui.row_height()), (14.5, 25.5));
    ui.set_font_size(13.25);
    assert_eq!(ui.font_size(), 13.25);
    const LABEL: Label = Label::BODY.size(17.5);
    let expected = ui.text_service().measure("Painted 0123", LABEL, None).unwrap();
    let painted = std::rc::Rc::new(Cell::new((0.0f32, 0.0f32)));
    let mut build = |ui: &mut Ui| {
        let node = ui.node("canvas", crate::Node::DEFAULT.w_px(200.0).h_px(60.0));
        let painted = painted.clone();
        ui.canvas(&node, move |paint, rect| {
            let m = paint.text_service().measure("Painted 0123", LABEL, None).unwrap();
            let spans = paint.text_spans(Point::new(rect.x, rect.y), LABEL, 0.0, "Painted 0123", &[{ let mut record = TextSpan::new(0, 7); record.weight = 700; record }]).unwrap();
            painted.set((m.width, spans.height));
        });
    };
    for t in 0..12 {
        if t == 8 { mem_sys::local_reset(); }
        ui.frame(&input().time(t as f64 / 60.0), &mut build).unwrap();
    }
    assert_eq!(mem_sys::local_alloc_count(), 0, "a warm painter measure and label spans allocate nothing");
    assert_eq!(painted.get().0, expected.width, "the painter measures what the Ui measures");
    assert!(painted.get().1 > 0.0);
}

#[test]
fn scroll_views_and_text_areas_take_flow_nodes_and_allocate_nothing_when_warm() {
    use crate::{AreaCallbacks, TextArea};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let (scroll, first, second, area) = (Cell::new(None), Cell::new(None), Cell::new(None), Cell::new(None));
    let mut build = |ui: &mut Ui| {
        ui.node("top", Node::DEFAULT.h_px(20.0));
        scroll.set(Some(ui.scroll_view("strip", Node::row().pad(4.0).h_px(50.0), |ui| {
            first.set(Some(ui.node("a", Node::DEFAULT.w_px(30.0).h_px(10.0))));
            second.set(Some(ui.node("b", Node::DEFAULT.w_px(30.0).h_px(10.0))));
        })));
        area.set(Some(ui.text_area("notes", "one\ntwo", Node::DEFAULT.h_px(40.0), &TextArea::DEFAULT, &AreaCallbacks::DEFAULT).response.handle));
    };
    for time in 0..10 {
        mem_sys::local_reset();
        let scope = (time >= 8).then(mem_sys::warm_scope);
        ui.frame(&input().time(time as f64 / 60.0), &mut build).unwrap();
        let allocated = mem_sys::local_alloc_count();
        drop(scope);
        if time >= 8 { assert_eq!(allocated, 0, "warm scroll view and text area"); assert!(mem_sys::take_reports().is_empty()); }
    }
    let (s, a, b, t) = (ui.rect(&scroll.get().unwrap()), ui.rect(&first.get().unwrap()), ui.rect(&second.get().unwrap()), ui.rect(&area.get().unwrap()));
    assert_eq!((s.y, s.h), (20.0, 50.0), "the scroll view flows after the first node at its node's height");
    assert_eq!((a.x - s.x, a.y - s.y), (5.0, 5.0), "the content sits a point inside the node's padding");
    assert_eq!(b.x, a.x + 30.0, "the content flows on the node's axis");
    assert_eq!((t.y, t.w, t.h), (70.0, 320.0, 40.0), "the text area flows after the scroll view, filling the width");
}

/// U2b.8: `state`/`set_state` through a previous frame's handle of a gone node read the default and store nothing, and never reach
/// the node that took its slot (both link modes).
#[test]
fn state_through_a_gone_handle_is_ignored() {
    use std::cell::Cell;
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    let gone = Cell::new(None);
    ui.frame(&input(), &mut |ui| { let a = ui.node("a", crate::Node::DEFAULT); ui.set_state(&a, 5); gone.set(Some(a)); }).unwrap();
    ui.frame(&input(), &mut |_| {}).unwrap();
    let gone = gone.get().unwrap();
    for k in 0..4 {
        ui.frame(&input(), &mut |ui| {
            let b = ui.node(crate::Key::Id(k), crate::Node::DEFAULT);
            assert_eq!(ui.state(&gone, -1), -1, "a gone node's handle reads the default");
            ui.set_state(&gone, 9);
            assert_eq!(ui.state(&b, 0), 0, "the stale write reached the node that took the slot");
        }).unwrap();
    }
}

thread_local! { static TIP_LINES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) }; }

/// A registered tooltip body that records the lines it reads through `text_at` (one past the end too).
fn tip_lines(_ui: &mut Ui, c: &crate::bodies::TooltipConfig) {
    use crate::bodies::BodyConfig;
    TIP_LINES.with(|l| { let mut l = l.borrow_mut(); l.clear(); for i in 0..=c.lines.len { l.push(c.text_at(&c.lines, i).to_owned()); } });
}

/// U2b.8: a registered tooltip body reads the tip's lines through `text_at` (both link modes): the line, then "" past the end.
#[test]
fn tooltip_body_reads_text_at() {
    use crate::{Event, ui::{Flags, Node, Role}};
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    ui.register_tooltip_body("lines", tip_lines).unwrap();
    ui.set_theme(&crate::ThemeHandle::parse("tooltip.body = lines").unwrap()).unwrap();
    let mut build = |ui: &mut Ui| {
        let b = ui.node("owner", Node::DEFAULT.flags(Flags::CLICKABLE).role(Role::Button).w(crate::ui::Size::Px(80.0)).h(crate::ui::Size::Px(30.0)));
        ui.tooltip(&b, "Reveal in Finder");
    };
    for k in 0..8 {
        let moved = Event::pointer(Event::POINTER_MOVED, 0, 10.0, 10.0);
        let mut frame = FrameIn::new(320.0, 240.0).time(0.2 * k as f64);
        frame.events = core::slice::from_ref(&moved);
        ui.frame(&frame, &mut build).unwrap();
    }
    assert_eq!(TIP_LINES.with(|l| l.borrow().clone()), ["Reveal in Finder", ""], "the body read the line, and none past the end");
}

/// U2b.4.5: under a theme that does not move, a node's own transition (`Node::transition`, or one `set_node_layout` carries) runs a
/// `style` state change and keeps it, then idles; an unchanged node starts nothing and an undeclared node changes at once. Warm frames
/// allocate nothing.
#[test]
fn a_nodes_own_transition_runs_under_a_still_theme_and_survives_style_and_layout() {
    use pxui_types::{node::Transition, theme::{Look, State, States}};
    const FADE: Transition = Transition::Bezier { duration: 0.14, points: [0.0, 0.0, 1.0, 1.0] };
    const LOOKS: States = States::of(Look::NONE.fill(crate::Rgba::rgb(10, 10, 10))).hover(Look::NONE.fill(crate::Rgba::rgb(200, 200, 200)));
    const LIT: [u8; 4] = [200, 200, 200, 255];
    let hovered = core::cell::Cell::new(false);
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    assert!(!ui.tokens().moves(), "the kit theme does not move");
    let mut build = |ui: &mut Ui| {
        let state = State::default().hovered(hovered.get());
        let own = ui.node("own", Node::DEFAULT.w_px(40.0).h_px(40.0).transition(Some(FADE)));
        ui.style(&own, &LOOKS, state);
        let laid = ui.node("laid", Node::DEFAULT.w_px(40.0).h_px(40.0));
        ui.set_node_layout(&laid, &Node::DEFAULT.w_px(40.0).h_px(40.0).transition(Some(FADE)));
        ui.style(&laid, &LOOKS, state);
        let plain = ui.node("plain", Node::DEFAULT.w_px(40.0).h_px(40.0));
        ui.style(&plain, &LOOKS, state);
    };
    let lit = |ui: &Ui| ui.layers().flat_map(|l| l.instances.iter()).filter(|i| i.color == LIT).count();
    for t in [0.0, 0.5] {
        ui.frame(&input().time(t), &mut build).unwrap();
        assert!(ui.repaint_at().is_none(), "an unchanged declared node starts nothing ({t})");
    }
    hovered.set(true);
    ui.frame(&input().time(1.0), &mut build).unwrap();
    assert_eq!(lit(&ui), 1, "only the undeclared node changes at once");
    assert!(ui.repaint_at().is_some(), "the two declared nodes fade");
    ui.frame(&input().time(1.07), &mut build).unwrap();
    assert_eq!(lit(&ui), 1, "half way through the 140 ms fade");
    ui.frame(&input().time(1.2), &mut build).unwrap();
    assert_eq!(lit(&ui), 3, "the fades land");
    assert!(ui.repaint_at().is_none(), "and idle");
    mem_sys::set_peer(crate::library_counter);
    let scope = mem_sys::warm_scope();
    for (round, t) in [1.3, 1.4, 1.5].into_iter().enumerate() {
        hovered.set(round % 2 == 0);
        mem_sys::local_reset();
        ui.frame(&input().time(t), &mut build).unwrap();
        assert_eq!(mem_sys::local_alloc_count(), 0, "a warm declared-transition frame allocated (round {round})");
    }
    drop(scope);
    assert!(mem_sys::take_reports().is_empty());
}


#[test]
fn frame_reasons_reports_the_selected_finite_keyframe() {
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).unwrap();
    assert!(!ui.step(0.0));
    let mut key = 0;
    ui.frame(&FrameIn::new(60.0, 60.0), &mut |ui| {
        let box_ = ui.node("animated", crate::Node::DEFAULT.w_px(20.0).h_px(20.0)
            .look(crate::ui::Look::NONE.fill(crate::Rgba::WHITE)));
        key = box_.box_key;
        ui.animate_keys(&box_, &[
            crate::ui::Keyframe::opacity(0.25, 0.5).curve(crate::ui::Curve::Steps { duration: 0.25, count: 2 }),
        ], &crate::Keyframes::DEFAULT);
    }).unwrap();
    assert!(ui.step(0.01));
    let reasons = ui.frame_reasons();
    if std::env::var_os("PXUI_FRAME_REASONS").is_some_and(|v| v == "1") {
        assert_eq!((reasons.step_state, reasons.stepped), (1, 1));
        assert!(reasons.finite_keys > 0);
        assert_eq!(reasons.transition_key, key);
        assert_eq!(reasons.transition_curve.count, 2);
        assert_eq!(reasons.transition_curve.duration, 0.25);
    } else {
        assert_eq!((reasons.step_state, reasons.stepped, reasons.finite_keys), (0, 0, 0));
    }
}
