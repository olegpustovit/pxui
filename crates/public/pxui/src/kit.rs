use crate::Handle;
use crate::{composite::{defer_callback_panic, deferred}, check, Ui};
use core::{ffi::c_void, mem::ManuallyDrop, panic::AssertUnwindSafe, ptr::NonNull};
use pxui_types::{ui::Target, Axis, Look, Str, ffi::{PxuiCurveDesc, PxuiFontDesc, PxuiLookDesc, PxuiRingDesc, PxuiShadowDesc, Slice}};
use pxui_types::ui::{Key, SectionList, SectionPart};

#[cfg_attr(feature = "static", allow(unused_imports))]
pub(crate) use pxui_types::ffi::{PxuiFooterDesc as FooterDesc, PxuiGridListDesc as GridListDesc, PxuiHintDesc as HintDesc, PxuiInspectorRowDesc as InspectorRowDesc, PxuiInspectorSectionDesc as InspectorSectionDesc, PxuiInspectorTextDesc as InspectorTextDesc, PxuiInspectorValueDesc as InspectorValueDesc, PxuiKeyframeDesc as KeyframeDesc, PxuiMenuBarDesc as MenuBarDesc, PxuiMenuDesc as MenuDesc, PxuiMenuOut as MenuOut, PxuiSplitDesc as SplitDesc};

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{pxui_ui_animate_keys, pxui_ui_animate_keys_repeat, pxui_ui_menu_bar_result, pxui_ui_wants_frame};
}
#[cfg(not(feature = "static"))]
mod sys {
    use super::{FooterDesc, GridListDesc, InspectorRowDesc, InspectorSectionDesc, InspectorTextDesc, InspectorValueDesc, KeyframeDesc, MenuBarDesc, MenuOut, SplitDesc};
    pub use crate::sys::PxuiUi;
    use pxui_types::{PxuiBox, Str, ffi::Slice};
    #[link(name = "pxui")]
    unsafe extern "C" {
    pub fn pxui_ui_section_list(ui:*mut PxuiUi,desc:*const pxui_types::ffi::PxuiSectionListDesc,out:*mut PxuiBox)->i32;
    pub fn pxui_ui_section_list_measured(ui:*mut PxuiUi,desc:*const pxui_types::ffi::PxuiMeasuredSectionDesc,out:*mut PxuiBox)->i32;
    pub fn pxui_ui_grid_list(ui: *mut PxuiUi, desc: *const GridListDesc, out: *mut PxuiBox) -> i32;
    pub fn pxui_ui_split_result(ui: *mut PxuiUi, desc: *const SplitDesc, out: *mut pxui_types::ffi::PxuiResponse, has_ratio: *mut u8, ratio: *mut f32) -> i32;
    pub fn pxui_ui_menu_bar_result(ui: *mut PxuiUi, desc: *const MenuBarDesc, out: *mut MenuOut, dismissed: *mut u8) -> i32;
    pub fn pxui_ui_inspector_row(ui: *mut PxuiUi, desc: *mut InspectorRowDesc) -> i32;
    pub fn pxui_ui_inspector_section(ui: *mut PxuiUi, desc: *mut InspectorSectionDesc) -> i32;
    pub fn pxui_ui_inspector_toggle(ui: *mut PxuiUi, key: Str, label: Str, value: u8, out: *mut u8) -> i32;
    pub fn pxui_ui_inspector_button(ui: *mut PxuiUi, key: Str, label: Str, out: *mut u8) -> i32;
    pub fn pxui_ui_inspector_readout(ui: *mut PxuiUi, desc: *const InspectorValueDesc) -> i32;
    pub fn pxui_ui_inspector_message(ui: *mut PxuiUi, desc: *const InspectorTextDesc) -> i32;
    pub fn pxui_ui_wants_frame(ui: *mut PxuiUi, events: Slice<pxui_types::Event>, now: f64, out: *mut u8) -> i32;
    pub fn pxui_ui_footer(ui: *mut PxuiUi, desc: *const FooterDesc) -> i32;
    /// The C entry `Ui::animate_keys` calls: plays the 1 to 8 keyframes `keys` on `box_` once and writes 1 to `out` while the run is going; an error code for an invalid keyframe or a stale box.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ui::{Curve, Keyframe};
    /// pxui::doc::frames(2, &mut |ui| {
    ///     let b = ui.node("n", pxui::Node::DEFAULT);
    ///     ui.animate_keys(&b, &[Keyframe::offset(0.0, 0.0, 8.0).curve(Curve::Spring { omega: 10.0 })], &pxui::Keyframes::DEFAULT);
    /// });
    /// ```
    pub fn pxui_ui_animate_keys(ui: *mut PxuiUi, box_: *const PxuiBox, keys: Slice<KeyframeDesc>, out: *mut u8) -> i32;
    /// The C entry `Ui::animate_keys` calls for a repeated, reversed or offset run: plays `keys` (1 to 8) on `box_` `repeat` times (0 is forever; a count includes the reverse leg), reversing every other cycle when `autoreverse` is non-zero, starting `start_offset` seconds in; writes 1 to `out` while it runs.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// use pxui::ui::{Curve, Keyframe};
    /// pxui::doc::frames(2, &mut |ui| {
    ///     let b = ui.node("n", pxui::Node::DEFAULT);
    ///     let curve = Curve::Bezier { duration: 0.4, points: [0.0, 0.0, 1.0, 1.0] };
    ///     let keys = [Keyframe::opacity(0.0, 1.0).curve(curve), Keyframe::opacity(0.4, 0.5).curve(curve)];
    ///     ui.animate_keys(&b, &keys, &pxui::Keyframes::DEFAULT.repeat(pxui::Repeat::Count(2)));
    /// });
    /// ```
    pub fn pxui_ui_animate_keys_repeat(ui: *mut PxuiUi, box_: *const PxuiBox, keys: Slice<KeyframeDesc>, repeat: u32, autoreverse: u8, start_offset: f32, out: *mut u8) -> i32;
    }
}

