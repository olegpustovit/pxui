//! The export macro's panic containment and the guards that keep a callback from re-entering the library.
use crate::state::{PxuiUi, panicked};
use crate::*;
use core::ffi::c_void;

fn create() -> *mut PxuiUi { unsafe { pxui_ui_create(&PxuiUiDesc::default()) } }
fn frame_in() -> FrameIn { FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() } }
fn error(ui: *mut PxuiUi) -> String { let e = unsafe { pxui_last_error(ui) }; String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(e.ptr, e.len) }).into_owned() }
fn run(ui: *mut PxuiUi, build: Build, user: *mut c_void) -> i32 {
    let mut out = FrameOut::default();
    unsafe { pxui_frame(ui, &frame_in(), build, user, &mut out) }
}
extern "C" fn nothing(_: *mut c_void, _: *mut c_void) {}
extern "C" fn count(user: *mut c_void, _: *mut c_void) { unsafe { *user.cast::<u32>() += 1; } }

#[test]
fn an_engine_panic_through_an_export_returns_the_error_and_poisons_the_ui() {
    let ui = create();
    // declaring a node outside a frame makes the engine panic ("boxes must be built inside Ui.frame")
    let desc = pxui_types::Node::DEFAULT;
    let mut out = PxuiBox::default();
    assert_eq!(unsafe { pxui_ui_node_keyed(ui, 7, &desc, &mut out) }, PXUI_ERR_PANIC);
    assert!(error(ui).starts_with("panic: "), "{}", error(ui));
    // every later call on this Ui fails at once with the same code, even valid ones
    assert_eq!(unsafe { pxui_ui_set_dir(ui, 0) }, PXUI_ERR_PANIC);
    assert_eq!(unsafe { pxui_ui_request_frame(ui) }, PXUI_ERR_PANIC);
    let mut pressed = 0;
    assert_eq!(unsafe { pxui_button(ui, Str { ptr: b"x".as_ptr(), len: 1 }, &mut pressed) }, PXUI_ERR_PANIC);
    // another Ui is untouched
    let other = create();
    assert_eq!(unsafe { pxui_ui_set_dir(other, 0) }, 0);
    // the next frame call reports the panic, skips that frame and clears the flag: the one after renders
    let mut built = 0u32;
    assert_eq!(run(ui, Some(count), (&mut built as *mut u32).cast()), PXUI_ERR_PANIC);
    assert_eq!(built, 0);
    assert_eq!(unsafe { pxui_ui_set_dir(ui, 0) }, 0);
    assert_eq!(run(ui, Some(count), (&mut built as *mut u32).cast()), 0);
    assert_eq!(built, 1);
    assert_eq!(unsafe { pxui_ui_destroy(ui) } | unsafe { pxui_ui_destroy(other) }, 0);
}

