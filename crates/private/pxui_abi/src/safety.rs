//! The export macro's panic containment and the guards that keep a callback from re-entering the library.
use crate::state::{PxuiUi, panicked};
use crate::*;
use core::ffi::c_void;

fn create() -> *mut PxuiUi { unsafe { crate::ui::create_test_ui() } }
fn frame_in() -> FrameIn { FrameIn::new(320.0, 240.0) }
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

#[test]
fn damage_list_and_legacy_bounding_box_agree() {
    use pxui_types::ffi::PxuiDamageList;
    let ui = create();
    let mut legacy = FrameOut::default();
    let mut damage = PxuiDamageList { count: 77, ..PxuiDamageList::default() };
    assert_eq!(unsafe { pxui_ui_damage_list(core::ptr::null_mut(), &mut damage) }, PXUI_ERR_ARGUMENT);
    assert_eq!(damage.count, 77);
    assert_eq!(unsafe { pxui_ui_damage_list(ui, core::ptr::null_mut()) }, PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe { pxui_frame(ui, &frame_in(), None, core::ptr::null_mut(), &mut legacy) }, 0);
    assert_eq!(unsafe { pxui_ui_damage_list(ui, &mut damage) }, 0);
    assert_eq!((damage.count, damage.full, damage.reserved), (1, 1, [0; 6]));
    assert_eq!((legacy.has_damage, legacy.damage), (1, damage.rects[0]));
    assert_eq!(unsafe { (*pxui_ui_output(ui)).damage_list }, damage);
    assert_eq!(unsafe { pxui_ui_layer_drawn(ui, 0) }, 0);
    assert_eq!(unsafe { pxui_ui_damage_list(ui, &mut damage) }, 0);
    assert_eq!(damage, PxuiDamageList::default());
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

#[test]
fn promotion_outputs_are_only_borrowed_between_frames() {
    let ui = create();
    let mut records = core::ptr::null();
    let mut len = 77;
    assert_eq!(unsafe { pxui_ui_set_promotion(ui, 2) }, PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe { pxui_ui_set_promotion(ui, 1) }, 0);
    assert_eq!(unsafe { pxui_ui_promoted(ui, core::ptr::null_mut(), &mut len) }, PXUI_ERR_ARGUMENT);
    assert_eq!(in_frame(ui, &mut |handle| {
        assert_eq!(unsafe { pxui_ui_set_promotion(handle, 0) }, PXUI_ERR_STATE);
        unsafe { pxui_ui_promoted(handle, &mut records, &mut len) }
    }), PXUI_ERR_STATE);
    assert!(records.is_null());
    assert_eq!(len, 77);
    assert_eq!(unsafe { pxui_ui_promoted(ui, &mut records, &mut len) }, 0);
    assert_eq!(len, 0);
    assert_eq!(unsafe { pxui_renderer_exclude_promoted(core::ptr::null_mut(), 1) }, PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

#[test]
fn diagnostics_respect_callback_renderer_and_poison_ownership_without_writing_outputs() {
    use pxui_types::ffi::PxuiDiagnostics;
    let ui = create();
    let mut d = PxuiDiagnostics { calls: 77, timing: 1, ..PxuiDiagnostics::new() };
    assert_eq!(in_frame(ui, &mut |h| {
        assert_eq!(unsafe { pxui_ui_diagnostics(ui, &mut d) }, PXUI_ERR_STATE);
        assert_eq!(d.calls, 77);
        assert_eq!(unsafe { pxui_ui_diagnostics(h, &mut PxuiDiagnostics::new()) }, 0);
        let fence = crate::state::NoCurrent::set(unsafe { (*h).shared });
        assert_eq!(unsafe { pxui_ui_diagnostics(h, &mut d) }, PXUI_ERR_STATE);
        assert_eq!(d.calls, 77);
        drop(fence);
        0
    }), 0);
    unsafe { panicked((*ui).shared, Box::new("diagnostics test"), false); }
    assert_eq!(unsafe { pxui_ui_diagnostics(ui, &mut d) }, PXUI_ERR_PANIC);
    assert_eq!(d.calls, 77);
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}
extern "C" fn nothing(_: *mut c_void, _: *mut c_void) {}

#[test]
fn refused_variant_builders_preserve_the_current_variant() {
    use pxui_types::{ffi::PxuiKeyedDesc, theme::BodyKind};
    let ui = create();
    let kind = BodyKind::progress as u32;
    let keyed = PxuiKeyedDesc { variant: 7, ..PxuiKeyedDesc::new("progress", "Progress") };
    let mut out = PxuiBox::default();
    let engine = unsafe { (*ui).engine };
    unsafe { (*engine).set_variant(kind, 9); }
    assert_eq!(unsafe { pxui_ui_progress(ui, &keyed, 0, 0.0, &mut out) }, PXUI_ERR_STATE);
    assert_eq!(unsafe { (*engine).variant_of(kind) }, 9);
    assert_eq!(in_frame(ui, &mut |h| {
        unsafe { (*engine).set_variant(kind, 9); }
        assert_eq!(unsafe { pxui_ui_progress(ui, &keyed, 0, 0.0, &mut out) }, PXUI_ERR_STATE);
        assert_eq!(unsafe { (*engine).variant_of(kind) }, 9);
        let fence = crate::state::NoCurrent::set(unsafe { (*h).shared });
        assert_eq!(unsafe { pxui_ui_progress(h, &keyed, 0, 0.0, &mut out) }, PXUI_ERR_STATE);
        assert_eq!(unsafe { (*engine).variant_of(kind) }, 9);
        drop(fence);
        0
    }), 0);
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}
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
    let x = Str::from("x");
    assert_eq!(unsafe { pxui_ui_button_plain(ui, x, x, &mut { pxui_types::ffi::PxuiResponse::EMPTY }) }, PXUI_ERR_STATE);
    assert!(!unsafe { (*(*ui).shared).poisoned });
    assert_eq!(unsafe { pxui_ui_set_dir(ui, 0) }, 0);
    let mut built = 0u32;
    assert_eq!(run(ui, Some(count), (&mut built as *mut u32).cast()), 0);
    assert_eq!(built, 1);
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

#[test]
fn frame_reasons_checks_output_and_copies_a_fresh_snapshot() {
    let ui = create();
    let mut out = pxui_types::ffi::PxuiFrameReasons { stepped: 17, ..Default::default() };
    assert_eq!(unsafe { pxui_ui_frame_reasons(core::ptr::null_mut(), &mut out) }, PXUI_ERR_ARGUMENT);
    assert_eq!(out.stepped, 17);
    assert_eq!(unsafe { pxui_ui_frame_reasons(ui, core::ptr::null_mut()) }, PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe { pxui_ui_frame_reasons(ui, &mut out) }, 0);
    assert_eq!(out.stepped, 0);
    assert_eq!(out.transition_curve.kind, 0);
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
    assert_eq!(unsafe { pxui_ui_step(ui, 1.0, &mut 0) }, PXUI_ERR_PANIC);
    let x = Str::from("x");
    assert_eq!(unsafe { pxui_ui_button_plain(ui, x, x, &mut { pxui_types::ffi::PxuiResponse::EMPTY }) }, PXUI_ERR_PANIC);
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
        unsafe {
            for ui in [borrowed, owner] {
                assert_eq!(pxui_frame(ui, &frame_in(), Some(nothing), core::ptr::null_mut(), &mut out), PXUI_ERR_STATE);
                assert_eq!(pxui_ui_use_theme(ui, core::ptr::null()), PXUI_ERR_STATE);
                assert_eq!(pxui_ui_destroy(ui), PXUI_ERR_STATE);
                assert!(!pxui_ui_output(ui).is_null());
            }
            let mut renderer = core::ptr::null_mut(); assert_ne!(pxui_renderer_create(borrowed, &pxui_types::Gpu::default(), &PxuiTargetDesc::default(), &mut renderer), 0); assert!(renderer.is_null());
        }
    }
    let ui = create();
    assert_eq!(run(ui, Some(reenter), ui.cast()), 0, "{}", error(ui));
    // after the frame the owner works again
    assert_eq!(run(ui, Some(nothing), core::ptr::null_mut()), 0);
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
        assert_eq!(pxui_renderer_create(ui, (&short as *const u32).cast(), &PxuiTargetDesc::default(), &mut renderer), PXUI_ERR_ARGUMENT);
        assert_eq!(renderer, core::ptr::dangling_mut::<PxuiRenderer>());
        assert_eq!(pxui_renderer_create(ui, &pxui_types::Gpu::default(), &PxuiTargetDesc::default(), core::ptr::null_mut()), PXUI_ERR_ARGUMENT);
        assert_eq!(pxui_body_register(ui, kind, name, Some(body), core::ptr::null_mut(), core::ptr::null_mut()), PXUI_ERR_ARGUMENT);
        assert_eq!(pxui_body_register(ui, u32::MAX, name, Some(body), core::ptr::null_mut(), &mut id), PXUI_ERR_ARGUMENT);
        assert_eq!(id, 77);
        assert_eq!(pxui_body_register(ui, kind, name, Some(body), core::ptr::null_mut(), &mut id), 0);
        assert_eq!(id, 1);
        assert_eq!(pxui_ui_destroy(ui), 0);
    }
}

#[test]
fn accessibility_setters_on_a_recycled_slot_report_it_stale() {
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
        assert_eq!(pxui_ui_set_role(ui, &retired, pxui_types::a11y::Role::Button as u32, 0), 0);
        assert_eq!(pxui_ui_a11y_name(ui, &retired, name), PXUI_ERR_STALE);
        assert_eq!(pxui_ui_a11y_number(ui, &retired, 0.0, 0.0, 1.0, 0.0), PXUI_ERR_STALE);
        assert_eq!(pxui_ui_a11y_value(ui, &retired, name, 0, 0), PXUI_ERR_STALE);
        assert_eq!(pxui_ui_destroy(ui), 0);
    }
}

#[test]
fn default_descriptors_carry_their_size() {
    use pxui_types::{Commands, Gpu, Pass, ffi::Input};
    assert_eq!(FrameIn::default().size as usize, core::mem::size_of::<FrameIn>());
    assert_eq!(Input::default().size as usize, core::mem::size_of::<Input>());
    assert_eq!(Gpu::default().size as usize, core::mem::size_of::<Gpu>());
    assert_eq!(Pass::default().size as usize, core::mem::size_of::<Pass>());
    assert_eq!(Commands::default().size as usize, core::mem::size_of::<Commands>());
    // and the library accepts the defaults it is given
    let ui = create();
    let mut out = FrameOut::default();
    assert_eq!(unsafe { pxui_frame(ui, &FrameIn::new(320.0, 240.0), Some(nothing), core::ptr::null_mut(), &mut out) }, 0);
    unsafe { pxui_ui_destroy(ui); }
}

#[test]
fn text_with_nul_crosses_as_ordinary_text() {
    extern "C" fn build(user: *mut c_void, ui: *mut c_void) {
        let ui = ui.cast::<PxuiUi>();
        let query = { let mut record = Str::default(); record.ptr = b"a\0b".as_ptr(); record.len = 3; record };
        let (mut returned, mut tag, mut index) = (Str::default(), 0u32, 0i64);
        let rc = unsafe { pxui_ui_picker(ui, &pxui_types::ffi::PxuiKeyedDesc::new("pick", ""), 5, 0, 0, query, None, None, None, None, core::ptr::null_mut(), &mut returned, &mut tag, &mut index) };
        assert_eq!(rc, 0, "{}", error(ui));
        assert_eq!(unsafe { core::slice::from_raw_parts(returned.ptr, returned.len) }, b"a\0b");
        unsafe { *user.cast::<u32>() += 1; }
    }
    let ui = create();
    let mut built = 0u32;
    assert_eq!(run(ui, Some(build), (&mut built as *mut u32).cast()), 0);
    assert_eq!(built, 2, "the first frame settles the picker's new results list once (U2b.4.15), the query echoed both times");
    unsafe { pxui_ui_destroy(ui); }
}

// ABI 1.86: picker fills hand back `PxuiPickRow`s (1.117: art kind 3, an app icon's 32-bit word). The first row's size is read before the span is taken as rows, so a caller built
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
        unsafe { pxui_ui_picker(ui, &keyed, 5, 0, 1, Str::from("s"), Some(rows), None, None, None, (fill as *mut Fill).cast(), &mut query, &mut tag, &mut index) }
    });
    let row = PxuiPickRow { art_kind: 1, art: 0x266B, heading: Str::from("S"), ..PxuiPickRow::new("Song", "Artist") };
    let good = [row, PxuiPickRow { art_kind: 2, art: 7, ..PxuiPickRow::new("Album", "") }];
    let mut fill: Fill = (good.as_ptr().cast(), 2, 0);
    assert_eq!(pick(&mut fill), 0, "{}", error(ui));
    // ABI 1.117 (U2b.4.19): an app icon's word
    let drawn = [row, PxuiPickRow { art_kind: 3, art: u64::from(u32::MAX), ..PxuiPickRow::new("Folder", "") }];
    let mut fill: Fill = (drawn.as_ptr().cast(), 2, 0);
    assert_eq!(pick(&mut fill), 0, "{}", error(ui));
    assert!(fill.2 > 0, "the fill ran");
    // a pre-1.86 fill: two `PxuiStr`s (one label and detail pair) whose pointers would fault if read
    let poison = [{ let mut record = Str::default(); record.ptr = 0xdead_0001usize as *const u8; record.len = 4; record }, { let mut record = Str::default(); record.ptr = 0xdead_1001usize as *const u8; record.len = 6; record }];
    let refused = |ptr: *const c_void, len: usize, why: &str| {
        let mut fill: Fill = (ptr, len, 0);
        assert_ne!(pick(&mut fill), 0, "{why}");
        assert!(fill.2 > 0, "{why}: the fill ran");
    };
    refused(poison.as_ptr().cast(), 2, "a pre-1.86 PxuiStr fill");
    let bad = |r: PxuiPickRow| [good[0], r];
    refused(bad(PxuiPickRow { size: 48, ..good[1] }).as_ptr().cast(), 2, "a later row of another size");
    refused(bad(PxuiPickRow { art_kind: 4, ..good[1] }).as_ptr().cast(), 2, "an unknown art kind");
    refused(bad(PxuiPickRow { art_kind: 3, art: 1 << 32, ..good[1] }).as_ptr().cast(), 2, "an icon word past 32 bits");
    refused(bad(PxuiPickRow { art_kind: 1, art: 0xD800, ..good[1] }).as_ptr().cast(), 2, "a glyph that is not a Unicode scalar value");
    refused(bad(PxuiPickRow { art_kind: 0, art: 5, ..good[1] }).as_ptr().cast(), 2, "art with no kind");
    refused(bad(PxuiPickRow { reserved: [0, 1, 0], ..good[1] }).as_ptr().cast(), 2, "a non-zero reserved byte");
    refused(bad(PxuiPickRow { heading: { let mut record = Str::default(); record.ptr = b"\xff".as_ptr(); record.len = 1; record }, ..good[1] }).as_ptr().cast(), 2, "a heading that is not UTF-8");
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
        pxui_ui_modal(ui, &keyed, desc, Some(ran), (built as *mut u32).cast(), open, root)
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
    assert_ne!(in_frame(ui, &mut |ui| unsafe { pxui_ui_modal(ui, &keyed, core::ptr::null(), Some(ran), core::ptr::null_mut(), &mut 0, &mut root) }), 0, "a null sheet");
    assert_ne!(in_frame(ui, &mut |ui| unsafe { pxui_ui_modal(ui, &keyed, &good, Some(ran), core::ptr::null_mut(), core::ptr::null_mut(), &mut root) }), 0, "a null output");
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