#[inline]
fn as_str(value: &str) -> Str { crate::as_str(value) }

type Build = pxui_types::ffi::UiBuild;
type GridBuild = Option<extern "C" fn(*mut c_void, *mut c_void, usize)>;

// Each context hands out its trampoline and its user pointer from one method, so the two cannot name different types.
struct Callback<F> { f: F }
impl<F: FnMut(&mut Ui)> Callback<F> {
    fn erased(&mut self) -> (Build, *mut c_void) { (Some(call::<F>), (self as *mut Self).cast()) }
}
struct GridCallback<F> { f: F }
impl<F: FnMut(&mut Ui, usize)> GridCallback<F> {
    fn erased(&mut self) -> (GridBuild, *mut c_void) { (Some(call_grid::<F>), (self as *mut Self).cast()) }
}
extern "C" fn call<F: FnMut(&mut Ui)>(user: *mut c_void, raw: *mut c_void) {
    if raw.is_null() || deferred() { return; }
    let state = unsafe { &mut *user.cast::<Callback<F>>() };
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut ui = ManuallyDrop::new(Ui(unsafe { NonNull::new_unchecked(raw.cast()) }));
        (state.f)(&mut ui);
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}
extern "C" fn call_grid<F: FnMut(&mut Ui, usize)>(user: *mut c_void, raw: *mut c_void, index: usize) {
    if raw.is_null() || deferred() { return; }
    let state = unsafe { &mut *user.cast::<GridCallback<F>>() };
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut ui = ManuallyDrop::new(Ui(unsafe { NonNull::new_unchecked(raw.cast()) }));
        (state.f)(&mut ui, index);
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}
/// The context of a keyed section list crossing the C ABI: the row key and the part builder (the fixed form passes Y 0).
#[cfg(not(feature = "static"))]
struct KeyedSection<'a> { key: &'a dyn Fn(usize) -> i64, build: &'a mut dyn FnMut(&mut Ui, usize, f32) }
#[cfg(not(feature = "static"))]
extern "C" fn call_row_key(user: *mut c_void, index: usize) -> i64 {
    if deferred() { return index as i64; }
    let state = unsafe { &*user.cast::<KeyedSection>() };
    match std::panic::catch_unwind(AssertUnwindSafe(|| (state.key)(index))) {
        Ok(id) => id,
        Err(panic) => { defer_callback_panic(panic); index as i64 }
    }
}
#[cfg(not(feature = "static"))]
extern "C" fn call_keyed_grid(user: *mut c_void, raw: *mut c_void, index: usize) { call_keyed_measured(user, raw, index, 0.0) }
#[cfg(not(feature = "static"))]
extern "C" fn call_keyed_measured(user: *mut c_void, raw: *mut c_void, index: usize, y: f32) {
    if raw.is_null() || deferred() { return; }
    let state = unsafe { &mut *user.cast::<KeyedSection>() };
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut ui = ManuallyDrop::new(Ui(unsafe { NonNull::new_unchecked(raw.cast()) }));
        (state.build)(&mut ui, index, y);
    }));
    if let Err(panic) = result { defer_callback_panic(panic); }
}
fn encode_look(value: Look) -> PxuiLookDesc {
    let shadow = |s: pxui_types::Shadow| PxuiShadowDesc { color: s.color, offset: s.offset, blur: s.blur, spread: s.spread, inner: s.inner as u8, reserved: [0; 3] };
    PxuiLookDesc {
        fill: value.fill, gradient: value.gradient.get().unwrap_or_default(), has_gradient: value.gradient.is_some() as u8, reserved: [0; 3], angle: value.angle,
        border: value.border, border_w: value.border_w, sides: value.sides as u32, radius: value.radius,
        shadows: [shadow(value.shadows[0]), shadow(value.shadows[1])], text: value.text,
        font: PxuiFontDesc { family: value.font.family, weight: value.font.weight, size_points: value.font.size_points, size: value.font.size, tracking: value.font.tracking, caps: value.font.caps as u8, reserved: [0; 3] },
        opacity: value.opacity, ring: PxuiRingDesc { color: value.ring.color, width: value.ring.width, sides: value.ring.sides as u32 },
        selection: value.selection, caret: value.caret, dashed: value.dashed as u8, reserved_tail: [0; 3],
    }
}