#[test]
fn a_poisoned_ui_can_still_be_read_and_destroyed() {
    let ui = create();
    unsafe { panicked((*ui).shared, Box::new("injected"), false); }
    assert_eq!(error(ui), "panic: injected");
    assert!(!unsafe { pxui_ui_output(ui) }.is_null());
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

#[test]
fn a_panic_inside_a_frame_fails_the_rest_of_the_frame_and_the_frame_itself() {
    extern "C" fn poison(_: *mut c_void, ui: *mut c_void) {
        let ui = ui.cast::<PxuiUi>();
        unsafe { panicked((*ui).shared, Box::new("mid-frame"), false); }
        assert_eq!(unsafe { pxui_ui_set_dir(ui, 0) }, PXUI_ERR_PANIC);
    }
    let ui = create();
    assert_eq!(run(ui, Some(poison), core::ptr::null_mut()), PXUI_ERR_PANIC);
    assert_eq!(run(ui, Some(nothing), core::ptr::null_mut()), 0);
    unsafe { pxui_ui_destroy(ui); }
}

#[test]
fn a_callback_cannot_start_a_frame_replace_the_theme_or_free_the_ui() {
    extern "C" fn reenter(user: *mut c_void, borrowed: *mut c_void) {
        let borrowed = borrowed.cast::<PxuiUi>();
        let owner = user.cast::<PxuiUi>();
        let mut out = FrameOut::default();
        let theme = ThemeSource::default();
        unsafe {
            for ui in [borrowed, owner] {
                assert_eq!(pxui_frame(ui, &frame_in(), Some(nothing), core::ptr::null_mut(), &mut out), PXUI_ERR_STATE);
                assert_eq!(pxui_ui_set_theme(ui, &theme), PXUI_ERR_STATE);
                assert_eq!(pxui_ui_use_theme(ui, core::ptr::null()), PXUI_ERR_STATE);
                assert_eq!(pxui_ui_destroy(ui), PXUI_ERR_STATE);
                assert!(!pxui_ui_output(ui).is_null());
            }
            assert!(pxui_renderer_create(borrowed, &pxui_types::Gpu::default(), &PxuiTargetDesc::default()).is_null());
        }
    }
    use pxui_types::ffi::ThemeSource;
    let ui = create();
    assert_eq!(run(ui, Some(reenter), ui.cast()), 0, "{}", error(ui));
    // after the frame the owner works again
    assert_eq!(run(ui, Some(nothing), core::ptr::null_mut()), 0);
    assert_eq!(unsafe { pxui_ui_set_theme(ui, &ThemeSource::default()) }, 0);
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

#[test]
fn a_ui_with_a_renderer_attached_refuses_to_be_destroyed() {
    let ui = create();
    unsafe { (*(*ui).shared).renderers = 1; }
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, PXUI_ERR_STATE);
    assert!(error(ui).contains("renderer"), "{}", error(ui));
    unsafe { (*(*ui).shared).renderers = 0; }
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

#[test]
fn default_descriptors_carry_their_size() {
    use pxui_types::{Commands, Gpu, Pass, ffi::{Input, ThemeSource}};
    assert_eq!(FrameIn::default().size as usize, core::mem::size_of::<FrameIn>());
    assert_eq!(Input::default().size as usize, core::mem::size_of::<Input>());
    assert_eq!(Gpu::default().size as usize, core::mem::size_of::<Gpu>());
    assert_eq!(Pass::default().size as usize, core::mem::size_of::<Pass>());
    assert_eq!(Commands::default().size as usize, core::mem::size_of::<Commands>());
    assert_eq!(ThemeSource::default().size as usize, core::mem::size_of::<ThemeSource>());
    // and the library accepts the defaults it is given
    let ui = create();
    assert_eq!(unsafe { pxui_ui_set_theme(ui, &ThemeSource::default()) }, 0);
    let mut out = FrameOut::default();
    let input = Input { logical_size: [320, 240], ..Input::default() };
    assert_eq!(unsafe { pxui_ui_frame(ui, &input, Some(nothing), core::ptr::null_mut(), &mut out) }, 0);
    unsafe { pxui_ui_destroy(ui); }
}

#[test]
fn text_with_nul_crosses_as_ordinary_text() {
    extern "C" fn build(user: *mut c_void, ui: *mut c_void) {
        let ui = ui.cast::<PxuiUi>();
        let query = Str { ptr: b"a\0b".as_ptr(), len: 3 };
        let (mut returned, mut tag, mut index) = (Str::default(), 0u32, 0i64);
        let rc = unsafe { pxui_ui_picker(ui, 5, 0, 0, Str { ptr: b"pick".as_ptr(), len: 4 }, query, None, None, None, core::ptr::null_mut(), &mut returned, &mut tag, &mut index) };
        assert_eq!(rc, 0, "{}", error(ui));
        assert_eq!(unsafe { core::slice::from_raw_parts(returned.ptr, returned.len) }, b"a\0b");
        unsafe { *user.cast::<u32>() += 1; }
    }
    let ui = create();
    let mut built = 0u32;
    assert_eq!(run(ui, Some(build), (&mut built as *mut u32).cast()), 0);
    assert_eq!(built, 1);
    unsafe { pxui_ui_destroy(ui); }
}

#[test]
fn theme_tokens_that_cannot_be_a_theme_make_none() {
    use pxui_types::ffi::ThemeTokens;
    let good = ThemeTokens::from(pxui_types::theme::default());
    let made = unsafe { crate::theme::pxui_theme_create(&good) };
    assert!(!made.is_null(), "the default tokens make a theme");
    unsafe { crate::theme::pxui_theme_destroy(made); }
    let bad = [
        ThemeTokens { density: f32::NAN, ..good }, ThemeTokens { density: 0.0, ..good }, ThemeTokens { space: [f32::INFINITY; 4], ..good },
        ThemeTokens { has_ink_ground: 2, ..good }, ThemeTokens { reduced_motion: 7, ..good }, ThemeTokens { font_size: 0, ..good },
        ThemeTokens { reserved: [1, 0, 0], ..good }, ThemeTokens { size: 4, ..good },
    ];
    for tokens in bad { assert!(unsafe { crate::theme::pxui_theme_create(&tokens) }.is_null()); }
}

/// Calls every kind of export on `ui` and returns the statuses: a default-form one, a trusted-form one (C entry), the trusted entry the
/// static wrapper calls, a bare one that touches the engine and a container.
fn probe(ui: *mut PxuiUi) -> [i32; 6] {
    let (label, mut pressed, mut out) = (Str { ptr: b"x".as_ptr(), len: 1 }, 0u8, PxuiBox::default());
    unsafe {
        [pxui_ui_set_dir(ui, 0), pxui_label(ui, label), pxui_button(ui, label, &mut pressed),
         crate::trusted::pxui_ui_node_keyed(ui, 7, &pxui_types::Node::DEFAULT, &mut out),
         pxui_ui_draw(ui, &out, 0, None, core::ptr::null_mut(), None), pxui_scope(ui, label, Some(nothing), core::ptr::null_mut())]
    }
}

#[test]
fn only_the_innermost_callbacks_handle_may_be_used_while_a_frame_is_built() {
    // user: [owner, outer handle]; the outer callback nests a scope whose callback probes the outer handle and the owner
    extern "C" fn inner(user: *mut c_void, ui: *mut c_void) {
        let [owner, outer] = unsafe { *user.cast::<[*mut PxuiUi; 2]>() };
        assert_eq!(probe(ui.cast()), [0; 6], "the innermost handle works");
        assert_eq!(probe(outer), [PXUI_ERR_STATE; 6], "the outer handle is stale while its child runs");
        assert_eq!(probe(owner), [PXUI_ERR_STATE; 6], "the owner is refused inside a frame");
        assert!(error(ui.cast()).contains("not the one"), "{}", error(ui.cast()));
        assert!(!unsafe { pxui_ui_output(owner) }.is_null(), "reading the output stays allowed");
        assert!(!unsafe { pxui_last_error(owner) }.ptr.is_null());
    }
    extern "C" fn outer(user: *mut c_void, ui: *mut c_void) {
        let owner = unsafe { *user.cast::<*mut PxuiUi>() };
        assert_eq!(probe(ui.cast()), [0; 6]);
        let mut handles = [owner, ui.cast::<PxuiUi>()];
        assert_eq!(unsafe { pxui_scope(ui.cast(), Str { ptr: b"s".as_ptr(), len: 1 }, Some(inner), handles.as_mut_ptr().cast()) }, 0);
        // the child returned: this handle is the current one again
        assert_eq!(probe(ui.cast()), [0; 6]);
        assert_eq!(probe(owner), [PXUI_ERR_STATE; 6]);
    }
    let ui = create();
    let mut owner = ui;
    assert_eq!(run(ui, Some(outer), (&mut owner as *mut *mut PxuiUi).cast()), 0, "{}", error(ui));
    // outside a frame the owner is the handle again
    assert_eq!(unsafe { pxui_ui_set_dir(ui, 0) }, 0);
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

#[test]
fn the_current_handle_is_restored_when_a_callback_unwinds() {
    let ui = create();
    // the callback panics on another thread (the gate allows no catch_unwind here); the state is plain data the thread reaches by address
    let shared = unsafe { (*ui).shared };
    let address = shared as usize;
    let guard = std::thread::spawn(move || {
        let shared = address as *mut crate::state::Shared;
        let mut engine = pxui_core::ui::Ui::create(input::Platform::Other, None, pxui_core::text::Text::new(&[pxui_core::text::PRAGMASEVKA]));
        crate::state::call_inner(shared, &mut engine, &mut |_| panic!("callback"));
    }).join();
    assert!(guard.is_err());
    assert_eq!(unsafe { (*shared).current }, ui, "the owner is current again");
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

#[test]
fn carry_text_borrows_survive_replacement_cancellation_and_many_reads_until_next_frame() {
    let ui = create();
    let text = |s: &str| Str { ptr: s.as_ptr(), len: s.len() };
    let mut kind = Str::default(); let mut value = Str::default(); let mut present = 0;
    assert_eq!(unsafe { pxui_ui_carry(ui, 0, core::ptr::null(), text("kind"), text("café")) }, 0);
    assert_eq!(unsafe { pxui_ui_carrying(ui, &mut present, &mut kind, &mut value) }, 0);
    assert_eq!(present, 1);
    let (first_kind, first_value) = (kind, value);
    assert_eq!(unsafe { pxui_ui_cancel_carry(ui) }, 0);
    assert_eq!(unsafe { pxui_ui_carry(ui, 0, core::ptr::null(), text("replacement"), text("different value")) }, 0);
    for _ in 0..128 { assert_eq!(unsafe { pxui_ui_carrying(ui, &mut present, &mut kind, &mut value) }, 0); }
    assert_eq!(unsafe { core::slice::from_raw_parts(first_kind.ptr, first_kind.len) }, b"kind");
    assert_eq!(unsafe { core::slice::from_raw_parts(first_value.ptr, first_value.len) }, "café".as_bytes());
    assert_eq!(unsafe { pxui_ui_cancel_carry(ui) }, 0);
    assert_eq!(run(ui, Some(nothing), core::ptr::null_mut()), 0);
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

#[test]
fn carry_transfer_validates_outputs_handles_and_has_no_capture_side_effect_without_a_carry() {
    extern "C" fn node(user: *mut c_void, raw: *mut c_void) {
        unsafe { pxui_ui_node_keyed(raw.cast(), 27, &pxui_types::Node::DEFAULT, user.cast()); }
    }
    let ui = create();
    let mut out = 9;
    let mut target = PxuiBox::default();
    assert_eq!(unsafe { pxui_ui_transfer_carry(core::ptr::null_mut(), &target, &mut out) }, PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe { pxui_ui_transfer_carry(ui, core::ptr::null(), &mut out) }, PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe { pxui_ui_transfer_carry(ui, &target, &mut out) }, PXUI_ERR_STALE);
    assert_eq!(out, 9);
    assert_eq!(run(ui, Some(node), (&mut target as *mut PxuiBox).cast()), 0);
    assert_eq!(target.box_key, 27);
    assert_eq!(unsafe { pxui_ui_transfer_carry(ui, &target, core::ptr::null_mut()) }, PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe { pxui_ui_transfer_carry(ui, &target, &mut out) }, 0);
    assert_eq!(out, 0);
    assert_eq!(run(ui, Some(nothing), core::ptr::null_mut()), 0);
    assert_eq!(unsafe { pxui_ui_transfer_carry(ui, &target, &mut out) }, 0);
    assert_eq!(out, 0, "a retired handle is quiet and cannot acquire capture");
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

#[test]
fn image_retirement_rejects_a_missing_renderer() {
    assert_eq!(unsafe { crate::pxui_renderer_forget_image(core::ptr::null_mut(),core::ptr::null(),0) }, crate::PXUI_ERR_ARGUMENT);
}

#[test]
fn accessibility_batches_validate_the_entire_span_before_applying_any_chunk() {
    extern "C" fn slider(user:*mut c_void,raw:*mut c_void) {
        let ui=raw.cast::<PxuiUi>();
        let desc=pxui_types::Node {flags:pxui_types::Flags::FOCUSABLE,role:pxui_types::a11y::Role::Slider,w:pxui_types::Size::Px(200.0),h:pxui_types::Size::Px(20.0),..pxui_types::Node::DEFAULT};
        unsafe { assert_eq!(pxui_ui_node_keyed(ui,27,&desc,user.cast()),0); }
    }
    let ui=create();
    assert_eq!(unsafe {pxui_ui_set_a11y(ui,1)},0);
    let mut node=PxuiBox::default();
    assert_eq!(run(ui,Some(slider),(&mut node as *mut PxuiBox).cast()),0);
    let output=unsafe {&*pxui_ui_output(ui)};
    let nodes=unsafe {core::slice::from_raw_parts(output.a11y.nodes.ptr,output.a11y.nodes.len)};
    let id=nodes.iter().find(|node|node.role==pxui_types::a11y::Role::Slider as u32).unwrap().id;
    let mut batch=[pxui_types::ffi::A11yAction {id,action_code:4,payload:3.0,..Default::default()};65];
    batch[64].action_code=3;
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,batch.as_ptr(),batch.len())},PXUI_ERR_ARGUMENT);
    assert_eq!(run(ui,Some(slider),(&mut node as *mut PxuiBox).cast()),0);
    let value=unsafe {(&*(*ui).engine).signal(&crate::state::core_box(node).unwrap()).a11y_value};
    assert_eq!(value,None,"invalid tail must not deliver earlier chunks");
    batch[64].action_code=4; batch[64].payload=77.0;
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,batch.as_ptr(),batch.len())},0);
    assert_eq!(run(ui,Some(slider),(&mut node as *mut PxuiBox).cast()),0);
    let value=unsafe {(&*(*ui).engine).signal(&crate::state::core_box(node).unwrap()).a11y_value};
    assert_eq!(value,Some(77.0));
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,core::ptr::null(),0)},0);
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,core::ptr::null(),1)},PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,batch.as_ptr(),4097)},PXUI_ERR_ARGUMENT);
    unsafe {pxui_ui_destroy(ui);}
}

