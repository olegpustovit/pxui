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
/// `f`'s return code, run inside one frame of `ui`.
fn in_frame(ui: *mut PxuiUi, f: &mut dyn FnMut(*mut PxuiUi) -> i32) -> i32 {
    extern "C" fn build(user: *mut c_void, ui: *mut c_void) {
        let (f, rc) = unsafe { &mut *user.cast::<(&mut dyn FnMut(*mut PxuiUi) -> i32, i32)>() };
        *rc = f(ui.cast());
    }
    let mut state = (f, i32::MIN);
    assert_eq!(run(ui, Some(build), (&mut state as *mut (&mut dyn FnMut(*mut PxuiUi) -> i32, i32)).cast()), 0, "{}", error(ui));
    state.1
}
extern "C" fn nothing(_: *mut c_void, _: *mut c_void) {}
extern "C" fn count(user: *mut c_void, _: *mut c_void) { unsafe { *user.cast::<u32>() += 1; } }

/// A builder called outside a frame (the owner handle is usable between frames) is refused with `PXUI_ERR_STATE` before the engine
/// is reached: no panic, no poison, and the next call and frame run normally.
#[test]
fn a_builder_outside_a_frame_is_refused_without_poisoning_the_ui() {
    let ui = create();
    let desc = pxui_types::Node::DEFAULT;
    let mut out = PxuiBox::default();
    assert_eq!(unsafe { pxui_ui_node_keyed(ui, 7, &desc, &mut out) }, PXUI_ERR_STATE);
    assert!(error(ui).contains("frame callback"), "{}", error(ui));
    let mut pressed = 0;
    assert_eq!(unsafe { pxui_button(ui, Str { ptr: b"x".as_ptr(), len: 1 }, &mut pressed) }, PXUI_ERR_STATE);
    assert!(!unsafe { (*(*ui).shared).poisoned });
    assert_eq!(unsafe { pxui_ui_set_dir(ui, 0) }, 0);
    let mut built = 0u32;
    assert_eq!(run(ui, Some(count), (&mut built as *mut u32).cast()), 0);
    assert_eq!(built, 1);
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

/// The backstop for a library bug: a panic an export catches poisons its `Ui`, every later call fails with `PXUI_ERR_PANIC`, and the
/// next frame reports it, skips itself and clears the flag. Another `Ui` is untouched.
#[test]
fn a_caught_panic_poisons_the_ui_until_its_next_frame() {
    let ui = create();
    unsafe { panicked((*ui).shared, Box::new("a library bug"), false); }
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

/// A renderer holds the engine while it draws: a split callback it runs during a frame callback's lifetime (`NoCurrent`) cannot build
/// through the frame's handle, and the handle builds again once the renderer returns.
#[test]
fn a_builder_call_from_a_renderer_split_callback_is_refused() {
    let ui = create();
    let desc = pxui_types::Node::DEFAULT;
    let rc = in_frame(ui, &mut |handle| {
        let mut out = PxuiBox::default();
        {
            let _renderer = crate::state::NoCurrent::set(unsafe { (*handle).shared });
            assert_eq!(unsafe { pxui_ui_node_keyed(handle, 7, &desc, &mut out) }, PXUI_ERR_STATE);
        }
        unsafe { pxui_ui_node_keyed(handle, 7, &desc, &mut out) }
    });
    assert_eq!(rc, 0, "{}", error(ui));
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

/// Renderer calls are refused from a frame callback (`PXUI_ERR_STATE`) before the renderer is read: only its `shared` is written here.
#[test]
fn a_renderer_call_inside_a_frame_is_refused() {
    let ui = create();
    let mut renderer = core::mem::MaybeUninit::<PxuiRenderer>::uninit();
    let raw = renderer.as_mut_ptr();
    unsafe { core::ptr::addr_of_mut!((*raw).shared).write((*ui).shared); }
    let commands = pxui_types::Commands { size: core::mem::size_of::<pxui_types::Commands>() as u32, ..unsafe { core::mem::zeroed() } };
    let pass = pxui_types::Pass::default();
    let (mut prepared, mut drawn) = (0, 0);
    assert_eq!(in_frame(ui, &mut |_| unsafe {
        prepared = pxui_renderer_prepare(raw, &commands, 1, 0);
        drawn = pxui_renderer_draw(raw, &pass);
        0
    }), 0);
    assert_eq!((prepared, drawn), (PXUI_ERR_STATE, PXUI_ERR_STATE));
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

#[test]
fn checked_constructors_leave_outputs_untouched_on_refusal() {
    extern "C" fn body(_: *mut c_void, _: *mut c_void, _: *const c_void) {}
    let ui = create();
    let short = 4_u32;
    let mut renderer = core::ptr::dangling_mut::<PxuiRenderer>();
    let mut id = 77;
    let name = crate::state::pair("body");
    let kind = pxui_types::theme::BodyKind::toggle as u32;
    unsafe {
        assert_eq!(pxui_renderer_create_checked(ui, (&short as *const u32).cast(), &PxuiTargetDesc::default(), &mut renderer), PXUI_ERR_ARGUMENT);
        assert_eq!(renderer, core::ptr::dangling_mut::<PxuiRenderer>());
        assert_eq!(pxui_renderer_create_checked(ui, &pxui_types::Gpu::default(), &PxuiTargetDesc::default(), core::ptr::null_mut()), PXUI_ERR_ARGUMENT);
        assert_eq!(pxui_body_register_checked(ui, kind, name, Some(body), core::ptr::null_mut(), core::ptr::null_mut()), PXUI_ERR_ARGUMENT);
        assert_eq!(pxui_body_register_checked(ui, u32::MAX, name, Some(body), core::ptr::null_mut(), &mut id), PXUI_ERR_ARGUMENT);
        assert_eq!(id, 77);
        assert_eq!(pxui_body_register_checked(ui, kind, name, Some(body), core::ptr::null_mut(), &mut id), 0);
        assert_eq!(id, 1);
        assert_eq!(pxui_ui_destroy(ui), 0);
    }
}

#[test]
fn legacy_accessibility_setters_keep_success_for_a_recycled_slot() {
    extern "C" fn node(user: *mut c_void, raw: *mut c_void) {
        let handle = unsafe { &mut *user.cast::<PxuiBox>() };
        assert_eq!(unsafe { pxui_ui_node_keyed(raw.cast(), handle.box_key, &pxui_types::Node::DEFAULT, handle) }, 0);
    }
    let ui = create();
    let mut retired = PxuiBox { box_key: 27, ..Default::default() };
    let mut live = PxuiBox { box_key: 28, ..Default::default() };
    assert_eq!(run(ui, Some(node), (&mut retired as *mut PxuiBox).cast()), 0);
    assert_eq!(run(ui, None, core::ptr::null_mut()), 0);
    assert_eq!(run(ui, Some(node), (&mut live as *mut PxuiBox).cast()), 0);
    assert_eq!(live.box_slot, retired.box_slot);
    assert_ne!(live.generation, retired.generation);
    let name = crate::state::pair("retired");
    unsafe {
        assert_eq!(pxui_ui_a11y_name(ui, &retired, name), 0);
        assert_eq!(pxui_ui_a11y_number(ui, &retired, 0.0, 0.0, 1.0, 0.0), 0);
        assert_eq!(pxui_ui_set_role(ui, &retired, pxui_types::a11y::Role::Button as u32, 0), 0);
        assert_eq!(pxui_ui_a11y_name_checked(ui, &retired, name), PXUI_ERR_STALE);
        assert_eq!(pxui_ui_a11y_number_checked(ui, &retired, 0.0, 0.0, 1.0, 0.0), PXUI_ERR_STALE);
        assert_eq!(pxui_ui_a11y_value(ui, &retired, name, 0, 0), PXUI_ERR_STALE);
        assert_eq!(pxui_ui_destroy(ui), 0);
    }
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

// ABI 1.86: picker fills hand back `PxuiPickRow`s. The first row's size is read before the span is taken as rows, so a caller built
// before 1.86 (16-byte `PxuiStr`s, twice as many) is refused without a pointer of it being read; every row is checked before any is used.
#[test]
fn picker_rows_are_checked_and_old_str_fills_are_refused_unread() {
    use pxui_types::ffi::{PxuiPickRow, Slice};
    // the rows the callback hands back: (pointer, length, calls)
    type Fill = (*const c_void, usize, u32);
    extern "C" fn rows(user: *mut c_void, _query: Str, out: *mut Slice<PxuiPickRow>) {
        let fill = unsafe { &mut *user.cast::<Fill>() };
        fill.2 += 1;
        unsafe { *out = Slice { ptr: fill.0.cast(), len: fill.1 }; }
    }
    let ui = create();
    let pick = |fill: &mut Fill| in_frame(ui, &mut |ui| {
        let (mut query, mut tag, mut index) = (Str::default(), 0u32, 0i64);
        let keyed = pxui_types::ffi::PxuiKeyedDesc::new("pick", "Search");
        unsafe { pxui_ui_picker_keyed(ui, &keyed, 5, 0, 1, Str { ptr: b"s".as_ptr(), len: 1 }, Some(rows), None, None, (fill as *mut Fill).cast(), &mut query, &mut tag, &mut index) }
    });
    let row = PxuiPickRow { art_kind: 1, art: 0x266B, heading: Str { ptr: b"S".as_ptr(), len: 1 }, ..PxuiPickRow::new("Song", "Artist") };
    let good = [row, PxuiPickRow { art_kind: 2, art: 7, ..PxuiPickRow::new("Album", "") }];
    let mut fill: Fill = (good.as_ptr().cast(), 2, 0);
    assert_eq!(pick(&mut fill), 0, "{}", error(ui));
    assert!(fill.2 > 0, "the fill ran");
    // a pre-1.86 fill: two `PxuiStr`s (one label and detail pair) whose pointers would fault if read
    let poison = [Str { ptr: 0xdead_0001usize as *const u8, len: 4 }, Str { ptr: 0xdead_1001usize as *const u8, len: 6 }];
    let refused = |ptr: *const c_void, len: usize, why: &str| {
        let mut fill: Fill = (ptr, len, 0);
        assert_ne!(pick(&mut fill), 0, "{why}");
        assert!(fill.2 > 0, "{why}: the fill ran");
    };
    refused(poison.as_ptr().cast(), 2, "a pre-1.86 PxuiStr fill");
    let bad = |r: PxuiPickRow| [good[0], r];
    refused(bad(PxuiPickRow { size: 48, ..good[1] }).as_ptr().cast(), 2, "a later row of another size");
    refused(bad(PxuiPickRow { art_kind: 3, ..good[1] }).as_ptr().cast(), 2, "an unknown art kind");
    refused(bad(PxuiPickRow { art_kind: 1, art: 0xD800, ..good[1] }).as_ptr().cast(), 2, "a glyph that is not a Unicode scalar value");
    refused(bad(PxuiPickRow { art_kind: 0, art: 5, ..good[1] }).as_ptr().cast(), 2, "art with no kind");
    refused(bad(PxuiPickRow { reserved: [0, 1, 0], ..good[1] }).as_ptr().cast(), 2, "a non-zero reserved byte");
    refused(bad(PxuiPickRow { heading: Str { ptr: b"\xff".as_ptr(), len: 1 }, ..good[1] }).as_ptr().cast(), 2, "a heading that is not UTF-8");
    refused(core::ptr::null(), 2, "a null span with a length");
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

// ABI 1.86: every field of a sheet is checked before its build runs; a refused sheet writes nothing and builds nothing.
#[test]
fn modal_sheet_checks_every_field_before_the_build() {
    use pxui_types::ffi::{PxuiKeyedDesc, PxuiSheetDesc};
    extern "C" fn ran(user: *mut c_void, _: *mut c_void) { unsafe { *user.cast::<u32>() += 1; } }
    let ui = create();
    let keyed = PxuiKeyedDesc::new("sheet", "Queue");
    let good = PxuiSheetDesc { size: core::mem::size_of::<PxuiSheetDesc>() as u32, has_width: 0, present: 1, reserved: [0; 2], width: 0.0, dismiss_fraction: 0.0, dismiss_velocity: 0.0, top: 0.0, max_h: 0.0 };
    let sheet = |desc: &PxuiSheetDesc, built: &mut u32, open: &mut u8, root: &mut PxuiBox| in_frame(ui, &mut |ui| unsafe {
        pxui_ui_modal_sheet(ui, &keyed, desc, Some(ran), (built as *mut u32).cast(), open, root)
    });
    let (mut built, mut open, mut root) = (0u32, 0u8, PxuiBox::default());
    assert_eq!(sheet(&good, &mut built, &mut open, &mut root), 0, "{}", error(ui));
    assert_eq!((built, open), (1, 1), "a bottom sheet builds and is open");
    assert_ne!(root.box_key, 0, "its root");
    let full = PxuiSheetDesc { present: 2, has_width: 1, width: 300.0, dismiss_fraction: 0.5, dismiss_velocity: 900.0, ..good };
    assert_eq!(sheet(&full, &mut built, &mut open, &mut root), 0, "{}", error(ui));
    for (bad, why) in [
        (PxuiSheetDesc { size: 8, ..good }, "a short record"),
        (PxuiSheetDesc { has_width: 2, ..good }, "has_width not 0 or 1"),
        (PxuiSheetDesc { present: 4, ..good }, "an unknown presentation"),
        (PxuiSheetDesc { present: 3, top: f32::NAN, ..good }, "a top that is not finite"),
        (PxuiSheetDesc { max_h: -1.0, ..good }, "a negative height cap"),
        (PxuiSheetDesc { reserved: [0, 1], ..good }, "a non-zero reserved byte"),
        (PxuiSheetDesc { has_width: 1, width: -1.0, ..good }, "a negative width"),
        (PxuiSheetDesc { dismiss_fraction: f32::NAN, ..good }, "a fraction that is not a number"),
        (PxuiSheetDesc { dismiss_velocity: f32::INFINITY, ..good }, "an infinite velocity"),
    ] {
        let (mut built, mut open, mut root) = (0u32, 9u8, PxuiBox::default());
        assert_ne!(sheet(&bad, &mut built, &mut open, &mut root), 0, "{why}");
        assert_eq!((built, open), (0, 9), "{why}: no build, no output");
    }
    let mut root = PxuiBox::default();
    assert_ne!(in_frame(ui, &mut |ui| unsafe { pxui_ui_modal_sheet(ui, &keyed, core::ptr::null(), Some(ran), core::ptr::null_mut(), &mut 0, &mut root) }), 0, "a null sheet");
    assert_ne!(in_frame(ui, &mut |ui| unsafe { pxui_ui_modal_sheet(ui, &keyed, &good, Some(ran), core::ptr::null_mut(), core::ptr::null_mut(), &mut root) }), 0, "a null output");
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

#[test]
fn theme_tokens_that_cannot_be_a_theme_make_none() {
    use pxui_types::ffi::ThemeTokens;
    let good = ThemeTokens::from(pxui_types::theme::Tokens::DEFAULT);
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
    let address = ui as usize;
    // the static wrapper's and a C export's callback handle (`Borrowed`) and `call_build`'s (`Current`): each restores the owner when its callback unwinds
    let guard = std::thread::spawn(move || {
        let ui = address as *mut PxuiUi;
        let mut engine = pxui_core::ui::Ui::create(input::Platform::Other, None, pxui_core::text::Text::new(&[pxui_core::text::PRAGMASEVKA]));
        let mut borrowed = unsafe { crate::state::Borrowed::new(&mut engine, ui) };
        let handle = unsafe { borrowed.enter() };
        assert_eq!(unsafe { ((*(*ui).shared).current, (*(*ui).shared).building) }, (handle, handle));
        panic!("callback");
    }).join();
    assert!(guard.is_err());
    assert_eq!(unsafe { (*shared).current }, ui, "the owner is current again");
    let guard = std::thread::spawn(move || {
        let ui = address as *mut PxuiUi;
        let mut engine = pxui_core::ui::Ui::create(input::Platform::Other, None, pxui_core::text::Text::new(&[pxui_core::text::PRAGMASEVKA]));
        let shared = unsafe { (*ui).shared };
        let mut handle = PxuiUi::borrowed(&mut engine, shared);
        let _current = crate::state::Current::set(shared, &mut handle);
        panic!("callback");
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

/// ABI 1.87: a section list's record is checked before any callback (fixed and measured rows); the build gets the lead-in, heading,
/// rows and trailer (`usize::MAX - 2`) with their logical Y, and every builder's parent is restored after it.
#[test]
fn section_list_validates_before_callbacks_and_builds_every_part() {
    use pxui_types::ffi::{PxuiKeyedDesc, PxuiListResult, PxuiSectionListDesc};
    type Seen = (usize, [bool; 3]);
    extern "C" fn part(user: *mut c_void, raw: *mut c_void, index: usize, y: f32) {
        let seen = unsafe { &mut *user.cast::<Seen>() };
        assert!(y.is_finite());
        if index >= usize::MAX - 2 { seen.1[usize::MAX - index] = true; }
        seen.0 += 1;
        let mut node = PxuiBox::default();
        assert_eq!(unsafe { pxui_ui_node_keyed(raw.cast(), 17, &pxui_types::Node { h: pxui_types::Size::Px(20.0), ..pxui_types::Node::DEFAULT }, &mut node) }, 0);
    }
    extern "C" fn build(user: *mut c_void, raw: *mut c_void) {
        let ui = raw.cast::<PxuiUi>();
        let keyed = PxuiKeyedDesc::new("section", "");
        let valid = PxuiSectionListDesc { size: core::mem::size_of::<PxuiSectionListDesc>() as u32, overlay_scrollbar: 1, measured: 0, reorder: 0, reserved: 0, count: 3,
            row_height: 20.0, leading_height: 30.0, header_height: 20.0, trailing_height: 40.0 };
        let mut out = PxuiListResult { selected: 7, ..PxuiListResult::EMPTY };
        let mut bad = [valid; 12];
        bad[0].size -= 1; bad[1].overlay_scrollbar = 2; bad[2].reserved = 1; bad[3].measured = 2; bad[4].count = 1_000_001;
        bad[5].row_height = 0.0; bad[6].leading_height = -1.0; bad[7].header_height = f32::NAN; bad[8].trailing_height = f32::INFINITY;
        bad[9].row_height = f32::MAX; bad[10].row_height = -20.0; bad[11].reorder = 2;
        let before = unsafe { (*user.cast::<Seen>()).0 };
        for desc in &bad {
            assert_eq!(unsafe { pxui_ui_section_list(ui, &keyed, desc, None, Some(part), user, &mut out) }, PXUI_ERR_ARGUMENT);
            assert_eq!(out.selected, 7, "an invalid record leaves the result unchanged");
            assert_eq!(unsafe { (*user.cast::<Seen>()).0 }, before, "an invalid record calls no builder");
        }
        assert_eq!(unsafe { pxui_ui_section_list(ui, core::ptr::null(), &valid, None, Some(part), user, &mut out) }, PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { pxui_ui_section_list(ui, &keyed, core::ptr::null(), None, Some(part), user, &mut out) }, PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { pxui_ui_section_list(ui, &keyed, &valid, None, Some(part), user, core::ptr::null_mut()) }, PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { pxui_ui_section_list(ui, &keyed, &valid, None, Some(part), user, &mut PxuiListResult { size: 8, ..PxuiListResult::EMPTY }) }, PXUI_ERR_ARGUMENT, "a short result record");
        assert_eq!(unsafe { (*user.cast::<Seen>()).0 }, before);
        assert_eq!(unsafe { pxui_ui_section_list(ui, &keyed, &valid, None, Some(part), user, &mut out) }, 0);
        assert_eq!(unsafe { pxui_ui_section_list(ui, &PxuiKeyedDesc::new("measured", ""), &PxuiSectionListDesc { measured: 1, ..valid }, None, Some(part), user, &mut out) }, 0, "measured rows");
        assert_eq!((out.activated, out.selected, out.anchor), (usize::MAX, usize::MAX, usize::MAX), "a section list selects nothing");
        let mut node = PxuiBox::default();
        assert_eq!(unsafe { pxui_ui_node_keyed(ui, 19, &pxui_types::Node::DEFAULT, &mut node) }, 0, "the builder's parent is restored");
    }
    let ui = create();
    let mut seen: Seen = (0, [false; 3]);
    assert_eq!(unsafe { pxui_ui_section_list(core::ptr::null_mut(), core::ptr::null(), core::ptr::null(), None, None, core::ptr::null_mut(), core::ptr::null_mut()) }, PXUI_ERR_ARGUMENT);
    assert_eq!(run(ui, Some(build), (&mut seen as *mut Seen).cast()), 0, "{}", error(ui));
    assert_eq!(seen.1, [true; 3], "lead-in, heading and trailer each built");
    assert!((10..40).contains(&seen.0), "{} parts", seen.0);
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

/// ABI 1.87: section list, selection list and table rows (and so their children) are keyed by `key(user, i)`, so an id's row keeps its
/// key when the items reorder; a NULL key keys them by index.
#[test]
fn list_and_table_rows_key_by_item_id() {
    use pxui_types::ffi::{PxuiColumn, PxuiColumnWidth, PxuiColumnWidthSpan, PxuiKeyedDesc, PxuiListResult, PxuiSectionListDesc, PxuiSelectionListDesc, PxuiTableDesc, PxuiTableResult, PxuiU8MutSpan, Slice};
    type State = ([i64; 3], [[i64; 3]; 3], [u8; 3], [PxuiColumnWidth; 1], usize);
    fn child(raw: *mut c_void) -> i64 {
        let mut node = PxuiBox::default();
        assert_eq!(unsafe { crate::pxui_ui_node(raw.cast(), Str { ptr: b"title".as_ptr(), len: 5 }, &pxui_types::Node::DEFAULT, &mut node) }, 0);
        node.box_key
    }
    extern "C" fn part(user: *mut c_void, raw: *mut c_void, index: usize, _: f32) { let s = unsafe { &mut *user.cast::<State>() }; if index < 3 { s.1[0][index] = child(raw); } }
    extern "C" fn cell(user: *mut c_void, raw: *mut c_void, index: usize, _: *const pxui_types::ffi::PxuiCellState) { let s = unsafe { &mut *user.cast::<State>() }; if index < 3 { s.1[1][index] = child(raw); } }
    extern "C" fn table_cell(user: *mut c_void, raw: *mut c_void, row: usize, _: usize, _: *const pxui_types::ffi::PxuiCellState) { let s = unsafe { &mut *user.cast::<State>() }; s.1[2][row] = child(raw); }
    extern "C" fn key(user: *mut c_void, row: usize) -> i64 { unsafe { (*user.cast::<State>()).0[row] } }
    extern "C" fn build(user: *mut c_void, raw: *mut c_void) {
        let (ui, state) = (raw.cast::<PxuiUi>(), unsafe { &mut *user.cast::<State>() });
        let mut out = PxuiListResult::EMPTY;
        let section = PxuiSectionListDesc { size: core::mem::size_of::<PxuiSectionListDesc>() as u32, overlay_scrollbar: 0, measured: 0, reorder: 0, reserved: 0, count: 3, row_height: 20.0, leading_height: 0.0, header_height: 0.0, trailing_height: 0.0 };
        assert_eq!(unsafe { pxui_ui_section_list(ui, &PxuiKeyedDesc::new("section", ""), &section, Some(key), Some(part), user, &mut out) }, 0);
        assert_eq!(unsafe { pxui_ui_section_list(ui, &PxuiKeyedDesc::new("plain", ""), &section, None, None, user, &mut out) }, 0, "no key: by index");
        let mut list: PxuiSelectionListDesc = unsafe { core::mem::zeroed() };
        (list.size, list.count, list.row_height, list.rows, list.anchor) = (core::mem::size_of::<PxuiSelectionListDesc>() as u32, 3, 20.0, PxuiU8MutSpan { ptr: state.2.as_mut_ptr(), len: 3 }, usize::MAX);
        assert_eq!(unsafe { pxui_ui_selection_list(ui, &PxuiKeyedDesc::new("list", ""), &list, Some(key), Some(cell), user, &mut out) }, 0);
        let columns = [PxuiColumn { size: core::mem::size_of::<PxuiColumn>() as u32, sortable: 0, grip: 0, has_id: 0, reserved: 0, title: Str { ptr: b"A".as_ptr(), len: 1 }, id: 0, share: 1.0, w: 0.0 }];
        let mut table: PxuiTableDesc = unsafe { core::mem::zeroed() };
        (table.size, table.columns, table.widths, table.rows, table.row_h, table.head_h, table.anchor) = (core::mem::size_of::<PxuiTableDesc>() as u32, Slice { ptr: columns.as_ptr(), len: 1 },
            PxuiColumnWidthSpan { ptr: state.3.as_mut_ptr(), len: 1 }, 3, 20.0, 20.0, usize::MAX);
        table.flags = PxuiU8MutSpan { ptr: state.2.as_mut_ptr(), len: 3 };
        let mut result: PxuiTableResult = unsafe { core::mem::zeroed() };
        result.size = core::mem::size_of::<PxuiTableResult>() as u32;
        assert_eq!(unsafe { pxui_ui_table(ui, &PxuiKeyedDesc::new("table", ""), &table, Some(key), Some(table_cell), user, &mut result) }, 0);
        state.4 += 1;
    }
    let ui = create();
    let mut state: State = ([10, 20, 30], [[0; 3]; 3], [0; 3], [PxuiColumnWidth::default()], 0);
    assert_eq!(run(ui, Some(build), (&mut state as *mut State).cast()), 0, "{}", error(ui));
    let before = state.1;
    state.0.reverse();
    assert_eq!(run(ui, Some(build), (&mut state as *mut State).cast()), 0, "{}", error(ui));
    for (now, was) in state.1.iter().zip(before.iter()) {
        assert_eq!((now[0], now[2]), (was[2], was[0]), "each id's row (and its child) keeps its key");
        assert_ne!(was[0], was[1]);
    }
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

/// ABI 1.87: the selection list, the table and the heading check every record, flag span and column in one pass before any effect: a
/// refused call runs no cell builder and writes no result; a table's columns are size-prefixed, carry 0/1 flags, a UTF-8 title, finite
/// non-negative share and width, unique ids, at most 64, with one width entry a column.
#[test]
fn selection_list_table_and_heading_refuse_invalid_records_before_effects() {
    use pxui_types::ffi::{PxuiColumn, PxuiColumnWidth, PxuiColumnWidthSpan, PxuiHeadResult, PxuiKeyedDesc, PxuiListResult, PxuiSelectionListDesc, PxuiTableDesc, PxuiTableHeadDesc, PxuiTableResult, PxuiU8MutSpan, Slice};
    extern "C" fn cell(user: *mut c_void, _: *mut c_void, _: usize, _: *const pxui_types::ffi::PxuiCellState) { unsafe { *user.cast::<u32>() += 1; } }
    extern "C" fn table_cell(user: *mut c_void, _: *mut c_void, _: usize, _: usize, _: *const pxui_types::ffi::PxuiCellState) { unsafe { *user.cast::<u32>() += 1; } }
    let ui = create();
    let keyed = PxuiKeyedDesc::new("k", "");
    let mut calls = 0u32;
    let user = (&mut calls as *mut u32).cast::<c_void>();
    let mut flags = [0u8; 4];
    let fp = flags.as_mut_ptr();
    let mut list: PxuiSelectionListDesc = unsafe { core::mem::zeroed() };
    (list.size, list.count, list.row_height, list.rows, list.anchor) = (core::mem::size_of::<PxuiSelectionListDesc>() as u32, 4, 20.0, PxuiU8MutSpan { ptr: flags.as_mut_ptr(), len: 4 }, usize::MAX);
    let refuse_list = |d: PxuiSelectionListDesc, why: &str| {
        let mut out = PxuiListResult { selected: 7, ..PxuiListResult::EMPTY };
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_selection_list(ui, &keyed, &d, None, Some(cell), user, &mut out) }), PXUI_ERR_ARGUMENT, "{why}");
        assert_eq!(out.selected, 7, "{why}: no result written");
    };
    refuse_list(PxuiSelectionListDesc { multi: 2, ..list }, "multi not 0 or 1");
    refuse_list(PxuiSelectionListDesc { fit: 2, ..list }, "fit not 0 or 1");
    refuse_list(PxuiSelectionListDesc { open_on_click: 2, ..list }, "open_on_click not 0 or 1");
    refuse_list(PxuiSelectionListDesc { reserved: 1, ..list }, "a reserved byte");
    refuse_list(PxuiSelectionListDesc { reorder: 2, ..list }, "reorder not 0 or 1");
    refuse_list(PxuiSelectionListDesc { has_drop_look: 2, ..list }, "has_drop_look not 0 or 1");
    refuse_list(PxuiSelectionListDesc { has_drop_look: 1, drop_look: pxui_types::ffi::PxuiLookDesc { opacity: f32::NAN, ..list.drop_look }, ..list }, "a drop look the look check refuses");
    refuse_list(PxuiSelectionListDesc { rows: PxuiU8MutSpan { ptr: flags.as_mut_ptr(), len: 3 }, ..list }, "fewer flags than rows");
    refuse_list(PxuiSelectionListDesc { rows: PxuiU8MutSpan { ptr: core::ptr::null_mut(), len: 4 }, ..list }, "a null flag span");
    refuse_list(PxuiSelectionListDesc { row_height: f32::NAN, ..list }, "a NaN row height");
    assert_eq!(calls, 0, "a refused list builds no cell");
    let mut out = PxuiListResult::EMPTY;
    assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_selection_list(ui, &keyed, &list, None, Some(cell), user, &mut out) }), 0, "{}", error(ui));
    assert!(calls >= 4, "the valid list builds its rows");

    let col = |title: &'static [u8], id: Option<i64>| PxuiColumn { size: core::mem::size_of::<PxuiColumn>() as u32, sortable: 1, grip: 1, has_id: id.is_some() as u8, reserved: 0,
        title: Str { ptr: title.as_ptr(), len: title.len() }, id: id.unwrap_or(0), share: 0.0, w: 80.0 };
    let good = [col(b"Name", Some(4)), col(b"Size", None)];
    let mut widths = [PxuiColumnWidth::default(); 65];
    let table_of = |columns: &[PxuiColumn], widths: &mut [PxuiColumnWidth], width_len: usize| {
        let mut t: PxuiTableDesc = unsafe { core::mem::zeroed() };
        (t.size, t.columns, t.widths, t.rows, t.row_h, t.head_h, t.anchor) = (core::mem::size_of::<PxuiTableDesc>() as u32, Slice { ptr: columns.as_ptr(), len: columns.len() },
            PxuiColumnWidthSpan { ptr: widths.as_mut_ptr(), len: width_len }, 4, 20.0, 20.0, usize::MAX);
        t.flags = PxuiU8MutSpan { ptr: fp, len: 4 };
        t
    };
    let refuse_table = |t: PxuiTableDesc, why: &str| {
        let mut out: PxuiTableResult = unsafe { core::mem::zeroed() };
        (out.size, out.rows.selected) = (core::mem::size_of::<PxuiTableResult>() as u32, 7);
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_table(ui, &keyed, &t, None, Some(table_cell), user, &mut out) }), PXUI_ERR_ARGUMENT, "{why}");
        assert_eq!(out.rows.selected, 7, "{why}: no result written");
        let mut head: PxuiTableHeadDesc = unsafe { core::mem::zeroed() };
        (head.size, head.columns, head.widths, head.h) = (core::mem::size_of::<PxuiTableHeadDesc>() as u32, t.columns, t.widths, 20.0);
        let mut hout: PxuiHeadResult = unsafe { core::mem::zeroed() };
        (hout.size, hout.sort_id) = (core::mem::size_of::<PxuiHeadResult>() as u32, 7);
        if t.multi == 0 && t.rows <= t.flags.len {
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_table_head(ui, &keyed, &head, &mut hout) }), PXUI_ERR_ARGUMENT, "heading: {why}");
            assert_eq!(hout.sort_id, 7, "heading: {why}: no result written");
        }
    };
    let before = calls;
    let wp = widths.as_mut_ptr();
    let ws = |n: usize| unsafe { core::slice::from_raw_parts_mut(wp, n) };
    refuse_table(table_of(&[PxuiColumn { size: 8, ..good[0] }, good[1]], ws(2), 2), "a short column record");
    refuse_table(table_of(&[PxuiColumn { sortable: 2, ..good[0] }, good[1]], ws(2), 2), "sortable not 0 or 1");
    refuse_table(table_of(&[PxuiColumn { grip: 2, ..good[0] }, good[1]], ws(2), 2), "grip not 0 or 1");
    refuse_table(table_of(&[PxuiColumn { reserved: 1, ..good[0] }, good[1]], ws(2), 2), "a reserved byte");
    refuse_table(table_of(&[col(b"\xff", None)], ws(1), 1), "a title that is not UTF-8");
    refuse_table(table_of(&[PxuiColumn { share: f32::NAN, ..good[0] }], ws(1), 1), "a NaN share");
    refuse_table(table_of(&[PxuiColumn { w: -1.0, ..good[0] }], ws(1), 1), "a negative width");
    refuse_table(table_of(&[col(b"A", Some(1)), col(b"B", Some(1))], ws(2), 2), "duplicate ids");
    refuse_table(table_of(&[col(b"A", Some(1)), col(b"B", None)], ws(2), 2), "an index id colliding with an explicit one");
    refuse_table(table_of(&good, ws(2), 1), "fewer widths than columns");
    refuse_table(table_of(&[good[1]; 65], ws(65), 65), "more than 64 columns");
    refuse_table(PxuiTableDesc { multi: 2, ..table_of(&good, ws(2), 2) }, "multi not 0 or 1");
    refuse_table(PxuiTableDesc { flags: PxuiU8MutSpan { ptr: fp, len: 3 }, ..table_of(&good, ws(2), 2) }, "fewer flags than rows");
    assert_eq!(calls, before, "a refused table builds no cell");
    let mut out: PxuiTableResult = unsafe { core::mem::zeroed() };
    out.size = core::mem::size_of::<PxuiTableResult>() as u32;
    let t = table_of(&good, ws(2), 2);
    assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_table(ui, &keyed, &t, None, Some(table_cell), user, &mut out) }), 0, "{}", error(ui));
    assert!(calls >= before + 8, "the valid table builds its cells");
    assert_eq!((widths[0].id, widths[1].id), (4, 1), "each column's id (an index without one) is written with its width");
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