/// A refused call writes none of its outputs (pxui.h: "a refused call writes neither"), whether it is refused outside a frame
/// (`PXUI_ERR_STATE`) or for a bad argument inside one: each output keeps the caller's sentinel.
#[test]
fn refused_calls_leave_their_outputs_untouched() {
    use pxui_types::ffi::{PxuiInspectorSectionDesc, PxuiKeyedDesc, PxuiResponse, PxuiSheetDesc, PxuiSplitDesc, PxuiTextAreaDesc};
    let ui = create();
    let s = |t: &'static str| { let mut record = Str::default(); record.ptr = t.as_ptr(); record.len = t.len(); record };
    let bad_utf8 = { let mut record = Str::default(); record.ptr = [0xff_u8].as_ptr(); record.len = 1; record };
    let sentinel = Str::from("sentinel");
    let marked = || { let mut r = PxuiResponse::EMPTY; r.flags = 0xdead; r };
    let keyed = PxuiKeyedDesc::new("k", "K");
    // the search field: outside a frame, and inside one with a label that is not UTF-8
    let search = |ui: *mut PxuiUi, label: Str| {
        let (mut changed, mut out, mut r) = (7u8, sentinel, marked());
        let rc = unsafe { pxui_ui_search_field(ui, label, s(""), s("q"), s(""), s("Find"), 2, 0, &mut changed, &mut out, &mut r) };
        assert_eq!((changed, out.len, r.flags), (7, 8, 0xdead), "search field rc {rc}");
        rc
    };
    assert_eq!(search(ui, s("find")), PXUI_ERR_STATE);
    assert_eq!(in_frame(ui, &mut |ui| search(ui, bad_utf8)), PXUI_ERR_ARGUMENT);
    // the split: outside a frame, and inside one with a ratio that is not finite (refused by the trusted entry)
    let split = |ui: *mut PxuiUi, ratio: f32| {
        let d = PxuiSplitDesc { size: core::mem::size_of::<PxuiSplitDesc>() as u32, axis: 0, label: s("split"), ratio, min_a: 0.0, min_b: 0.0, reserved: 0,
            build_a: Some(nothing), user_a: core::ptr::null_mut(), build_b: Some(nothing), user_b: core::ptr::null_mut() };
        let (mut r, mut has, mut at) = (marked(), 7u8, 9.0f32);
        let rc = unsafe { pxui_ui_split(ui, &d, &mut r, &mut has, &mut at) };
        assert_eq!((r.flags, has, at), (0xdead, 7, 9.0), "split rc {rc}");
        rc
    };
    assert_eq!(split(ui, 0.5), PXUI_ERR_STATE);
    assert_eq!(in_frame(ui, &mut |ui| split(ui, f32::NAN)), PXUI_ERR_ARGUMENT);
    // the modal: outside a frame
    let sheet = PxuiSheetDesc { size: core::mem::size_of::<PxuiSheetDesc>() as u32, has_width: 0, present: 0, reserved: [0; 2], width: 0.0, dismiss_fraction: 0.0, dismiss_velocity: 0.0, top: 0.0, max_h: 0.0 };
    let (mut open, mut root) = (7u8, PxuiBox { box_key: 99, ..PxuiBox::default() });
    assert_eq!(unsafe { pxui_ui_modal(ui, &keyed, &sheet, Some(nothing), core::ptr::null_mut(), &mut open, &mut root) }, PXUI_ERR_STATE);
    assert_eq!((open, root.box_key), (7, 99), "modal");
    // the text area: inside a frame with a label that is not UTF-8
    let mut area: PxuiTextAreaDesc = unsafe { core::mem::zeroed() };
    (area.size, area.label) = (core::mem::size_of::<PxuiTextAreaDesc>() as u32, bad_utf8);
    let (mut text, mut changed, mut submitted, mut r) = (sentinel, 7u8, 7u8, marked());
    assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_text_area(ui, &area, &pxui_types::Node::DEFAULT, core::ptr::null(), s("Notes"), &mut text, &mut changed, &mut submitted, &mut r) }), PXUI_ERR_ARGUMENT);
    assert_eq!((text.len, changed, submitted, r.flags), (8, 7, 7, 0xdead), "text area");
    // the inspector's trusted builders: outside a frame
    let mut on = 7u8;
    assert_eq!(unsafe { pxui_ui_inspector_toggle(ui, s("t"), s("T"), 1, &mut on) }, PXUI_ERR_STATE);
    assert_eq!(unsafe { pxui_ui_inspector_button(ui, s("b"), s("B"), &mut on) }, PXUI_ERR_STATE);
    assert_eq!(on, 7, "inspector toggle and button");
    let mut section = PxuiInspectorSectionDesc { size: core::mem::size_of::<PxuiInspectorSectionDesc>() as u32, expanded: 1, has_set: 0, set_expanded: 0, reserved: 0,
        key: s("s"), label: s("S"), build: Some(nothing), user: core::ptr::null_mut(), out_open: 7 };
    assert_eq!(unsafe { pxui_ui_inspector_section(ui, &mut section) }, PXUI_ERR_STATE);
    assert_eq!(section.out_open, 7, "inspector section");
    assert!(!unsafe { (*(*ui).shared).poisoned });
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
        ThemeTokens { reserved: [1, 0, 0], ..good }, ThemeTokens { size: 4, ..good }, ThemeTokens { dark: 2, ..good }, ThemeTokens { reserved_tail: [0, 1], ..good },
    ];
    for tokens in bad { assert!(unsafe { crate::theme::pxui_theme_create(&tokens) }.is_null()); }
}

/// Calls every kind of export on `ui` and returns the statuses: a default-form one, a trusted-form one (C entry), the trusted entry the
/// static wrapper calls, a bare one that touches the engine and a container.
fn probe(ui: *mut PxuiUi) -> [i32; 6] {
    let (label, mut out) = (Str::from("x"), PxuiBox::default());
    unsafe {
        [pxui_ui_set_dir(ui, 0), pxui_ui_toggle_plain(ui, label, label, 0, &mut { pxui_types::ffi::PxuiResponse::EMPTY }), pxui_ui_button_plain(ui, label, label, &mut { pxui_types::ffi::PxuiResponse::EMPTY }),
         crate::trusted::pxui_ui_node_keyed(ui, 7, &pxui_types::Node::DEFAULT, &mut out),
         pxui_ui_draw(ui, &out, 0, None, core::ptr::null_mut(), None), pxui_ui_scope(ui, label, Some(nothing), core::ptr::null_mut())]
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
        assert_eq!(unsafe { pxui_ui_scope(ui.cast(), Str::from("s"), Some(inner), handles.as_mut_ptr().cast()) }, 0);
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
    let text = |s: &str| { let mut record = Str::default(); record.ptr = s.as_ptr(); record.len = s.len(); record };
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
        let desc=pxui_types::Node::DEFAULT.flags(pxui_types::Flags::FOCUSABLE).role(pxui_types::a11y::Role::Slider).w(pxui_types::Size::Px(200.0)).h(pxui_types::Size::Px(20.0));
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
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,pxui_types::ffi::Strided::of(&batch))},PXUI_ERR_ARGUMENT);
    assert_eq!(run(ui,Some(slider),(&mut node as *mut PxuiBox).cast()),0);
    let value=unsafe {(&*(*ui).engine).signal(&crate::state::core_box_of(ui, node).unwrap()).a11y_value};
    assert_eq!(value,None,"invalid tail must not deliver earlier chunks");
    batch[64].action_code=4; batch[64].payload=77.0;
    batch[64].surface=9;
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,pxui_types::ffi::Strided::of(&batch))},PXUI_ERR_ARGUMENT);
    assert_eq!(run(ui,Some(slider),(&mut node as *mut PxuiBox).cast()),0);
    assert_eq!(unsafe {(&*(*ui).engine).signal(&crate::state::core_box_of(ui, node).unwrap()).a11y_value},None,"a foreign surface in the tail refuses every chunk");
    batch[64].surface=FrameIn::MAIN;
    for payload in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        batch[64].payload=payload;
        assert_eq!(unsafe {pxui_ui_inject_a11y(ui,pxui_types::ffi::Strided::of(&batch))},PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe {(&*(*ui).engine).pending_a11y_actions()},0,"invalid payload refuses every chunk");
    }
    batch[64].payload=77.0;
    batch[64].reserved=1;
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,pxui_types::ffi::Strided::of(&batch))},PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe {(&*(*ui).engine).pending_a11y_actions()},0);
    batch[64].reserved=0;
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,pxui_types::ffi::Strided::of(&batch))},0);
    assert_eq!(run(ui,Some(slider),(&mut node as *mut PxuiBox).cast()),0);
    let value=unsafe {(&*(*ui).engine).signal(&crate::state::core_box_of(ui, node).unwrap()).a11y_value};
    assert_eq!(value,Some(77.0));
    batch[0].action_code=8; batch[0].payload=f64::MAX;
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,pxui_types::ffi::Strided::of(&[batch[0];2]))},0);
    assert_eq!(in_frame(ui,&mut |h| {
        let b=unsafe {crate::state::core_box_of(h,node)}.unwrap();
        assert!(unsafe {(&*(*h).engine).scroll_offset(&b)}.is_finite(),"finite deltas cannot overflow point storage before layout");
        slider((&mut node as *mut PxuiBox).cast(),h.cast());
        0
    }),0);
    batch[0].action_code=4;
    batch[0].payload=123.0;
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,pxui_types::ffi::Strided::of(&batch[..1]))},0);
    assert_eq!(unsafe {pxui_ui_remove_surface(ui,FrameIn::MAIN)},0);
    assert_eq!(unsafe {pxui_ui_add_surface(ui,9)},0);
    assert_eq!(unsafe {pxui_frame(ui,&{ let mut record = frame_in(); record.surface = 9; record },Some(slider),(&mut node as *mut PxuiBox).cast(),&mut FrameOut::default())},0);
    assert_eq!(unsafe {(&*(*ui).engine).signal(&crate::state::core_box_of(ui, node).unwrap()).a11y_value},None,"removing a surface discards its pending actions");
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,pxui_types::ffi::Strided{ptr:core::ptr::null(),len:0,stride:40})},0);
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,pxui_types::ffi::Strided{ptr:core::ptr::null(),len:1,stride:40})},PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe {pxui_ui_inject_a11y(ui,pxui_types::ffi::Strided{ptr:batch.as_ptr(),len:4097,stride:40})},PXUI_ERR_ARGUMENT);
    unsafe {pxui_ui_destroy(ui);}
}

#[test]
fn accessibility_actions_route_identical_node_ids_to_their_own_surface() {
    extern "C" fn build(_: *mut c_void, raw: *mut c_void) {
        let desc = pxui_types::Node::DEFAULT.role(pxui_types::a11y::Role::Slider).flags(pxui_types::Flags::FOCUSABLE);
        assert_eq!(unsafe { pxui_ui_node_keyed(raw.cast(), 27, &desc, &mut PxuiBox::default()) }, 0);
    }
    let (a, b) = (create(), create());
    let mut ids = [0; 2];
    for (i, (ui, surface)) in [(a, 7), (b, 9)].into_iter().enumerate() {
        assert_eq!(unsafe { pxui_ui_remove_surface(ui, FrameIn::MAIN) }, 0);
        assert_eq!(unsafe { pxui_ui_add_surface(ui, surface) }, 0);
        assert_eq!(unsafe { pxui_ui_set_a11y(ui, 1) }, 0);
        assert_eq!(unsafe { pxui_frame(ui, &{ let mut record = frame_in(); record.surface = surface; record }, Some(build), core::ptr::null_mut(), &mut FrameOut::default()) }, 0);
        let output = unsafe { &*pxui_ui_output(ui) };
        let nodes = unsafe { core::slice::from_raw_parts(output.a11y.nodes.ptr, output.a11y.nodes.len) };
        ids[i] = nodes.iter().find(|n| n.role == pxui_types::a11y::Role::Slider as u32).unwrap().id;
    }
    assert_eq!(ids[0], ids[1], "independent engines may reuse node ids");
    for (ui, surface, other) in [(a, 7, 9), (b, 9, 7)] {
        let action = pxui_types::ffi::A11yAction { id: ids[0], action_code: 4, payload: 1.0, surface, ..Default::default() };
        assert_eq!(unsafe { pxui_ui_inject_a11y(ui, pxui_types::ffi::Strided::of(&[action])) }, 0);
        for wrong in [0, other] {
            let action = pxui_types::ffi::A11yAction { surface: wrong, ..action };
            assert_eq!(unsafe { pxui_ui_inject_a11y(ui, pxui_types::ffi::Strided::of(&[action])) }, PXUI_ERR_ARGUMENT);
        }
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    }
}

