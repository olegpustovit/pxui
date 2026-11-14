//! The engine-shaped surface through the wrapper: queries, boxes, patches, scroll and text state, carries, view calls
//! and the loud failure of a caller bug.
use crate::Ui;
use crate::opts::BoxOpts;
use pxui_types::{geom::Rect, ui::{Cursor, Drop, UndoKind, CLICKABLE, CLIP, SCROLL}, FrameIn, Mark, Size};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn input() -> FrameIn { FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() } }

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
                let base=index as u64*8+100;
                let a=ui.text_service().measure(base,value,&plain,50.0).unwrap();
                let b=ui.text_service().measure(base+1,value,&logical,50.0).unwrap();
                assert_eq!((a.height,a.lines,a.baseline),(b.height,b.lines,b.baseline));
                assert!(b.width>=a.width && b.width.is_finite());
                if index==0 {
                    let space=ui.text_service().width(base+2," ",&plain,0.0).unwrap();
                    assert!((b.width-a.width-space).abs()<0.001,"the longest wrapped word retains its break space");
                }
                if round==0 {
                    let a=ui.text_service().quads(base,value,&plain,50.0,(0.0,0.0),1.0).unwrap();
                    let b=ui.text_service().quads(base+1,value,&logical,50.0,(0.0,0.0),1.0).unwrap();
                    assert_eq!(a,b,"separator metrics leave visible glyphs unchanged");
                }
                let plain=crate::TextStyle {max_lines:1,..plain};
                let logical=crate::TextStyle {max_lines:1,..logical};
                assert_eq!(ui.text_service().measure(base+3,value,&plain,50.0).unwrap(),ui.text_service().measure(base+4,value,&logical,50.0).unwrap(),"hidden paragraphs must not contribute trailing spaces");
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
        assert_eq!(ui.text_state(&b).as_deref(), Some("kept"));
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
    let hints = [("Esc".to_string(), "close".to_string())];
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
                assert_eq!((text.as_str(), step), ("abc", LineStep::None));
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
    let metrics = text.measure(1, "abc", &style, 0.0).expect("measure");
    assert!(metrics.width > 0.0 && metrics.lines == 1);
    let glyphs = text.quads(1, "abc", &style, 0.0, (10.0, 20.0), 1.0).expect("quads");
    assert_eq!(glyphs.len(), 3, "one quad per glyph");
    assert!(glyphs.iter().all(|g| g.rect[0] >= 10.0));
    let (w, h, pixels) = text.rasterize_rgba(2, "abc", &style, 0.0, 1.0, [255, 255, 255, 255]).expect("rasterize");
    assert_eq!(pixels.len(), (w * h * 4) as usize);
    let mut bad = style;
    bad.size_points = f32::NAN;
    assert!(text.measure(3, "abc", &bad, 0.0).is_err(), "a NaN size is refused");
}

#[test]
fn second_atlas_pages_reach_public_quads_and_both_rasterizers() {
    let mut ui=Ui::new().unwrap();
    let style=crate::TextStyle { size_points:600.0,..Default::default() };
    let mut text=ui.text_service();
    let first=text.quads(1,"A",&style,0.0,(0.0,0.0),2.0).unwrap();
    let glyphs=text.quads(1,"ABCDEF",&style,0.0,(0.0,0.0),2.0).unwrap();
    assert_eq!(glyphs.len(),6);
    assert_eq!(glyphs[0],first[0],"page growth preserves the first public quad");
    assert!(glyphs.iter().any(|g|g.atlas==0) && glyphs.iter().any(|g|g.atlas==2));
    assert!(glyphs.iter().all(|g|g.atlas==0 || g.atlas==2));
    let (w,h,pixels)=text.rasterize_rgba(1,"ABCDEF",&style,0.0,2.0,[17,33,65,255]).unwrap();
    let mut shaper=crate::Text::new(&[crate::text::pragmasevka()]).unwrap();
    let (sw,sh,rgba)=shaper.rasterize_rgba(1,"ABCDEF",&style,0.0,2.0,[17,33,65,255]).unwrap();
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
    assert_eq!(text.quads(1,"ABCDEF",&style,0.0,(0.0,0.0),2.0).unwrap(),glyphs);
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
                assert_eq!((text.as_str(), editing), ("0.25", false));
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
    let metrics = text.measure(1, "abc", &style, 0.0).expect("measure");
    assert!(metrics.width > 0.0);
    // the same text measures the same in the Ui's own service: one font, two homes
    let mut ui = Ui::new().expect("library");
    assert_eq!(ui.text_service().measure(1, "abc", &style, 0.0).expect("service").width, metrics.width);
    let (w, h, pixels) = text.rasterize_rgba(2, "abc", &style, 0.0, 1.0, [255, 255, 255, 255]).expect("rasterize");
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
        assert_eq!(text.len(), source.len(), "the area returns the whole text");
    }).expect("frame");
}