// ABI 1.84: every tab argument is checked before the bar is built; a refused call writes no result.
#[test]
fn tabs_result_refuses_invalid_arguments_and_items() {
    use pxui_types::ffi::{PxuiKeyedDesc, PxuiPickItem, PxuiResponse, PxuiSegmentedResult, Slice};
    let item = |text: &'static [u8]| PxuiPickItem { text: Str { ptr: text.as_ptr(), len: text.len() }, ..PxuiPickItem::default() };
    let good = [item(b"Songs"), PxuiPickItem { has_id: 1, id: 7, detail: Str { ptr: b"12".as_ptr(), len: 2 }, ..item(b"Albums") }];
    let ui = create();
    let keyed = PxuiKeyedDesc::new("tabs", "Sections");
    let tabs = |items: &[PxuiPickItem], has_selected: u8, focus: u8, result: &mut PxuiSegmentedResult| in_frame(ui, &mut |ui| {
        let mut out = PxuiResponse::EMPTY;
        unsafe { pxui_ui_tabs_result(ui, &keyed, Slice { ptr: items.as_ptr(), len: items.len() }, has_selected, 1, focus, &mut out, result) }
    });
    let mut result = PxuiSegmentedResult::EMPTY;
    assert_eq!(tabs(&good, 1, 1, &mut result), 0, "{}", error(ui));
    assert_eq!((result.has_chosen, result.chosen), (1, 1), "the tab in use");
    let refused = |items: &[PxuiPickItem], has_selected: u8, focus: u8, why: &str| {
        let mut result = PxuiSegmentedResult { has_chosen: 9, ..PxuiSegmentedResult::EMPTY };
        assert_ne!(tabs(items, has_selected, focus, &mut result), 0, "{why}");
        assert_eq!(result.has_chosen, 9, "{why}: no result written");
    };
    refused(&good, 2, 0, "has_selected not 0 or 1");
    refused(&good, 0, 2, "focus_on_click not 0 or 1");
    refused(&[PxuiPickItem { has_id: 2, ..good[0] }], 0, 0, "has_id not 0 or 1");
    refused(&[PxuiPickItem { reserved: [0, 0, 0, 1, 0, 0, 0], ..good[0] }], 0, 0, "a non-zero reserved byte");
    refused(&[item(b"\xff")], 0, 0, "text that is not UTF-8");
    refused(&[PxuiPickItem { detail: Str { ptr: core::ptr::null(), len: 3 }, ..good[0] }], 0, 0, "a null detail with a length");
    let mut result = PxuiSegmentedResult::EMPTY;
    assert_ne!(in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_tabs_result(ui, &keyed, Slice { ptr: core::ptr::null(), len: 2 }, 0, 0, 0, &mut out, &mut result) } }), 0, "a null span with a length");
    assert_ne!(in_frame(ui, &mut |ui| unsafe { pxui_ui_tabs_result(ui, &keyed, Slice { ptr: good.as_ptr(), len: 2 }, 0, 0, 0, core::ptr::null_mut(), &mut result) }), 0, "a null response");
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