#[test]
fn section_list_descriptors_validate_before_callbacks_and_restore_the_owner() {
    use pxui_types::ffi::PxuiSectionListDesc;
    extern "C" fn part(user:*mut c_void,raw:*mut c_void,index:usize) {
        let count=unsafe {&mut *user.cast::<usize>()};
        if *count==0 {assert_eq!(index,usize::MAX);}
        if *count==1 {assert_eq!(index,usize::MAX-1);}
        *count+=1;
        let mut node=PxuiBox::default();
        assert_eq!(unsafe {pxui_ui_node_keyed(raw.cast(),17,&pxui_types::Node::DEFAULT,&mut node)},0);
    }
    extern "C" fn build(user:*mut c_void,raw:*mut c_void) {
        let ui=raw.cast::<PxuiUi>();
        let valid=PxuiSectionListDesc {size:core::mem::size_of::<PxuiSectionListDesc>() as u32,
            overlay_scrollbar:1,reserved:[0;3],label:Str {ptr:b"section".as_ptr(),len:7},count:10_000,
            row_height:20.0,leading_height:100.0,header_height:30.0,trailing_height:40.0,build:Some(part),user,key:None};
        let mut out=PxuiBox {box_key:777,..PxuiBox::default()};
        let mut bad=[valid;10];
        bad[0].size-=1; bad[1].overlay_scrollbar=2; bad[2].reserved[1]=1;
        bad[3].label=Str {ptr:core::ptr::null(),len:1}; bad[4].count=1_000_001;
        bad[5].row_height=0.0; bad[6].leading_height=-1.0; bad[7].header_height=f32::NAN;
        bad[8].trailing_height=f32::INFINITY; bad[9].row_height=f32::MAX;
        for desc in &bad {
            assert_eq!(unsafe {pxui_ui_section_list(ui,desc,&mut out)},PXUI_ERR_ARGUMENT);
            assert_eq!(out.box_key,777,"invalid descriptor leaves output unchanged");
            assert_eq!(unsafe {*user.cast::<usize>()},0,"invalid descriptor calls no builder");
        }
        assert_eq!(unsafe {pxui_ui_section_list(ui,core::ptr::null(),&mut out)},PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe {pxui_ui_section_list(ui,&valid,core::ptr::null_mut())},PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe {pxui_ui_section_list(ui,&valid,&mut out)},0);
        assert!((3..30).contains(&unsafe {*user.cast::<usize>()}));
        let mut node=PxuiBox::default();
        assert_eq!(unsafe {pxui_ui_node_keyed(ui,19,&pxui_types::Node::DEFAULT,&mut node)},0,"section callback restores its parent handle");
    }
    let ui=create(); let mut calls=0usize;
    assert_eq!(unsafe {pxui_ui_section_list(core::ptr::null_mut(),core::ptr::null(),core::ptr::null_mut())},PXUI_ERR_ARGUMENT);
    assert_eq!(run(ui,Some(build),(&mut calls as *mut usize).cast()),0,"{}",error(ui));
    assert_eq!(unsafe {pxui_ui_destroy(ui)},0);
}