#[test]
fn accessibility_queue_is_bounded_across_calls_and_reuses_its_storage() {
    use pxui_types::ffi::{A11yAction, Strided};
    let ui = create();
    let batch = [A11yAction { id: 1, action_code: 2, ..A11yAction::default() }; 4096];
    for _ in 0..2 {
        assert_eq!(unsafe { pxui_ui_inject_a11y(ui, Strided::of(&batch)) }, 0);
        assert_eq!(unsafe { pxui_ui_inject_a11y(ui, Strided::of(&[])) }, 0);
        assert_eq!(unsafe { pxui_ui_inject_a11y(ui, Strided::of(&batch[..1])) }, PXUI_ERR_STATE);
        assert_eq!(unsafe { (&*(*ui).engine).pending_a11y_actions() }, 4096, "no partial acceptance");
        assert_eq!(run(ui, Some(nothing), core::ptr::null_mut()), 0);
    }
    let before = mem_sys::local_alloc_count();
    assert_eq!(unsafe { pxui_ui_inject_a11y(ui, Strided::of(&batch)) }, 0);
    assert_eq!(mem_sys::local_alloc_count(), before, "a warm bounded queue reuses its storage");
    assert_eq!(unsafe { pxui_ui_remove_surface(ui, FrameIn::MAIN) }, 0);
    assert_eq!(unsafe { (&*(*ui).engine).pending_a11y_actions() }, 0);
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

/// ABI 1.87: a section list's record is checked before any callback (fixed and measured rows); the build gets the lead-in, heading,
/// rows and trailer (`usize::MAX - 2`) with their logical Y (1.103: and a row its box), and every builder's parent is restored after it.
#[test]
fn section_list_validates_before_callbacks_and_builds_every_part() {
    use pxui_types::ffi::{PxuiKeyedDesc, PxuiListResult, PxuiSectionListDesc};
    type Seen = (usize, [bool; 3]);
    extern "C" fn part(user: *mut c_void, raw: *mut c_void, index: usize, cell: *const pxui_types::ffi::PxuiCellState) {
        let (seen, cell) = (unsafe { &mut *user.cast::<Seen>() }, unsafe { &*cell });
        assert!(cell.y.is_finite() && cell.enabled == 1 && cell.reserved == 0);
        // 1.119: the record is size-prefixed
        assert_eq!(cell.size as usize, core::mem::size_of::<pxui_types::ffi::PxuiCellState>(), "part {index}");
        // 1.103: a row is told its box, the other parts the empty box
        assert_eq!(index < usize::MAX - 2, cell.handle.index != 0, "part {index}");
        if index >= usize::MAX - 2 { seen.1[usize::MAX - index] = true; }
        seen.0 += 1;
        let mut node = PxuiBox::default();
        assert_eq!(unsafe { pxui_ui_node_keyed(raw.cast(), 17, &pxui_types::Node::DEFAULT.h(pxui_types::Size::Px(20.0)), &mut node) }, 0);
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
        assert_eq!(unsafe { crate::pxui_ui_node(raw.cast(), Str::from("title"), &pxui_types::Node::DEFAULT, &mut node) }, 0);
        node.box_key
    }
    extern "C" fn part(user: *mut c_void, raw: *mut c_void, index: usize, _: *const pxui_types::ffi::PxuiCellState) { let s = unsafe { &mut *user.cast::<State>() }; if index < 3 { s.1[0][index] = child(raw); } }
    // 1.119: a selection list's and a table's records are size-prefixed too
    extern "C" fn cell(user: *mut c_void, raw: *mut c_void, index: usize, c: *const pxui_types::ffi::PxuiCellState) { assert_eq!(unsafe { (*c).size }, 48); let s = unsafe { &mut *user.cast::<State>() }; if index < 3 { s.1[1][index] = child(raw); } }
    extern "C" fn table_cell(user: *mut c_void, raw: *mut c_void, row: usize, _: usize, c: *const pxui_types::ffi::PxuiCellState) { assert_eq!(unsafe { (*c).size }, 48); let s = unsafe { &mut *user.cast::<State>() }; s.1[2][row] = child(raw); }
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
        let columns = [PxuiColumn { size: core::mem::size_of::<PxuiColumn>() as u32, sortable: 0, grip: 0, has_id: 0, reserved: 0, title: Str::from("A"), id: 0, share: 1.0, w: 0.0 }];
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
    refuse_list(PxuiSelectionListDesc { mode: 4, ..list }, "mode above 3");
    refuse_list(PxuiSelectionListDesc { fit: 2, ..list }, "fit not 0 or 1");
    refuse_list(PxuiSelectionListDesc { open_on_click: 2, ..list }, "open_on_click not 0 or 1");
    refuse_list(PxuiSelectionListDesc { bare: 2, ..list }, "a bare flag past 1");
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
        title: { let mut record = Str::default(); record.ptr = title.as_ptr(); record.len = title.len(); record }, id: id.unwrap_or(0), share: 0.0, w: 80.0 };
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
        if t.mode == 0 && t.open_on_click == 0 && t.bare == 0 && t.reserved2 == [0; 4] && t.rows <= t.flags.len {
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
    refuse_table(PxuiTableDesc { mode: 4, ..table_of(&good, ws(2), 2) }, "mode past 3");
    refuse_table(PxuiTableDesc { open_on_click: 2, ..table_of(&good, ws(2), 2) }, "open_on_click not 0 or 1");
    refuse_table(PxuiTableDesc { bare: 2, ..table_of(&good, ws(2), 2) }, "bare not 0 or 1");
    refuse_table(PxuiTableDesc { reserved2: [0, 0, 0, 1], ..table_of(&good, ws(2), 2) }, "a reserved byte");
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
    let item = |text: &'static [u8]| PxuiPickItem { text: { let mut record = Str::default(); record.ptr = text.as_ptr(); record.len = text.len(); record }, ..PxuiPickItem::default() };
    let good = [item(b"Songs"), PxuiPickItem { has_id: 1, id: 7, detail: Str::from("12"), ..item(b"Albums") }];
    let ui = create();
    let keyed = PxuiKeyedDesc::new("tabs", "Sections");
    let tabs = |items: &[PxuiPickItem], has_selected: u8, focus: u8, result: &mut PxuiSegmentedResult| in_frame(ui, &mut |ui| {
        let mut out = PxuiResponse::EMPTY;
        unsafe { pxui_ui_tabs(ui, &keyed, Slice { ptr: items.as_ptr(), len: items.len() }, has_selected, 1, focus, &mut out, result) }
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
    refused(&[PxuiPickItem { detail: { let mut record = Str::default(); record.ptr = core::ptr::null(); record.len = 3; record }, ..good[0] }], 0, 0, "a null detail with a length");
    let mut result = PxuiSegmentedResult::EMPTY;
    assert_ne!(in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_tabs(ui, &keyed, Slice { ptr: core::ptr::null(), len: 2 }, 0, 0, 0, &mut out, &mut result) } }), 0, "a null span with a length");
    assert_ne!(in_frame(ui, &mut |ui| unsafe { pxui_ui_tabs(ui, &keyed, Slice { ptr: good.as_ptr(), len: 2 }, 0, 0, 0, core::ptr::null_mut(), &mut result) }), 0, "a null response");
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

// ABI 1.85: the section index and the disclosure check every argument before building; a refused call writes no result and never runs
// the disclosure's build.
#[test]
fn section_index_and_disclosure_refuse_invalid_arguments() {
    use pxui_types::ffi::{PxuiKeyedDesc, PxuiResponse, PxuiSegmentedResult, Slice};
    let s = |t: &'static [u8]| { let mut record = Str::default(); record.ptr = t.as_ptr(); record.len = t.len(); record };
    let letters = [s(b"A"), s(b"B")];
    let ui = create();
    let keyed = PxuiKeyedDesc::new("index", "Jump to");
    let index = |letters: Slice<Str>, keyed: &PxuiKeyedDesc, result: &mut PxuiSegmentedResult| in_frame(ui, &mut |ui| {
        let mut out = PxuiResponse::EMPTY;
        unsafe { pxui_ui_section_index(ui, keyed, letters, &mut out, result) }
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
    assert_ne!(in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_section_index(ui, &keyed, Slice { ptr: letters.as_ptr(), len: 2 }, &mut out, core::ptr::null_mut()) } }), 0, "a null result");

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
        assert_eq!(unsafe {pxui_ui_node_keyed(ui,123,&pxui_types::Node::DEFAULT.role(pxui_types::a11y::Role::Button).w(pxui_types::Size::Px(100.0)).h(pxui_types::Size::Px(30.0)),&mut node)},0);
        let valid={ let mut record = Str::default(); record.ptr = "Café".as_ptr(); record.len = 5; record };
        assert_eq!(unsafe {pxui_ui_a11y_value(ui,&node,valid,5,0)},0);
        for value in [{ let mut record = Str::default(); record.ptr = core::ptr::null(); record.len = 1; record },{ let mut record = Str::default(); record.ptr = b"x".as_ptr(); record.len = crate::state::MAX_TEXT_BYTES+1; record },{ let mut record = Str::default(); record.ptr = b"\xff".as_ptr(); record.len = 1; record }] {
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

fn str_of(bytes: &[u8]) -> Str { { let mut record = Str::default(); record.ptr = bytes.as_ptr(); record.len = bytes.len(); record } }
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
        vec![E::pointer(E::POINTER_MOVED, 0, f64::NAN, 0.0)],
        vec![{ let mut record = E::default(); record.kind = E::KEY_PRESSED; record.code = input::Key::Enter.code(); record.value = f64::INFINITY; record }],
        vec![E::pointer(E::SCROLLED, 0, 0.0, f64::NEG_INFINITY)],
        vec![{ let mut record = E::default(); record.kind = E::TEXT_EDITING; record.x = -1.0; record.text = str_of(b"ime"); record }],
        vec![E::pointer(E::RESIZED, 0, 70_000.0, 10.0)],
        vec![E::pointer(E::RESIZED, 0, -1.0, 10.0)],
        // each text within the cap, the two past the frame's budget
        vec![{ let mut record = E::default(); record.kind = E::PASTE; record.text = str_of(&big); record }, { let mut record = E::default(); record.kind = E::PASTE; record.text = str_of(&big); record }],
        // a length past the cap is refused before a byte is read
        vec![{ let mut record = E::default(); record.kind = E::TEXT_INPUT; record.text = { let mut record = Str::default(); record.ptr = b"x".as_ptr(); record.len = crate::state::MAX_TEXT_BYTES + 1; record }; record }],
    ];
    for (i, events) in refused.iter().enumerate() {
        let input = { let mut record = frame_in(); record.events = pxui_types::ffi::Strided { ptr: events.as_ptr(), len: events.len(), stride: core::mem::size_of::<pxui_types::Event>() }; record };
        assert_eq!(frame_with(ui, &input), PXUI_ERR_ARGUMENT, "frame, case {i}");
        let mut wants = 7u8;
        assert_eq!(unsafe { pxui_ui_wants_frame(ui, pxui_types::ffi::Strided { ptr: events.as_ptr(), len: events.len(), stride: 64 }, 0.0, &mut wants) }, PXUI_ERR_ARGUMENT, "wants_frame, case {i}");
        assert_eq!(wants, 7);
    }
    // one paste of half the budget is fine, and so is a resize at the limit
    let fine = [{ let mut record = E::default(); record.kind = E::PASTE; record.text = str_of(&big); record }, E::pointer(E::RESIZED, 0, 65_536.0, 0.0)];
    assert_eq!(frame_with(ui, &{ let mut record = frame_in(); record.events = pxui_types::ffi::Strided { ptr: fine.as_ptr(), len: 2, stride: core::mem::size_of::<pxui_types::Event>() }; record }), 0, "{}", error(ui));
    let moves = [E::pointer(E::POINTER_MOVED, 0, 1.0, 2.0); 3];
    let mut wants = 0u8;
    for _ in 0..2 { assert_eq!(unsafe { pxui_ui_wants_frame(ui, pxui_types::ffi::Strided { ptr: moves.as_ptr(), len: 3, stride: 64 }, 0.0, &mut wants) }, 0); }
    mem_sys::local_reset();
    let before = mem_sys::local_alloc_count();
    assert_eq!(unsafe { pxui_ui_wants_frame(ui, pxui_types::ffi::Strided { ptr: moves.as_ptr(), len: 3, stride: 64 }, 0.0, &mut wants) }, 0);
    assert_eq!(mem_sys::local_alloc_count() - before, 0, "a settled wants_frame allocates");
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

/// U5.4: a record array crosses as {ptr, len, stride}. The reader steps by the stride (a wider record than this build's is read as its
/// prefix), refuses a stride below the record size, one that is not a multiple of the alignment and a null pointer with a length;
/// each record's own size must still cover the record. The same rules for events (frame, wants_frame) and accessibility actions.
#[test]
fn strided_spans_step_by_stride_and_refuse_a_stride_below_the_record() {
    use pxui_types::{Event as E, ffi::{A11yAction, Strided}};
    #[repr(C)] #[derive(Clone, Copy)] struct WideEvent { e: E, tail: [u8; 8] }
    #[repr(C)] #[derive(Clone, Copy)] struct WideAction { a: A11yAction, tail: [u8; 8] }
    let ui = create();
    let moved = E::pointer(E::POINTER_MOVED, 0, 1.0, 2.0);
    let wide = [WideEvent { e: moved, tail: [0; 8] }; 2];
    let span = |stride: usize, len: usize| Strided::<E> { ptr: wide.as_ptr().cast(), len, stride };
    let frame = |events: Strided<E>| frame_with(ui, &{ let mut record = frame_in(); record.events = events; record });
    let mut wants = 0u8;
    let mut ask = |events: Strided<E>| unsafe { pxui_ui_wants_frame(ui, events, 0.0, &mut wants) };
    // the real stride of the wide record reads both events; the dense one reads the second at the tail (size 0, refused)
    assert_eq!(frame(span(72, 2)), 0, "{}", error(ui));
    assert_eq!(ask(span(72, 2)), 0);
    assert_eq!(frame(span(64, 2)), PXUI_ERR_ARGUMENT, "a second record read at the wrong stride is not an event");
    assert_eq!(ask(span(64, 2)), PXUI_ERR_ARGUMENT);
    for (stride, what) in [(63, "one byte below the event"), (0, "zero"), (4, "a field"), (68, "not a multiple of the alignment")] {
        assert_eq!(frame(span(stride, 2)), PXUI_ERR_ARGUMENT, "frame, stride {stride} ({what})");
        assert_eq!(ask(span(stride, 2)), PXUI_ERR_ARGUMENT, "wants_frame, stride {stride} ({what})");
    }
    assert_eq!(frame(Strided { ptr: core::ptr::null(), len: 1, stride: 64 }), PXUI_ERR_ARGUMENT, "a null pointer with a length");
    assert_eq!(frame(Strided { ptr: core::ptr::null(), len: 0, stride: 64 }), 0, "an empty span needs no pointer");
    assert_eq!(frame(Strided { ptr: core::ptr::null(), len: 0, stride: 0 }), PXUI_ERR_ARGUMENT, "a stride below the record is refused even when empty");
    // actions: a set-value on the slider's role is not needed, an unknown id is dropped by the engine; the span rules are what is checked
    let action = A11yAction { id: 1, action_code: 4, payload: 1.0, ..A11yAction::default() };
    let actions = [WideAction { a: action, tail: [0; 8] }; 2];
    let aspan = |stride: usize| Strided::<A11yAction> { ptr: actions.as_ptr().cast(), len: 2, stride };
    assert_eq!(unsafe { pxui_ui_inject_a11y(ui, aspan(48)) }, 0, "{}", error(ui));
    assert_eq!(unsafe { pxui_ui_inject_a11y(ui, aspan(40)) }, PXUI_ERR_ARGUMENT, "the second action read at the dense stride is not one");
    assert_eq!(unsafe { pxui_ui_inject_a11y(ui, aspan(39)) }, PXUI_ERR_ARGUMENT);
    assert_eq!(unsafe { pxui_ui_inject_a11y(ui, aspan(44)) }, PXUI_ERR_ARGUMENT, "not a multiple of the alignment");
    let short = [A11yAction { size: 28, ..action }];
    assert_eq!(unsafe { pxui_ui_inject_a11y(ui, Strided::of(&short)) }, PXUI_ERR_ARGUMENT, "an action whose own size is short");
    // what the library writes is dense: the output's spans step by the record sizes
    assert_eq!(frame(span(72, 0)), 0);
    let output = unsafe { &*pxui_ui_output(ui) };
    assert_eq!(output.a11y.size as usize, core::mem::size_of::<pxui_types::ffi::A11yTree>());
    assert_eq!((output.a11y.nodes.stride, output.layers.stride), (core::mem::size_of::<pxui_types::ffi::A11yNode>(), core::mem::size_of::<pxui_types::ffi::PxuiLayerInfo>()));
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
}

/// U5.6: every export that takes a box answers another Ui's handle, a zeroed one and one with its serial cleared with the one stale status
/// (`PXUI_ERR_STALE`), and not that status for the Ui's own handle. The table is the box-taking calls of `pxui.h`; a call needing a build
/// context gets one.
#[test]
fn every_box_taking_export_answers_a_foreign_handle_with_the_stale_status() {
    use pxui_types::{ffi::{PxuiGestureIn, PxuiGestureOut, PxuiLookDesc, PxuiMarkDesc, PxuiNodeFields, PxuiSignal, PxuiStateDesc, PxuiSwipeIn, PxuiDismissIn, PxuiDismissOut, PxuiKeyframeDesc, PxuiLabelStyle}, geom::{Point as PxuiPoint, Rect, Rect as PxuiRect}, Node};
    let (a, b) = (create(), create());
    let mut theirs = PxuiBox::default();
    assert_eq!(in_frame(b, &mut |ui| unsafe { pxui_ui_node_keyed(ui, 4242, &Node::DEFAULT, &mut theirs) }), 0);
    let text = Str::from("x");
    let (look, state, rect, mark) = (PxuiLookDesc::default(), PxuiStateDesc::default(), Rect::new(0.0, 0.0, 10.0, 10.0), PxuiMarkDesc::default());
    let label = PxuiLabelStyle::from(&pxui_types::Label::BODY);
    let spans = pxui_types::TextSpans::default();
    let (gin, swipe, dismiss) = (PxuiGestureIn::default(), PxuiSwipeIn::default(), PxuiDismissIn::default());
    let key = PxuiKeyframeDesc::default();
    let transform = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
    type Call = Box<dyn Fn(*mut PxuiUi, &PxuiBox) -> i32>;
    let calls: Vec<(&str, Call)> = vec![
        ("a11y_name", Box::new(move |u, h| unsafe { pxui_ui_a11y_name(u, h, text) })),
        ("a11y_value", Box::new(move |u, h| unsafe { pxui_ui_a11y_value(u, h, text, 0, 0) })),
        ("a11y_number", Box::new(|u, h| unsafe { pxui_ui_a11y_number(u, h, 1.0, 0.0, 2.0, 1.0) })),
        ("set_role", Box::new(|u, h| unsafe { pxui_ui_set_role(u, h, 0, 0) })),
        ("focus_box", Box::new(|u, h| unsafe { pxui_ui_focus_box(u, h) })),
        ("focus_default", Box::new(|u, h| unsafe { pxui_ui_focus_default(u, h) })),
        ("focused", Box::new(|u, h| unsafe { pxui_ui_focused(u, h, &mut 0) })),
        ("state", Box::new(|u, h| unsafe { pxui_ui_state(u, h, 0, &mut 0) })),
        ("set_state", Box::new(|u, h| unsafe { pxui_ui_set_state(u, h, 1) })),
        ("signal", Box::new(|u, h| unsafe { let mut out: PxuiSignal = core::mem::zeroed(); out.size = core::mem::size_of::<PxuiSignal>() as u32; pxui_ui_signal(u, h, &mut out) })),
        ("patch_text", Box::new(move |u, h| unsafe { pxui_ui_patch_text(u, h, text, &mut 0) })),
        ("busy_step", Box::new(|u, h| unsafe { pxui_ui_busy_step(u, h, 1.0, &mut 0) })),
        ("rect", Box::new(|u, h| unsafe { pxui_ui_rect(u, h, &mut PxuiRect::default()) })),
        ("text", Box::new(move |u, h| unsafe { pxui_ui_text(u, h, text, core::ptr::null(), core::ptr::null(), 0) })),
        ("style", Box::new(move |u, h| unsafe { pxui_ui_style(u, h, &look, &state) })),
        ("within", Box::new(|u, h| unsafe { pxui_ui_within(u, h, None, core::ptr::null_mut()) })),
        ("tip", Box::new(move |u, h| unsafe { pxui_ui_tip(u, h, text) })),
        ("label_text_spans", Box::new(move |u, h| unsafe { pxui_ui_text(u, h, text, &label, &spans, 0) })),
        ("label_text_line_box", Box::new(move |u, h| unsafe { pxui_ui_text(u, h, text, &label, &spans, 1) })),
        ("set_at", Box::new(|u, h| unsafe { pxui_ui_set_at(u, h, 1.0, 2.0) })),
        ("patch_look", Box::new(move |u, h| unsafe { pxui_ui_patch_look(u, h, &look, &mut 0) })),
        ("patch_rect", Box::new(move |u, h| unsafe { pxui_ui_patch_rect(u, h, rect, &mut 0) })),
        ("patch_mark", Box::new(move |u, h| unsafe { pxui_ui_patch_mark(u, h, &mark, &mut 0) })),
        ("node_fields", Box::new(|u, h| unsafe { let mut out: PxuiNodeFields = core::mem::zeroed(); out.size = core::mem::size_of::<PxuiNodeFields>() as u32; pxui_ui_node_fields(u, h, &mut out) })),
        ("node_layout", Box::new(|u, h| unsafe { let mut out = Node::DEFAULT; pxui_ui_node_layout(u, h, &mut out) })),
        ("set_node_layout", Box::new(|u, h| unsafe { pxui_ui_set_node_layout(u, h, &Node::DEFAULT) })),
        ("local_rect", Box::new(|u, h| unsafe { pxui_ui_local_rect(u, h, &mut PxuiRect::default()) })),
        ("scroll_offset", Box::new(|u, h| unsafe { pxui_ui_scroll_offset(u, h, 0, &mut 0.0) })),
        ("set_scroll_offset", Box::new(|u, h| unsafe { pxui_ui_set_scroll_offset(u, h, 1.0) })),
        ("reveal", Box::new(|u, h| unsafe { pxui_ui_reveal(u, h, 0) })),
        ("gesture", Box::new(move |u, h| unsafe { let mut out: PxuiGestureOut = core::mem::zeroed(); out.size = core::mem::size_of::<PxuiGestureOut>() as u32; pxui_ui_gesture(u, h, &gin, 0, &mut out) })),
        ("swipe_actions", Box::new(move |u, h| unsafe { pxui_ui_swipe_actions(u, h, &swipe) })),
        ("dismiss", Box::new(move |u, h| unsafe { let mut out: PxuiDismissOut = core::mem::zeroed(); out.size = core::mem::size_of::<PxuiDismissOut>() as u32; pxui_ui_dismiss(u, h, &dismiss, &mut out) })),
        ("to_front", Box::new(|u, h| unsafe { pxui_ui_to_front(u, 1, h) })),
        ("context_at", Box::new(|u, h| unsafe { pxui_ui_context_at(u, h, &mut 0, &mut PxuiPoint::default()) })),
        ("hovered", Box::new(|u, h| unsafe { pxui_ui_hovered(u, h, 0, &mut 0) })),
        ("press_keys", Box::new(|u, h| unsafe { let (mut ptr, mut len) = (core::ptr::null(), 0usize); pxui_ui_press_keys(u, h, &mut ptr, &mut len) })),
        ("view_offset", Box::new(|u, h| unsafe { pxui_ui_view_offset(u, h, 1.0, 1.0) })),
        ("promote", Box::new(|u, h| unsafe { pxui_ui_promote(u, h) })),
        ("view_opacity", Box::new(|u, h| unsafe { pxui_ui_view_opacity(u, h, 0.5) })),
        ("view_scale", Box::new(|u, h| unsafe { pxui_ui_view_scale(u, h, 1.0) })),
        ("view_clip", Box::new(|u, h| unsafe { pxui_ui_view_clip(u, h, &PxuiRect::default()) })),
        ("view_transform_about", Box::new(move |u, h| unsafe { pxui_ui_view_transform_about(u, h, 1.0, transform.as_ptr(), 0.0, 0.0) })),
        ("view_backdrop_blur", Box::new(|u, h| unsafe { pxui_ui_view_backdrop_blur(u, h, 1.0) })),
        ("view_backdrop_saturation", Box::new(|u, h| unsafe { pxui_ui_view_backdrop_saturation(u, h, 1.0) })),
        ("animate", Box::new(move |u, h| unsafe { pxui_ui_animate(u, h, &key, &mut 0) })),
        ("field_drag", Box::new(|u, h| unsafe { pxui_ui_field_drag(u, h, &mut 0, &mut PxuiPoint::default()) })),
        ("cached_label", Box::new(move |u, h| unsafe { pxui_ui_cached_label(u, h, 0, text, &mut Str::default()) })),
        ("text_state", Box::new(|u, h| unsafe { pxui_ui_text_state(u, h, &mut 0, &mut Str::default()) })),
        ("set_text_state", Box::new(move |u, h| unsafe { pxui_ui_set_text_state(u, h, 1, text) })),
        ("animate_keys_at_rate", Box::new(|u, h| unsafe { pxui_ui_animate_keys_at_rate(u, h, pxui_types::ffi::Slice { ptr: core::ptr::null(), len: 0 }, 1, 0, 0.0, 20, &mut 0) })),
        ("animate_keys", Box::new(|u, h| unsafe { pxui_ui_animate_keys(u, h, pxui_types::ffi::Slice { ptr: core::ptr::null(), len: 0 }, 1, 0, 0.0, &mut 0) })),
        ("carry", Box::new(move |u, h| unsafe { pxui_ui_carry(u, 1, h, text, text) })),
        ("transfer_carry", Box::new(|u, h| unsafe { pxui_ui_transfer_carry(u, h, &mut 0) })),
        ("draw", Box::new(|u, h| unsafe { pxui_ui_draw(u, h, 0, None, core::ptr::null_mut(), None) })),
    ];
    let fixed_serial = |mine: PxuiBox| PxuiBox { ui: 0, ..mine };
    let run = |ui: *mut PxuiUi, mine: &PxuiBox, bad: [PxuiBox; 3], calls: &[(&str, Call)]| {
        let _ = mine;
        for (name, call) in calls {
            for handle in &bad { assert_eq!(call(ui, handle), PXUI_ERR_STALE, "{name} with {handle:?}: {}", error(ui)); }
        }
    };
    let mut mine = PxuiBox::default();
    let rc = in_frame(a, &mut |ui| {
        assert_eq!(unsafe { pxui_ui_node_keyed(ui, 4242, &Node::DEFAULT, &mut mine) }, 0);
        run(ui, &mine, [theirs, PxuiBox::default(), fixed_serial(mine)], &calls);
        // the Ui's own handle is never the stale status (the call may refuse for another reason, or do its job)
        for (name, call) in &calls { assert_ne!(call(ui, &mine), PXUI_ERR_STALE, "{name} with its own handle: {}", error(ui)); }
        0
    });
    assert_eq!(rc, 0);
    // outside a frame the queries answer too
    for (name, call) in &calls { if ["rect", "state", "signal", "focused", "scroll_offset", "local_rect"].contains(name) { assert_eq!(call(a, &theirs), PXUI_ERR_STALE, "{name} outside a frame"); } }
    unsafe { pxui_ui_destroy(a); pxui_ui_destroy(b); }
}

/// U5.6: one rule each for SIZE_MAX as none, the text key 0, a SET mask against a documented 0, and `repaint_at` infinity (the header's
/// "Conventions of the encodings").
#[test]
fn encoding_rules() {
    use pxui_types::ffi::{PxuiKeyedDesc, PxuiLabelStyle, PxuiListResult, PxuiSelectionListDesc, PxuiTextStyle, PxuiU8MutSpan};
    let ui = create();
    // none: SIZE_MAX, and any index at or past the count, is no anchor (never an error); an index below the count is kept
    let mut rows = [0u8; 3];
    for (anchor, kept) in [(usize::MAX, false), (3, false), (1_000_000, false), (1, true)] {
        let mut list: PxuiSelectionListDesc = unsafe { core::mem::zeroed() };
        (list.size, list.count, list.row_height, list.rows, list.anchor) = (core::mem::size_of::<PxuiSelectionListDesc>() as u32, 3, 20.0, PxuiU8MutSpan { ptr: rows.as_mut_ptr(), len: 3 }, anchor);
        let mut out = PxuiListResult::EMPTY;
        let rc = in_frame(ui, &mut |ui| unsafe { pxui_ui_selection_list(ui, &PxuiKeyedDesc::new("rule", ""), &list, None, None, core::ptr::null_mut(), &mut out) });
        assert_eq!(rc, 0, "anchor {anchor}: {}", error(ui));
        assert_eq!(out.anchor == 1, kept, "anchor {anchor} came back as {}", out.anchor);
        if !kept { assert_eq!(out.anchor, usize::MAX, "anchor {anchor} is none"); }
    }
    // the text key: 0 derives the layout from the content; a nonzero key is a cache name, so other content under it is laid out again
    let (style, text_a, text_b) = (PxuiTextStyle::default(), Str::from("a"), Str::from("abcdef"));
    let shaper = unsafe { pxui_text_fork(ui) };
    assert!(!shaper.is_null());
    let width = |key: u64, text: Str| { let mut m = pxui_types::TextMetrics::default(); assert_eq!(unsafe { pxui_shaper_measure(shaper, key, text, &style, 0.0, core::ptr::null(), &mut m) }, 0); m.width };
    let (a, b) = (width(0, text_a), width(0, text_b));
    assert!(b > a, "key 0 measures each content");
    assert_eq!((width(0, text_a), width(0, text_b)), (a, b), "key 0 is repeatable");
    assert_eq!((width(7, text_a), width(7, text_b), width(7, text_a)), (a, b, a), "a nonzero key is a name: other content under it is laid out again");
    unsafe { pxui_shaper_destroy(shaper) };
    // zero: a clear SET bit ignores the field whatever it holds, a set bit reads it; a documented 0 outside a mask is the default
    let label = PxuiLabelStyle::from(&pxui_types::Label::BODY);
    let (mut base, mut ignored, mut used) = (pxui_types::TextMetrics::default(), pxui_types::TextMetrics::default(), pxui_types::TextMetrics::default());
    let text = Str::from("Song title");
    for (record, out) in [(PxuiLabelStyle { set: 0, ..label }, &mut base), (PxuiLabelStyle { set: 0, size_points: 90.0, ..label }, &mut ignored), (PxuiLabelStyle { set: PxuiLabelStyle::SET_SIZE, size_points: 90.0, ..label }, &mut used)] {
        assert_eq!(unsafe { pxui_ui_label_measure(ui, text, &record, 0.0, core::ptr::null(), out) }, 0, "{}", error(ui));
    }
    assert_eq!(base, ignored, "a clear SET_SIZE ignores size_points");
    assert!(used.width > base.width * 3.0, "a set SET_SIZE reads it");
    let (mut zero, mut explicit) = (pxui_types::TextMetrics::default(), pxui_types::TextMetrics::default());
    let wrapped = |line_height: f32| PxuiTextStyle { line_height, size_points: 20.0, ..PxuiTextStyle::default() };
    let height = |style: &PxuiTextStyle, out: &mut pxui_types::TextMetrics| assert_eq!(unsafe { crate::text::text_width(ui, 0, text, style, 30.0, out) }, 0);
    height(&wrapped(0.0), &mut zero);
    height(&wrapped(24.0), &mut explicit);
    assert_eq!(zero.height, explicit.height, "a line height of 0 outside a mask is 1.2 times the size (24 for 20 points)");
    // repaint_at: an idle Ui wants no frame (infinity); a running animation names a time
    assert_eq!(frame_with(ui, &frame_in()), 0, "{}", error(ui));
    assert_eq!(unsafe { (*pxui_ui_output(ui)).repaint_at }, f64::INFINITY, "an idle Ui has no scheduled repaint");
    let moving = in_frame(ui, &mut |ui| {
        let mut node = PxuiBox::default();
        let swatch = pxui_types::Node::DEFAULT.w_px(20.0).h_px(20.0)
            .look(pxui_types::theme::Look::NONE.fill(pxui_types::geom::Rgba::rgb(240, 40, 40)));
        assert_eq!(unsafe { pxui_ui_node_keyed(ui, 77, &swatch, &mut node) }, 0);
        let mut key: pxui_types::ffi::PxuiKeyframeDesc = unsafe { core::mem::zeroed() };
        key.size = core::mem::size_of::<pxui_types::ffi::PxuiKeyframeDesc>() as u32;
        (key.time, key.target, key.values, key.look, key.curve) = (1.0, 0, [10.0, 0.0, 0.0, 0.0], pxui_types::ffi::PxuiLookDesc::default(), pxui_types::ffi::PxuiCurveDesc { kind: pxui_types::ffi::PxuiCurveDesc::BEZIER, duration: 1.0, points: [0.0, 0.0, 1.0, 1.0], ..pxui_types::ffi::PxuiCurveDesc::default() });
        let mut running = 0u8;
        unsafe { pxui_ui_animate(ui, &node, &key, &mut running) }
    });
    assert_eq!(moving, 0, "{}", error(ui));
    let repaint = unsafe { (*pxui_ui_output(ui)).repaint_at };
    assert!(repaint.is_finite(), "a running animation names a time (never NaN, never infinity): {repaint}");
    unsafe { pxui_ui_destroy(ui); }
}

/// U5.5: an enum a caller writes (a tag byte or a number) is checked against its range before anything is built from it: the largest
/// valid value is accepted, the next one is refused, and the refusal leaves the call a no-op. One row each for the records a caller fills.
#[test]
fn enum_values_are_range_checked_on_input() {
    use pxui_types::{Node, ffi::{PxuiAnchorDesc, PxuiButtonDesc, PxuiKeyframeDesc, PxuiLabelStyle, PxuiResponse, PxuiSheetDesc, PxuiSizeDesc}};
    let ui = create();
    // the Node's tag bytes: (offset, largest valid value)
    let node_bytes = [(core::mem::offset_of!(Node, axis), 2u8), (core::mem::offset_of!(Node, justify), 5), (core::mem::offset_of!(Node, align), 4), (core::mem::offset_of!(Node, wrap), 1),
        (core::mem::offset_of!(Node, role), pxui_types::a11y::Role::Separator as u8), (core::mem::offset_of!(Node, state.hovered), 1), (core::mem::offset_of!(Node, mark), 2)];
    for (at, largest) in node_bytes {
        for (value, accepted) in [(largest, true), (largest + 1, false)] {
            let mut node = Node::DEFAULT;
            unsafe { (&mut node as *mut Node).cast::<u8>().add(at).write(value) };
            let rc = in_frame(ui, &mut |ui| unsafe { pxui_ui_node_keyed(ui, 31, &node, &mut PxuiBox::default()) });
            assert_eq!(rc == 0, accepted, "node byte at {at} = {value}: {}", error(ui));
        }
    }
    // the Ui's platform, the frame's appearance and reduced-motion, an event's kind
    for (platform, accepted) in [(2, true), (3, false)] {
        let desc = crate::PxuiUiDesc { platform, ..crate::PxuiUiDesc::default() };
        let made = unsafe { pxui_ui_create(&desc, core::ptr::null(), 0, core::ptr::null_mut()) };
        assert_eq!(!made.is_null(), accepted, "platform {platform}");
        if !made.is_null() { assert_eq!(unsafe { pxui_ui_destroy(made) }, 0); }
    }
    for (input, accepted) in [({ let mut record = frame_in(); record.appearance = 2; record }, true), ({ let mut record = frame_in(); record.appearance = 3; record }, false), ({ let mut record = frame_in(); record.reduced_motion = 1; record }, true), ({ let mut record = frame_in(); record.reduced_motion = 2; record }, false)] {
        assert_eq!(frame_with(ui, &input) == 0, accepted, "{}", error(ui));
    }
    for (kind, accepted) in [(pxui_types::Event::FILES_DROPPED, true), (0, false), (11, false), (pxui_types::Event::FILES_DROPPED + 1, false)] {
        let event = [{ let mut record = pxui_types::Event::default(); record.kind = kind; record }];
        let input = { let mut record = frame_in(); record.events = pxui_types::ffi::Strided::of(&event); record };
        // FILES_DROPPED carries paths: an empty payload is the kind that decides, so only the rest (unknown kinds) are asserted refused
        if !accepted { assert_eq!(frame_with(ui, &input), PXUI_ERR_ARGUMENT, "event kind {kind}"); }
    }
    // a button's justification and size kinds, an anchor's side and alignment, a sheet's presentation, a keyframe's target
    let k = Str::from("k");
    let mut button: PxuiButtonDesc = unsafe { core::mem::zeroed() };
    (button.size, button.enabled, button.key, button.text) = (core::mem::size_of::<PxuiButtonDesc>() as u32, 1, k, k);
    for (field, largest) in [(0u8, 2u8), (1, 2), (2, 2)] {
        for (value, accepted) in [(largest, true), (largest + 1, false)] {
            let mut d = button;
            match field { 0 => d.justify = value, 1 => d.w_kind = value, _ => d.h_kind = value }
            let rc = in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_button(ui, &d, None, core::ptr::null_mut(), &mut out) } });
            assert_eq!(rc == 0, accepted, "button field {field} = {value}: {}", error(ui));
        }
    }
    let px = PxuiSizeDesc { kind: PxuiSizeDesc::PX, value: 40.0, relative: None };
    for (side, align, accepted) in [(3, 2, true), (4, 2, false), (0, 5, false)] {
        let anchor = PxuiAnchorDesc { has_anchor: 1, side, align, ..PxuiAnchorDesc::default() };
        let rc = in_frame(ui, &mut |ui| unsafe { pxui_ui_anchored(ui, &pxui_types::ffi::PxuiKeyedDesc::new("a", ""), &anchor, px, px, None, core::ptr::null_mut(), &mut PxuiBox::default()) });
        assert_eq!(rc == 0, accepted, "anchor side {side} align {align}");
    }
    for (present, accepted) in [(3, true), (4, false)] {
        let sheet = PxuiSheetDesc { size: core::mem::size_of::<PxuiSheetDesc>() as u32, present, has_width: 0, reserved: [0; 2], width: 0.0, dismiss_fraction: 0.0, dismiss_velocity: 0.0, top: 0.0, max_h: 0.0 };
        let rc = in_frame(ui, &mut |ui| { let mut open = 0u8; unsafe { pxui_ui_modal(ui, &pxui_types::ffi::PxuiKeyedDesc::new("m", ""), &sheet, None, core::ptr::null_mut(), &mut open, &mut PxuiBox::default()) } });
        assert_eq!(rc == 0, accepted, "sheet present {present}: {}", error(ui));
    }
    for (target, accepted) in [(3u32, true), (4, false)] {
        let mut key: PxuiKeyframeDesc = unsafe { core::mem::zeroed() };
        key.size = core::mem::size_of::<PxuiKeyframeDesc>() as u32;
        (key.target, key.look, key.curve) = (target, pxui_types::ffi::PxuiLookDesc::default(), pxui_types::ffi::PxuiCurveDesc { kind: 2, omega: 20.0, ..pxui_types::ffi::PxuiCurveDesc::default() });
        let rc = in_frame(ui, &mut |ui| { let b = PxuiBox::default(); let _ = b; let mut node = PxuiBox::default(); assert_eq!(unsafe { pxui_ui_node_keyed(ui, 9, &Node::DEFAULT, &mut node) }, 0); let mut running = 0u8; unsafe { pxui_ui_animate(ui, &node, &key, &mut running) } });
        assert_eq!(rc == 0, accepted, "keyframe target {target}: {}", error(ui));
    }
    // a label style's wrap, alignment and ellipsis
    let label = PxuiLabelStyle::from(&pxui_types::Label::BODY);
    let (text, mut out) = (Str::from("x"), pxui_types::TextMetrics::default());
    for (what, bad) in [("wrap", PxuiLabelStyle { wrap: 9, ..label }), ("align", PxuiLabelStyle { align: 9, ..label }), ("ellipsis", PxuiLabelStyle { ellipsis: 9, ..label })] {
        assert_eq!(unsafe { pxui_ui_label_measure(ui, text, &bad, 0.0, core::ptr::null(), &mut out) }, PXUI_ERR_ARGUMENT, "label style {what}");
    }
    assert_eq!(unsafe { pxui_ui_label_measure(ui, text, &label, 0.0, core::ptr::null(), &mut out) }, 0);
    unsafe { pxui_ui_destroy(ui); }
}