// ABI 1.85: the section index and the disclosure check every argument before building; a refused call writes no result and never runs
// the disclosure's build.
#[test]
fn section_index_and_disclosure_refuse_invalid_arguments() {
    use pxui_types::ffi::{PxuiKeyedDesc, PxuiResponse, PxuiSegmentedResult, Slice};
    let s = |t: &'static [u8]| Str { ptr: t.as_ptr(), len: t.len() };
    let letters = [s(b"A"), s(b"B")];
    let ui = create();
    let keyed = PxuiKeyedDesc::new("index", "Jump to");
    let index = |letters: Slice<Str>, keyed: &PxuiKeyedDesc, result: &mut PxuiSegmentedResult| in_frame(ui, &mut |ui| {
        let mut out = PxuiResponse::EMPTY;
        unsafe { pxui_ui_section_index_result(ui, keyed, letters, &mut out, result) }
    });
    let mut result = PxuiSegmentedResult::EMPTY;
    assert_eq!(index(Slice { ptr: letters.as_ptr(), len: 2 }, &keyed, &mut result), 0, "{}", error(ui));
    assert_eq!(result.has_chosen, 0, "nothing moved it");
    let refused = |letters: Slice<Str>, keyed: &PxuiKeyedDesc, why: &str| {
        let mut result = PxuiSegmentedResult { has_chosen: 9, ..PxuiSegmentedResult::EMPTY };
        assert_ne!(index(letters, keyed, &mut result), 0, "{why}");
        assert_eq!(result.has_chosen, 9, "{why}: no result written");
    };
    let bad = [s(b"A"), s(b"\xff")];
    refused(Slice { ptr: bad.as_ptr(), len: 2 }, &keyed, "a letter that is not UTF-8");
    refused(Slice { ptr: core::ptr::null(), len: 2 }, &keyed, "a null span with a length");
    refused(Slice { ptr: letters.as_ptr(), len: 5000 }, &keyed, "more than 4096 letters");
    refused(Slice { ptr: letters.as_ptr(), len: 2 }, &PxuiKeyedDesc { size: 8, ..keyed }, "a short keyed record");
    refused(Slice { ptr: letters.as_ptr(), len: 2 }, &PxuiKeyedDesc { reserved: 1, ..keyed }, "a reserved bit");
    assert_ne!(in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_section_index_result(ui, &keyed, Slice { ptr: letters.as_ptr(), len: 2 }, &mut out, core::ptr::null_mut()) } }), 0, "a null result");

    extern "C" fn ran(user: *mut c_void, _: *mut c_void) { unsafe { *user.cast::<u32>() += 1; } }
    let mut built = 0u32;
    let more = PxuiKeyedDesc::new("more", "More");
    let disclosure = |keyed: &PxuiKeyedDesc, open: u8, built: &mut u32| in_frame(ui, &mut |ui| {
        let mut out = PxuiResponse::EMPTY;
        unsafe { pxui_ui_disclosure(ui, keyed, open, Some(ran), (built as *mut u32).cast(), &mut out) }
    });
    assert_eq!(disclosure(&more, 1, &mut built), 0, "{}", error(ui));
    assert_eq!(built, 1, "an open disclosure builds its content");
    assert_eq!(disclosure(&more, 0, &mut built), 0);
    assert_eq!(built, 1, "a closed one does not");
    assert_ne!(disclosure(&more, 2, &mut built), 0, "open not 0 or 1");
    assert_ne!(disclosure(&PxuiKeyedDesc { size: 8, ..more }, 1, &mut built), 0, "a short keyed record");
    assert_ne!(disclosure(&PxuiKeyedDesc { name: s(b"\xff"), ..more }, 1, &mut built), 0, "a name that is not UTF-8");
    assert_eq!(built, 1, "a refused disclosure never builds");
    assert_ne!(in_frame(ui, &mut |ui| unsafe { pxui_ui_disclosure(ui, &more, 1, Some(ran), (&mut built as *mut u32).cast(), core::ptr::null_mut()) }), 0, "a null response");
    assert_eq!(built, 1);
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
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