use pxui_types::ui::{Keyframe, Keyframes, Repeat};

const HINT_NONE: HintDesc = HintDesc { size: core::mem::size_of::<HintDesc>() as u32, reserved: 0, key: Str { ptr: core::ptr::null(), len: 0 }, value: Str { ptr: core::ptr::null(), len: 0 } };

/// Hint rows as descriptors for one call: up to 16 on the stack, more on the heap.
pub(crate) struct Hints {
    stack: [HintDesc; 16],
    heap: Vec<HintDesc>,
    len: usize,
}
impl Hints {
    pub(crate) fn new(hints: &[(&str, &str)]) -> Hints {
        let mut out = Hints { stack: [HINT_NONE; 16], heap: Vec::new(), len: hints.len() };
        let values: &mut [HintDesc] = if hints.len() <= 16 { &mut out.stack[..hints.len()] } else { out.heap = vec![HINT_NONE; hints.len()]; &mut out.heap[..] };
        for (out, (key, value)) in values.iter_mut().zip(hints) { *out = HintDesc { size: core::mem::size_of::<HintDesc>() as u32, reserved: 0, key: as_str(key), value: as_str(value) }; }
        out
    }
    pub(crate) fn span(&self) -> Slice<HintDesc> { Slice { ptr: if self.len <= 16 { self.stack.as_ptr() } else { self.heap.as_ptr() }, len: self.len } }
}

/// The layout and minimum pane sizes used by [Ui::split].
#[derive(Clone, Copy, Debug)]
pub struct Split {
    axis: Axis,
    min_a: f32,
    min_b: f32,
}