/// A frame's size is within `0..=MAX_SURFACE`; a key event's code must name a key.
#[test]
fn frame_sizes_and_held_keys_are_checked() {
    let ui = create();
    for (w, h) in [(-1.0, 10.0), (10.0, 1e30), (f32::NAN, 10.0), (65_537.0, 10.0)] {
        assert_eq!(frame_with(ui, &{ let mut record = frame_in(); record.width = w; record.height = h; record }), PXUI_ERR_ARGUMENT, "{w} x {h}");
    }
    assert_eq!(frame_with(ui, &{ let mut record = frame_in(); record.width = 65_536.0; record.height = 0.0; record }), 0);
    let key = |code| [pxui_types::Event::key(pxui_types::Event::KEY_PRESSED, code)];
    let with = |events: &[pxui_types::Event]| frame_with(ui, &{ let mut record = frame_in(); record.events = pxui_types::ffi::Strided { ptr: events.as_ptr(), len: events.len(), stride: core::mem::size_of::<pxui_types::Event>() }; record });
    assert_eq!(with(&key(0)), PXUI_ERR_ARGUMENT, "code 0 names no key");
    assert_eq!(with(&key(input::Key::Enter.code())), 0);
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
        assert_eq!(unsafe { pxui_ui_text(ui, &b, str_of(label), core::ptr::null(), core::ptr::null(), 0) }, 0);
        assert_eq!(unsafe { pxui_ui_text(ui, &b, str_of(&long), core::ptr::null(), core::ptr::null(), 0) }, PXUI_ERR_ARGUMENT);
        // a text field's value is editor text: past the label cap is fine
        let (mut changed, mut flag) = (Str::default(), 0u8);
        let field = pxui_types::ffi::PxuiTextFieldDesc { size: core::mem::size_of::<pxui_types::ffi::PxuiTextFieldDesc>() as u32, autocorrect: 0, capitalization: 0, action: 0, reserved: 0,
            key: str_of(b"f"), name: Str::default(), value: str_of(&long), variant: 0, reserved_variant: [0; 6] };
        assert_eq!(unsafe { pxui_ui_text_field(ui, &field, &mut changed, &mut flag, &mut { pxui_types::ffi::PxuiResponse::EMPTY }) }, 0, "{}", error(ui));
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
        assert_eq!(pxui_shaper_measure(shaper, 0, str_of(b"a"), &style, 0.0, core::ptr::null(), &mut metrics), 0);
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
    assert_eq!(unsafe { pxui_shaper_measure(shaper, 0, str_of(b"a"), &style, 0.0, core::ptr::null(), &mut metrics) }, 0);
    unsafe { (*shaper).poisoned = true; }
    assert_eq!(unsafe { pxui_shaper_measure(shaper, 0, str_of(b"a"), &style, 0.0, core::ptr::null(), &mut metrics) }, PXUI_ERR_PANIC);
    assert_eq!(unsafe { pxui_shaper_add_font(shaper, str_of(b"")) }, u32::MAX);
    unsafe { pxui_shaper_destroy(shaper); }
    let theme = unsafe { pxui_theme_create(core::ptr::null()) };
    unsafe { (*theme).poisoned = true; }
    assert_eq!(unsafe { pxui_theme_parse(theme, str_of(b"tokens.accent = #ff0000")) }, PXUI_ERR_PANIC);
    unsafe { pxui_theme_destroy(theme); }
}