fn str_of(bytes: &[u8]) -> Str { Str { ptr: bytes.as_ptr(), len: bytes.len() } }
fn frame_with(ui: *mut PxuiUi, input: &FrameIn) -> i32 { let mut out = FrameOut::default(); unsafe { pxui_frame(ui, input, None, core::ptr::null_mut(), &mut out) } }

/// One validator for the frame entries and `pxui_ui_wants_frame`: coordinates and values finite for every kind, a text editing range
/// not negative, a resize within the surface limit, each event's text within `MAX_TEXT_BYTES` and a frame's within
/// `MAX_TOTAL_TEXT_BYTES`; refused, never cut. A settled `wants_frame` allocates nothing.
#[test]
fn events_are_validated_alike_on_every_path_with_a_frame_text_budget() {
    use pxui_types::Event as E;
    let ui = create();
    let big = vec![b'a'; crate::state::MAX_TOTAL_TEXT_BYTES / 2 + 1];
    let refused = [
        vec![E { kind: E::POINTER_MOVED, x: f64::NAN, ..E::default() }],
        vec![E { kind: E::KEY_PRESSED, code: input::Key::Enter.code(), value: f64::INFINITY, ..E::default() }],
        vec![E { kind: E::SCROLLED, y: f64::NEG_INFINITY, ..E::default() }],
        vec![E { kind: E::TEXT_EDITING, x: -1.0, text: str_of(b"ime"), ..E::default() }],
        vec![E { kind: E::RESIZED, x: 70_000.0, y: 10.0, ..E::default() }],
        vec![E { kind: E::RESIZED, x: -1.0, y: 10.0, ..E::default() }],
        // each text within the cap, the two past the frame's budget
        vec![E { kind: E::PASTE, text: str_of(&big), ..E::default() }, E { kind: E::PASTE, text: str_of(&big), ..E::default() }],
        // a length past the cap is refused before a byte is read
        vec![E { kind: E::TEXT_INPUT, text: Str { ptr: b"x".as_ptr(), len: crate::state::MAX_TEXT_BYTES + 1 }, ..E::default() }],
    ];
    for (i, events) in refused.iter().enumerate() {
        let input = FrameIn { events: events.as_ptr(), event_count: events.len(), ..frame_in() };
        assert_eq!(frame_with(ui, &input), PXUI_ERR_ARGUMENT, "frame, case {i}");
        let mut wants = 7u8;
        assert_eq!(unsafe { pxui_ui_wants_frame(ui, pxui_types::ffi::Slice { ptr: events.as_ptr(), len: events.len() }, 0.0, &mut wants) }, PXUI_ERR_ARGUMENT, "wants_frame, case {i}");
        assert_eq!(wants, 7);
    }
    // one paste of half the budget is fine, and so is a resize at the limit
    let fine = [E { kind: E::PASTE, text: str_of(&big), ..E::default() }, E { kind: E::RESIZED, x: 65_536.0, y: 0.0, ..E::default() }];
    assert_eq!(frame_with(ui, &FrameIn { events: fine.as_ptr(), event_count: 2, ..frame_in() }), 0, "{}", error(ui));
    let moves = [E { kind: E::POINTER_MOVED, x: 1.0, y: 2.0, ..E::default() }; 3];
    let mut wants = 0u8;
    for _ in 0..2 { assert_eq!(unsafe { pxui_ui_wants_frame(ui, pxui_types::ffi::Slice { ptr: moves.as_ptr(), len: 3 }, 0.0, &mut wants) }, 0); }
    mem_sys::local_reset();
    let before = mem_sys::local_alloc_count();
    assert_eq!(unsafe { pxui_ui_wants_frame(ui, pxui_types::ffi::Slice { ptr: moves.as_ptr(), len: 3 }, 0.0, &mut wants) }, 0);
    assert_eq!(mem_sys::local_alloc_count() - before, 0, "a settled wants_frame allocates");
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

/// A frame's size and the host input's logical size are within `0..=MAX_SURFACE`; a held key code must name a key.
#[test]
fn frame_sizes_and_held_keys_are_checked() {
    let ui = create();
    for (w, h) in [(-1.0, 10.0), (10.0, 1e30), (f32::NAN, 10.0), (65_537.0, 10.0)] {
        assert_eq!(frame_with(ui, &FrameIn { width: w, height: h, ..frame_in() }), PXUI_ERR_ARGUMENT, "{w} x {h}");
    }
    assert_eq!(frame_with(ui, &FrameIn { width: 65_536.0, height: 0.0, ..frame_in() }), 0);
    let mut out = FrameOut::default();
    let mut held = |keys: &[u32], size: [i64; 2]| {
        let input = pxui_types::ffi::Input { logical_size: size, keys: pxui_types::ffi::Slice { ptr: keys.as_ptr(), len: keys.len() }, ..Default::default() };
        unsafe { pxui_ui_frame(ui, &input, None, core::ptr::null_mut(), &mut out) }
    };
    assert_eq!(held(&[], [-1, 10]), PXUI_ERR_ARGUMENT);
    assert_eq!(held(&[], [10, 1 << 17]), PXUI_ERR_ARGUMENT);
    assert_eq!(held(&[0], [10, 10]), PXUI_ERR_ARGUMENT, "code 0 names no key");
    assert_eq!(held(&[input::Key::Enter.code()], [10, 10]), 0);
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

/// One checker per role: an identifier at most `MAX_KEY_BYTES`, display text at most `MAX_LABEL_BYTES`, editor text at most
/// `MAX_TEXT_BYTES`, on the C path and on the trusted (static wrapper) path alike.
#[test]
fn strings_are_capped_by_role() {
    use crate::state::{MAX_KEY_BYTES, MAX_LABEL_BYTES};
    extern "C" fn build(_: *mut c_void, ui: *mut c_void) {
        let ui = ui.cast::<PxuiUi>();
        let long = vec![b'k'; MAX_LABEL_BYTES + 1];
        let (key, label) = (&long[..MAX_KEY_BYTES], &long[..MAX_LABEL_BYTES]);
        let mut out = PxuiBox::default();
        assert_eq!(unsafe { pxui_ui_node(ui, str_of(key), &pxui_types::Node::DEFAULT, &mut out) }, 0);
        assert_eq!(unsafe { pxui_ui_node(ui, str_of(&long[..MAX_KEY_BYTES + 1]), &pxui_types::Node::DEFAULT, &mut out) }, PXUI_ERR_ARGUMENT);
        let key_str = core::str::from_utf8(&long[..MAX_KEY_BYTES + 1]).unwrap();
        assert_eq!(unsafe { crate::trusted::node_str(ui, key_str, &pxui_types::Node::DEFAULT, &mut out) }, PXUI_ERR_ARGUMENT);
        let b = { let mut b = PxuiBox::default(); assert_eq!(unsafe { pxui_ui_node(ui, str_of(b"t"), &pxui_types::Node::DEFAULT, &mut b) }, 0); b };
        assert_eq!(unsafe { pxui_ui_text(ui, &b, str_of(label)) }, 0);
        assert_eq!(unsafe { pxui_ui_text(ui, &b, str_of(&long)) }, PXUI_ERR_ARGUMENT);
        // a text field's value is editor text: past the label cap is fine
        let (mut changed, mut flag) = (Str::default(), 0u8);
        assert_eq!(unsafe { pxui_text_field(ui, str_of(b"f"), str_of(&long), &mut changed, &mut flag) }, 0, "{}", error(ui));
    }
    let ui = create();
    assert_eq!(run(ui, Some(build), core::ptr::null_mut()), 0, "{}", error(ui));
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

/// Strings handed back between frames are bounded: past `MAX_RETURNED` a call answers `PXUI_ERR_STATE`; the next frame frees them.
#[test]
fn strings_handed_back_between_frames_are_bounded() {
    let ui = create();
    let (mut text, mut caret, mut anchor) = (Str::default(), 0u64, 0u64);
    let load = |text: &mut Str, caret: &mut u64, anchor: &mut u64| unsafe { pxui_ui_load_text_edit(ui, 5, str_of(b"abc"), text, caret, anchor) };
    for _ in 0..crate::state::MAX_RETURNED { assert_eq!(load(&mut text, &mut caret, &mut anchor), 0); }
    assert_eq!(load(&mut text, &mut caret, &mut anchor), PXUI_ERR_STATE);
    assert!(error(ui).contains("handed back"), "{}", error(ui));
    assert_eq!(run(ui, Some(nothing), core::ptr::null_mut()), 0);
    assert_eq!(load(&mut text, &mut caret, &mut anchor), 0);
    assert_eq!(unsafe { core::slice::from_raw_parts(text.ptr, text.len) }, b"abc");
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

/// A shaper needs a font: none (or only refused ones) is refused at create, never a panic at its first text.
#[test]
fn a_shaper_without_a_font_is_refused_at_create() {
    let font = unsafe { pxui_default_font() };
    let empty = Str::default();
    unsafe {
        assert!(pxui_shaper_create(core::ptr::null(), 0).is_null());
        assert!(pxui_shaper_create(&font, 0).is_null());
        assert!(pxui_shaper_create(core::ptr::null(), 1).is_null());
        assert!(pxui_shaper_create(&empty, 1).is_null());
        assert!(pxui_shaper_create(&str_of(b"not a font"), 1).is_null());
        let shaper = pxui_shaper_create(&font, 1);
        let (style, mut metrics) = (pxui_types::ffi::PxuiTextStyle::default(), pxui_types::TextMetrics::default());
        assert_eq!(pxui_shaper_width(shaper, 0, str_of(b"a"), &style, 0.0, &mut metrics), 0);
        pxui_shaper_destroy(shaper);
    }
}

/// A shaper or theme whose call panicked fails every later call with its failure value and is still freed by its destroy.
#[test]
fn a_poisoned_shaper_or_theme_fails_its_calls_and_is_still_freed() {
    let font = unsafe { pxui_default_font() };
    let shaper = unsafe { pxui_shaper_create(&font, 1) };
    let style = pxui_types::ffi::PxuiTextStyle::default();
    let mut metrics = pxui_types::TextMetrics::default();
    assert_eq!(unsafe { pxui_shaper_width(shaper, 0, str_of(b"a"), &style, 0.0, &mut metrics) }, 0);
    unsafe { (*shaper).poisoned = true; }
    assert_eq!(unsafe { pxui_shaper_width(shaper, 0, str_of(b"a"), &style, 0.0, &mut metrics) }, PXUI_ERR_PANIC);
    assert_eq!(unsafe { pxui_shaper_add_font(shaper, str_of(b"")) }, u32::MAX);
    unsafe { pxui_shaper_destroy(shaper); }
    let theme = unsafe { pxui_theme_create(core::ptr::null()) };
    unsafe { (*theme).poisoned = true; }
    assert_eq!(unsafe { pxui_theme_set(theme, str_of(b"tokens.accent"), str_of(b"#ff0000")) }, PXUI_ERR_PANIC);
    unsafe { pxui_theme_destroy(theme); }
}

/// Calls are counted per `Ui`: one `Ui`'s calls never move another's, and reading the count is not a call.
#[test]
fn calls_are_counted_per_ui() {
    let (a, b) = (create(), create());
    let (at_a, at_b) = unsafe { (pxui_ui_calls(a), pxui_ui_calls(b)) };
    for _ in 0..5 { assert_eq!(unsafe { pxui_ui_set_dir(a, 0) }, 0); }
    assert_eq!(unsafe { pxui_ui_calls(a) } - at_a, 5);
    assert_eq!(unsafe { pxui_ui_calls(b) }, at_b);
    let mut built = 0u32;
    assert_eq!(run(b, Some(count), (&mut built as *mut u32).cast()), 0);
    assert_eq!(unsafe { pxui_ui_calls(a) } - at_a, 5);
    assert!(unsafe { pxui_ui_calls(b) } > at_b);
    assert_eq!(unsafe { pxui_ui_calls(core::ptr::null()) } | unsafe { pxui_counter(0) }, 0);
    assert_eq!(unsafe { pxui_ui_destroy(a) } | unsafe { pxui_ui_destroy(b) }, 0);
}

/// Grid nodes with equal track lists share one stored list, found by hash; a list no box holds goes at the end of a frame.
#[test]
fn grid_track_lists_are_shared_by_hash_and_retired_with_their_boxes() {
    use pxui_types::layout::{Grid, Track};
    static COLS: [Track; 2] = [Track::Px(40.0), Track::Fr(1.0)];
    static ROWS: [Track; 1] = [Track::Fit];
    extern "C" fn grids(user: *mut c_void, ui: *mut c_void) {
        let n = unsafe { *user.cast::<i64>() };
        for key in 0..n {
            let node = pxui_types::Node { grid: Some(Grid::new(&COLS, &ROWS)).into(), ..pxui_types::Node::DEFAULT };
            let mut out = PxuiBox::default();
            assert_eq!(unsafe { pxui_ui_node_keyed(ui.cast(), 100 + key, &node, &mut out) }, 0);
        }
    }
    let ui = create();
    let mut n = 3i64;
    assert_eq!(run(ui, Some(grids), (&mut n as *mut i64).cast()), 0);
    let shared = unsafe { &*(*ui).shared };
    assert_eq!((shared.grid_tracks.len(), shared.grid_shared.len()), (3, 1));
    let first = shared.grid_tracks.values().next().unwrap();
    assert!(shared.grid_tracks.values().all(|stored| std::rc::Rc::ptr_eq(stored, first)));
    n = 0;
    assert_eq!(run(ui, Some(grids), (&mut n as *mut i64).cast()), 0);
    let shared = unsafe { &*(*ui).shared };
    assert_eq!((shared.grid_tracks.len(), shared.grid_shared.len()), (0, 0));
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

// Size-prefixed records against a guard page. Each record is copied to the very end of a readable page with an inaccessible page
// behind it, so a read or write past the bytes the test supplies faults and kills the case. Five calls per record: only the size
// word present (refused), one byte short (refused), exact, a caller whose record claims 64 bytes more than the library knows (the
// extra bytes are on the guard page: accepted, never read) and a claim of u32::MAX (accepted by the same rule, never read).
mod sized_records {
    use super::*;
    use pxui_types::{FrameIn, ffi::*};

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

    /// `four` for a record that grew: `olds` are the sizes it has had before the current one (the first is the minimum). Every size from the size word up
    /// to the whole record is tried: an old size is accepted, every other (below the minimum or ending inside a field) is refused, and none reads past
    /// the bytes supplied; then exact, 64 more than known and u32::MAX as `four` does.
    fn four_min(ui: *mut PxuiUi, (at, known, align): (*const u8, usize, usize), olds: &[usize], call: &mut dyn FnMut(*mut u8) -> i32) {
        let bytes = unsafe { core::slice::from_raw_parts(at, known) };
        for size in 4..known {
            let want = if olds.contains(&size) { 0 } else { REFUSED };
            assert_eq!(call(edge(bytes, size, align, size as u32)), want, "a record of {size} bytes of {known}: {}", error(ui));
        }
        assert_eq!(call(edge(bytes, known, align, known as u32)), 0, "exact: {}", error(ui));
        assert_eq!(call(edge(bytes, known, align, known as u32 + 64)), 0, "larger than known: {}", error(ui));
        assert_eq!(call(edge(bytes, known, align, u32::MAX)), 0, "huge: {}", error(ui));
    }

    // FrameIn has had 48, 56, 128 and 152 bytes: only those and the whole record (and more) are read.
    #[test]
    fn frame_in() {
        let ui = create();
        let value = FrameIn { width: 320.0, height: 240.0, ..FrameIn::default() };
        four_min(ui, ((&value as *const FrameIn).cast(), size_of_val(&value), align_of_val(&value)), &[48, 56, 128, 152], &mut |p| {
            let mut out = FrameOut::default();
            unsafe { pxui_frame(ui, p.cast(), None, core::ptr::null_mut(), &mut out) }
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    // PxuiMenuExtra has had 80, 104, 144 and 184 bytes: only those and the whole record (and more) are read.
    #[test]
    fn menu_extra() {
        let ui = create();
        let mut value: PxuiMenuExtra = unsafe { core::mem::zeroed() };
        value.size = size_of::<PxuiMenuExtra>() as u32;
        four_min(ui, ((&value as *const PxuiMenuExtra).cast(), size_of_val(&value), align_of_val(&value)), &[80, 104, 144, 184], &mut |p| {
            let p = p.cast::<PxuiMenuExtra>();
            in_frame(ui, &mut |ui| {
                let (mut tag, mut index) = (0u32, 0i64);
                let none = Slice { ptr: core::ptr::null(), len: 0 };
                unsafe { pxui_ui_context_menu(ui, 1.0, 1.0, Str { ptr: b"m".as_ptr(), len: 1 }, none, Slice { ptr: core::ptr::null(), len: 0 }, 0, 0.0, 0, 0, p, &mut tag, &mut index) }
            })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn ui_desc() {
        let ui = create();
        let mut value: PxuiUiDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiUiDesc) = |_| {};
        fix(&mut value);
        four_min(ui, ((&value as *const PxuiUiDesc).cast(), size_of_val(&value), align_of_val(&value)), &[12], &mut |p| {
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
        four_min(ui, ((&value as *const PxuiUiDesc).cast(), size_of_val(&value), align_of_val(&value)), &[12], &mut |p| {
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
        let fix: fn(&mut ThemeTokens) = |t| *t = ThemeTokens::from(pxui_types::theme::Tokens::DEFAULT);
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
        four_min(ui, ((&value as *const PxuiButtonDesc).cast(), size_of_val(&value), align_of_val(&value)), &[96, 104, 192], &mut |p| {
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
        four_min(ui, ((&value as *const PxuiIconButtonDesc).cast(), size_of_val(&value), align_of_val(&value)), &[96, 104], &mut |p| {
            let p = p.cast::<PxuiIconButtonDesc>();
            in_frame(ui, &mut |ui| { let (mut out, mut signal, mut focused) = (PxuiBox::default(), PxuiSignal::default(), 0u8); unsafe { pxui_ui_icon_button_with(ui, p, &mut out, &mut signal, &mut focused) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    /// ABI 1.54: the 96-byte button and icon button records from before `centered` and the press curves are still read (as a plain button and
    /// the kit's springs); a size between the two, a `centered` past 1, a nonzero reserved byte or an invalid curve is refused.
    #[test]
    fn grown_button_records_take_their_old_size() {
        let ui = create();
        let k = Str { ptr: b"k".as_ptr(), len: 1 };
        let mut button: PxuiButtonDesc = unsafe { core::mem::zeroed() };
        (button.size, button.enabled, button.key, button.text, button.centered) = (size_of::<PxuiButtonDesc>() as u32, 1, k, k, 1);
        let mut icon: PxuiIconButtonDesc = unsafe { core::mem::zeroed() };
        (icon.size, icon.enabled, icon.press_scale, icon.key, icon.name, icon.icon) = (size_of::<PxuiIconButtonDesc>() as u32, 1, 0.96, k, k, k);
        let press = |b: &PxuiButtonDesc| in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_button_content(ui, b, None, core::ptr::null_mut(), &mut out) } });
        let tap = |b: &PxuiIconButtonDesc| in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_icon_button_response(ui, b, &mut out) } });
        assert_eq!(press(&button), 0);
        assert_eq!(press(&PxuiButtonDesc { size: 96, centered: 7, ..button }), 0, "a 96-byte record's tail is not read");
        assert_eq!(press(&PxuiButtonDesc { size: 100, ..button }), PXUI_ERR_ARGUMENT, "a size that ends inside a field");
        assert_eq!(press(&PxuiButtonDesc { size: 95, ..button }), PXUI_ERR_ARGUMENT, "below the minimum");
        assert_eq!(press(&PxuiButtonDesc { centered: 2, ..button }), PXUI_ERR_ARGUMENT);
        assert_eq!(press(&PxuiButtonDesc { reserved2: [0, 0, 1, 0, 0, 0, 0], ..button }), PXUI_ERR_ARGUMENT);
        // ABI 1.56: the 104-byte record of 1.54 is read without its tail; the label, layout and focus fields are checked
        assert_eq!(press(&PxuiButtonDesc { size: 104, axis: 9, gap: f32::NAN, has_label: 7, ..button }), 0, "a 104-byte record's tail is not read");
        assert_eq!(press(&PxuiButtonDesc { size: 150, ..button }), PXUI_ERR_ARGUMENT);
        let label = pxui_types::ffi::PxuiLabelStyle::from(&pxui_types::Label::BODY.size(14.0));
        let label_box = pxui_types::ffi::PxuiLabelBox::from(&pxui_types::Label::BODY.h_px(20.0));
        let laid = PxuiButtonDesc { has_label: 1, label, label_box, axis: 2, content_align: 3, gap: 4.0, focus_on_click: 1, ..button };
        assert_eq!(press(&laid), 0);
        for bad in [PxuiButtonDesc { axis: 4, ..laid }, PxuiButtonDesc { content_align: 4, ..laid }, PxuiButtonDesc { gap: -1.0, ..laid }, PxuiButtonDesc { gap: f32::INFINITY, ..laid },
            PxuiButtonDesc { has_label: 2, ..laid }, PxuiButtonDesc { focus_on_click: 2, ..laid }, PxuiButtonDesc { label: pxui_types::ffi::PxuiLabelStyle { size: 8, ..label }, ..laid },
            PxuiButtonDesc { label: pxui_types::ffi::PxuiLabelStyle { size_points: -3.0, ..label }, ..laid }, PxuiButtonDesc { label_box: pxui_types::ffi::PxuiLabelBox { whole_pixels: 2, ..label_box }, ..laid }] {
            assert_eq!(press(&bad), PXUI_ERR_ARGUMENT);
        }
        assert_eq!(press(&PxuiButtonDesc { has_label: 0, label: pxui_types::ffi::PxuiLabelStyle { size: 8, ..label }, ..laid }), 0, "the label is read only when has_label is 1");
        // ABI 1.71: a 192-byte record (1.56) has no leading icon, detail or accessory, whatever its tail holds; the whole record's are checked
        let garbage = Str { ptr: b"\xff".as_ptr(), len: 1 };
        assert_eq!(press(&PxuiButtonDesc { size: 192, has_detail_label: 9, reserved3: [9; 6], detail: garbage, ..laid }), 0, "a 192-byte record's tail is not read");
        assert_eq!(press(&PxuiButtonDesc { size: 200, ..laid }), PXUI_ERR_ARGUMENT);
        let parts = PxuiButtonDesc { lead: k, detail: k, accessory: k, has_detail_label: 1, detail_label: label, detail_box: label_box, has_accessory_label: 1, accessory_label: label, accessory_box: label_box, ..laid };
        assert_eq!(press(&parts), 0, "{}", error(ui));
        for bad in [PxuiButtonDesc { has_detail_label: 2, ..parts }, PxuiButtonDesc { has_accessory_label: 2, ..parts }, PxuiButtonDesc { reserved3: [0, 0, 0, 0, 0, 1], ..parts },
            PxuiButtonDesc { detail: garbage, ..parts }, PxuiButtonDesc { accessory: garbage, ..parts }, PxuiButtonDesc { lead: garbage, ..parts },
            PxuiButtonDesc { detail_label: pxui_types::ffi::PxuiLabelStyle { size_points: -3.0, ..label }, ..parts }, PxuiButtonDesc { accessory_box: pxui_types::ffi::PxuiLabelBox { whole_pixels: 2, ..label_box }, ..parts }] {
            assert_eq!(press(&bad), PXUI_ERR_ARGUMENT);
        }
        assert_eq!(press(&PxuiButtonDesc { has_detail_label: 0, detail_label: pxui_types::ffi::PxuiLabelStyle { size: 8, ..label }, ..parts }), 0, "a detail style is read only when has_detail_label is 1");
        // ABI 1.71: the icon button's on/off state: 0 or 1, checked only when checkable; a 104-byte record is a plain button
        assert_eq!(tap(&PxuiIconButtonDesc { checkable: 1, checked: 1, ..icon }), 0);
        assert_eq!(tap(&PxuiIconButtonDesc { size: 104, checkable: 9, checked: 9, reserved2: [9; 6], ..icon }), 0, "a 104-byte record's tail is not read");
        for bad in [PxuiIconButtonDesc { checkable: 2, ..icon }, PxuiIconButtonDesc { checked: 1, ..icon }, PxuiIconButtonDesc { checkable: 1, checked: 2, ..icon }, PxuiIconButtonDesc { reserved2: [0, 1, 0, 0, 0, 0], ..icon }] {
            assert_eq!(tap(&bad), PXUI_ERR_ARGUMENT);
        }
        assert_eq!(tap(&icon), 0, "null curves are the kit's springs");
        use pxui_types::Curve;
        let springs = [Curve::Spring { omega: 30.0 }, Curve::Spring { omega: 10.0 }];
        assert_eq!(tap(&PxuiIconButtonDesc { press: springs.as_ptr(), ..icon }), 0);
        let bad_tag = { let mut raw = [0u32; 70]; raw[0] = 9; raw };
        assert_eq!(tap(&PxuiIconButtonDesc { size: 96, press: bad_tag.as_ptr().cast(), ..icon }), 0, "a 96-byte record's tail is not read");
        assert_eq!(tap(&PxuiIconButtonDesc { press: bad_tag.as_ptr().cast(), ..icon }), PXUI_ERR_ARGUMENT, "an unknown curve kind is refused before it is read");
        assert_eq!(tap(&PxuiIconButtonDesc { press: springs.as_ptr().cast::<u8>().wrapping_add(1).cast(), ..icon }), PXUI_ERR_ARGUMENT, "a misaligned pointer");
        assert_eq!(tap(&PxuiIconButtonDesc { size: 100, ..icon }), PXUI_ERR_ARGUMENT);
        assert_eq!(tap(&PxuiIconButtonDesc { size: 95, ..icon }), PXUI_ERR_ARGUMENT);
        let still = [Curve::Spring { omega: 30.0 }, Curve::Spring { omega: 0.0 }];
        assert_eq!(tap(&PxuiIconButtonDesc { press: still.as_ptr(), ..icon }), PXUI_ERR_ARGUMENT);
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
        four_min(ui, ((&value as *const PxuiButtonDesc).cast(), size_of_val(&value), align_of_val(&value)), &[96, 104, 192], &mut |p| {
            let p = p.cast::<PxuiButtonDesc>();
            in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_button_response(ui, p, None, core::ptr::null_mut(), &mut out) } })
        });
        four_min(ui, ((&value as *const PxuiButtonDesc).cast(), size_of_val(&value), align_of_val(&value)), &[96, 104, 192], &mut |p| {
            let p = p.cast::<PxuiButtonDesc>();
            in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_button_content(ui, p, None, core::ptr::null_mut(), &mut out) } })
        });
        let mut icon: PxuiIconButtonDesc = unsafe { core::mem::zeroed() };
        (icon.enabled, icon.press_scale, icon.key, icon.name, icon.icon) = (1, 1.0, k, k, k);
        four_min(ui, ((&icon as *const PxuiIconButtonDesc).cast(), size_of_val(&icon), align_of_val(&icon)), &[96, 104], &mut |p| {
            let p = p.cast::<PxuiIconButtonDesc>();
            in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_icon_button_response(ui, p, &mut out) } })
        });
        let short = PxuiResponse { size: 8, ..PxuiResponse::EMPTY };
        for out in [core::ptr::null_mut(), &short as *const PxuiResponse as *mut PxuiResponse] {
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_button_plain(ui, k, k, out) }), PXUI_ERR_ARGUMENT);
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_icon_button_plain(ui, k, k, k, out) }), PXUI_ERR_ARGUMENT);
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_button_response(ui, &value, None, core::ptr::null_mut(), out) }), PXUI_ERR_ARGUMENT);
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_button_content(ui, &value, None, core::ptr::null_mut(), out) }), PXUI_ERR_ARGUMENT);
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
        four_min(ui, ((&value as *const PxuiDialogDesc).cast(), size_of_val(&value), align_of_val(&value)), &[88], &mut |p| {
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
        // ABI 1.71: as many details as buttons (or none); an 88-byte record's details are not read
        let details = [k, Str { ptr: b"\xff".as_ptr(), len: 1 }];
        let mut sink = PxuiDialogResult::EMPTY;
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_dialog_response(ui, &PxuiDialogDesc { details: Slice { ptr: buttons.as_ptr(), len: 2 }, ..value }, None, core::ptr::null_mut(), &mut sink) }), 0, "{}", error(ui));
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_dialog_response(ui, &PxuiDialogDesc { size: 88, details: Slice { ptr: core::ptr::null(), len: 7 }, ..value }, None, core::ptr::null_mut(), &mut sink) }), 0, "{}", error(ui));
        for bad in [Slice { ptr: buttons.as_ptr(), len: 1 }, Slice { ptr: details.as_ptr(), len: 2 }, Slice { ptr: core::ptr::null(), len: 2 }] {
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_dialog_response(ui, &PxuiDialogDesc { details: bad, ..value }, None, core::ptr::null_mut(), &mut sink) }), PXUI_ERR_ARGUMENT);
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

    /// ABI 1.67: `pxui_ui_segmented_result` reads its descriptor as `pxui_ui_segmented_response` does (64, 96 and 104 bytes; the older
    /// sizes always carry a choice), refuses a null ui, descriptor or out, a null or short `PxuiSegmentedResult`, `has_selected` over 1
    /// or a reserved byte set; `has_selected` 0 answers no choice there and is refused by the 1.51 call, which cannot report none.
    #[test]
    fn segmented_result_desc() {
        let ui = create();
        let k = Str { ptr: b"k".as_ptr(), len: 1 };
        let options = [k, k];
        let span = Slice { ptr: options.as_ptr(), len: 2 };
        let mut value: PxuiSegmentedDesc = unsafe { core::mem::zeroed() };
        (value.size, value.enabled, value.key, value.name, value.padding, value.selected) = (size_of_val(&value) as u32, 1, k, k, 8.0, 5);
        four_min(ui, ((&value as *const PxuiSegmentedDesc).cast(), size_of_val(&value), align_of_val(&value)), &[64, 96, 104], &mut |p| {
            let p = p.cast::<PxuiSegmentedDesc>();
            in_frame(ui, &mut |ui| { let (mut out, mut result) = (PxuiResponse::EMPTY, PxuiSegmentedResult::EMPTY); unsafe { pxui_ui_segmented_result(ui, p, span, &mut out, &mut result) } })
        });
        let (mut out, mut result) = (PxuiResponse::EMPTY, PxuiSegmentedResult { has_chosen: 9, has_secondary: 9, reserved: [9; 2], ..PxuiSegmentedResult::EMPTY });
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented_result(ui, &value, span, &mut out, &mut result) }), 0, "{}", error(ui));
        assert_eq!((result.has_chosen, result.has_secondary, result.reserved), (0, 0, [0; 2]), "has_selected 0 is no choice");
        let mut chosen = 7usize;
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented_response(ui, &value, span, &mut out, &mut chosen) }), PXUI_ERR_ARGUMENT, "the 1.51 call cannot report none");
        let old = PxuiSegmentedDesc { size: 96, ..value };
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented_response(ui, &old, span, &mut out, &mut chosen) }), 0);
        assert_eq!(chosen, 1, "a 1.55 record's `selected` is the choice, clamped");
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented_result(ui, &old, span, &mut out, &mut result) }), 0);
        assert_eq!((result.has_chosen, result.chosen), (1, 1));
        // a 1.67 record (104 bytes) has its own has_selected (0 here: none) and never has its `focus_on_click` bytes read
        let v67 = PxuiSegmentedDesc { size: 104, focus_on_click: 9, reserved4: [9; 7], ..value };
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented_result(ui, &v67, span, &mut out, &mut result) }), 0, "{}", error(ui));
        assert_eq!((result.has_chosen, result.chosen), (0, 0), "has_selected 0 at 104 bytes is no choice");
        let fine = PxuiSegmentedDesc { selected: 0, has_selected: 1, focus_on_click: 1, ..value };
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented_result(ui, &fine, span, &mut out, &mut result) }), 0);
        assert_eq!((result.has_chosen, result.chosen), (1, 0));
        for bad in [PxuiSegmentedDesc { has_selected: 2, ..value }, PxuiSegmentedDesc { reserved3: [0, 0, 1, 0, 0, 0, 0], ..value }, PxuiSegmentedDesc { focus_on_click: 2, ..value }, PxuiSegmentedDesc { reserved4: [0, 1, 0, 0, 0, 0, 0], ..value }] {
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented_result(ui, &bad, span, &mut out, &mut result) }), PXUI_ERR_ARGUMENT);
        }
        let short = PxuiSegmentedResult { size: 4, ..PxuiSegmentedResult::EMPTY };
        for bad in [core::ptr::null_mut(), &short as *const PxuiSegmentedResult as *mut PxuiSegmentedResult] {
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented_result(ui, &value, span, &mut out, bad) }), PXUI_ERR_ARGUMENT);
        }
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented_result(ui, core::ptr::null(), span, &mut out, &mut result) }), PXUI_ERR_ARGUMENT);
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented_result(ui, &value, span, core::ptr::null_mut(), &mut result) }), PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { pxui_ui_segmented_result(core::ptr::null_mut(), &value, span, &mut out, &mut result) }, PXUI_ERR_ARGUMENT);
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

    fn keyed() -> PxuiKeyedDesc { PxuiKeyedDesc::new("k", "") }

    #[test]
    fn section_list() {
        let ui = create();
        let mut value: PxuiSectionListDesc = unsafe { core::mem::zeroed() };
        value.row_height = 20.0;
        four(ui, ((&value as *const PxuiSectionListDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiSectionListDesc>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_section_list(ui, &keyed(), p, None, None, core::ptr::null_mut(), &mut { PxuiListResult::EMPTY }) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn selection_list() {
        let ui = create();
        let mut value: PxuiSelectionListDesc = unsafe { core::mem::zeroed() };
        value.row_height = 20.0;
        four(ui, ((&value as *const PxuiSelectionListDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiSelectionListDesc>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_selection_list(ui, &keyed(), p, None, None, core::ptr::null_mut(), &mut { PxuiListResult::EMPTY }) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn list_result() {
        let ui = create();
        let value = PxuiListResult::EMPTY;
        four(ui, ((&value as *const PxuiListResult).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiListResult>();
            let mut d: PxuiSelectionListDesc = unsafe { core::mem::zeroed() };
            (d.size, d.row_height) = (size_of_val(&d) as u32, 20.0);
            in_frame(ui, &mut |ui| unsafe { pxui_ui_selection_list(ui, &keyed(), &d, None, None, core::ptr::null_mut(), p) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn table_head() {
        let ui = create();
        let mut value: PxuiTableHeadDesc = unsafe { core::mem::zeroed() };
        value.h = 20.0;
        four(ui, ((&value as *const PxuiTableHeadDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiTableHeadDesc>();
            in_frame(ui, &mut |ui| { let mut out: PxuiHeadResult = unsafe { core::mem::zeroed() }; out.size = size_of_val(&out) as u32; unsafe { pxui_ui_table_head(ui, &keyed(), p, &mut out) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn table_result() {
        let ui = create();
        let mut value: PxuiTableResult = unsafe { core::mem::zeroed() };
        value.size = size_of_val(&value) as u32;
        four(ui, ((&value as *const PxuiTableResult).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiTableResult>();
            let mut d: PxuiTableDesc = unsafe { core::mem::zeroed() };
            (d.size, d.row_h, d.head_h) = (size_of_val(&d) as u32, 20.0, 20.0);
            in_frame(ui, &mut |ui| unsafe { pxui_ui_table(ui, &keyed(), &d, None, None, core::ptr::null_mut(), p) })
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
        (value.row_h, value.head_h) = (20.0, 20.0);
        four(ui, ((&value as *const PxuiTableDesc).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiTableDesc>();
            in_frame(ui, &mut |ui| { let mut out: PxuiTableResult = unsafe { core::mem::zeroed() }; out.size = size_of_val(&out) as u32; unsafe { pxui_ui_table(ui, &keyed(), p, None, None, core::ptr::null_mut(), &mut out) } })
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
        let mut value: pxui_types::ffi::PxuiTextStyle = unsafe { core::mem::zeroed() };
        let fix: fn(&mut pxui_types::ffi::PxuiTextStyle) = |s| *s = pxui_types::ffi::PxuiTextStyle::default();
        fix(&mut value);
        four(ui, ((&value as *const pxui_types::ffi::PxuiTextStyle).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<pxui_types::ffi::PxuiTextStyle>();
            {
        let mut out = pxui_types::TextMetrics::default();
        unsafe { pxui_text_width(ui, 0, Str::default(), p, 100.0, &mut out) }
    }
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    /// The prepare queue: the style record is read through its size prefix like `text_width`'s; null or bad arguments
    /// (ui, style, a negative or NaN width, bad UTF-8, text over the 16 KiB a request takes) are refused with or without a
    /// worker; a valid request is accepted without one (nothing queued), attached, after detaching, and after the worker is
    /// destroyed while still attached (the Ui shapes inline). One worker serves two Uis.
    #[test]
    fn text_prewarm() {
        let (ui, other) = (create(), create());
        let mut value: pxui_types::ffi::PxuiTextStyle = unsafe { core::mem::zeroed() };
        let fix: fn(&mut pxui_types::ffi::PxuiTextStyle) = |s| *s = pxui_types::ffi::PxuiTextStyle::default();
        fix(&mut value);
        four(ui, ((&value as *const pxui_types::ffi::PxuiTextStyle).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            unsafe { pxui_text_prewarm(ui, Str { ptr: b"Song".as_ptr(), len: 4 }, p.cast::<pxui_types::ffi::PxuiTextStyle>(), 0.0) }
        });
        let (style, title) = (pxui_types::ffi::PxuiTextStyle::default(), Str { ptr: b"Song title".as_ptr(), len: 10 });
        let long = vec![b'a'; pxui_core::text::QUEUE_BYTES + 1];
        unsafe {
            let worker = pxui_text_worker_create();
            assert!(!worker.is_null());
            for attached in [core::ptr::null(), worker.cast_const()] {
                assert_eq!(pxui_ui_set_text_worker(ui, attached), 0);
                assert_eq!(pxui_ui_set_text_worker(other, attached), 0);
                assert_eq!(pxui_text_prewarm(core::ptr::null_mut(), title, &style, 0.0), PXUI_ERR_ARGUMENT);
                assert_eq!(pxui_text_prewarm(ui, title, core::ptr::null(), 0.0), PXUI_ERR_ARGUMENT);
                assert_eq!(pxui_text_prewarm(ui, title, &style, f32::NAN), PXUI_ERR_ARGUMENT);
                assert_eq!(pxui_text_prewarm(ui, title, &style, -1.0), PXUI_ERR_ARGUMENT);
                assert_eq!(pxui_text_prewarm(ui, Str { ptr: b"\xff".as_ptr(), len: 1 }, &style, 0.0), PXUI_ERR_ARGUMENT);
                assert_eq!(pxui_text_prewarm(ui, Str { ptr: long.as_ptr(), len: long.len() }, &style, 0.0), PXUI_ERR_ARGUMENT);
                assert_eq!(pxui_text_prewarm(ui, title, &style, 200.0), 0);
                assert_eq!(pxui_text_prewarm(other, title, &style, 0.0), 0);
            }
            assert_eq!(pxui_ui_set_text_worker(core::ptr::null_mut(), worker), PXUI_ERR_ARGUMENT);
            pxui_text_worker_destroy(worker);
            pxui_text_worker_destroy(core::ptr::null_mut());
            assert_eq!(pxui_text_prewarm(ui, title, &style, 100.0), 0);
            assert_eq!(pxui_ui_set_text_worker(other, core::ptr::null()), 0);
            pxui_ui_destroy(ui);
            pxui_ui_destroy(other);
        }
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
        let (style, mut out) = (pxui_types::ffi::PxuiTextStyle::default(), pxui_types::TextMetrics::default());
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

// ABI 1.60: the version an app was built against, recorded sizes of grown input records and `pxui_supports`.
#[test]
fn the_library_checks_the_version_the_app_was_built_against_once_at_create() {
    assert_eq!(pxui_types::ABI_BUILT, ABI_VERSION, "bump ABI_BUILT with ABI_VERSION");
    let major = ABI_VERSION >> 16;
    let make = |built: u32| { let ui = unsafe { pxui_ui_create(&PxuiUiDesc { abi_built: built, ..PxuiUiDesc::default() }) }; let ok = !ui.is_null(); if ok { unsafe { pxui_ui_destroy(ui); } } ok };
    assert!(make(ABI_VERSION) && make(0) && make(major << 16) && make(ABI_VERSION - 1) && make(major << 16 | 57), "the same, unstated, the first and an older minor");
    // the minimum-library rule: an app built for a newer minor may write or read records grown past this library's, so it is refused
    assert!(!make(ABI_VERSION + 1) && !make(major << 16 | 0xFFFF), "an app built for a newer minor than the library");
    assert!(!make((major + 1) << 16) && !make((major - 1) << 16 | 1) && !make(u32::MAX), "another major");
    // a record from before 1.60 (12 bytes, no abi_built) is read as unstated; no other size short of the whole record is read
    let rec = |size: u32| [size.to_ne_bytes(), 1u32.to_ne_bytes(), 13u32.to_ne_bytes(), 0u32.to_ne_bytes()].concat();
    for size in 0..16u32 {
        let bytes = rec(size);
        let ui = unsafe { pxui_ui_create(bytes.as_ptr().cast()) };
        assert_eq!(!ui.is_null(), size == 12, "a {size}-byte record");
        if !ui.is_null() { unsafe { pxui_ui_destroy(ui); } }
    }
}

#[test]
fn supports_answers_from_the_feature_registry_of_this_build() {
    use pxui_types::feature as f;
    let has = |id: u32| unsafe { pxui_supports(id) };
    assert_eq!((has(0), has(30), has(u32::MAX), has(ABI_VERSION)), (0, 0, 0, 0), "no id 0, none past the registry, versions are not ids");
    assert_eq!((has(f::A11Y_BRIDGE) == 1, has(f::METAL) == 1, has(f::VULKAN) == 1), (cfg!(target_os = "linux"), cfg!(any(target_os = "macos", target_os = "ios")), cfg!(any(windows, target_os = "linux"))));
    for id in [f::EXTENSION_SET, f::BUTTON_CONTENT, f::ICON_BUTTON_CURVES, f::TOGGLE_RESPONSE, f::SEGMENTED_RESPONSE, f::DIALOG_RESPONSE, f::WIDGET_VARIANTS, f::LABEL_NODE, f::MENU_ITEM_IDS, f::FRAME_SURFACE, f::ITEM_TEXT, f::COLOR_TOKENS, f::SWIPE_ACTIONS, f::DISMISS, f::LOOK_TOKENS, f::ICON_WORD, f::TEXT_WORKER, f::TEXT_FIELD_KEYED, f::LABEL_TEXT, f::MENU_MAX_H, f::SEGMENTED_RESULT, f::WIDGETS_KEYED, f::SEGMENTED_STORED_FOCUS, f::IMAGE_CHECKED, f::BUTTON_PARTS, f::VIEW_KEYED] {
        assert_eq!(has(id), 1, "feature {id}");
    }
    // pxui.h numbers the same ids
    let header = include_str!("../../../../include/pxui.h");
    // pxui.h names the version the app is built for
    let built: u32 = header.split("#define PXUI_ABI_BUILT ((1u << 16) | ").nth(1).and_then(|rest| rest.split('u').next()).and_then(|minor| minor.parse().ok()).expect("PXUI_ABI_BUILT in pxui.h");
    assert_eq!(1 << 16 | built, ABI_VERSION, "pxui.h PXUI_ABI_BUILT");
    for (name, id) in [("A11Y_BRIDGE", f::A11Y_BRIDGE), ("METAL", f::METAL), ("VULKAN", f::VULKAN), ("EXTENSION_SET", f::EXTENSION_SET), ("BUTTON_CONTENT", f::BUTTON_CONTENT), ("ICON_BUTTON_CURVES", f::ICON_BUTTON_CURVES),
        ("TOGGLE_RESPONSE", f::TOGGLE_RESPONSE), ("SEGMENTED_RESPONSE", f::SEGMENTED_RESPONSE), ("DIALOG_RESPONSE", f::DIALOG_RESPONSE), ("WIDGET_VARIANTS", f::WIDGET_VARIANTS), ("LABEL_NODE", f::LABEL_NODE),
        ("MENU_ITEM_IDS", f::MENU_ITEM_IDS), ("FRAME_SURFACE", f::FRAME_SURFACE), ("ITEM_TEXT", f::ITEM_TEXT), ("COLOR_TOKENS", f::COLOR_TOKENS), ("SWIPE_ACTIONS", f::SWIPE_ACTIONS), ("DISMISS", f::DISMISS), ("LOOK_TOKENS", f::LOOK_TOKENS), ("ICON_WORD", f::ICON_WORD), ("TEXT_WORKER", f::TEXT_WORKER), ("TEXT_FIELD_KEYED", f::TEXT_FIELD_KEYED), ("LABEL_TEXT", f::LABEL_TEXT), ("MENU_MAX_H", f::MENU_MAX_H), ("SEGMENTED_RESULT", f::SEGMENTED_RESULT), ("WIDGETS_KEYED", f::WIDGETS_KEYED), ("SEGMENTED_STORED_FOCUS", f::SEGMENTED_STORED_FOCUS), ("IMAGE_CHECKED", f::IMAGE_CHECKED), ("BUTTON_PARTS", f::BUTTON_PARTS), ("VIEW_KEYED", f::VIEW_KEYED)] {
        assert!(header.contains(&format!("PXUI_FEATURE_{name} = {id}")), "{name}");
    }
}

// ABI 1.60: the toggle, segmented and icon button records of earlier minors never have their grown fields read, whatever the bytes after them hold.
#[test]
fn grown_fields_are_read_only_when_the_callers_size_covers_them() {
    use pxui_types::ffi::{PxuiResponse, PxuiSegmentedDesc, PxuiToggleDesc, Slice};
    let ui = create();
    let k = Str { ptr: b"k".as_ptr(), len: 1 };
    let build = |f: &mut dyn FnMut(*mut PxuiUi) -> i32| { extern "C" fn b(user: *mut c_void, ui: *mut c_void) { let (f, rc) = unsafe { &mut *user.cast::<(&mut dyn FnMut(*mut PxuiUi) -> i32, i32)>() }; *rc = f(ui.cast()); }
        let mut st = (f, i32::MIN); assert_eq!(run(ui, Some(b), (&mut st as *mut (&mut dyn FnMut(*mut PxuiUi) -> i32, i32)).cast()), 0); st.1 };
    let mut toggle: PxuiToggleDesc = unsafe { core::mem::zeroed() };
    (toggle.size, toggle.enabled, toggle.key, toggle.text) = (size_of::<PxuiToggleDesc>() as u32, 1, k, k);
    let mut response = PxuiResponse::EMPTY;
    // an 88-byte record: a title style the caller never had is not read, even when has_style and a bad reserved byte are in memory after it
    assert_eq!(build(&mut |ui| unsafe { pxui_ui_toggle_response(ui, &PxuiToggleDesc { size: 88, has_style: 7, reserved2: [9; 7], ..toggle }, &mut response) }), 0);
    assert_eq!(build(&mut |ui| unsafe { pxui_ui_toggle_response(ui, &PxuiToggleDesc { has_style: 7, ..toggle }, &mut response) }), PXUI_ERR_ARGUMENT);
    let mut seg: PxuiSegmentedDesc = unsafe { core::mem::zeroed() };
    (seg.size, seg.enabled, seg.key, seg.name, seg.has_selected) = (size_of::<PxuiSegmentedDesc>() as u32, 1, k, k, 1);
    let options = [k, k];
    let mut chosen = 0usize;
    let names = [k];
    let bad = Slice { ptr: names.as_ptr(), len: 1 }; // one name for two options: invalid only when read
    assert_eq!(build(&mut |ui| unsafe { pxui_ui_segmented_response(ui, &PxuiSegmentedDesc { size: 64, names: bad, reserved2: 5, ..seg }, Slice { ptr: options.as_ptr(), len: 2 }, &mut response, &mut chosen) }), 0);
    // a 96-byte (1.55) record: the 1.67 `has_selected` and reserved bytes after it are never read
    assert_eq!(build(&mut |ui| unsafe { pxui_ui_segmented_response(ui, &PxuiSegmentedDesc { size: 96, has_selected: 7, reserved3: [9; 7], ..seg }, Slice { ptr: options.as_ptr(), len: 2 }, &mut response, &mut chosen) }), 0);
    assert_eq!(build(&mut |ui| unsafe { pxui_ui_segmented_response(ui, &PxuiSegmentedDesc { names: bad, ..seg }, Slice { ptr: options.as_ptr(), len: 2 }, &mut response, &mut chosen) }), PXUI_ERR_ARGUMENT);
    unsafe { pxui_ui_destroy(ui); }
}

#[test]
fn fonts_and_renderer_records_are_validated() {
    let ui = create();
    let font = |bytes: &[u8]| Str { ptr: bytes.as_ptr(), len: bytes.len() };
    unsafe {
        assert_eq!(pxui_text_add_font(ui, font(b"not a font")), u32::MAX);
        assert_eq!(pxui_text_add_font(ui, Str { ptr: core::ptr::null(), len: 0 }), u32::MAX);
        assert_eq!(pxui_text_add_font(ui, Str { ptr: b"x".as_ptr(), len: pxui_text::MAX_FONT_BYTES + 1 }), u32::MAX);
        assert_ne!(pxui_text_add_font(ui, font(pxui_text::PRAGMASEVKA)), u32::MAX);
        let garbage = [font(b"not a font")];
        assert!(pxui_ui_create_with_fonts(&PxuiUiDesc::default(), garbage.as_ptr(), 1).is_null());
        let two = [font(pxui_text::PRAGMASEVKA), font(pxui_text::PRAGMASEVKA)];
        let made = pxui_ui_create_with_fonts(&PxuiUiDesc::default(), two.as_ptr(), 2);
        assert!(!made.is_null());
        assert_eq!(pxui_ui_destroy(made), 0);
        // a gpu or target record shorter than its type is refused before it is read
        let short = 4u32;
        assert!(pxui_renderer_create(ui, (&short as *const u32).cast(), &PxuiTargetDesc::default()).is_null());
        assert!(pxui_renderer_create(ui, &pxui_types::Gpu::default(), (&short as *const u32).cast()).is_null());
        assert_eq!(pxui_ui_destroy(ui), 0);
    }
}

#[test]
fn every_text_entry_refuses_text_over_the_cap() {
    let ui = create();
    let over = Str { ptr: b"x".as_ptr(), len: crate::state::MAX_TEXT_BYTES + 1 };
    let style = pxui_types::ffi::PxuiTextStyle::default();
    let mut out = pxui_types::TextMetrics::default();
    unsafe {
        let font = pxui_default_font();
        let shaper = pxui_shaper_create(&font, 1);
        assert!(!shaper.is_null());
        assert_eq!(pxui_shaper_width(shaper, 0, over, &style, 100.0, &mut out), PXUI_ERR_ARGUMENT);
        assert_eq!(pxui_text_width(ui, 0, over, &style, 100.0, &mut out), PXUI_ERR_ARGUMENT);
        let spans = pxui_types::TextSpans::default();
        assert_eq!(pxui_shaper_width_spans(shaper, 0, over, &style, 100.0, &spans, &mut out), PXUI_ERR_ARGUMENT);
        assert_eq!(pxui_text_width_spans(ui, 0, over, &style, 100.0, &spans, &mut out), PXUI_ERR_ARGUMENT);
        assert_eq!(pxui_text_prewarm(ui, over, &style, 100.0), PXUI_ERR_ARGUMENT);
        let (mut w, mut h) = (0u32, 0u32);
        assert_eq!(pxui_shaper_rasterize_rgba(shaper, 0, over, &style, 100.0, 1.0, 0, 0, 0, 255, &mut w, &mut h, core::ptr::null_mut(), 0), usize::MAX);
        assert_eq!(pxui_text_rasterize_rgba(ui, 0, over, &style, 100.0, 1.0, 0, 0, 0, 255, &mut w, &mut h, core::ptr::null_mut(), 0), usize::MAX);
        assert_eq!(pxui_text_quads(ui, 0, over, &style, 100.0, 0.0, 0.0, 1.0, core::ptr::null_mut(), 0), usize::MAX);
        pxui_shaper_destroy(shaper);
        assert_eq!(pxui_ui_destroy(ui), 0);
    }
}

// ABI 1.65: a text call on a label style measures what the resolved style measures, in one call; a null, invalid or unresolvable
// label record, a null Ui or a null output is refused, never read.
#[test]
fn label_text_calls_resolve_in_the_same_call_and_refuse_bad_records() {
    use pxui_types::{ffi::{PxuiLabelStyle, PxuiTextStyle}, Label, TextMetrics};
    let ui = create();
    let value = Str { ptr: b"Song title".as_ptr(), len: 10 };
    let label = PxuiLabelStyle::from(&Label::BODY.size(15.0).bold());
    let mut resolved = PxuiTextStyle::default();
    assert_eq!(unsafe { crate::ui::pxui_ui_resolve_label(ui, &label, &mut resolved) }, 0);
    let (mut a, mut b) = (TextMetrics::default(), TextMetrics::default());
    assert_eq!(unsafe { pxui_ui_label_measure(ui, value, &label, 0.0, core::ptr::null(), &mut a) }, 0);
    assert_eq!(unsafe { pxui_text_width(ui, 0, value, &resolved, 0.0, &mut b) }, 0);
    assert!(a == b && a.width > 0.0, "{a:?} {b:?}");
    let bad = [PxuiLabelStyle { wrap: 9, ..label }, PxuiLabelStyle { size: 8, ..label }, PxuiLabelStyle { set: PxuiLabelStyle::SET_SIZE, size_points: -1.0, ..label }];
    for bad in &bad { assert_eq!(unsafe { pxui_ui_label_measure(ui, value, bad, 0.0, core::ptr::null(), &mut a) }, PXUI_ERR_ARGUMENT, "{bad:?}"); }
    unsafe {
        assert_eq!(pxui_ui_label_measure(ui, value, core::ptr::null(), 0.0, core::ptr::null(), &mut a), PXUI_ERR_ARGUMENT);
        assert_eq!(pxui_ui_label_measure(core::ptr::null_mut(), value, &label, 0.0, core::ptr::null(), &mut a), PXUI_ERR_ARGUMENT);
        assert_eq!(pxui_ui_label_measure(ui, value, &label, 0.0, core::ptr::null(), core::ptr::null_mut()), PXUI_ERR_ARGUMENT);
        assert_eq!(pxui_ui_label_quads(ui, value, core::ptr::null(), 0.0, 0.0, 0.0, 1.0, core::ptr::null_mut(), 0), usize::MAX);
        assert_eq!(pxui_ui_label_rasterize_rgba(ui, value, &bad[0], 0.0, 1.0, 0, 0, 0, 255, core::ptr::null_mut(), core::ptr::null_mut(), core::ptr::null_mut(), 0), usize::MAX);
        assert_eq!(pxui_ui_label_prewarm(ui, value, core::ptr::null(), 0.0), PXUI_ERR_ARGUMENT);
        assert_eq!(pxui_ui_label_text_spans(ui, core::ptr::null(), value, core::ptr::null(), core::ptr::null()), PXUI_ERR_ARGUMENT);
        assert_eq!(pxui_ui_label_text_line_box(core::ptr::null_mut(), core::ptr::null(), value, &label, core::ptr::null()), PXUI_ERR_ARGUMENT);
        pxui_ui_destroy(ui);
    }
}
