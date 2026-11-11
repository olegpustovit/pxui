//! The export macro's panic containment and the guards that keep a callback from re-entering the library.
use crate::state::{PxuiUi, panicked};
use crate::*;
use core::ffi::c_void;
use pxui_types::ffi::NodeDesc;

fn create() -> *mut PxuiUi { unsafe { pxui_ui_create(&PxuiUiDesc::default()) } }
fn frame_in() -> FrameIn { FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() } }
fn error(ui: *mut PxuiUi) -> String { let e = unsafe { pxui_last_error(ui) }; String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(e.ptr, e.len) }).into_owned() }
fn run(ui: *mut PxuiUi, build: Build, user: *mut c_void) -> i32 {
    let mut out = FrameOut::default();
    unsafe { pxui_frame(ui, &frame_in(), build, user, &mut out) }
}
extern "C" fn nothing(_: *mut c_void, _: *mut PxuiUi) {}
extern "C" fn count(user: *mut c_void, _: *mut PxuiUi) { unsafe { *user.cast::<u32>() += 1; } }

#[test]
fn an_engine_panic_through_an_export_returns_the_error_and_poisons_the_ui() {
    let ui = create();
    // declaring a node outside a frame makes the engine panic ("boxes must be built inside Ui.frame")
    let mut desc: NodeDesc = unsafe { core::mem::zeroed() };
    desc.size = core::mem::size_of::<NodeDesc>() as u32;
    let mut out = PxuiBox::default();
    assert_eq!(unsafe { pxui_ui_node_keyed(ui, 1, &desc, &mut out) }, PXUI_ERR_PANIC);
    assert!(error(ui).starts_with("panic: "), "{}", error(ui));
    // every later call on this Ui fails at once with the same code, even valid ones
    assert_eq!(unsafe { pxui_ui_set_dir(ui, 0) }, PXUI_ERR_PANIC);
    assert_eq!(unsafe { pxui_ui_request_frame(ui) }, PXUI_ERR_PANIC);
    assert_eq!(unsafe { pxui_button(ui, Str { ptr: b"x".as_ptr(), len: 1 }) }, 0);
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
    extern "C" fn poison(_: *mut c_void, ui: *mut PxuiUi) {
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
    extern "C" fn reenter(user: *mut c_void, borrowed: *mut PxuiUi) {
        let owner = user.cast::<PxuiUi>();
        let mut out = FrameOut::default();
        let theme = ThemeSource::default();
        unsafe {
            for ui in [borrowed, owner] {
                assert_eq!(pxui_frame(ui, &frame_in(), Some(nothing), core::ptr::null_mut(), &mut out), PXUI_ERR_STATE);
                assert_eq!(pxui_ui_set_theme(ui, &theme), PXUI_ERR_STATE);
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
    extern "C" fn build(user: *mut c_void, ui: *mut PxuiUi) {
        let query = Str { ptr: b"a\0b".as_ptr(), len: 3 };
        let (mut returned, mut tag, mut index) = (Str::default(), 0u32, 0i64);
        let rc = unsafe { pxui_ui_picker(ui, 5, 0, 0, Str { ptr: b"pick".as_ptr(), len: 4 }, query, None, core::ptr::null_mut(), &mut returned, &mut tag, &mut index) };
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