fn calls_of(ui: *mut PxuiUi) -> u64 {
    let mut d = pxui_types::ffi::PxuiDiagnostics::new();
    if unsafe { pxui_ui_diagnostics(ui, &mut d) } == 0 { d.calls } else { 0 }
}

/// Calls are counted per `Ui`: one `Ui`'s calls never move another's, and reading the count is not a call.
#[test]
fn calls_are_counted_per_ui() {
    let (a, b) = (create(), create());
    let (at_a, at_b) = (calls_of(a), calls_of(b));
    for _ in 0..5 { assert_eq!(unsafe { pxui_ui_set_dir(a, 0) }, 0); }
    assert_eq!(calls_of(a) - at_a, 5);
    assert_eq!(calls_of(b), at_b);
    let mut built = 0u32;
    assert_eq!(run(b, Some(count), (&mut built as *mut u32).cast()), 0);
    assert_eq!(calls_of(a) - at_a, 5);
    assert!(calls_of(b) > at_b);
    assert_eq!(calls_of(core::ptr::null_mut()) | unsafe { pxui_counter(0) }, 0);
    assert_eq!(unsafe { pxui_ui_destroy(a) } | unsafe { pxui_ui_destroy(b) }, 0);
}

/// A builder export that ends in its trusted entry is one call: `item_text`, `disclosure` and `part` count once (they counted twice,
/// as an export and again as the entry, and checked the handle twice) and refuse as before.
#[test]
fn a_builder_export_ending_in_its_trusted_entry_counts_one_call() {
    let ui = create();
    let mut deltas = [0u64; 3];
    assert_eq!(in_frame(ui, &mut |h| {
        let mut at = calls_of(h);
        let mut next = |i: usize| { let now = calls_of(h); deltas[i] = now - at; at = now; };
        assert_ne!(unsafe { pxui_ui_item_text(h, str_of(b"row")) }, 0, "item_text outside a row is refused");
        next(0);
        let mut out = pxui_types::ffi::PxuiResponse::EMPTY;
        assert_eq!(unsafe { pxui_ui_disclosure(h, &pxui_types::ffi::PxuiKeyedDesc::new("more", "More"), 0, None, core::ptr::null_mut(), &mut out) }, 0);
        next(1);
        let mut part = PxuiBox::default();
        assert_eq!(unsafe { pxui_ui_part(h, pxui_types::theme::Part::HIT.0, &pxui_types::Node::DEFAULT, &mut part) }, PXUI_ERR_STATE, "a part outside a body");
        next(2);
        0
    }), 0);
    assert_eq!(deltas, [1, 1, 1]);
    assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
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
            let node = pxui_types::Node::DEFAULT.grid(Grid::new(&COLS, &ROWS));
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
    #[test]
    fn inspector_header_keeps_each_title_alive_until_the_next_frame() {
        let ui = create();
        assert_eq!(in_frame(ui, &mut |ui| {
            let build = |key: &[u8], title: &[u8]| {
                let mut d: PxuiHeaderDesc = unsafe { core::mem::zeroed() };
                d.size = core::mem::size_of::<PxuiHeaderDesc>() as u32;
                d.key = { let mut record = Str::default(); record.ptr = key.as_ptr(); record.len = key.len(); record };
                d.title = { let mut record = Str::default(); record.ptr = title.as_ptr(); record.len = title.len(); record };
                let mut out = head_out();
                assert_eq!(unsafe { pxui_ui_inspector_header(ui, &d, None, core::ptr::null_mut(), &mut out) }, 0);
                out.renamed
            };
            let mut title = *b"First title";
            let a = build(b"first", &title);
            title.copy_from_slice(b"Other title");
            let b = build(b"second", &title);
            title.fill(b'x');
            assert_eq!(unsafe { crate::state::text(a) }, Some("First title"));
            assert_eq!(unsafe { crate::state::text(b) }, Some("Other title"));
            0
        }), 0);
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    }

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

    // FrameIn has one size: the whole record (and more) is read, any shorter one (an older layout included) is refused.
    #[test]
    fn frame_in() {
        let ui = create();
        let value = FrameIn::new(320.0, 240.0);
        four_min(ui, ((&value as *const FrameIn).cast(), size_of_val(&value), align_of_val(&value)), &[], &mut |p| {
            let mut out = FrameOut::default();
            unsafe { pxui_frame(ui, p.cast(), None, core::ptr::null_mut(), &mut out) }
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    // PxuiMenuOptions is read whole (its size or more): a shorter record is refused.
    #[test]
    fn menu_options() {
        let ui = create();
        let mut value: PxuiMenuOptions = unsafe { core::mem::zeroed() };
        value.size = size_of::<PxuiMenuOptions>() as u32;
        four_min(ui, ((&value as *const PxuiMenuOptions).cast(), size_of_val(&value), align_of_val(&value)), &[], &mut |p| {
            let p = p.cast::<PxuiMenuOptions>();
            in_frame(ui, &mut |ui| {
                let (mut tag, mut index) = (0u32, 0i64);
                unsafe { pxui_ui_context_menu(ui, 1.0, 1.0, Str::from("m"), pxui_types::ffi::Strided::default(), 0, 0.0, p, &mut tag, &mut index) }
            })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn ui_desc() {
        let ui = create();
        let mut value: PxuiUiDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiUiDesc) = |v| v.abi_built = crate::ABI_VERSION;
        fix(&mut value);
        four_min(ui, ((&value as *const PxuiUiDesc).cast(), size_of_val(&value), align_of_val(&value)), &[], &mut |p| {
            let p = p.cast::<PxuiUiDesc>();
            created(unsafe { pxui_ui_create(p, core::ptr::null(), 0, core::ptr::null_mut()) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn ui_desc_with_fonts() {
        let ui = create();
        let mut value: PxuiUiDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiUiDesc) = |v| v.abi_built = crate::ABI_VERSION;
        fix(&mut value);
        four_min(ui, ((&value as *const PxuiUiDesc).cast(), size_of_val(&value), align_of_val(&value)), &[], &mut |p| {
            let p = p.cast::<PxuiUiDesc>();
            {
        let font = { let mut record = Str::default(); record.ptr = pxui_core::text::PRAGMASEVKA.as_ptr(); record.len = pxui_core::text::PRAGMASEVKA.len(); record };
        created(unsafe { pxui_ui_create(p, &font, 1, core::ptr::null_mut()) })
    }
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
    fn panel() {
        let ui = create();
        let mut value: PxuiPanel = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiPanel) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiPanel).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiPanel>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_panel(ui, &pxui_types::ffi::PxuiKeyedDesc::new("k", ""), p, None, core::ptr::null_mut(), &mut PxuiBox::default()) })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    // U5.4: the value records that carry their own size are read at one size only: a size short of the record is refused before a byte past
    // it is read, a longer one is read as its first sizeof bytes. The look and the state through `pxui_ui_style`, the anchor through
    // `pxui_ui_anchored`, the text input options and the text metrics as out records.
    #[test]
    fn look_state_anchor_options_and_metrics() {
        use pxui_types::ffi::{PxuiAnchorDesc, PxuiLookDesc, PxuiStateDesc, PxuiLabelStyle, PxuiSizeDesc};
        let ui = create();
        let (look, state, anchor) = (PxuiLookDesc::default(), PxuiStateDesc::default(), PxuiAnchorDesc { has_anchor: 1, ..PxuiAnchorDesc::default() });
        four(ui, ((&look as *const PxuiLookDesc).cast(), size_of_val(&look), align_of_val(&look)), true, &mut |p| {
            in_frame(ui, &mut |ui| { let b = a_box(ui); unsafe { pxui_ui_style(ui, &b, p.cast(), &state) } })
        });
        four(ui, ((&look as *const PxuiLookDesc).cast(), size_of_val(&look), align_of_val(&look)), true, &mut |p| {
            in_frame(ui, &mut |ui| { let b = a_box(ui); let mut changed = 0u8; unsafe { pxui_ui_patch_look(ui, &b, p.cast(), &mut changed) } })
        });
        four(ui, ((&state as *const PxuiStateDesc).cast(), size_of_val(&state), align_of_val(&state)), true, &mut |p| {
            in_frame(ui, &mut |ui| { let b = a_box(ui); unsafe { pxui_ui_style(ui, &b, &look, p.cast()) } })
        });
        four(ui, ((&anchor as *const PxuiAnchorDesc).cast(), size_of_val(&anchor), align_of_val(&anchor)), true, &mut |p| {
            let px = PxuiSizeDesc { kind: PxuiSizeDesc::PX, value: 40.0, relative: None };
            in_frame(ui, &mut |ui| unsafe { pxui_ui_anchored(ui, &pxui_types::ffi::PxuiKeyedDesc::new("a", ""), p.cast(), px, px, None, core::ptr::null_mut(), &mut PxuiBox::default()) })
        });
        let options = pxui_types::PxuiTextInputOptions::default();
        four(ui, ((&options as *const pxui_types::PxuiTextInputOptions).cast(), size_of_val(&options), align_of_val(&options)), true, &mut |p| unsafe { pxui_ui_text_input_options(ui, p.cast()) });
        let metrics = pxui_types::TextMetrics::default();
        let (label, text) = (PxuiLabelStyle::from(&pxui_types::Label::BODY), Str::from("x"));
        four(ui, ((&metrics as *const pxui_types::TextMetrics).cast(), size_of_val(&metrics), align_of_val(&metrics)), true, &mut |p| unsafe { pxui_ui_label_measure(ui, text, &label, 0.0, core::ptr::null(), p.cast()) });
        // an anchor with a reserved byte set is refused (the reserved bytes are zero)
        let reserved = PxuiAnchorDesc { reserved: [0, 1], ..anchor };
        let px = PxuiSizeDesc { kind: PxuiSizeDesc::PX, value: 40.0, relative: None };
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_anchored(ui, &pxui_types::ffi::PxuiKeyedDesc::new("a", ""), &reserved, px, px, None, core::ptr::null_mut(), &mut PxuiBox::default()) }), REFUSED);
        unsafe { pxui_ui_destroy(ui); }
    }

    // An array of looks (`states`: six) is spaced by its first element's size: elements of a longer record are read as their prefix at that
    // spacing, a first size short of the record or an element that disagrees with it is refused, and none is read past the array.
    #[test]
    fn six_looks_are_spaced_by_the_first_size() {
        use pxui_types::ffi::{PxuiButtonDesc, PxuiLookDesc};
        let ui = create();
        let k = Str::from("k");
        let full = size_of::<PxuiLookDesc>();
        let wide = full + 8;
        let build = |size: usize, stride: usize, disagree: Option<usize>| {
            let mut bytes = vec![0u8; 6 * stride + 8];
            for i in 0..6 {
                let mut look = PxuiLookDesc::default();
                look.size = if disagree == Some(i) { size as u32 + 8 } else { size as u32 };
                unsafe { core::ptr::copy_nonoverlapping((&look as *const PxuiLookDesc).cast::<u8>(), bytes.as_mut_ptr().add(i * stride), full) };
            }
            bytes
        };
        let press = |bytes: &[u8]| {
            let mut value: PxuiButtonDesc = unsafe { core::mem::zeroed() };
            (value.size, value.enabled, value.key, value.text, value.states) = (size_of::<PxuiButtonDesc>() as u32, 1, k, k, bytes.as_ptr().cast());
            in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_button(ui, &value, None, core::ptr::null_mut(), &mut out) } })
        };
        assert_eq!(press(&build(full, full, None)), 0, "dense: {}", error(ui));
        assert_eq!(press(&build(wide, wide, None)), 0, "a longer record spaced by its size: {}", error(ui));
        assert_eq!(press(&build(wide, full, None)), REFUSED, "elements read at the wrong spacing are not looks");
        assert_eq!(press(&build(full - 1, full, None)), REFUSED, "a first size short of the record");
        assert_eq!(press(&build(full, full, Some(3))), REFUSED, "one element with another size");
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn node_fields_in() {
        let ui = create();
        let mut value: PxuiNodeFields = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiNodeFields) = |f| f.look = pxui_types::ffi::PxuiLookDesc::default();
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
            in_frame(ui, &mut |ui| { let mut open = 0u8; unsafe { pxui_ui_popup(ui, &pxui_types::ffi::PxuiKeyedDesc::new("k", ""), p, None, core::ptr::null_mut(), &mut open, &mut PxuiBox::default()) } })
        });
        unsafe { pxui_ui_destroy(ui); }
    }

    /// The button and icon button records have one size: an earlier layout (96, 104, 150 bytes) is refused, as a `centered` past 1, a nonzero
    /// reserved byte or an invalid curve.
    #[test]
    fn grown_button_records_take_their_old_size() {
        let ui = create();
        let k = Str::from("k");
        let mut button: PxuiButtonDesc = unsafe { core::mem::zeroed() };
        (button.size, button.enabled, button.key, button.text, button.centered) = (size_of::<PxuiButtonDesc>() as u32, 1, k, k, 1);
        let mut icon: PxuiIconButtonDesc = unsafe { core::mem::zeroed() };
        (icon.size, icon.enabled, icon.press_scale, icon.key, icon.name, icon.icon) = (size_of::<PxuiIconButtonDesc>() as u32, 1, 0.96, k, k, k);
        let press = |b: &PxuiButtonDesc| in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_button(ui, b, None, core::ptr::null_mut(), &mut out) } });
        let tap = |b: &PxuiIconButtonDesc| in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_icon_button(ui, b, &mut out) } });
        assert_eq!(press(&button), 0);
        for size in [95, 96, 100, 104, 150, 192] { assert_eq!(press(&PxuiButtonDesc { size, ..button }), PXUI_ERR_ARGUMENT, "an earlier size {size}"); }
        assert_eq!(press(&PxuiButtonDesc { centered: 2, ..button }), PXUI_ERR_ARGUMENT);
        assert_eq!(press(&PxuiButtonDesc { reserved5: [0, 0, 1, 0], ..button }), PXUI_ERR_ARGUMENT);
        // the label, layout and focus fields are checked
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
        // the whole record's leading icon, detail and accessory are checked; an earlier size is refused
        let garbage = { let mut record = Str::default(); record.ptr = b"\xff".as_ptr(); record.len = 1; record };
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
        assert_eq!(tap(&PxuiIconButtonDesc { size: 104, ..icon }), PXUI_ERR_ARGUMENT, "an earlier size");
        for bad in [PxuiIconButtonDesc { checkable: 2, ..icon }, PxuiIconButtonDesc { checked: 1, ..icon }, PxuiIconButtonDesc { checkable: 1, checked: 2, ..icon }, PxuiIconButtonDesc { reserved2: [0, 1, 0, 0], ..icon }] {
            assert_eq!(tap(&bad), PXUI_ERR_ARGUMENT);
        }
        assert_eq!(tap(&icon), 0, "null curves are the kit's springs");
        use pxui_types::Curve;
        let springs = [Curve::Spring { omega: 30.0 }, Curve::Spring { omega: 10.0 }];
        assert_eq!(tap(&PxuiIconButtonDesc { press: springs.as_ptr(), ..icon }), 0);
        let bad_tag = { let mut raw = [0u32; 70]; raw[0] = 9; raw };
        assert_eq!(tap(&PxuiIconButtonDesc { press: bad_tag.as_ptr().cast(), ..icon }), PXUI_ERR_ARGUMENT, "an unknown curve kind is refused before it is read");
        assert_eq!(tap(&PxuiIconButtonDesc { press: springs.as_ptr().cast::<u8>().wrapping_add(1).cast(), ..icon }), PXUI_ERR_ARGUMENT, "a misaligned pointer");
        assert_eq!(tap(&PxuiIconButtonDesc { size: 100, ..icon }), PXUI_ERR_ARGUMENT);
        assert_eq!(tap(&PxuiIconButtonDesc { size: 95, ..icon }), PXUI_ERR_ARGUMENT);
        let still = [Curve::Spring { omega: 30.0 }, Curve::Spring { omega: 0.0 }];
        assert_eq!(tap(&PxuiIconButtonDesc { press: still.as_ptr(), ..icon }), PXUI_ERR_ARGUMENT);
        unsafe { pxui_ui_destroy(ui); }
    }

    /// ABI 1.48: the button calls check their descriptors at every size they have had, and refuse a null or short `PxuiResponse`; a plain
    /// call fills the record with the box it declared.
    #[test]
    fn response_calls() {
        let ui = create();
        let k = Str::from("k");
        let mut value: PxuiButtonDesc = unsafe { core::mem::zeroed() };
        (value.enabled, value.key, value.text) = (1, k, k);
        four_min(ui, ((&value as *const PxuiButtonDesc).cast(), size_of_val(&value), align_of_val(&value)), &[], &mut |p| {
            let p = p.cast::<PxuiButtonDesc>();
            in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_button(ui, p, None, core::ptr::null_mut(), &mut out) } })
        });
        let mut icon: PxuiIconButtonDesc = unsafe { core::mem::zeroed() };
        (icon.enabled, icon.press_scale, icon.key, icon.name, icon.icon) = (1, 1.0, k, k, k);
        four_min(ui, ((&icon as *const PxuiIconButtonDesc).cast(), size_of_val(&icon), align_of_val(&icon)), &[], &mut |p| {
            let p = p.cast::<PxuiIconButtonDesc>();
            in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_icon_button(ui, p, &mut out) } })
        });
        let short = PxuiResponse { size: 8, ..PxuiResponse::EMPTY };
        for out in [core::ptr::null_mut(), &short as *const PxuiResponse as *mut PxuiResponse] {
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_button_plain(ui, k, k, out) }), PXUI_ERR_ARGUMENT);
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_icon_button_plain(ui, k, k, k, out) }), PXUI_ERR_ARGUMENT);
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_button(ui, &value, None, core::ptr::null_mut(), out) }), PXUI_ERR_ARGUMENT);
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_icon_button(ui, &icon, out) }), PXUI_ERR_ARGUMENT);
        }
        let bad = { let mut record = Str::default(); record.ptr = [0xffu8].as_ptr(); record.len = 1; record };
        assert_eq!(in_frame(ui, &mut |ui| { let mut out = PxuiResponse::EMPTY; unsafe { pxui_ui_button_plain(ui, k, bad, &mut out) } }), PXUI_ERR_ARGUMENT);
        let mut plain = (PxuiResponse::EMPTY, PxuiResponse::EMPTY);
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_button_plain(ui, k, k, &mut plain.0) + pxui_ui_icon_button_plain(ui, Str::from("i"), k, k, &mut plain.1) }), 0);
        assert!(plain.0.node.box_key != 0 && plain.1.node.box_key != 0 && plain.0.node.box_key != plain.1.node.box_key, "{plain:?}");
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn plain_icon_button_rejects_invalid_keys_and_text_before_building() {
        use crate::state::{MAX_KEY_BYTES, MAX_LABEL_BYTES};

        let ui = create();
        let long = vec![b'x'; MAX_LABEL_BYTES + 1];
        let (key, icon, name) = (b"icon".as_slice(), b"+".as_slice(), b"Add".as_slice());
        assert_eq!(in_frame(ui, &mut |h| {
            let cases = [
                ("long key", &long[..MAX_KEY_BYTES + 1], icon, name),
                ("unusable key", b"\x010000000000000000", icon, name),
                ("long icon", key, long.as_slice(), name),
                ("long name", key, icon, long.as_slice()),
                ("invalid key UTF-8", b"\xff", icon, name),
                ("invalid icon UTF-8", key, b"\xff", name),
                ("invalid name UTF-8", key, icon, b"\xff"),
            ];
            let nodes = || unsafe { (&*(*h).engine).built_nodes() };
            let before = nodes();
            for (case, key, icon, name) in cases {
                let mut out = PxuiResponse::EMPTY;
                let rc = unsafe { pxui_ui_icon_button_plain(h, str_of(key), str_of(icon), str_of(name), &mut out) };
                assert_eq!(rc, PXUI_ERR_ARGUMENT, "{case}");
                assert_eq!(nodes(), before, "{case}");
            }
            let mut out = PxuiResponse::EMPTY;
            let rc = unsafe { pxui_ui_icon_button_plain(h, str_of(key), str_of(icon), str_of(name), &mut out) };
            assert_eq!(rc, 0, "{}", error(h));
            assert!(nodes() > before);
            assert_ne!(out.node.box_key, 0);
            0
        }), 0);
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    }

    #[test]
    fn panel_rejects_invalid_keys_and_names_before_building() {
        use crate::state::{MAX_KEY_BYTES, MAX_LABEL_BYTES};
        use pxui_types::ffi::PxuiKeyedDesc;

        let ui = create();
        let long = vec![b'x'; MAX_LABEL_BYTES + 1];
        let panel = PxuiPanel { width: 100.0, ..PxuiPanel::default() };
        let (key, name) = (b"panel".as_slice(), b"Options".as_slice());
        assert_eq!(in_frame(ui, &mut |h| {
            let cases = [
                ("long key", &long[..MAX_KEY_BYTES + 1], name),
                ("unusable key", b"\x010000000000000000", name),
                ("long name", key, long.as_slice()),
                ("invalid key UTF-8", b"\xff", name),
                ("invalid name UTF-8", key, b"\xff"),
            ];
            let nodes = || unsafe { (&*(*h).engine).built_nodes() };
            let before = nodes();
            let call = |key, name, out| {
                let keyed = PxuiKeyedDesc { key: str_of(key), name: str_of(name), ..PxuiKeyedDesc::new("", "") };
                unsafe { pxui_ui_panel(h, &keyed, &panel, None, core::ptr::null_mut(), out) }
            };
            for (case, key, name) in cases {
                assert_eq!(call(key, name, &mut PxuiBox::default()), PXUI_ERR_ARGUMENT, "{case}");
                assert_eq!(nodes(), before, "{case}");
            }
            let mut out = PxuiBox::default();
            assert_eq!(call(key, name, &mut out), 0, "{}", error(h));
            assert!(nodes() > before);
            assert_ne!(out.box_key, 0);
            0
        }), 0);
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    }

    /// ABI 1.53: `pxui_ui_dialog` checks its descriptor as the other sized records (size word, text, masks naming a button
    /// that is not there, an index out of range, a bad width) and refuses a null or short `PxuiDialogResult`; a good call answers.
    #[test]
    fn dialog_response_desc() {
        let ui = create();
        let k = Str::from("k");
        let buttons = [k, k];
        let mut value: PxuiDialogDesc = unsafe { core::mem::zeroed() };
        value.size = size_of_val(&value) as u32;
        (value.width, value.key, value.title, value.buttons, value.default_at, value.cancel_at) = (320.0, k, k, Slice { ptr: buttons.as_ptr(), len: 2 }, 1, 0);
        four_min(ui, ((&value as *const PxuiDialogDesc).cast(), size_of_val(&value), align_of_val(&value)), &[], &mut |p| {
            let p = p.cast::<PxuiDialogDesc>();
            in_frame(ui, &mut |ui| { let mut out = PxuiDialogResult::EMPTY; unsafe { pxui_ui_dialog(ui, p, None, core::ptr::null_mut(), &mut out) } })
        });
        let mut out = PxuiDialogResult::EMPTY;
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_dialog(ui, &value, None, core::ptr::null_mut(), &mut out) }), 0, "{}", error(ui));
        assert_eq!((out.has_chosen, out.dismissed), (0, 0), "an open dialog chose nothing");
        let short = PxuiDialogResult { size: 4, ..PxuiDialogResult::EMPTY };
        for bad_out in [core::ptr::null_mut(), &short as *const PxuiDialogResult as *mut PxuiDialogResult] {
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_dialog(ui, &value, None, core::ptr::null_mut(), bad_out) }), PXUI_ERR_ARGUMENT);
        }
        // as many details as buttons (or none); the 88-byte record is refused
        let details = [k, { let mut record = Str::default(); record.ptr = b"\xff".as_ptr(); record.len = 1; record }];
        let mut sink = PxuiDialogResult::EMPTY;
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_dialog(ui, &PxuiDialogDesc { details: Slice { ptr: buttons.as_ptr(), len: 2 }, ..value }, None, core::ptr::null_mut(), &mut sink) }), 0, "{}", error(ui));
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_dialog(ui, &PxuiDialogDesc { size: 88, ..value }, None, core::ptr::null_mut(), &mut sink) }), PXUI_ERR_ARGUMENT);
        for bad in [Slice { ptr: buttons.as_ptr(), len: 1 }, Slice { ptr: details.as_ptr(), len: 2 }, Slice { ptr: core::ptr::null(), len: 2 }] {
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_dialog(ui, &PxuiDialogDesc { details: bad, ..value }, None, core::ptr::null_mut(), &mut sink) }), PXUI_ERR_ARGUMENT);
        }
        let fixes: [fn(&mut PxuiDialogDesc); 7] = [|d| d.default_at = 2, |d| d.cancel_at = -2, |d| d.danger = 4, |d| d.disabled = 1 << 40, |d| d.width = f32::NAN, |d| d.width = -1.0, |d| d.buttons.len = 65];
        for fix in fixes {
            let mut bad = value;
            fix(&mut bad);
            let mut sink = PxuiDialogResult::EMPTY;
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_dialog(ui, &bad, None, core::ptr::null_mut(), &mut sink) }), PXUI_ERR_ARGUMENT);
        }
        unsafe { pxui_ui_destroy(ui); }
    }

    /// ABI 1.67: `pxui_ui_segmented` reads its descriptor at 64, 96 and 104 bytes (the older sizes always carry a choice), refuses a
    /// null ui, descriptor or out, a null or short `PxuiSegmentedResult`, `has_selected` over 1 or a reserved byte set; `has_selected` 0
    /// answers no choice.
    #[test]
    fn segmented_desc() {
        let ui = create();
        let k = Str::from("k");
        let options = [PxuiPickItem { text: k, ..PxuiPickItem::default() }; 2];
        let span = Slice { ptr: options.as_ptr(), len: 2 };
        let mut value: PxuiSegmentedDesc = unsafe { core::mem::zeroed() };
        (value.size, value.enabled, value.key, value.name, value.padding, value.selected) = (size_of_val(&value) as u32, 1, k, k, 8.0, 5);
        four_min(ui, ((&value as *const PxuiSegmentedDesc).cast(), size_of_val(&value), align_of_val(&value)), &[], &mut |p| {
            let p = p.cast::<PxuiSegmentedDesc>();
            in_frame(ui, &mut |ui| { let (mut out, mut result) = (PxuiResponse::EMPTY, PxuiSegmentedResult::EMPTY); unsafe { pxui_ui_segmented(ui, p, span, &mut out, &mut result) } })
        });
        let (mut out, mut result) = (PxuiResponse::EMPTY, PxuiSegmentedResult { has_chosen: 9, has_secondary: 9, reserved: [9; 2], ..PxuiSegmentedResult::EMPTY });
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented(ui, &value, span, &mut out, &mut result) }), 0, "{}", error(ui));
        assert_eq!((result.has_chosen, result.has_secondary, result.reserved), (0, 0, [0; 2]), "has_selected 0 is no choice");
        for size in [96, 104] {
            let old = PxuiSegmentedDesc { size, ..value };
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented(ui, &old, span, &mut out, &mut result) }), PXUI_ERR_ARGUMENT, "an earlier size {size}");
        }
        let fine = PxuiSegmentedDesc { selected: 0, has_selected: 1, focus_on_click: 1, ..value };
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented(ui, &fine, span, &mut out, &mut result) }), 0);
        assert_eq!((result.has_chosen, result.chosen), (1, 0));
        for bad in [PxuiSegmentedDesc { has_selected: 2, ..value }, PxuiSegmentedDesc { reserved3: [0, 0, 1, 0, 0, 0, 0], ..value }, PxuiSegmentedDesc { focus_on_click: 2, ..value }, PxuiSegmentedDesc { reserved4: [0, 1, 0, 0, 0, 0, 0], ..value }] {
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented(ui, &bad, span, &mut out, &mut result) }), PXUI_ERR_ARGUMENT);
        }
        let short = PxuiSegmentedResult { size: 4, ..PxuiSegmentedResult::EMPTY };
        for bad in [core::ptr::null_mut(), &short as *const PxuiSegmentedResult as *mut PxuiSegmentedResult] {
            assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented(ui, &value, span, &mut out, bad) }), PXUI_ERR_ARGUMENT);
        }
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented(ui, core::ptr::null(), span, &mut out, &mut result) }), PXUI_ERR_ARGUMENT);
        assert_eq!(in_frame(ui, &mut |ui| unsafe { pxui_ui_segmented(ui, &value, span, core::ptr::null_mut(), &mut result) }), PXUI_ERR_ARGUMENT);
        assert_eq!(unsafe { pxui_ui_segmented(core::ptr::null_mut(), &value, span, &mut out, &mut result) }, PXUI_ERR_ARGUMENT);
        unsafe { pxui_ui_destroy(ui); }
    }

    #[test]
    fn header_desc() {
        let ui = create();
        let mut value: PxuiHeaderDesc = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiHeaderDesc) = |d| { d.key = Str::from("k"); d.title = d.key; };
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
            in_frame(ui, &mut |ui| { let mut d: PxuiHeaderDesc = unsafe { core::mem::zeroed() }; d.size = core::mem::size_of::<PxuiHeaderDesc>() as u32; d.key = Str::from("k"); d.title = d.key; unsafe { pxui_ui_inspector_header(ui, &d, None, core::ptr::null_mut(), p) } })
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
        unsafe { pxui_ui_value_field(ui, p, None, None, None, core::ptr::null_mut(), Str::default(), &mut text, &mut changed, &mut editing, &mut { pxui_types::ffi::PxuiResponse::EMPTY }) }
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
        let fix: fn(&mut PxuiKeyframeDesc) = |k| { k.target = 1; k.values[0] = 1.0; k.look = pxui_types::ffi::PxuiLookDesc::default(); k.curve = PxuiCurveDesc { kind: PxuiCurveDesc::SPRING, omega: 20.0, ..PxuiCurveDesc::default() }; };
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
            in_frame(ui, &mut |ui| { let (mut changed, mut ratio) = (0u8, 0.5f32); unsafe { pxui_ui_split(ui, p, &mut { pxui_types::ffi::PxuiResponse::EMPTY }, &mut changed, &mut ratio) } })
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
            in_frame(ui, &mut |ui| { let mut out = PxuiMenuOut::default(); out.size = core::mem::size_of::<PxuiMenuOut>() as u32; unsafe { pxui_ui_menu_bar(ui, p, &mut out, &mut 0) } })
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
            in_frame(ui, &mut |ui| { let mut d: PxuiMenuBarDesc = unsafe { core::mem::zeroed() }; d.size = core::mem::size_of::<PxuiMenuBarDesc>() as u32; unsafe { pxui_ui_menu_bar(ui, &d, p, &mut 0) } })
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
    fn scrub_out() {
        let ui = create();
        let mut value: PxuiScrubOut = unsafe { core::mem::zeroed() };
        let fix: fn(&mut PxuiScrubOut) = |_| {};
        fix(&mut value);
        four(ui, ((&value as *const PxuiScrubOut).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<PxuiScrubOut>();
            in_frame(ui, &mut |ui| unsafe { pxui_ui_scrub(ui, Str::default(), 0.0, 0.0, 1.0, 0.1, &mut { PxuiResponse::EMPTY }, p) })
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
        unsafe { crate::text::text_width(ui, 0, Str::default(), p, 100.0, &mut out) }
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
            unsafe { crate::text::text_prewarm(ui, Str::from("Song"), p.cast::<pxui_types::ffi::PxuiTextStyle>(), 0.0) }
        });
        let (style, title) = (pxui_types::ffi::PxuiTextStyle::default(), Str::from("Song title"));
        let long = vec![b'a'; pxui_core::text::QUEUE_BYTES + 1];
        unsafe {
            let worker = pxui_text_worker_create();
            assert!(!worker.is_null());
            for attached in [core::ptr::null(), worker.cast_const()] {
                assert_eq!(pxui_ui_set_text_worker(ui, attached), 0);
                assert_eq!(pxui_ui_set_text_worker(other, attached), 0);
                assert_eq!(crate::text::text_prewarm(core::ptr::null_mut(), title, &style, 0.0), PXUI_ERR_ARGUMENT);
                assert_eq!(crate::text::text_prewarm(ui, title, core::ptr::null(), 0.0), PXUI_ERR_ARGUMENT);
                assert_eq!(crate::text::text_prewarm(ui, title, &style, f32::NAN), PXUI_ERR_ARGUMENT);
                assert_eq!(crate::text::text_prewarm(ui, title, &style, -1.0), PXUI_ERR_ARGUMENT);
                assert_eq!(crate::text::text_prewarm(ui, { let mut record = Str::default(); record.ptr = b"\xff".as_ptr(); record.len = 1; record }, &style, 0.0), PXUI_ERR_ARGUMENT);
                assert_eq!(crate::text::text_prewarm(ui, { let mut record = Str::default(); record.ptr = long.as_ptr(); record.len = long.len(); record }, &style, 0.0), PXUI_ERR_ARGUMENT);
                assert_eq!(crate::text::text_prewarm(ui, title, &style, 200.0), 0);
                assert_eq!(crate::text::text_prewarm(other, title, &style, 0.0), 0);
            }
            assert_eq!(pxui_ui_set_text_worker(core::ptr::null_mut(), worker), PXUI_ERR_ARGUMENT);
            pxui_text_worker_destroy(worker);
            pxui_text_worker_destroy(core::ptr::null_mut());
            assert_eq!(crate::text::text_prewarm(ui, title, &style, 100.0), 0);
            assert_eq!(pxui_ui_set_text_worker(other, core::ptr::null()), 0);
            pxui_ui_destroy(ui);
            pxui_ui_destroy(other);
        }
    }

    #[test]
    fn text_spans() {
        let ui = create();
        let value = pxui_types::TextSpans::default();
        four(ui, ((&value as *const pxui_types::TextSpans).cast(), size_of_val(&value), align_of_val(&value)), true, &mut |p| {
            let p = p.cast::<pxui_types::TextSpans>();
            {
        let (style, mut out) = (pxui_types::ffi::PxuiTextStyle::default(), pxui_types::TextMetrics::default());
        unsafe { crate::text::text_width_spans(ui, 0, Str::default(), &style, 100.0, p, &mut out) }
    }
        });
        unsafe { pxui_ui_destroy(ui); }
    }
}