impl Split {
    /// Side-by-side panes with no minimum size.
    pub const DEFAULT: Split = Split { axis: Axis::Row, min_a: 0.0, min_b: 0.0 };

    /// Chooses side-by-side or vertically stacked panes.
    ///
    /// The split validates its layout before either pane is built.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Split = pxui::Split::DEFAULT.axis(pxui::ui::Axis::Column);
    /// ```
    pub const fn axis(mut self, axis: Axis) -> Split { self.axis = axis; self }

    /// Sets the first pane's minimum length in logical points.
    ///
    /// The split validates its layout before either pane is built.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Split = pxui::Split::DEFAULT.min_a(120.0);
    /// ```
    pub const fn min_a(mut self, points: f32) -> Split { self.min_a = points; self }

    /// Sets the second pane's minimum length in logical points.
    ///
    /// The split validates its layout before either pane is built.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const STYLE: pxui::Split = pxui::Split::DEFAULT.min_b(200.0);
    /// ```
    pub const fn min_b(mut self, points: f32) -> Split { self.min_b = points; self }
}

/// Visual options for the registered menu-bar body.
#[derive(Clone, Copy, Debug)]
pub struct MenuBar {
    variant: u16,
}

impl MenuBar {
    /// The theme's default body variant.
    pub const DEFAULT: MenuBar = MenuBar { variant: 0 };

    /// Selects the variant the menu-bar body receives.
    ///
    /// Zero uses the theme's variant.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const BAR: pxui::MenuBar = pxui::MenuBar::DEFAULT.variant(2);
    /// ```
    pub const fn variant(mut self, variant: u16) -> MenuBar { self.variant = variant; self }
}

/// One named menu and its ordered items in a menu bar.
#[derive(Clone, Copy, Debug)]
pub struct MenuBarMenu<'a> {
    title: &'a str,
    items: &'a [crate::MenuItem<'a>],
}