/// An ABI 1.20 caller's 64-byte record (no `key`) still builds, its rows keyed by index; a 72-byte one with `key` keys each row box
/// by the id it returns, so the id's row keeps its key when the items reorder.
#[test]
fn section_lists_take_the_old_record_and_key_rows_by_item_id() {
    use pxui_types::ffi::PxuiSectionListDesc;
    extern "C" fn row(user:*mut c_void,raw:*mut c_void,index:usize) {
        let state=unsafe {&mut *user.cast::<([i64;3],[i64;3])>()};
        let mut node=PxuiBox::default();
        // a child of the row box: its key follows the row's
        assert_eq!(unsafe {crate::pxui_ui_node(raw.cast(),Str {ptr:b"title".as_ptr(),len:5},&pxui_types::Node::DEFAULT,&mut node)},0);
        if index<3 {state.1[index]=node.box_key;}
    }
    extern "C" fn key(user:*mut c_void,index:usize)->i64 { unsafe {(*user.cast::<([i64;3],[i64;3])>()).0[index]} }
    extern "C" fn build(user:*mut c_void,raw:*mut c_void) {
        let ui=raw.cast::<PxuiUi>();
        let keyed=PxuiSectionListDesc {size:core::mem::size_of::<PxuiSectionListDesc>() as u32,overlay_scrollbar:0,reserved:[0;3],
            label:Str {ptr:b"keyed".as_ptr(),len:5},count:3,row_height:20.0,leading_height:0.0,header_height:0.0,trailing_height:0.0,
            build:Some(row),user,key:Some(key)};
        let mut out=PxuiBox::default();
        assert_eq!(unsafe {pxui_ui_section_list(ui,&keyed,&mut out)},0);
        let old=PxuiSectionListDesc {size:64,label:Str {ptr:b"old".as_ptr(),len:3},build:None,..keyed};
        assert_eq!(unsafe {pxui_ui_section_list(ui,&old,&mut out)},0);
    }
    let ui=create();
    let mut state=([10i64,20,30],[0i64;3]);
    assert_eq!(run(ui,Some(build),(&mut state as *mut ([i64;3],[i64;3])).cast()),0,"{}",error(ui));
    let before=state.1;
    state.0.reverse();
    assert_eq!(run(ui,Some(build),(&mut state as *mut ([i64;3],[i64;3])).cast()),0,"{}",error(ui));
    assert_eq!((state.1[0],state.1[2]),(before[2],before[0]),"each id's row (and its child) keeps its key");
    assert_ne!(before[0],before[1]);
    assert_eq!(unsafe {pxui_ui_destroy(ui)},0);
}

#[test]
fn measured_section_descriptors_validate_before_callbacks_and_restore_the_owner() {
    use pxui_types::ffi::PxuiMeasuredSectionDesc;
    extern "C" fn part(user:*mut c_void,raw:*mut c_void,index:usize,y:f32) {
        let count=unsafe {&mut *user.cast::<usize>()};
        assert!(y.is_finite());
        if *count==0 {assert_eq!(index,usize::MAX);assert_eq!(y,0.0);}
        if *count==1 {assert_eq!(index,usize::MAX-1);}
        *count+=1;
        let mut node=PxuiBox::default();
        assert_eq!(unsafe {pxui_ui_node_keyed(raw.cast(),17,&pxui_types::Node {h:pxui_types::Size::Px(20.0),..pxui_types::Node::DEFAULT},&mut node)},0);
    }
    extern "C" fn build(user:*mut c_void,raw:*mut c_void) {
        let ui=raw.cast::<PxuiUi>();
        let valid=PxuiMeasuredSectionDesc {size:core::mem::size_of::<PxuiMeasuredSectionDesc>() as u32,
            overlay_scrollbar:1,reserved:[0;3],label:Str {ptr:b"section".as_ptr(),len:7},count:10_000,
            row_height:20.0,leading_height:100.0,header_height:30.0,trailing_height:40.0,build:Some(part),user,key:None};
        let before=unsafe {*user.cast::<usize>()};
        let mut out=PxuiBox {box_key:777,..PxuiBox::default()};
        let mut bad=[valid;10];
        bad[0].size-=1; bad[1].overlay_scrollbar=2; bad[2].reserved[1]=1;
        bad[3].label=Str {ptr:core::ptr::null(),len:1}; bad[4].count=1_000_001;
        bad[5].row_height=0.0; bad[6].leading_height=-1.0; bad[7].header_height=f32::NAN;
        bad[8].trailing_height=f32::INFINITY; bad[9].row_height=f32::MAX;
        for desc in &bad {
            assert_eq!(unsafe {pxui_ui_section_list_measured(ui,desc,&mut out)},PXUI_ERR_ARGUMENT);
            assert_eq!(out.box_key,777,"invalid descriptor leaves output unchanged");
            assert_eq!(unsafe {*user.cast::<usize>()},before,"invalid descriptor calls no builder");
        }
        assert_eq!(unsafe {pxui_ui_section_list_measured(ui,core::ptr::null(),&mut out)},PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe {pxui_ui_section_list_measured(ui,&valid,core::ptr::null_mut())},PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe {pxui_ui_section_list_measured(ui,&valid,&mut out)},0);
        assert!((3..30).contains(&(unsafe {*user.cast::<usize>()}-before)));
        let mut node=PxuiBox::default();
        assert_eq!(unsafe {pxui_ui_node_keyed(ui,19,&pxui_types::Node::DEFAULT,&mut node)},0,"section callback restores its parent handle");
    }
    let ui=create(); let mut calls=0usize;
    assert_eq!(unsafe {pxui_ui_section_list_measured(core::ptr::null_mut(),core::ptr::null(),core::ptr::null_mut())},PXUI_ERR_ARGUMENT);
    assert_eq!(run(ui,Some(build),(&mut calls as *mut usize).cast()),0,"{}",error(ui));
    assert_eq!(unsafe {pxui_ui_destroy(ui)},0);
}