// ABI 1.60: the version an app was built against, recorded sizes of grown input records and `pxui_supports`.
#[test]
fn the_library_checks_the_version_the_app_was_built_against_once_at_create() {
    assert_eq!(pxui_types::ABI_BUILT, ABI_VERSION, "bump ABI_BUILT with ABI_VERSION");
    let major = ABI_VERSION >> 16;
    // the status says why: 0 served, PXUI_ERR_VERSION for another major or a newer minor
    let make = |built: u32| { let mut status = 1; let ui = unsafe { pxui_ui_create(&PxuiUiDesc { abi_built: built, ..PxuiUiDesc::default() }, core::ptr::null(), 0, &mut status) }; assert_eq!(!ui.is_null(), status == 0); if !ui.is_null() { unsafe { pxui_ui_destroy(ui); } } status };
    assert!([make(ABI_VERSION), make(major << 16)].iter().all(|status| *status == 0), "the same minor and the first");
    // the unstated version is refused: the app must say what it was built for (2.0 baseline)
    assert_eq!(make(0), crate::PXUI_ERR_VERSION, "abi_built 0");
    // a 1.x app (any 1.minor) is refused cleanly at create: nothing is created and the status says why
    assert_eq!([make(1 << 16), make(1 << 16 | 57), make(1 << 16 | 155)], [crate::PXUI_ERR_VERSION; 3], "1.x apps");
    // the minimum-library rule: an app built for a newer minor may write or read records grown past this library's, so it is refused
    assert_eq!([make(ABI_VERSION + 1), make(major << 16 | 0xFFFF)], [crate::PXUI_ERR_VERSION; 2], "an app built for a newer minor than the library");
    assert_eq!([make((major + 1) << 16), make((major - 1) << 16 | 1), make(u32::MAX)], [crate::PXUI_ERR_VERSION; 3], "another major");
    let mut status = 0;
    assert!(unsafe { pxui_ui_create(core::ptr::null(), core::ptr::null(), 0, &mut status) }.is_null() && status == PXUI_ERR_ARGUMENT, "no record");
    // a record that does not cover abi_built (the 12-byte 1.x record included) is refused, whatever it says
    let rec = |size: u32| [size.to_ne_bytes(), 1u32.to_ne_bytes(), 13u32.to_ne_bytes(), ABI_VERSION.to_ne_bytes()].concat();
    for size in 0..16u32 {
        let bytes = rec(size);
        let ui = unsafe { pxui_ui_create(bytes.as_ptr().cast(), core::ptr::null(), 0, core::ptr::null_mut()) };
        assert!(ui.is_null(), "a {size}-byte record");
        if !ui.is_null() { unsafe { pxui_ui_destroy(ui); } }
    }
}