impl<'a> MenuBarMenu<'a> {
    /// Gives one menu its title and borrowed items.
    ///
    /// Use explicit item ids to identify actions across menus.
    /// Allocates: never.
    /// Panics: never.
    ///
    /// ```
    /// const FILE: pxui::MenuBarMenu = pxui::MenuBarMenu::new("File", &[pxui::MenuItem::new("Open").id(1)]);
    /// ```
    pub const fn new(title: &'a str, items: &'a [crate::MenuItem<'a>]) -> MenuBarMenu<'a> { MenuBarMenu { title, items } }
}

impl Ui {
    /// The one body of `section_list` (and of `list`): keyed by `key`, measured or fixed rows as `section` says; `build` is given the part's
    /// logical content Y (fixed rows: the lead-in and heading heights and the rows above it).
    #[track_caller]
    pub(crate) fn section_list_key(&mut self, key: Key, section: &SectionList, id: &dyn Fn(usize) -> i64, build: &mut dyn FnMut(&mut Ui, SectionPart, f32)) -> Handle {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let part = |index| match index { usize::MAX => SectionPart::Leading, index if index == usize::MAX - 1 => SectionPart::Header, index => SectionPart::Row(index) };
        // a fixed row's Y is known without a layout: the C fixed form passes none
        #[cfg(not(feature = "static"))]
        let fixed_y = |index: usize| match part(index) { SectionPart::Leading => 0.0, SectionPart::Header => section.leading_height, SectionPart::Row(i) => section.leading_height + section.header_height + i as f32 * section.row_height };
        #[cfg(not(feature = "static"))]
        let measured = section.measured;
        let mut out = Handle::default();
        #[cfg(feature = "static")]
        {
            let mut invoke = |raw: *mut crate::sys::PxuiUi, index, y| { let mut ui = ManuallyDrop::new(unsafe { Ui(NonNull::new_unchecked(raw)) }); build(&mut ui, part(index), y) };
            check(self, unsafe { pxui_abi_link::trusted::section_list(self.0.as_ptr(), label, section, Some(id), &mut invoke, &mut out) });
        }
        #[cfg(not(feature = "static"))]
        {
            let mut invoke = |ui: &mut Ui, index, y: f32| build(ui, part(index), if measured { y } else { fixed_y(index) });
            let mut context = KeyedSection { key: id, build: &mut invoke };
            let user = (&mut context as *mut KeyedSection).cast();
            let (overlay_scrollbar, label, count) = (u8::from(section.overlay_scrollbar), as_str(label), section.count);
            let (row_height, leading_height, header_height, trailing_height) = (section.row_height, section.leading_height, section.header_height, section.trailing_height);
            check(self, unsafe { if measured {
                let desc = pxui_types::ffi::PxuiMeasuredSectionDesc { size: core::mem::size_of::<pxui_types::ffi::PxuiMeasuredSectionDesc>() as u32, overlay_scrollbar, reserved: [0; 3], label, count, row_height, leading_height, header_height, trailing_height, build: Some(call_keyed_measured), user, key: Some(call_row_key) };
                sys::pxui_ui_section_list_measured(self.0.as_ptr(), &desc, &mut out)
            } else {
                let desc = pxui_types::ffi::PxuiSectionListDesc { size: core::mem::size_of::<pxui_types::ffi::PxuiSectionListDesc>() as u32, overlay_scrollbar, reserved: [0; 3], label, count, row_height, leading_height, header_height, trailing_height, build: Some(call_keyed_grid), user, key: Some(call_row_key) };
                sys::pxui_ui_section_list(self.0.as_ptr(), &desc, &mut out)
            } });
        }
        out
    }

    /// The one body of `grid_list`: keyed by `key`.
    #[track_caller]
    pub(crate) fn grid_list_key(&mut self, key: Key, count: usize, cell_w: f32, cell_h: f32, build: &mut dyn FnMut(&mut Ui, usize)) -> Handle {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let mut out = Handle::default();
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::grid_list(self.0.as_ptr(), label, count, cell_w, cell_h, &mut |raw: *mut crate::sys::PxuiUi, index: usize| { let mut ui = ManuallyDrop::new(Ui(NonNull::new_unchecked(raw))); build(&mut ui, index) }, &mut out) });
        #[cfg(not(feature = "static"))]
        {
            let mut context = GridCallback { f: build };
            let (build, user) = context.erased();
            let desc = GridListDesc { size: core::mem::size_of::<GridListDesc>() as u32, reserved: 0, label: as_str(label), count, cell_w, cell_h, build, user };
            check(self, unsafe { sys::pxui_ui_grid_list(self.0.as_ptr(), &desc, &mut out) });
        }
        out
    }

    /// The one body of `split`: linked statically, the library's trusted entry with the panes as they are; else the split as a
    /// descriptor with the panes as callbacks.
    #[track_caller]
    pub(crate) fn split_key(&mut self, key: crate::Key, ratio: f32, split: &Split, a: &mut dyn FnMut(&mut Ui), b: &mut dyn FnMut(&mut Ui)) -> crate::SplitResponse {
        let Split { axis, min_a, min_b } = *split;
        if axis == Axis::Stack { panic!("pxui: a split is a row or a column") }
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let mut out = pxui_types::ffi::PxuiResponse::EMPTY;
        #[cfg(feature = "static")]
        let at = {
            let (mut moved, mut at) = (false, None);
            check(self, unsafe { pxui_abi_link::trusted::split(self.0.as_ptr(), label, axis, ratio, min_a, min_b, inner!(|ui: &mut Ui| a(ui)), inner!(|ui: &mut Ui| b(ui)), &mut out, &mut moved, &mut at) });
            at
        };
        #[cfg(not(feature = "static"))]
        let at = {
            let (mut context_a, mut context_b) = (Callback { f: a }, Callback { f: b });
            let ((build_a, user_a), (build_b, user_b)) = (context_a.erased(), context_b.erased());
            let desc = SplitDesc { size: core::mem::size_of::<SplitDesc>() as u32, reserved: 0, label: as_str(label), axis: match axis { Axis::Row => 0, _ => 1 }, ratio, min_a, min_b, build_a, user_a, build_b, user_b };
            let (mut has, mut at) = (0u8, 0.0f32);
            check(self, unsafe { sys::pxui_ui_split_result(self.0.as_ptr(), &desc, &mut out, &mut has, &mut at) });
            (has != 0).then_some(at)
        };
        crate::SplitResponse { response: crate::kit_ext::response(&out), ratio: at }
    }

