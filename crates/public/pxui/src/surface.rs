//! The engine-shaped surface through the wrapper: queries, boxes, patches, scroll and text state, carries, view calls
//! and the loud failure of a caller bug.
use crate::Ui;
use crate::opts::BoxOpts;
use pxui_types::{geom::Rect, ui::{Cursor, Drop, UndoKind, CLICKABLE, CLIP, SCROLL}, FrameIn, Mark, Size};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn input() -> FrameIn { FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() } }

#[test]
fn prepared_text_moves_between_worker_and_ui_without_shaping_or_allocating_on_install() {
    use crate::{PreparedText,RetiredText,TextStyle,TextSpan};
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    let mut worker=ui.text_service().fork().unwrap();
    let style=TextStyle::default();
    let spans=[TextSpan {start:0,end:1,weight:700,..Default::default()}];
    let records=std::thread::spawn(move || {
        let (width,height,pixels)=worker.rasterize_rgba("forked",&style,200.0,1.0,[255;4]).unwrap();
        assert!(width>0 && height>0 && pixels.chunks_exact(4).any(|pixel|pixel[3]>0),"a fork retains standalone rasterization");
        (0..64).map(|index| {
            let text=format!("Track {index:05}");
            let record=worker.prepare(&text,&style,200.0,&spans).unwrap();
            (text,record)
        }).collect::<Vec<_>>()
    }).join().unwrap();
    let mut other=Ui::new().unwrap();
    let _=other.text_service().fork().unwrap();
    let mut records=records.into_iter();
    let (first_text,first)=records.next().unwrap();
    let rejected=other.text_service().install(first);
    let (code,first)=match rejected {Err(error)=>error,Ok(_)=>panic!("foreign fonts accepted")};
    assert_eq!(code,pxui_types::PXUI_ERR_STATE);
    let records=std::iter::once((first_text,first)).chain(records).collect::<Vec<_>>();
    let mut retired:Vec<RetiredText>=Vec::with_capacity(records.len());
    let layouts=ui.text_layout_count();
    mem_sys::local_reset();
    for (text,record) in records {
        let expected=record.metrics();
        let storage=match ui.text_service().install(record) {Ok(storage)=>storage,Err((code,_))=>panic!("installation failed: {code}")};
        retired.push(storage);
        let actual=ui.text_service().measure_spans(&text,&style,200.0,&spans).unwrap();
        assert_eq!((actual.width,actual.height,actual.baseline,actual.lines),(expected.width,expected.height,expected.baseline,expected.lines));
    }
    assert_eq!(ui.text_layout_count(),layouts);
    assert_eq!(mem_sys::local_alloc_count(),0);
    ui.frame(&input(),&mut |ui| {
        let node=ui.node("prepared-label",crate::Node {w:Size::Px(200.0),h:Size::Px(30.0),..crate::Node::DEFAULT});
        ui.text_spans(&node,"Track 00000",&style,&spans).unwrap();
    }).unwrap();
    assert_eq!(ui.text_layout_count(),layouts,"ordinary node drawing must use the prepared content key");
    std::thread::spawn(move || drop(retired)).join().unwrap();
    let mut worker=ui.text_service().fork().unwrap();
    let stale:PreparedText=std::thread::spawn(move || worker.prepare("stale",&style,200.0,&[]).unwrap()).join().unwrap();
    assert!(ui.text_service().add_font(crate::text::pragmasevka()).is_some());
    let (code,stale)=match ui.text_service().install(stale) {Err(error)=>error,Ok(_)=>panic!("stale fonts accepted")};
    assert_eq!(code,pxui_types::PXUI_ERR_STATE);
    std::thread::spawn(move || drop(stale)).join().unwrap();
}

#[cfg(any(target_os="macos",target_os="ios"))]
#[test]
fn phase_cpu_timing_is_opt_in_and_zero_warm_allocation() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    assert_eq!(ui.phase_cpu_times(), Err(pxui_types::PXUI_ERR_STATE));
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
    assert_eq!(ui.phase_cpu_times(), Err(pxui_types::PXUI_ERR_STATE));
    ui.frame(&input(), &mut |_| {}).unwrap();
    assert_eq!(ui.phase_cpu_times(), Err(pxui_types::PXUI_ERR_STATE));
}

#[test]
fn text_layout_count_distinguishes_cold_content_from_retained_warm_frames_without_allocations() {
    use crate::ui::Node;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    assert_eq!(ui.text_layout_count(),0);
    let mut build=|ui:&mut Ui| {
        let node=ui.node("label",Node {w:Size::Px(200.0),h:Size::Px(30.0),..Node::DEFAULT});
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
        let node=ui.node("label",Node {w:Size::Px(200.0),h:Size::Px(30.0),..Node::DEFAULT});
        ui.text(&node,"Replacement identity");
    }).unwrap();
    assert!(ui.text_layout_count()>first);
}

#[test]
fn framework_hash_preserves_caller_identities_borrows_once_and_allocates_nothing() {
    use std::hash::{Hash,Hasher};
    use crate::geom::hash::FastHasher;
    for text in ["", "café", "a longer library identity with more than eight bytes"] {
        let value=(42u64,true,Some(text));
        let mut old=FastHasher::default();
        value.hash(&mut old);
        let expected=old.finish();
        let mut calls=0;
        assert_eq!(crate::hash(|state|{calls+=1;value.hash(state)}),expected);
        assert_eq!(calls,1);
        let owned=text.to_owned();
        assert_eq!(crate::hash(|state|(&owned).hash(state)),crate::hash(|state|(&text).hash(state)));
        assert_eq!(owned,text,"the expression stays borrowed");
        mem_sys::local_reset();
        for _ in 0..100 {assert_eq!(crate::hash(|state|value.hash(state)),expected);}
        assert_eq!(mem_sys::local_alloc_count(),0);
    }
}

#[test]
fn completed_file_drop_events_preserve_paths_positions_errors_and_empty_warm_storage() {
    use crate::{Event,Str};
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    for (code,payload) in [(0,"/Music/名前.flac\0/Music/Björk.m4a"),(1,"transport limit exceeded"),(0,"")] {
        let event=Event {kind:Event::FILES_DROPPED,code,x:-12.5,y:31.25,text:Str {ptr:payload.as_ptr(),len:payload.len()},..Default::default()};
        ui.frame(&FrameIn {events:&event,event_count:1,..input()},&mut |_| {}).unwrap();
        let routed=ui.routed_input(None);
        assert_eq!(routed.events().len(),1);
        let event=&routed.events()[0];
        assert_eq!((event.kind,event.code,event.x,event.y),(Event::FILES_DROPPED,code,-12.5,31.25));
        assert_eq!(crate::event_text(event),payload);
    }
    let bad_utf8=[255u8];
    for invalid in [
        Event {kind:Event::FILES_DROPPED,code:2,..Default::default()},
        Event {kind:Event::FILES_DROPPED,x:f64::NAN,..Default::default()},
        Event {kind:Event::FILES_DROPPED,y:f64::INFINITY,..Default::default()},
        Event {kind:Event::FILES_DROPPED,text:Str {ptr:core::ptr::null(),len:1},..Default::default()},
        Event {kind:Event::FILES_DROPPED,text:Str {ptr:bad_utf8.as_ptr(),len:1},..Default::default()},
    ] {
        let mut built=false;
        assert_eq!(ui.frame(&FrameIn {events:&invalid,event_count:1,..input()},&mut |_| built=true).err(),Some(pxui_types::PXUI_ERR_ARGUMENT));
        assert!(!built);
    }
    let empty=Event {kind:Event::FILES_DROPPED,..Default::default()};
    for round in 0..20 {
        mem_sys::local_reset();
        ui.frame(&FrameIn {events:&empty,event_count:1,..input()},&mut |_| {}).unwrap();
        assert_eq!(ui.routed_input(None).events().len(),1);
        if round>2 {assert_eq!(mem_sys::local_alloc_count(),0);}
    }
}

#[test]
fn reveal_uses_fixed_measured_section_and_grid_geometry_without_warm_allocations() {
    use crate::ui::{Node, SectionList, SectionPart, Look};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    for kind in 0..4 {
        let mut ui=Ui::new().unwrap();
        ui.set_dir(kind==3);
        let body=Cell::new(None);let count=Cell::new(10_000);let requested=Cell::new(None);
        let size=Cell::new(100.0);let calls=Cell::new(0);
        let row_key=|index| pxui_types::ui::int_key(5678,index as i64);
        let mut build=|ui:&mut Ui| {
            ui.view("reveal-page",count.get() as u64,Size::Px(size.get()),Size::Px(size.get()),&mut |ui| {
                let mut row=|ui:&mut Ui,index| {
                    calls.set(calls.get()+1);
                    let h=if kind==1 || kind==2 {10.0+(index%3) as f32*20.0} else {20.0};
                    ui.node_keyed(row_key(index),Node {w:Size::Px(25.0),h:Size::Px(h),look:Look {fill:crate::geom::Rgba {r:255,a:255,..Default::default()},..Look::NONE},..Node::DEFAULT});
                };
                let section=SectionList {count:count.get(),row_height:20.0,leading_height:35.0,header_height:14.0,trailing_height:11.0,overlay_scrollbar:true};
                let list=match kind {
                    0=>ui.section_list("rows",&section,&mut |ui,part| if let SectionPart::Row(index)=part {row(ui,index)}),
                    1=>ui.list_measured("rows",count.get(),20.0,&mut row),
                    2=>ui.section_list_measured("rows",&section,&mut |ui,part,_| if let SectionPart::Row(index)=part {row(ui,index)}),
                    _=>ui.grid_list("rows",count.get(),25.0,20.0,&mut row),
                };
                ui.set_node_layout(&list,&Node {w:Size::Grow,h:Size::Grow,padding:crate::geom::Edges::all(6.0),..Node::DEFAULT});
                body.set(Some(list));
                if let Some(index)=requested.take() {ui.reveal(&list,index);}
            });
        };
        ui.frame(&input(),&mut build).unwrap();
        ui.frame(&input(),&mut build).unwrap();
        if kind==0 {
            for (index, expected) in [(1,1.0),(0,1.0),(4,61.0),(3,61.0),(0,35.0)] {
                ui.reveal(&body.get().unwrap(),index);
                ui.frame(&input(),&mut build).unwrap();
                assert_eq!(ui.scroll_offset(&body.get().unwrap()),expected,"minimum adjustment for row {index}");
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
                let r=ui.rect(&ui.box_of_key(row_key(index)).unwrap());let viewport=ui.rect(&list);
                let header=if kind==0 || kind==2 {14.0} else {0.0};
                assert!(r.y>=viewport.y+6.0+header-0.05 && r.y+r.h<=viewport.y+viewport.h-5.95,"kind {kind}, index {index}: {r:?} in {viewport:?}");
                assert!(calls.get()<100,"reveal must not build all 10,000 rows");
                let offset=ui.scroll_offset(&list);
                ui.reveal(&list,index);
                assert_eq!(ui.scroll_offset(&list),offset,"visible rows need no additional scrolling");
                if round>1 {assert_eq!(allocations,0,"the request itself allocates nothing");assert_eq!(mem_sys::local_alloc_count(),0,"warm reveal/build/draw kind {kind}");}
            }
        }
        for _ in 0..605 {ui.frame(&input(),&mut build).unwrap();}
        mem_sys::local_reset();
        ui.reveal(&body.get().unwrap(),1);
        ui.frame(&input(),&mut build).unwrap();
        assert!(ui.box_of_key(row_key(1)).is_some(),"live cached-list geometry must survive cache expiry");
        assert_eq!(mem_sys::local_alloc_count(),0);
        let list=body.get().unwrap();let offset=ui.scroll_offset(&list);
        for index in [10_000,usize::MAX-1,usize::MAX] {ui.reveal(&list,index);}
        assert_eq!(ui.scroll_offset(&list),offset);
        // A request made after the builder must settle in the same frame with the new viewport.
        size.set(80.0);requested.set(Some(9_999));
        ui.frame(&input(),&mut build).unwrap();
        let list=body.get().unwrap();let r=ui.rect(&ui.box_of_key(row_key(9_999)).unwrap_or_else(||panic!("resize kind {kind}, offset {}, request {:?}",ui.scroll_offset(&list),requested.get())));let v=ui.rect(&list);
        assert!(r.y>=v.y+5.95 && r.y+r.h<=v.y+v.h-5.95);
        count.set(0);ui.frame(&input(),&mut build).unwrap();
        assert_eq!(ui.scroll_offset(&body.get().unwrap()),0.0);
        ui.frame(&input(),&mut |_| {}).unwrap();
        ui.reveal(&list,5);
    }
}

#[test]
fn gestures_swipe_long_press_and_stale_handles_through_the_abi_without_warm_allocations() {
    use crate::{Event, GestureOpts, GestureState, SwipeActions, Touch, Str, ui::{BoxOpts, Size, CLICKABLE}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    let state = Cell::new(GestureState::default());
    let node = Cell::new(None);
    let mut time = 0.0;
    let mut build = |ui: &mut Ui| {
        let row = ui.box_("row", BoxOpts::default().at((0.0, 0.0)).w(Size::Px(200.0)).h(Size::Px(50.0)).flags(CLICKABLE));
        node.set(Some(row));
        state.set(ui.gesture(&row, GestureOpts { long_press: true, swipe_actions: Some(SwipeActions { leading: 60.0, trailing: 80.0, full: 120.0 }), ..Default::default() }));
    };
    let touch = |ui: &mut Ui, phase: u32, x: f64, at: f64, build: &mut dyn FnMut(&mut Ui)| {
        let packet = Touch { device: 1, finger: 7, x, y: 25.0, phase, time: at, pressure: 1.0 };
        let event = Event { kind: Event::TOUCH, text: Str { ptr: (&packet as *const Touch).cast(), len: core::mem::size_of::<Touch>() }, ..Default::default() };
        ui.frame(&FrameIn { time: at, events: &event, event_count: 1, ..input() }, build).unwrap();
    };
    ui.frame(&FrameIn { time, ..input() }, &mut build).unwrap();
    for round in 0..4 {
        time = f64::from(round) * 20.0 + 1.0;
        mem_sys::local_reset();
        touch(&mut ui, 0, 150.0, time, &mut build);
        touch(&mut ui, 1, 142.0, time + 0.016, &mut build);
        assert_eq!(state.get().swipe, -8.0);
        touch(&mut ui, 1, 100.0, time + 0.032, &mut build);
        assert_eq!((state.get().swiping, state.get().swipe), (true, -50.0));
        touch(&mut ui, 2, 100.0, time + 0.4, &mut build);
        ui.frame(&FrameIn { time: time + 0.5, ..input() }, &mut build).unwrap();
        assert!(state.get().swipe_open && !state.get().swiping);
        ui.frame(&FrameIn { time: time + 2.0, ..input() }, &mut build).unwrap();
        assert_eq!(state.get().swipe, -80.0);
        ui.close_swipe(&node.get().unwrap());
        ui.frame(&FrameIn { time: time + 3.0, ..input() }, &mut build).unwrap();
        ui.frame(&FrameIn { time: time + 4.0, ..input() }, &mut build).unwrap();
        assert_eq!(state.get().swipe, 0.0);
        // a held touch: highlight then long press come from frames alone
        touch(&mut ui, 0, 100.0, time + 5.0, &mut build);
        ui.frame(&FrameIn { time: time + 5.2, ..input() }, &mut build).unwrap();
        assert!(state.get().highlighted && !state.get().long_pressed);
        ui.frame(&FrameIn { time: time + 5.6, ..input() }, &mut build).unwrap();
        assert!(state.get().long_pressed && !state.get().highlighted);
        touch(&mut ui, 2, 100.0, time + 5.7, &mut build);
        // leading side opens; a full swipe commits each side for one frame and settles closed
        touch(&mut ui, 0, 100.0, time + 8.1, &mut build);
        touch(&mut ui, 1, 108.0, time + 8.116, &mut build);
        touch(&mut ui, 1, 150.0, time + 8.132, &mut build);
        assert_eq!((state.get().swiping, state.get().swipe), (true, 50.0));
        touch(&mut ui, 2, 150.0, time + 8.5, &mut build);
        ui.frame(&FrameIn { time: time + 10.0, ..input() }, &mut build).unwrap();
        let open = state.get();
        assert!(open.swipe_open && open.swipe_open_leading && !open.swipe_open_trailing && open.swipe == 60.0 && !open.swipe_commit_leading);
        ui.close_swipe(&node.get().unwrap());
        ui.frame(&FrameIn { time: time + 10.1, ..input() }, &mut build).unwrap();
        ui.frame(&FrameIn { time: time + 11.0, ..input() }, &mut build).unwrap();
        assert_eq!(state.get().swipe, 0.0);
        for (i, (end, trailing)) in [(20.0, true), (280.0, false)].into_iter().enumerate() {
            let t = time + 12.0 + 3.0 * i as f64;
            touch(&mut ui, 0, 150.0, t, &mut build);
            touch(&mut ui, 1, 150.0 + (end - 150.0) * 0.1, t + 0.016, &mut build);
            touch(&mut ui, 1, end, t + 0.032, &mut build);
            touch(&mut ui, 2, end, t + 0.5, &mut build);
            let done = state.get();
            assert_eq!((done.swipe_commit_trailing, done.swipe_commit_leading, done.swipe_open), (trailing, !trailing, false));
            ui.frame(&FrameIn { time: t + 0.6, ..input() }, &mut build).unwrap();
            assert!(!state.get().swipe_commit_trailing && !state.get().swipe_commit_leading);
            ui.frame(&FrameIn { time: t + 2.0, ..input() }, &mut build).unwrap();
            assert_eq!(state.get().swipe, 0.0);
        }
        if round > 1 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm gesture frames"); }
    }
    let stale = node.get().unwrap();
    ui.frame(&FrameIn { time, ..input() }, &mut |_| {}).unwrap();
    assert_eq!(ui.gesture_state(&stale), GestureState::default(), "a retired node reads as rest");
    ui.close_swipe(&stale);
}

#[test]
fn dismiss_drags_commits_and_springs_back_through_the_abi_without_warm_allocations() {
    use crate::{DismissOpts, Event, GestureOpts, GestureState, Touch, Str, ui::{BoxOpts, Size, CLICKABLE}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    let state = Cell::new(GestureState::default());
    let mut build = |ui: &mut Ui| {
        let sheet = ui.box_("sheet", BoxOpts::default().at((0.0, 0.0)).w(Size::Px(200.0)).h(Size::Px(200.0)).flags(CLICKABLE));
        state.set(ui.gesture(&sheet, GestureOpts { dismiss: Some(DismissOpts::default()), ..Default::default() }));
    };
    let touch = |ui: &mut Ui, phase: u32, y: f64, at: f64, build: &mut dyn FnMut(&mut Ui)| {
        let packet = Touch { device: 1, finger: 7, x: 100.0, y, phase, time: at, pressure: 1.0 };
        let event = Event { kind: Event::TOUCH, text: Str { ptr: (&packet as *const Touch).cast(), len: core::mem::size_of::<Touch>() }, ..Default::default() };
        ui.frame(&FrameIn { time: at, events: &event, event_count: 1, ..input() }, build).unwrap();
    };
    ui.frame(&FrameIn { time: 0.0, ..input() }, &mut build).unwrap();
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
            ui.frame(&FrameIn { time: t + 0.6, ..input() }, &mut build).unwrap();
            assert!(!state.get().dismiss_committed && !state.get().dismiss_cancelled);
            ui.frame(&FrameIn { time: t + 3.0, ..input() }, &mut build).unwrap();
            assert_eq!((state.get().dismiss, state.get().dismissing), (if i == 0 { 200.0 } else { 0.0 }, false));
            if i == 0 { ui.frame(&FrameIn { time: t + 4.0, ..input() }, &mut |_| {}).unwrap(); ui.frame(&FrameIn { time: t + 4.1, ..input() }, &mut build).unwrap(); }
        }
        if round > 1 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm dismiss frames"); }
    }
}