#[test]
fn supports_answers_from_the_feature_registry_of_this_build() {
    use pxui_types::feature as f;
    let has = |id: u32| unsafe { pxui_supports(id) };
    assert_eq!((has(0), has(5), has(30), has(u32::MAX), has(ABI_VERSION)), (0, 0, 0, 0, 0), "no id 0, none past the registry, versions are not ids");
    assert_eq!((has(f::A11Y_BRIDGE) == 1, has(f::METAL) == 1, has(f::VULKAN) == 1), (cfg!(target_os = "linux"), cfg!(any(target_os = "macos", target_os = "ios")), cfg!(any(windows, target_os = "linux"))));
    assert_eq!(has(f::EXTENSION_SET), 1);
    // pxui.h numbers the same ids
    let header = include_str!("../../../../include/pxui.h");
    // pxui.h names the version the app is built for
    let built: u32 = header.split("#define PXUI_ABI_BUILT ((2u << 16) | ").nth(1).and_then(|rest| rest.split('u').next()).and_then(|minor| minor.parse().ok()).expect("PXUI_ABI_BUILT in pxui.h");
    assert_eq!(2 << 16 | built, ABI_VERSION, "pxui.h PXUI_ABI_BUILT");
    for (name, id) in [("A11Y_BRIDGE", f::A11Y_BRIDGE), ("METAL", f::METAL), ("VULKAN", f::VULKAN), ("EXTENSION_SET", f::EXTENSION_SET)] {
        assert!(header.contains(&format!("PXUI_FEATURE_{name} = {id}")), "{name}");
    }
}