    /// The one body of `menu_bar`: the titles and item texts as the descriptor's spans (a bar of up to 8 menus and 64 items on the
    /// stack, a larger one on the heap); the pick comes back as the flat item index and that item's id.
    #[track_caller]
    pub(crate) fn menu_bar_key(&mut self, key: crate::Key, menus: &[MenuBarMenu<'_>], bar: &MenuBar) -> crate::Chosen {
        let mut id_label = [0u8; 17];
        let label = key.label(&mut id_label);
        let total: usize = menus.iter().map(|menu| menu.items.len()).sum();
        let none = MenuDesc { size: core::mem::size_of::<MenuDesc>() as u32, reserved: 0, title: Str::default(), items: Slice { ptr: core::ptr::null(), len: 0 } };
        let (mut flat_stack, mut menus_stack) = ([Str::default(); 64], [none; 8]);
        let (mut flat_heap, mut menus_heap): (Vec<Str>, Vec<MenuDesc>);
        let (flat, descs): (&mut [Str], &mut [MenuDesc]) = if total <= 64 && menus.len() <= 8 {
            (&mut flat_stack[..total], &mut menus_stack[..menus.len()])
        } else {
            flat_heap = vec![Str::default(); total];
            menus_heap = vec![none; menus.len()];
            (&mut flat_heap[..], &mut menus_heap[..])
        };
        let mut at = 0;
        for (menu, desc) in menus.iter().zip(descs.iter_mut()) {
            for (out, item) in flat[at..at + menu.items.len()].iter_mut().zip(menu.items) { *out = as_str(item.text); }
            *desc = MenuDesc { size: core::mem::size_of::<MenuDesc>() as u32, reserved: 0, title: as_str(menu.title), items: Slice { ptr: flat[at..].as_ptr(), len: menu.items.len() } };
            at += menu.items.len();
        }
        let desc = MenuBarDesc { size: core::mem::size_of::<MenuBarDesc>() as u32, reserved: 0, label: as_str(label), menus: Slice { ptr: descs.as_ptr(), len: descs.len() } };
        let mut out = MenuOut { size: core::mem::size_of::<MenuOut>() as u32, reserved: 0, menu: usize::MAX, item: usize::MAX };
        let mut dismissed = 0u8;
        self.with_variant(pxui_types::theme::BodyKind::menu_bar, bar.variant, &mut |ui| check(ui, unsafe { sys::pxui_ui_menu_bar_result(ui.0.as_ptr(), &desc, &mut out, &mut dismissed) }));
        let picked = menus.get(out.menu).and_then(|menu| Some((menu.items.get(out.item)?, out.item)));
        let chosen = picked.map(|(_, item)| menus[..out.menu].iter().map(|menu| menu.items.len()).sum::<usize>() + item);
        crate::Chosen { chosen, chosen_id: picked.and_then(|(item, _)| item.id), dismissed: dismissed != 0 }
    }

    #[inline]
    #[track_caller] pub fn inspector_row(&mut self, width: Option<f32>, pin: Option<bool>, key: &str, label: &str) -> (Handle, f32, f32, f32) {
        #[cfg(feature = "static")]
        { let mut out = (Handle::default(), 0.0, 0.0, 0.0); check(self, unsafe { pxui_abi_link::trusted::inspector_row(self.0.as_ptr(), width, pin, key, label, &mut out) }); out }
        #[cfg(not(feature = "static"))]
        {
            let mut desc = InspectorRowDesc { size: core::mem::size_of::<InspectorRowDesc>() as u32, has_width: width.is_some() as u8, has_pin: pin.is_some() as u8, pin: pin.unwrap_or(false) as u8, reserved: 0, key: as_str(key), label: as_str(label), width: width.unwrap_or_default(), reserved_tail: 0, row: Handle::default(), control_x: 0.0, control_y: 0.0, control_w: 0.0 };
            check(self, unsafe { sys::pxui_ui_inspector_row(self.0.as_ptr(), &mut desc) });
            (desc.row, desc.control_x, desc.control_y, desc.control_w)
        }
    }