#[test]
fn scrub_previews_capture_commits_release_and_routes_keys_a11y_touch_without_warm_allocations() {
    use crate::{Event, Scrub, Touch, Str, ui::{Node, Role}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    ui.set_a11y(true);
    let value = Cell::new(40.0);
    let range = Cell::new((10.0, 110.0));
    let result = Cell::new(None::<Scrub>);
    let popup = Cell::new(false);
    let present = Cell::new(true);
    let mut build = |ui: &mut Ui| {
        if popup.get() { ui.popup(None,None,false,false,(0.0,0.0),200.0,30.0,"scrub-popup",&mut |_| {}); }
        if !present.get() { result.set(None); return; }
        let (lo, hi) = range.get();
        let scrub = ui.scrub("Position", value.get(), lo..=hi, 5.0);
        ui.set_node_layout(&scrub.node, &Node { w: Size::Px(200.0), h: Size::Px(20.0), ..Node::DEFAULT });
        result.set(Some(scrub));
    };
    let pointer = |kind, x, code| Event { kind, x, y: 10.0, code, ..Default::default() };
    let key = |index| Event { kind: Event::KEY_PRESSED, code: pxui_types::KEY_SPECIAL + index, ..Default::default() };
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
            (key(3), Some(45.0), Some(45.0), false),
            (key(2), Some(35.0), Some(35.0), false),
            (key(0), Some(45.0), Some(45.0), false),
            (key(1), Some(35.0), Some(35.0), false),
            (key(27), Some(90.0), Some(90.0), false),
            (key(28), Some(10.0), Some(10.0), false),
            (key(25), Some(10.0), Some(10.0), false),
            (key(26), Some(110.0), Some(110.0), false),
            (pointer(Event::POINTER_PRESSED, 50.0, 0), Some(35.0), None, true),
            (pointer(Event::POINTER_CANCELLED, 50.0, 0), None, None, false),
            (pointer(Event::POINTER_RELEASED, 50.0, 0), None, None, false),
            (pointer(Event::POINTER_PRESSED, 50.0, 0), Some(35.0), None, true),
            (Event { kind: Event::FOCUS_LOST, ..Default::default() }, None, None, false),
            (pointer(Event::POINTER_RELEASED, 50.0, 0), None, None, false),
        ] {
            time += 0.01;
            ui.frame(&FrameIn { time, events: &event, event_count: 1, ..input() }, &mut build).unwrap();
            let s = result.get().unwrap();
            assert_eq!((s.preview, s.commit, s.held), (preview, commit, held), "event {event:?}");
            assert_eq!(ui.rect(&s.node), Rect::new(0.0, 0.0, 200.0, 20.0));
        }
        // The release point survives a later move in the same batch.
        let press = pointer(Event::POINTER_PRESSED, 50.0, 0);
        ui.frame(&FrameIn { time, events: &press, event_count: 1, ..input() }, &mut build).unwrap();
        let events = [pointer(Event::POINTER_RELEASED, 150.0, 0), pointer(Event::POINTER_MOVED, 5.0, 0)];
        ui.frame(&FrameIn { time, events: events.as_ptr(), event_count: 2, ..input() }, &mut build).unwrap();
        assert_eq!(result.get().unwrap().commit, Some(85.0));
        ui.frame(&FrameIn { time, events: &press, event_count: 1, ..input() }, &mut build).unwrap();
        let events = [pointer(Event::POINTER_RELEASED,150.0,0),pointer(Event::POINTER_PRESSED,20.0,0)];
        ui.frame(&FrameIn { time, events: events.as_ptr(), event_count: 2, ..input() }, &mut build).unwrap();
        let s = result.get().unwrap();
        assert_eq!((s.preview,s.commit,s.held),(Some(20.0),Some(85.0),true),"a new capture previews independently of the previous release");
        let cancel = pointer(Event::POINTER_CANCELLED,20.0,0);
        ui.frame(&FrameIn { time, events: &cancel, event_count: 1, ..input() }, &mut build).unwrap();
        for accessible in [false,true] {
            ui.frame(&FrameIn { time, events: &press, event_count: 1, ..input() }, &mut build).unwrap();
            let right=key(3);
            if accessible { ui.inject_a11y(&[crate::A11yAction {id:identity,action_code:4,payload:75.0,..Default::default()}]); }
            ui.frame(&FrameIn { time, events: if accessible {core::ptr::null()} else {&right}, event_count:usize::from(!accessible), ..input() }, &mut build).unwrap();
            let s=result.get().unwrap();let expected=if accessible {75.0} else {40.0};
            assert_eq!((s.preview,s.commit,s.held),(Some(expected),Some(expected),false),"digital adjustment takes over a held pointer");
            let release=pointer(Event::POINTER_RELEASED,150.0,0);
            ui.frame(&FrameIn {time,events:&release,event_count:1,..input()},&mut build).unwrap();
            assert!(result.get().unwrap().commit.is_none());
        }
        let events = [key(3), key(3), key(2)];
        ui.frame(&FrameIn { time, events: events.as_ptr(), event_count: events.len(), ..input() }, &mut build).unwrap();
        assert_eq!(result.get().unwrap().commit, Some(45.0), "all routed presses retain their order");
        let events = [key(12), key(3), Event { kind: Event::KEY_RELEASED, code: pxui_types::KEY_SPECIAL + 12, ..Default::default() }];
        ui.frame(&FrameIn { time, events: events.as_ptr(), event_count: events.len(), ..input() }, &mut build).unwrap();
        assert!(result.get().unwrap().commit.is_none(),"command chords remain available to the application");
        for (payload, expected) in [(75.0, 75.0), (-1.0, 10.0), (f64::MAX, 110.0)] {
            ui.inject_a11y(&[crate::A11yAction { id: identity, action_code: 4, payload, ..Default::default() }]);
            ui.frame(&FrameIn { time, ..input() }, &mut build).unwrap();
            assert_eq!(result.get().unwrap().commit, Some(expected));
            let node = ui.a11y_nodes().iter().find(|n| n.id == identity).unwrap();
            assert_eq!((node.value.now, node.value.min, node.value.max, node.value.step), (expected as f64, 10.0, 110.0, 5.0));
        }
        for (phase, x, preview, commit) in [(0,50.0,Some(35.0),None),(1,250.0,Some(110.0),None),(2,150.0,Some(85.0),Some(85.0)),(0,50.0,Some(35.0),None),(3,150.0,None,None)] {
            time += 0.01;
            let touch = Touch { device: 1, finger: 7, x, y: 10.0, phase, time, pressure: 1.0 };
            let event = Event { kind: Event::TOUCH, text: Str { ptr: (&touch as *const Touch).cast(), len: core::mem::size_of::<Touch>() }, ..Default::default() };
            ui.frame(&FrameIn { time, events: &event, event_count: 1, ..input() }, &mut build).unwrap();
            let s = result.get().unwrap();
            assert_eq!((s.preview, s.commit), (preview, commit));
        }
        for (phase, finger, held) in [(0,7,true),(0,8,false),(2,7,false),(2,8,false)] {
            time += 0.01;
            let touch = Touch { device: 1, finger, x: 50.0, y: 10.0, phase, time, pressure: 1.0 };
            let event = Event { kind: Event::TOUCH, text: Str { ptr: (&touch as *const Touch).cast(), len: core::mem::size_of::<Touch>() }, ..Default::default() };
            ui.frame(&FrameIn { time, events: &event, event_count: 1, ..input() }, &mut build).unwrap();
            let s = result.get().unwrap();
            assert_eq!(s.held,held);
            assert!(s.commit.is_none(),"multitouch cancels the original adjustment");
        }
        ui.frame(&FrameIn { time, events: &press, event_count: 1, ..input() }, &mut build).unwrap();
        popup.set(true);
        ui.frame(&FrameIn { time, ..input() }, &mut build).unwrap();
        assert!(!result.get().unwrap().held);
        let release = pointer(Event::POINTER_RELEASED,150.0,0);
        ui.frame(&FrameIn { time, events: &release, event_count: 1, ..input() }, &mut build).unwrap();
        assert!(result.get().unwrap().commit.is_none(),"a modal cancels an outside scrub");
        ui.dismiss_popup(); popup.set(false);
        ui.frame(&FrameIn { time, ..input() }, &mut build).unwrap();
        ui.frame(&FrameIn { time, events: &press, event_count: 1, ..input() }, &mut build).unwrap();
        present.set(false);
        ui.frame(&FrameIn { time, ..input() }, &mut build).unwrap();
        assert!(result.get().is_none());
        present.set(true);
        ui.frame(&FrameIn { time, events: &release, event_count: 1, ..input() }, &mut build).unwrap();
        assert!(result.get().unwrap().commit.is_none(),"a retired owner cannot commit after reappearing");
        identity = ui.a11y_nodes().iter().find(|n| ui.a11y_name_of(n) == "Position").unwrap().id;
        if round > 1 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm pointer/keyboard/a11y/touch scrub frames"); }
    }
    let press = pointer(Event::POINTER_PRESSED, 50.0, 0);
    ui.frame(&FrameIn { time, events: &press, event_count: 1, ..input() }, &mut build).unwrap();
    range.set((10.0, 10.0)); value.set(10.0);
    ui.frame(&FrameIn { time, ..input() }, &mut build).unwrap();
    assert!(!result.get().unwrap().held);
    assert!(ui.a11y_nodes().iter().find(|n| n.id == identity).unwrap().is_role(Role::Progress));
    range.set((10.0, 110.0)); value.set(40.0);
    let release = pointer(Event::POINTER_RELEASED, 150.0, 0);
    ui.frame(&FrameIn { time, events: &release, event_count: 1, ..input() }, &mut build).unwrap();
    assert!(result.get().unwrap().commit.is_none(), "a read-only transition cancels capture");
    ui.set_dir(true);
    ui.frame(&FrameIn { time, ..input() }, &mut build).unwrap();
    let x = f64::from(ui.rect(&result.get().unwrap().node).x);
    let press = pointer(Event::POINTER_PRESSED, x + 50.0, 0);
    let release = pointer(Event::POINTER_RELEASED, x + 150.0, 0);
    ui.frame(&FrameIn { time, events: &press, event_count: 1, ..input() }, &mut build).unwrap();
    assert_eq!(result.get().unwrap().preview, Some(85.0));
    ui.frame(&FrameIn { time, events: &release, event_count: 1, ..input() }, &mut build).unwrap();
    assert_eq!(result.get().unwrap().commit, Some(35.0));
    let right = key(3);
    ui.frame(&FrameIn { time, events: &right, event_count: 1, ..input() }, &mut build).unwrap();
    assert_eq!(result.get().unwrap().commit, Some(35.0));
}

#[test]
fn repeating_keyframes_keep_instances_and_builds_idle_when_hidden_and_stop_without_allocating() {
    use crate::{kit::{AnimationTarget, Keyframe, Repeat}, ui::{Curve, Look, Node, Target}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    ui.set_a11y(true);
    let view = Cell::new(None);
    let child = Cell::new(None);
    let builds = Cell::new(0);
    let mut build = |ui: &mut Ui| {
        view.set(Some(ui.view("repeat", 1, Size::Px(20.0), Size::Px(10.0), &mut |ui| {
            builds.set(builds.get() + 1);
            let bar = ui.node("bar", Node { w: Size::Grow, h: Size::Grow, role: crate::ui::Role::Image, look: Look { fill: crate::geom::Rgba { r: 255, a: 255, ..Default::default() }, ..Look::NONE }, ..Node::DEFAULT });
            ui.a11y_name(&bar, "Repeated bar");
            child.set(Some(bar));
        })));
    };
    let curve = Curve::Bezier { duration: 99.0, points: [0.0, 0.0, 1.0, 1.0] };
    let scale = |y| AnimationTarget::Scale { x: 1.0, y, anchor: (0.5, 1.0) };
    let keys = [Keyframe { time: 0.0, target: scale(0.4), curve }, Keyframe { time: 0.5, target: scale(1.0), curve }];
    ui.frame(&input(), &mut build).unwrap();
    ui.frame(&input(), &mut build).unwrap();
    let view = view.get().unwrap();
    let child = child.get().unwrap();
    let identity = ui.a11y_nodes().iter().find(|node| ui.a11y_name_of(node) == "Repeated bar").unwrap().id;
    assert!(ui.box_of_a11y(u64::MAX).is_none());
    let instances = ui.layer(0).instances.to_vec();
    let built = builds.get();
    for round in 0..3 {
        let start = round as f64 * 10.0;
        ui.frame(&FrameIn { time: start, ..input() }, &mut build).unwrap();
        mem_sys::local_reset();
        assert!(ui.animate_keys_repeat(&view, &keys, Repeat::Forever, true, 0.25));
        for tick in 0..61 {
            let dt = tick as f64 / 60.0;
            ui.frame(&FrameIn { time: start + dt, ..input() }, &mut build).unwrap();
            let phase = (dt + 0.25) % 1.0;
            let progress = if phase < 0.5 { phase * 2.0 } else { 2.0 - phase * 2.0 };
            let rect = ui.rect(&child);
            let resolved = ui.box_of_a11y(identity).unwrap();
            assert_eq!(resolved.box_key, child.box_key);
            assert_eq!(ui.rect(&resolved), rect);
            assert!((rect.h - (4.0 + 6.0 * progress as f32)).abs() < 0.001);
            assert!((rect.y + rect.h - 10.0).abs() < 0.001, "fixed bottom anchor");
            assert!(ui.wants_frame(&[], start + dt));
            assert_eq!(builds.get(), built);
            assert_eq!(ui.layer(0).instances, instances.as_slice());
        }
        ui.view_clip(&view, Some(Rect::new(0.0, 0.0, 0.0, 0.0)));
        ui.frame(&FrameIn { time: start + 2.0, ..input() }, &mut build).unwrap();
        assert!(!ui.wants_frame(&[], start + 2.0), "hidden loops must idle");
        ui.view_clip(&view, None);
        ui.frame(&FrameIn { time: start + 3.0, ..input() }, &mut build).unwrap();
        assert!(ui.wants_frame(&[], start + 3.0));
        ui.view_opacity(&view, 0.0);
        ui.frame(&FrameIn { time: start + 3.0, ..input() }, &mut build).unwrap();
        assert!(!ui.wants_frame(&[], start + 3.0), "manually hidden loops must idle");
        ui.view_opacity(&view, 1.0);
        ui.frame(&FrameIn { time: start + 3.0, ..input() }, &mut build).unwrap();
        assert!(ui.wants_frame(&[], start + 3.0));
        // Finite cycles count both legs and settle at their authored first stop.
        assert!(ui.animate_keys_repeat(&view, &keys, Repeat::Count(2), true, 0.0));
        for (dt, height, running) in [(0.5, 10.0, true), (1.0, 4.0, true), (1.5, 10.0, true), (2.0, 4.0, false)] {
            ui.frame(&FrameIn { time: start + 3.0 + dt, ..input() }, &mut build).unwrap();
            assert!((ui.rect(&child).h - height).abs() < 0.001);
            assert_eq!(ui.wants_frame(&[], start + 3.0 + dt), running);
        }
        assert!(ui.animate_keys_repeat(&view, &keys, Repeat::Count(2), false, 0.25));
        ui.frame(&FrameIn { time: start + 5.75, ..input() }, &mut build).unwrap();
        assert!((ui.rect(&child).h - 10.0).abs() < 0.001);
        assert!(!ui.wants_frame(&[], start + 5.75));
        assert!(ui.animate_keys_repeat(&view, &keys, Repeat::Forever, true, 0.0));
        assert!(ui.animate(&view, Target::Scale { x: 1.0, y: 0.4, anchor: (0.5, 1.0) }, curve.with_duration(0.0)));
        ui.frame(&FrameIn { time: start + 6.0, ..input() }, &mut build).unwrap();
        assert!(!ui.wants_frame(&[], start + 6.0), "retargeting stops a loop");
        if round > 0 { assert_eq!(mem_sys::local_alloc_count(), 0); }
    }
    assert!(ui.animate_keys_repeat(&view, &keys, Repeat::Forever, true, 0.25));
    ui.frame(&FrameIn { time: 27.0, reduced_motion: 1, ..input() }, &mut build).unwrap();
    assert!((ui.rect(&child).h - 4.0).abs() < 0.001);
    assert!(!ui.wants_frame(&[], 28.0));
    assert!(ui.animate_keys_repeat(&view, &keys, Repeat::Forever, true, 0.0));
    ui.frame(&FrameIn { time: 28.0, reduced_motion: 1, ..input() }, &mut build).unwrap();
    assert!(!ui.wants_frame(&[], 29.0));
    ui.frame(&FrameIn { time: 29.0, ..input() }, &mut |_| {}).unwrap();
    assert!(ui.box_of_a11y(identity).is_none());
    ui.frame(&FrameIn { time: 30.0, ..input() }, &mut build).unwrap();
    assert!(ui.box_of_a11y(identity).is_none(), "recycled slots must not revive retired identities");
}

#[test]
fn shrinking_virtual_sections_select_the_clamped_rows_in_one_build_without_warm_allocations() {
    use std::cell::Cell;
    use crate::ui::{SectionList,SectionPart};
    mem_sys::set_peer(crate::library_counter);
    let count=Cell::new(10_000);let body=Cell::new(None);let first=Cell::new(None);let rows=Cell::new(0);let builds=Cell::new(0);
    let mut ui=Ui::new().unwrap();
    let mut build=|ui:&mut Ui| {
        builds.set(builds.get()+1);rows.set(0);first.set(None);
        body.set(Some(ui.section_list("shrinking",&SectionList {count:count.get(),row_height:20.0,leading_height:100.0,header_height:30.0,trailing_height:40.0,overlay_scrollbar:true},&mut |_,part| {
            if let SectionPart::Row(index)=part {if first.get().is_none() {first.set(Some(index));}rows.set(rows.get()+1);}
        })));
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
            assert_eq!(ui.scroll_offset(&body.get().unwrap()),0.0);
            assert_eq!(rows.get(),next,"the first shrink frame must not leave an empty viewport");
            assert_eq!(first.get(),if next>0 {Some(0)} else {None});
        }
        if cycle>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"warm shrink/grow virtual sections");}
    }
}

#[test]
fn section_list_scrolls_leading_content_pins_blocks_and_virtualizes_without_warm_allocations() {
    use std::cell::Cell;
    use crate::ui::{Node,SectionList,SectionPart,Role};
    mem_sys::set_peer(crate::library_counter);
    for flat in [false,true] {
    let mut ui=Ui::new().unwrap(); ui.set_a11y(true);
    let section=SectionList {count:10_000,row_height:20.0,leading_height:100.0,header_height:30.0,trailing_height:40.0,overlay_scrollbar:true};
    let body=Cell::new(None); let lead=Cell::new(None); let header=Cell::new(None); let first=Cell::new(None);
    let rows=Cell::new(0usize); let frame_builds=Cell::new(0usize); let clicked=Cell::new(false); let sorted=Cell::new(false);
    let mut build=|ui:&mut Ui| {
        frame_builds.set(frame_builds.get()+1);
        rows.set(0); clicked.set(false); sorted.set(false); first.set(None);
        body.set(Some(ui.section_list("section",&section,&mut |ui,part| {
            match part {
                SectionPart::Leading=>{
                    let clip=ui.node("heading-clip",Node {flags:CLIP,w:Size::Grow,h:Size::Px(100.0),..Node::DEFAULT});
                    ui.within(&clip,&mut |ui| {
                        let view=if flat {Ui::view_flat} else {Ui::view};
                        view(ui,"heading-cache",0,Size::Grow,Size::Fit,&mut |ui| {
                            let node=ui.node("title",Node {role:Role::Label,w:Size::Grow,h:Size::Px(100.0),..Node::DEFAULT});
                            ui.a11y_name(&node,"Page heading"); lead.set(Some(node));
                        });
                    });
                }
                SectionPart::Header=>{
                    let node=ui.node("sort",Node {flags:CLICKABLE,role:Role::Button,w:Size::Px(50.0),h:Size::Grow,..Node::DEFAULT});
                    ui.a11y_name(&node,"Sort"); header.set(Some(node)); sorted.set(ui.signal_state(&node).clicked);
                }
                SectionPart::Row(index)=>{
                    rows.set(rows.get()+1);
                    let node=ui.node("row",Node {flags:CLICKABLE,role:Role::Button,w:Size::Grow,h:Size::Grow,..Node::DEFAULT});
                    ui.a11y_name(&node,"Row");
                    if index==0 {first.set(Some(node));}
                    clicked.set(clicked.get() || ui.signal_state(&node).clicked);
                }
            }
        })));
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
            let previous=ui.scroll_offset(&body.get().unwrap());
            let previous_builds=frame_builds.get();
            ui.inject_a11y(&[crate::A11yAction {id,action_code:8,payload:(offset-previous) as f64,..Default::default()}]);
            ui.frame(&input(),&mut build).unwrap();
            if !flat {assert_eq!(frame_builds.get()-previous_builds,1,"indexed heading return must not settle an extra frame build");}
            assert!((ui.scroll_offset(&body.get().unwrap())-offset).abs()<0.001,"scroll offset");
            assert!((ui.rect(&lead.get().unwrap()).y+offset).abs()<0.001,"retained heading rect: {:?}",ui.rect(&lead.get().unwrap()));
            let heading=ui.a11y_nodes().iter().find(|n|ui.a11y_name_of(n)=="Page heading").unwrap();
            assert!((heading.rect.y+offset).abs()<0.001,"retained heading accessibility: {:?}",heading.rect);
            assert_eq!(heading.states & crate::a11y::state::VISIBLE!=0,offset<100.0,"offscreen headings are not visible to accessibility");
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
            let event=crate::Event {kind,x,y,..Default::default()};
            ui.frame(&FrameIn {events:&event,event_count:1,..input()},&mut build).unwrap();
        }
        assert_eq!(sorted.get(),sort); assert_eq!(clicked.get(),row,"sticky padding blocks rows underneath");
    }
    }
}