#[test]
fn accessible_text_value_guards_validate_the_whole_input_before_mutation() {
    extern "C" fn build(_: *mut c_void,raw:*mut c_void) {
        let ui=raw.cast::<PxuiUi>();let mut node=PxuiBox::default();
        assert_eq!(unsafe {pxui_ui_node_keyed(ui,123,&pxui_types::Node {role:pxui_types::a11y::Role::Button,w:pxui_types::Size::Px(100.0),h:pxui_types::Size::Px(30.0),..pxui_types::Node::DEFAULT},&mut node)},0);
        let valid=Str {ptr:"Café".as_ptr(),len:5};
        assert_eq!(unsafe {pxui_ui_a11y_value(ui,&node,valid,5,0)},0);
        for value in [Str {ptr:core::ptr::null(),len:1},Str {ptr:b"x".as_ptr(),len:crate::state::MAX_TEXT_BYTES+1},Str {ptr:b"\xff".as_ptr(),len:1}] {
            assert_eq!(unsafe {pxui_ui_a11y_value(ui,&node,value,0,0)},PXUI_ERR_ARGUMENT);
        }
        for (anchor,caret) in [(4,0),(0,4),(6,0),(0,u32::MAX)] {assert_eq!(unsafe {pxui_ui_a11y_value(ui,&node,valid,anchor,caret)},PXUI_ERR_ARGUMENT);}
        assert_eq!(unsafe {pxui_ui_a11y_value(ui,core::ptr::null(),valid,0,0)},PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe {pxui_ui_a11y_value(ui,&PxuiBox::default(),valid,0,0)},crate::PXUI_ERR_STALE);
    }
    let ui=create();assert_eq!(unsafe {pxui_ui_set_a11y(ui,1)},0);
    let mut out=FrameOut::default();assert_eq!(unsafe {pxui_frame(ui,&frame_in(),Some(build),core::ptr::null_mut(),&mut out)},0,"{}",error(ui));
    let out=unsafe {&*pxui_ui_output(ui)};
    let nodes=unsafe {core::slice::from_raw_parts(out.a11y.nodes.ptr,out.a11y.nodes.len)};
    let strings=unsafe {core::slice::from_raw_parts(out.a11y.strings.ptr,out.a11y.strings.len)};
    let node=nodes.iter().find(|node|node.value.tag==1).unwrap();assert_eq!(&strings[node.value.text_start as usize..node.value.text_end as usize],"Café".as_bytes());assert_eq!(node.selection,[4,0]);
    assert_eq!(unsafe {pxui_ui_a11y_value(core::ptr::null_mut(),core::ptr::null(),Str::default(),0,0)},PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe {pxui_ui_destroy(ui)},0);
}

// Size-prefixed records against a guard page. Each record is copied to the very end of a readable page with an inaccessible page
// behind it, so a read or write past the bytes the test supplies faults and kills the case. Five calls per record: only the size
// word present (refused), one byte short (refused), exact, a caller whose record claims 64 bytes more than the library knows (the
// extra bytes are on the guard page: accepted, never read) and a claim of u32::MAX (accepted by the same rule, never read).
mod sized_records {
    use super::*;
    use pxui_types::ffi::*;