#[test]
fn tabular_digits_reach_shaping_and_keep_warm_layouts_allocation_free() {
    const FONT: &[u8]=include_bytes!("../../../../assets/fonts/Inter-Regular.ttf");
    mem_sys::set_peer(crate::library_counter);
    let mut text=crate::Text::new(&[FONT]).expect("shaper");
    let plain=pxui_types::TextStyle { size_points:20.0,..Default::default() };
    let tabular=pxui_types::TextStyle { features:pxui_types::TextStyle::FEATURE_TABULAR_NUMBERS,..plain };
    let proportional=text.measure(1,"1111",&plain,0.0).unwrap();
    let tabular_one=text.measure(1,"1111",&tabular,0.0).expect("tabular feature is accepted");
    let tabular_eight=text.measure(2,"8888",&tabular,0.0).unwrap();
    assert!((proportional.width-tabular_one.width).abs()>1.0,"font fixture must exercise a real substitution");
    assert!((tabular_one.width-tabular_eight.width).abs()<1e-5,"equal digit counts have equal advances");
    assert_eq!(text.measure(1,"1111",&plain,0.0).unwrap(),proportional,"feature changes invalidate the caller-keyed cache");
    text.measure(1,"1111",&tabular,0.0).unwrap();
    for _ in 0..4 {
        mem_sys::local_reset();
        assert_eq!(text.measure(1,"1111",&tabular,0.0).unwrap(),tabular_one);
        assert_eq!(text.measure(2,"8888",&tabular,0.0).unwrap(),tabular_eight);
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
    let a=ui.text_service().measure(1,"1111",&plain,0.0).unwrap();
    let b=ui.text_service().measure(2,"1111",&tabular,0.0).unwrap();
    assert!((a.width-b.width).abs()>1.0);
    for features in [2,3,u32::MAX] {
        assert_eq!(ui.text_service().measure(1,"1111",&TextStyle { features,..plain },0.0),Err(pxui_types::PXUI_ERR_ARGUMENT));
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
    let measured=shaper.measure_spans(1,text,&style,200.0,&[span]).unwrap();
    assert_eq!(ui.text_service().measure_spans(1,text,&style,200.0,&[span]).unwrap(),measured);
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
        assert!(ui.text_service().measure_spans(1,"é!",&style,0.0,&[bad]).is_err());
    }
    assert!(ui.text_service().measure_spans(1,"abcdefghij",&style,0.0,&[red;9]).is_err());
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
    let metrics=ui.text_service().measure_spans(1,"aa bb cc",&style,45.0,&[span]).unwrap();
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
        let a=shaper.measure(1,source,&style,100.0).unwrap();
        assert_eq!(a,ui.text_service().measure(1,source,&style,100.0).unwrap());
        let raster=shaper.rasterize_rgba(1,source,&style,100.0,1.0,[0,0,0,255]).unwrap();
        assert_eq!(raster,ui.text_service().rasterize_rgba(1,source,&style,100.0,1.0,[0,0,0,255]).unwrap());
        let glyphs=ui.text_service().quads(1,source,&style,100.0,(0.0,0.0),1.0).unwrap();
        assert!(!glyphs.is_empty());
        if ellipsis!=TextStyle::ELLIPSIS_NONE { assert!(a.width<=100.001 && glyphs.len()<26); }
        if let Some(previous)=previous { assert_ne!(raster,previous,"mode changes invalidate the same caller key"); }
        previous=Some(raster);
    }
    for ellipsis in [4,255] {
        let style=TextStyle { ellipsis,..Default::default() };
        assert_eq!(shaper.measure(1,source,&style,100.0),Err(pxui_types::PXUI_ERR_ARGUMENT));
        assert_eq!(ui.text_service().measure(1,source,&style,100.0),Err(pxui_types::PXUI_ERR_ARGUMENT));
        assert!(shaper.rasterize_rgba(1,source,&style,100.0,1.0,[0,0,0,255]).is_err());
        assert!(ui.text_service().quads(1,source,&style,100.0,(0.0,0.0),1.0).is_err());
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
    let old=text.rasterize_rgba(1,"漢字",&style,300.0,1.0,[0,0,0,255]).unwrap();
    text.enable_system_fonts().unwrap();
    ui.enable_system_fonts().unwrap();
    let new=text.rasterize_rgba(1,"漢字",&style,300.0,1.0,[0,0,0,255]).unwrap();
    assert_ne!(old,new,"enabling invalidates an earlier missing-font layout");
    let cases=["漢字","العربية","😀"];
    let mut measured=[pxui_types::TextMetrics::default();3];
    for (i,source) in cases.into_iter().enumerate() {
        measured[i]=text.measure(i as u64+1,source,&style,300.0).unwrap();
        assert_eq!(measured[i],ui.text_service().measure(i as u64+1,source,&style,300.0).unwrap());
        let (w,h,rgba)=text.rasterize_rgba(i as u64+1,source,&style,300.0,1.0,[0,0,0,255]).unwrap();
        assert!(w>0 && h>0 && rgba.chunks_exact(4).any(|p|p[3]>0),"native fallback rasterizes {source}");
    }
    for _ in 0..4 {
        mem_sys::local_reset();
        text.enable_system_fonts().unwrap();
        ui.enable_system_fonts().unwrap();
        for (i,source) in cases.into_iter().enumerate() {
            assert_eq!(text.measure(i as u64+1,source,&style,300.0).unwrap(),measured[i]);
            assert_eq!(ui.text_service().measure(i as u64+1,source,&style,300.0).unwrap(),measured[i]);
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
            assert_eq!(ui.signal(&ui.box_of_key(ROW.get()).unwrap()).keys.len(),keys.len(),"editing must restore the original key stream for later consumers");
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
        assert!(state.keys.is_empty());
        clicks.set(clicks.get() + i32::from(state.clicked));
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
    assert_eq!(clicks.get(), 60);
    let node = button.get().unwrap();
    let owned = ui.signal(&node);
    assert!(owned.clicked);
    assert_eq!(owned.keys.len(), 4);
    for (actual, expected) in owned.keys.iter().zip(events) {
        assert_eq!((actual.kind, actual.code), (expected.kind, expected.code));
    }
    let text = "café";
    let events = [Event { kind: Event::TEXT_EDITING, x: 1.0, y: 2.0,
        text: pxui_types::Str { ptr: text.as_ptr(), len: text.len() }, ..Default::default() },
        Event { kind: Event::TEXT_INPUT, text: pxui_types::Str { ptr: text.as_ptr(), len: text.len() }, ..Default::default() }];
    ui.frame(&FrameIn { time: 3.0, events: events.as_ptr(), event_count: events.len(), ..input() }, &mut build).unwrap();
    let owned = ui.signal(&node);
    assert_eq!(owned.keys.len(), 2);
    assert_eq!((owned.keys[0].kind, owned.keys[0].x, owned.keys[0].y, owned.keys[0].text.as_str()), (Event::TEXT_EDITING, 1.0, 2.0, text));
    assert_eq!((owned.keys[1].kind, owned.keys[1].text.as_str()), (Event::TEXT_INPUT, text));
    ui.frame(&FrameIn { time: 3.1, ..input() }, &mut |_| {}).unwrap();
    assert!(ui.signal_state(&node).keys.is_empty() && !ui.signal_state(&node).clicked);
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
                assert_eq!(ui.press_keys(&list.get().unwrap()), vec![KEY_SPECIAL + 9], "capture transfer preserves the press modifiers through release");
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
    let expected=[80.0,200.0].map(|width| text.measure(1,value,&style,width).unwrap().lines as f32*20.0);
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