    #[inline]
    #[track_caller] pub fn inspector_section(&mut self, key: &str, expanded: bool, set_expanded: Option<bool>, label: &str, build: &mut dyn FnMut(&mut Ui)) -> bool {
        #[cfg(feature = "static")]
        { let mut out = false; check(self, unsafe { pxui_abi_link::trusted::inspector_section(self.0.as_ptr(), key, expanded, set_expanded, label, inner!(|ui: &mut Ui| build(ui)), &mut out) }); out }
        #[cfg(not(feature = "static"))]
        {
            let mut context = Callback { f: build };
            let (build, user) = context.erased();
            let mut desc = InspectorSectionDesc { size: core::mem::size_of::<InspectorSectionDesc>() as u32, expanded: expanded as u8, has_set: set_expanded.is_some() as u8, set_expanded: set_expanded.unwrap_or(false) as u8, reserved: 0, key: as_str(key), label: as_str(label), build, user, out_open: 0 };
            check(self, unsafe { sys::pxui_ui_inspector_section(self.0.as_ptr(), &mut desc) });
            desc.out_open != 0
        }
    }

    #[inline]
    #[track_caller] pub fn inspector_toggle(&mut self, key: &str, label: &str, value: bool) -> bool {
        #[cfg(feature = "static")]
        { let mut out = false; check(self, unsafe { pxui_abi_link::trusted::inspector_toggle(self.0.as_ptr(), key, label, value, &mut out) }); out }
        #[cfg(not(feature = "static"))]
        { let mut out = 0; check(self, unsafe { sys::pxui_ui_inspector_toggle(self.0.as_ptr(), as_str(key), as_str(label), value as u8, &mut out) }); out != 0 }
    }
    #[inline]
    #[track_caller] pub fn inspector_button(&mut self, key: &str, label: &str) -> bool {
        #[cfg(feature = "static")]
        { let mut out = false; check(self, unsafe { pxui_abi_link::trusted::inspector_button(self.0.as_ptr(), key, label, &mut out) }); out }
        #[cfg(not(feature = "static"))]
        { let mut out = 0; check(self, unsafe { sys::pxui_ui_inspector_button(self.0.as_ptr(), as_str(key), as_str(label), &mut out) }); out != 0 }
    }
    #[inline]
    #[track_caller] pub fn inspector_readout(&mut self, width: Option<f32>, key: &str, label: &str, value: &str)  {
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::inspector_readout(self.0.as_ptr(), width, key, label, value) });
        #[cfg(not(feature = "static"))]
        {
            let desc = InspectorValueDesc { size: core::mem::size_of::<InspectorValueDesc>() as u32, has_width: width.is_some() as u8, reserved: [0; 3], key: as_str(key), label: as_str(label), value: as_str(value), width: width.unwrap_or_default() };
            check(self, unsafe { sys::pxui_ui_inspector_readout(self.0.as_ptr(), &desc) })
        }
    }
    #[inline]
    #[track_caller] pub fn inspector_message(&mut self, key: &str, message: &str)  {
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::inspector_message(self.0.as_ptr(), key, message) });
        #[cfg(not(feature = "static"))]
        {
            let desc = InspectorTextDesc { size: core::mem::size_of::<InspectorTextDesc>() as u32, reserved: 0, key: as_str(key), text: as_str(message) };
            check(self, unsafe { sys::pxui_ui_inspector_message(self.0.as_ptr(), &desc) })
        }
    }

    #[inline]
    #[track_caller] pub fn wants_frame(&self, events: &[pxui_types::Event], now: f64) -> bool {
        let mut out = 0;
        check(self, unsafe { sys::pxui_ui_wants_frame(self.0.as_ptr(), Slice { ptr: events.as_ptr(), len: events.len() }, now, &mut out) });
        out != 0
    }
    #[inline]
    #[track_caller] pub fn footer(&mut self, right: Option<&str>, hints: &[(&str, &str)])  {
        #[cfg(feature = "static")]
        check(self, unsafe { pxui_abi_link::trusted::footer(self.0.as_ptr(), right, hints) });
        #[cfg(not(feature = "static"))]
        {
            let hints = Hints::new(hints);
            let desc = FooterDesc { size: core::mem::size_of::<FooterDesc>() as u32, has_right: right.is_some() as u8, reserved: [0; 3], right: right.map_or_else(Str::default, as_str), hints: hints.span() };
            check(self, unsafe { sys::pxui_ui_footer(self.0.as_ptr(), &desc) })
        }
    }
    /// Runs the keyframes `keys` (1 to 8, times ascending from 0) on the box `box_` as `play` says: how often they repeat (a count includes the reverse legs), whether each cycle plays back, and how far into the first cycle they start.
    /// The run needs a positive duration; hidden runs request no frames. Call `animate_keys` with another run, or `animate`, to stop it. True while it runs; false for a stale handle.
    /// Allocates: never.
    /// Panics: when `keys` is empty or over 8, the count is `Repeat::Count(0)`, or the library refuses the run.
    ///
    /// ```
    /// use pxui::ui::{Curve, Keyframe};
    /// use pxui::{Keyframes, Repeat};
    /// pxui::doc::frames(2, &mut |ui| {
    ///     let b = ui.node("pulse", pxui::Node::DEFAULT);
    ///     let spring = Curve::Spring { omega: 8.0 };
    ///     let keys = [Keyframe::opacity(0.0, 1.0).curve(spring), Keyframe::opacity(0.5, 0.4).curve(spring)];
    ///     ui.animate_keys(&b, &keys, &Keyframes::DEFAULT.repeat(Repeat::Forever).autoreverse());
    ///     ui.animate_keys(&b, &keys[..1], &Keyframes::DEFAULT);
    /// });
    /// ```
    #[track_caller]
    pub fn animate_keys(&mut self, box_: &Handle, keys: &[Keyframe], play: &Keyframes) -> bool {
        assert!(play.cycles() != Repeat::Count(0), "pxui: repeat count must be positive");
        assert!(!keys.is_empty() && keys.len() <= 8, "pxui: animate_keys accepts 1..=8 keyframes");
        let mut descriptors = [KeyframeDesc::default(); 8];
        for (out, key) in descriptors.iter_mut().zip(keys) {
            let (target, values, look) = match key.target() {
                Target::Offset(x, y) => (0, [x, y, 0.0, 0.0], encode_look(Look::NONE)),
                Target::Opacity(value) => (1, [value, 0.0, 0.0, 0.0], encode_look(Look::NONE)),
                Target::Look(value) => (2, [0.0; 4], encode_look(value)),
                Target::Scale { x, y, anchor } => (3, [x, y, anchor.0, anchor.1], encode_look(Look::NONE)),
            };
            *out = KeyframeDesc { size: core::mem::size_of::<KeyframeDesc>() as u32, time: key.time(), target, reserved: 0, values, look, curve: PxuiCurveDesc::from(key.segment()) };
        }
        let mut out = 0;
        let span = Slice { ptr: descriptors.as_ptr(), len: keys.len() };
        let rc = if *play == Keyframes::DEFAULT {
            unsafe { sys::pxui_ui_animate_keys(self.0.as_ptr(), box_, span, &mut out) }
        } else {
            let count = match play.cycles() { Repeat::Count(n) => n.into(), Repeat::Forever => 0 };
            unsafe { sys::pxui_ui_animate_keys_repeat(self.0.as_ptr(), box_, span, count, play.reverses() as u8, play.start(), &mut out) }
        };
        crate::live(self, rc) && out != 0
    }

}
