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
        let desc=pxui_types::Node {flags:pxui_types::ui::FOCUSABLE,role:pxui_types::a11y::Role::Slider,w:pxui_types::Size::Px(200.0),h:pxui_types::Size::Px(20.0),..pxui_types::Node::DEFAULT};
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
    let value=unsafe {(&*(*ui).engine).signal_state(&crate::state::core_box(node).unwrap()).a11y_value};
    assert_eq!(value,None,"invalid tail must not deliver earlier chunks");
    batch[64].action_code=4; batch[64].payload=77.0;
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,batch.as_ptr(),batch.len())},0);
    assert_eq!(run(ui,Some(slider),(&mut node as *mut PxuiBox).cast()),0);
    let value=unsafe {(&*(*ui).engine).signal_state(&crate::state::core_box(node).unwrap()).a11y_value};
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
            row_height:20.0,leading_height:100.0,header_height:30.0,trailing_height:40.0,build:Some(part),user};
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