#[test]
fn section_lists_validate_geometry_recover_callbacks_and_support_empty_unpinned_sections() {
    use crate::ui::{SectionList,SectionPart};
    let valid=SectionList {count:0,row_height:20.0,leading_height:30.0,header_height:0.0,trailing_height:40.0,overlay_scrollbar:false};
    let mut ui=Ui::new().unwrap();
    let called=std::cell::Cell::new(false);
    for bad in [SectionList {count:1_000_001,..valid},SectionList {row_height:0.0,..valid},SectionList {row_height:f32::NAN,..valid},SectionList {leading_height:-1.0,..valid},SectionList {header_height:f32::INFINITY,..valid},SectionList {trailing_height:-1.0,..valid},SectionList {count:2,row_height:f32::MAX,..valid}] {
        assert!(catch_unwind(AssertUnwindSafe(|| {let _=ui.frame(&input(),&mut |ui| {ui.section_list("bad",&bad,&mut |_,_|called.set(true));});})).is_err());
        assert!(!called.get(),"invalid geometry reached the callback");
    }
    for fail in [SectionPart::Leading,SectionPart::Header,SectionPart::Row(0)] {
        let section=SectionList {count:1,header_height:20.0,..valid};
        assert!(catch_unwind(AssertUnwindSafe(|| {let _=ui.frame(&input(),&mut |ui| {ui.section_list("panic",&section,&mut |_,part| {if part==fail {panic!("section callback");}});});})).is_err());
        let mut calls=0;
        ui.frame(&input(),&mut |ui| {ui.section_list("empty",&valid,&mut |_,part| {assert_eq!(part,SectionPart::Leading);calls+=1;});}).unwrap();
        assert!(calls>=1,"empty sections retain the lead-in and omit a zero heading and all rows");
    }
}

#[test]
fn measured_sections_validate_geometry_recover_callbacks_and_support_empty_unpinned_sections() {
    use crate::ui::{SectionList,SectionPart};
    let valid=SectionList {count:0,row_height:20.0,leading_height:30.0,header_height:0.0,trailing_height:40.0,overlay_scrollbar:false};
    let mut ui=Ui::new().unwrap();
    let called=std::cell::Cell::new(false);
    for bad in [SectionList {count:1_000_001,..valid},SectionList {row_height:0.0,..valid},SectionList {row_height:f32::NAN,..valid},SectionList {leading_height:-1.0,..valid},SectionList {header_height:f32::INFINITY,..valid},SectionList {trailing_height:-1.0,..valid},SectionList {count:2,row_height:f32::MAX,..valid}] {
        assert!(catch_unwind(AssertUnwindSafe(|| {let _=ui.frame(&input(),&mut |ui| {ui.section_list_measured("bad",&bad,&mut |_,_,_|called.set(true));});})).is_err());
        assert!(!called.get(),"invalid geometry reached the callback");
    }
    for fail in [SectionPart::Leading,SectionPart::Header,SectionPart::Row(0)] {
        let section=SectionList {count:1,header_height:20.0,..valid};
        assert!(catch_unwind(AssertUnwindSafe(|| {let _=ui.frame(&input(),&mut |ui| {ui.section_list_measured("panic",&section,&mut |_,part,_| {if part==fail {panic!("section callback");}});});})).is_err());
        let mut calls=0;
        ui.frame(&input(),&mut |ui| {ui.section_list_measured("empty",&valid,&mut |_,part,_| {assert_eq!(part,SectionPart::Leading);calls+=1;});}).unwrap();
        assert!(calls>=1,"empty sections retain the lead-in and omit a zero heading and all rows");
    }
}

#[test]
fn rendering_density_matches_paint_updates_cached_views_and_allocates_nothing() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    assert_eq!(ui.density(),1);
    let builds=std::cell::Cell::new(0);
    for round in 0..6 {
        mem_sys::local_reset();
        let before=builds.get();
        for scale in [1.0,2.0,3.0,3.0] {
            ui.frame(&FrameIn {scale,..input()},&mut |ui| {
                assert_eq!(ui.density(),scale as i64);
                ui.view("density",0,Size::Px(40.0),Size::Px(40.0),&mut |ui| {
                    builds.set(builds.get()+1);
                    let density=ui.density();
                    let node=ui.node("paint",crate::Node {w:Size::Px(40.0),h:Size::Px(40.0),..crate::Node::DEFAULT});
                    ui.draw(&node,move |paint,_| {assert_eq!(paint.density,density);});
                });
            }).unwrap();
            assert_eq!(ui.density(),scale as i64,"owner keeps the last frame's density");
        }
        assert_eq!(builds.get()-before,3,"density changes invalidate cached contents; equal density retains them");
        if round>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"cached density changes");}
    }
}