    unsafe extern "C" {
        fn mmap(addr: *mut c_void, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut c_void;
        fn mprotect(addr: *mut c_void, len: usize, prot: i32) -> i32;
    }
    const PAGE: usize = 1 << 14; // a multiple of the page size of every supported target
    #[cfg(target_os = "macos")]
    const ANON_PRIVATE: i32 = 0x1002;
    #[cfg(not(target_os = "macos"))]
    const ANON_PRIVATE: i32 = 0x22;
    const REFUSED: i32 = PXUI_ERR_ARGUMENT;

    /// `len` bytes of `bytes` placed so they end where the guard page starts (less the gap `align` needs), with `claimed` as the leading size word.
    fn edge(bytes: &[u8], len: usize, align: usize, claimed: u32) -> *mut u8 {
        let map = unsafe { mmap(core::ptr::null_mut(), 2 * PAGE, 3, ANON_PRIVATE, -1, 0) };
        assert_ne!(map as isize, -1);
        assert_eq!(unsafe { mprotect(map.cast::<u8>().add(PAGE).cast(), PAGE, 0) }, 0);
        let at = unsafe { map.cast::<u8>().add((PAGE - len) & !(align - 1)) };
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), at, len); at.cast::<u32>().write_unaligned(claimed); }
        at
    }

    /// One frame on `ui` whose build runs `f` once; its result.
    fn in_frame(ui: *mut PxuiUi, f: &mut dyn FnMut(*mut PxuiUi) -> i32) -> i32 {
        extern "C" fn build(user: *mut c_void, ui: *mut c_void) {
            let (f, rc) = unsafe { &mut *user.cast::<(&mut dyn FnMut(*mut PxuiUi) -> i32, i32)>() };
            *rc = f(ui.cast());
        }
        let mut state = (f, i32::MIN);
        assert_eq!(run(ui, Some(build), (&mut state as *mut (&mut dyn FnMut(*mut PxuiUi) -> i32, i32)).cast()), 0, "{}", error(ui));
        state.1
    }
    fn a_box(ui: *mut PxuiUi) -> PxuiBox {
        let mut node = PxuiBox::default();
        assert_eq!(unsafe { pxui_ui_node_keyed(ui, 9, &pxui_types::Node::DEFAULT, &mut node) }, 0);
        node
    }
    fn head_out() -> PxuiHeadOut { let mut out: PxuiHeadOut = unsafe { core::mem::zeroed() }; out.size = core::mem::size_of::<PxuiHeadOut>() as u32; out }
    fn created(ui: *mut PxuiUi) -> i32 {
        if ui.is_null() { return REFUSED; }
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
        0
    }

    /// Runs `call` on five placements of the record at `at` (`known` bytes, aligned to `align`): only the size word, one byte short
    /// (both refused unless `refusals` is off), exact, a claim of 64 bytes more than known and a claim of u32::MAX.
    fn four(ui: *mut PxuiUi, (at, known, align): (*const u8, usize, usize), refusals: bool, call: &mut dyn FnMut(*mut u8) -> i32) {
        let bytes = unsafe { core::slice::from_raw_parts(at, known) };
        if refusals {
            assert_eq!(call(edge(bytes, 4, align, 4)), REFUSED, "only the size word");
            assert_eq!(call(edge(bytes, known - 1, align, known as u32 - 1)), REFUSED, "one byte short");
        }
        assert_eq!(call(edge(bytes, known, align, known as u32)), 0, "exact: {}", error(ui));
        assert_eq!(call(edge(bytes, known, align, known as u32 + 64)), 0, "larger than known: {}", error(ui));
        assert_eq!(call(edge(bytes, known, align, u32::MAX)), 0, "huge: {}", error(ui));
    }

    #[test]
    fn ui_desc() {
        let ui = create();
        let mut value: PxuiUiDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiUiDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiUiDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiUiDesc>();
            created(unsafe { pxui_ui_create(p) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn ui_desc_with_fonts() {
        let ui = create();
        let mut value: PxuiUiDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiUiDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiUiDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiUiDesc>();
            {
        let font = Str { ptr: pxui_core::text::PRAGMASEVKA.as_ptr(), len: pxui_core::text::PRAGMASEVKA.len() };
        created(unsafe { pxui_ui_create_with_fonts(p, &font, 1) })
    }
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn theme_source() {
        let ui = create();
        let mut value: ThemeSource = unsafe { core::mem::zeroed() };
        let fix: fn(&mut ThemeSource) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const ThemeSource).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<ThemeSource>();
            unsafe { pxui_ui_set_theme(ui, p) }
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn theme_tokens() {
        let ui = create();
        let mut value: ThemeTokens = unsafe { core::mem::zeroed() };
        let fix: fn(&mut ThemeTokens) = |t| *t = ThemeTokens::from(pxui_types::theme::default());
        fix(&mut value);
        four(ui, ((&value as *const ThemeTokens).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<ThemeTokens>();
            {
        let theme = unsafe { pxui_theme_create(p) };
        if theme.is_null() { return REFUSED; }
        unsafe { pxui_theme_destroy(theme); }
        0
    }
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn input_frame() {
        let ui = create();
        let mut value: Input = unsafe { core::mem::zeroed() };
        let fix: fn(&mut Input) = |i| *i = Input { logical_size: [320, 240], ..Input::default() };
        fix(&mut value);
        four(ui, ((&value as *const Input).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<Input>();
            {
        let mut out = FrameOut::default();
        unsafe { pxui_ui_frame(ui, p, None, core::ptr::null_mut(), &mut out) }
    }
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn panel() {
        let ui = create();
        let mut value: PxuiPanel = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiPanel) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiPanel).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiPanel>();
            in_frame(ui, &mut |ui| unsafe { pxui_panel(ui, p, Str::default(), None, core::ptr::null_mut()) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn box_desc() {
        let ui = create();
        let mut value: PxuiBoxDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiBoxDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiBoxDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiBoxDesc>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_box(ui, Str::default(), p, &mut PxuiBox::default()) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn node_fields_in() {
        let ui = create();
        let mut value: PxuiNodeFields = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiNodeFields) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiNodeFields).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiNodeFields>();
            in_frame(ui, &mut |ui| { let b = a_box(ui); unsafe { pxui_ui_set_node_fields(ui, &b, p) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn node_fields_out() {
        let ui = create();
        let mut value: PxuiNodeFields = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiNodeFields) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiNodeFields).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiNodeFields>();
            in_frame(ui, &mut |ui| { let b = a_box(ui); unsafe { pxui_ui_node_fields(ui, &b, p) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn node_out() {
        let ui = create();
        let mut value: pxui_types::Node = unsafe { core::mem::zeroed() };
        let fix: fn(&mut pxui_types::Node) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const pxui_types::Node).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<pxui_types::Node>();
            in_frame(ui, &mut |ui| { let b = a_box(ui); unsafe { pxui_ui_node_layout(ui, &b, p) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }
    #[test]
    fn gesture_in() {
        let ui = create();
        let mut value: PxuiGestureIn = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiGestureIn) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiGestureIn).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiGestureIn>();
            in_frame(ui, &mut |ui| { let b = a_box(ui); unsafe { pxui_ui_gesture(ui, &b, p, 0, &mut PxuiGestureOut::default()) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn gesture_out() {
        let ui = create();
        let mut value: PxuiGestureOut = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiGestureOut) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiGestureOut).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiGestureOut>();
            in_frame(ui, &mut |ui| { let b = a_box(ui); unsafe { pxui_ui_gesture(ui, &b, core::ptr::null(), 0, p) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn swipe_in() {
        let ui = create();
        let mut value: PxuiSwipeIn = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiSwipeIn) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiSwipeIn).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiSwipeIn>();
            in_frame(ui, &mut |ui| { let b = a_box(ui); unsafe { pxui_ui_swipe_actions(ui, &b, p) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn dismiss_in() {
        let ui = create();
        let mut value: PxuiDismissIn = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiDismissIn) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiDismissIn).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiDismissIn>();
            in_frame(ui, &mut |ui| { let b = a_box(ui); unsafe { pxui_ui_dismiss(ui, &b, p, &mut PxuiDismissOut::default()) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn dismiss_out() {
        let ui = create();
        let mut value: PxuiDismissOut = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiDismissOut) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiDismissOut).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiDismissOut>();
            in_frame(ui, &mut |ui| { let b = a_box(ui); let request = PxuiDismissIn::default(); unsafe { pxui_ui_dismiss(ui, &b, &request, p) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn popup() {
        let ui = create();
        let mut value: PxuiPopupDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiPopupDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiPopupDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiPopupDesc>();
            in_frame(ui, &mut |ui| { let mut open = 0u8; unsafe { pxui_ui_popup(ui, p, None, core::ptr::null_mut(), &mut open) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn button_desc() {
        let ui = create();
        let mut value: PxuiButtonDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiButtonDesc) = |d| { d.enabled = 1; d.key = Str { ptr: b"k".as_ptr(), len: 1 }; d.text = d.key; };
        fix(&mut value);
        four(ui, ((&value as *const PxuiButtonDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiButtonDesc>();
            in_frame(ui, &mut |ui| { let (mut out, mut signal, mut focused) = (PxuiBox::default(), PxuiSignal::default(), 0u8); unsafe { pxui_ui_button_with(ui, p, None, core::ptr::null_mut(), &mut out, &mut signal, &mut focused) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn icon_button_desc() {
        let ui = create();
        let mut value: PxuiIconButtonDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiIconButtonDesc) = |d| { d.enabled = 1; d.press_scale = 1.0; d.key = Str { ptr: b"k".as_ptr(), len: 1 }; d.name = d.key; d.icon = d.key; };
        fix(&mut value);
        four(ui, ((&value as *const PxuiIconButtonDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiIconButtonDesc>();
            in_frame(ui, &mut |ui| { let (mut out, mut signal, mut focused) = (PxuiBox::default(), PxuiSignal::default(), 0u8); unsafe { pxui_ui_icon_button_with(ui, p, &mut out, &mut signal, &mut focused) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    /// ABI 1.48: the response calls check their descriptors as the `_with` ones do, and refuse a null or short `PxuiResponse`; a plain
    /// call fills the record with the box it declared.
    #[test]
    fn response_calls() {
        let ui = create();
        let k = Str { ptr: b"k".as_ptr(), len: 1 };
        let mut value: PxuiButtonDesc = unsafe { core::mem::zeroed() };
        (value.enabled, value.key, value.text) = (1, k, k);
        four(ui, ((&value as *const PxuiButtonDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiButtonDesc>();
            in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_button_response(ui, p, None, core::ptr::null_mut(), &mut out) } })
        });
        let mut icon: PxuiIconButtonDesc = unsafe { core::mem::zeroed() };
        (icon.enabled, icon.press_scale, icon.key, icon.name, icon.icon) = (1, 1.0, k, k, k);
        four(ui, ((&icon as *const PxuiIconButtonDesc).cast(), size_of_val(&icon), align_of_val(&icon)), true, &mut |p| {
            let p = p.cast::<PxuiIconButtonDesc>();
            in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_icon_button_response(ui, p, &mut out) } })
        });
        let short = PxuiResponse { size: 8, ..PxuiResponse::EMPTY };
        for out in [core::ptr::null_mut(), &short as *const PxuiResponse as *mut PxuiResponse] {
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_button_plain(ui, k, k, out) }), PXUI_ERR_ARGUMENT);
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_icon_button_plain(ui, k, k, k, out) }), PXUI_ERR_ARGUMENT);
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_button_response(ui, &value, None, core::ptr::null_mut(), out) }), PXUI_ERR_ARGUMENT);
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_icon_button_response(ui, &icon, out) }), PXUI_ERR_ARGUMENT);
        }
        let bad = Str { ptr: [0xffu8].as_ptr(), len: 1 };
        assert_eq!(in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_button_plain(ui, k, bad, &mut out) } }), PXUI_ERR_ARGUMENT);
        let mut plain = (PxuiResponse::EMPTY, PxuiResponse::EMPTY);
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_button_plain(ui, k, k, &mut plain.0) + pxui_ui_icon_button_plain(ui, Str { ptr: b"i".as_ptr(), len: 1 }, k, k, &mut plain.1) }), 0);
        assert!(plain.0.node.box_key != 0 && plain.1.node.box_key != 0 && plain.0.node.box_key != plain.1.node.box_key, "{plain:?}");
        unsafe { pxui_ui_destroy(ui); }
    }

    /// ABI 1.53: `pxui_ui_dialog_response` checks its descriptor as the other sized records (size word, text, masks naming a button
    /// that is not there, an index out of range, a bad width) and refuses a null or short `PxuiDialogResult`; a good call answers.
    #[test]
    fn dialog_response_desc() {
        let ui = create();
        let k = Str { ptr: b"k".as_ptr(), len: 1 };
        let buttons = [k, k];
        let mut value: PxuiDialogDesc = unsafe { core::mem::zeroed() };
        value.size = size_of_val(&value) as u32;
        (value.width, value.key, value.title, value.buttons, value.default_at, value.cancel_at) = (320.0, k, k, Slice { ptr: buttons.as_ptr(), len: 2 }, 1, 0);
        four(ui, ((&value as *const PxuiDialogDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiDialogDesc>();
            in_frame(ui, &mut |ui| { let mut out = PxuiDialogResult::EMPTY; unsafe { pxui_ui_dialog_response(ui, p, None, core::ptr::null_mut(), &mut out) } })
        });
        let mut out = PxuiDialogResult::EMPTY;
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_dialog_response(ui, &value, None, core::ptr::null_mut(), &mut out) }), 0, "{}", error(ui));
        assert_eq!((out.has_chosen, out.dismissed), (0, 0), "an open dialog chose nothing");
        let short = PxuiDialogResult { size: 4, ..PxuiDialogResult::EMPTY };
        for bad_out in [core::ptr::null_mut(), &short as *const PxuiDialogResult as *mut PxuiDialogResult] {
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_dialog_response(ui, &value, None, core::ptr::null_mut(), bad_out) }), PXUI_ERR_ARGUMENT);
        }
        let fixes: [fn(&mut PxuiDialogDesc); 7] = [|d| d.default_at = 2, |d| d.cancel_at = -2, |d| d.danger = 4, |d| d.disabled = 1 << 40, |d| d.width = f32::NAN, |d| d.width = -1.0, |d| d.buttons.len = 65];
        for fix in fixes {
            let mut bad = value;
            fix(&mut bad);
            let mut sink = PxuiDialogResult::EMPTY;
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_dialog_response(ui, &bad, None, core::ptr::null_mut(), &mut sink) }), PXUI_ERR_ARGUMENT);
        }
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn header_desc() {
        let ui = create();
        let mut value: PxuiHeaderDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiHeaderDesc) = |d| { d.key = Str { ptr: b"k".as_ptr(), len: 1 }; d.title = d.key; };
        fix(&mut value);
        four(ui, ((&value as *const PxuiHeaderDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiHeaderDesc>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_inspector_header(ui, p, None, core::ptr::null_mut(), &mut head_out()) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn header_out() {
        let ui = create();
        let mut value: PxuiHeadOut = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiHeadOut) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiHeadOut).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiHeadOut>();
            in_frame(ui, &mut |ui| { let mut d: PxuiHeaderDesc = unsafe { core::mem::zeroed() }; d.size = core::mem::size_of::<PxuiHeaderDesc>() as u32; d.key = Str { ptr: b"k".as_ptr(), len: 1 }; d.title = d.key; unsafe { pxui_ui_inspector_header(ui, &d, None, core::ptr::null_mut(), p) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn value_field() {
        let ui = create();
        let mut value: PxuiValueFieldDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiValueFieldDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiValueFieldDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiValueFieldDesc>();
            in_frame(ui, &mut |ui| {
        let (mut text, mut changed, mut editing) = (Str::default(), 0u8, 0u8);
        unsafe { pxui_ui_value_field(ui, p, None, None, None, core::ptr::null_mut(), &mut text, &mut changed, &mut editing) }
    })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn section_list() {
        let ui = create();
        let mut value: PxuiSectionListDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiSectionListDesc) = |d| d.row_height = 20.0;
        fix(&mut value);
        four(ui, ((&value as *const PxuiSectionListDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiSectionListDesc>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_section_list(ui, p, &mut PxuiBox::default()) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn section_list_measured() {
        let ui = create();
        let mut value: PxuiMeasuredSectionDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiMeasuredSectionDesc) = |d| d.row_height = 20.0;
        fix(&mut value);
        four(ui, ((&value as *const PxuiMeasuredSectionDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiMeasuredSectionDesc>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_section_list_measured(ui, p, &mut PxuiBox::default()) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn grid_list() {
        let ui = create();
        let mut value: PxuiGridListDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiGridListDesc) = |d| { d.cell_w = 10.0; d.cell_h = 10.0; };
        fix(&mut value);
        four(ui, ((&value as *const PxuiGridListDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiGridListDesc>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_grid_list(ui, p, &mut PxuiBox::default()) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn footer() {
        let ui = create();
        let mut value: PxuiFooterDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiFooterDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiFooterDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiFooterDesc>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_footer(ui, p) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn keyframe() {
        let ui = create();
        let mut value: PxuiKeyframeDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiKeyframeDesc) = |k| { k.target = 1; k.values[0] = 1.0; k.curve.kind = PxuiCurveDesc::SPRING; k.curve.omega = 20.0; };
        fix(&mut value);
        four(ui, ((&value as *const PxuiKeyframeDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiKeyframeDesc>();
            in_frame(ui, &mut |ui| { let b = a_box(ui); let mut running = 0u8; unsafe { pxui_ui_animate(ui, &b, p, &mut running) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn split() {
        let ui = create();
        let mut value: PxuiSplitDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiSplitDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiSplitDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiSplitDesc>();
            in_frame(ui, &mut |ui| { let (mut changed, mut ratio) = (0u8, 0.5f32); unsafe { pxui_ui_split(ui, p, &mut changed, &mut ratio) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn menu_bar_desc() {
        let ui = create();
        let mut value: PxuiMenuBarDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiMenuBarDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiMenuBarDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiMenuBarDesc>();
            in_frame(ui, &mut |ui| { let mut out = PxuiMenuOut::default(); out.size = core::mem::size_of::<PxuiMenuOut>() as u32; unsafe { pxui_ui_menu_bar(ui, p, &mut out) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn menu_bar_out() {
        let ui = create();
        let mut value: PxuiMenuOut = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiMenuOut) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiMenuOut).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiMenuOut>();
            in_frame(ui, &mut |ui| { let mut d: PxuiMenuBarDesc = unsafe { core::mem::zeroed() }; d.size = core::mem::size_of::<PxuiMenuBarDesc>() as u32; unsafe { pxui_ui_menu_bar(ui, &d, p) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn inspector_row() {
        let ui = create();
        let mut value: PxuiInspectorRowDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiInspectorRowDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiInspectorRowDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiInspectorRowDesc>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_inspector_row(ui, p) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn inspector_section() {
        let ui = create();
        let mut value: PxuiInspectorSectionDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiInspectorSectionDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiInspectorSectionDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiInspectorSectionDesc>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_inspector_section(ui, p) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn inspector_readout() {
        let ui = create();
        let mut value: PxuiInspectorValueDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiInspectorValueDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiInspectorValueDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiInspectorValueDesc>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_inspector_readout(ui, p) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn inspector_message() {
        let ui = create();
        let mut value: PxuiInspectorTextDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiInspectorTextDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiInspectorTextDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiInspectorTextDesc>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_inspector_message(ui, p) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn table() {
        let ui = create();
        let mut value: PxuiTableDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiTableDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiTableDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiTableDesc>();
            in_frame(ui, &mut |ui| { let mut clicked = 0i64; unsafe { pxui_ui_table(ui, p, &mut clicked) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn list_view() {
        let ui = create();
        let mut value: PxuiListViewDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiListViewDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiListViewDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiListViewDesc>();
            in_frame(ui, &mut |ui| { let mut hit = 0usize; unsafe { pxui_ui_list_view(ui, p, &mut hit) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn list_view_with() {
        let ui = create();
        let mut value: PxuiListViewWithDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiListViewWithDesc) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiListViewWithDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiListViewWithDesc>();
            in_frame(ui, &mut |ui| { let mut hit = 0usize; unsafe { pxui_ui_list_view_with(ui, p, &mut hit) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn tree() {
        let ui = create();
        let mut value: PxuiTreeViewDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiTreeViewDesc) = |d| { d.selected = Box::leak(Box::new(0)); d.visible_len = Box::leak(Box::new(0)); };
        fix(&mut value);
        four(ui, ((&value as *const PxuiTreeViewDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiTreeViewDesc>();
            in_frame(ui, &mut |ui| { let mut hit = 0usize; unsafe { pxui_ui_tree(ui, p, &mut hit) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn tree_with() {
        let ui = create();
        let mut value: PxuiTreeViewWithDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiTreeViewWithDesc) = |d| { d.selected = Box::leak(Box::new(0)); d.visible_len = Box::leak(Box::new(0)); };
        fix(&mut value);
        four(ui, ((&value as *const PxuiTreeViewWithDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiTreeViewWithDesc>();
            in_frame(ui, &mut |ui| { let mut hit = 0usize; unsafe { pxui_ui_tree_with(ui, p, &mut hit) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn table_view() {
        let ui = create();
        let mut value: PxuiTableViewDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiTableViewDesc) = |d| d.selected = Box::leak(Box::new(0));
        fix(&mut value);
        four(ui, ((&value as *const PxuiTableViewDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiTableViewDesc>();
            in_frame(ui, &mut |ui| { let mut hit = 0usize; unsafe { pxui_ui_table_view(ui, p, &mut hit) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn table_view_with() {
        let ui = create();
        let mut value: PxuiTableViewWithDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiTableViewWithDesc) = |d| d.selected = Box::leak(Box::new(0));
        fix(&mut value);
        four(ui, ((&value as *const PxuiTableViewWithDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiTableViewWithDesc>();
            in_frame(ui, &mut |ui| { let mut hit = 0usize; unsafe { pxui_ui_table_view_with(ui, p, &mut hit) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn scrub_out() {
        let ui = create();
        let mut value: PxuiScrubOut = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiScrubOut) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiScrubOut).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiScrubOut>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_scrub(ui, Str::default(), 0.0, 0.0, 1.0, 0.1, p) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn text_style() {
        let ui = create();
        let mut value: pxui_types::TextStyle = unsafe { core::mem::zeroed() };
        let fix: fn(&mut pxui_types::TextStyle) = |s| *s = pxui_types::TextStyle::default();
        fix(&mut value);
        four(ui, ((&value as *const pxui_types::TextStyle).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<pxui_types::TextStyle>();
            {
        let mut out = pxui_types::TextMetrics::default();
        unsafe { pxui_text_width(ui, 0, Str::default(), p, 100.0, &mut out) }
    }
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn text_spans() {
        let ui = create();
        let mut value: pxui_types::TextSpans = unsafe { core::mem::zeroed() };
        let fix: fn(&mut pxui_types::TextSpans) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const pxui_types::TextSpans).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<pxui_types::TextSpans>();
            {
        let (style, mut out) = (pxui_types::TextStyle::default(), pxui_types::TextMetrics::default());
        unsafe { pxui_text_width_spans(ui, 0, Str::default(), &style, 100.0, p, &mut out) }
    }
        });
        unsafe { pxui_ui_destroy(ui); }
    }
    #[test]
    fn saved_in() {
        let ui = create();
        let mut value: PxuiSaved = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiSaved) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiSaved).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiSaved>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_leave(ui, p) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }
}