// The toggle and segmented records have one size: an earlier layout is refused, and the whole record's grown fields are checked.
#[test]
fn earlier_toggle_and_segmented_sizes_are_refused() {
    use pxui_types::ffi::{PxuiResponse, PxuiSegmentedDesc, PxuiToggleDesc, Slice};
    let ui = create();
    let k = Str::from("k");
    let build = |f: &mut dyn FnMut(*mut PxuiUi) -> i32| { extern "C" fn b(user: *mut c_void, ui: *mut c_void) { let (f, rc) = unsafe { &mut *user.cast::<(&mut dyn FnMut(*mut PxuiUi) -> i32, i32)>() }; *rc = f(ui.cast()); }
        let mut st = (f, i32::MIN); assert_eq!(run(ui, Some(b), (&mut st as *mut (&mut dyn FnMut(*mut PxuiUi) -> i32, i32)).cast()), 0); st.1 };
    let mut toggle: PxuiToggleDesc = unsafe { core::mem::zeroed() };
    (toggle.size, toggle.enabled, toggle.key, toggle.text) = (size_of::<PxuiToggleDesc>() as u32, 1, k, k);
    let mut response = PxuiResponse::EMPTY;
    assert_eq!(build(&mut |ui| unsafe { pxui_ui_toggle(ui, &toggle, &mut response) }), 0);
    assert_eq!(build(&mut |ui| unsafe { pxui_ui_toggle(ui, &PxuiToggleDesc { size: 88, ..toggle }, &mut response) }), PXUI_ERR_ARGUMENT, "the 88-byte record");
    assert_eq!(build(&mut |ui| unsafe { pxui_ui_toggle(ui, &PxuiToggleDesc { has_style: 7, ..toggle }, &mut response) }), PXUI_ERR_ARGUMENT);
    let mut seg: PxuiSegmentedDesc = unsafe { core::mem::zeroed() };
    (seg.size, seg.enabled, seg.key, seg.name, seg.has_selected) = (size_of::<PxuiSegmentedDesc>() as u32, 1, k, k, 1);
    let options = [pxui_types::ffi::PxuiPickItem { text: k, ..Default::default() }; 2];
    let mut result = pxui_types::ffi::PxuiSegmentedResult::EMPTY;
    let names = [k];
    let bad = Slice { ptr: names.as_ptr(), len: 1 };
    let call = |d: &PxuiSegmentedDesc, build: &dyn Fn(&mut dyn FnMut(*mut PxuiUi) -> i32) -> i32, response: &mut PxuiResponse, result: &mut pxui_types::ffi::PxuiSegmentedResult| build(&mut |ui| unsafe { pxui_ui_segmented(ui, d, Slice { ptr: options.as_ptr(), len: 2 }, response, result) });
    assert_eq!(call(&seg, &build, &mut response, &mut result), 0);
    for size in [64, 96, 104] { assert_eq!(call(&PxuiSegmentedDesc { size, ..seg }, &build, &mut response, &mut result), PXUI_ERR_ARGUMENT, "an earlier size {size}"); }
    assert_eq!(call(&PxuiSegmentedDesc { names: bad, ..seg }, &build, &mut response, &mut result), PXUI_ERR_ARGUMENT);
    unsafe { pxui_ui_destroy(ui); }
}

#[test]
fn fonts_and_renderer_records_are_validated() {
    let ui = create();
    let font = |bytes: &[u8]| { let mut record = Str::default(); record.ptr = bytes.as_ptr(); record.len = bytes.len(); record };
    unsafe {
        assert_eq!(pxui_text_add_font(ui, font(b"not a font")), u32::MAX);
        assert_eq!(pxui_text_add_font(ui, { let mut record = Str::default(); record.ptr = core::ptr::null(); record.len = 0; record }), u32::MAX);
        assert_eq!(pxui_text_add_font(ui, { let mut record = Str::default(); record.ptr = b"x".as_ptr(); record.len = pxui_text::MAX_FONT_BYTES + 1; record }), u32::MAX);
        assert_ne!(pxui_text_add_font(ui, font(pxui_text::PRAGMASEVKA)), u32::MAX);
        let garbage = [font(b"not a font")];
        assert!(pxui_ui_create(&PxuiUiDesc::default(), garbage.as_ptr(), 1, core::ptr::null_mut()).is_null());
        let two = [font(pxui_text::PRAGMASEVKA), font(pxui_text::PRAGMASEVKA)];
        let made = pxui_ui_create(&PxuiUiDesc::default(), two.as_ptr(), 2, core::ptr::null_mut());
        assert!(!made.is_null());
        assert_eq!(pxui_ui_destroy(made), 0);
        // a gpu or target record shorter than its type is refused before it is read
        let short = 4u32;
        let mut renderer = core::ptr::null_mut();
        assert_ne!(pxui_renderer_create(ui, (&short as *const u32).cast(), &PxuiTargetDesc::default(), &mut renderer), 0);
        assert_ne!(pxui_renderer_create(ui, &pxui_types::Gpu::default(), (&short as *const u32).cast(), &mut renderer), 0);
        let unknown=PxuiTargetDesc { format:1, reserved_tail:!PxuiTargetDesc::BACKDROP_UI_DAMAGE, ..PxuiTargetDesc::default() };
        assert_eq!(pxui_renderer_create(ui, &pxui_types::Gpu::default(), &unknown, &mut renderer), PXUI_ERR_ARGUMENT);
        assert_eq!(error(ui), "invalid target descriptor");
        assert!(renderer.is_null());
        assert_eq!(pxui_ui_destroy(ui), 0);
    }
}

#[test]
fn every_text_entry_refuses_text_over_the_cap() {
    let ui = create();
    let over = { let mut record = Str::default(); record.ptr = b"x".as_ptr(); record.len = crate::state::MAX_TEXT_BYTES + 1; record };
    let style = pxui_types::ffi::PxuiTextStyle::default();
    let mut out = pxui_types::TextMetrics::default();
    unsafe {
        let font = pxui_default_font();
        let shaper = pxui_shaper_create(&font, 1);
        assert!(!shaper.is_null());
        assert_eq!(pxui_shaper_measure(shaper, 0, over, &style, 100.0, core::ptr::null(), &mut out), PXUI_ERR_ARGUMENT);
        assert_eq!(crate::text::text_width(ui, 0, over, &style, 100.0, &mut out), PXUI_ERR_ARGUMENT);
        let spans = pxui_types::TextSpans::default();
        assert_eq!(pxui_shaper_measure(shaper, 0, over, &style, 100.0, &spans, &mut out), PXUI_ERR_ARGUMENT);
        assert_eq!(crate::text::text_width_spans(ui, 0, over, &style, 100.0, &spans, &mut out), PXUI_ERR_ARGUMENT);
        assert_eq!(crate::text::text_prewarm(ui, over, &style, 100.0), PXUI_ERR_ARGUMENT);
        let (mut w, mut h) = (0u32, 0u32);
        assert_eq!(pxui_shaper_rasterize_rgba(shaper, 0, over, &style, 100.0, 1.0, 0, 0, 0, 255, &mut w, &mut h, core::ptr::null_mut(), 0), usize::MAX);
        assert_eq!(crate::text::text_rasterize_rgba(ui, 0, over, &style, 100.0, 1.0, 0, 0, 0, 255, &mut w, &mut h, core::ptr::null_mut(), 0), usize::MAX);
        assert_eq!(crate::text::text_quads(ui, 0, over, &style, 100.0, 0.0, 0.0, 1.0, core::ptr::null_mut(), 0), usize::MAX);
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
    let value = Str::from("Song title");
    let label = PxuiLabelStyle::from(&Label::BODY.size(15.0).bold());
    let mut resolved = PxuiTextStyle::default();
    assert_eq!(unsafe { crate::ui::pxui_ui_resolve_label(ui, &label, &mut resolved) }, 0);
    let (mut a, mut b) = (TextMetrics::default(), TextMetrics::default());
    assert_eq!(unsafe { pxui_ui_label_measure(ui, value, &label, 0.0, core::ptr::null(), &mut a) }, 0);
    assert_eq!(unsafe { crate::text::text_width(ui, 0, value, &resolved, 0.0, &mut b) }, 0);
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
        assert_eq!(pxui_ui_text(ui, core::ptr::null(), value, core::ptr::null(), core::ptr::null(), 0), PXUI_ERR_ARGUMENT);
        assert_eq!(pxui_ui_text(core::ptr::null_mut(), core::ptr::null(), value, &label, core::ptr::null(), 1), PXUI_ERR_ARGUMENT);
        pxui_ui_destroy(ui);
    }
}

/// ABI 1.108: `pxui_ui_style` takes the one look the state picks, checks it and applies it; an invalid look or a null one is refused
/// and leaves the node's look as it was.
#[test]
fn style_applies_one_checked_look() {
    use pxui_types::ffi::{PxuiLookDesc, PxuiStateDesc};
    let ui = create();
    let mut good = PxuiLookDesc::default();
    good.opacity = 0.5;
    let mut bad = good;
    bad.opacity = f32::NAN;
    let state = PxuiStateDesc::default();
    let rcs = in_frame(ui, &mut |h| {
        let mut b = PxuiBox::default();
        assert_eq!(unsafe { pxui_ui_node_keyed(h, 7, &pxui_types::Node::DEFAULT, &mut b) }, 0);
        let applied = unsafe { pxui_ui_style(h, &b, &good, &state) };
        let refused = unsafe { pxui_ui_style(h, &b, &bad, &state) };
        let null = unsafe { pxui_ui_style(h, &b, core::ptr::null(), &state) };
        let opacity = unsafe { (*(*h).engine).node_mut(&crate::state::core_box_of(h, b).unwrap()).look.opacity };
        (applied == 0 && refused == PXUI_ERR_ARGUMENT && null == PXUI_ERR_ARGUMENT && opacity == 0.5) as i32
    });
    assert_eq!(rcs, 1, "{}", error(ui));
    unsafe { pxui_ui_destroy(ui) };
}