#[test]
fn analytic_curves_preserve_fractional_geometry_validate_and_reuse_warm_storage() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    for round in 0..6 {
        mem_sys::local_reset();
        for scale in [1.0,2.0] {
            ui.frame(&FrameIn {scale,..input()},&mut |ui| {
                let node=ui.node("curves",crate::Node {w:Size::Px(100.0),h:Size::Px(100.0),..crate::Node::DEFAULT});
                ui.draw(&node,|paint,_| {
                    let color=crate::geom::Rgba {r:12,g:34,b:56,a:255};
                    paint.curve((1.25,2.75),(3.5,4.25),(5.75,6.5),1.6,color);
                    paint.cubic([(7.0,7.0),(11.5,7.0),(12.5,17.0),(17.0,17.0)],1.6,color);
                    for width in [0.0,-1.0,f32::NAN,f32::INFINITY,f32::MAX] {
                        paint.curve((0.0,0.0),(1.0,1.0),(2.0,2.0),width,color);
                        paint.cubic([(0.0,0.0);4],width,color);
                    }
                    paint.curve((f32::INFINITY,0.0),(1.0,1.0),(2.0,2.0),1.0,color);
                    paint.cubic([(f32::NAN,0.0);4],1.0,color);
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
    let mut ui=Ui::new().unwrap();
    let values=["one two three four","one   two three four","one two\nthree four","one\u{00a0}two three four","one אב two three four","\t one two three four"];
    for round in 0..6 {
        mem_sys::local_reset();
        ui.frame(&input(),&mut |ui| {
            for (index,value) in values.into_iter().enumerate() {
                let plain=crate::TextStyle {size_points:12.0,line_height:20.0,wrap:crate::TextStyle::WRAP_WORD,..Default::default()};
                let logical=crate::TextStyle {wrap:crate::TextStyle::WRAP_WORD_SPACES,..plain};
                let a=ui.text_service().measure(value,&plain,50.0).unwrap();
                let b=ui.text_service().measure(value,&logical,50.0).unwrap();
                assert_eq!((a.height,a.lines,a.baseline),(b.height,b.lines,b.baseline));
                assert!(b.width>=a.width && b.width.is_finite());
                if index==0 {
                    let space=ui.text_service().width(" ",&plain,0.0).unwrap();
                    assert!((b.width-a.width-space).abs()<0.001,"the longest wrapped word retains its break space");
                }
                if round==0 {
                    let a=ui.text_service().quads(value,&plain,50.0,(0.0,0.0),1.0).unwrap();
                    let b=ui.text_service().quads(value,&logical,50.0,(0.0,0.0),1.0).unwrap();
                    assert_eq!(a,b,"separator metrics leave visible glyphs unchanged");
                }
                let plain=crate::TextStyle {max_lines:1,..plain};
                let logical=crate::TextStyle {max_lines:1,..logical};
                assert_eq!(ui.text_service().measure(value,&plain,50.0).unwrap(),ui.text_service().measure(value,&logical,50.0).unwrap(),"hidden paragraphs must not contribute trailing spaces");
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
    let mut ui = Ui::new().unwrap();
    let open = Cell::new(true);
    let pressed = Cell::new(false);
    let clicked = Cell::new(false);
    let mut build = |ui: &mut Ui| {
        if open.get() {
            ui.popup(None,None,false,false,(0.0,0.0),180.0,100.0,"popup",&mut |ui| {
                ui.node("cover",crate::Node {flags:CLICKABLE,w:Size::Grow,h:Size::Px(100.0),..crate::Node::DEFAULT});
            });
        }
        let underlying=ui.node("underlying",crate::Node {flags:CLICKABLE,w:Size::Px(180.0),h:Size::Px(100.0),..crate::Node::DEFAULT});
        let signal=ui.signal_state(&underlying);
        pressed.set(signal.pressed); clicked.set(signal.clicked);
    };
    for round in 0..8 {
        mem_sys::local_reset();
        open.set(true);
        ui.frame(&input(),&mut build).unwrap();
        ui.dismiss_popup();
        open.set(false);
        for kind in [Event::POINTER_PRESSED,Event::POINTER_RELEASED] {
            let event=Event {kind,x:60.0,y:50.0,..Event::default()};
            ui.frame(&FrameIn {events:&event,event_count:1,..input()},&mut build).unwrap();
            assert_eq!(pressed.get(),kind==Event::POINTER_PRESSED);
            assert_eq!(clicked.get(),kind==Event::POINTER_RELEASED);
        }
        if round>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"dismiss/next-press warm frames"); }
    }
}

#[test]
fn held_keys_include_focused_and_modal_modifiers_without_allocating_or_consuming_presses() {
    use std::cell::Cell;
    use pxui_types::{Event, KEY_SPECIAL};
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    let meta = KEY_SPECIAL + 12;
    assert!(!ui.key_down(meta));
    let modal = Cell::new(false);
    let held = Cell::new(false);
    let comma = Cell::new(false);
    let mut contents = |ui: &mut Ui| {
        let field = ui.node("focused", crate::Node { flags: crate::ui::FOCUSABLE, w: Size::Px(120.0), h: Size::Px(32.0), ..crate::Node::DEFAULT });
        ui.focus_box(&field);
        assert_eq!(ui.key_down(meta), held.get());
        assert_eq!(ui.key_pressed(','), comma.get());
        assert_eq!(ui.key_down(meta), held.get());
        assert_eq!(ui.key_pressed(','), comma.get(), "reading held keys must not consume activation");
    };
    let mut build = |ui: &mut Ui| {
        if modal.get() { ui.popup(None, None, false, false, (10.0,10.0),180.0,40.0,"keys",&mut contents); }
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
                (Event::KEY_PRESSED, meta, true, false),
                (Event::POINTER_MOVED, 0, true, false),
                (Event::KEY_PRESSED, ',' as u32, true, true),
                (Event::KEY_RELEASED, ',' as u32, true, false),
                (Event::KEY_RELEASED, meta, false, false),
                (Event::KEY_PRESSED, meta, true, false),
                (Event::FOCUS_LOST, 0, false, false),
            ] {
                held.set(down); comma.set(pressed);
                let event=Event {kind,code,..Event::default()};
                ui.frame(&FrameIn {time:round as f64+0.01,events:&event,event_count:1,..input()},&mut build).unwrap();
                assert_eq!(ui.key_down(meta), down);
                trace[index]=mem_sys::local_alloc_count(); index+=1;
            }
        }
        if round>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"raw held-key queries with focus and a popup: {trace:?}"); }
    }
    modal.set(false); held.set(true); comma.set(false);
    let keys=[meta];
    ui.input_frame(&crate::Input { logical_size:[320,240], keys:crate::Slice {ptr:keys.as_ptr(),len:1}, ..Default::default() },&mut build).unwrap();
    held.set(false);
    ui.input_frame(&crate::Input { logical_size:[320,240], ..Default::default() },&mut build).unwrap();
}

#[test]
fn modifier_at_press_queries_keep_event_order_focus_loss_and_warm_storage() {
    use pxui_types::{Event, KEY_SPECIAL};
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    for round in 0..5 {
        mem_sys::local_reset();
        for modifier in [KEY_SPECIAL+9,KEY_SPECIAL+10,KEY_SPECIAL+11,KEY_SPECIAL+12] {
            let key=|kind,code| Event {kind,code,..Default::default()};
            for popup in [false,true] {
                for (events,expected) in [
                    ([key(Event::KEY_PRESSED,modifier),key(Event::KEY_PRESSED,b'k' as u32),key(Event::KEY_RELEASED,modifier)],true),
                    ([key(Event::KEY_PRESSED,b'k' as u32),key(Event::KEY_PRESSED,modifier),key(Event::KEY_RELEASED,modifier)],false),
                    ([key(Event::KEY_PRESSED,modifier),key(Event::FOCUS_LOST,0),key(Event::KEY_PRESSED,b'k' as u32)],false),
                    ([key(Event::KEY_PRESSED,modifier),key(Event::KEY_RELEASED,modifier),key(Event::KEY_PRESSED,b'k' as u32)],false),
                ] {
                    let mut contents=|ui:&mut Ui| {
                        let field=ui.node("focus",crate::Node {flags:crate::ui::FOCUSABLE,w:Size::Px(100.0),h:Size::Px(24.0),..crate::Node::DEFAULT});
                        ui.focus_box(&field);
                        assert_eq!(ui.key_pressed_with('k' as u32,modifier),expected);
                        assert_eq!(ui.key_pressed_with('k' as u32,modifier),expected,"query consumes nothing");
                        assert!(ui.key_pressed('k'));
                        assert!(!ui.key_down(modifier));
                    };
                    ui.frame(&FrameIn {time:round as f64,events:events.as_ptr(),event_count:events.len(),..input()},&mut |ui| {
                        if popup {ui.popup(None,None,false,true,(0.0,0.0),120.0,30.0,"chord",&mut contents);}
                        else {contents(ui);}
                    }).unwrap();
                    ui.dismiss_popup();
                }
                let press=key(Event::KEY_PRESSED,modifier);
                ui.frame(&FrameIn {events:&press,event_count:1,..input()},&mut |_| {}).unwrap();
                let events=[key(Event::KEY_PRESSED,b'k' as u32),key(Event::KEY_RELEASED,modifier)];
                ui.frame(&FrameIn {events:events.as_ptr(),event_count:events.len(),..input()},&mut |ui| {
                    assert!(ui.key_pressed_with('k' as u32,modifier),"modifier can be held from the previous frame");
                    assert!(!ui.key_down(modifier));
                }).unwrap();
            }
        }
        if round>=2 {assert_eq!(mem_sys::local_alloc_count(),0,"focused and modal modifier-at-press queries");}
    }
}

#[test]
fn backdrop_saturation_validates_and_updates_without_warm_allocations() {
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    let view = Cell::new(None);
    let mut build = |ui: &mut Ui| {
        view.set(Some(ui.view("glass", 1, Size::Px(120.0), Size::Px(40.0), &mut |ui| {
            ui.node("pane", crate::Node { w: Size::Px(120.0), h: Size::Px(40.0), look: crate::ui::Look { fill: pxui_types::geom::color::white(), ..crate::ui::Look::NONE }, ..crate::Node::DEFAULT });
        })));
    };
    ui.frame(&input(), &mut build).unwrap();
    let view = view.get().unwrap();
    ui.view_backdrop_blur(&view, 8.0);
    for time in [1.0, 2.0] { ui.frame(&FrameIn { time, ..input() }, &mut build).unwrap(); }
    for saturation in [0.0, 0.9, 1.0, 1.5] {
        mem_sys::local_reset();
        ui.view_backdrop_saturation(&view, saturation);
        ui.frame(&FrameIn { time: 3.0, ..input() }, &mut build).unwrap();
        assert_eq!(mem_sys::local_alloc_count(), 0, "warm backdrop saturation update");
    }
    for invalid in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(catch_unwind(AssertUnwindSafe(|| ui.view_backdrop_saturation(&view, invalid))).is_err());
        assert!(ui.last_error().contains("saturation"));
    }
}

#[test]
fn scale_animation_retargets_keyframes_and_accessibility_allocate_nothing_when_warm() {
    use crate::{kit::{AnimationTarget, Keyframe}, ui::{Curve, Look, Node, Role, Target}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    for nested in [false,true] {
    let mut ui = Ui::new().unwrap();
    ui.set_a11y(true);
    let (child, view, builds) = (Cell::new(None), Cell::new(None), Cell::new(0));
    let mut content = |ui: &mut Ui| {
        let v = ui.view("scale", 1, Size::Px(100.0), Size::Px(80.0), &mut |ui| {
            builds.set(builds.get() + 1);
            let b = ui.node("child", Node { w: Size::Px(20.0), h: Size::Px(10.0), role: Role::Button, at: Some((20.0, 10.0)).into(), look: Look { fill: pxui_types::geom::color::rgb(9, 9, 9), ..Look::NONE }, ..Node::DEFAULT });
            ui.a11y_name(&b, "Scaled");
            child.set(Some(b));
        });
        ui.set_at(&v, (100.0, 40.0));
        view.set(Some(v));
    };
    let mut build=|ui:&mut Ui| {if nested {ui.view("outer",0,Size::Grow,Size::Grow,&mut content);} else {content(ui);}};
    for time in [0.0, 0.1] { ui.frame(&FrameIn { time, ..input() }, &mut build).unwrap(); }
    let (view, child, built) = (view.get().unwrap(), child.get().unwrap(), builds.get());
    let instances = ui.layer(0).instances.to_vec();
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
        ui.frame(&FrameIn { time: base, ..input() }, &mut build).unwrap();
        assert!(ui.animate(&view, large, spring));
        ui.frame(&FrameIn { time: base + 0.1, ..input() }, &mut build).unwrap();
        shown(&ui, Rect::new(32.0, 88.5, 42.0, 4.5));
        let before = ui.rect(&child);
        assert!(ui.animate(&view, normal, linear));
        ui.frame(&FrameIn { time: base + 0.1, ..input() }, &mut build).unwrap();
        assert_eq!(ui.rect(&child), before, "retargeting cannot jump");
        ui.frame(&FrameIn { time: base + 0.2, ..input() }, &mut build).unwrap();
        shown(&ui, Rect::new(76.0, 69.25, 31.0, 7.25));
        ui.frame(&FrameIn { time: base + 0.3, ..input() }, &mut build).unwrap();
        shown(&ui, Rect::new(120.0, 50.0, 20.0, 10.0));
        let keys = [
            Keyframe { time: 0.2, target: AnimationTarget::Scale { x: 2.0, y: 0.5, anchor: (1.0, 1.0) }, curve: spring.with_duration(99.0) },
            Keyframe { time: 0.4, target: AnimationTarget::Scale { x: 1.0, y: 1.0, anchor: (1.0, 1.0) }, curve: Curve::Steps { duration: 99.0, count: 2 } },
        ];
        assert!(ui.animate_keys(&view, &keys));
        for (dt, expected) in [(0.1, Rect::new(32.0, 88.5, 42.0, 4.5)), (0.3, Rect::new(80.0, 67.5, 30.0, 7.5)), (0.4, Rect::new(120.0, 50.0, 20.0, 10.0))] {
            ui.frame(&FrameIn { time: base + 0.3 + dt, ..input() }, &mut build).unwrap();
            shown(&ui, expected);
        }
        let allocations = mem_sys::local_alloc_count();
        assert!(round == 0 || allocations == 0, "warm scale animations allocated {allocations} times");
        assert_eq!(builds.get(), built);
        assert!(!ui.wants_frame(&[], base + 1.9));
        assert_eq!(ui.layer(0).instances, instances.as_slice(), "scale must only update the transform table");
    }
    ui.set_theme(&crate::ThemeSource { flags: crate::ThemeSource::REDUCED_MOTION, ..crate::ThemeSource::default() }).unwrap();
    assert!(ui.tokens().reduced_motion);
    ui.frame(&FrameIn { time: 7.0, ..input() }, &mut build).unwrap();
    assert!(ui.animate(&view, large, spring));
    ui.frame(&FrameIn { time: 7.0, ..input() }, &mut build).unwrap();
    shown(&ui, Rect::new(40.0, 85.0, 40.0, 5.0));
    assert!(!ui.wants_frame(&[], 7.1));
    }
}

#[test]
fn sampled_and_stepped_animations_keep_overshoot_and_allocate_nothing_when_warm() {
    use crate::{kit::{AnimationTarget, Keyframe}, ui::{Curve, Look, Node, Target}};
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
    let mut ui = Ui::new().expect("library");
    let mut build = |ui: &mut Ui| {
        ui.view("animation", 1, Size::Px(250.0), Size::Px(50.0), &mut |ui| {
            builds.set(builds.get() + 1);
            chip.set(Some(ui.node("chip", Node { w: Size::Px(40.0), h: Size::Px(20.0), look: Look { fill: pxui_types::geom::color::rgb(200, 40, 40), ..Look::NONE }, ..Node::DEFAULT })));
        });
    };
    for time in [0.0, 0.1] { ui.frame(&FrameIn { time, ..input() }, &mut build).unwrap(); }
    let chip = chip.get().unwrap();
    let built = builds.get();
    let x = |ui: &Ui| ui.layer(0).instances.iter().find(|i| i.color[0] == 200).unwrap().rect[0];
    for round in 0..3 {
        let base = 0.1 + round as f64 * 2.0;
        mem_sys::local_reset();
        assert!(ui.animate(&chip, Target::Offset(0.0, 0.0), instant));
        ui.frame(&FrameIn { time: base, ..input() }, &mut build).unwrap();
        assert!(ui.animate(&chip, Target::Offset(100.0, 0.0), table));
        for (dt, expected) in [(0.1, 120.0), (0.2, 90.0), (0.3, 95.0), (0.4, 100.0)] {
            ui.frame(&FrameIn { time: base + dt, ..input() }, &mut build).unwrap();
            assert!((x(&ui) - expected).abs() < 0.001, "table {dt}: {}", x(&ui));
            assert_eq!(ui.repaint_at().is_some(), dt < 0.4);
        }
        assert!(ui.animate(&chip, Target::Offset(0.0, 0.0), steps));
        for (dt, expected) in [(0.02, 100.0), (0.1, 50.0), (0.15, 50.0), (0.2, 0.0)] {
            ui.frame(&FrameIn { time: base + 0.4 + dt, ..input() }, &mut build).unwrap();
            assert!((x(&ui) - expected).abs() < 0.001, "steps {dt}: {}", x(&ui));
        }
        let keys = [
            Keyframe { time: 0.4, target: AnimationTarget::Offset(100.0, 0.0), curve: table.with_duration(99.0) },
            Keyframe { time: 0.6, target: AnimationTarget::Offset(0.0, 0.0), curve: steps.with_duration(99.0) },
        ];
        assert!(ui.animate_keys(&chip, &keys));
        for (dt, expected) in [(0.1, 120.0), (0.2, 90.0), (0.4, 100.0), (0.5, 50.0), (0.6, 0.0)] {
            ui.frame(&FrameIn { time: base + 0.6 + dt, ..input() }, &mut build).unwrap();
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
    let mut ui = Ui::new().expect("library");
    assert_eq!(ui.font_size(), 13);
    ui.set_font_size(14);
    assert_eq!((ui.font_size(), ui.row_height()), (14, 25));
    ui.set_font_size(13);
    assert_eq!(ui.text_top(None, 10.0, 24.0), 10.0 + ((24.0f32 - 13.0) / 2.0).floor());
    let mut seen = false;
    ui.frame(&input(), &mut |ui: &mut Ui| {
        assert!(!ui.captured() && !ui.text_input_focused() && !ui.verifying());
        assert_eq!(ui.passed_undo(), None::<UndoKind>);
        let b = ui.box_("scroller", BoxOpts::default().flags(CLICKABLE | SCROLL | CLIP).w(Size::Px(100.0)).h(Size::Px(60.0)).at((10.0, 20.0)).scroll_step(12.0));
        ui.box_fmt(format_args!("{}-fmt", "fmt"), BoxOpts::default().w(Size::Px(10.0)).h(Size::Px(10.0)));
        assert_eq!(ui.scroll_offset(&b), 0.0);
        ui.set_scroll_offset(&b, 5.0);
        assert_eq!((ui.scroll_offset(&b), ui.scroll_position(&b)), (5.0, 5.0));
        assert_eq!(ui.text_state(&b), None);
        ui.set_text_state(&b, Some("kept".into()));
        assert_eq!(ui.text_state(&b), Some("kept"));
        assert_eq!(ui.cached_label(&b, 0, "label"), "label");
        ui.set_at(&b, (11.0, 21.0));
        let _ = ui.patch_rect(&b, Rect::new(0.0, 0.0, 1.0, 1.0));
        let _ = ui.patch_mark(&b, Mark::None);
        let _ = ui.local_rect(&b);
        assert!(!ui.hovered_exactly(&b) && !ui.hovered_within(&b));
        assert_eq!(ui.context_at(&b), None);
        assert_eq!(ui.drop_target(&b), None::<Drop>);
        assert!(ui.press_keys(&b).is_empty());
        assert_eq!(ui.root_at((1.0, 1.0), &[]), None);
        ui.carry(None, "kind", "value");
        let payload = ui.carrying().expect("a key carry is held at once");
        assert_eq!((payload.kind.as_str(), payload.value.as_str()), ("kind", "value"));
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
    let mut ui = Ui::new().expect("library");
    let stale = pxui_types::PxuiBox { index: 0, box_key: 1, box_slot: 99, generation: 7 };
    let err = catch_unwind(AssertUnwindSafe(|| { let _ = ui.scroll_offset(&stale); })).expect_err("a stale box is refused");
    let message = err.downcast_ref::<String>().expect("a formatted message");
    assert!(message.contains("stale box"), "{message}");
    let err = catch_unwind(AssertUnwindSafe(|| ui.set_font_size(0))).expect_err("a font size of zero is refused");
    assert!(err.downcast_ref::<String>().is_some_and(|m| m.contains("font size")));
    assert_eq!(ui.font_size(), 13, "the refused call changed nothing");
}

#[test]
fn kit_extras_build_and_report_through_the_engine_shapes() {
    use pxui_types::{theme::Curve, ui::{HeadAction, LineStep, Target}, Axis};
    let mut ui = Ui::new().expect("library");
    let hints = [("Esc", "close")];
    let actions = [HeadAction { caption: "Run".into(), keycap: "R".into(), active: false, usable: true }];
    for round in 0..3 {
        ui.frame(&input(), &mut |ui: &mut Ui| {
            ui.panel(pxui_types::ui::PanelOpts::default(), "extras", &mut |ui: &mut Ui| {
                assert!(!ui.button_styled("Styled", true, false, false, Some(pxui_types::geom::Rgba { r: 9, g: 9, b: 9, a: 255 })));
                let head = ui.inspector_head_for_test(&actions);
                assert_eq!(head.renamed, "Title");
                ui.inspector_body(&mut |ui: &mut Ui| {
                    assert!(ui.inspector_section_view("section", round, true, None, "Section", &mut |ui: &mut Ui| { ui.label("inside"); }));
                    assert!(!ui.inspector_toggle_value("toggle", (0.0, 0.0), false));
                });
                ui.inspector_bar(&hints, 3);
                let (text, step) = ui.line_field((0.0, 0.0), 120.0, 20.0, false, "find", "line", "abc");
                assert_eq!((text, step), (None, LineStep::None));
                let (grip, signal) = ui.grip(format_args!("grip-{}", 1), Axis::Row, None);
                assert!(!signal.clicked);
                assert_eq!(ui.field_drag(&grip), None);
                if round == 0 { assert!(ui.animate(&grip, Target::Opacity(0.5), Curve::Spring { omega: 30.0 }), "a new target starts moving"); }
                let mut ran = false;
                let open = ui.popup(None, Some(100.0), true, false, (10.0, 10.0), 100.0, 40.0, "pop", &mut |ui: &mut Ui| { ran = true; ui.label("popped"); });
                assert_eq!(open, ran, "the build runs exactly while the popup is open");
            });
        }).expect("frame");
    }
}

impl Ui {
    fn inspector_head_for_test(&mut self, actions: &[pxui_types::ui::HeadAction]) -> pxui_types::ui::Head {
        self.inspector_header("head", Some("kind"), None, Some("1"), Some(std::rc::Rc::new(|name: &str| !name.is_empty())), actions, Some("reset"), true, "Title", "detail")
    }
}

#[test]
fn a_theme_built_from_key_values_scopes_over_a_subtree_and_refuses_bad_keys() {
    use crate::Theme;
    use pxui_types::theme::default;
    let mut theme = Theme::from_tokens(default());
    theme.set("tokens.accent", "#ff0000").expect("a token");
    theme.set("button.hover.fill", "#00ff00").expect("a look key");
    let err = theme.set("button.nonsense", "1").expect_err("an unknown key is refused");
    assert!(err.contains("unknown key"), "{err}");
    assert!(theme.set("tokens.accent", "red").is_err(), "a malformed value is refused");
    assert!(theme.load("/no/such/file.theme").is_err());
    let mut ui = Ui::new().expect("library");
    let mut inside = None;
    ui.frame(&input(), &mut |ui: &mut Ui| {
        let outer = ui.tokens().accent;
        ui.themed(&theme, &mut |ui: &mut Ui| inside = Some(ui.tokens().accent));
        assert_eq!(ui.tokens().accent, outer, "the Ui's own theme is back");
    }).expect("frame");
    assert_eq!(inside, Some(pxui_types::geom::Rgba { r: 255, g: 0, b: 0, a: 255 }));
}

#[test]
fn canvas_text_goes_through_the_uis_own_text_service() {
    let mut ui = Ui::new().expect("library");
    let style = pxui_types::TextStyle::default();
    let mut text = ui.text_service();
    let metrics = text.measure("abc", &style, 0.0).expect("measure");
    assert!(metrics.width > 0.0 && metrics.lines == 1);
    let glyphs = text.quads("abc", &style, 0.0, (10.0, 20.0), 1.0).expect("quads");
    assert_eq!(glyphs.len(), 3, "one quad per glyph");
    assert!(glyphs.iter().all(|g| g.rect[0] >= 10.0));
    let (w, h, pixels) = text.rasterize_rgba("abc", &style, 0.0, 1.0, [255, 255, 255, 255]).expect("rasterize");
    assert_eq!(pixels.len(), (w * h * 4) as usize);
    let mut bad = style;
    bad.size_points = f32::NAN;
    assert!(text.measure("abc", &bad, 0.0).is_err(), "a NaN size is refused");
}

#[test]
fn second_atlas_pages_reach_public_quads_and_both_rasterizers() {
    let mut ui=Ui::new().unwrap();
    let style=crate::TextStyle { size_points:600.0,..Default::default() };
    let mut text=ui.text_service();
    let first=text.quads("A",&style,0.0,(0.0,0.0),2.0).unwrap();
    let glyphs=text.quads("ABCDEF",&style,0.0,(0.0,0.0),2.0).unwrap();
    assert_eq!(glyphs.len(),6);
    assert_eq!(glyphs[0],first[0],"page growth preserves the first public quad");
    assert!(glyphs.iter().any(|g|g.atlas==0) && glyphs.iter().any(|g|g.atlas==2));
    assert!(glyphs.iter().all(|g|g.atlas==0 || g.atlas==2));
    let (w,h,pixels)=text.rasterize_rgba("ABCDEF",&style,0.0,2.0,[17,33,65,255]).unwrap();
    let mut shaper=crate::Text::new(&[crate::text::pragmasevka()]).unwrap();
    let (sw,sh,rgba)=shaper.rasterize_rgba("ABCDEF",&style,0.0,2.0,[17,33,65,255]).unwrap();
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
    assert_eq!(text.quads("ABCDEF",&style,0.0,(0.0,0.0),2.0).unwrap(),glyphs);
}

#[test]
fn value_fields_take_their_options_and_text_edits_are_kept() {
    use crate::opts::ValueFieldOpts;
    use pxui_types::ui::TextEdit;
    use std::rc::Rc;
    let mut ui = Ui::new().expect("library");
    let opts = ValueFieldOpts {
        fraction: Some(0.5), left: true, kept_drag: true, trail: Some(("px".into(), pxui_types::geom::Rgba { r: 1, g: 2, b: 3, a: 255 })),
        slide: Some(Rc::new(|f| format!("{f:.2}"))), scrub: Some(Rc::new(|origin, dx, _| format!("{origin}{dx}"))), ..ValueFieldOpts::default()
    };
    for _ in 0..2 {
        ui.frame(&input(), &mut |ui: &mut Ui| {
            ui.panel(pxui_types::ui::PanelOpts::default(), "fields", &mut |ui: &mut Ui| {
                let (text, editing) = ui.value_field((10.0, 10.0), 120.0, 20.0, &opts, &|t: &str| t.parse::<f32>().is_ok(), "amount", "0.25");
                assert_eq!((text, editing), (None, false));
                let edit = ui.load_text_edit(7, "abc");
                assert_eq!((edit.text.as_str(), edit.caret, edit.anchor), ("abc", 3, 3));
                ui.remember(&TextEdit { text: "abc".into(), caret: 1, anchor: 1 }, false);
                assert!(ui.text_undo_bytes() >= 3, "the snapshot is held");
            });
        }).expect("frame");
    }
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ui.frame(&input(), &mut |ui: &mut Ui| ui.remember(&TextEdit { text: "é".into(), caret: 1, anchor: 0 }, false));
    })).expect_err("a caret inside a character is a caller bug");
    assert!(err.downcast_ref::<String>().is_some_and(|m| m.contains("caret and anchor")));
}

#[test]
fn a_shaper_of_its_own_measures_and_rasterizes_off_the_ui() {
    let mut text = crate::Text::new(&[crate::text::pragmasevka()]).expect("a shaper");
    let style = pxui_types::TextStyle::default();
    let metrics = text.measure("abc", &style, 0.0).expect("measure");
    assert!(metrics.width > 0.0);
    // the same text measures the same in the Ui's own service: one font, two homes
    let mut ui = Ui::new().expect("library");
    assert_eq!(ui.text_service().measure("abc", &style, 0.0).expect("service").width, metrics.width);
    let (w, h, pixels) = text.rasterize_rgba("abc", &style, 0.0, 1.0, [255, 255, 255, 255]).expect("rasterize");
    assert_eq!(pixels.len(), (w * h * 4) as usize);
    assert!(pixels.chunks(4).any(|p| p[3] != 0), "something was inked");
}

#[test]
fn a_stale_field_patches_nothing_and_whole_source_files_round_trip() {
    use crate::opts::ValueFieldOpts;
    use pxui_types::ui::TextEdit;
    let mut ui = Ui::new().expect("library");
    let stale = pxui_types::PxuiBox { index: 0, box_key: 1, box_slot: 99, generation: 7 };
    // the engine answers a stale handle with false, so does the one export that replaced the wrapper's several calls
    assert!(!ui.patch_field(&stale, &ValueFieldOpts::default(), "1.5"));
    assert!(!ui.patch_text(&stale, "x"));
    // text far over the old 1 MiB limit crosses whole in both directions, and a refused call is a loud one
    let source = "(graph g)\n".repeat(300_000);
    assert!(source.len() > 1 << 21);
    ui.frame(&input(), &mut |ui: &mut Ui| {
        let edit: TextEdit = ui.load_text_edit(9, &source);
        assert_eq!(edit.text.len(), source.len(), "the edit holds the whole text");
        let (text, _) = ui.text_area_submit((0.0, 0.0), 100.0, 100.0, &Default::default(), "area", &source);
        assert_eq!(text, None, "the whole text crossed and compared equal");
    }).expect("frame");
}


#[test]
fn text_area_submit_borrows_its_text_and_allocates_nothing_on_warm_frames() {
    use crate::opts::TextAreaOpts;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().expect("library");
    let source = "alpha\nbeta\ngamma";
    let plain = TextAreaOpts::default();
    for tick in 0..12 {
        // warm-up and measured frames make the same call; the first 8 only warm the caches
        mem_sys::local_reset();
        let scope = (tick >= 8).then(mem_sys::warm_scope);
        ui.frame(&FrameIn { time: tick as f64 / 60.0, ..input() }, &mut |ui: &mut Ui| {
            ui.panel(pxui_types::ui::PanelOpts::default(), "borrowed", &mut |ui: &mut Ui| {
                assert_eq!(ui.text_area_submit((0.0, 0.0), 100.0, 100.0, &plain, "area", source), (None, false));
            });
        }).expect("frame");
        let allocated = mem_sys::local_alloc_count();
        drop(scope);
        if tick >= 8 {
            assert_eq!(allocated, 0, "text_area_submit on warm frame {tick}");
            assert!(mem_sys::take_reports().is_empty(), "warm frame {tick} allocated");
        }
    }
    // a splice into the focused area changes the text: it comes back borrowed
    let edit = TextAreaOpts { select: Some((0, 0)), splice: vec![(0, 5, "ALPHA".into())], ..TextAreaOpts::default() };
    let mut changed = None;
    for tick in 12..16 {
        ui.frame(&FrameIn { time: tick as f64 / 60.0, ..input() }, &mut |ui: &mut Ui| {
            ui.panel(pxui_types::ui::PanelOpts::default(), "borrowed", &mut |ui: &mut Ui| {
                changed = changed.take().or(ui.text_area_submit((0.0, 0.0), 100.0, 100.0, &edit, "area", source).0.map(str::to_owned));
            });
        }).expect("frame");
    }
    assert_eq!(changed.as_deref(), Some("ALPHA\nbeta\ngamma"));
}

#[test]
fn tabular_digits_reach_shaping_and_keep_warm_layouts_allocation_free() {
    const FONT: &[u8]=include_bytes!("../../../../assets/fonts/Inter-Regular.ttf");
    mem_sys::set_peer(crate::library_counter);
    let mut text=crate::Text::new(&[FONT]).expect("shaper");
    let plain=pxui_types::TextStyle { size_points:20.0,..Default::default() };
    let tabular=pxui_types::TextStyle { features:pxui_types::TextStyle::FEATURE_TABULAR_NUMBERS,..plain };
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
    use pxui_types::TextStyle;
    const FONT: &[u8]=include_bytes!("../../../../assets/fonts/Inter-Regular.ttf");
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::create(crate::Platform::Apple,None,&[FONT]);
    let plain=TextStyle { size_points:20.0,..Default::default() };
    let tabular=TextStyle { features:TextStyle::FEATURE_TABULAR_NUMBERS,..plain };
    let a=ui.text_service().measure("1111",&plain,0.0).unwrap();
    let b=ui.text_service().measure("1111",&tabular,0.0).unwrap();
    assert!((a.width-b.width).abs()>1.0);
    for features in [2,3,u32::MAX] {
        assert_eq!(ui.text_service().measure("1111",&TextStyle { features,..plain },0.0),Err(pxui_types::PXUI_ERR_ARGUMENT));
    }
    for frame in 0..7 {
        mem_sys::local_reset();
        ui.frame(&FrameIn { time:frame as f64/60.0,..input() },&mut |ui| {
            let bounds=ui.node("numbers",crate::Node { w:Size::Px(240.0),h:Size::Px(80.0),..crate::Node::DEFAULT });
            ui.canvas(&bounds,move |paint,rect| {
                assert_eq!(paint.text_styled((rect.x,rect.y),&plain,0.0,None,"1111"),a);
                assert_eq!(paint.text_styled((rect.x,rect.y+30.0),&tabular,0.0,None,"1111"),b);
            });
        }).unwrap();
        if frame>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"mixed feature styles must keep separate warm layout/glyph caches"); }
    }
}


#[test]
fn inline_spans_on_nodes_preserve_labels_opacity_and_zero_allocation_rebuilds() {
    use crate::{TextSpan,TextStyle};
    use pxui_types::geom::color;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    ui.set_a11y(true);
    let text="ab CD ef";
    let style=TextStyle { size_points:20.0,line_height:24.0,..Default::default() };
    let span=TextSpan { start:3,end:5,flags:TextSpan::COLOR|TextSpan::BACKGROUND|TextSpan::UNDERLINE,
        color:color::rgba(255,0,0,255),background:color::rgba(0,0,255,64),underline:color::rgba(0,255,0,255),..Default::default() };
    let mut shaper=crate::Text::new(&[crate::text::pragmasevka()]).unwrap();
    let measured=shaper.measure_spans(text,&style,200.0,&[span]).unwrap();
    assert_eq!(ui.text_service().measure_spans(text,&style,200.0,&[span]).unwrap(),measured);
    for frame in 0..10 {
        let plain=frame%3==0;
        let opacity=if frame%2==0 { 0.5 } else { 1.0 };
        mem_sys::local_reset();
        ui.frame(&FrameIn { time:frame as f64/60.0,..input() },&mut |ui| {
            let b=ui.node("label",crate::Node { role:crate::ui::Role::Label,w:Size::Px(200.0),h:Size::Px(40.0),look:crate::ui::Look { text:color::rgba(30,30,30,255),opacity,..crate::ui::Look::NONE },..crate::Node::DEFAULT });
            if plain { ui.text(&b,text); } else { ui.text_spans(&b,text,&style,&[span]).unwrap(); }
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
    use crate::{TextSpan,TextStyle};
    use pxui_types::geom::color;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    let style=TextStyle { size_points:20.0,..Default::default() };
    let red=TextSpan { start:0,end:2,flags:TextSpan::COLOR|TextSpan::UNDERLINE,color:color::rgba(255,0,0,255),underline:color::rgba(255,0,0,255),..Default::default() };
    let green=TextSpan { color:color::rgba(0,255,0,255),underline:color::rgba(0,255,0,255),..red };
    for bad in [TextSpan { start:1,end:2,..red },TextSpan { end:99,..red },TextSpan { flags:8,..red }] {
        assert!(ui.text_service().measure_spans("é!",&style,0.0,&[bad]).is_err());
    }
    assert!(ui.text_service().measure_spans("abcdefghij",&style,0.0,&[red;9]).is_err());
    for frame in 0..7 {
        mem_sys::local_reset();
        ui.frame(&FrameIn { time:frame as f64/60.0,..input() },&mut |ui| {
            let b=ui.node("canvas",crate::Node { w:Size::Px(200.0),h:Size::Px(80.0),..crate::Node::DEFAULT });
            ui.canvas(&b,move |paint,rect| {
                let a=paint.text_spans((rect.x,rect.y),&style,0.0,None,"ab cd",&[red]).unwrap();
                let b=paint.text_spans((rect.x,rect.y+30.0),&style,0.0,None,"ab cd",&[green]).unwrap();
                assert_eq!(a,b,"decoration colour changes preserve text geometry");
            });
        }).unwrap();
        if frame>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"warm mixed spans keep separate shape/glyph caches"); }
    }
}


#[test]
fn inline_node_fractional_font_alignment_matches_canvas_text() {
    use crate::{TextSpan,TextStyle};
    use pxui_types::geom::color;
    let mut ui=Ui::new().unwrap();
    let style=TextStyle { size_points:20.25,..Default::default() };
    let span=TextSpan { start:0,end:2,flags:TextSpan::COLOR,color:color::rgba(255,0,0,255),..Default::default() };
    ui.frame(&input(),&mut |ui| {
        let b=ui.node("fractional",crate::Node { w:Size::Px(200.0),h:Size::Px(40.0),look:crate::ui::Look { text:color::rgba(0,0,0,255),..crate::ui::Look::NONE },..crate::Node::DEFAULT });
        ui.text_spans(&b,"ab cd",&style,&[span]).unwrap();
        ui.canvas(&b,move |paint,rect| {
            let top=rect.y+((rect.h-style.size_points)/2.0).floor();
            paint.text_spans((rect.x,top),&style,200.0,Some(color::rgba(0,0,0,255)),"ab cd",&[span]).unwrap();
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
    use crate::{TextSpan,TextStyle};
    use pxui_types::geom::color;
    let mut ui=Ui::new().unwrap();
    let style=TextStyle { size_points:20.0,line_height:24.0,wrap:TextStyle::WRAP_WORD,..Default::default() };
    let span=TextSpan { start:3,end:5,flags:TextSpan::COLOR,color:color::rgba(255,0,0,255),..Default::default() };
    let metrics=ui.text_service().measure_spans("aa bb cc",&style,45.0,&[span]).unwrap();
    assert_eq!(metrics.lines,3);
    ui.frame(&input(),&mut |ui| {
        let b=ui.node("wrapped",crate::Node { w:Size::Px(45.0),h:Size::Px(80.0),look:crate::ui::Look { text:color::rgba(0,0,0,255),..crate::ui::Look::NONE },..crate::Node::DEFAULT });
        ui.text_spans(&b,"aa bb cc",&style,&[span]).unwrap();
        ui.canvas(&b,move |paint,rect| {
            let block=style.size_points+(metrics.lines-1) as f32*style.line_height;
            let top=rect.y+((rect.h-block)/2.0).floor();
            paint.text_spans((rect.x,top),&style,45.0,Some(color::rgba(0,0,0,255)),"aa bb cc",&[span]).unwrap();
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
    use crate::TextStyle;
    let mut ui=Ui::new().unwrap();
    let mut shaper=crate::Text::new(&[crate::text::pragmasevka()]).unwrap();
    let source="ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut previous=None;
    for ellipsis in [TextStyle::ELLIPSIS_NONE,TextStyle::ELLIPSIS_END,TextStyle::ELLIPSIS_MIDDLE,TextStyle::ELLIPSIS_HEAD] {
        let style=TextStyle { ellipsis,size_points:20.0,..Default::default() };
        let a=shaper.measure(source,&style,100.0).unwrap();
        assert_eq!(a,ui.text_service().measure(source,&style,100.0).unwrap());
        let raster=shaper.rasterize_rgba(source,&style,100.0,1.0,[0,0,0,255]).unwrap();
        assert_eq!(raster,ui.text_service().rasterize_rgba(source,&style,100.0,1.0,[0,0,0,255]).unwrap());
        let glyphs=ui.text_service().quads(source,&style,100.0,(0.0,0.0),1.0).unwrap();
        assert!(!glyphs.is_empty());
        if ellipsis!=TextStyle::ELLIPSIS_NONE { assert!(a.width<=100.001 && glyphs.len()<26); }
        if let Some(previous)=previous { assert_ne!(raster,previous,"mode changes select distinct cached layouts"); }
        previous=Some(raster);
    }
    for ellipsis in [4,255] {
        let style=TextStyle { ellipsis,..Default::default() };
        assert_eq!(shaper.measure(source,&style,100.0),Err(pxui_types::PXUI_ERR_ARGUMENT));
        assert_eq!(ui.text_service().measure(source,&style,100.0),Err(pxui_types::PXUI_ERR_ARGUMENT));
        assert!(shaper.rasterize_rgba(source,&style,100.0,1.0,[0,0,0,255]).is_err());
        assert!(ui.text_service().quads(source,&style,100.0,(0.0,0.0),1.0).is_err());
    }
}

#[test]
fn ellipsis_modes_on_nodes_and_canvas_preserve_labels_fit_modes_and_warm_storage() {
    use crate::{TextStyle,TextSpan};
    use pxui_types::geom::color;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    ui.set_a11y(true);
    let source="ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let spans=[TextSpan { start:0,end:1,flags:TextSpan::COLOR,color:color::rgba(255,0,0,255),..Default::default() },
        TextSpan { start:25,end:26,flags:TextSpan::COLOR,color:color::rgba(0,0,255,255),..Default::default() }];
    for frame in 0..15 {
        let ellipsis=[TextStyle::ELLIPSIS_END,TextStyle::ELLIPSIS_MIDDLE,TextStyle::ELLIPSIS_HEAD][frame%3];
        let style=TextStyle { ellipsis,size_points:20.0,..Default::default() };
        mem_sys::local_reset();
        ui.frame(&FrameIn { time:frame as f64/60.0,..input() },&mut |ui| {
            let b=ui.node("ellipsis",crate::Node { flags:pxui_types::ui::FIT_TEXT,role:crate::ui::Role::Label,w:Size::Px(100.0),h:Size::Px(40.0),look:crate::ui::Look { text:color::rgba(0,0,0,255),..crate::ui::Look::NONE },..crate::Node::DEFAULT });
            ui.text_spans(&b,source,&style,&spans).unwrap();
            ui.canvas(&b,move |paint,rect| {
                let top=rect.y+((rect.h-style.size_points)/2.0).floor();
                paint.text_spans((rect.x,top),&style,100.0,Some(color::rgba(0,0,0,255)),source,&spans).unwrap();
            });
        }).unwrap();
        if frame>=9 { assert_eq!(mem_sys::local_alloc_count(),0,"mixed modes retain independent layout/glyph caches"); }
        let instances:Vec<_>=ui.layers().flat_map(|l|l.instances.iter().copied()).collect();
        let n=instances.len()/2;
        assert!(n>0 && instances.len()%2==0);
        for (node,canvas) in instances[..n].iter().zip(&instances[n..]) {
            assert_eq!((node.rect,node.aux,node.color),(canvas.rect,canvas.aux,canvas.color),"FIT_TEXT preserves an explicit head/middle mode");
        }
        let red=instances[..n].iter().any(|i|i.color==[255,0,0,255]);
        let blue=instances[..n].iter().any(|i|i.color==[0,0,255,255]);
        assert_eq!((red,blue),match ellipsis { TextStyle::ELLIPSIS_END=>(true,false),TextStyle::ELLIPSIS_HEAD=>(false,true),_=>(true,true) });
        assert!(ui.a11y_nodes().iter().any(|node|ui.a11y_name_of(node)==source),"truncation keeps the complete accessible source");
    }
}

#[cfg(target_os="macos")]
#[test]
fn system_fonts_reach_standalone_and_ui_shapers_without_warm_allocations() {
    use crate::TextStyle;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    let mut text=crate::Text::new(&[crate::text::pragmasevka()]).unwrap();
    let style=TextStyle { size_points:20.0,..Default::default() };
    let old=text.rasterize_rgba("漢字",&style,300.0,1.0,[0,0,0,255]).unwrap();
    text.enable_system_fonts().unwrap();
    ui.enable_system_fonts().unwrap();
    let new=text.rasterize_rgba("漢字",&style,300.0,1.0,[0,0,0,255]).unwrap();
    assert_ne!(old,new,"enabling invalidates an earlier missing-font layout");
    let cases=["漢字","العربية","😀"];
    let mut measured=[pxui_types::TextMetrics::default();3];
    for (i,source) in cases.into_iter().enumerate() {
        measured[i]=text.measure(source,&style,300.0).unwrap();
        assert_eq!(measured[i],ui.text_service().measure(source,&style,300.0).unwrap());
        let (w,h,rgba)=text.rasterize_rgba(source,&style,300.0,1.0,[0,0,0,255]).unwrap();
        assert!(w>0 && h>0 && rgba.chunks_exact(4).any(|p|p[3]>0),"native fallback rasterizes {source}");
    }
    for _ in 0..4 {
        mem_sys::local_reset();
        text.enable_system_fonts().unwrap();
        ui.enable_system_fonts().unwrap();
        for (i,source) in cases.into_iter().enumerate() {
            assert_eq!(text.measure(source,&style,300.0).unwrap(),measured[i]);
            assert_eq!(ui.text_service().measure(source,&style,300.0).unwrap(),measured[i]);
        }
        assert_eq!(mem_sys::local_alloc_count(),0,"warm fallback measurement crosses both ABI paths without allocating");
    }
}

#[cfg(target_os="macos")]
#[test]
fn system_font_nodes_keep_labels_warm_storage_and_refuse_late_enable() {
    use crate::TextStyle;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    ui.enable_system_fonts().unwrap();
    ui.set_a11y(true);
    let source="漢字 العربية 😀";
    let style=TextStyle { size_points:20.0,..Default::default() };
    for frame in 0..7 {
        mem_sys::local_reset();
        ui.frame(&FrameIn { time:frame as f64/60.0,..input() },&mut |ui| {
            let b=ui.node("native-fonts",crate::Node { role:crate::ui::Role::Label,w:Size::Px(300.0),h:Size::Px(40.0),..crate::Node::DEFAULT });
            ui.text_spans(&b,source,&style,&[]).unwrap();
            ui.canvas(&b,move |paint,rect| { paint.text_styled((rect.x,rect.y+50.0),&style,300.0,None,source); });
        }).unwrap();
        if frame>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"mixed-script nodes and canvas reuse warm layouts and glyphs"); }
        assert!(ui.a11y_nodes().iter().any(|node|ui.a11y_name_of(node)==source));
        assert!(!ui.layers().all(|layer|layer.instances.is_empty()));
    }
    assert_eq!(ui.enable_system_fonts(),Err(pxui_types::PXUI_ERR_STATE));
    assert!(ui.last_error().contains("before the first frame"));
}

#[test]
fn system_preferences_invalidate_kept_views_snap_running_motion_and_restore_authored_flags() {
    use std::cell::Cell;
    use crate::ui::Curve;
    use crate::kit::{AnimationTarget,Keyframe};
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    let scoped=crate::Theme::default();
    let handle=Cell::new(None);
    let child=Cell::new(None);
    let builds=Cell::new(0);
    let seen=Cell::new(0);
    let mut build=|ui:&mut Ui| {
        handle.set(Some(ui.view("system",1,Size::Px(100.0),Size::Px(60.0),&mut |ui| {
            builds.set(builds.get()+1);
            seen.set(ui.system_appearance());
            child.set(Some(ui.node("child",crate::Node { w:Size::Px(20.0),h:Size::Px(10.0),at:Some((10.0,10.0)).into(),..crate::Node::DEFAULT })));
        })));
        let reduced=ui.tokens().reduced_motion;
        ui.themed(&scoped,&mut |ui|assert_eq!(ui.tokens().reduced_motion,reduced,"a scoped theme cannot bypass OS motion policy"));
    };
    let ease=Curve::Bezier { duration:10.0,points:[0.0,0.0,1.0,1.0] };
    for cycle in 0..5 {
        let time=cycle as f64*20.0;
        ui.frame(&FrameIn { time,appearance:FrameIn::APPEARANCE_LIGHT,..input() },&mut build).unwrap();
        ui.frame(&FrameIn { time:time+0.01,appearance:FrameIn::APPEARANCE_LIGHT,..input() },&mut build).unwrap();
        let before=builds.get();
        let view=handle.get().unwrap();
        ui.animate_keys(&view,&[Keyframe { time:5.0,target:AnimationTarget::Scale { x:0.9,y:0.9,anchor:(0.5,0.5) },curve:ease },
            Keyframe { time:10.0,target:AnimationTarget::Scale { x:0.8,y:0.8,anchor:(0.5,0.5) },curve:ease }]);
        ui.frame(&FrameIn { time:time+0.1,appearance:FrameIn::APPEARANCE_LIGHT,..input() },&mut build).unwrap();
        mem_sys::local_reset();
        ui.frame(&FrameIn { time:time+0.2,appearance:FrameIn::APPEARANCE_DARK,reduced_motion:1,..input() },&mut build).unwrap();
        assert!(builds.get()>before);
        assert_eq!(seen.get(),FrameIn::APPEARANCE_DARK);
        assert!(ui.tokens().reduced_motion);
        assert!(ui.repaint_at().is_none(),"turning on reduced motion settles existing keyframes");
        let rect=ui.rect(&child.get().unwrap());
        assert!((rect.w-16.0).abs()<0.001 && (rect.h-8.0).abs()<0.001,"motion reaches the final keyframe: {rect:?}");
        if cycle>=2 { assert_eq!(mem_sys::local_alloc_count(),0,"warm preference changes retain caches and animation storage"); }
        ui.frame(&FrameIn { time:time+0.3,appearance:FrameIn::APPEARANCE_DARK,reduced_motion:1,..input() },&mut build).unwrap();
        let before=builds.get();
        ui.frame(&FrameIn { time:time+0.4,appearance:FrameIn::APPEARANCE_DARK,reduced_motion:1,..input() },&mut build).unwrap();
        assert_eq!(builds.get(),before,"unchanged OS inputs do not invalidate views");
        ui.frame(&FrameIn { time:time+0.5,appearance:FrameIn::APPEARANCE_LIGHT,..input() },&mut build).unwrap();
        assert!(!ui.tokens().reduced_motion,"the app's original motion flag is restored");
    }
    ui.set_theme(&crate::ThemeSource { flags:crate::ThemeSource::REDUCED_MOTION,..Default::default() }).unwrap();
    ui.frame(&FrameIn { time:120.0,..input() },&mut |_|{}).unwrap();
    assert!(ui.tokens().reduced_motion,"an authored motion restriction is preserved");
    for (appearance,reduced_motion) in [(3,0),(u32::MAX,0),(0,2),(0,u32::MAX)] {
        assert!(ui.frame(&FrameIn { appearance,reduced_motion,..input() },&mut |_|panic!("invalid inputs must not build")).is_err());
    }
}

#[test]
fn public_view_controls_retain_instances_geometry_and_hit_clips_without_warm_allocations() {
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let view=Cell::new(None);
    let child=Cell::new(None);
    let builds=Cell::new(0);
    let mut ui=Ui::new().unwrap();
    let mut build=|ui:&mut Ui| {
        let v=ui.view("controlled",1,Size::Px(100.0),Size::Px(80.0),&mut |ui| {
            builds.set(builds.get()+1);
            child.set(Some(ui.node("child",crate::Node { flags:CLICKABLE,w:Size::Px(20.0),h:Size::Px(10.0),at:Some((10.0,10.0)).into(),
                look:crate::ui::Look { fill:pxui_types::geom::color::rgb(255,0,0),..crate::ui::Look::NONE },..crate::Node::DEFAULT })));
        });
        ui.set_at(&v,(40.0,30.0));
        view.set(Some(v));
    };
    for time in [0.0,0.1] { ui.frame(&FrameIn { time,..input() },&mut build).unwrap(); }
    let (v,c,built)=(view.get().unwrap(),child.get().unwrap(),builds.get());
    let original=ui.layer(0).instances.to_vec();
    let near=|a:Rect,b:Rect|[a.x-b.x,a.y-b.y,a.w-b.w,a.h-b.h].iter().all(|v|v.abs()<0.001);
    ui.view_transform_about(&v,1.0,[0.0,-1.0,1.0,0.0],(90.0,70.0));
    ui.view_scale(&v,1.0);
    assert!(near(ui.rect(&c),Rect::new(50.0,40.0,20.0,10.0)),"centre scale replaces an earlier manual rotation and fixed pivot");
    for cycle in 0..4 {
        mem_sys::local_reset();
        for (scale,expected) in [(2.0,Rect::new(10.0,10.0,40.0,20.0)),(1.0,Rect::new(50.0,40.0,20.0,10.0)),
            (0.0,Rect::new(90.0,70.0,0.0,0.0)),(-1.0,Rect::new(110.0,90.0,20.0,10.0))] {
            ui.view_opacity(&v,0.5);
            ui.view_scale(&v,scale);
            ui.view_clip(&v,Some(Rect::new(0.0,0.0,320.0,240.0)));
            ui.view_clip(&v,None);
            assert!(near(ui.rect(&c),expected),"scale {scale}: {:?}",ui.rect(&c));
            assert_eq!(ui.layer(0).instances,original.as_slice(),"view table edits never rewrite instances");
        }
        ui.frame(&FrameIn { time:0.2+cycle as f64*0.1,..input() },&mut build).unwrap();
        assert_eq!(builds.get(),built);
        if cycle>0 { assert_eq!(mem_sys::local_alloc_count(),0); }
    }
    ui.view_scale(&v,2.0);
    ui.view_clip(&v,Some(Rect::new(20.0,15.0,10.0,10.0)));
    for (time,x,hovered) in [(1.0,25.0,true),(1.1,15.0,false)] {
        let event=pxui_types::Event { kind:pxui_types::Event::POINTER_MOVED,x,y:20.0,..Default::default() };
        ui.frame(&FrameIn { time,events:&event,event_count:1,..input() },&mut build).unwrap();
        assert_eq!(ui.signal(&c).hovered,hovered,"a view's window clip also clips hit testing");
    }
    ui.view_clip(&v,None);
    let event=pxui_types::Event { kind:pxui_types::Event::POINTER_MOVED,x:15.0,y:20.0,..Default::default() };
    ui.frame(&FrameIn { time:1.2,events:&event,event_count:1,..input() },&mut build).unwrap();
    assert!(ui.signal(&c).hovered);
    for opacity in [-1.0,1.01,f32::NAN,f32::INFINITY] { assert!(catch_unwind(AssertUnwindSafe(||ui.view_opacity(&v,opacity))).is_err()); }
    for scale in [f32::NAN,f32::INFINITY,f32::NEG_INFINITY] { assert!(catch_unwind(AssertUnwindSafe(||ui.view_scale(&v,scale))).is_err()); }
    for clip in [Rect::new(0.0,0.0,-1.0,2.0),Rect::new(f32::NAN,0.0,1.0,1.0),Rect::new(f32::MAX,0.0,f32::MAX,1.0)] {
        assert!(catch_unwind(AssertUnwindSafe(||ui.view_clip(&v,Some(clip)))).is_err());
    }
}

#[test]
fn public_measured_list_settles_virtual_rows_and_reuses_warm_storage() {
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let rows:[Cell<Option<pxui_types::PxuiBox>>;200]=std::array::from_fn(|_|Cell::new(None));
    let calls=Cell::new(0);
    let list=Cell::new(None);
    let count=Cell::new(200);
    let mut ui=Ui::new().unwrap();
    let mut build=|ui:&mut Ui| {
        let col=ui.node("column",crate::Node { w:Size::Px(100.0),h:Size::Px(200.0),..crate::Node::DEFAULT });
        ui.within(&col,&mut |ui| {
            list.set(Some(ui.list_measured("measured",count.get(),12.0,&mut |ui,i| {
                assert!(i<count.get());
                calls.set(calls.get()+1);
                rows[i].set(Some(ui.node("row",crate::Node { w:Size::Px(80.0),h:Size::Px(10.0+(i%3) as f32*10.0),..crate::Node::DEFAULT })));
            })));
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
        ui.frame(&FrameIn { time:frame as f64/60.0,..input() },&mut build).unwrap();
        assert!(calls.get()>0 && calls.get()<40);
        if frame>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"warm measured rows keep their cache and callback storage"); }
    }
    ui.set_scroll_offset(&list.get().unwrap(),120.0);
    ui.frame(&FrameIn { time:1.0,..input() },&mut build).unwrap();
    assert!(ui.scroll_offset(&list.get().unwrap())>=120.0);
    for frame in 1..5 {
        mem_sys::local_reset();
        ui.frame(&FrameIn { time:1.0+frame as f64/60.0,..input() },&mut build).unwrap();
        if frame>=2 { assert_eq!(mem_sys::local_alloc_count(),0); }
    }
    count.set(0);
    calls.set(0);
    ui.frame(&FrameIn { time:2.0,..input() },&mut build).unwrap();
    assert_eq!(calls.get(),0);
    assert_eq!(ui.scroll_offset(&list.get().unwrap()),0.0);
}

#[test]
fn measured_list_validation_and_callback_panics_leave_frames_reusable() {
    let mut ui=Ui::new().unwrap();
    for (count,estimate) in [(1,0.0),(1,-1.0),(1,f32::NAN),(1,f32::INFINITY),(1_000_001,12.0),(10,f32::MAX)] {
        assert!(catch_unwind(AssertUnwindSafe(|| {
            ui.frame(&input(),&mut |ui| { ui.list_measured("bad",count,estimate,&mut |_,_|panic!("invalid arguments reached the row")); }).unwrap();
        })).is_err());
    }
    assert!(catch_unwind(AssertUnwindSafe(|| {
        ui.frame(&input(),&mut |ui| { ui.list_measured("panic",1,20.0,&mut |_,_|panic!("row callback")); }).unwrap();
    })).is_err());
    ui.frame(&input(),&mut |ui| { ui.list_measured("recovered",1,20.0,&mut |ui,_| { ui.label("Recovered"); }); }).unwrap();
}

#[test]
fn short_startup_input_keeps_capacity_for_the_first_background_resume_batch() {
    use pxui_types::Event;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    let mut build = |ui: &mut Ui| { ui.label("Lifecycle"); };
    let first = Event { kind: Event::FOCUS_GAINED, ..Event::default() };
    ui.frame(&FrameIn { events: &first, event_count: 1, ..input() }, &mut build).unwrap();
    for frame in 0..82 {
        ui.frame(&FrameIn { time: frame as f64 / 60.0, ..input() }, &mut build).unwrap();
    }
    let events = [
        Event { kind: Event::VISIBILITY_CHANGED, value: 0.0, ..Event::default() },
        Event { kind: Event::FOCUS_LOST, ..Event::default() },
        Event { kind: Event::FOCUS_GAINED, ..Event::default() },
        Event { kind: Event::VISIBILITY_CHANGED, value: 1.0, ..Event::default() },
        Event { kind: Event::ENVIRONMENT_CHANGED, ..Event::default() },
    ];
    mem_sys::local_reset();
    ui.frame(&FrameIn { time: 30.0, events: events.as_ptr(), event_count: events.len(), ..input() }, &mut build).unwrap();
    assert_eq!(mem_sys::local_alloc_count(), 0, "first resume must retain the normal input capacity");
    assert!(!ui.text_input_focused());
}

#[test]
fn focused_text_field_keeps_selection_and_ime_without_warm_allocations() {
    use pxui_types::{Event, Str, KEY_SPECIAL};
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    let mut build = |ui: &mut Ui| {
        assert!(ui.text_field("Caption", "café track 10").is_none());
    };
    ui.frame(&input(), &mut build).unwrap();
    let key = |kind, code| Event { kind, code: KEY_SPECIAL + code, ..Event::default() };
    let focus = [key(Event::KEY_PRESSED, 8)];
    ui.frame(&FrameIn { time: 0.1, events: focus.as_ptr(), event_count: focus.len(), ..input() }, &mut build).unwrap();
    assert!(ui.text_input_focused());
    let composition = "é";
    let select = [key(Event::KEY_PRESSED, 9), key(Event::KEY_PRESSED, 2), key(Event::KEY_RELEASED, 9),
        Event { kind: Event::TEXT_EDITING, text: Str { ptr: composition.as_ptr(), len: composition.len() }, y: 1.0, ..Event::default() }];
    ui.frame(&FrameIn { time: 0.2, events: select.as_ptr(), event_count: select.len(), ..input() }, &mut build).unwrap();
    for tick in 0..80 {
        mem_sys::local_reset();
        ui.frame(&FrameIn { time: 0.3 + tick as f64 / 60.0, width: 320.0 + (tick % 2) as f32, ..input() }, &mut build).unwrap();
        if tick >= 10 { assert_eq!(mem_sys::local_alloc_count(), 0, "warm focused text field across the ABI"); }
        assert!(ui.text_input_focused());
        let ((_, _, w, h), caret) = ui.ime().expect("focused field must publish IME geometry");
        assert!(w > 0 && h > 0 && caret >= 0 && caret <= w);
    }
    let commit = Event { kind: Event::TEXT_INPUT, text: Str { ptr: composition.as_ptr(), len: composition.len() }, ..Event::default() };
    let mut changed = false;
    ui.frame(&FrameIn { time: 2.0, events: &commit, event_count: 1, ..input() }, &mut |ui| {
        changed = ui.text_field("Caption", "café track 10") == Some("café track 1é");
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
    fn body(ui: &mut Ui, c: &crate::bodies::PxuiTextFieldConfig) {
        ROW.set(c.row.box_key);
        let mut row = ui.node_layout(&c.row);
        row.h = Size::Px(59.0);
        ui.set_node_layout(&c.row, &row);
        ui.set_look(&c.row, Look::NONE, State::default());
        let editor = ui.part(Part::EDITOR, Node {
            at: Some((18.0, 0.0)).into(), w: Size::Px(280.0), h: Size::Px(59.0),
            inset: Edges {l:2.0,..Edges::ZERO},
            look: Look {text:ui.theme().tokens.foreground,font:Font {size:SIZE.get(),..Font::BODY},..Look::NONE},
            ..Node::DEFAULT
        });
        if c.state.focused == 0 { ui.text(&editor, crate::bodies::text_of(&c.value)); }
    }
    mem_sys::set_peer(crate::library_counter);
    for size in [20,-3,0] {
        for scale in [1.0,2.0] {
            SIZE.set(size);
            let mut ui = Ui::new().unwrap();
            ui.set_a11y(true);
            ui.register_text_field_body("font-sized-editor",body).unwrap();
            let source = "text_field.body = font-sized-editor";
            ui.set_theme(&crate::ThemeSource {source:Str {ptr:source.as_ptr(),len:source.len()},..Default::default()}).unwrap();
            let value = "café track 10";
            let mut build = |ui: &mut Ui| assert!(ui.text_field("Styled",value).is_none());
            let frame = FrameIn {scale,..input()};
            for tick in 0..3 { ui.frame(&FrameIn {time:tick as f64/60.0,..frame},&mut build).unwrap(); }
            let glyphs = ui.layers().flat_map(|layer|layer.instances.iter()).filter(|quad|quad.kind==crate::draw::kind::MASK).map(|quad|(quad.rect,quad.aux)).collect::<Vec<_>>();
            assert!(!glyphs.is_empty());
            let points = if size==0 {ui.font_size()} else {Font {size,..Font::BODY}.points(ui.font_size())};
            let full_width = ui.text_width(Some(points),value);
            let focus = Event {kind:Event::KEY_PRESSED,code:pxui_types::KEY_SPECIAL+8,..Default::default()};
            ui.frame(&FrameIn {time:0.1,events:&focus,event_count:1,..frame},&mut build).unwrap();
            assert_eq!(ui.ime().unwrap().1,(2.0+full_width).round() as i64,"focused text uses the body's editor font size {size}");
            assert!(ui.layers().flat_map(|layer|layer.instances.iter()).filter(|quad|quad.kind==crate::draw::kind::MASK).map(|quad|(quad.rect,quad.aux)).eq(glyphs.iter().copied()),"focus preserves the exact glyph positions and atlas samples");
            let editor = ui.part_rect(ROW.get(),Part::EDITOR).unwrap();
            let prefix = ui.text_width(Some(points),"café ");
            let x = editor.x+2.0+prefix;
            let y = editor.y+editor.h/2.0;
            if size==20 { SIZE.set(24); }
            for kind in [Event::POINTER_PRESSED,Event::POINTER_RELEASED] {
                let click = Event {kind,x:x as f64,y:y as f64,..Default::default()};
                ui.frame(&FrameIn {time:0.2,events:&click,event_count:1,..frame},&mut build).unwrap();
            }
            let selected = ui.a11y_nodes().iter().find(|node|node.role==Role::TextField as u32).unwrap();
            assert_eq!(selected.selection,[5,5],"pointer hit testing uses the displayed UTF-8 text at size {size}");
            if size==20 {
                let width = ui.text_width(Some(24),"café ");
                assert_eq!(ui.ime().unwrap().1,(2.0+width).round() as i64,"a body size change paints the new font immediately, after hit testing the previous frame");
            }
            let key = |kind,code| Event {kind,code:pxui_types::KEY_SPECIAL+code,..Default::default()};
            let composition = "é";
            let select = [key(Event::KEY_PRESSED,9),key(Event::KEY_PRESSED,2),key(Event::KEY_RELEASED,9),Event {kind:Event::TEXT_EDITING,text:Str {ptr:composition.as_ptr(),len:composition.len()},y:1.0,..Default::default()}];
            ui.frame(&FrameIn {time:0.3,events:select.as_ptr(),event_count:select.len(),..frame},&mut build).unwrap();
            let selected = ui.a11y_nodes().iter().find(|node|node.role==Role::TextField as u32).unwrap();
            assert_eq!(selected.selection,[5,4]);
            let keys = [Event {kind:Event::KEY_PRESSED,code:pxui_types::KEY_SPECIAL+11,..Default::default()},Event {kind:Event::KEY_PRESSED,code:pxui_types::KEY_SPECIAL+14,..Default::default()},Event {kind:Event::KEY_RELEASED,code:pxui_types::KEY_SPECIAL+14,..Default::default()},Event {kind:Event::KEY_RELEASED,code:pxui_types::KEY_SPECIAL+11,..Default::default()}];
            for tick in 0..24 {
                mem_sys::local_reset();
                ui.frame(&FrameIn {time:0.4+tick as f64/60.0,events:keys.as_ptr(),event_count:keys.len(),..frame},&mut build).unwrap();
                if tick>=8 {assert_eq!(mem_sys::local_alloc_count(),0,"warm custom body, focus, selection and composition");}
                assert!(ui.ime().is_some());
            }
            assert_eq!(ui.signal_keys(&ui.box_of_key(ROW.get()).unwrap()).len(),keys.len(),"editing must restore the original key stream for later consumers");
            let commit = Event {kind:Event::TEXT_INPUT,text:Str {ptr:composition.as_ptr(),len:composition.len()},..Default::default()};
            let mut changed = false;
            ui.frame(&FrameIn {time:1.0,events:&commit,event_count:1,..frame},&mut |ui| {changed=ui.text_field("Styled",value)==Some("caféétrack 10");}).unwrap();
            assert!(changed,"composition commit replaces exactly the selected UTF-8 range");
        }
    }
}

#[test]
fn numeric_signal_state_preserves_legacy_keys_without_warm_keyboard_allocations() {
    use std::cell::Cell;
    use pxui_types::{Event, KEY_SPECIAL};
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    let button = Cell::new(None);
    let clicks = Cell::new(0);
    let mut build = |ui: &mut Ui| {
        let node = ui.node("button", crate::Node { flags: CLICKABLE | crate::ui::TAB_STOP, w: Size::Px(120.0), h: Size::Px(32.0), ..crate::Node::DEFAULT });
        let state = ui.signal_state(&node);
        clicks.set(clicks.get() + i32::from(state.clicked));
        // the measured frame below reads the borrowed key slices; warm frames take the same path
        let _ = ui.signal_keys(&node).len() + ui.press_keys(&node).len();
        button.set(Some(node));
    };
    ui.frame(&input(), &mut build).unwrap();
    ui.focus_box(&button.get().unwrap());
    let key = |kind, code| Event { kind, code: KEY_SPECIAL + code, ..Default::default() };
    let events = [key(Event::KEY_PRESSED, 9), key(Event::KEY_PRESSED, 4), key(Event::KEY_RELEASED, 4), key(Event::KEY_RELEASED, 9)];
    for tick in 0..60 {
        mem_sys::local_reset();
        ui.frame(&FrameIn { time: 1.0 + tick as f64 / 60.0, events: events.as_ptr(), event_count: events.len(), ..input() }, &mut build).unwrap();
        if tick >= 5 { assert_eq!(mem_sys::local_alloc_count(), 0, "numeric keyboard routing and scalar signal across the ABI"); }
    }
    // a warm frame whose box has key events: reading them through the borrowed slice allocates nothing
    let mut seen = 0;
    let scope = mem_sys::warm_scope();
    ui.frame(&FrameIn { time: 9.0, events: events.as_ptr(), event_count: events.len(), ..input() }, &mut |ui: &mut Ui| {
        let node = ui.node("button", crate::Node { flags: CLICKABLE | crate::ui::TAB_STOP, w: Size::Px(120.0), h: Size::Px(32.0), ..crate::Node::DEFAULT });
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
    let events = [Event { kind: Event::TEXT_EDITING, x: 1.0, y: 2.0,
        text: pxui_types::Str { ptr: text.as_ptr(), len: text.len() }, ..Default::default() },
        Event { kind: Event::TEXT_INPUT, text: pxui_types::Str { ptr: text.as_ptr(), len: text.len() }, ..Default::default() }];
    ui.frame(&FrameIn { time: 3.0, events: events.as_ptr(), event_count: events.len(), ..input() }, &mut build).unwrap();
    let owned = ui.signal_keys(&node);
    assert_eq!(owned.len(), 2);
    let str_of = |e: &Event| e.text.len;
    assert_eq!((owned[0].kind, owned[0].x, owned[0].y, str_of(&owned[0])), (Event::TEXT_EDITING, 1.0, 2.0, text.len()));
    assert_eq!((owned[1].kind, str_of(&owned[1])), (Event::TEXT_INPUT, text.len()));
    ui.frame(&FrameIn { time: 3.1, ..input() }, &mut |_| {}).unwrap();
    assert!(ui.signal_keys(&node).is_empty() && !ui.signal_state(&node).clicked);
}

#[test]
fn borrowed_carry_queries_and_whole_warm_pointer_gestures_allocate_nothing() {
    use crate::{DropRef, PayloadRef};
    use pxui_types::Event;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    let mut hover = false;
    let mut dropped = false;
    let mut deliveries = 0;
    let payload = PayloadRef { kind: "queue-occurrence", value: "18446744073709551615" };
    let mut build = |ui: &mut Ui| {
        let source = ui.box_("carry-source", BoxOpts::default().flags(CLICKABLE).at((0.0, 0.0)).w(Size::Px(100.0)).h(Size::Px(40.0)));
        let target = ui.box_("carry-target", BoxOpts::default().flags(CLICKABLE).at((0.0, 100.0)).w(Size::Px(100.0)).h(Size::Px(40.0)));
        if ui.signal_state(&source).held { ui.carry(Some(&source), payload.kind, payload.value); }
        if let Some(p) = ui.carrying_ref() { assert_eq!(p, payload); }
        hover = false; dropped = false;
        if let Some(state) = ui.drop_target_ref(&target) {
            match state {
                DropRef::Hover(p) => { assert_eq!(p, payload); hover = true; }
                DropRef::Dropped(p) => { assert_eq!(p, payload); dropped = true; deliveries += 1; }
            }
        }
        assert!(!ui.signal_state(&target).clicked);
    };
    let event = |kind, y| Event { kind, x: 10.0, y, ..Default::default() };
    let mut tick = 0u32;
    for round in 0..24 {
        mem_sys::local_reset();
        for e in [None, Some(event(Event::POINTER_PRESSED, 10.0)), Some(event(Event::POINTER_MOVED, 120.0)), Some(event(Event::POINTER_RELEASED, 120.0)), None] {
            tick += 1;
            ui.frame(&FrameIn { time: tick as f64 / 60.0, events: e.as_ref().map_or(core::ptr::null(), |e| e), event_count: usize::from(e.is_some()), ..input() }, &mut build).unwrap();
        }
        if round > 3 { assert_eq!(mem_sys::local_alloc_count(), 0, "whole warm gesture including payload start, hover, release and retirement"); }
    }
    drop(build);
    assert!(!hover && !dropped, "a drop lasts one frame");
    assert_eq!(deliveries, 24);
    // Key carries reuse the same storage, including replacement and cancellation.
    for round in 0..12 {
        ui.frame(&FrameIn { time: 10.0 + round as f64, ..input() }, &mut |_| {}).unwrap();
        mem_sys::local_reset();
        ui.carry(None, payload.kind, payload.value);
        assert_eq!(ui.carrying_ref(), Some(payload));
        ui.carry(None, "kind", "café");
        assert_eq!(ui.carrying_ref(), Some(PayloadRef { kind: "kind", value: "café" }));
        ui.cancel_carry();
        assert_eq!(ui.carrying_ref(), None);
        if round > 1 { assert_eq!(mem_sys::local_alloc_count(), 0); }
    }
}

#[test]
fn transferred_pointer_carries_survive_virtualization_and_keep_warm_modified_gestures_allocation_free() {
    use crate::{DropRef, Node};
    use pxui_types::{Event, KEY_SPECIAL};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    let list = Cell::new(None);
    let source = Cell::new(None);
    let dropped = Cell::new(None);
    let visible = Cell::new(true);
    let mut build = |ui: &mut Ui| {
        if !visible.get() { return; }
        let body = ui.grid_list("virtual-carry", 10_000, 320.0, 20.0, &mut |ui, index| {
            let row = ui.node("row", Node { flags: CLICKABLE, w: Size::Grow, h: Size::Grow, ..Node::DEFAULT });
            if index == 0 {
                source.set(Some(row));
                if ui.signal_state(&row).held { ui.carry(Some(&row), "occurrence", "duplicate song"); }
            }
            if let Some(DropRef::Dropped(payload)) = ui.drop_target_ref(&row) {
                assert_eq!((payload.kind, payload.value), ("occurrence", "duplicate song"));
                dropped.set(Some(index));
            }
        });
        list.set(Some(body));
        let unrelated = ui.node("unrelated", Node { flags: CLICKABLE, at: Some((400.0, 0.0)).into(), w: Size::Px(10.0), h: Size::Px(10.0), ..Node::DEFAULT });
        assert!(!ui.transfer_carry(&unrelated));
        ui.transfer_carry(&body);
        assert!(!ui.signal_state(&body).clicked, "finishing a carry must not click its capture owner");
    };
    ui.frame(&input(), &mut build).unwrap();
    let event = |kind, y| Event { kind, x: 10.0, y, ..Default::default() };
    let shift = |kind| Event { kind, code: KEY_SPECIAL + 9, ..Default::default() };
    let mut time = 0.0;
    for round in 0..20 {
        dropped.set(None);
        mem_sys::local_reset();
        for events in [
            [shift(Event::KEY_PRESSED), event(Event::POINTER_PRESSED, 10.0)],
            [event(Event::POINTER_MOVED, 80.0), Event::default()],
            [Event { kind: Event::SCROLLED, y: -30.0, ..Default::default() }, Event::default()],
            [Event::default(), Event::default()],
            [event(Event::POINTER_RELEASED, 80.0), shift(Event::KEY_RELEASED)],
            [Event { kind: Event::SCROLLED, y: 30.0, ..Default::default() }, Event::default()],
            [Event::default(), Event::default()],
        ] {
            time += 0.02;
            // Zero-kind events are not wire events; omit the unused slots.
            let count = if events[0].kind == 0 { 0 } else { 1 + usize::from(events[1].kind != 0) };
            ui.frame(&FrameIn { time, events: events.as_ptr(), event_count: count, ..input() }, &mut build).unwrap();
            if round == 0 && events[0].kind == Event::POINTER_RELEASED {
                assert_eq!(ui.press_keys(&list.get().unwrap()), &[KEY_SPECIAL + 9], "capture transfer preserves the press modifiers through release");
            }
            if events[0].kind == Event::SCROLLED && events[0].y < 0.0 {
                assert!(!ui.transfer_carry(&source.get().unwrap()), "a virtualized child is stale");
                assert!(ui.carrying_ref().is_some());
            }
        }
        assert_eq!(dropped.get(), Some(34));
        assert!(ui.carrying_ref().is_none());
        if round > 4 { assert_eq!(mem_sys::local_alloc_count(), 0, "modified cross-page gesture across both allocator copies"); }
    }
    for cancel in [Some(event(Event::POINTER_CANCELLED, 0.0)), Some(event(Event::FOCUS_LOST, 0.0)), None] {
        for e in [event(Event::POINTER_PRESSED, 10.0), event(Event::POINTER_MOVED, 80.0)] {
            time += 0.02;
            ui.frame(&FrameIn { time, events: &e, event_count: 1, ..input() }, &mut build).unwrap();
        }
        dropped.set(None);
        if let Some(e) = cancel {
            time += 0.02;
            ui.frame(&FrameIn { time, events: &e, event_count: 1, ..input() }, &mut build).unwrap();
        } else { ui.cancel_carry(); }
        let e = event(Event::POINTER_RELEASED, 80.0);
        time += 0.02;
        ui.frame(&FrameIn { time, events: &e, event_count: 1, ..input() }, &mut build).unwrap();
        assert!(dropped.get().is_none() && ui.carrying_ref().is_none());
    }
    for e in [event(Event::POINTER_PRESSED, 10.0), event(Event::POINTER_MOVED, 80.0)] {
        time += 0.02;
        ui.frame(&FrameIn { time, events: &e, event_count: 1, ..input() }, &mut build).unwrap();
    }
    visible.set(false);
    time += 0.02;
    ui.frame(&FrameIn { time, ..input() }, &mut build).unwrap();
    assert!(ui.carrying_ref().is_none() && !ui.captured());
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
    let mut ui=Ui::new().unwrap();
    for frame in 0..20 {
        mem_sys::local_reset();
        ui.frame(&FrameIn{time:frame as f64,..input()},&mut |ui| {
            let node=ui.node("fractional",crate::Node{w:Size::Px(320.0),h:Size::Px(40.0),..crate::Node::DEFAULT});
            ui.canvas(&node,|paint,rect| {
                let white=pxui_types::geom::color::white();
                for i in 0..200 { paint.fill_fractional(rect.x+i as f32*1.5+0.25,rect.y+1.25,0.5,4.0,white); }
                for (x,y,w,h) in [(f32::NAN,0.0,1.0,1.0),(0.0,f32::INFINITY,1.0,1.0),(f32::MAX,0.0,f32::MAX,1.0),(f32::MAX,0.0,1.0,1.0),(0.0,0.0,0.0,1.0),(0.0,0.0,1.0,-1.0)] {
                    paint.fill_fractional(x,y,w,h,white);
                }
            });
        }).unwrap();
        if frame>3 { assert_eq!(mem_sys::local_alloc_count(),0); }
        let mut quads=ui.layers().flat_map(|layer| layer.instances.iter());
        for i in 0..200 {
            let q=quads.next().unwrap();
            assert_eq!(q.rect,[i as f32*1.5+0.25,1.25,i as f32*1.5+0.75,5.25]);
            assert_eq!(q.flags,pxui_types::draw::flags::AREA);
        }
        assert!(quads.next().is_none());
    }
}

#[test]
fn output_partitions_reuse_the_engine_dirty_capacity_after_unchanged_frames() {
    use crate::ui::{Node,Look};
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    let names:Vec<_>=(0..32).map(|index|format!("part-{index}")).collect();
    for frame in 0..8 {
        ui.frame(&FrameIn{time:frame as f64,..input()},&mut |ui| {
            for (index,name) in names.iter().enumerate() {
                let phase=if frame<3 {frame.min(1)} else {frame-1};
                let changed=index%2==0 && phase%2==1;
                let revision=if index%2==0 {phase as u64} else {0};
                ui.view(name.as_str(),revision,Size::Px(20.0),Size::Px(4.0),&mut |ui| {
                    ui.node("quad",Node{w:Size::Px(20.0),h:Size::Px(4.0),look:Look{fill:if changed {pxui_types::geom::Rgba{r:0,g:0,b:255,a:255}} else {pxui_types::geom::color::white()},..Look::NONE},..Node::DEFAULT});
                });
            }
        }).unwrap();
        if frame<2 { ui.drawn(0); continue; }
        mem_sys::local_reset();
        let layer=ui.layer(0);
        if frame>2 {
            assert_eq!(layer.dirty.len(),16);
            assert_eq!(mem_sys::local_alloc_count(),0,"output partition metadata must reuse capacity when no draw storage grows");
        }
        ui.drawn(0);
    }
}

#[test]
fn simultaneous_view_entrance_channels_move_cached_children_and_settle_without_allocating() {
    use crate::kit::{AnimationTarget, Keyframe};
    use crate::ui::{Curve, Node, Role};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    ui.set_a11y(true);
    let (view, child, builds) = (Cell::new(None), Cell::new(None), Cell::new(0));
    let mut build = |ui: &mut Ui| {
        let v=ui.view("entrance",1,Size::Px(100.0),Size::Px(80.0),&mut |ui| {
            builds.set(builds.get()+1);
            let b=ui.node("child",Node { w:Size::Px(20.0),h:Size::Px(10.0),role:Role::Button,
                look:crate::ui::Look {fill:pxui_types::geom::color::rgb(20,30,40),..crate::ui::Look::NONE},..Node::DEFAULT });
            ui.a11y_name(&b,"Moving child"); child.set(Some(b));
        });
        ui.set_at(&v,(20.0,20.0)); view.set(Some(v));
    };
    ui.frame(&input(),&mut build).unwrap();
    let instances=ui.layer(0).instances.to_vec();
    let curve=Curve::Bezier {duration:1.0,points:[0.0,0.0,1.0,1.0]};
    let keys=[
        Keyframe {time:0.0,target:AnimationTarget::Opacity(0.0),curve},
        Keyframe {time:0.0,target:AnimationTarget::Offset(0.0,10.0),curve},
        Keyframe {time:0.1,target:AnimationTarget::Opacity(0.0),curve},
        Keyframe {time:0.34,target:AnimationTarget::Opacity(1.0),curve},
        Keyframe {time:0.34,target:AnimationTarget::Offset(0.0,0.0),curve},
    ];
    for round in 0..4 {
        let base=round as f64;
        ui.frame(&FrameIn {time:base,..input()},&mut build).unwrap();
        mem_sys::local_reset();
        assert!(ui.animate_keys(&view.get().unwrap(),&keys));
        for (dt,y) in [(0.0,30.0),(0.05,30.0),(0.22,25.0),(0.34,20.0),(0.5,20.0)] {
            ui.frame(&FrameIn {time:base+dt,..input()},&mut build).unwrap();
            assert_eq!(ui.now(),base+dt);
            let r=ui.rect(&child.get().unwrap());
            assert!((r.y-y).abs()<0.005,"entrance y {r:?} expected {y}");
            let node=ui.a11y_nodes().iter().find(|n|ui.a11y_name_of(n)=="Moving child").unwrap();
            assert!((node.rect.y-y).abs()<0.005);
            assert_eq!(ui.layer(0).instances,instances.as_slice(),"entrance updates the view table");
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
    let mut ui=Ui::new().unwrap();
    let seed=|ui:&mut Ui,label:&str| {
        for index in 0..16 {
            let parent=ui.node_keyed(crate::ui::int_key(0x91b17,index),Node {w:Size::Px(10.0),h:Size::Px(10.0),..Node::DEFAULT});
            ui.within(&parent,&mut |ui| {ui.view(label,1,Size::Grow,Size::Grow,&mut |_| {});});
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
    let mut ui=Ui::new().unwrap();
    ui.set_a11y(true);
    let shown=Cell::new(true);
    let mut build=|ui:&mut Ui| {
        if shown.get() {
            ui.view("panel",1,Size::Grow,Size::Grow,&mut |ui| {
                for (label,count) in [("header",2),("information",10),("clock",1),("queue",24)] {
                    ui.view(label,1,Size::Grow,Size::Px(40.0),&mut |ui| {
                        for _index in 0..count {
                            let b=ui.node("item",Node {flags:CLICKABLE,w:Size::Px(10.0),h:Size::Px(1.0),role:Role::Button,..Node::DEFAULT});
                            ui.a11y_name(&b,"item");
                        }
                    });
                }
            });
        }
    };
    for round in 0..8 {
        mem_sys::local_reset();
        for (step,value) in [true,false,true,false].into_iter().enumerate() {
            shown.set(value);
            ui.frame(&FrameIn {time:(round*4+step) as f64,..input()},&mut build).unwrap();
        }
        if round>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"reopening view kinds"); }
    }
}

#[test]
fn accessibility_action_batches_keep_order_without_warm_allocations() {
    use crate::ui::{Node, Role, FOCUSABLE};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap(); ui.set_a11y(true);
    let value=Cell::new(None);
    let mut build=|ui:&mut Ui| {
        let slider=ui.node("slider",Node {flags:FOCUSABLE,role:Role::Slider,w:Size::Px(200.0),h:Size::Px(20.0),..Node::DEFAULT});
        ui.a11y_name(&slider,"Accessible value");
        ui.a11y_number(&slider,0.0,0.0,100.0,1.0);
        value.set(ui.signal_state(&slider).a11y_value);
    };
    ui.frame(&input(),&mut build).unwrap();
    let id=ui.a11y_nodes().iter().find(|node|ui.a11y_name_of(node)=="Accessible value").unwrap().id;
    let mut batch=[crate::A11yAction {id,action_code:4,..Default::default()};65];
    for cycle in 0..8 {
        for (index,action) in batch.iter_mut().enumerate() {action.payload=if cycle%2==0 {index as f64} else {(64-index) as f64};}
        mem_sys::local_reset();
        ui.inject_a11y(&batch);
        ui.frame(&FrameIn {time:cycle as f64,..input()},&mut build).unwrap();
        assert_eq!(value.get(),Some(if cycle%2==0 {64.0} else {0.0}));
        if cycle>1 {assert_eq!(mem_sys::local_alloc_count(),0,"ordered accessibility chunks");}
    }
}

#[test]
fn line_box_text_fits_wraps_and_resizes_in_one_frame_without_warm_allocations() {
    use crate::{TextStyle, ui::{Node, Role}};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap(); ui.set_a11y(true);
    let style=TextStyle {size_points:12.0,line_height:20.0,wrap:TextStyle::WRAP_WORD,..Default::default()};
    let value="one two three four";
    let mut text=crate::Text::new(&[crate::text::pragmasevka()]).unwrap();
    let expected=[80.0,200.0].map(|width| text.measure(value,&style,width).unwrap().lines as f32*20.0);
    assert!(expected[0]>expected[1],"fixture must wrap");
    let handle=Cell::new(None);
    for round in 0..6 {
        mem_sys::local_reset();
        for (index,width) in [80.0,200.0].into_iter().enumerate() {
            for empty in [false,true] {
                ui.frame(&FrameIn {time:round as f64,..input()},&mut |ui| {
                    ui.view("wrapper",(width as u64)*2+u64::from(empty),Size::Px(width),Size::Fit,&mut |ui| {
                        let node=ui.node("line",Node {role:Role::Label,w:Size::Grow,h:Size::Fit,..Node::DEFAULT});
                        ui.text_line_box(&node,if empty {""} else {value},&style,&[]).unwrap();
                        handle.set(Some(node));
                    });
                }).unwrap();
                let rect=ui.rect(&handle.get().unwrap());
                assert!((rect.w-width).abs()<0.001);
                assert!((rect.h-if empty {20.0} else {expected[index]}).abs()<0.001,"wrapped intrinsic height {rect:?}");
            }
        }
        if round>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"line-box fit/resize reuses layout and glyphs");}
    }
    ui.frame(&input(),&mut |ui| {
        let node=ui.node("checked",Node {role:Role::Label,w:Size::Px(200.0),h:Size::Px(20.0),..Node::DEFAULT});
        ui.text_line_box(&node,value,&style,&[]).unwrap();
        for invalid in [TextStyle {line_height:0.0,..style},TextStyle {line_height:f32::NAN,..style},TextStyle {font:u16::MAX,..style}] {
            assert!(ui.text_line_box(&node,"wrong",&invalid,&[]).is_err());
        }
        assert!(ui.text_line_box(&node,"é",&style,&[crate::TextSpan {start:1,end:2,..Default::default()}]).is_err());
    }).unwrap();
    assert!(ui.a11y_nodes().iter().any(|node|ui.a11y_name_of(node)==value));
}

#[test]
fn measured_section_rows_settle_positions_pin_heading_and_reuse_warm_storage() {
    use std::cell::{Cell,RefCell};
    use crate::ui::{Node,SectionList,SectionPart};
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    let count=Cell::new(10_000);let taller=Cell::new(false);let body=Cell::new(None);let header=Cell::new(None);
    let positions=RefCell::new(Vec::with_capacity(100));
    let mut build=|ui:&mut Ui| {
        positions.borrow_mut().clear();
        body.set(Some(ui.section_list_measured("measured",&SectionList {count:count.get(),row_height:20.0,leading_height:100.0,header_height:30.0,trailing_height:40.0,overlay_scrollbar:true},&mut |ui,part,y| {
            match part {
                SectionPart::Leading=>assert_eq!(y,0.0),
                SectionPart::Header=>{assert_eq!(y,100.0);header.set(Some(ui.node("heading",Node {w:Size::Grow,h:Size::Grow,..Node::DEFAULT})));},
                SectionPart::Row(index)=>{
                    let h=if taller.get() {40.0} else {20.0};
                    let row=ui.node("content",Node {w:Size::Grow,h:Size::Px(h),..Node::DEFAULT});
                    positions.borrow_mut().push((index,y,row));
                }
            }
        })));
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
            assert_eq!(ui.scroll_offset(&body.get().unwrap()),(170.0+2.0*h-240.0).max(0.0));
            assert_eq!(positions.borrow().len(),2);
        }
        if cycle>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"warm measured height changes/scroll/shrink");}
    }
}

#[test]
fn filled_triangles_preserve_vertices_winding_validate_and_allocate_nothing_when_warm() {
    mem_sys::set_peer(crate::library_counter);let mut ui=Ui::new().unwrap();
    for round in 0..8 {
        mem_sys::local_reset();
        for scale in [1.0,2.0] {
            ui.frame(&FrameIn {scale,..input()},&mut |ui| {
                let node=ui.node("triangles",crate::Node {w:Size::Px(100.0),h:Size::Px(100.0),..crate::Node::DEFAULT});
                ui.draw(&node,|paint,_| {
                    let color=crate::geom::Rgba {r:12,g:34,b:56,a:128};
                    paint.triangle((1.25,2.75),(5.75,2.75),(3.5,6.5),color);
                    paint.triangle((1.25,2.75),(3.5,6.5),(5.75,2.75),color);
                    for points in [
                        [(0.0,0.0);3],[(0.0,0.0),(1.0,1.0),(2.0,2.0)],
                        [(f32::NAN,0.0),(1.0,0.0),(2.0,2.0)],[(0.0,0.0),(f32::INFINITY,1.0),(2.0,2.0)],
                        [(-f32::MAX,0.0),(f32::MAX,0.0),(0.0,1.0)],
                        [(0.0,0.0),(1e-30,0.0),(0.0,1.0)],[(0.0,0.0),(1e30,0.0),(0.0,1e30)],
                    ] {paint.triangle(points[0],points[1],points[2],color);}
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
    mem_sys::set_peer(crate::library_counter);let mut ui=Ui::new().unwrap();ui.set_a11y(true);
    for round in 0..8 {
        mem_sys::local_reset();
        for value in ["Not kept offline","Kept offline","Café","", "ready\0now","🎵e\u{301}","👩‍💻"] {
            ui.frame(&input(),&mut |ui| {
                let button=ui.node("value",crate::Node {w:Size::Px(100.0),h:Size::Px(30.0),role:crate::ui::Role::Button,..crate::Node::DEFAULT});
                ui.a11y_name(&button,"Offline");ui.a11y_value(&button,value,(value.len() as u32,0));
            }).unwrap();
            let node=ui.a11y_nodes().iter().find(|node|ui.a11y_name_of(node)=="Offline").unwrap();
            assert_eq!(ui.a11y_value_of(node),if value.is_empty() {None} else {Some(value)});assert_eq!(node.selection,[value.chars().count() as u32,0],"accessibility output uses character offsets, as native text widgets do");
        }
        if round>=3 {assert_eq!(mem_sys::local_alloc_count(),0,"warm accessible value/selection changes");}
    }
    ui.frame(&input(),&mut |ui| {
        let node=ui.node("number",crate::Node {w:Size::Px(100.0),h:Size::Px(30.0),role:crate::ui::Role::Slider,..crate::Node::DEFAULT});
        ui.a11y_value(&node,"one",(0,0));ui.a11y_number(&node,1.0,0.0,2.0,1.0);
    }).unwrap();
    let node=ui.a11y_nodes().iter().find(|node|node.is_role(crate::ui::Role::Slider)).unwrap();assert_eq!(node.value.tag,2);assert_eq!(ui.a11y_value_of(node),None);
    let stale=pxui_types::PxuiBox {index:0,box_key:1,box_slot:99,generation:99};ui.a11y_value(&stale,"stale",(0,0));
    assert!(ui.last_error().contains("stale"));
    assert!(catch_unwind(AssertUnwindSafe(|| {ui.frame(&input(),&mut |ui| {
        let node=ui.node("invalid",crate::Node::DEFAULT);ui.a11y_value(&node,"é",(1,0));
    }).unwrap();})).is_err(),"selection bytes cannot split UTF-8");
}

#[test]
fn host_environment_invalidates_cached_layout_and_reuses_warm_geometry() {
    use std::cell::Cell;
    let mut ui=Ui::new().unwrap();
    mem_sys::set_peer(crate::library_counter);
    let builds=Cell::new(0);
    let seen=Cell::new(crate::Environment::default());
    let mut build=|ui:&mut Ui| {
        ui.view("environment",1,Size::Grow,Size::Grow,&mut |ui| {
            builds.set(builds.get()+1); seen.set(ui.environment()); ui.label("Environment");
        });
    };
    for round in 0..100 {
        let environment=crate::Environment { safe_area:[47.0,0.0,34.0,0.0],keyboard_frame:[0.0,if round%2==0 {500.0} else {844.0},390.0,344.0],
            keyboard_from:[0.0,844.0,390.0,344.0],keyboard_started:1.0,keyboard_duration:0.25,keyboard_curve:7,keyboard_visible:u32::from(round%2==0) };
        mem_sys::local_reset();
        let previous=builds.get();
        ui.frame(&FrameIn { time:round as f64,environment,..input() },&mut build).unwrap();
        assert!(builds.get()>previous,"geometry changes must rebuild a kept view");
        assert_eq!(seen.get(),environment); assert_eq!(ui.environment(),environment);
        let previous=builds.get();
        ui.frame(&FrameIn { time:round as f64,environment,..input() },&mut build).unwrap();
        assert_eq!(builds.get(),previous,"unchanged geometry must keep cached content");
        if round>=10 {assert_eq!(mem_sys::local_alloc_count(),0,"warm environment updates allocated");}
    }
    let prior=ui.environment();
    let mut invalid=prior; invalid.safe_area[0]=f32::NAN;
    assert_eq!(ui.frame(&FrameIn { environment:invalid,..input() },&mut build).err(),Some(pxui_types::PXUI_ERR_ARGUMENT));
    assert_eq!(ui.environment(),prior);
    ui.frame(&FrameIn { size:56,environment:invalid,..input() },&mut build).unwrap();
    assert_eq!(ui.environment(),crate::Environment::default(),"an old complete descriptor ignores the new tail");
}

#[test]
fn typed_touch_batches_keep_ids_phases_lifetimes_and_zero_warm_allocations() {
    use crate::{Touch,Event,Str};
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    let mut packets=[Touch::default();32];
    let mut events=[Event::default();32];
    for frame in 0..182 {
        let count=[32,1,0,3][frame%4];
        for (index,packet) in packets.iter_mut().enumerate() {
            *packet=Touch {device:u64::MAX,finger:u64::MAX-index as u64,x:-1.25,y:99.5,
                time:frame as f64/120.,pressure:0.375,phase:(frame%4) as u32};
            events[index]=Event {kind:Event::TOUCH,text:Str {ptr:(packet as *const Touch).cast(),len:core::mem::size_of::<Touch>()},..Event::default()};
        }
        let expected=packets;
        mem_sys::local_reset();
        ui.frame(&FrameIn {events:events.as_ptr(),event_count:count,time:frame as f64/120.,..input()},&mut |_| {}).unwrap();
        // The library must own the decoded value after the call returns.
        packets.fill(Touch::default());
        let routed=ui.routed_input(None);
        assert_eq!(routed.events().len(),count);
        for (index,packet) in expected[..count].iter().enumerate() {
            assert_eq!(routed.touch(index),Some(*packet));
            assert_eq!(crate::event_text(&routed.events()[index]),"");
        }
        assert_eq!(routed.touch(count),None);
        if frame>=82 {assert_eq!(mem_sys::local_alloc_count(),0,"warm typed touch frame and routed read allocated");}
    }
}

#[test]
fn touch_gestures_drive_virtual_rows_catch_coasts_and_keep_warm_allocations_zero() {
    use crate::{Event, Str, Touch};
    use crate::ui::{Node, SectionList, SectionPart};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    ui.set_a11y(true);
    let body = Cell::new(None);
    let clicked = Cell::new(None);
    let built = Cell::new(0);
    let mut build = |ui: &mut Ui| {
        built.set(0);
        ui.view("touch-page", 0, Size::Grow, Size::Grow, &mut |ui| {
            body.set(Some(ui.section_list("touch-list", &SectionList {
                count: 10_000, row_height: 24.0, leading_height: 0.0, header_height: 0.0,
                trailing_height: 0.0, overlay_scrollbar: true,
            }, &mut |ui, part| {
                if let SectionPart::Row(index) = part {
                    built.set(built.get() + 1);
                    let row = ui.node("touch-row", Node { flags: CLICKABLE, w: Size::Grow, h: Size::Grow, ..Node::DEFAULT });
                    ui.a11y_name(&row, "Song");
                    let signal = ui.signal_state(&row);
                    if signal.clicked { clicked.set(Some(index)); }
                    assert!(!signal.hovered, "finger input must not leave mouse hover on a row");
                }
            })));
        });
    };
    let mut packet = Touch { device: u64::MAX, finger: 7, x: 50.0, y: 100.0, pressure: 1.0, ..Touch::default() };
    // Start outside the list so the initial host mouse position does not hover it.
    let outside = Event { kind: Event::POINTER_MOVED, x: -1.0, y: -1.0, ..Event::default() };
    ui.frame(&FrameIn { events: &outside, event_count: 1, ..input() }, &mut build).unwrap();
    let send = |ui: &mut Ui, packet: &Touch, build: &mut dyn FnMut(&mut Ui)| {
        let event = Event { kind: Event::TOUCH, text: Str { ptr: (packet as *const Touch).cast(), len: core::mem::size_of::<Touch>() }, ..Event::default() };
        ui.frame(&FrameIn { time: packet.time, events: &event, event_count: 1, ..input() }, build).unwrap();
    };
    send(&mut ui, &packet, &mut build);
    packet.phase = 2; packet.time = 0.01;
    send(&mut ui, &packet, &mut build);
    assert!(clicked.take().is_some(), "a touch tap did not activate a real virtual row");
    for (phase, y, time) in [(0,200.0,0.02),(1,-200.0,0.03),(1,-300.0,0.04),(3,-300.0,0.05)] {
        packet.phase = phase; packet.y = y; packet.time = time;
        send(&mut ui, &packet, &mut build);
    }
    assert!(ui.scroll_offset(&body.get().unwrap()) >= 499.0, "retiring the original row cancelled its ancestor scroll capture");
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
    assert!(ui.scroll_offset(&body.get().unwrap()) > 100.0);
}

#[test]
fn keyboard_presentation_updates_cached_content_bounds_without_warm_allocations() {
    use crate::{Environment, Keyboard};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    let node = Cell::new(None);
    let builds = Cell::new(0);
    let expected = Cell::new([0.0, 47.0, 390.0, 763.0]);
    let mut build = |ui: &mut Ui| {
        ui.view("keyboard-content", 0, Size::Grow, Size::Grow, &mut |ui| {
            builds.set(builds.get()+1);
            let rect = ui.content_rect();
            assert_eq!([rect.x, rect.y, rect.w, rect.h], expected.get());
            node.set(Some(ui.node("visible content", crate::Node {
                at: Some((rect.x, rect.y)).into(), w: Size::Px(rect.w), h: Size::Px(rect.h),
                ..crate::Node::DEFAULT
            })));
        });
    };
    let environment = Environment {safe_area:[47.0,0.0,34.0,0.0],..Environment::default()};
    for frame in 0..182 {
        let y = [810.0,750.0,600.0,508.0,600.0,750.0][frame%6];
        let flags = Keyboard::VALID | if matches!(frame%6,0|3) {0} else {Keyboard::ANIMATING};
        let keyboard = Keyboard {frame:[0.0,y,390.0,844.0-y],flags};
        expected.set([0.0,47.0,390.0,y.min(810.0)-47.0]);
        mem_sys::local_reset();
        ui.frame(&FrameIn {width:390.0,height:844.0,time:frame as f64/120.0,environment,keyboard,..FrameIn::default()}, &mut build).unwrap();
        assert_eq!(ui.keyboard(), keyboard);
        let rect=ui.rect(&node.get().unwrap());
        assert_eq!([rect.x,rect.y,rect.w,rect.h],expected.get());
        assert_eq!(ui.repaint_at().is_some(),flags & Keyboard::ANIMATING != 0);
        let before=builds.get();
        ui.frame(&FrameIn {width:390.0,height:844.0,time:frame as f64/120.0,environment,keyboard,..FrameIn::default()}, &mut build).unwrap();
        assert_eq!(builds.get(),before,"unchanged keyboard geometry rebuilt a cached view");
        if frame>=82 {assert_eq!(mem_sys::local_alloc_count(),0,"warm keyboard frame {frame}");}
    }
    for (keyboard,height) in [
        (Keyboard {frame:[100.0,400.0,200.0,200.0],flags:Keyboard::VALID},763.0),
        (Keyboard {frame:[0.0,900.0,390.0,100.0],flags:Keyboard::VALID},763.0),
        (Keyboard {frame:[0.0,-100.0,390.0,1000.0],flags:Keyboard::VALID},0.0),
    ] {
        expected.set([0.0,47.0,390.0,height]);
        ui.frame(&FrameIn {width:390.0,height:844.0,environment,keyboard,..FrameIn::default()},&mut build).unwrap();
    }
}

#[test]
fn text_field_refocus_reuses_its_warm_editor_storage() {
    use pxui_types::{Event, KEY_SPECIAL};
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    let mut build = |ui: &mut Ui| assert!(ui.text_field("Caption", "café track 10").is_none());
    let focus = [
        Event {kind:Event::KEY_PRESSED,code:KEY_SPECIAL+8,..Event::default()},
        Event {kind:Event::KEY_RELEASED,code:KEY_SPECIAL+8,..Event::default()},
    ];
    ui.frame(&input(), &mut build).unwrap();
    for frame in 0..182 {
        mem_sys::local_reset();
        let focusing = matches!(frame, 1 | 90);
        let focused = (1..40).contains(&frame) || (90..130).contains(&frame);
        if matches!(frame,40|130) { ui.unfocus(); }
        ui.frame(&FrameIn {
            time:frame as f64/60.0,
            events:if focusing {focus.as_ptr()} else {std::ptr::null()},
            event_count:if focusing {focus.len()} else {0},
            ..input()
        }, &mut build).unwrap();
        assert_eq!(ui.text_input_focused(),focused);
        if frame>=82 {assert_eq!(mem_sys::local_alloc_count(),0,"warm field refocus frame {frame}");}
    }
}

#[test]
fn text_input_options_follow_the_focused_field_and_done_keeps_warm_storage() {
    use crate::{Event, TextInputOptions as Options};
    use std::cell::Cell;
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    let show_search=Cell::new(true);
    let done=Options {autocorrect:0,capitalization:0,action:Options::DONE};
    let mut build=|ui:&mut Ui| {
        ui.view("input options",show_search.get() as u64,Size::Grow,Size::Grow,&mut |ui| {
            if show_search.get() { assert!(ui.text_field_with_options("Search","café",Options::SEARCH_FIELD).is_none()); }
            assert!(ui.text_field_with_options("Name","café",done).is_none());
        });
    };
    ui.frame(&input(),&mut build).unwrap();
    for frame in 0..182 {
        let step=frame%6;
        let code=pxui_types::KEY_SPECIAL+if matches!(step,0|2) {8} else {5};
        let events=[Event {kind:Event::KEY_PRESSED,code,..Default::default()},Event {kind:Event::KEY_RELEASED,code,..Default::default()}];
        let active=matches!(step,0|2|4);
        mem_sys::local_reset();
        ui.frame(&FrameIn {time:frame as f64,events:events.as_ptr(),event_count:if active {2} else {0},..input()},&mut build).unwrap();
        let expected=match step {0|1=>Options::SEARCH_FIELD,2|3=>done,_=>Options::default()};
        assert_eq!(ui.text_input_options(),expected,"frame {frame}");
        assert_eq!(ui.ime().is_some(),step<4,"Done must dismiss native input in the same frame");
        if step==4 {assert!(ui.key_pressed(code),"raw key queries remain non-consuming");}
        if frame>=82 {assert_eq!(mem_sys::local_alloc_count(),0,"warm keyboard options frame {frame}");}
    }
    show_search.set(false);
    ui.frame(&input(),&mut build).unwrap();
    assert_eq!(ui.text_input_options(),Options::default(),"an unfocused or omitted field cannot retain options");
}

#[test]
fn marked_text_replaces_the_selection_and_tracks_unicode_caret_without_warm_allocations() {
    use crate::{Event,Part,Str};
    fn body(ui:&mut Ui,c:&crate::bodies::PxuiTextFieldConfig) {
        ui.set_look(&c.row,crate::theme::Look::NONE,crate::theme::State::default());
        let editor=ui.part(Part::EDITOR,crate::Node {w:Size::Px(240.0),h:Size::Px(30.0),
            look:crate::theme::Look {font:crate::theme::Font {size:18,..crate::theme::Font::BODY},..crate::theme::Look::NONE},..crate::Node::DEFAULT});
        if c.state.focused==0 {ui.text(&editor,crate::bodies::text_of(&c.value));}
    }
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();
    ui.register_text_field_body("marked-editor",body).unwrap();
    let source="text_field.body = marked-editor";
    ui.set_theme(&crate::ThemeSource {source:Str {ptr:source.as_ptr(),len:source.len()},..Default::default()}).unwrap();
    let mut build=|ui:&mut Ui|assert!(ui.text_field("Marked","café track").is_none());
    ui.frame(&input(),&mut build).unwrap();
    let key=|kind,n|Event {kind,code:pxui_types::KEY_SPECIAL+n,..Default::default()};
    let focus=[key(Event::KEY_PRESSED,8),key(Event::KEY_RELEASED,8),key(Event::KEY_PRESSED,12),
        Event {kind:Event::KEY_PRESSED,code:'a' as u32,..Default::default()},key(Event::KEY_RELEASED,12)];
    ui.frame(&FrameIn {events:focus.as_ptr(),event_count:focus.len(),..input()},&mut build).unwrap();
    let value="éix";
    for (start,length) in [(f64::from(i32::MAX),f64::from(i32::MAX)),(1.0,1.0)] {
        let event=Event {kind:Event::TEXT_EDITING,text:Str {ptr:value.as_ptr(),len:value.len()},x:start,y:length,..Default::default()};
        ui.frame(&FrameIn {events:&event,event_count:1,..input()},&mut build).unwrap();
        let prefix=if start>3.0 {value} else {"éi"};
        assert_eq!(ui.ime().unwrap().1,(2.0+ui.text_width(Some(18),prefix)).round() as i64,"marked selection uses Unicode scalar offsets");
    }
    for frame in 0..182 {
        mem_sys::local_reset();
        ui.frame(&FrameIn {time:frame as f64/120.0,..input()},&mut build).unwrap();
        assert_eq!(ui.layers().flat_map(|layer|layer.instances.iter()).filter(|quad|quad.kind==crate::draw::kind::MASK).count(),3,
            "preedit must replace the selected base text instead of appending to it");
        assert_eq!(ui.ime().unwrap().1,(2.0+ui.text_width(Some(18),"éi")).round() as i64);
        if frame>=82 {assert_eq!(mem_sys::local_alloc_count(),0,"warm marked selection frame {frame}");}
    }
    let commit=[Event {kind:Event::TEXT_INPUT,text:Str {ptr:value.as_ptr(),len:value.len()},..Default::default()},key(Event::KEY_PRESSED,5),key(Event::KEY_RELEASED,5)];
    let mut changed=false;
    ui.frame(&FrameIn {events:commit.as_ptr(),event_count:commit.len(),..input()},&mut |ui| {
        changed=ui.text_field_with_options("Marked","café track",crate::TextInputOptions {action:crate::TextInputOptions::DONE,..Default::default()})==Some(value);
    }).unwrap();
    assert!(changed,"commit replaces precisely the original UTF-8 selection");
    assert!(ui.ime().is_none());
    assert_eq!(ui.layers().flat_map(|layer|layer.instances.iter()).filter(|quad|quad.kind==crate::draw::kind::MASK).count(),3,"commit and Done paint the latest value in the same frame");
}

#[test]
fn keyless_text_measurement_retains_interleaved_values_styles_widths_and_spans() {
    mem_sys::set_peer(crate::library_counter);
    let mut ui = Ui::new().unwrap();
    let mut text = crate::Text::new(&[crate::text::pragmasevka()]).unwrap();
    let plain = crate::TextStyle { size_points:14.0, line_height:20.0, wrap:crate::TextStyle::WRAP_WORD, ..Default::default() };
    let cases = [
        ("1111", plain, 0.0),
        ("88888", plain, 0.0),
        ("1111", crate::TextStyle { features:crate::TextStyle::FEATURE_TABULAR_NUMBERS, ..plain }, 0.0),
        ("office café words", plain, 40.0),
        ("office café words", plain, 90.0),
        ("office café words", crate::TextStyle { weight:700, tracking:0.2, ..plain }, 40.0),
    ];
    let expected = cases.map(|(value,style,width)| text.measure(value,&style,width).unwrap());
    assert_ne!(expected[0].width,expected[1].width);
    assert_ne!(expected[3].lines,expected[4].lines);
    let spans = [crate::TextSpan { start:0, end:6, weight:700, ..Default::default() }];
    let rich = text.measure_spans(cases[3].0,&plain,40.0,&spans).unwrap();
    for round in 0..6 {
        mem_sys::local_reset();
        ui.frame(&input(), &mut |ui| {
            for (index,(value,style,width)) in cases.into_iter().enumerate() {
                assert_eq!(text.measure(value,&style,width).unwrap(),expected[index]);
                assert_eq!(text.width(value,&style,width).unwrap(),expected[index].width);
                assert_eq!(ui.text_service().measure(value,&style,width).unwrap(),expected[index]);
            }
            assert_eq!(text.measure_spans(cases[3].0,&plain,40.0,&spans).unwrap(),rich);
            assert_eq!(ui.text_service().measure_spans(cases[3].0,&plain,40.0,&spans).unwrap(),rich);
        }).unwrap();
        if round>=3 { assert_eq!(mem_sys::local_alloc_count(),0,"keyless layout identity must retain every interleaved variant"); }
    }
}

#[test]
fn pixel_fit_text_rounds_density_and_reuses_recycled_identity_style_and_span_layouts() {
    use crate::{TextStyle,TextSpan,ui::{Node,Look,Font,Role,FIT_TEXT,FIT_TEXT_PIXEL}};
    mem_sys::set_peer(crate::library_counter);
    let mut ui=Ui::new().unwrap();ui.set_a11y(true);
    let mut text=crate::Text::new(&[crate::text::pragmasevka()]).unwrap();
    let span=[TextSpan {start:0,end:2,weight:700,..Default::default()}];
    let mut nodes=[None;9];
    let look=Look {text:crate::geom::color::rgba(0,0,0,255),..Look::NONE};
    let values=["ab cd","écho","a slightly longer label",""];
    let styles=[TextStyle {size_points:14.25,line_height:20.0,..Default::default()},TextStyle {size_points:16.5,line_height:22.0,tracking:0.15,..Default::default()}];
    let widths=styles.map(|style|values.map(|value| text.measure(value,&style,0.0).unwrap().width));
    let span_width=text.measure_spans(values[0],&styles[0],0.0,&span).unwrap().width;
    let plain_width=text.width("AB CD",&TextStyle {size_points:14.0,..Default::default()},0.0).unwrap();
    let mut serial=0;
    for round in 0..8 {
        for scale in [1.0,2.0,1.25,1.75] {
            for (variant,style) in styles.into_iter().enumerate() {
                mem_sys::local_reset();serial+=1;
                ui.frame(&FrameIn {scale,time:serial as f64/60.0,..input()},&mut |ui| {
                    for (index,value) in values.into_iter().enumerate() {
                        let node=ui.node_keyed(index as i64+100+100*(round%2) as i64,Node {flags:FIT_TEXT|FIT_TEXT_PIXEL,w:Size::Fit,h:Size::Px(24.0),role:Role::Label,look,..Node::DEFAULT});
                        ui.text_line_box(&node,value,&style,&[]).unwrap();nodes[index]=Some(node);
                    }
                    let node=ui.node("spans",Node {flags:FIT_TEXT_PIXEL,w:Size::Fit,h:Size::Px(24.0),look,..Node::DEFAULT});
                    ui.text_spans(&node,values[0],&styles[0],&span).unwrap();nodes[4]=Some(node);
                    let node=ui.node("plain-caps",Node {flags:FIT_TEXT_PIXEL,w:Size::Fit,h:Size::Px(24.0),role:Role::Label,look:Look {font:Font {size:14,caps:true,..Font::BODY},..look},..Node::DEFAULT});
                    ui.text(&node,values[0]);nodes[5]=Some(node);
                    let node=ui.node("limited",Node {flags:FIT_TEXT_PIXEL,w:Size::Fit,max_w:10.25,h:Size::Px(24.0),look,..Node::DEFAULT});
                    ui.text_line_box(&node,values[2],&style,&[]).unwrap();nodes[6]=Some(node);
                    let node=ui.node("insets",Node {flags:FIT_TEXT_PIXEL,w:Size::Fit,h:Size::Px(24.0),inset:crate::ui::Edges {l:0.2,r:0.3,..Default::default()},look,..Node::DEFAULT});
                    ui.text_line_box(&node,values[0],&style,&[]).unwrap();nodes[7]=Some(node);
                    let node=ui.node("plain-limited",Node {flags:FIT_TEXT|FIT_TEXT_PIXEL,w:Size::Fit,max_w:10.25,h:Size::Px(24.0),look:Look {text:crate::geom::color::rgba(255,0,0,255),font:Font {size:14,..Font::BODY},..Look::NONE},..Node::DEFAULT});
                    ui.text(&node,values[0]);nodes[8]=Some(node);
                }).unwrap();
                let density=ui.density() as f32;
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
    let mut ui = Ui::new().unwrap();
    let mut shaper = crate::Text::new(&[crate::text::pragmasevka()]).unwrap();
    let style = pxui_types::TextStyle::default();
    let color = [1, 2, 3, 255];
    let mut glyphs = Vec::new();
    let mut ui_pixels = Vec::new();
    let mut shaper_pixels = Vec::new();
    for _ in 0..2 {
        ui.text_service().quads_into("abc def", &style, 0.0, (3.0, 4.0), 1.0, &mut glyphs).unwrap();
        ui.text_service().rasterize_rgba_into("abc def", &style, 0.0, 1.0, color, &mut ui_pixels).unwrap();
        shaper.rasterize_rgba_into("abc def", &style, 0.0, 1.0, color, &mut shaper_pixels).unwrap();
    }
    assert_eq!(glyphs, ui.text_service().quads("abc def", &style, 0.0, (3.0, 4.0), 1.0).unwrap());
    let (w, h, old) = ui.text_service().rasterize_rgba("abc def", &style, 0.0, 1.0, color).unwrap();
    assert_eq!(ui.text_service().rasterize_rgba_into("abc def", &style, 0.0, 1.0, color, &mut ui_pixels), Ok((w, h)));
    assert_eq!(ui_pixels, old);
    assert_eq!(shaper.rasterize_rgba("abc def", &style, 0.0, 1.0, color).unwrap(), (w, h, shaper_pixels.clone()));
    let scope = mem_sys::warm_scope();
    mem_sys::local_reset();
    ui.text_service().quads_into("abc def", &style, 0.0, (3.0, 4.0), 1.0, &mut glyphs).unwrap();
    ui.text_service().rasterize_rgba_into("abc def", &style, 0.0, 1.0, color, &mut ui_pixels).unwrap();
    shaper.rasterize_rgba_into("abc def", &style, 0.0, 1.0, color, &mut shaper_pixels).unwrap();
    assert_eq!(mem_sys::local_alloc_count(), 0);
    drop(scope);
    assert!(mem_sys::take_reports().is_empty());
}
